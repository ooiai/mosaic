use std::io;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use crossterm::execute;
use crossterm::terminal::{self, EnterAlternateScreen, LeaveAlternateScreen};
use mosaic_agent::{AgentEvent, AgentRunOptions, AgentRunner};
use mosaic_core::error::{MosaicError, Result};
use mosaic_core::session::{
    EventKind, SessionEvent, SessionRuntimeMetadata, SessionStore, SessionSummary,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::mpsc;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TuiFocus {
    Messages,
    Input,
    Sessions,
    Inspector,
}

impl TuiFocus {
    fn next(self, inspector_visible: bool) -> Self {
        match (self, inspector_visible) {
            (Self::Messages, _) => Self::Input,
            (Self::Input, _) => Self::Sessions,
            (Self::Sessions, true) => Self::Inspector,
            (Self::Sessions, false) => Self::Messages,
            (Self::Inspector, _) => Self::Messages,
        }
    }
}

#[derive(Clone)]
pub struct TuiRuntime {
    pub agent: AgentRunner,
    pub profile_name: String,
    pub agent_id: Option<String>,
    pub session_metadata: SessionRuntimeMetadata,
    pub policy_summary: String,
    pub switch_agent: Arc<dyn Fn(&str) -> Result<SwitchedTuiRuntime> + Send + Sync>,
    pub load_session_runtime: Arc<dyn Fn(Option<&str>) -> Result<SwitchedTuiRuntime> + Send + Sync>,
    pub list_agents: Arc<dyn Fn() -> Result<Vec<TuiAgentOption>> + Send + Sync>,
}

pub struct SwitchedTuiRuntime {
    pub agent: AgentRunner,
    pub profile_name: String,
    pub agent_id: Option<String>,
    pub session_metadata: SessionRuntimeMetadata,
    pub policy_summary: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TuiAgentOption {
    pub id: String,
    pub name: String,
    pub profile_name: String,
    pub is_default: bool,
    pub route_keys: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct TuiOptions {
    pub initial_session_id: Option<String>,
    pub initial_focus: TuiFocus,
    pub show_inspector: bool,
    pub yes: bool,
    pub cwd: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TuiAction {
    CycleFocus,
    ToggleInspector,
    ToggleHelp,
    NewSession,
    SelectNextSession,
    SelectPrevSession,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ChatLine {
    role: String,
    text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct InspectorLine {
    kind: String,
    detail: String,
}

#[derive(Debug, Clone)]
pub struct TuiState {
    focus: TuiFocus,
    show_inspector: bool,
    show_help: bool,
    show_agent_picker: bool,
    show_session_picker: bool,
    input: String,
    running: bool,
    status: String,
    sessions: Vec<SessionSummary>,
    selected_session: usize,
    agents: Vec<TuiAgentOption>,
    selected_agent: usize,
    active_session_id: Option<String>,
    messages: Vec<ChatLine>,
    inspector: Vec<InspectorLine>,
}

impl TuiState {
    fn new(
        focus: TuiFocus,
        show_inspector: bool,
        sessions: Vec<SessionSummary>,
        active_session_id: Option<String>,
        _profile_name: &str,
        _agent_id: Option<&str>,
        _policy_summary: &str,
    ) -> Self {
        let selected_session = active_session_id
            .as_ref()
            .and_then(|session_id| {
                sessions
                    .iter()
                    .position(|entry| entry.session_id == *session_id)
            })
            .unwrap_or(0);
        Self {
            focus,
            show_inspector,
            show_help: false,
            show_agent_picker: false,
            show_session_picker: false,
            input: String::new(),
            running: false,
            status: "idle".to_string(),
            sessions,
            selected_session,
            agents: Vec::new(),
            selected_agent: 0,
            active_session_id,
            messages: Vec::new(),
            inspector: Vec::new(),
        }
    }

    fn reduce(&mut self, action: TuiAction) {
        match action {
            TuiAction::CycleFocus => {
                self.focus = self.focus.next(self.show_inspector);
            }
            TuiAction::ToggleInspector => {
                self.show_inspector = !self.show_inspector;
                if !self.show_inspector && self.focus == TuiFocus::Inspector {
                    self.focus = TuiFocus::Messages;
                }
            }
            TuiAction::ToggleHelp => {
                self.show_help = !self.show_help;
            }
            TuiAction::NewSession => {
                self.active_session_id = None;
                self.messages.clear();
                self.inspector.clear();
                self.status = "new session".to_string();
            }
            TuiAction::SelectNextSession => {
                if self.sessions.is_empty() {
                    return;
                }
                self.selected_session = (self.selected_session + 1) % self.sessions.len();
            }
            TuiAction::SelectPrevSession => {
                if self.sessions.is_empty() {
                    return;
                }
                if self.selected_session == 0 {
                    self.selected_session = self.sessions.len() - 1;
                } else {
                    self.selected_session -= 1;
                }
            }
        }
    }

    fn refresh_sessions(&mut self, store: &SessionStore) -> Result<()> {
        self.sessions = store.list_sessions()?;
        if self.sessions.is_empty() {
            self.selected_session = 0;
            return Ok(());
        }
        let selected = self
            .active_session_id
            .as_ref()
            .and_then(|session_id| {
                self.sessions
                    .iter()
                    .position(|entry| entry.session_id == *session_id)
            })
            .unwrap_or(0);
        self.selected_session = selected;
        Ok(())
    }

    fn load_session(&mut self, store: &SessionStore, session_id: Option<String>) -> Result<()> {
        self.active_session_id = session_id;
        self.messages.clear();
        self.inspector.clear();
        if let Some(id) = self.active_session_id.clone() {
            let events = store.read_events(&id)?;
            self.apply_persisted_events(&events);
            self.status = format!("resumed session={id}");
        } else {
            self.status = "new session".to_string();
        }
        Ok(())
    }

    fn apply_persisted_events(&mut self, events: &[SessionEvent]) {
        self.messages.clear();
        self.inspector.clear();
        for event in events {
            match event.kind {
                EventKind::User => {
                    if let Some(text) = event.payload.get("text").and_then(|value| value.as_str()) {
                        self.messages.push(ChatLine {
                            role: "user".to_string(),
                            text: text.to_string(),
                        });
                    }
                }
                EventKind::Assistant => {
                    if let Some(text) = event.payload.get("text").and_then(|value| value.as_str()) {
                        self.messages.push(ChatLine {
                            role: "assistant".to_string(),
                            text: text.to_string(),
                        });
                    }
                }
                EventKind::ToolCall => {
                    let name = event
                        .payload
                        .get("name")
                        .and_then(|value| value.as_str())
                        .unwrap_or("tool");
                    self.inspector.push(InspectorLine {
                        kind: "tool_call".to_string(),
                        detail: format!("{name} {}", summarize_json(event.payload.get("args"))),
                    });
                }
                EventKind::ToolResult => {
                    let name = event
                        .payload
                        .get("name")
                        .and_then(|value| value.as_str())
                        .unwrap_or("tool");
                    self.inspector.push(InspectorLine {
                        kind: "tool_result".to_string(),
                        detail: format!("{name} {}", summarize_json(event.payload.get("result"))),
                    });
                }
                EventKind::Error => {
                    self.inspector.push(InspectorLine {
                        kind: "error".to_string(),
                        detail: summarize_json(event.payload.get("message")),
                    });
                }
                EventKind::System => {}
            }
        }
    }

    fn apply_agent_event(&mut self, event: AgentEvent) {
        match event {
            AgentEvent::User { session_id, text } => {
                self.active_session_id = Some(session_id.clone());
                self.messages.push(ChatLine {
                    role: "user".to_string(),
                    text,
                });
                self.status = format!("running session={session_id}");
            }
            AgentEvent::Assistant { session_id, text } => {
                self.active_session_id = Some(session_id.clone());
                self.messages.push(ChatLine {
                    role: "assistant".to_string(),
                    text,
                });
                self.status = format!("assistant replied session={session_id}");
            }
            AgentEvent::ToolCall {
                session_id,
                name,
                args,
            } => {
                self.active_session_id = Some(session_id.clone());
                self.inspector.push(InspectorLine {
                    kind: "tool_call".to_string(),
                    detail: format!(
                        "[{}] {name} {}",
                        short_id(&session_id),
                        summarize_json(Some(&args))
                    ),
                });
                self.status = format!("running tool={name}");
            }
            AgentEvent::ToolResult {
                session_id,
                name,
                result,
            } => {
                self.active_session_id = Some(session_id);
                self.inspector.push(InspectorLine {
                    kind: "tool_result".to_string(),
                    detail: format!(
                        "[{}] {name} {}",
                        short_id(self.active_session_id.as_deref().unwrap_or_default()),
                        summarize_json(Some(&result))
                    ),
                });
            }
            AgentEvent::Error {
                session_id,
                message,
            } => {
                self.active_session_id = Some(session_id);
                self.inspector.push(InspectorLine {
                    kind: "error".to_string(),
                    detail: format!(
                        "[{}] {message}",
                        short_id(self.active_session_id.as_deref().unwrap_or_default())
                    ),
                });
            }
        }
        const MAX_INSPECTOR: usize = 200;
        if self.inspector.len() > MAX_INSPECTOR {
            let overflow = self.inspector.len() - MAX_INSPECTOR;
            self.inspector.drain(0..overflow);
        }
    }
}

#[derive(Debug)]
enum AppEvent {
    Agent(AgentEvent),
    AskDone(Result<mosaic_agent::AgentRunResult>),
}

pub async fn run_tui(runtime: TuiRuntime, options: TuiOptions) -> Result<()> {
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen).map_err(map_io)?;
    terminal::enable_raw_mode().map_err(map_io)?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend).map_err(map_io)?;
    let app_result = run_app(&mut terminal, runtime, options).await;

    terminal::disable_raw_mode().map_err(map_io)?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen).map_err(map_io)?;
    terminal.show_cursor().map_err(map_io)?;

    app_result
}

async fn run_app(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    mut runtime: TuiRuntime,
    options: TuiOptions,
) -> Result<()> {
    let session_store = runtime.agent.session_store().clone();
    session_store.ensure_dirs()?;

    let mut sessions = session_store.list_sessions()?;
    let mut active_session_id = options.initial_session_id.clone();
    if active_session_id.is_none() {
        active_session_id = session_store.latest_session_id()?;
    }

    let mut state = TuiState::new(
        options.initial_focus,
        options.show_inspector,
        std::mem::take(&mut sessions),
        active_session_id.clone(),
        &runtime.profile_name,
        runtime.agent_id.as_deref(),
        &runtime.policy_summary,
    );

    if let Some(session_id) = active_session_id {
        state.load_session(&session_store, Some(session_id))?;
    }

    let (app_tx, mut app_rx) = mpsc::unbounded_channel::<AppEvent>();

    let mut should_exit = false;
    while !should_exit {
        while let Ok(event) = app_rx.try_recv() {
            match event {
                AppEvent::Agent(agent_event) => {
                    state.apply_agent_event(agent_event);
                }
                AppEvent::AskDone(result) => {
                    state.running = false;
                    match result {
                        Ok(run) => {
                            state.active_session_id = Some(run.session_id);
                            state.status = "completed".to_string();
                            state.refresh_sessions(&session_store)?;
                        }
                        Err(err) => {
                            state.status = format!("error: {}", err);
                            state.inspector.push(InspectorLine {
                                kind: "error".to_string(),
                                detail: err.to_string(),
                            });
                        }
                    }
                }
            }
        }

        terminal
            .draw(|frame| {
                render(
                    frame,
                    &state,
                    runtime.agent_id.as_deref(),
                    &runtime.profile_name,
                    &runtime.policy_summary,
                )
            })
            .map_err(map_io)?;

        if event::poll(Duration::from_millis(40)).map_err(map_io)? {
            let Event::Key(key) = event::read().map_err(map_io)? else {
                continue;
            };
            if handle_key(
                &mut state,
                &session_store,
                key,
                &mut runtime,
                &options,
                &app_tx,
            )
            .await?
            {
                should_exit = true;
            }
        }
    }

    Ok(())
}

async fn handle_key(
    state: &mut TuiState,
    session_store: &SessionStore,
    key: KeyEvent,
    runtime: &mut TuiRuntime,
    options: &TuiOptions,
    app_tx: &mpsc::UnboundedSender<AppEvent>,
) -> Result<bool> {
    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
        return Ok(true);
    }

    if state.show_help {
        if key.code == KeyCode::Esc || key.code == KeyCode::Char('?') {
            state.reduce(TuiAction::ToggleHelp);
        }
        return Ok(false);
    }

    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('a') {
        toggle_agent_picker(state, runtime)?;
        return Ok(false);
    }

    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('s') {
        toggle_session_picker(state, session_store)?;
        return Ok(false);
    }

    if state.show_agent_picker {
        match key.code {
            KeyCode::Esc => {
                state.show_agent_picker = false;
            }
            KeyCode::Down => {
                if !state.agents.is_empty() {
                    state.selected_agent = (state.selected_agent + 1) % state.agents.len();
                }
            }
            KeyCode::Up => {
                if !state.agents.is_empty() {
                    if state.selected_agent == 0 {
                        state.selected_agent = state.agents.len() - 1;
                    } else {
                        state.selected_agent -= 1;
                    }
                }
            }
            KeyCode::Enter => {
                if let Some(agent) = state.agents.get(state.selected_agent).cloned() {
                    state.show_agent_picker = false;
                    switch_active_agent(state, runtime, &agent.id)?;
                }
            }
            _ => {}
        }
        return Ok(false);
    }

    if state.show_session_picker {
        match key.code {
            KeyCode::Esc => {
                state.show_session_picker = false;
            }
            KeyCode::Down => state.reduce(TuiAction::SelectNextSession),
            KeyCode::Up => state.reduce(TuiAction::SelectPrevSession),
            KeyCode::Enter => {
                if let Some(entry) = state.sessions.get(state.selected_session) {
                    let session_id = entry.session_id.clone();
                    state.show_session_picker = false;
                    load_selected_session(state, session_store, runtime, &session_id)?;
                }
            }
            _ => {}
        }
        return Ok(false);
    }

    if (key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('i'))
        || (key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Tab)
    {
        state.reduce(TuiAction::ToggleInspector);
        return Ok(false);
    }
    if key.code == KeyCode::Tab {
        state.reduce(TuiAction::CycleFocus);
        return Ok(false);
    }
    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('n') {
        state.reduce(TuiAction::NewSession);
        return Ok(false);
    }
    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('r') {
        state.refresh_sessions(session_store)?;
        state.status = "session list refreshed".to_string();
        return Ok(false);
    }
    if key.code == KeyCode::Char('?') {
        state.reduce(TuiAction::ToggleHelp);
        return Ok(false);
    }
    if key.code == KeyCode::Char('q') && key.modifiers.is_empty() {
        return Ok(true);
    }

    match state.focus {
        TuiFocus::Sessions => match key.code {
            KeyCode::Down => state.reduce(TuiAction::SelectNextSession),
            KeyCode::Up => state.reduce(TuiAction::SelectPrevSession),
            KeyCode::Enter => {
                if let Some(entry) = state.sessions.get(state.selected_session) {
                    let session_id = entry.session_id.clone();
                    load_selected_session(state, session_store, runtime, &session_id)?;
                }
            }
            _ => {}
        },
        TuiFocus::Input => match key.code {
            KeyCode::Char('j') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                state.input.push('\n');
            }
            KeyCode::Backspace => {
                state.input.pop();
            }
            KeyCode::Enter => {
                if !state.running {
                    let prompt = state.input.trim().to_string();
                    if !prompt.is_empty() {
                        state.input.clear();
                        if let Some(command) = parse_input_command(&prompt) {
                            handle_input_command(state, session_store, runtime, command)?;
                        } else {
                            state.running = true;
                            state.status = "running".to_string();
                            spawn_agent_task(state, runtime, options, app_tx, prompt);
                        }
                    }
                }
            }
            KeyCode::Char(ch) => {
                if key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT {
                    state.input.push(ch);
                }
            }
            _ => {}
        },
        TuiFocus::Messages | TuiFocus::Inspector => {}
    }

    Ok(false)
}

fn load_selected_session(
    state: &mut TuiState,
    session_store: &SessionStore,
    runtime: &mut TuiRuntime,
    session_id: &str,
) -> Result<()> {
    let switched = match (runtime.load_session_runtime)(Some(session_id)) {
        Ok(switched) => switched,
        Err(err) => {
            state.status = format!("error: {err}");
            state.inspector.push(InspectorLine {
                kind: "error".to_string(),
                detail: err.to_string(),
            });
            return Ok(());
        }
    };
    runtime.agent = switched.agent;
    runtime.profile_name = switched.profile_name;
    runtime.agent_id = switched.agent_id;
    runtime.session_metadata = switched.session_metadata;
    runtime.policy_summary = switched.policy_summary;
    state.load_session(session_store, Some(session_id.to_string()))?;
    let active_agent = runtime.agent_id.as_deref().unwrap_or("<none>");
    state.status = format!("resumed session={session_id} | agent={active_agent}");
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TuiInputCommand<'a> {
    Agent,
    Agents,
    AgentSet(&'a str),
    Session,
    SessionSet(&'a str),
    NewSession,
    Status,
}

fn parse_input_command(input: &str) -> Option<TuiInputCommand<'_>> {
    if let Some(rest) = input.strip_prefix("/agent") {
        if rest.is_empty() {
            return Some(TuiInputCommand::Agent);
        }
        if rest.chars().next().is_some_and(char::is_whitespace) {
            let requested = rest.trim();
            if requested.is_empty() {
                return Some(TuiInputCommand::Agent);
            }
            return Some(TuiInputCommand::AgentSet(requested));
        }
    }
    if input == "/agents" {
        return Some(TuiInputCommand::Agents);
    }
    if let Some(rest) = input.strip_prefix("/session") {
        if rest.is_empty() {
            return Some(TuiInputCommand::Session);
        }
        if rest.chars().next().is_some_and(char::is_whitespace) {
            let requested = rest.trim();
            if requested.is_empty() {
                return Some(TuiInputCommand::Session);
            }
            return Some(TuiInputCommand::SessionSet(requested));
        }
    }
    if input == "/new" {
        return Some(TuiInputCommand::NewSession);
    }
    if input == "/status" {
        return Some(TuiInputCommand::Status);
    }
    None
}

fn handle_input_command(
    state: &mut TuiState,
    session_store: &SessionStore,
    runtime: &mut TuiRuntime,
    command: TuiInputCommand<'_>,
) -> Result<()> {
    match command {
        TuiInputCommand::Agent => {
            state.status = format!("agent={}", runtime.agent_id.as_deref().unwrap_or("<none>"));
        }
        TuiInputCommand::Agents => {
            toggle_agent_picker(state, runtime)?;
        }
        TuiInputCommand::AgentSet(agent_id) => {
            switch_active_agent(state, runtime, agent_id)?;
        }
        TuiInputCommand::Session => {
            state.status = format!(
                "session={}",
                state.active_session_id.as_deref().unwrap_or("<new>")
            );
        }
        TuiInputCommand::SessionSet(session_id) => {
            load_selected_session(state, session_store, runtime, session_id)?;
        }
        TuiInputCommand::NewSession => {
            state.reduce(TuiAction::NewSession);
        }
        TuiInputCommand::Status => {
            state.status = compose_status_line(
                "runtime",
                &runtime.profile_name,
                runtime.agent_id.as_deref(),
                state.active_session_id.as_deref(),
                &runtime.policy_summary,
            );
        }
    }
    Ok(())
}

fn toggle_agent_picker(state: &mut TuiState, runtime: &TuiRuntime) -> Result<()> {
    if state.show_agent_picker {
        state.show_agent_picker = false;
        return Ok(());
    }

    let agents = (runtime.list_agents)()?;
    if agents.is_empty() {
        state.status = "no configured agents".to_string();
        return Ok(());
    }
    state.selected_agent = runtime
        .agent_id
        .as_ref()
        .and_then(|active| agents.iter().position(|entry| &entry.id == active))
        .unwrap_or(0);
    state.agents = agents;
    state.show_session_picker = false;
    state.show_agent_picker = true;
    state.status = "agent picker".to_string();
    Ok(())
}

fn toggle_session_picker(state: &mut TuiState, session_store: &SessionStore) -> Result<()> {
    if state.show_session_picker {
        state.show_session_picker = false;
        return Ok(());
    }

    state.refresh_sessions(session_store)?;
    if state.sessions.is_empty() {
        state.status = "no sessions".to_string();
        return Ok(());
    }
    state.show_agent_picker = false;
    state.show_session_picker = true;
    state.status = "session picker".to_string();
    Ok(())
}

fn switch_active_agent(
    state: &mut TuiState,
    runtime: &mut TuiRuntime,
    agent_id: &str,
) -> Result<()> {
    if runtime.agent_id.as_deref() == Some(agent_id) {
        state.status = format!("agent unchanged={agent_id}");
        return Ok(());
    }

    let switched = match (runtime.switch_agent)(agent_id) {
        Ok(switched) => switched,
        Err(err) => {
            state.status = format!("error: {err}");
            state.inspector.push(InspectorLine {
                kind: "error".to_string(),
                detail: err.to_string(),
            });
            return Ok(());
        }
    };

    let had_session = state.active_session_id.is_some();
    if had_session {
        state.reduce(TuiAction::NewSession);
    }
    runtime.agent = switched.agent;
    runtime.profile_name = switched.profile_name;
    runtime.agent_id = switched.agent_id;
    runtime.session_metadata = switched.session_metadata;
    runtime.policy_summary = switched.policy_summary;

    let active_agent = runtime.agent_id.as_deref().unwrap_or("<none>");
    state.status = if had_session {
        format!("agent switched={active_agent} | session reset")
    } else {
        format!("agent switched={active_agent}")
    };
    state.inspector.push(InspectorLine {
        kind: "agent".to_string(),
        detail: if had_session {
            format!("switched to {active_agent}; new session started")
        } else {
            format!("switched to {active_agent}")
        },
    });
    Ok(())
}

fn spawn_agent_task(
    state: &TuiState,
    runtime: &TuiRuntime,
    options: &TuiOptions,
    app_tx: &mpsc::UnboundedSender<AppEvent>,
    prompt: String,
) {
    let tx_done = app_tx.clone();
    let tx_events = app_tx.clone();
    let runner = runtime.agent.clone();
    let cwd = options.cwd.clone();
    let yes = options.yes;
    let session_id = state.active_session_id.clone();
    let session_metadata = runtime.session_metadata.clone();
    tokio::spawn(async move {
        let callback = Arc::new(move |event: AgentEvent| {
            let _ = tx_events.send(AppEvent::Agent(event));
        });
        let result = runner
            .ask(
                &prompt,
                AgentRunOptions {
                    session_id,
                    session_metadata,
                    cwd,
                    yes,
                    interactive: true,
                    event_callback: Some(callback),
                },
            )
            .await;
        let _ = tx_done.send(AppEvent::AskDone(result));
    });
}

fn render(
    frame: &mut ratatui::Frame,
    state: &TuiState,
    agent_id: Option<&str>,
    profile_name: &str,
    policy_summary: &str,
) {
    let area = frame.area();
    let show_inspector = state.show_inspector && area.width >= 120;
    let compact = area.width < 80;

    if compact {
        render_compact(frame, state, area, agent_id, profile_name, policy_summary);
    } else {
        render_wide(
            frame,
            state,
            area,
            show_inspector,
            agent_id,
            profile_name,
            policy_summary,
        );
    }

    if state.show_help {
        render_help_overlay(frame, area);
    }

    if state.show_agent_picker {
        render_agent_picker(frame, state, area);
    }

    if state.show_session_picker {
        render_session_picker(frame, state, area);
    }

    if state.focus == TuiFocus::Input {
        let cursor = input_cursor(
            area,
            compact,
            state.show_inspector && area.width >= 120,
            &state.input,
        );
        frame.set_cursor_position(cursor);
    }
}

fn render_compact(
    frame: &mut ratatui::Frame,
    state: &TuiState,
    area: Rect,
    agent_id: Option<&str>,
    profile_name: &str,
    policy_summary: &str,
) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(6),
            Constraint::Length(4),
            Constraint::Length(1),
        ])
        .split(area);

    let agent = agent_id.unwrap_or("<none>");
    let session = state.active_session_id.as_deref().unwrap_or("<new>");
    let header = Paragraph::new(format!(
        "profile={profile_name} | agent={agent} | policy={policy_summary} | session={session}"
    ))
    .style(Style::default().fg(Color::Cyan));
    frame.render_widget(header, chunks[0]);

    render_messages(frame, state, chunks[1], true);
    render_input(frame, state, chunks[2], true);
    render_status(
        frame,
        state,
        chunks[3],
        profile_name,
        agent_id,
        policy_summary,
    );
}

