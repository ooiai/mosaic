use std::{collections::BTreeMap, path::PathBuf};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use mosaic_control_protocol::RunDetailDto;
use mosaic_runtime::events::RunEvent;
use mosaic_session_core::{
    SessionRecord as StoredSessionRecord, TranscriptMessage, TranscriptRole, session_route_for_id,
};

use crate::mock;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandCategory {
    Gateway,
    Adapter,
    Node,
    Session,
    Model,
    Run,
    Sandbox,
    Tool,
    Skill,
    Workflow,
    Inspect,
    Ui,
}

impl CommandCategory {
    pub fn label(self) -> &'static str {
        match self {
            Self::Gateway => "gateway",
            Self::Adapter => "adapter",
            Self::Node => "node",
            Self::Session => "session",
            Self::Model => "model",
            Self::Run => "run",
            Self::Sandbox => "sandbox",
            Self::Tool => "tool",
            Self::Skill => "skill",
            Self::Workflow => "workflow",
            Self::Inspect => "inspect",
            Self::Ui => "ui",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "gateway" => Some(Self::Gateway),
            "adapter" | "adapters" => Some(Self::Adapter),
            "node" | "nodes" => Some(Self::Node),
            "session" | "sessions" => Some(Self::Session),
            "model" | "models" | "profile" | "profiles" => Some(Self::Model),
            "run" | "runs" => Some(Self::Run),
            "sandbox" | "sandboxes" => Some(Self::Sandbox),
            "tool" | "tools" => Some(Self::Tool),
            "skill" | "skills" => Some(Self::Skill),
            "workflow" | "workflows" => Some(Self::Workflow),
            "ui" | "command" | "commands" => Some(Self::Ui),
            _ => None,
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

pub const LOCAL_COMMANDS: [CommandSpec; 23] = [
    CommandSpec {
        command: "/mosaic",
        category: CommandCategory::Ui,
        summary: "Show the Mosaic command catalog inline",
        usage: "/mosaic",
        detail: "Renders the grouped Mosaic command catalog for this gateway-backed TUI session directly into the transcript.",
    },
    CommandSpec {
        command: "/mosaic help",
        category: CommandCategory::Ui,
        summary: "Show grouped help or one command category",
        usage: "/mosaic help [category]",
        detail: "Lists grouped commands for this TUI session or narrows the output to one category such as session, model, sandbox, tool, skill, workflow, gateway, node, adapter, or inspect.",
    },
    CommandSpec {
        command: "/mosaic gateway status",
        category: CommandCategory::Gateway,
        summary: "Show gateway transport and readiness",
        usage: "/mosaic gateway status",
        detail: "Prints the current gateway transport, readiness, node summary, and adapter state into the transcript. Short alias: /gateway status.",
    },
    CommandSpec {
        command: "/mosaic adapter status",
        category: CommandCategory::Adapter,
        summary: "Show adapter readiness and outbound state",
        usage: "/mosaic adapter status",
        detail: "Prints registered adapters, bot bindings, outbound readiness, and channel-facing detail inline in the transcript. Short alias: /adapter status.",
    },
    CommandSpec {
        command: "/mosaic node list",
        category: CommandCategory::Node,
        summary: "List node health and affinity state",
        usage: "/mosaic node list",
        detail: "Shows registered nodes, health, disconnect state, and whether the current session is pinned to a specific node. Short alias: /node list.",
    },
    CommandSpec {
        command: "/mosaic node show <id>",
        category: CommandCategory::Node,
        summary: "Inspect one node and its declared capabilities",
        usage: "/mosaic node show <id>",
        detail: "Prints one node's transport, platform, health, disconnect reason, and declared capabilities into the transcript. Short alias: /node show <id>.",
    },
    CommandSpec {
        command: "/mosaic session list",
        category: CommandCategory::Session,
        summary: "List known sessions",
        usage: "/mosaic session list",
        detail: "Shows the sessions currently loaded into this TUI context so the operator can switch without leaving chat. Short alias: /session list.",
    },
    CommandSpec {
        command: "/mosaic session new <id>",
        category: CommandCategory::Session,
        summary: "Create or stage a new session",
        usage: "/mosaic session new <id>",
        detail: "Creates a new active session target. The next submitted turn persists the session if it does not already exist. Short alias: /session new <id>.",
    },
    CommandSpec {
        command: "/mosaic session show",
        category: CommandCategory::Session,
        summary: "Explain the current session binding",
        usage: "/mosaic session show",
        detail: "Prints the active session route, run ids, channel metadata, memory summary, compressed context, and references. Short alias: /session show.",
    },
    CommandSpec {
        command: "/mosaic session switch <id>",
        category: CommandCategory::Session,
        summary: "Switch the active conversation",
        usage: "/mosaic session switch <id>",
        detail: "Moves the composer to another session and refreshes that session from the gateway or local runtime store. Short alias: /session switch <id>.",
    },
    CommandSpec {
        command: "/mosaic model list",
        category: CommandCategory::Model,
        summary: "List available runtime profiles",
        usage: "/mosaic model list",
        detail: "Prints configured runtime profiles with provider type and model so the next turn can be scheduled intentionally. Short aliases: /model list and /profile list.",
    },
    CommandSpec {
        command: "/mosaic model use <profile>",
        category: CommandCategory::Model,
        summary: "Switch the profile for future turns",
        usage: "/mosaic model use <profile>",
        detail: "Updates the active profile used by interactive submissions; the next message will use the new profile. Short aliases: /model use <profile> and /profile <name>.",
    },
    CommandSpec {
        command: "/mosaic model show",
        category: CommandCategory::Model,
        summary: "Show the current profile and model binding",
        usage: "/mosaic model show",
        detail: "Explains which runtime profile is selected for the next turn and which provider/model the current session last used. Short aliases: /model show and /profile show.",
    },
    CommandSpec {
        command: "/mosaic run stop",
        category: CommandCategory::Run,
        summary: "Cancel the active run for this session",
        usage: "/mosaic run stop",
        detail: "Sends a real cancel request to the attached gateway for the current run when one is active. Short alias: /run stop.",
    },
    CommandSpec {
        command: "/mosaic run retry",
        category: CommandCategory::Run,
        summary: "Retry the last completed run",
        usage: "/mosaic run retry",
        detail: "Requests a real retry from the attached gateway using the last known gateway run id for this session. Short alias: /run retry.",
    },
    CommandSpec {
        command: "/mosaic sandbox status",
        category: CommandCategory::Sandbox,
        summary: "Show workspace sandbox lifecycle status",
        usage: "/mosaic sandbox status",
        detail: "Prints Python and Node sandbox strategies, install policies, runtime availability, and current env counts into the transcript. Short alias: /sandbox status.",
    },
    CommandSpec {
        command: "/mosaic sandbox inspect <env>",
        category: CommandCategory::Sandbox,
        summary: "Inspect one sandbox env record",
        usage: "/mosaic sandbox inspect <env>",
        detail: "Loads one sandbox env record and renders lifecycle state, install policy, dependencies, and failure details inline. Short alias: /sandbox inspect <env>.",
    },
    CommandSpec {
        command: "/mosaic sandbox rebuild <env>",
        category: CommandCategory::Sandbox,
        summary: "Rebuild a sandbox env",
        usage: "/mosaic sandbox rebuild <env>",
        detail: "Deletes and recreates one sandbox env so the next capability run can reuse a fresh local execution environment. Short alias: /sandbox rebuild <env>.",
    },
    CommandSpec {
        command: "/mosaic sandbox clean",
        category: CommandCategory::Sandbox,
        summary: "Clean sandbox run and attachment workdirs",
        usage: "/mosaic sandbox clean",
        detail: "Removes sandbox run workdirs and attachment workdirs without leaving the chat transcript. Short alias: /sandbox clean.",
    },
    CommandSpec {
        command: "/mosaic inspect last",
        category: CommandCategory::Inspect,
        summary: "Inspect the most recent run inline",
        usage: "/mosaic inspect last",
        detail: "Fetches the most recent run detail for the active session and renders the summary inline in the transcript. Short alias: /inspect last.",
    },
    CommandSpec {
        command: "/mosaic tool <name> <input>",
        category: CommandCategory::Tool,
        summary: "Invoke a tool explicitly",
        usage: "/mosaic tool <name> <input>",
        detail: "Submits a real run that explicitly targets one tool and shows the capability events inline in the transcript. Short alias: /tool <name> <input>.",
    },
    CommandSpec {
        command: "/mosaic skill <name> <input>",
        category: CommandCategory::Skill,
        summary: "Invoke a skill explicitly",
        usage: "/mosaic skill <name> <input>",
        detail: "Submits a real run that explicitly targets one skill and streams the result back into the active transcript. Short alias: /skill <name> <input>.",
    },
    CommandSpec {
        command: "/mosaic workflow <name> <input>",
        category: CommandCategory::Workflow,
        summary: "Invoke a workflow explicitly",
        usage: "/mosaic workflow <name> <input>",
        detail: "Submits a real run that explicitly targets one workflow and renders step activity inline in the transcript. Short alias: /workflow <name> <input>.",
    },
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComposerRunRequest {
    pub input: String,
    pub tool: Option<String>,
    pub skill: Option<String>,
    pub workflow: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppAction {
    Continue,
    Quit,
    SubmitRun(ComposerRunRequest),
    GatewayConnect,
    GatewayDisconnect,
    AdapterStatus,
    NodeList,
    NodeShow(String),
    SandboxStatus,
    SandboxInspect(String),
    SandboxRebuild(String),
    SandboxClean,
    SwitchSession(String),
    CancelRun(String),
    RetryRun(String),
    InspectRun(String),
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillOption {
    pub name: String,
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
    pub transcript_len: usize,
    pub current_run_id: Option<String>,
    pub current_gateway_run_id: Option<String>,
    pub last_gateway_run_id: Option<String>,
    pub memory_summary: Option<String>,
    pub compressed_context: Option<String>,
    pub references: Vec<String>,
    pub streaming_preview: Option<String>,
    pub streaming_run_id: Option<String>,
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
    pub gateway_target: String,
    pub runtime_status: String,
    pub control_model: String,
    pub selected_profile: String,
    pub available_profiles: Vec<ProfileOption>,
    pub available_skills: Vec<SkillOption>,
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

        let mut app = Self {
            mode: AppMode::Mock,
            workspace_name,
            workspace_path,
            sessions: mock::sessions(),
            activity: mock::activity_feed(),
            surface: Surface::Console,
            selected_session: 0,
            resume_scope: ResumeScope::Local,
            resume_query: String::new(),
            resume_search: false,
            command_menu_index: 0,
            show_console_history: true,
            focus: Focus::Composer,
            show_observability: false,
            show_help_overlay: false,
            timeline_scroll: 0,
            observability_scroll: 0,
            gateway_connected: true,
            gateway_target: "local".to_owned(),
            runtime_status: "warm".to_owned(),
            control_model: "gpt-5.4-control".to_owned(),
            selected_profile: "gpt-5.4-control".to_owned(),
            available_profiles: Vec::new(),
            available_skills: Vec::new(),
            extension_summary: None,
            extension_policy_summary: None,
            extension_errors: Vec::new(),
            gateway_summary: None,
            gateway_detail: None,
            node_summary: None,
            node_detail: None,
            heartbeat: 0,
        };
        if start_in_resume {
            app.push_system_entry(
                "Chat-first TUI",
                "Resume mode now stays inside the transcript. Use /session list to browse sessions and /session switch <id> to jump.",
            );
        }
        app
    }

    pub fn new_interactive(
        workspace_path: PathBuf,
        session_id: String,
        profile: String,
        model: String,
        available_profiles: Vec<ProfileOption>,
        available_skills: Vec<SkillOption>,
        start_in_resume: bool,
    ) -> Self {
        let workspace_name = workspace_path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("workspace")
            .to_owned();
        let workspace_path = workspace_path.display().to_string();

        let mut app = Self {
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
            surface: Surface::Console,
            selected_session: 0,
            resume_scope: ResumeScope::Local,
            resume_query: String::new(),
            resume_search: false,
            command_menu_index: 0,
            show_console_history: true,
            focus: Focus::Composer,
            show_observability: false,
            show_help_overlay: false,
            timeline_scroll: 0,
            observability_scroll: 0,
            gateway_connected: true,
            gateway_target: "local".to_owned(),
            runtime_status: "idle".to_owned(),
            control_model: model,
            selected_profile: profile,
            available_profiles,
            available_skills,
            extension_summary: None,
            extension_policy_summary: None,
            extension_errors: Vec::new(),
            gateway_summary: None,
            gateway_detail: None,
            node_summary: None,
            node_detail: None,
            heartbeat: 0,
        };
        if start_in_resume {
            app.push_system_entry(
                "Chat-first TUI",
                "Resume mode now stays inside the transcript. Use /session list or /session switch <id> to move between sessions.",
            );
        }
        app
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

    pub fn set_gateway_target(&mut self, target: impl Into<String>) {
        self.gateway_target = target.into();
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
            let transcript_len = session.transcript.len();
            if view.transcript_len == 0 || transcript_len < view.transcript_len {
                view.timeline = session.transcript.iter().map(transcript_entry).collect();
            } else {
                for message in session.transcript.iter().skip(view.transcript_len) {
                    view.timeline.push(transcript_entry(message));
                }
            }
            view.transcript_len = transcript_len;
            if !runtime_status_is_busy(run_label) {
                view.streaming_preview = None;
                view.streaming_run_id = None;
            }
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
            view.current_run_id = session.run.current_run_id.clone();
            view.current_gateway_run_id = session.run.current_gateway_run_id.clone();
            view.last_gateway_run_id = session.gateway.last_gateway_run_id.clone();
            view.memory_summary = session.memory.latest_summary.clone();
            view.compressed_context = session.memory.compressed_context.clone();
            view.references = session
                .references
                .iter()
                .map(|reference| format!("{} ({})", reference.session_id, reference.reason))
                .collect();
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

        if matches!(key.code, KeyCode::F(1))
            || (matches!(key.code, KeyCode::Char('?')) && self.active_draft().is_empty())
        {
            self.push_help(None);
            return AppAction::Continue;
        }

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
                KeyCode::Tab if self.command_menu_should_complete() => {
                    self.complete_selected_command();
                    return AppAction::Continue;
                }
                _ => {}
            }
        }

        match key.code {
            KeyCode::Esc => {
                if self.command_menu_active() || !self.active_draft().is_empty() {
                    self.active_session_mut().draft.clear();
                    self.command_menu_index = 0;
                }
                AppAction::Continue
            }
            KeyCode::PageDown => {
                self.timeline_scroll = self.timeline_scroll.saturating_add(5);
                AppAction::Continue
            }
            KeyCode::PageUp => {
                self.timeline_scroll = self.timeline_scroll.saturating_sub(5);
                AppAction::Continue
            }
            KeyCode::Home => {
                self.timeline_scroll = 0;
                AppAction::Continue
            }
            KeyCode::End => {
                self.timeline_scroll = self.timeline_scroll.saturating_add(20);
                AppAction::Continue
            }
            _ => self.handle_composer_key(key),
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
                summary,
            } => {
                self.push_activity(
                    "workflow",
                    format!("Step started: {}.{} ({})", workflow, step, kind),
                );
                self.push_timeline(
                    TimelineKind::Agent,
                    "workflow",
                    &format!("Workflow step: {}", step),
                    &match summary {
                        Some(summary) => {
                            format!("workflow={}\nkind={}\n{}", workflow, kind, summary)
                        }
                        None => format!("workflow={}\nkind={}", workflow, kind),
                    },
                );
            }
            RunEvent::WorkflowStepFinished {
                workflow,
                step,
                summary,
            } => {
                self.push_activity("workflow", format!("Step finished: {}.{}", workflow, step));
                self.push_timeline(
                    TimelineKind::Agent,
                    "workflow",
                    &format!("Workflow step finished: {}", step),
                    &match summary {
                        Some(summary) => format!("workflow={}\n{}", workflow, summary),
                        None => format!("workflow={}", workflow),
                    },
                );
            }
            RunEvent::WorkflowStepFailed {
                workflow,
                step,
                error,
                summary,
            } => {
                self.runtime_status = "error".to_owned();
                self.active_session_mut().state = SessionState::Degraded;
                self.push_activity("workflow", format!("Step failed: {}.{}", workflow, step));
                self.push_timeline(
                    TimelineKind::System,
                    "workflow",
                    &format!("Workflow step failed: {}", step),
                    &match summary {
                        Some(summary) => format!(
                            "workflow={}\n{}\nerror={}",
                            workflow,
                            summary,
                            truncate_for_timeline(&error, 180)
                        ),
                        None => format!(
                            "workflow={}\nerror={}",
                            workflow,
                            truncate_for_timeline(&error, 180)
                        ),
                    },
                );
            }
            RunEvent::WorkflowFinished { name } => {
                self.push_activity("workflow", format!("Workflow finished: {}", name));
                self.push_timeline(
                    TimelineKind::Agent,
                    "workflow",
                    "Workflow finished",
                    &format!("Workflow: {}", name),
                );
            }
            RunEvent::SkillStarted { name, summary } => {
                self.push_activity("skill", &format!("Executing skill: {}", name));
                self.push_timeline(
                    TimelineKind::Agent,
                    "skill",
                    "Skill started",
                    &match summary {
                        Some(summary) => format!("Skill: {}\n{}", name, summary),
                        None => format!("Skill: {}", name),
                    },
                );
            }
            RunEvent::SkillFinished { name, summary } => {
                self.push_activity("skill", &format!("Skill finished: {}", name));
                self.push_timeline(
                    TimelineKind::Agent,
                    "skill",
                    "Skill finished",
                    &match summary {
                        Some(summary) => format!("Skill: {}\n{}", name, summary),
                        None => format!("Skill: {}", name),
                    },
                );
            }
            RunEvent::SkillFailed {
                name,
                error,
                summary,
            } => {
                self.runtime_status = "error".to_owned();
                self.active_session_mut().state = SessionState::Degraded;
                self.push_activity("skill", &format!("Skill failed: {}", name));
                self.push_timeline(
                    TimelineKind::System,
                    "skill",
                    "Skill failed",
                    &match summary {
                        Some(summary) => format!(
                            "Skill: {}\n{}\nError: {}",
                            name,
                            summary,
                            truncate_for_timeline(&error, 180)
                        ),
                        None => format!(
                            "Skill: {}\nError: {}",
                            name,
                            truncate_for_timeline(&error, 180)
                        ),
                    },
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
            RunEvent::ToolCalling {
                name,
                call_id,
                summary,
            } => {
                self.push_activity("tool", &format!("Calling tool: {}", name));
                self.push_timeline(
                    TimelineKind::Tool,
                    "tool",
                    &format!("Tool call: {}", name),
                    &match summary {
                        Some(summary) => format!("call_id={}\n{}", call_id, summary),
                        None => format!("call_id={}", call_id),
                    },
                );
            }
            RunEvent::ToolFinished {
                name,
                call_id,
                summary,
            } => {
                self.push_activity("tool", &format!("Tool finished: {}", name));
                self.push_timeline(
                    TimelineKind::Tool,
                    "tool",
                    &format!("Tool finished: {}", name),
                    &match summary {
                        Some(summary) => format!("call_id={}\n{}", call_id, summary),
                        None => format!("call_id={}", call_id),
                    },
                );
            }
            RunEvent::ToolFailed {
                name,
                call_id,
                error,
                summary,
            } => {
                self.runtime_status = "error".to_owned();
                self.active_session_mut().state = SessionState::Degraded;
                self.push_activity("tool", &format!("Tool failed: {}", name));
                self.push_timeline(
                    TimelineKind::System,
                    "tool",
                    &format!("Tool failed: {}", name),
                    &match summary {
                        Some(summary) => format!(
                            "call_id={}\n{}\nerror={}",
                            call_id,
                            summary,
                            truncate_for_timeline(&error, 180)
                        ),
                        None => format!(
                            "call_id={}\nerror={}",
                            call_id,
                            truncate_for_timeline(&error, 180)
                        ),
                    },
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
                    let session = self.active_session_mut();
                    session.streaming_run_id = Some(run_id);
                    let preview = session.streaming_preview.get_or_insert_with(String::new);
                    preview.push_str(&chunk);
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
                    self.push_timeline(
                        TimelineKind::Agent,
                        "runtime",
                        "Final answer ready",
                        &format!("run_id={run_id}\nWaiting for the final transcript to land."),
                    );
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
                    self.push_timeline(
                        TimelineKind::Agent,
                        "runtime",
                        "Run finished",
                        &format!(
                            "run_id={}\nOutput preview: {}",
                            run_id,
                            truncate_for_timeline(&output_preview, 180)
                        ),
                    );
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
                self.active_session_mut().streaming_preview = None;
                self.active_session_mut().streaming_run_id = None;
                self.push_activity("runtime", "Run failed");
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
                self.active_session_mut().streaming_preview = None;
                self.active_session_mut().streaming_run_id = None;
                self.push_activity("runtime", "Run canceled");
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

    pub fn active_streaming_preview(&self) -> Option<&str> {
        self.active_session().streaming_preview.as_deref()
    }

    pub fn current_run_identifier(&self) -> Option<&str> {
        self.active_session()
            .current_gateway_run_id
            .as_deref()
            .or(self.active_session().current_run_id.as_deref())
    }

    pub fn last_run_identifier(&self) -> Option<&str> {
        self.active_session()
            .last_gateway_run_id
            .as_deref()
            .or(self.active_session().current_gateway_run_id.as_deref())
            .or(self.active_session().current_run_id.as_deref())
    }

    pub fn visible_timeline(&self) -> &[TimelineEntry] {
        &self.active_session().timeline
    }

    pub fn input_mode(&self) -> InputMode {
        if self.active_draft().trim_start().starts_with('/') {
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
                "Gateway paused ({}). Reconnect it before sending another turn for session {}.",
                self.gateway_target,
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
                "Send a message to the active session. Type / to browse the /mosaic command catalog."
            }
            InputMode::Command => {
                "Run a /mosaic command. Tab accepts the highlighted command and Enter executes it."
            }
            InputMode::Search => "Type / to browse commands.",
        }
    }

    pub fn gateway_target_label(&self) -> &str {
        &self.gateway_target
    }

    pub fn enter_hint(&self) -> String {
        match self.input_mode() {
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
                    "Run the slash command".to_owned()
                }
            }
            InputMode::Search => "Apply the search filter".to_owned(),
        }
    }

    pub fn escape_hint(&self) -> &'static str {
        "clear draft or close the command popup"
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

    pub fn command_completion_suffix(&self) -> Option<String> {
        if let Some(completion) = self.selected_skill_completion() {
            return completion
                .strip_prefix(self.active_draft())
                .filter(|suffix| !suffix.is_empty())
                .map(str::to_owned);
        }

        let command = self.selected_command_match()?;
        let completion = command_completion(command.command);
        let draft = self.active_draft();
        completion
            .strip_prefix(draft)
            .filter(|suffix| !suffix.is_empty())
            .map(str::to_owned)
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
                if session.transcript_len == 0 {
                    session.transcript_len = existing.transcript_len;
                }
                if session.current_run_id.is_none() {
                    session.current_run_id = existing.current_run_id.clone();
                }
                if session.current_gateway_run_id.is_none() {
                    session.current_gateway_run_id = existing.current_gateway_run_id.clone();
                }
                if session.last_gateway_run_id.is_none() {
                    session.last_gateway_run_id = existing.last_gateway_run_id.clone();
                }
                if session.streaming_preview.is_none() {
                    session.streaming_preview = existing.streaming_preview.clone();
                }
                if session.streaming_run_id.is_none() {
                    session.streaming_run_id = existing.streaming_run_id.clone();
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
            let mut placeholder = self
                .sessions
                .iter()
                .find(|session| session.id == selected_session_id)
                .cloned()
                .unwrap_or_else(|| {
                    interactive_session_record(selected_session_id, &self.control_model)
                });
            if placeholder.origin.is_empty() {
                placeholder.origin = default_origin;
            }
            if placeholder.runtime.is_empty() {
                placeholder.runtime = default_runtime;
            }
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
                self.push_system_entry(
                    "Assistant pending",
                    format!(
                        "Waiting for session {} to reply via profile {} ({}).\ninput: {}",
                        session_id,
                        profile,
                        model,
                        truncate_for_timeline(&message, 180)
                    ),
                );
                self.push_activity(
                    "composer",
                    format!("Submitted message to session {}", session_id),
                );
                AppAction::SubmitRun(ComposerRunRequest {
                    input: message.clone(),
                    tool: None,
                    skill: None,
                    workflow: None,
                })
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

    fn select_session(&mut self, index: usize) {
        self.selected_session = index;
        self.timeline_scroll = 0;
        self.sessions[index].unread = 0;
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

    fn command_menu_active(&self) -> bool {
        self.focus == Focus::Composer && self.command_query().is_some()
    }

    fn command_menu_should_complete(&self) -> bool {
        if !self.command_menu_active() {
            return false;
        }

        if command_invocation_ready(self.active_draft()) {
            return false;
        }

        if let Some(completion) = self.selected_skill_completion() {
            return self.active_draft().trim_end() != completion.trim_end();
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
        if let Some(completion) = self.selected_skill_completion() {
            self.active_session_mut().draft = completion;
            self.command_menu_index = 0;
            return;
        }

        let Some(command) = self.selected_command_match() else {
            return;
        };

        self.active_session_mut().draft = command_completion(command.command);
        self.command_menu_index = 0;
    }

    fn selected_skill_completion(&self) -> Option<String> {
        let draft = self.active_draft().trim_start();
        let (command_prefix, prefix) = if let Some(prefix) = draft.strip_prefix("/mosaic skill ") {
            ("/mosaic skill ", prefix)
        } else if let Some(prefix) = draft.strip_prefix("/mosaic skill") {
            ("/mosaic skill ", prefix)
        } else if let Some(prefix) = draft.strip_prefix("/skill ") {
            ("/skill ", prefix)
        } else if let Some(prefix) = draft.strip_prefix("/skill") {
            ("/skill ", prefix)
        } else {
            return None;
        };
        let query = prefix.trim();
        let matches = self
            .available_skills
            .iter()
            .filter(|skill| {
                query.is_empty()
                    || skill
                        .name
                        .to_ascii_lowercase()
                        .starts_with(&query.to_ascii_lowercase())
            })
            .collect::<Vec<_>>();
        let selected = matches.get(self.command_menu_index.min(matches.len().saturating_sub(1)))?;
        Some(format!("{command_prefix}{} ", selected.name))
    }

    fn route_command(&mut self, command: &str) -> AppAction {
        let parts = command.split_whitespace().collect::<Vec<_>>();
        let Some(name) = parts.first().copied() else {
            self.push_command_error("Usage: /mosaic help");
            return AppAction::Continue;
        };

        match name {
            "mosaic" => match parts.as_slice() {
                ["mosaic"] => {
                    self.push_help(None);
                    AppAction::Continue
                }
                ["mosaic", "help"] => {
                    self.push_help(None);
                    AppAction::Continue
                }
                ["mosaic", "help", category, ..] => {
                    if let Some(category) = CommandCategory::parse(category) {
                        self.push_help(Some(category));
                    } else {
                        self.push_command_error(format!(
                            "Unknown /mosaic help category: {}. Use /mosaic help to browse the grouped catalog.",
                            category
                        ));
                    }
                    AppAction::Continue
                }
                ["mosaic", root, rest @ ..] => self.route_command_root(root, rest),
                _ => AppAction::Continue,
            },
            "help" => {
                self.push_help(None);
                AppAction::Continue
            }
            "gateway" | "adapter" | "node" | "sandbox" | "model" | "profile" | "run"
            | "inspect" | "tool" | "skill" | "workflow" | "session" => {
                self.route_command_root(name, &parts[1..])
            }
            _ => {
                self.push_command_error(unknown_command_message(command));
                AppAction::Continue
            }
        }
    }

    fn route_command_root(&mut self, name: &str, args: &[&str]) -> AppAction {
        match name {
            "gateway" => self.route_gateway_command(args.to_vec()),
            "adapter" => self.route_adapter_command(args.to_vec()),
            "node" => self.route_node_command(args.to_vec()),
            "sandbox" => self.route_sandbox_command(args.to_vec()),
            "model" => self.route_model_command(args.to_vec()),
            "profile" => self.route_profile_command(args.to_vec()),
            "run" => self.route_run_command(args.to_vec()),
            "inspect" => self.route_inspect_command(args.to_vec()),
            "tool" => self.route_explicit_capability("tool", args.to_vec()),
            "skill" => self.route_explicit_capability("skill", args.to_vec()),
            "workflow" => self.route_explicit_capability("workflow", args.to_vec()),
            "session" => self.route_session_command(args.to_vec()),
            _ => {
                self.push_command_error(unknown_command_message(name));
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
            ["show"] => {
                self.show_model_binding();
                AppAction::Continue
            }
            ["use", rest @ ..] if !rest.is_empty() => {
                self.set_active_profile(&rest.join(" "));
                AppAction::Continue
            }
            _ => {
                self.push_command_error(
                    "Usage: /mosaic model list | /mosaic model show | /mosaic model use <profile>",
                );
                AppAction::Continue
            }
        }
    }

    fn route_profile_command(&mut self, args: Vec<&str>) -> AppAction {
        match args.as_slice() {
            [] | ["show"] => self.route_model_command(vec!["show"]),
            ["list"] => self.route_model_command(vec!["list"]),
            ["use", rest @ ..] if !rest.is_empty() => {
                let mut forwarded = vec!["use"];
                forwarded.extend(rest.iter().copied());
                self.route_model_command(forwarded)
            }
            [profile] => self.route_model_command(vec!["use", profile]),
            _ => {
                self.push_command_error(
                    "Usage: /mosaic profile <name> | /mosaic profile show | /mosaic profile list",
                );
                AppAction::Continue
            }
        }
    }

    fn route_gateway_command(&mut self, args: Vec<&str>) -> AppAction {
        match args.as_slice() {
            ["status"] => {
                self.show_gateway_status();
                AppAction::Continue
            }
            _ => {
                self.push_command_error("Usage: /mosaic gateway status");
                AppAction::Continue
            }
        }
    }

    fn route_adapter_command(&mut self, args: Vec<&str>) -> AppAction {
        match args.as_slice() {
            ["status"] => {
                self.push_activity("adapter", "Requested adapter status.");
                self.push_system_entry(
                    "Adapter status requested",
                    "Loading adapter readiness and outbound status inline.",
                );
                AppAction::AdapterStatus
            }
            _ => {
                self.push_command_error("Usage: /mosaic adapter status");
                AppAction::Continue
            }
        }
    }

    fn route_node_command(&mut self, args: Vec<&str>) -> AppAction {
        match args.as_slice() {
            ["list"] => {
                self.push_activity("node", "Requested node list.");
                self.push_system_entry(
                    "Node list requested",
                    "Loading node health, affinity, and capability summaries inline.",
                );
                AppAction::NodeList
            }
            ["show", rest @ ..] if !rest.is_empty() => {
                let node_id = rest.join(" ");
                self.push_activity("node", format!("Requested node details for {node_id}."));
                self.push_system_entry(
                    "Node inspect requested",
                    format!("Inspecting node {} inline.", node_id),
                );
                AppAction::NodeShow(node_id)
            }
            _ => {
                self.push_command_error("Usage: /mosaic node list | /mosaic node show <id>");
                AppAction::Continue
            }
        }
    }

    fn route_sandbox_command(&mut self, args: Vec<&str>) -> AppAction {
        match args.as_slice() {
            ["status"] => {
                self.push_activity("sandbox", "Requested sandbox status.");
                self.push_system_entry(
                    "Sandbox status requested",
                    "Loading workspace-local sandbox env and runtime status inline.",
                );
                AppAction::SandboxStatus
            }
            ["inspect", rest @ ..] if !rest.is_empty() => {
                let env_id = rest.join(" ");
                self.push_activity(
                    "sandbox",
                    format!("Requested sandbox inspect for {env_id}."),
                );
                self.push_system_entry(
                    "Sandbox inspect requested",
                    format!("Inspecting sandbox env {} inline.", env_id),
                );
                AppAction::SandboxInspect(env_id)
            }
            ["rebuild", rest @ ..] if !rest.is_empty() => {
                let env_id = rest.join(" ");
                self.push_activity(
                    "sandbox",
                    format!("Requested sandbox rebuild for {env_id}."),
                );
                self.push_system_entry(
                    "Sandbox rebuild requested",
                    format!("Rebuilding sandbox env {} inline.", env_id),
                );
                AppAction::SandboxRebuild(env_id)
            }
            ["clean"] => {
                self.push_activity("sandbox", "Requested sandbox clean.");
                self.push_system_entry(
                    "Sandbox clean requested",
                    "Cleaning sandbox run and attachment workdirs inline.",
                );
                AppAction::SandboxClean
            }
            _ => {
                self.push_command_error(
                    "Usage: /mosaic sandbox status | /mosaic sandbox inspect <env> | /mosaic sandbox rebuild <env> | /mosaic sandbox clean",
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
            _ => {
                self.push_command_error(
                    "Usage: /mosaic session list | /mosaic session show | /mosaic session switch <id> | /mosaic session new <id>",
                );
                AppAction::Continue
            }
        }
    }

    fn route_run_command(&mut self, args: Vec<&str>) -> AppAction {
        match args.as_slice() {
            ["stop"] => {
                let Some(run_id) = self.current_run_identifier().map(str::to_owned) else {
                    self.push_command_error("No active run is attached to the current session.");
                    return AppAction::Continue;
                };
                self.push_activity("run", format!("Requested cancellation for {run_id}."));
                self.push_system_entry(
                    "Run cancellation requested",
                    format!("Submitting a real cancel request for run {run_id}."),
                );
                AppAction::CancelRun(run_id)
            }
            ["retry"] => {
                let Some(run_id) = self.last_run_identifier().map(str::to_owned) else {
                    self.push_command_error(
                        "No completed run is available to retry for this session.",
                    );
                    return AppAction::Continue;
                };
                self.push_activity("run", format!("Requested retry for {run_id}."));
                self.push_system_entry(
                    "Run retry requested",
                    format!("Submitting a real retry request for run {run_id}."),
                );
                AppAction::RetryRun(run_id)
            }
            _ => {
                self.push_command_error("Usage: /mosaic run stop | /mosaic run retry");
                AppAction::Continue
            }
        }
    }

    fn route_inspect_command(&mut self, args: Vec<&str>) -> AppAction {
        match args.as_slice() {
            ["last"] => {
                let Some(run_id) = self.last_run_identifier().map(str::to_owned) else {
                    self.push_command_error("No run is available to inspect for this session.");
                    return AppAction::Continue;
                };
                self.push_activity("inspect", format!("Requested inline inspect for {run_id}."));
                AppAction::InspectRun(run_id)
            }
            _ => {
                self.push_command_error("Usage: /mosaic inspect last");
                AppAction::Continue
            }
        }
    }

    fn route_explicit_capability(&mut self, kind: &str, args: Vec<&str>) -> AppAction {
        let Some((name, input)) = split_capability_args(args) else {
            self.push_command_error(match kind {
                "tool" => "Usage: /mosaic tool <name> <input>",
                "skill" => "Usage: /mosaic skill <name> <input>",
                _ => "Usage: /mosaic workflow <name> <input>",
            });
            return AppAction::Continue;
        };

        let request = ComposerRunRequest {
            input: input.to_owned(),
            tool: (kind == "tool").then_some(name.to_owned()),
            skill: (kind == "skill").then_some(name.to_owned()),
            workflow: (kind == "workflow").then_some(name.to_owned()),
        };
        let label = match kind {
            "tool" => "Tool invocation queued",
            "skill" => "Skill invocation queued",
            _ => "Workflow invocation queued",
        };
        self.push_activity(kind, format!("Queued explicit {kind}: {name}"));
        self.push_system_entry(
            label,
            format!(
                "{} {}\ninput: {}",
                capitalize(kind),
                name,
                truncate_for_timeline(&request.input, 180)
            ),
        );
        AppAction::SubmitRun(request)
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
        let current_run = self
            .active_session()
            .current_gateway_run_id
            .clone()
            .or_else(|| self.active_session().current_run_id.clone())
            .unwrap_or_else(|| "none".to_owned());
        let last_run = self
            .active_session()
            .last_gateway_run_id
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
current run={}
last run={}
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
                current_run,
                last_run,
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
                "Unknown session: {}. Use /session list to browse known sessions.",
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

    fn show_model_binding(&mut self) {
        self.push_activity("model", "Displayed active profile and model binding.");
        self.push_system_entry(
            "Model binding",
            format!(
                "next-turn profile={}\nnext-turn model={}\ncurrent runtime={}\ncurrent model={}",
                self.active_profile(),
                self.control_model,
                self.active_session().runtime,
                self.active_session().model
            ),
        );
    }

    pub(crate) fn show_run_detail(&mut self, detail: &RunDetailDto) {
        let summary = &detail.summary;
        let capability_details = if detail.capability_explanations.is_empty() {
            "<none>".to_owned()
        } else {
            detail
                .capability_explanations
                .iter()
                .map(|explanation| {
                    let mut line = format!(
                        "{}:{} | route_kind={} | source={} | exec_target={} | orchestration_owner={} | status={}",
                        explanation.scope,
                        explanation.name,
                        explanation.route_kind.as_deref().unwrap_or("<none>"),
                        explanation
                            .capability_source_kind
                            .as_deref()
                            .unwrap_or("<none>"),
                        explanation.execution_target.as_deref().unwrap_or("<none>"),
                        explanation
                            .orchestration_owner
                            .as_deref()
                            .unwrap_or("<none>"),
                        explanation.status,
                    );
                    if !explanation.summary.is_empty() {
                        line.push_str(&format!(
                            " | summary={}",
                            truncate_for_timeline(&explanation.summary, 120)
                        ));
                    }
                    if let Some(decision_basis) = explanation.decision_basis.as_deref() {
                        line.push_str(&format!(
                            " | decision={}",
                            truncate_for_timeline(decision_basis, 140)
                        ));
                    }
                    if let Some(origin) = explanation.failure_origin.as_deref() {
                        line.push_str(&format!(" | failure_origin={origin}"));
                    }
                    line
                })
                .collect::<Vec<_>>()
                .join("\n")
        };
        self.push_activity(
            "inspect",
            format!(
                "Displayed run {} ({:?})",
                summary.gateway_run_id, summary.status
            ),
        );
        self.push_system_entry(
            "Run inspect",
            format!(
                "run={}\nsession={}\nstatus={:?}\nrequested_profile={}\neffective_profile={}\neffective_provider={}\neffective_model={}\ntool={}\nskill={}\nworkflow={}\ntrace={}\nerror={}\noutput={}\ncapability_proof=\n{}",
                summary.gateway_run_id,
                summary.session_id.as_deref().unwrap_or("<none>"),
                summary.status,
                summary.requested_profile.as_deref().unwrap_or("<none>"),
                summary.effective_profile.as_deref().unwrap_or("<none>"),
                summary.effective_provider_type.as_deref().unwrap_or("<none>"),
                summary.effective_model.as_deref().unwrap_or("<none>"),
                summary.tool.as_deref().unwrap_or("<none>"),
                summary.skill.as_deref().unwrap_or("<none>"),
                summary.workflow.as_deref().unwrap_or("<none>"),
                summary.trace_path.as_deref().unwrap_or("<none>"),
                summary.error.as_deref().unwrap_or("<none>"),
                summary.output_preview.as_deref().unwrap_or("<none>"),
                capability_details,
            ),
        );
    }

    fn push_help(&mut self, selected_category: Option<CommandCategory>) {
        let mut grouped = BTreeMap::new();
        for category in [
            CommandCategory::Ui,
            CommandCategory::Gateway,
            CommandCategory::Adapter,
            CommandCategory::Node,
            CommandCategory::Session,
            CommandCategory::Model,
            CommandCategory::Run,
            CommandCategory::Sandbox,
            CommandCategory::Tool,
            CommandCategory::Skill,
            CommandCategory::Workflow,
            CommandCategory::Inspect,
        ] {
            if selected_category.is_some_and(|selected| selected != category) {
                continue;
            }
            let entries = LOCAL_COMMANDS
                .iter()
                .filter(|spec| spec.category == category)
                .map(|spec| format!("{}  {}", spec.usage, spec.summary))
                .collect::<Vec<_>>();
            grouped.insert(category.label(), entries);
        }

        let mut body = vec![
            format!(
                "input.chat    Enter => {}",
                if self.is_interactive() {
                    "send a real gateway-backed turn to the active session"
                } else {
                    "queue a mock instruction"
                }
            ),
            "input.command / opens the /mosaic popup, typing filters, Tab completes, Enter executes"
                .to_owned(),
            "navigation     PageUp/PageDown scroll the transcript, Ctrl+C quits".to_owned(),
            "aliases        Short forms like /session show and /model list still work, but /mosaic ... is canonical.".to_owned(),
            String::new(),
        ];
        for (category, entries) in grouped {
            if entries.is_empty() {
                continue;
            }
            body.push(format!("[{category}]"));
            body.extend(entries);
            body.push(String::new());
        }
        body.push(format!(
            "current session={} next-profile={} next-model={} current-run={} gateway-backed={}",
            self.session_label(),
            self.active_profile(),
            self.control_model,
            self.current_run_identifier().unwrap_or("<none>"),
            if self.gateway_connected { "yes" } else { "no" }
        ));

        self.push_activity("command", "Displayed TUI command reference.");
        self.push_system_entry(
            match selected_category {
                Some(category) => format!("Mosaic {} command reference", category.label()),
                None => "Mosaic command reference".to_owned(),
            },
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

    pub(crate) fn push_system_entry(&mut self, title: impl Into<String>, body: impl Into<String>) {
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
        transcript_len: 0,
        current_run_id: None,
        current_gateway_run_id: None,
        last_gateway_run_id: None,
        memory_summary: None,
        compressed_context: None,
        references: Vec::new(),
        streaming_preview: None,
        streaming_run_id: None,
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

fn split_capability_args(args: Vec<&str>) -> Option<(&str, String)> {
    let mut iter = args.into_iter();
    let name = iter.next()?;
    let input = iter.collect::<Vec<_>>().join(" ");
    if input.trim().is_empty() {
        return None;
    }
    Some((name, input))
}

fn capitalize(value: &str) -> String {
    let mut chars = value.chars();
    match chars.next() {
        Some(first) => format!("{}{}", first.to_ascii_uppercase(), chars.as_str()),
        None => String::new(),
    }
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

fn command_invocation_ready(draft: &str) -> bool {
    let command = draft.trim().trim_start_matches('/');
    if command.is_empty() {
        return false;
    }

    let parts = command.split_whitespace().collect::<Vec<_>>();
    match parts.as_slice() {
        ["mosaic"] | ["help"] | ["mosaic", "help"] => true,
        ["mosaic", "help", ..] => true,
        ["mosaic", root, rest @ ..] => command_root_invocation_ready(root, rest),
        [root, rest @ ..] => command_root_invocation_ready(root, rest),
        _ => false,
    }
}

fn command_root_invocation_ready(root: &str, args: &[&str]) -> bool {
    match root {
        "gateway" | "adapter" => matches!(args, ["status"]),
        "node" => {
            matches!(args, ["list"]) || matches!(args, ["show", rest @ ..] if !rest.is_empty())
        }
        "session" => {
            matches!(args, ["list"] | ["show"])
                || matches!(args, ["switch", rest @ ..] if !rest.is_empty())
                || matches!(args, ["new", rest @ ..] if !rest.is_empty())
        }
        "model" => {
            matches!(args, ["list"] | ["show"])
                || matches!(args, ["use", rest @ ..] if !rest.is_empty())
        }
        "profile" => {
            matches!(args, ["list"] | ["show"])
                || matches!(args, [profile] if !profile.is_empty())
                || matches!(args, ["use", rest @ ..] if !rest.is_empty())
        }
        "run" => matches!(args, ["stop"] | ["retry"]),
        "sandbox" => {
            matches!(args, ["status"] | ["clean"])
                || matches!(args, ["inspect", rest @ ..] if !rest.is_empty())
                || matches!(args, ["rebuild", rest @ ..] if !rest.is_empty())
        }
        "inspect" => matches!(args, ["last"]),
        "tool" | "skill" | "workflow" => command_capability_invocation_ready(args),
        _ => false,
    }
}

fn command_capability_invocation_ready(args: &[&str]) -> bool {
    if args.len() < 2 {
        return false;
    }

    let input = args[1..].join(" ");
    !args[0].trim().is_empty() && !input.trim().is_empty()
}

fn unknown_command_message(command: &str) -> String {
    let suggestions = suggest_commands(command);
    if suggestions.is_empty() {
        return format!(
            "Unknown command: /{}. Use /mosaic help to browse the command catalog.",
            command.trim()
        );
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
            let alias_searchable = command_alias_search_text(spec.command);
            let score = if searchable.starts_with(&normalized)
                || alias_searchable.starts_with(&normalized)
            {
                0usize
            } else {
                edit_distance(&normalized, &searchable)
                    .min(edit_distance(&normalized, &alias_searchable))
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
    let command_tokens = command_search_tokens(spec.command);

    if query_tokens.len() == 1 {
        return command_tokens
            .iter()
            .any(|token| token.starts_with(query_tokens[0]));
    }

    command_tokens_match_query(&command_tokens, &query_tokens)
        || if command_tokens
            .first()
            .is_some_and(|token| token == "mosaic")
        {
            command_tokens_match_query(&command_tokens[1..], &query_tokens)
        } else {
            false
        }
}

fn command_search_text(command: &str) -> String {
    normalize_command_text(command)
}

fn command_alias_search_text(command: &str) -> String {
    let tokens = command_search_tokens(command);
    if tokens.first().is_some_and(|token| token == "mosaic") && tokens.len() > 1 {
        tokens[1..].join(" ")
    } else {
        tokens.join(" ")
    }
}

fn command_search_tokens(command: &str) -> Vec<String> {
    command_search_text(command)
        .split_whitespace()
        .map(str::to_owned)
        .collect::<Vec<_>>()
}

fn command_tokens_match_query(command_tokens: &[String], query_tokens: &[&str]) -> bool {
    query_tokens.len() <= command_tokens.len()
        && query_tokens
            .iter()
            .zip(command_tokens.iter())
            .all(|(query, command)| command.starts_with(query))
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
    use mosaic_control_protocol::{RunDetailDto, RunSubmission, RunSummaryDto};
    use mosaic_inspect::{CapabilityExplanationTrace, RunLifecycleStatus};
    use mosaic_runtime::events::RunEvent;

    use super::{App, AppAction, ComposerRunRequest, InputMode, SkillOption, TimelineKind};

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
    fn interactive_submit_returns_real_run_request() {
        let mut app = App::new_interactive(
            "/tmp/mosaic".into(),
            "demo".to_owned(),
            "openai".to_owned(),
            "gpt-5.4-mini".to_owned(),
            Vec::new(),
            Vec::new(),
            false,
        );
        app.active_session_mut().draft = "hello runtime".to_owned();

        let action = app.submit_composer();

        assert_eq!(
            action,
            AppAction::SubmitRun(ComposerRunRequest {
                input: "hello runtime".to_owned(),
                tool: None,
                skill: None,
                workflow: None,
            })
        );
        assert_eq!(app.runtime_status, "running");
        assert_eq!(app.active_draft(), "");
    }

    #[test]
    fn explicit_tool_command_returns_targeted_run_request() {
        let mut app = App::new_interactive(
            "/tmp/mosaic".into(),
            "demo".to_owned(),
            "openai".to_owned(),
            "gpt-5.4-mini".to_owned(),
            Vec::new(),
            Vec::new(),
            false,
        );
        app.active_session_mut().draft = "/tool read_file README.md".to_owned();

        let action = app.submit_composer();

        assert_eq!(
            action,
            AppAction::SubmitRun(ComposerRunRequest {
                input: "README.md".to_owned(),
                tool: Some("read_file".to_owned()),
                skill: None,
                workflow: None,
            })
        );
    }

    #[test]
    fn interactive_session_new_command_returns_switch_action() {
        let mut app = App::new_interactive(
            "/tmp/mosaic".into(),
            "demo".to_owned(),
            "openai".to_owned(),
            "gpt-5.4-mini".to_owned(),
            Vec::new(),
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
        assert_eq!(app.escape_hint(), "clear draft or close the command popup");
    }

    #[test]
    fn question_mark_opens_help_when_composer_is_empty() {
        let mut app = App::new("/tmp/mosaic".into());

        app.handle_key(KeyEvent::new(KeyCode::Char('?'), KeyModifiers::NONE));

        assert_eq!(app.active_draft(), "");
        assert_eq!(
            app.active_session()
                .timeline
                .last()
                .map(|entry| entry.title.as_str()),
            Some("Mosaic command reference")
        );
    }

    #[test]
    fn command_menu_selection_completes_draft_without_submitting() {
        let mut app = App::new("/tmp/mosaic".into());
        app.active_session_mut().draft = "/g".to_owned();

        app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

        assert_eq!(app.active_draft(), "/mosaic gateway status ");
    }

    #[test]
    fn tab_accepts_current_command_completion() {
        let mut app = App::new("/tmp/mosaic".into());
        app.active_session_mut().draft = "/m".to_owned();

        app.handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));

        assert_eq!(app.active_draft(), "/mosaic ");
    }

    #[test]
    fn gateway_status_command_renders_inline_card() {
        let mut app = App::new("/tmp/mosaic".into());
        app.gateway_summary = Some("Gateway ready".to_owned());
        app.gateway_detail = Some("transport=http+sse".to_owned());
        app.node_summary = Some("nodes=1".to_owned());
        app.node_detail = Some("healthy".to_owned());
        app.active_session_mut().draft = "/mosaic gateway status".to_owned();

        app.submit_composer();

        assert_eq!(
            app.active_session()
                .timeline
                .last()
                .map(|entry| entry.title.as_str()),
            Some("Gateway status")
        );
    }

    #[test]
    fn sandbox_status_command_returns_inline_action() {
        let mut app = App::new("/tmp/mosaic".into());
        app.active_session_mut().draft = "/mosaic sandbox status".to_owned();

        let action = app.submit_composer();

        assert_eq!(action, AppAction::SandboxStatus);
        assert_eq!(
            app.active_session()
                .timeline
                .last()
                .map(|entry| entry.title.as_str()),
            Some("Sandbox status requested")
        );
    }

    #[test]
    fn sandbox_rebuild_command_returns_targeted_action() {
        let mut app = App::new("/tmp/mosaic".into());
        app.active_session_mut().draft =
            "/mosaic sandbox rebuild python-capability-demo".to_owned();

        let action = app.submit_composer();

        assert_eq!(
            action,
            AppAction::SandboxRebuild("python-capability-demo".to_owned())
        );
    }

    #[test]
    fn slash_tab_can_complete_sandbox_command() {
        let mut app = App::new("/tmp/mosaic".into());
        app.active_session_mut().draft = "/sand".to_owned();

        app.handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));

        assert_eq!(app.active_draft(), "/mosaic sandbox status ");
    }

    #[test]
    fn slash_tab_can_complete_registered_skill_name() {
        let mut app = App::new_interactive(
            "/tmp/mosaic".into(),
            "demo".to_owned(),
            "gpt-5.4-mini".to_owned(),
            "gpt-5.4-mini".to_owned(),
            Vec::new(),
            vec![SkillOption {
                name: "operator_note".to_owned(),
            }],
            false,
        );
        app.active_session_mut().draft = "/skill op".to_owned();

        app.handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));

        assert_eq!(app.active_draft(), "/skill operator_note ");
    }

    #[test]
    fn slash_tab_can_complete_registered_skill_name_for_mosaic_command() {
        let mut app = App::new_interactive(
            "/tmp/mosaic".into(),
            "demo".to_owned(),
            "gpt-5.4-mini".to_owned(),
            "gpt-5.4-mini".to_owned(),
            Vec::new(),
            vec![SkillOption {
                name: "operator_note".to_owned(),
            }],
            false,
        );
        app.active_session_mut().draft = "/mosaic skill op".to_owned();

        app.handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));

        assert_eq!(app.active_draft(), "/mosaic skill operator_note ");
    }

    #[test]
    fn adapter_status_command_returns_inline_action() {
        let mut app = App::new("/tmp/mosaic".into());
        app.active_session_mut().draft = "/mosaic adapter status".to_owned();

        let action = app.submit_composer();

        assert_eq!(action, AppAction::AdapterStatus);
        assert_eq!(
            app.active_session()
                .timeline
                .last()
                .map(|entry| entry.title.as_str()),
            Some("Adapter status requested")
        );
    }

    #[test]
    fn node_commands_return_expected_actions() {
        let mut list_app = App::new("/tmp/mosaic".into());
        list_app.active_session_mut().draft = "/mosaic node list".to_owned();
        assert_eq!(list_app.submit_composer(), AppAction::NodeList);

        let mut show_app = App::new("/tmp/mosaic".into());
        show_app.active_session_mut().draft = "/mosaic node show headless-1".to_owned();
        assert_eq!(
            show_app.submit_composer(),
            AppAction::NodeShow("headless-1".to_owned())
        );
    }

    #[test]
    fn typing_into_the_composer_updates_the_draft_immediately() {
        let mut app = App::new_interactive(
            "/tmp/mosaic".into(),
            "demo".to_owned(),
            "openai".to_owned(),
            "gpt-5.4-mini".to_owned(),
            Vec::new(),
            Vec::new(),
            false,
        );

        assert_eq!(app.active_draft(), "");
        app.handle_key(KeyEvent::new(KeyCode::Char('h'), KeyModifiers::NONE));
        app.handle_key(KeyEvent::new(KeyCode::Char('i'), KeyModifiers::NONE));

        assert_eq!(app.active_draft(), "hi");
        assert_eq!(app.input_mode(), InputMode::Chat);
    }

    #[test]
    fn sync_session_catalog_preserves_draft_for_missing_active_session_placeholder() {
        let mut app = App::new_interactive(
            "/tmp/mosaic".into(),
            "demo".to_owned(),
            "openai".to_owned(),
            "gpt-5.4-mini".to_owned(),
            Vec::new(),
            Vec::new(),
            false,
        );

        app.handle_key(KeyEvent::new(KeyCode::Char('h'), KeyModifiers::NONE));
        app.handle_key(KeyEvent::new(KeyCode::Char('i'), KeyModifiers::NONE));

        app.sync_session_catalog(Vec::new(), "demo");

        assert_eq!(app.session_label(), "demo");
        assert_eq!(app.active_draft(), "hi");
    }

    #[test]
    fn enter_submits_a_typed_chat_message_from_the_composer() {
        let mut app = App::new_interactive(
            "/tmp/mosaic".into(),
            "demo".to_owned(),
            "openai".to_owned(),
            "gpt-5.4-mini".to_owned(),
            Vec::new(),
            Vec::new(),
            false,
        );

        app.handle_key(KeyEvent::new(KeyCode::Char('o'), KeyModifiers::NONE));
        app.handle_key(KeyEvent::new(KeyCode::Char('k'), KeyModifiers::NONE));
        let action = app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

        assert_eq!(
            action,
            AppAction::SubmitRun(ComposerRunRequest {
                input: "ok".to_owned(),
                tool: None,
                skill: None,
                workflow: None,
            })
        );
        assert_eq!(app.active_draft(), "");
    }

    #[test]
    fn entering_mosaic_runs_inline_help_instead_of_stalling_on_completion() {
        let mut app = App::new("/tmp/mosaic".into());
        app.active_session_mut().draft = "/mosaic".to_owned();

        let action = app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

        assert_eq!(action, AppAction::Continue);
        assert_eq!(app.active_draft(), "");
        assert_eq!(
            app.active_session()
                .timeline
                .last()
                .map(|entry| entry.title.as_str()),
            Some("Mosaic command reference")
        );
    }

    #[test]
    fn short_alias_commands_execute_without_forcing_canonical_completion() {
        let mut app = App::new("/tmp/mosaic".into());
        app.active_session_mut().draft = "/session list".to_owned();

        let action = app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

        assert_eq!(action, AppAction::Continue);
        assert_eq!(app.active_draft(), "");
        assert_eq!(
            app.active_session()
                .timeline
                .last()
                .map(|entry| entry.title.as_str()),
            Some("Sessions")
        );
    }

    #[test]
    fn mosaic_and_short_session_commands_route_to_the_same_inline_result() {
        let mut mosaic = App::new("/tmp/mosaic".into());
        mosaic.active_session_mut().draft = "/mosaic session show".to_owned();
        mosaic.submit_composer();

        let mut alias = App::new("/tmp/mosaic".into());
        alias.active_session_mut().draft = "/session show".to_owned();
        alias.submit_composer();

        assert_eq!(
            mosaic
                .active_session()
                .timeline
                .last()
                .map(|entry| entry.title.as_str()),
            Some("Session details")
        );
        assert_eq!(
            alias
                .active_session()
                .timeline
                .last()
                .map(|entry| entry.title.as_str()),
            Some("Session details")
        );
    }

    #[test]
    fn app_starts_in_chat_surface_even_when_resume_flag_is_set() {
        let app = App::new_with_resume("/tmp/mosaic".into(), true);

        assert_eq!(app.surface, super::Surface::Console);
        assert_eq!(app.focus, super::Focus::Composer);
        assert_eq!(
            app.active_session()
                .timeline
                .last()
                .map(|entry| entry.title.as_str()),
            Some("Chat-first TUI")
        );
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
            app.active_session()
                .timeline
                .last()
                .map(|entry| entry.title.as_str()),
            Some("Run started")
        );
    }

    #[test]
    fn skill_events_render_markdown_pack_summary_into_timeline() {
        let mut app = App::new("/tmp/mosaic".into());

        app.apply_run_event(RunEvent::SkillStarted {
            name: "operator_note".to_owned(),
            summary: Some(
                "source=markdown_pack | template=note.md | references=escalation.md | script=annotate.py"
                    .to_owned(),
            ),
        });

        let body = app
            .active_session()
            .timeline
            .last()
            .map(|entry| entry.body.clone())
            .unwrap_or_default();
        assert!(body.contains("template=note.md"));
        assert!(body.contains("script=annotate.py"));
    }

    #[test]
    fn interactive_output_delta_updates_streaming_preview() {
        let mut app = App::new_interactive(
            "/tmp/mosaic".into(),
            "demo".to_owned(),
            "openai".to_owned(),
            "gpt-5.4-mini".to_owned(),
            Vec::new(),
            Vec::new(),
            false,
        );

        app.apply_run_event(RunEvent::OutputDelta {
            run_id: "run-1".to_owned(),
            chunk: "partial".to_owned(),
            accumulated_chars: 7,
        });

        assert_eq!(app.runtime_status, "streaming");
        assert_eq!(app.active_streaming_preview(), Some("partial"));
    }

    #[test]
    fn apply_run_event_marks_failures_in_activity_and_timeline() {
        let mut app = App::new("/tmp/mosaic".into());

        app.apply_run_event(RunEvent::ToolFailed {
            name: "read_file".to_owned(),
            call_id: "call-123".to_owned(),
            error: "permission denied".to_owned(),
            summary: Some("source=builtin | exec_target=local".to_owned()),
        });

        assert_eq!(app.runtime_status, "error");
        let last = app
            .active_session()
            .timeline
            .last()
            .expect("timeline entry should exist");
        assert_eq!(last.kind, TimelineKind::System);
        assert_eq!(last.title, "Tool failed: read_file");
        assert!(last.body.contains("permission denied"));
    }

    #[test]
    fn apply_run_event_renders_tool_and_workflow_summaries() {
        let mut app = App::new("/tmp/mosaic".into());

        app.apply_run_event(RunEvent::ToolCalling {
            name: "read_file".to_owned(),
            call_id: "call-1".to_owned(),
            summary: Some("source=builtin | exec_target=local".to_owned()),
        });
        app.apply_run_event(RunEvent::WorkflowStepStarted {
            workflow: "ops_review".to_owned(),
            step: "fanout".to_owned(),
            kind: "tool".to_owned(),
            summary: Some("target=read_file | orchestration_owner=workflow_engine".to_owned()),
        });

        let timeline = &app.active_session().timeline;
        let tool_body = timeline
            .iter()
            .find(|entry| entry.title == "Tool call: read_file")
            .map(|entry| entry.body.clone())
            .unwrap_or_default();
        let workflow_body = timeline
            .iter()
            .find(|entry| entry.title == "Workflow step: fanout")
            .map(|entry| entry.body.clone())
            .unwrap_or_default();
        assert!(tool_body.contains("exec_target=local"));
        assert!(workflow_body.contains("orchestration_owner=workflow_engine"));
    }

    #[test]
    fn show_run_detail_renders_capability_explanations() {
        let mut app = App::new("/tmp/mosaic".into());
        let detail = RunDetailDto {
            summary: RunSummaryDto {
                gateway_run_id: "gw-1".to_owned(),
                correlation_id: "corr-1".to_owned(),
                run_id: "run-1".to_owned(),
                session_id: Some("sess-1".to_owned()),
                session_route: "local/sess-1".to_owned(),
                status: RunLifecycleStatus::Success,
                requested_profile: Some("openai".to_owned()),
                effective_profile: Some("openai".to_owned()),
                effective_provider_type: Some("openai".to_owned()),
                effective_model: Some("gpt-5.4-mini".to_owned()),
                tool: Some("read_file".to_owned()),
                skill: None,
                workflow: None,
                retry_of: None,
                created_at: chrono::Utc::now(),
                updated_at: chrono::Utc::now(),
                finished_at: Some(chrono::Utc::now()),
                input_preview: "read README".to_owned(),
                output_preview: Some("done".to_owned()),
                error: None,
                failure_kind: None,
                failure_origin: None,
                trace_path: Some("/tmp/trace.json".to_owned()),
            },
            ingress: None,
            outbound_deliveries: Vec::new(),
            capability_explanations: vec![CapabilityExplanationTrace {
                scope: "tool".to_owned(),
                name: "read_file".to_owned(),
                route_kind: Some("tool".to_owned()),
                capability_source_kind: Some("builtin_tool".to_owned()),
                execution_target: Some("local".to_owned()),
                orchestration_owner: Some("tool_loop".to_owned()),
                status: "success".to_owned(),
                summary: "read README.md".to_owned(),
                decision_basis: Some("policy=workspace".to_owned()),
                failure_origin: None,
            }],
            submission: RunSubmission {
                system: None,
                input: "read README".to_owned(),
                tool: Some("read_file".to_owned()),
                skill: None,
                workflow: None,
                session_id: Some("sess-1".to_owned()),
                profile: Some("openai".to_owned()),
                ingress: None,
            },
        };

        app.show_run_detail(&detail);

        let body = app
            .active_session()
            .timeline
            .last()
            .map(|entry| entry.body.clone())
            .unwrap_or_default();
        assert!(body.contains("capability_proof="));
        assert!(body.contains("tool:read_file"));
        assert!(body.contains("decision=policy=workspace"));
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
        let last_timeline = app
            .active_session()
            .timeline
            .last()
            .expect("timeline entry should exist");
        assert_eq!(last_timeline.title, "Run failed");
        assert!(last_timeline.body.contains("boom"));
    }

    #[test]
    fn resume_scope_filtering_still_works_for_session_list_logic() {
        let mut app = App::new_with_resume("/tmp/mosaic".into(), true);
        app.resume_scope = super::ResumeScope::Remote;
        app.resume_query = "ios".to_owned();
        app.ensure_selected_session_visible();

        let visible = app.visible_session_indices();
        assert_eq!(visible.len(), 1);
        assert_eq!(app.sessions[visible[0]].id, "sess-node-007");
    }
}
