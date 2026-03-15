use std::io;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use crossterm::event::{self, Event};
use crossterm::execute;
use crossterm::terminal::{self, EnterAlternateScreen, LeaveAlternateScreen};
use mosaic_agent::{AgentEvent, AgentRunOptions, AgentRunner};
use mosaic_core::error::{MosaicError, Result};
use mosaic_core::session::SessionRuntimeMetadata;
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::mpsc;

mod commands;
mod events;
mod keys;
mod pickers;
mod render;
mod state;

use events::AppEvent;
use keys::handle_key;
use render::render;
use state::{InspectorLine, TuiState};

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

    fn prev(self, inspector_visible: bool) -> Self {
        match (self, inspector_visible) {
            (Self::Messages, true) => Self::Inspector,
            (Self::Messages, false) => Self::Sessions,
            (Self::Input, _) => Self::Messages,
            (Self::Sessions, _) => Self::Input,
            (Self::Inspector, _) => Self::Sessions,
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
    let cwd_display = options.cwd.display().to_string();

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
                    &cwd_display,
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
    use crate::commands::{TuiInputCommand, parse_input_command};
    use crate::render::{
        command_palette_items, compose_header_line, compose_shortcuts_line, compose_status_line,
    };
    use crate::state::TuiAction;
    use mosaic_core::session::SessionSummary;

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
    fn header_line_mentions_quick_actions() {
        let rendered = compose_header_line(
            false,
            TuiFocus::Input,
            "default",
            Some("writer"),
            "confirm_dangerous",
        );
        assert!(rendered.contains("Mosaic CLI"));
        assert!(rendered.contains("/agent"));
        assert!(rendered.contains("focus=input"));
    }

    #[test]
    fn shortcuts_line_mentions_core_shortcuts() {
        let rendered = compose_shortcuts_line(false, true);
        assert!(rendered.contains("Ctrl+A"));
        assert!(rendered.contains("Ctrl+S"));
        assert!(rendered.contains("Shift+Tab"));
        assert!(rendered.contains("q quit"));
    }

    #[test]
    fn command_palette_filters_slash_commands() {
        let all = command_palette_items("/");
        assert!(!all.is_empty());

        let agent_only = command_palette_items("/agent writer");
        assert!(
            agent_only
                .iter()
                .any(|(command, _)| *command == "/agent <id>")
        );
        assert!(agent_only.iter().any(|(command, _)| *command == "/agents"));

        let status_only = command_palette_items("/status");
        assert_eq!(
            status_only,
            vec![("/status", "print the active runtime summary")]
        );
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