fn render_wide(
    frame: &mut ratatui::Frame,
    state: &TuiState,
    area: Rect,
    show_inspector: bool,
    agent_id: Option<&str>,
    profile_name: &str,
    policy_summary: &str,
) {
    let columns = if show_inspector {
        Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Length(28),
                Constraint::Min(50),
                Constraint::Length(34),
            ])
            .split(area)
    } else {
        Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Length(28), Constraint::Min(50)])
            .split(area)
    };

    render_sessions(
        frame,
        state,
        columns[0],
        agent_id,
        profile_name,
        policy_summary,
    );

    let center = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(6),
            Constraint::Length(4),
            Constraint::Length(1),
        ])
        .split(columns[1]);
    render_messages(frame, state, center[0], false);
    render_input(frame, state, center[1], false);
    render_status(
        frame,
        state,
        center[2],
        profile_name,
        agent_id,
        policy_summary,
    );

    if show_inspector {
        render_inspector(frame, state, columns[2]);
    }
}

fn render_sessions(
    frame: &mut ratatui::Frame,
    state: &TuiState,
    area: Rect,
    agent_id: Option<&str>,
    profile_name: &str,
    policy_summary: &str,
) {
    let title = format!(
        "sessions | profile={} | agent={} | policy={}",
        profile_name,
        agent_id.unwrap_or("<none>"),
        policy_summary,
    );
    let items = if state.sessions.is_empty() {
        vec![ListItem::new("<no sessions>")]
    } else {
        state
            .sessions
            .iter()
            .map(|entry| {
                let marker = if state
                    .active_session_id
                    .as_ref()
                    .is_some_and(|active| active == &entry.session_id)
                {
                    "*"
                } else {
                    " "
                };
                let runtime = entry.runtime.as_ref().map_or_else(
                    || "<unknown> / <none>".to_string(),
                    |runtime| {
                        format!(
                            "{} / {}",
                            runtime.profile_name,
                            runtime.agent_id.as_deref().unwrap_or("<none>")
                        )
                    },
                );
                ListItem::new(vec![
                    Line::raw(format!(
                        "{marker} {} ({})",
                        short_id(&entry.session_id),
                        entry.event_count
                    )),
                    Line::styled(format!("  {runtime}"), Style::default().fg(Color::Gray)),
                ])
            })
            .collect::<Vec<_>>()
    };

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(border_style(state.focus == TuiFocus::Sessions));
    let list = List::new(items).block(block).highlight_style(
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD),
    );
    let mut list_state = ListState::default();
    if !state.sessions.is_empty() {
        list_state.select(Some(state.selected_session.min(state.sessions.len() - 1)));
    }
    frame.render_stateful_widget(list, area, &mut list_state);
}

