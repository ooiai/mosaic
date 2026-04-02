use std::{collections::BTreeMap, path::PathBuf};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use mosaic_runtime::events::RunEvent;
use mosaic_session_core::{
    SessionRecord as StoredSessionRecord, TranscriptMessage, TranscriptRole, session_route_for_id,
};

use crate::mock;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandCategory {
    Gateway,
    Session,
    Model,
    Ui,
    Debug,
}

impl CommandCategory {
    pub fn label(self) -> &'static str {
        match self {
            Self::Gateway => "gateway",
            Self::Session => "session",
            Self::Model => "model",
            Self::Ui => "ui",
            Self::Debug => "debug",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommandSpec {
    pub command: &'static str,
    pub category: CommandCategory,
    pub summary: &'static str,
    pub usage: &'static str,
    pub detail: &'static str,
}

pub const LOCAL_COMMANDS: [CommandSpec; 14] = [
    CommandSpec {
        command: "/help",
        category: CommandCategory::Ui,
        summary: "Open the command reference and shortcut guide",
        usage: "/help",
        detail: "Lists command categories, keyboard shortcuts, and the current session/runtime bindings.",
    },
    CommandSpec {
        command: "/logs",
        category: CommandCategory::Ui,
        summary: "Show or hide the observability feed",
        usage: "/logs",
        detail: "Toggles the right-side activity feed so operators can focus on chat or runtime events.",
    },
    CommandSpec {
        command: "/gateway connect",
        category: CommandCategory::Gateway,
        summary: "Resume gateway refresh and event streaming",
        usage: "/gateway connect",
        detail: "Re-enables session refresh and runtime event delivery for the current TUI attach target.",
    },
    CommandSpec {
        command: "/gateway disconnect",
        category: CommandCategory::Gateway,
        summary: "Pause gateway refresh and event streaming",
        usage: "/gateway disconnect",
        detail: "Leaves the TUI open but stops live refresh so the operator can inspect a frozen session state.",
    },
    CommandSpec {
        command: "/gateway status",
        category: CommandCategory::Gateway,
        summary: "Show the current gateway transport and readiness summary",
        usage: "/gateway status",
        detail: "Prints the current gateway transport, readiness, and node summary into the session timeline.",
    },
    CommandSpec {
        command: "/session list",
        category: CommandCategory::Session,
        summary: "List sessions currently loaded into the operator console",
        usage: "/session list",
        detail: "Shows known local or remote sessions with the current selection marker so operators can jump quickly.",
    },
    CommandSpec {
        command: "/session show",
        category: CommandCategory::Session,
        summary: "Explain the selected session route, memory, and references",
        usage: "/session show",
        detail: "Prints the active session route, runtime binding, memory summary, compressed context, and references.",
    },
    CommandSpec {
        command: "/session switch <id>",
        category: CommandCategory::Session,
        summary: "Switch the active operator conversation",
        usage: "/session switch <id>",
        detail: "Moves the composer to another session and refreshes that session from the gateway.",
    },
    CommandSpec {
        command: "/session new <id>",
        category: CommandCategory::Session,
        summary: "Create or stage a new session id for the next turn",
        usage: "/session new <id>",
        detail: "Adds a new session placeholder immediately; the session is persisted on the next submitted turn.",
    },
    CommandSpec {
        command: "/session state <active|waiting|degraded>",
        category: CommandCategory::Session,
        summary: "Override the selected session state label",
        usage: "/session state <active|waiting|degraded>",
        detail: "Useful in mock mode or demos when the operator wants to pin a local state marker.",
    },
    CommandSpec {
        command: "/session model <name>",
        category: CommandCategory::Session,
        summary: "Override the selected session model label",
        usage: "/session model <name>",
        detail: "Adjusts the visible session model label without changing the next-turn runtime profile.",
    },
    CommandSpec {
        command: "/model list",
        category: CommandCategory::Model,
        summary: "List runtime profiles available to this TUI session",
        usage: "/model list",
        detail: "Prints configured profiles with provider and model so the next turn can be scheduled intentionally.",
    },
    CommandSpec {
        command: "/model use <profile>",
        category: CommandCategory::Model,
        summary: "Switch the real runtime profile for future turns",
        usage: "/model use <profile>",
        detail: "Updates the active profile used by interactive submissions; the next message will use the new profile.",
    },
    CommandSpec {
        command: "/runtime <status>",
        category: CommandCategory::Debug,
        summary: "Set the control runtime status label",
        usage: "/runtime <status>",
        detail: "Debug helper for setting the local runtime status badge when working in mock mode or demos.",
    },
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppAction {
    Continue,
    Quit,
    Submit(String),
    GatewayConnect,
    GatewayDisconnect,
    SwitchSession(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppMode {
    Mock,
    Interactive,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputMode {
    Chat,
    Command,
    Search,
}

impl InputMode {
    pub fn label(self) -> &'static str {
        match self {
            Self::Chat => "chat",
            Self::Command => "command",
            Self::Search => "search",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileOption {
    pub name: String,
    pub model: String,
    pub provider_type: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Surface {
    Console,
    Resume,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResumeScope {
    Local,
    Remote,
    All,
}

impl ResumeScope {
    pub fn next(self) -> Self {
        match self {
            Self::Local => Self::Remote,
            Self::Remote => Self::All,
            Self::All => Self::Local,
        }
    }

    pub fn previous(self) -> Self {
        match self {
            Self::Local => Self::All,
            Self::Remote => Self::Local,
            Self::All => Self::Remote,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Local => "Local",
            Self::Remote => "Remote",
            Self::All => "All",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    Sessions,
    Timeline,
    Composer,
    Observability,
}

impl Focus {
    pub fn next(self) -> Self {
        match self {
            Self::Sessions => Self::Timeline,
            Self::Timeline => Self::Composer,
            Self::Composer => Self::Observability,
            Self::Observability => Self::Sessions,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionState {
    Active,
    Waiting,
    Degraded,
}

impl SessionState {
    pub fn label(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Waiting => "waiting",
            Self::Degraded => "degraded",
        }
    }
}

fn session_state_from_run_label(status: &str) -> SessionState {
    match status {
        "queued" | "running" | "streaming" | "cancel_requested" => SessionState::Active,
        "failed" | "canceled" => SessionState::Degraded,
        _ => SessionState::Waiting,
    }
}

fn runtime_status_from_run_label(status: &str) -> &'static str {
    match status {
        "queued" | "running" => "running",
        "streaming" => "streaming",
        "cancel_requested" => "canceling",
        "failed" => "error",
        "canceled" => "canceled",
        _ => "idle",
    }
}

fn runtime_status_is_busy(status: &str) -> bool {
    matches!(status, "running" | "streaming" | "canceling")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimelineKind {
    Operator,
    Agent,
    Tool,
    System,
}

impl TimelineKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Operator => "operator",
            Self::Agent => "agent",
            Self::Tool => "tool",
            Self::System => "system",
        }
    }
}

#[derive(Debug, Clone)]
pub struct TimelineEntry {
    pub timestamp: String,
    pub kind: TimelineKind,
    pub actor: String,
    pub title: String,
    pub body: String,
}

#[derive(Debug, Clone)]
pub struct SessionRecord {
    pub id: String,
    pub title: String,
    pub origin: String,
    pub modified: String,
    pub created: String,
    pub channel: String,
    pub actor: Option<String>,
    pub thread: Option<String>,
    pub route: String,
    pub runtime: String,
    pub model: String,
    pub state: SessionState,
    pub unread: usize,
    pub draft: String,
    pub memory_summary: Option<String>,
    pub compressed_context: Option<String>,
    pub references: Vec<String>,
    pub timeline: Vec<TimelineEntry>,
}

#[derive(Debug, Clone)]
pub struct ActivityEntry {
    pub timestamp: String,
    pub scope: String,
    pub message: String,
}

#[derive(Debug, Clone)]
pub struct App {
    pub mode: AppMode,
    pub workspace_name: String,
    pub workspace_path: String,
    pub sessions: Vec<SessionRecord>,
    pub activity: Vec<ActivityEntry>,
    pub surface: Surface,
    pub selected_session: usize,
    pub resume_scope: ResumeScope,
    pub resume_query: String,
    pub resume_search: bool,
    pub command_menu_index: usize,
    pub show_console_history: bool,
    pub focus: Focus,
    pub show_observability: bool,
    pub show_help_overlay: bool,
    pub timeline_scroll: u16,
    pub observability_scroll: u16,
    pub gateway_connected: bool,
    pub runtime_status: String,
    pub control_model: String,
    pub selected_profile: String,
    pub available_profiles: Vec<ProfileOption>,
    pub extension_summary: Option<String>,
    pub extension_policy_summary: Option<String>,
    pub extension_errors: Vec<String>,
    pub gateway_summary: Option<String>,
    pub gateway_detail: Option<String>,
    pub node_summary: Option<String>,
    pub node_detail: Option<String>,
    heartbeat: usize,
}

impl App {
    pub fn new(workspace_path: PathBuf) -> Self {
        Self::new_with_resume(workspace_path, false)
    }

    pub fn new_with_resume(workspace_path: PathBuf, start_in_resume: bool) -> Self {
        let workspace_name = workspace_path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("workspace")
            .to_owned();
        let workspace_path = workspace_path.display().to_string();

        Self {
            mode: AppMode::Mock,
            workspace_name,
            workspace_path,
            sessions: mock::sessions(),
            activity: mock::activity_feed(),
            surface: if start_in_resume {
                Surface::Resume
            } else {
                Surface::Console
            },
            selected_session: 0,
            resume_scope: ResumeScope::Local,
            resume_query: String::new(),
            resume_search: false,
            command_menu_index: 0,
            show_console_history: false,
            focus: Focus::Composer,
            show_observability: true,
            show_help_overlay: false,
            timeline_scroll: 0,
            observability_scroll: 0,
            gateway_connected: true,
            runtime_status: "warm".to_owned(),
            control_model: "gpt-5.4-control".to_owned(),
            selected_profile: "gpt-5.4-control".to_owned(),
            available_profiles: Vec::new(),
            extension_summary: None,
            extension_policy_summary: None,
            extension_errors: Vec::new(),
            gateway_summary: None,
            gateway_detail: None,
            node_summary: None,
            node_detail: None,
            heartbeat: 0,
        }
    }

    pub fn new_interactive(
        workspace_path: PathBuf,
        session_id: String,
        profile: String,
        model: String,
        available_profiles: Vec<ProfileOption>,
        start_in_resume: bool,
    ) -> Self {
        let workspace_name = workspace_path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("workspace")
            .to_owned();
        let workspace_path = workspace_path.display().to_string();

        Self {
            mode: AppMode::Interactive,
            workspace_name,
            workspace_path,
            sessions: vec![interactive_session_record(&session_id, &model)],
            activity: vec![ActivityEntry {
                timestamp: current_hhmm(),
                scope: "session".to_owned(),
                message: format!(
                    "Interactive session {} is ready. Profile={} model={}",
                    session_id, profile, model
                ),
            }],
            surface: if start_in_resume {
                Surface::Resume
            } else {
                Surface::Console
            },
            selected_session: 0,
            resume_scope: ResumeScope::Local,
            resume_query: String::new(),
            resume_search: false,
            command_menu_index: 0,
            show_console_history: true,
            focus: Focus::Composer,
            show_observability: true,
            show_help_overlay: false,
            timeline_scroll: 0,
            observability_scroll: 0,
            gateway_connected: true,
            runtime_status: "idle".to_owned(),
            control_model: model,
            selected_profile: profile,
            available_profiles,
            extension_summary: None,
            extension_policy_summary: None,
            extension_errors: Vec::new(),
            gateway_summary: None,
            gateway_detail: None,
            node_summary: None,
            node_detail: None,
            heartbeat: 0,
        }
    }

    pub fn is_interactive(&self) -> bool {
        self.mode == AppMode::Interactive
    }

    pub fn active_profile(&self) -> &str {
        &self.selected_profile
    }

    pub fn set_extension_state(
        &mut self,
        extension_summary: String,
        extension_policy_summary: String,
        extension_errors: Vec<String>,
    ) {
        self.extension_summary = Some(extension_summary);
        self.extension_policy_summary = Some(extension_policy_summary);
        self.extension_errors = extension_errors;
    }

    pub fn set_gateway_state(
        &mut self,
        gateway_summary: Option<String>,
        gateway_detail: Option<String>,
    ) {
        self.gateway_summary = gateway_summary;
        self.gateway_detail = gateway_detail;
    }

    pub fn set_node_state(&mut self, node_summary: Option<String>, node_detail: Option<String>) {
        self.node_summary = node_summary;
        self.node_detail = node_detail;
    }

    pub fn sync_runtime_session(&mut self, session: &StoredSessionRecord) {
        self.sync_runtime_session_with_origin(session, "Local");
    }

    pub fn sync_runtime_session_with_origin(
        &mut self,
        session: &StoredSessionRecord,
        origin: &str,
    ) {
        let draft = self.active_session().draft.clone();
        let unread = self.active_session().unread;
        let run_label = session.run.status.label();
        let state = session_state_from_run_label(run_label);
        self.runtime_status = runtime_status_from_run_label(run_label).to_owned();

        if let Some(view) = self.sessions.get_mut(self.selected_session) {
            view.id = session.id.clone();
            view.title = session.title.clone();
            view.origin = origin.to_owned();
            view.modified = session.updated_at.format("%Y-%m-%d %H:%M").to_string();
            view.created = session.created_at.format("%Y-%m-%d %H:%M").to_string();
            view.channel = session
                .channel_context
                .channel
                .clone()
                .unwrap_or_else(|| "control".to_owned());
            view.actor = session
                .channel_context
                .actor_name
                .clone()
                .or(session.channel_context.actor_id.clone());
            view.thread = session
                .channel_context
                .thread_title
                .clone()
                .or(session.channel_context.thread_id.clone());
            view.route = session.gateway.route.clone();
            view.runtime = session.provider_type.clone();
            view.model = session.model.clone();
            view.state = state;
            view.unread = unread;
            view.draft = draft;
            view.memory_summary = session.memory.latest_summary.clone();
            view.compressed_context = session.memory.compressed_context.clone();
            view.references = session
                .references
                .iter()
                .map(|reference| format!("{} ({})", reference.session_id, reference.reason))
                .collect();
            view.timeline = session.transcript.iter().map(transcript_entry).collect();
        }

        self.show_console_history = true;
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> AppAction {
        if matches!(
            key,
            KeyEvent {
                code: KeyCode::Char('c'),
                modifiers: KeyModifiers::CONTROL,
                ..
            }
        ) {
            return AppAction::Quit;
        }

        if self.show_help_overlay {
            match key.code {
                KeyCode::Esc | KeyCode::F(1) | KeyCode::Char('?') => {
                    self.show_help_overlay = false;
                }
                _ => {}
            }
            return AppAction::Continue;
        }

        if matches!(key.code, KeyCode::F(1))
            || (matches!(key.code, KeyCode::Char('?')) && self.focus != Focus::Composer)
        {
            self.show_help_overlay = true;
            return AppAction::Continue;
        }

        if self.surface == Surface::Resume {
            return self.handle_resume_key(key);
        }

        if matches!(
            key,
            KeyEvent {
                code: KeyCode::Char('l'),
                modifiers: KeyModifiers::CONTROL,
                ..
            }
        ) {
            self.show_observability = !self.show_observability;
            if !self.show_observability && self.focus == Focus::Observability {
                self.focus = Focus::Sessions;
            }
            return AppAction::Continue;
        }

        if self.command_menu_active() {
            match key.code {
                KeyCode::Tab => {
                    self.select_next_command_match();
                    return AppAction::Continue;
                }
                KeyCode::BackTab => {
                    self.select_previous_command_match();
                    return AppAction::Continue;
                }
                _ => {}
            }
        }

        match key.code {
            KeyCode::Char('q') if self.focus != Focus::Composer => AppAction::Quit,
            KeyCode::Char('r') if self.focus != Focus::Composer => {
                self.open_resume();
                AppAction::Continue
            }
            KeyCode::Tab => {
                self.focus = self.advance_focus();
                AppAction::Continue
            }
            KeyCode::BackTab => {
                self.open_resume();
                AppAction::Continue
            }
            KeyCode::Esc => {
                self.focus = Focus::Sessions;
                AppAction::Continue
            }
            KeyCode::Char('i') if self.focus != Focus::Composer => {
                self.focus = Focus::Composer;
                AppAction::Continue
            }
            _ => self.handle_focus_key(key),
        }
    }

    pub fn tick(&mut self) {
        self.heartbeat = self.heartbeat.wrapping_add(1);
    }

    pub fn apply_run_event(&mut self, event: RunEvent) {
        match event {
            RunEvent::RunStarted { run_id, input } => {
                self.runtime_status = "running".to_owned();
                self.active_session_mut().state = SessionState::Active;
                self.push_activity("runtime", "Run started");
                if self.is_interactive() {
                    return;
                }
                self.push_timeline(
                    TimelineKind::System,
                    "runtime",
                    "Run started",
                    &format!(
                        "run_id={}
Input: {}",
                        run_id,
                        truncate_for_timeline(&input, 180)
                    ),
                );
            }
            RunEvent::WorkflowStarted { name, step_count } => {
                self.push_activity(
                    "workflow",
                    format!("Workflow started: {} ({} steps)", name, step_count),
                );
                if self.is_interactive() {
                    return;
                }
                self.push_timeline(
                    TimelineKind::Agent,
                    "workflow",
                    "Workflow started",
                    &format!("Workflow: {}\nsteps={}", name, step_count),
                );
            }
            RunEvent::WorkflowStepStarted {
                workflow,
                step,
                kind,
            } => {
                self.push_activity(
                    "workflow",
                    format!("Step started: {}.{} ({})", workflow, step, kind),
                );
                if self.is_interactive() {
                    return;
                }
                self.push_timeline(
                    TimelineKind::Agent,
                    "workflow",
                    &format!("Workflow step: {}", step),
                    &format!("workflow={}\nkind={}", workflow, kind),
                );
            }
            RunEvent::WorkflowStepFinished { workflow, step } => {
                self.push_activity("workflow", format!("Step finished: {}.{}", workflow, step));
                if self.is_interactive() {
                    return;
                }
                self.push_timeline(
                    TimelineKind::Agent,
                    "workflow",
                    &format!("Workflow step finished: {}", step),
                    &format!("workflow={}", workflow),
                );
            }
            RunEvent::WorkflowStepFailed {
                workflow,
                step,
                error,
            } => {
                self.runtime_status = "error".to_owned();
                self.active_session_mut().state = SessionState::Degraded;
                self.push_activity("workflow", format!("Step failed: {}.{}", workflow, step));
                if self.is_interactive() {
                    return;
                }
                self.push_timeline(
                    TimelineKind::System,
                    "workflow",
                    &format!("Workflow step failed: {}", step),
                    &format!(
                        "workflow={}\nerror={}",
                        workflow,
                        truncate_for_timeline(&error, 180)
                    ),
                );
            }
            RunEvent::WorkflowFinished { name } => {
                self.push_activity("workflow", format!("Workflow finished: {}", name));
                if self.is_interactive() {
                    return;
                }
                self.push_timeline(
                    TimelineKind::Agent,
                    "workflow",
                    "Workflow finished",
                    &format!("Workflow: {}", name),
                );
            }
            RunEvent::SkillStarted { name } => {
                self.push_activity("skill", &format!("Executing skill: {}", name));
                if self.is_interactive() {
                    return;
                }
                self.push_timeline(
                    TimelineKind::Agent,
                    "skill",
                    "Skill started",
                    &format!("Skill: {}", name),
                );
            }
            RunEvent::SkillFinished { name } => {
                self.push_activity("skill", &format!("Skill finished: {}", name));
                if self.is_interactive() {
                    return;
                }
                self.push_timeline(
                    TimelineKind::Agent,
                    "skill",
                    "Skill finished",
                    &format!("Skill: {}", name),
                );
            }
            RunEvent::SkillFailed { name, error } => {
                self.runtime_status = "error".to_owned();
                self.active_session_mut().state = SessionState::Degraded;
                self.push_activity("skill", &format!("Skill failed: {}", name));
                if self.is_interactive() {
                    return;
                }
                self.push_timeline(
                    TimelineKind::System,
                    "skill",
                    "Skill failed",
                    &format!(
                        "Skill: {}\nError: {}",
                        name,
                        truncate_for_timeline(&error, 180)
                    ),
                );
            }
            RunEvent::ProviderRequest {
                provider_type,
                profile,
                model,
                tool_count,
                message_count,
                max_attempts,
            } => {
                self.push_activity(
                    "provider",
                    &format!(
                        "Provider {} / {} request dispatched (model={}, tools={}, messages={}, attempts={})",
                        provider_type, profile, model, tool_count, message_count, max_attempts
                    ),
                );
                if self.is_interactive() {
                    return;
                }
                self.push_timeline(
                    TimelineKind::System,
                    "provider",
                    "Provider request",
                    &format!(
                        "provider={} profile={} model={} tools={} messages={} attempts={}",
                        provider_type, profile, model, tool_count, message_count, max_attempts
                    ),
                );
            }
            RunEvent::ProviderRetry {
                provider_type,
                profile,
                model,
                attempt,
                max_attempts,
                kind,
                error,
                ..
            } => {
                self.push_activity(
                    "provider",
                    &format!(
                        "Provider retry {} / {} (model={}, attempt={}/{}, kind={})",
                        provider_type, profile, model, attempt, max_attempts, kind
                    ),
                );
                if self.is_interactive() {
                    return;
                }
                self.push_timeline(
                    TimelineKind::System,
                    "provider",
                    "Provider retry",
                    &format!(
                        "provider={} profile={} model={} attempt={}/{} kind={} error={}",
                        provider_type,
                        profile,
                        model,
                        attempt,
                        max_attempts,
                        kind,
                        truncate_for_timeline(&error, 180)
                    ),
                );
            }
            RunEvent::ProviderFailed {
                provider_type,
                profile,
                model,
                kind,
                error,
                ..
            } => {
                self.runtime_status = "error".to_owned();
                self.active_session_mut().state = SessionState::Degraded;
                self.push_activity(
                    "provider",
                    &format!(
                        "Provider failed {} / {} (model={}, kind={})",
                        provider_type, profile, model, kind
                    ),
                );
                if self.is_interactive() {
                    return;
                }
                self.push_timeline(
                    TimelineKind::System,
                    "provider",
                    "Provider failed",
                    &format!(
                        "provider={} profile={} model={} kind={} error={}",
                        provider_type,
                        profile,
                        model,
                        kind,
                        truncate_for_timeline(&error, 180)
                    ),
                );
            }
            RunEvent::ToolCalling { name, call_id } => {
                self.push_activity("tool", &format!("Calling tool: {}", name));
                if self.is_interactive() {
                    return;
                }
                self.push_timeline(
                    TimelineKind::Tool,
                    "tool",
                    &format!("Tool call: {}", name),
                    &format!("call_id={}", call_id),
                );
            }
            RunEvent::ToolFinished { name, call_id } => {
                self.push_activity("tool", &format!("Tool finished: {}", name));
                if self.is_interactive() {
                    return;
                }
                self.push_timeline(
                    TimelineKind::Tool,
                    "tool",
                    &format!("Tool finished: {}", name),
                    &format!("call_id={}", call_id),
                );
            }
            RunEvent::ToolFailed {
                name,
                call_id,
                error,
            } => {
                self.runtime_status = "error".to_owned();
                self.active_session_mut().state = SessionState::Degraded;
                self.push_activity("tool", &format!("Tool failed: {}", name));
                if self.is_interactive() {
                    return;
                }
                self.push_timeline(
                    TimelineKind::System,
                    "tool",
                    &format!("Tool failed: {}", name),
                    &format!(
                        "call_id={}\nerror={}",
                        call_id,
                        truncate_for_timeline(&error, 180)
                    ),
                );
            }
            RunEvent::CapabilityJobQueued {
                job_id,
                name,
                kind,
                risk,
                permission_scopes,
            } => {
                self.push_activity(
                    "capability",
                    &format!("Queued capability job: {} ({}, risk={})", name, kind, risk),
                );
                if self.is_interactive() {
                    return;
                }
                self.push_timeline(
                    TimelineKind::Tool,
                    "capability",
                    &format!("Capability queued: {}", name),
                    &format!(
                        "job_id={}
kind={}
risk={}
permissions={}",
                        job_id,
                        kind,
                        risk,
                        permission_scopes.join(", ")
                    ),
                );
            }
            RunEvent::CapabilityJobStarted { job_id, name } => {
                self.push_activity("capability", &format!("Capability running: {}", name));
                if self.is_interactive() {
                    return;
                }
                self.push_timeline(
                    TimelineKind::Tool,
                    "capability",
                    &format!("Capability running: {}", name),
                    &format!("job_id={}", job_id),
                );
            }
            RunEvent::CapabilityJobRetried {
                job_id,
                name,
                attempt,
                error,
            } => {
                self.push_activity(
                    "capability",
                    &format!("Capability retry {}: {}", attempt, name),
                );
                if self.is_interactive() {
                    return;
                }
                self.push_timeline(
                    TimelineKind::System,
                    "capability",
                    &format!("Capability retry: {}", name),
                    &format!(
                        "job_id={}
attempt={}
error={}",
                        job_id,
                        attempt,
                        truncate_for_timeline(&error, 180)
                    ),
                );
            }
            RunEvent::CapabilityJobFinished {
                job_id,
                name,
                status,
                summary,
            } => {
                self.push_activity(
                    "capability",
                    &format!("Capability finished: {} ({})", name, status),
                );
                if self.is_interactive() {
                    return;
                }
                self.push_timeline(
                    TimelineKind::Tool,
                    "capability",
                    &format!("Capability finished: {}", name),
                    &format!(
                        "job_id={}
status={}
summary={}",
                        job_id,
                        status,
                        truncate_for_timeline(&summary, 180)
                    ),
                );
            }
            RunEvent::CapabilityJobFailed {
                job_id,
                name,
                error,
            } => {
                self.runtime_status = "error".to_owned();
                self.active_session_mut().state = SessionState::Degraded;
                self.push_activity("capability", &format!("Capability failed: {}", name));
                if self.is_interactive() {
                    return;
                }
                self.push_timeline(
                    TimelineKind::System,
                    "capability",
                    &format!("Capability failed: {}", name),
                    &format!(
                        "job_id={}
error={}",
                        job_id,
                        truncate_for_timeline(&error, 180)
                    ),
                );
            }
            RunEvent::PermissionCheckFailed {
                name,
                call_id,
                reason,
            } => {
                self.runtime_status = "error".to_owned();
                self.active_session_mut().state = SessionState::Degraded;
                self.push_activity(
                    "permission",
                    &format!("Permission denied for capability: {}", name),
                );
                if self.is_interactive() {
                    return;
                }
                self.push_timeline(
                    TimelineKind::System,
                    "permission",
                    &format!("Permission check failed: {}", name),
                    &format!(
                        "call_id={}
reason={}",
                        call_id,
                        truncate_for_timeline(&reason, 180)
                    ),
                );
            }
            RunEvent::OutputDelta {
                run_id,
                chunk,
                accumulated_chars,
            } => {
                self.runtime_status = "streaming".to_owned();
                self.active_session_mut().state = SessionState::Active;
                if self.is_interactive() {
                    return;
                }
                self.push_timeline(
                    TimelineKind::Agent,
                    "runtime",
                    "Output delta",
                    &format!(
                        "run_id={}
chars={}
chunk={}",
                        run_id,
                        accumulated_chars,
                        truncate_for_timeline(&chunk, 180)
                    ),
                );
            }
            RunEvent::FinalAnswerReady { run_id } => {
                self.runtime_status = "streaming".to_owned();
                self.active_session_mut().state = SessionState::Active;
                self.push_activity("runtime", "Final answer ready");
                if self.is_interactive() {
                    return;
                }
                self.push_timeline(
                    TimelineKind::Agent,
                    "runtime",
                    "Final answer ready",
                    &format!(
                        "run_id={}
Assistant output is ready to present.",
                        run_id
                    ),
                );
            }
            RunEvent::RunFinished {
                run_id,
                output_preview,
            } => {
                self.runtime_status = "idle".to_owned();
                self.active_session_mut().state = SessionState::Waiting;
                self.push_activity("runtime", "Run finished");
                if self.is_interactive() {
                    return;
                }
                self.push_timeline(
                    TimelineKind::Agent,
                    "runtime",
                    "Run finished",
                    &format!(
                        "run_id={}
Output preview: {}",
                        run_id,
                        truncate_for_timeline(&output_preview, 180)
                    ),
                );
            }
            RunEvent::RunFailed {
                run_id,
                error,
                failure_kind,
                ..
            } => {
                self.runtime_status = "error".to_owned();
                self.active_session_mut().state = SessionState::Degraded;
                self.push_activity("runtime", "Run failed");
                if self.is_interactive() {
                    return;
                }
                self.push_timeline(
                    TimelineKind::System,
                    "runtime",
                    "Run failed",
                    &format!(
                        "run_id={}
failure_kind={}
error={}",
                        run_id,
                        failure_kind.unwrap_or_else(|| "<none>".to_owned()),
                        truncate_for_timeline(&error, 180)
                    ),
                );
            }
            RunEvent::RunCanceled { run_id, reason } => {
                self.runtime_status = "canceled".to_owned();
                self.active_session_mut().state = SessionState::Degraded;
                self.push_activity("runtime", "Run canceled");
                if self.is_interactive() {
                    return;
                }
                self.push_timeline(
                    TimelineKind::System,
                    "runtime",
                    "Run canceled",
                    &format!(
                        "run_id={}
reason={}",
                        run_id,
                        truncate_for_timeline(&reason, 180)
                    ),
                );
            }
        }
    }

    pub fn pulse_frame(&self) -> usize {
        self.heartbeat % 4
    }

    pub fn active_session(&self) -> &SessionRecord {
        &self.sessions[self.selected_session]
    }

    pub fn active_session_mut(&mut self) -> &mut SessionRecord {
        &mut self.sessions[self.selected_session]
    }

    pub fn session_label(&self) -> &str {
        &self.active_session().id
    }

    pub fn active_draft(&self) -> &str {
        &self.active_session().draft
    }

    pub fn input_mode(&self) -> InputMode {
        if self.surface == Surface::Resume && self.resume_search {
            InputMode::Search
        } else if self.active_draft().trim_start().starts_with('/') {
            InputMode::Command
        } else {
            InputMode::Chat
        }
    }

    pub fn latest_activity(&self) -> Option<&ActivityEntry> {
        self.activity.last()
    }

    pub fn operator_status(&self) -> String {
        if !self.gateway_connected {
            return format!(
                "Gateway paused. /gateway connect resumes refresh for session {}.",
                self.session_label()
            );
        }

        match self.runtime_status.as_str() {
            "running" => format!(
                "Assistant pending for session {} via profile {} ({}).",
                self.session_label(),
                self.active_profile(),
                self.control_model
            ),
            "streaming" => format!(
                "Assistant is streaming output for session {} via profile {} ({}).",
                self.session_label(),
                self.active_profile(),
                self.control_model
            ),
            "canceling" => format!(
                "Cancellation requested for session {}. Waiting for the runtime to stop.",
                self.session_label()
            ),
            "error" => self
                .latest_activity()
                .map(|entry| format!("Last event [{}] {}", entry.scope, entry.message))
                .unwrap_or_else(|| {
                    "Last run failed. Inspect the activity feed for details.".to_owned()
                }),
            "canceled" => self
                .latest_activity()
                .map(|entry| format!("Last event [{}] {}", entry.scope, entry.message))
                .unwrap_or_else(|| "Last run was canceled by the operator.".to_owned()),
            _ => format!(
                "Next turn uses profile {} ({}) while the selected session currently shows {} / {}.",
                self.active_profile(),
                self.control_model,
                self.active_session().runtime,
                self.active_session().model
            ),
        }
    }

    pub fn composer_placeholder(&self) -> &'static str {
        match self.input_mode() {
            InputMode::Chat => {
                "Send a message to the active session. Use / for local commands or Shift+Tab to browse sessions."
            }
            InputMode::Command => {
                "Run a local control command. Tab cycles suggestions and Enter executes the highlighted command."
            }
            InputMode::Search => "Filter sessions by id, title, route, channel, or origin.",
        }
    }

    pub fn enter_hint(&self) -> String {
        match self.surface {
            Surface::Resume => {
                if self.resume_search {
                    "Apply the search filter and keep browsing sessions".to_owned()
                } else {
                    format!("Open session {}", self.session_label())
                }
            }
            Surface::Console => match self.input_mode() {
                InputMode::Chat => {
                    if self.is_interactive() {
                        format!("Send to session {}", self.session_label())
                    } else {
                        "Queue a local mock instruction".to_owned()
                    }
                }
                InputMode::Command => {
                    if self.command_menu_should_complete() {
                        "Complete the highlighted slash command".to_owned()
                    } else {
                        "Run the local slash command".to_owned()
                    }
                }
                InputMode::Search => "Apply the search filter".to_owned(),
            },
        }
    }

    pub fn escape_hint(&self) -> &'static str {
        match self.surface {
            Surface::Resume => {
                if self.resume_search {
                    "leave search"
                } else {
                    "back to console"
                }
            }
            Surface::Console => "focus sessions",
        }
    }

    pub fn command_query(&self) -> Option<&str> {
        self.active_draft().trim_start().strip_prefix('/')
    }

    pub fn command_matches(&self) -> Vec<CommandSpec> {
        matching_commands(self.command_query().unwrap_or_default())
    }

    pub fn selected_command_match(&self) -> Option<CommandSpec> {
        let matches = self.command_matches();
        matches
            .get(self.command_menu_index.min(matches.len().saturating_sub(1)))
            .copied()
    }

    pub fn command_suggestions(&self) -> Vec<CommandSpec> {
        suggest_commands(self.command_query().unwrap_or_default())
    }

    pub fn sync_session_catalog(
        &mut self,
        mut sessions: Vec<SessionRecord>,
        selected_session_id: &str,
    ) {
        let default_origin = self
            .sessions
            .get(self.selected_session)
            .map(|session| session.origin.clone())
            .unwrap_or_else(|| "Local".to_owned());
        let default_runtime = self
            .sessions
            .get(self.selected_session)
            .map(|session| session.runtime.clone())
            .unwrap_or_else(|| "agent-runtime".to_owned());

        for session in &mut sessions {
            if let Some(existing) = self
                .sessions
                .iter()
                .find(|candidate| candidate.id == session.id)
            {
                session.draft = existing.draft.clone();
                session.unread = existing.unread;
                if session.memory_summary.is_none() {
                    session.memory_summary = existing.memory_summary.clone();
                }
                if session.compressed_context.is_none() {
                    session.compressed_context = existing.compressed_context.clone();
                }
                if session.references.is_empty() {
                    session.references = existing.references.clone();
                }
                if session.timeline.is_empty() {
                    session.timeline = existing.timeline.clone();
                }
                if session.actor.is_none() {
                    session.actor = existing.actor.clone();
                }
                if session.thread.is_none() {
                    session.thread = existing.thread.clone();
                }
            }
        }

        if !sessions
            .iter()
            .any(|session| session.id == selected_session_id)
        {
            let mut placeholder =
                interactive_session_record(selected_session_id, &self.control_model);
            placeholder.origin = default_origin;
            placeholder.runtime = default_runtime;
            sessions.push(placeholder);
        }

        self.sessions = sessions;
        if let Some(index) = self
            .sessions
            .iter()
            .position(|session| session.id == selected_session_id)
        {
            self.select_session(index);
        }
        self.normalize_resume_scope();
    }

    pub fn visible_session_indices(&self) -> Vec<usize> {
        self.sessions
            .iter()
            .enumerate()
            .filter_map(|(index, session)| self.session_visible_in_resume(session).then_some(index))
            .collect()
    }

    fn handle_focus_key(&mut self, key: KeyEvent) -> AppAction {
        match self.focus {
            Focus::Sessions => {
                self.handle_sessions_key(key.code);
                AppAction::Continue
            }
            Focus::Timeline => {
                self.handle_timeline_key(key.code);
                AppAction::Continue
            }
            Focus::Composer => self.handle_composer_key(key),
            Focus::Observability => {
                self.handle_observability_key(key.code);
                AppAction::Continue
            }
        }
    }

    fn handle_resume_key(&mut self, key: KeyEvent) -> AppAction {
        if self.resume_search {
            return self.handle_resume_search_key(key);
        }

        match key.code {
            KeyCode::Down | KeyCode::Char('j') => self.select_next_visible_session(),
            KeyCode::Up | KeyCode::Char('k') => self.select_previous_visible_session(),
            KeyCode::Tab => self.set_resume_scope(self.resume_scope.next()),
            KeyCode::BackTab => self.set_resume_scope(self.resume_scope.previous()),
            KeyCode::Char('/') => self.resume_search = true,
            KeyCode::Enter => {
                let selected_id = self.active_session().id.clone();
                self.surface = Surface::Console;
                self.show_console_history = true;
                self.focus = Focus::Composer;
                if self.is_interactive() {
                    return AppAction::SwitchSession(selected_id);
                }
            }
            KeyCode::Esc => {
                self.surface = Surface::Console;
                self.focus = Focus::Composer;
            }
            _ => {}
        }

        AppAction::Continue
    }

    fn handle_resume_search_key(&mut self, key: KeyEvent) -> AppAction {
        match key.code {
            KeyCode::Esc => {
                self.resume_search = false;
                self.resume_query.clear();
                self.ensure_selected_session_visible();
            }
            KeyCode::Enter => {
                self.resume_search = false;
                self.ensure_selected_session_visible();
            }
            KeyCode::Backspace => {
                self.resume_query.pop();
                self.ensure_selected_session_visible();
            }
            KeyCode::Char(character)
                if !key.modifiers.contains(KeyModifiers::CONTROL)
                    && !key.modifiers.contains(KeyModifiers::ALT) =>
            {
                self.resume_query.push(character);
                self.ensure_selected_session_visible();
            }
            _ => {}
        }

        AppAction::Continue
    }

    fn handle_sessions_key(&mut self, key: KeyCode) {
        match key {
            KeyCode::Down | KeyCode::Char('j') => self.select_next_session(),
            KeyCode::Up | KeyCode::Char('k') => self.select_previous_session(),
            KeyCode::Home => self.select_session(0),
            KeyCode::End => {
                if !self.sessions.is_empty() {
                    self.select_session(self.sessions.len() - 1);
                }
            }
            _ => {}
        }
    }

    fn handle_timeline_key(&mut self, key: KeyCode) {
        match key {
            KeyCode::Down | KeyCode::Char('j') => {
                self.timeline_scroll = self.timeline_scroll.saturating_add(1);
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.timeline_scroll = self.timeline_scroll.saturating_sub(1);
            }
            KeyCode::PageDown => {
                self.timeline_scroll = self.timeline_scroll.saturating_add(5);
            }
            KeyCode::PageUp => {
                self.timeline_scroll = self.timeline_scroll.saturating_sub(5);
            }
            KeyCode::Home => {
                self.timeline_scroll = 0;
            }
            _ => {}
        }
    }

    fn handle_observability_key(&mut self, key: KeyCode) {
        match key {
            KeyCode::Down | KeyCode::Char('j') => {
                self.observability_scroll = self.observability_scroll.saturating_add(1);
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.observability_scroll = self.observability_scroll.saturating_sub(1);
            }
            KeyCode::PageDown => {
                self.observability_scroll = self.observability_scroll.saturating_add(5);
            }
            KeyCode::PageUp => {
                self.observability_scroll = self.observability_scroll.saturating_sub(5);
            }
            KeyCode::Home => {
                self.observability_scroll = 0;
            }
            _ => {}
        }
    }

    fn handle_composer_key(&mut self, key: KeyEvent) -> AppAction {
        if self.command_menu_active() {
            match key.code {
                KeyCode::Down | KeyCode::Char('j') => {
                    self.select_next_command_match();
                    return AppAction::Continue;
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    self.select_previous_command_match();
                    return AppAction::Continue;
                }
                KeyCode::Tab => {
                    self.select_next_command_match();
                    return AppAction::Continue;
                }
                KeyCode::Enter if self.command_menu_should_complete() => {
                    self.complete_selected_command();
                    return AppAction::Continue;
                }
                _ => {}
            }
        }

        match key.code {
            KeyCode::Enter => return self.submit_composer(),
            KeyCode::Backspace => {
                self.active_session_mut().draft.pop();
                self.command_menu_index = 0;
            }
            KeyCode::Char(character)
                if !key.modifiers.contains(KeyModifiers::CONTROL)
                    && !key.modifiers.contains(KeyModifiers::ALT) =>
            {
                self.active_session_mut().draft.push(character);
                self.command_menu_index = 0;
            }
            _ => {}
        }

        AppAction::Continue
    }

    fn submit_composer(&mut self) -> AppAction {
        let message = self.active_draft().trim().to_owned();
        if message.is_empty() {
            return AppAction::Continue;
        }

        let action = if let Some(command) = message.strip_prefix('/') {
            self.route_command(command.trim())
        } else if self.is_interactive() {
            if runtime_status_is_busy(&self.runtime_status) {
                self.push_command_error("A run is already in progress for this session");
                AppAction::Continue
            } else {
                let session_id = self.session_label().to_owned();
                let profile = self.active_profile().to_owned();
                let model = self.control_model.clone();
                self.runtime_status = "running".to_owned();
                self.active_session_mut().state = SessionState::Active;
                self.push_timeline(
                    TimelineKind::Operator,
                    "operator",
                    "Message queued",
                    &message,
                );
                self.push_system_entry(
                    "Assistant pending",
                    format!(
                        "Waiting for session {} to reply via profile {} ({}).",
                        session_id, profile, model
                    ),
                );
                self.push_activity(
                    "composer",
                    format!("Submitted message to session {}", session_id),
                );
                AppAction::Submit(message.clone())
            }
        } else {
            self.queue_operator_instruction(&message);
            AppAction::Continue
        };

        self.show_console_history = true;
        self.active_session_mut().draft.clear();
        self.timeline_scroll = 0;

        action
    }

    fn queue_operator_instruction(&mut self, message: &str) {
        let session_label = self.session_label().to_owned();

        self.push_timeline(
            TimelineKind::Operator,
            "operator",
            "Queued operator instruction",
            message,
        );

        self.push_activity(
            "composer",
            format!("Buffered command for {}", session_label),
        );
    }

    fn select_next_session(&mut self) {
        if self.sessions.is_empty() {
            return;
        }

        let next = (self.selected_session + 1) % self.sessions.len();
        self.select_session(next);
    }

    fn select_previous_session(&mut self) {
        if self.sessions.is_empty() {
            return;
        }

        let next = if self.selected_session == 0 {
            self.sessions.len() - 1
        } else {
            self.selected_session - 1
        };
        self.select_session(next);
    }

    fn select_session(&mut self, index: usize) {
        self.selected_session = index;
        self.timeline_scroll = 0;
        self.sessions[index].unread = 0;
    }

    fn select_next_visible_session(&mut self) {
        let visible = self.visible_session_indices();
        if visible.is_empty() {
            return;
        }

        let current = visible
            .iter()
            .position(|index| *index == self.selected_session)
            .unwrap_or(0);
        let next = (current + 1) % visible.len();
        self.select_session(visible[next]);
    }

    fn select_previous_visible_session(&mut self) {
        let visible = self.visible_session_indices();
        if visible.is_empty() {
            return;
        }

        let current = visible
            .iter()
            .position(|index| *index == self.selected_session)
            .unwrap_or(0);
        let next = if current == 0 {
            visible.len() - 1
        } else {
            current - 1
        };
        self.select_session(visible[next]);
    }

    fn set_resume_scope(&mut self, scope: ResumeScope) {
        self.resume_scope = scope;
        self.ensure_selected_session_visible();
    }

    fn ensure_selected_session_visible(&mut self) {
        if self
            .visible_session_indices()
            .contains(&self.selected_session)
        {
            return;
        }

        if let Some(first) = self.visible_session_indices().first().copied() {
            self.select_session(first);
        }
    }

    fn normalize_resume_scope(&mut self) {
        if !self.resume_query.is_empty() {
            return;
        }

        let has_local = self
            .sessions
            .iter()
            .any(|session| session.origin == "Local");
        let has_remote = self
            .sessions
            .iter()
            .any(|session| session.origin == "Remote");

        self.resume_scope = match (has_local, has_remote) {
            (true, false) => ResumeScope::Local,
            (false, true) => ResumeScope::Remote,
            _ => {
                if self.visible_session_indices().is_empty() {
                    ResumeScope::All
                } else {
                    self.resume_scope
                }
            }
        };
        self.ensure_selected_session_visible();
    }

    fn session_visible_in_resume(&self, session: &SessionRecord) -> bool {
        let matches_scope = match self.resume_scope {
            ResumeScope::Local => session.origin == "Local",
            ResumeScope::Remote => session.origin == "Remote",
            ResumeScope::All => true,
        };
        let query = self.resume_query.trim();
        let matches_query = query.is_empty()
            || [
                session.id.as_str(),
                session.title.as_str(),
                session.origin.as_str(),
                session.route.as_str(),
                session.channel.as_str(),
            ]
            .iter()
            .any(|value| {
                value
                    .to_ascii_lowercase()
                    .contains(&query.to_ascii_lowercase())
            });

        matches_scope && matches_query
    }

    fn advance_focus(&self) -> Focus {
        let next = self.focus.next();
        if !self.show_observability && next == Focus::Observability {
            next.next()
        } else {
            next
        }
    }

    fn open_resume(&mut self) {
        self.surface = Surface::Resume;
        self.resume_search = false;
        self.resume_query.clear();
        self.normalize_resume_scope();
    }

    fn command_menu_active(&self) -> bool {
        self.focus == Focus::Composer && self.command_query().is_some()
    }

    fn command_menu_should_complete(&self) -> bool {
        if !self.command_menu_active() {
            return false;
        }

        let Some(command) = self.selected_command_match() else {
            return false;
        };

        self.active_draft().trim_end() != command_completion(command.command).trim_end()
    }

    fn select_next_command_match(&mut self) {
        let matches = self.command_matches();
        if matches.is_empty() {
            self.command_menu_index = 0;
            return;
        }

        self.command_menu_index = (self.command_menu_index + 1) % matches.len();
    }

    fn select_previous_command_match(&mut self) {
        let matches = self.command_matches();
        if matches.is_empty() {
            self.command_menu_index = 0;
            return;
        }

        self.command_menu_index = if self.command_menu_index == 0 {
            matches.len() - 1
        } else {
            self.command_menu_index - 1
        };
    }

    fn complete_selected_command(&mut self) {
        let Some(command) = self.selected_command_match() else {
            return;
        };

        self.active_session_mut().draft = command_completion(command.command);
        self.command_menu_index = 0;
    }

    fn route_command(&mut self, command: &str) -> AppAction {
        let mut parts = command.split_whitespace();
        let Some(name) = parts.next() else {
            self.push_command_error("Usage: /help");
            return AppAction::Continue;
        };

        match name {
            "help" => {
                self.push_help();
                AppAction::Continue
            }
            "logs" => {
                self.toggle_logs();
                AppAction::Continue
            }
            "gateway" => self.route_gateway_command(parts.collect()),
            "model" => self.route_model_command(parts.collect()),
            "runtime" => {
                let status = parts.collect::<Vec<_>>().join(" ");
                self.set_runtime_status(status.trim());
                AppAction::Continue
            }
            "session" => self.route_session_command(parts.collect()),
            _ => {
                self.push_command_error(unknown_command_message(command));
                AppAction::Continue
            }
        }
    }

    fn route_model_command(&mut self, args: Vec<&str>) -> AppAction {
        match args.as_slice() {
            ["list"] => {
                if self.available_profiles.is_empty() {
                    self.push_command_error(
                        "No runtime profiles are available in this TUI session",
                    );
                    return AppAction::Continue;
                }

                let profiles = self
                    .available_profiles
                    .iter()
                    .map(|profile| {
                        let marker = if profile.name == self.selected_profile {
                            "*"
                        } else {
                            "-"
                        };
                        format!(
                            "{} {} | provider={} | model={}",
                            marker, profile.name, profile.provider_type, profile.model
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(
                        "
",
                    );

                self.push_activity("model", "Listed available runtime profiles.");
                self.push_system_entry("Runtime profiles", profiles);
                AppAction::Continue
            }
            ["use", rest @ ..] if !rest.is_empty() => {
                self.set_active_profile(&rest.join(" "));
                AppAction::Continue
            }
            _ => {
                self.push_command_error("Usage: /model list | /model use <profile>");
                AppAction::Continue
            }
        }
    }

    fn route_gateway_command(&mut self, args: Vec<&str>) -> AppAction {
        match args.as_slice() {
            ["connect"] => {
                self.gateway_connected = true;
                self.push_activity("gateway", "Gateway link marked connected in the TUI.");
                self.push_system_entry(
                    "Gateway link updated",
                    "Gateway refresh and event streaming were resumed by operator command.",
                );
                AppAction::GatewayConnect
            }
            ["disconnect"] => {
                self.gateway_connected = false;
                self.push_activity("gateway", "Gateway link marked disconnected in the TUI.");
                self.push_system_entry(
                    "Gateway link updated",
                    "Gateway refresh and event streaming were paused by operator command.",
                );
                AppAction::GatewayDisconnect
            }
            ["status"] => {
                self.show_gateway_status();
                AppAction::Continue
            }
            _ => {
                self.push_command_error(
                    "Usage: /gateway connect | /gateway disconnect | /gateway status",
                );
                AppAction::Continue
            }
        }
    }

    fn show_gateway_status(&mut self) {
        let summary = self
            .gateway_summary
            .clone()
            .unwrap_or_else(|| "Gateway summary unavailable".to_owned());
        let detail = self
            .gateway_detail
            .clone()
            .unwrap_or_else(|| "Gateway readiness detail unavailable".to_owned());
        let node_summary = self
            .node_summary
            .clone()
            .unwrap_or_else(|| "Node summary unavailable".to_owned());
        let node_detail = self
            .node_detail
            .clone()
            .unwrap_or_else(|| "Node detail unavailable".to_owned());
        self.push_activity("gateway", "Displayed gateway transport summary.");
        self.push_system_entry(
            "Gateway status",
            format!(
                "{}
{}
{}
{}",
                summary, detail, node_summary, node_detail
            ),
        );
    }

    fn set_runtime_status(&mut self, status: &str) {
        if status.is_empty() {
            self.push_command_error("Usage: /runtime <status>");
            return;
        }

        self.runtime_status = status.to_owned();
        self.active_session_mut().state = session_state_from_run_label(status);
        self.push_activity("runtime", format!("Runtime status set to {status}."));
        self.push_system_entry(
            "Runtime status updated",
            format!("Control-plane runtime status is now {status}."),
        );
    }

    fn route_session_command(&mut self, args: Vec<&str>) -> AppAction {
        match args.as_slice() {
            ["list"] => {
                self.show_session_list();
                AppAction::Continue
            }
            ["show"] => {
                self.show_session_details();
                AppAction::Continue
            }
            ["switch", rest @ ..] if !rest.is_empty() => {
                self.prepare_session_switch(&rest.join(" "), false)
            }
            ["new", rest @ ..] if !rest.is_empty() => {
                self.prepare_session_switch(&rest.join(" "), true)
            }
            ["state", "active"] => {
                self.set_session_state(SessionState::Active);
                AppAction::Continue
            }
            ["state", "waiting"] => {
                self.set_session_state(SessionState::Waiting);
                AppAction::Continue
            }
            ["state", "degraded"] => {
                self.set_session_state(SessionState::Degraded);
                AppAction::Continue
            }
            ["model", rest @ ..] if !rest.is_empty() => {
                self.set_session_model(&rest.join(" "));
                AppAction::Continue
            }
            _ => {
                self.push_command_error(
                    "Usage: /session list | /session show | /session switch <id> | /session new <id> | /session state <active|waiting|degraded> | /session model <name>",
                );
                AppAction::Continue
            }
        }
    }

    fn show_session_list(&mut self) {
        let body = self
            .sessions
            .iter()
            .enumerate()
            .map(|(index, session)| {
                let marker = if index == self.selected_session {
                    "*"
                } else {
                    "-"
                };
                format!(
                    "{} {} | origin={} | route={} | model={} | state={}",
                    marker,
                    session.id,
                    session.origin,
                    session.route,
                    session.model,
                    session.state.label()
                )
            })
            .collect::<Vec<_>>()
            .join(
                "
",
            );
        self.push_activity("session", "Displayed operator session list.");
        self.push_system_entry("Sessions", body);
    }

    fn show_session_details(&mut self) {
        let session_id = self.active_session().id.clone();
        let route = self.active_session().route.clone();
        let origin = self.active_session().origin.clone();
        let channel = self.active_session().channel.clone();
        let runtime = self.active_session().runtime.clone();
        let model = self.active_session().model.clone();
        let references = if self.active_session().references.is_empty() {
            "none".to_owned()
        } else {
            self.active_session().references.join(", ")
        };
        let memory = self
            .active_session()
            .memory_summary
            .clone()
            .unwrap_or_else(|| "none".to_owned());
        let compressed = self
            .active_session()
            .compressed_context
            .clone()
            .unwrap_or_else(|| "none".to_owned());
        let next_profile = self.active_profile().to_owned();
        let next_model = self.control_model.clone();
        self.push_activity(
            "session",
            format!("Displayed details for session {}.", session_id),
        );
        self.push_system_entry(
            "Session details",
            format!(
                "session={}
route={}
origin={}
channel={}
current runtime={}
current model={}
next-turn profile={}
next-turn model={}
memory={}
compressed={}
references={}",
                session_id,
                route,
                origin,
                channel,
                runtime,
                model,
                next_profile,
                next_model,
                memory,
                compressed,
                references
            ),
        );
    }

    fn prepare_session_switch(&mut self, session_id: &str, create_if_missing: bool) -> AppAction {
        let target_index = if let Some(index) = self
            .sessions
            .iter()
            .position(|session| session.id == session_id)
        {
            index
        } else if create_if_missing {
            let mut placeholder = interactive_session_record(session_id, &self.control_model);
            placeholder.origin = self.active_session().origin.clone();
            placeholder.runtime = self.active_session().runtime.clone();
            self.sessions.push(placeholder);
            self.sessions.len() - 1
        } else {
            self.push_command_error(format!(
                "Unknown session: {}. Use /session list or Shift+Tab to browse.",
                session_id
            ));
            return AppAction::Continue;
        };

        self.select_session(target_index);
        self.surface = Surface::Console;
        self.focus = Focus::Composer;
        self.show_console_history = true;
        self.push_activity(
            "session",
            format!("Selected session {} in the operator console.", session_id),
        );
        self.push_system_entry(
            "Session selected",
            format!(
                "Composer now targets session {}. The next turn will use profile {} ({}).",
                session_id,
                self.active_profile(),
                self.control_model
            ),
        );

        if self.is_interactive() {
            AppAction::SwitchSession(session_id.to_owned())
        } else {
            AppAction::Continue
        }
    }

    fn set_session_state(&mut self, state: SessionState) {
        self.active_session_mut().state = state;
        self.push_activity(
            "session",
            format!("{} state set to {}.", self.session_label(), state.label()),
        );
        self.push_system_entry(
            "Session state updated",
            format!("Selected session is now {}.", state.label()),
        );
    }

    fn set_session_model(&mut self, model: &str) {
        self.active_session_mut().model = model.to_owned();
        self.push_activity(
            "session",
            format!("{} model set to {model}.", self.session_label()),
        );
        self.push_system_entry(
            "Session model updated",
            format!("Selected session now shows {model}. Future turns still use the active profile unless it is changed with /model use."),
        );
    }

    fn set_active_profile(&mut self, profile: &str) {
        let Some(option) = self
            .available_profiles
            .iter()
            .find(|candidate| candidate.name == profile)
            .cloned()
        else {
            self.push_command_error(format!("Unknown profile: {profile}"));
            return;
        };

        self.selected_profile = option.name.clone();
        self.control_model = option.model.clone();
        self.push_activity(
            "model",
            format!("Interactive runtime profile switched to {}.", option.name),
        );
        self.push_system_entry(
            "Runtime profile updated",
            format!(
                "Future turns will use profile {} (type={}, model={}). The currently selected session still shows {} / {} until the next run completes.",
                option.name,
                option.provider_type,
                option.model,
                self.active_session().runtime,
                self.active_session().model
            ),
        );
    }

    fn toggle_logs(&mut self) {
        self.show_observability = !self.show_observability;
        if !self.show_observability && self.focus == Focus::Observability {
            self.focus = Focus::Sessions;
        }

        let visibility = if self.show_observability {
            "visible"
        } else {
            "hidden"
        };
        self.push_activity("ui", format!("Observability panel is now {visibility}."));
        self.push_system_entry(
            "Observability visibility updated",
            format!("Right-side observability panel is now {visibility}."),
        );
    }

    fn push_help(&mut self) {
        self.show_help_overlay = true;
        let mut grouped = BTreeMap::new();
        for category in [
            CommandCategory::Ui,
            CommandCategory::Gateway,
            CommandCategory::Session,
            CommandCategory::Model,
            CommandCategory::Debug,
        ] {
            let entries = LOCAL_COMMANDS
                .iter()
                .filter(|spec| spec.category == category)
                .map(|spec| format!("{}  {}", spec.usage, spec.summary))
                .collect::<Vec<_>>();
            grouped.insert(category.label(), entries);
        }

        let mut body = vec![
            format!("input.chat    Enter => {}", if self.is_interactive() { "send to the active session" } else { "queue a mock instruction" }),
            "input.command Enter => run local slash command, Tab => complete, Esc => leave command mode".to_owned(),
            "input.search  / in resume view starts search over session id, title, route, and channel".to_owned(),
            String::new(),
        ];
        for (category, entries) in grouped {
            body.push(format!("[{category}]"));
            body.extend(entries);
            body.push(String::new());
        }
        body.push(format!(
            "current session={} next-profile={} next-model={}",
            self.session_label(),
            self.active_profile(),
            self.control_model
        ));

        self.push_activity("command", "Displayed local control command reference.");
        self.push_system_entry(
            "Local command reference",
            body.join(
                "
",
            ),
        );
    }

    pub(crate) fn push_command_error(&mut self, message: impl Into<String>) {
        let message = message.into();
        self.push_activity("command", format!("Rejected command: {message}"));
        self.push_system_entry("Command rejected", message);
    }

    fn push_system_entry(&mut self, title: impl Into<String>, body: impl Into<String>) {
        self.push_timeline(
            TimelineKind::System,
            "control-plane",
            &title.into(),
            &body.into(),
        );
    }

    fn push_activity(&mut self, scope: impl Into<String>, message: impl Into<String>) {
        self.activity.push(ActivityEntry {
            timestamp: current_hhmm(),
            scope: scope.into(),
            message: message.into(),
        });

        if self.activity.len() > 200 {
            let overflow = self.activity.len() - 200;
            self.activity.drain(0..overflow);
        }
    }

    fn push_timeline(&mut self, kind: TimelineKind, actor: &str, title: &str, body: &str) {
        if let Some(session) = self.sessions.get_mut(self.selected_session) {
            session.timeline.push(TimelineEntry {
                timestamp: current_hhmm(),
                kind,
                actor: actor.to_owned(),
                title: title.to_owned(),
                body: body.to_owned(),
            });

            if session.timeline.len() > 400 {
                let overflow = session.timeline.len() - 400;
                session.timeline.drain(0..overflow);
            }
        }

        self.show_console_history = true;
    }
}

fn interactive_session_record(session_id: &str, model: &str) -> SessionRecord {
    SessionRecord {
        id: session_id.to_owned(),
        title: "Untitled session".to_owned(),
        origin: "Local".to_owned(),
        modified: current_hhmm(),
        created: current_hhmm(),
        channel: "control".to_owned(),
        actor: None,
        thread: None,
        route: session_route_for_id(session_id),
        runtime: "agent-runtime".to_owned(),
        model: model.to_owned(),
        state: SessionState::Waiting,
        unread: 0,
        draft: String::new(),
        memory_summary: None,
        compressed_context: None,
        references: Vec::new(),
        timeline: Vec::new(),
    }
}

fn transcript_entry(message: &TranscriptMessage) -> TimelineEntry {
    let (kind, actor, title) = match message.role {
        TranscriptRole::System => (TimelineKind::System, "system", "System"),
        TranscriptRole::User => (TimelineKind::Operator, "user", "User"),
        TranscriptRole::Assistant => (TimelineKind::Agent, "assistant", "Assistant"),
        TranscriptRole::Tool => (TimelineKind::Tool, "tool", "Tool"),
    };

    TimelineEntry {
        timestamp: message.created_at.format("%H:%M").to_string(),
        kind,
        actor: actor.to_owned(),
        title: title.to_owned(),
        body: message.content.clone(),
    }
}

fn current_hhmm() -> String {
    use chrono::Local;

    Local::now().format("%H:%M").to_string()
}

fn truncate_for_timeline(value: &str, limit: usize) -> String {
    if value.chars().count() <= limit {
        return value.to_owned();
    }

    let truncated: String = value.chars().take(limit).collect();
    format!("{truncated}...")
}

fn command_completion(command: &str) -> String {
    let tokens = command
        .split_whitespace()
        .take_while(|token| !token.starts_with('<'))
        .collect::<Vec<_>>();
    if tokens.is_empty() {
        command.to_owned()
    } else {
        format!("{} ", tokens.join(" "))
    }
}

fn unknown_command_message(command: &str) -> String {
    let suggestions = suggest_commands(command);
    if suggestions.is_empty() {
        return format!("Unknown command: /{}", command.trim());
    }

    format!(
        "Unknown command: /{}. Did you mean {}?",
        command.trim(),
        suggestions
            .into_iter()
            .take(3)
            .map(|spec| spec.command)
            .collect::<Vec<_>>()
            .join(", ")
    )
}

fn suggest_commands(query: &str) -> Vec<CommandSpec> {
    let normalized = normalize_command_text(query);
    if normalized.is_empty() {
        return Vec::new();
    }

    let mut scored = LOCAL_COMMANDS
        .iter()
        .copied()
        .map(|spec| {
            let searchable = command_search_text(spec.command);
            let score = if searchable.starts_with(&normalized) {
                0usize
            } else {
                edit_distance(&normalized, &searchable)
            };
            (score, spec)
        })
        .collect::<Vec<_>>();
    scored.sort_by_key(|(score, spec)| (*score, spec.command.len()));
    scored
        .into_iter()
        .filter(|(score, _)| *score <= normalized.len().max(3))
        .map(|(_, spec)| spec)
        .collect()
}

pub fn matching_commands(query: &str) -> Vec<CommandSpec> {
    let normalized = normalize_command_text(query);
    LOCAL_COMMANDS
        .iter()
        .copied()
        .filter(|spec| command_matches_query(*spec, &normalized))
        .collect()
}

fn command_matches_query(spec: CommandSpec, normalized_query: &str) -> bool {
    if normalized_query.is_empty() {
        return true;
    }

    let query_tokens = normalized_query.split_whitespace().collect::<Vec<_>>();
    let command_tokens = command_search_text(spec.command)
        .split_whitespace()
        .map(str::to_owned)
        .collect::<Vec<_>>();

    if query_tokens.len() == 1 {
        return command_tokens
            .iter()
            .any(|token| token.starts_with(query_tokens[0]));
    }

    query_tokens.len() <= command_tokens.len()
        && query_tokens
            .iter()
            .zip(command_tokens.iter())
            .all(|(query, command)| command.starts_with(query))
}

fn command_search_text(command: &str) -> String {
    normalize_command_text(command)
}

fn normalize_command_text(value: &str) -> String {
    value
        .trim()
        .trim_start_matches('/')
        .chars()
        .map(|character| match character {
            '<' | '>' | '|' => ' ',
            _ => character.to_ascii_lowercase(),
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn edit_distance(left: &str, right: &str) -> usize {
    if left.is_empty() {
        return right.chars().count();
    }
    if right.is_empty() {
        return left.chars().count();
    }

    let left = left.chars().collect::<Vec<_>>();
    let right = right.chars().collect::<Vec<_>>();
    let mut previous = (0..=right.len()).collect::<Vec<_>>();
    let mut current = vec![0usize; right.len() + 1];

    for (row, left_char) in left.iter().enumerate() {
        current[0] = row + 1;
        for (column, right_char) in right.iter().enumerate() {
            let substitution_cost = usize::from(left_char != right_char);
            current[column + 1] = (current[column] + 1)
                .min(previous[column + 1] + 1)
                .min(previous[column] + substitution_cost);
        }
        previous.clone_from(&current);
    }

    previous[right.len()]
}

#[cfg(test)]
mod tests {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use mosaic_runtime::events::RunEvent;

    use super::{
        App, AppAction, Focus, InputMode, ResumeScope, SessionState, Surface, TimelineKind,
    };

    #[test]
    fn tab_skips_hidden_observability_panel() {
        let mut app = App::new("/tmp/mosaic".into());
        app.show_observability = false;
        app.focus = Focus::Composer;

        app.focus = app.advance_focus();

        assert_eq!(app.focus, Focus::Sessions);
    }

    #[test]
    fn switching_sessions_resets_unread_and_scroll() {
        let mut app = App::new("/tmp/mosaic".into());
        app.timeline_scroll = 8;
        app.sessions[1].unread = 3;

        app.select_session(1);

        assert_eq!(app.timeline_scroll, 0);
        assert_eq!(app.sessions[1].unread, 0);
    }

    #[test]
    fn submitting_composer_appends_operator_entry() {
        let mut app = App::new("/tmp/mosaic".into());
        let initial_len = app.active_session().timeline.len();
        app.active_session_mut().draft = "Trace gateway route".to_owned();

        app.submit_composer();

        assert_eq!(app.active_draft(), "");
        assert_eq!(app.active_session().timeline.len(), initial_len + 1);
        assert_eq!(
            app.active_session().timeline.last().map(|entry| entry.kind),
            Some(TimelineKind::Operator)
        );
    }

    #[test]
    fn drafts_are_session_scoped() {
        let mut app = App::new("/tmp/mosaic".into());
        app.active_session_mut().draft = "gateway command".to_owned();

        app.select_session(1);
        app.active_session_mut().draft = "release note".to_owned();

        app.select_session(0);
        assert_eq!(app.active_draft(), "gateway command");

        app.select_session(1);
        assert_eq!(app.active_draft(), "release note");
    }

    #[test]
    fn gateway_command_updates_connection_state() {
        let mut app = App::new("/tmp/mosaic".into());
        app.active_session_mut().draft = "/gateway disconnect".to_owned();

        app.submit_composer();

        assert!(!app.gateway_connected);
        assert_eq!(
            app.active_session().timeline.last().map(|entry| entry.kind),
            Some(TimelineKind::System)
        );
    }

    #[test]
    fn session_state_command_updates_selected_session() {
        let mut app = App::new("/tmp/mosaic".into());
        app.active_session_mut().draft = "/session state degraded".to_owned();

        app.submit_composer();

        assert_eq!(app.active_session().state, SessionState::Degraded);
    }

    #[test]
    fn interactive_session_new_command_returns_switch_action() {
        let mut app = App::new_interactive(
            "/tmp/mosaic".into(),
            "demo".to_owned(),
            "mock".to_owned(),
            "mock".to_owned(),
            Vec::new(),
            false,
        );
        app.active_session_mut().draft = "/session new ops-2".to_owned();

        let action = app.submit_composer();

        assert_eq!(action, AppAction::SwitchSession("ops-2".to_owned()));
        assert_eq!(app.session_label(), "ops-2");
    }

    #[test]
    fn slash_input_switches_app_into_command_mode() {
        let mut app = App::new("/tmp/mosaic".into());
        app.active_session_mut().draft = "/model".to_owned();

        assert_eq!(app.input_mode(), InputMode::Command);
        assert!(app.enter_hint().contains("Complete"));
        assert_eq!(app.escape_hint(), "focus sessions");
    }

    #[test]
    fn help_overlay_opens_and_blocks_navigation_until_closed() {
        let mut app = App::new("/tmp/mosaic".into());
        let initial_focus = app.focus;

        assert_eq!(
            app.handle_key(KeyEvent::new(KeyCode::F(1), KeyModifiers::NONE)),
            AppAction::Continue
        );
        assert!(app.show_help_overlay);

        app.handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
        assert!(app.show_help_overlay);
        assert_eq!(app.focus, initial_focus);

        app.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        assert!(!app.show_help_overlay);
    }

    #[test]
    fn question_mark_remains_typable_inside_composer() {
        let mut app = App::new("/tmp/mosaic".into());
        app.focus = Focus::Composer;

        app.handle_key(KeyEvent::new(KeyCode::Char('?'), KeyModifiers::NONE));

        assert_eq!(app.active_draft(), "?");
        assert!(!app.show_help_overlay);
    }

    #[test]
    fn command_menu_selection_completes_draft_without_submitting() {
        let mut app = App::new("/tmp/mosaic".into());
        app.focus = Focus::Composer;
        app.active_session_mut().draft = "/g".to_owned();

        app.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
        app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

        assert_eq!(app.active_draft(), "/gateway disconnect ");
        assert_eq!(
            app.active_session()
                .timeline
                .last()
                .map(|entry| entry.title.as_str()),
            Some("Working")
        );
    }

    #[test]
    fn tab_cycles_command_menu_selection() {
        let mut app = App::new("/tmp/mosaic".into());
        app.focus = Focus::Composer;
        app.active_session_mut().draft = "/g".to_owned();

        app.handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
        app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

        assert_eq!(app.active_draft(), "/gateway disconnect ");
    }

    #[test]
    fn app_can_start_in_resume_surface() {
        let app = App::new_with_resume("/tmp/mosaic".into(), true);

        assert_eq!(app.surface, Surface::Resume);
        assert_eq!(app.focus, Focus::Composer);
    }

    #[test]
    fn resume_surface_filters_sessions_and_selects_visible_match() {
        let mut app = App::new_with_resume("/tmp/mosaic".into(), true);
        app.resume_scope = ResumeScope::Remote;
        app.resume_query = "ios".to_owned();
        app.ensure_selected_session_visible();

        let visible = app.visible_session_indices();

        assert_eq!(visible.len(), 1);
        assert_eq!(app.sessions[visible[0]].id, "sess-node-007");
        assert_eq!(app.selected_session, visible[0]);
    }

    #[test]
    fn enter_from_resume_returns_to_console() {
        let mut app = App::new_with_resume("/tmp/mosaic".into(), true);

        app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

        assert_eq!(app.surface, Surface::Console);
        assert_eq!(app.focus, Focus::Composer);
    }

    #[test]
    fn apply_run_event_updates_activity_timeline_and_runtime_status() {
        let mut app = App::new("/tmp/mosaic".into());
        let initial_activity_len = app.activity.len();
        let initial_timeline_len = app.active_session().timeline.len();

        app.apply_run_event(RunEvent::RunStarted {
            run_id: "run-1".to_owned(),
            input: "Explain what happened in the control plane".to_owned(),
        });

        assert_eq!(app.runtime_status, "running");
        assert!(app.show_console_history);
        assert_eq!(app.activity.len(), initial_activity_len + 1);
        assert_eq!(
            app.active_session().timeline.len(),
            initial_timeline_len + 1
        );
        assert_eq!(
            app.activity.last().map(|entry| entry.scope.as_str()),
            Some("runtime")
        );
        assert_eq!(
            app.active_session()
                .timeline
                .last()
                .map(|entry| entry.title.as_str()),
            Some("Run started")
        );
    }

    #[test]
    fn apply_run_event_marks_failures_in_activity_and_timeline() {
        let mut app = App::new("/tmp/mosaic".into());

        app.apply_run_event(RunEvent::ToolFailed {
            name: "read_file".to_owned(),
            call_id: "call-123".to_owned(),
            error: "permission denied".to_owned(),
        });

        assert_eq!(app.runtime_status, "error");
        assert_eq!(
            app.activity.last().map(|entry| entry.message.as_str()),
            Some("Tool failed: read_file")
        );
        let last = app
            .active_session()
            .timeline
            .last()
            .expect("timeline entry should exist");
        assert_eq!(last.kind, TimelineKind::System);
        assert_eq!(last.title, "Tool failed: read_file");
        assert!(last.body.contains("call_id=call-123"));
        assert!(last.body.contains("permission denied"));
    }

    #[test]
    fn apply_tool_calling_appends_tool_activity_and_timeline() {
        let mut app = App::new("/tmp/mosaic".into());

        app.apply_run_event(RunEvent::ToolCalling {
            name: "read_file".to_owned(),
            call_id: "call_123".to_owned(),
        });

        let last_activity = app.activity.last().expect("activity entry should exist");
        assert_eq!(last_activity.scope, "tool");
        assert_eq!(last_activity.message, "Calling tool: read_file");

        let last_timeline = app
            .active_session()
            .timeline
            .last()
            .expect("timeline entry should exist");
        assert!(last_timeline.title.contains("Tool call"));
        assert!(last_timeline.title.contains("read_file"));
        assert!(last_timeline.body.contains("call_123"));
    }

    #[test]
    fn apply_run_finished_sets_runtime_status_to_idle() {
        let mut app = App::new("/tmp/mosaic".into());
        app.runtime_status = "running".to_owned();

        app.apply_run_event(RunEvent::RunFinished {
            run_id: "run-1".to_owned(),
            output_preview: "done".to_owned(),
        });

        assert_eq!(app.runtime_status, "idle");

        let last_activity = app.activity.last().expect("activity entry should exist");
        assert_eq!(last_activity.scope, "runtime");
        assert_eq!(last_activity.message, "Run finished");

        let last_timeline = app
            .active_session()
            .timeline
            .last()
            .expect("timeline entry should exist");
        assert_eq!(last_timeline.title, "Run finished");
        assert!(last_timeline.body.contains("done"));
    }

    #[test]
    fn apply_run_failed_sets_runtime_status_to_error() {
        let mut app = App::new("/tmp/mosaic".into());

        app.apply_run_event(RunEvent::RunFailed {
            run_id: "run-1".to_owned(),
            error: "boom".to_owned(),
            failure_kind: Some("runtime".to_owned()),
            failure_origin: Some("runtime".to_owned()),
        });

        assert_eq!(app.runtime_status, "error");

        let last_activity = app.activity.last().expect("activity entry should exist");
        assert_eq!(last_activity.message, "Run failed");

        let last_timeline = app
            .active_session()
            .timeline
            .last()
            .expect("timeline entry should exist");
        assert_eq!(last_timeline.title, "Run failed");
        assert!(last_timeline.body.contains("boom"));
    }
}