fn render_messages(frame: &mut ratatui::Frame, state: &TuiState, area: Rect, compact: bool) {
    let mut lines = Vec::new();
    for entry in &state.messages {
        let role_style = match entry.role.as_str() {
            "user" => Style::default().fg(Color::LightBlue),
            "assistant" => Style::default().fg(Color::LightGreen),
            _ => Style::default().fg(Color::Gray),
        };
        lines.push(Line::from(vec![
            Span::styled(format!("{}> ", entry.role), role_style),
            Span::raw(entry.text.clone()),
        ]));
        lines.push(Line::raw(""));
    }
    if lines.is_empty() {
        lines.push(Line::raw(
            "No messages yet. Press Tab to focus input and hit Enter to send.",
        ));
    }

    let title = if compact {
        "messages"
    } else {
        "message stream"
    };
    let paragraph = Paragraph::new(Text::from(lines))
        .block(
            Block::default()
                .title(title)
                .borders(Borders::ALL)
                .border_style(border_style(state.focus == TuiFocus::Messages)),
        )
        .wrap(Wrap { trim: false });
    frame.render_widget(paragraph, area);
}

fn render_input(frame: &mut ratatui::Frame, state: &TuiState, area: Rect, compact: bool) {
    let title = if state.running {
        "input (running...)"
    } else if compact {
        "input"
    } else {
        "input (Enter send, Ctrl+J newline)"
    };
    let input = Paragraph::new(state.input.as_str())
        .block(
            Block::default()
                .title(title)
                .borders(Borders::ALL)
                .border_style(border_style(state.focus == TuiFocus::Input)),
        )
        .wrap(Wrap { trim: false });
    frame.render_widget(input, area);
}

fn render_status(
    frame: &mut ratatui::Frame,
    state: &TuiState,
    area: Rect,
    profile_name: &str,
    agent_id: Option<&str>,
    policy_summary: &str,
) {
    let status = Paragraph::new(compose_status_line(
        &state.status,
        profile_name,
        agent_id,
        state.active_session_id.as_deref(),
        policy_summary,
    ))
    .style(Style::default().fg(Color::Gray));
    frame.render_widget(status, area);
}

fn compose_status_line(
    detail: &str,
    profile_name: &str,
    agent_id: Option<&str>,
    session_id: Option<&str>,
    policy_summary: &str,
) -> String {
    format!(
        "{} | profile={} | agent={} | session={} | policy={}",
        detail,
        profile_name,
        agent_id.unwrap_or("<none>"),
        session_id.unwrap_or("<new>"),
        policy_summary
    )
}

fn render_inspector(frame: &mut ratatui::Frame, state: &TuiState, area: Rect) {
    let items = if state.inspector.is_empty() {
        vec![ListItem::new("No tool events.")]
    } else {
        state
            .inspector
            .iter()
            .rev()
            .take(40)
            .map(|entry| ListItem::new(format!("{}: {}", entry.kind, entry.detail)))
            .collect::<Vec<_>>()
    };

    let block = Block::default()
        .title("inspector")
        .borders(Borders::ALL)
        .border_style(border_style(state.focus == TuiFocus::Inspector));
    let list = List::new(items).block(block);
    frame.render_widget(list, area);
}

fn render_help_overlay(frame: &mut ratatui::Frame, area: Rect) {
    let popup = centered_rect(75, 70, area);
    let help = vec![
        Line::from("Keyboard shortcuts"),
        Line::from(""),
        Line::from("Enter        send message"),
        Line::from("Ctrl+J       insert newline"),
        Line::from("Ctrl+A       open agent picker"),
        Line::from("Ctrl+S       open session picker"),
        Line::from("Tab          switch focus"),
        Line::from("Ctrl+N       new session"),
        Line::from("Ctrl+R       refresh sessions"),
        Line::from("Ctrl+I       toggle inspector"),
        Line::from("/agents      open agent picker in input"),
        Line::from("/agent ID    switch active agent in input"),
        Line::from("/session ID  resume a session by id"),
        Line::from("/new         start a fresh session"),
        Line::from("/status      print active runtime summary"),
        Line::from("?            toggle this help"),
        Line::from("q / Ctrl+C   quit"),
        Line::from(""),
        Line::from("When terminal is narrow, inspector is automatically hidden."),
    ];

    frame.render_widget(Clear, popup);
    let widget = Paragraph::new(help)
        .block(
            Block::default()
                .title("help")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Yellow)),
        )
        .wrap(Wrap { trim: false });
    frame.render_widget(widget, popup);
}

fn render_agent_picker(frame: &mut ratatui::Frame, state: &TuiState, area: Rect) {
    let popup = centered_rect(55, 55, area);
    let items = state
        .agents
        .iter()
        .map(|entry| {
            let mut details = vec![format!("profile={}", entry.profile_name)];
            if entry.is_default {
                details.push("default".to_string());
            }
            if !entry.route_keys.is_empty() {
                details.push(format!("routes={}", entry.route_keys.join(",")));
            }
            ListItem::new(vec![
                Line::raw(format!("{} ({})", entry.id, entry.name)),
                Line::styled(
                    format!("  {}", details.join(" | ")),
                    Style::default().fg(Color::Gray),
                ),
            ])
        })
        .collect::<Vec<_>>();

    frame.render_widget(Clear, popup);
    let list = List::new(items)
        .block(
            Block::default()
                .title("agent picker")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Yellow)),
        )
        .highlight_style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        );
    let mut stateful = ListState::default();
    if !state.agents.is_empty() {
        stateful.select(Some(state.selected_agent.min(state.agents.len() - 1)));
    }
    frame.render_stateful_widget(list, popup, &mut stateful);
}

fn render_session_picker(frame: &mut ratatui::Frame, state: &TuiState, area: Rect) {
    let popup = centered_rect(62, 60, area);
    let items = if state.sessions.is_empty() {
        vec![ListItem::new("<no sessions>")]
    } else {
        state
            .sessions
            .iter()
            .map(|entry| {
                let runtime = entry.runtime.as_ref().map_or_else(
                    || "<unknown> / <none>".to_string(),
                    |runtime| {
                        format!(
                            "{} / {}",
                            runtime.profile_name,
                            runtime.agent_id.as_deref().unwrap_or("<none>")
                        )
                    },
                );
                ListItem::new(vec![
                    Line::raw(format!(
                        "{} ({})",
                        short_id(&entry.session_id),
                        entry.event_count
                    )),
                    Line::styled(format!("  {runtime}"), Style::default().fg(Color::Gray)),
                ])
            })
            .collect::<Vec<_>>()
    };

    frame.render_widget(Clear, popup);
    let list = List::new(items)
        .block(
            Block::default()
                .title("session picker")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Yellow)),
        )
        .highlight_style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        );
    let mut stateful = ListState::default();
    if !state.sessions.is_empty() {
        stateful.select(Some(state.selected_session.min(state.sessions.len() - 1)));
    }
    frame.render_stateful_widget(list, popup, &mut stateful);
}

fn centered_rect(width_percent: u16, height_percent: u16, area: Rect) -> Rect {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - height_percent) / 2),
            Constraint::Percentage(height_percent),
            Constraint::Percentage((100 - height_percent) / 2),
        ])
        .split(area);
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - width_percent) / 2),
            Constraint::Percentage(width_percent),
            Constraint::Percentage((100 - width_percent) / 2),
        ])
        .split(vertical[1])[1]
}

fn border_style(active: bool) -> Style {
    if active {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::DarkGray)
    }
}

fn input_cursor(area: Rect, compact: bool, show_inspector: bool, input: &str) -> (u16, u16) {
    let base_x = if compact { area.x + 1 } else { area.x + 29 + 1 };
    let base_y = if compact {
        area.y + area.height - 3
    } else {
        area.y + area.height - 2
    };

    let rows = input.lines().collect::<Vec<_>>();
    let row_offset = rows.len().saturating_sub(1) as u16;
    let col_offset = rows
        .last()
        .map(|line| line.chars().count() as u16)
        .unwrap_or(0);

    let max_x = if compact {
        area.x + area.width.saturating_sub(2)
    } else if show_inspector {
        area.x + area.width.saturating_sub(36)
    } else {
        area.x + area.width.saturating_sub(2)
    };
    let x = base_x.saturating_add(col_offset).min(max_x);
    let y = base_y.saturating_add(row_offset);
    (x, y)
}

fn short_id(session_id: &str) -> String {
    if session_id.len() <= 8 {
        return session_id.to_string();
    }
    session_id[..8].to_string()
}

fn summarize_json(value: Option<&Value>) -> String {
    let Some(value) = value else {
        return "{}".to_string();
    };
    let rendered = match value {
        Value::String(text) => text.clone(),
        _ => value.to_string(),
    };
    if rendered.len() <= 120 {
        return rendered;
    }
    format!("{}...", &rendered[..120])
}

fn map_io(err: impl std::fmt::Display) -> MosaicError {
    MosaicError::Io(err.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state_with_sessions(show_inspector: bool) -> TuiState {
        TuiState::new(
            TuiFocus::Messages,
            show_inspector,
            vec![
                SessionSummary {
                    session_id: "session-a".to_string(),
                    event_count: 2,
                    last_updated: None,
                    runtime: Some(SessionRuntimeMetadata {
                        agent_id: Some("writer".to_string()),
                        profile_name: "default".to_string(),
                    }),
                },
                SessionSummary {
                    session_id: "session-b".to_string(),
                    event_count: 3,
                    last_updated: None,
                    runtime: Some(SessionRuntimeMetadata {
                        agent_id: Some("reviewer".to_string()),
                        profile_name: "default".to_string(),
                    }),
                },
            ],
            Some("session-a".to_string()),
            "default",
            Some("writer"),
            "confirm_dangerous",
        )
    }

    #[test]
    fn reducer_cycles_focus_across_visible_panes() {
        let mut state = state_with_sessions(true);
        assert_eq!(state.focus, TuiFocus::Messages);

        state.reduce(TuiAction::CycleFocus);
        assert_eq!(state.focus, TuiFocus::Input);

        state.reduce(TuiAction::CycleFocus);
        assert_eq!(state.focus, TuiFocus::Sessions);

        state.reduce(TuiAction::CycleFocus);
        assert_eq!(state.focus, TuiFocus::Inspector);

        state.reduce(TuiAction::CycleFocus);
        assert_eq!(state.focus, TuiFocus::Messages);
    }

    #[test]
    fn reducer_toggle_inspector_updates_focus() {
        let mut state = state_with_sessions(true);
        state.focus = TuiFocus::Inspector;
        state.reduce(TuiAction::ToggleInspector);

        assert!(!state.show_inspector);
        assert_eq!(state.focus, TuiFocus::Messages);
    }

    #[test]
    fn reducer_toggles_help_overlay() {
        let mut state = state_with_sessions(true);
        assert!(!state.show_help);

        state.reduce(TuiAction::ToggleHelp);
        assert!(state.show_help);

        state.reduce(TuiAction::ToggleHelp);
        assert!(!state.show_help);
    }

    #[test]
    fn reducer_session_navigation_wraps() {
        let mut state = state_with_sessions(true);
        assert_eq!(state.selected_session, 0);

        state.reduce(TuiAction::SelectPrevSession);
        assert_eq!(state.selected_session, 1);

        state.reduce(TuiAction::SelectNextSession);
        assert_eq!(state.selected_session, 0);
    }

    #[test]
    fn agent_event_bridge_populates_inspector_and_messages() {
        let mut state = state_with_sessions(true);

        state.apply_agent_event(AgentEvent::ToolCall {
            session_id: "session-a".to_string(),
            name: "read_file".to_string(),
            args: serde_json::json!({ "path": "README.md" }),
        });
        state.apply_agent_event(AgentEvent::ToolResult {
            session_id: "session-a".to_string(),
            name: "read_file".to_string(),
            result: serde_json::json!({ "content": "ok" }),
        });
        state.apply_agent_event(AgentEvent::Assistant {
            session_id: "session-a".to_string(),
            text: "done".to_string(),
        });

        assert_eq!(state.messages.len(), 1);
        assert_eq!(state.messages[0].role, "assistant");
        assert_eq!(state.inspector.len(), 2);
        assert_eq!(state.inspector[0].kind, "tool_call");
        assert_eq!(state.inspector[1].kind, "tool_result");
    }

    #[test]
    fn state_status_includes_policy_summary() {
        let state = state_with_sessions(true);
        let rendered = compose_status_line(
            &state.status,
            "default",
            Some("writer"),
            state.active_session_id.as_deref(),
            "confirm_dangerous",
        );
        assert!(rendered.contains("policy=confirm_dangerous"));
        assert!(rendered.contains("agent=writer"));
        assert!(rendered.contains("session=session-a"));
    }

    #[test]
    fn parse_input_command_recognizes_agent_switch() {
        assert_eq!(parse_input_command("/agent"), Some(TuiInputCommand::Agent));
        assert_eq!(
            parse_input_command("/agents"),
            Some(TuiInputCommand::Agents)
        );
        assert_eq!(
            parse_input_command("/agent writer"),
            Some(TuiInputCommand::AgentSet("writer"))
        );
        assert_eq!(
            parse_input_command("/agent   reviewer"),
            Some(TuiInputCommand::AgentSet("reviewer"))
        );
        assert_eq!(
            parse_input_command("/session"),
            Some(TuiInputCommand::Session)
        );
        assert_eq!(
            parse_input_command("/session abc123"),
            Some(TuiInputCommand::SessionSet("abc123"))
        );
        assert_eq!(
            parse_input_command("/new"),
            Some(TuiInputCommand::NewSession)
        );
        assert_eq!(
            parse_input_command("/status"),
            Some(TuiInputCommand::Status)
        );
        assert_eq!(parse_input_command("hello"), None);
    }
}
