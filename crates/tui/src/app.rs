use std::{cell::RefCell, collections::BTreeMap, path::PathBuf};

use crossterm::event::{KeyCode, KeyEvent};
use mosaic_control_protocol::RunDetailDto;
use mosaic_runtime::events::RunEvent;
use mosaic_session_core::{
    SessionRecord as StoredSessionRecord, TranscriptMessage, TranscriptRole, session_route_for_id,
};
use ratatui::text::Line;

use crate::app_event::{AppEvent, interpret_key_event};
pub use crate::bottom_pane::{ApprovalRequest, BottomPaneState, InputMode, RiskLevel};
use crate::chat_widget::{ChatView, TranscriptSurfaceView};
use crate::command_popup::CommandPopupView;
use crate::composer::ComposerView;
use crate::history_cell::{HistoryCellKey, HistoryCells};
use crate::mock;
use crate::overlays::{
    OverlayStackView, OverlayState, TranscriptOverlayView, TurnDetailOverlayView,
};
use crate::shell_view::{ShellChromeView, ShellSnapshot};
use crate::status_bar::{StatusBarView, display_workspace_path};
pub use crate::transcript::{
    ActiveTurn, EXEC_MAX_OUTPUT_LINES, ExecCallState, TimelineEntry, TimelineKind, TranscriptBlock,
    TranscriptDetail, TranscriptDetailKind, TranscriptState, TranscriptView, TurnPhase,
};

#[derive(Debug, Clone)]
struct TranscriptOverlayCache {
    key: (usize, Option<usize>, bool),
    lines: Vec<Line<'static>>,
}

#[derive(Debug, Clone)]
struct DetailOverlayCache {
    key: (usize, Option<usize>, bool, Option<HistoryCellKey>),
    title: String,
    lines: Vec<Line<'static>>,
}

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
    pub arg_hint: &'static str,
    pub category: CommandCategory,
    pub summary: &'static str,
    pub detail: &'static str,
    pub aliases: &'static [&'static str],
}

impl CommandSpec {
    pub fn usage(self) -> String {
        if self.arg_hint.is_empty() {
            self.command.to_owned()
        } else {
            format!("{} {}", self.command, self.arg_hint)
        }
    }
}

pub const LOCAL_COMMANDS: [CommandSpec; 22] = [
    CommandSpec {
        command: "/help",
        arg_hint: "[category]",
        category: CommandCategory::Ui,
        summary: "Show grouped help or one command category",
        detail: "Renders the grouped TUI command catalog inline or narrows it to one category such as session, model, sandbox, tool, skill, workflow, gateway, node, adapter, or inspect.",
        aliases: &["/mosaic", "/mosaic help"],
    },
    CommandSpec {
        command: "/new",
        arg_hint: "[id]",
        category: CommandCategory::Session,
        summary: "Create a fresh active session",
        detail: "Creates or stages a fresh active session. If no id is supplied, the TUI generates one and keeps the operator inside the same transcript shell.",
        aliases: &["/session new", "/mosaic session new"],
    },
    CommandSpec {
        command: "/gateway status",
        arg_hint: "",
        category: CommandCategory::Gateway,
        summary: "Show gateway transport and readiness",
        detail: "Prints the current gateway transport, readiness, node summary, and adapter state into the transcript. Short alias: /gateway status.",
        aliases: &["/mosaic gateway status"],
    },
    CommandSpec {
        command: "/adapter status",
        arg_hint: "",
        category: CommandCategory::Adapter,
        summary: "Show adapter readiness and outbound state",
        detail: "Prints registered adapters, bot bindings, outbound readiness, and channel-facing detail inline in the transcript. Short alias: /adapter status.",
        aliases: &["/mosaic adapter status"],
    },
    CommandSpec {
        command: "/node list",
        arg_hint: "",
        category: CommandCategory::Node,
        summary: "List node health and affinity state",
        detail: "Shows registered nodes, health, disconnect state, and whether the current session is pinned to a specific node. Short alias: /node list.",
        aliases: &["/mosaic node list"],
    },
    CommandSpec {
        command: "/node show",
        arg_hint: "<id>",
        category: CommandCategory::Node,
        summary: "Inspect one node and its declared capabilities",
        detail: "Prints one node's transport, platform, health, disconnect reason, and declared capabilities into the transcript. Short alias: /node show <id>.",
        aliases: &["/mosaic node show"],
    },
    CommandSpec {
        command: "/session list",
        arg_hint: "",
        category: CommandCategory::Session,
        summary: "List known sessions",
        detail: "Shows the sessions currently loaded into this TUI context so the operator can switch without leaving chat. Short alias: /session list.",
        aliases: &["/mosaic session list"],
    },
    CommandSpec {
        command: "/session show",
        arg_hint: "",
        category: CommandCategory::Session,
        summary: "Explain the current session binding",
        detail: "Prints the active session route, run ids, channel metadata, memory summary, compressed context, and references. Short alias: /session show.",
        aliases: &["/mosaic session show"],
    },
    CommandSpec {
        command: "/session switch",
        arg_hint: "<id>",
        category: CommandCategory::Session,
        summary: "Switch the active conversation",
        detail: "Moves the composer to another session and refreshes that session from the gateway or local runtime store. Short alias: /session switch <id>.",
        aliases: &["/mosaic session switch"],
    },
    CommandSpec {
        command: "/model list",
        arg_hint: "",
        category: CommandCategory::Model,
        summary: "List available runtime profiles",
        detail: "Prints configured runtime profiles with provider type and model so the next turn can be scheduled intentionally. Short aliases: /model list and /profile list.",
        aliases: &[
            "/mosaic model list",
            "/profile list",
            "/mosaic profile list",
        ],
    },
    CommandSpec {
        command: "/model use",
        arg_hint: "<profile>",
        category: CommandCategory::Model,
        summary: "Switch the profile for future turns",
        detail: "Updates the active profile used by interactive submissions; the next message will use the new profile. Short aliases: /model use <profile> and /profile <name>.",
        aliases: &[
            "/mosaic model use",
            "/profile",
            "/profile use",
            "/mosaic profile",
            "/mosaic profile use",
        ],
    },
    CommandSpec {
        command: "/model show",
        arg_hint: "",
        category: CommandCategory::Model,
        summary: "Show the current profile and model binding",
        detail: "Explains which runtime profile is selected for the next turn and which provider/model the current session last used. Short aliases: /model show and /profile show.",
        aliases: &[
            "/mosaic model show",
            "/profile show",
            "/mosaic profile show",
        ],
    },
    CommandSpec {
        command: "/run stop",
        arg_hint: "",
        category: CommandCategory::Run,
        summary: "Cancel the active run for this session",
        detail: "Sends a real cancel request to the attached gateway for the current run when one is active. Short alias: /run stop.",
        aliases: &["/mosaic run stop"],
    },
    CommandSpec {
        command: "/run retry",
        arg_hint: "",
        category: CommandCategory::Run,
        summary: "Retry the last completed run",
        detail: "Requests a real retry from the attached gateway using the last known gateway run id for this session. Short alias: /run retry.",
        aliases: &["/mosaic run retry"],
    },
    CommandSpec {
        command: "/sandbox status",
        arg_hint: "",
        category: CommandCategory::Sandbox,
        summary: "Show workspace sandbox lifecycle status",
        detail: "Prints Python and Node sandbox strategies, install policies, runtime availability, and current env counts into the transcript. Short alias: /sandbox status.",
        aliases: &["/mosaic sandbox status"],
    },
    CommandSpec {
        command: "/sandbox inspect",
        arg_hint: "<env>",
        category: CommandCategory::Sandbox,
        summary: "Inspect one sandbox env record",
        detail: "Loads one sandbox env record and renders lifecycle state, install policy, dependencies, and failure details inline. Short alias: /sandbox inspect <env>.",
        aliases: &["/mosaic sandbox inspect"],
    },
    CommandSpec {
        command: "/sandbox rebuild",
        arg_hint: "<env>",
        category: CommandCategory::Sandbox,
        summary: "Rebuild a sandbox env",
        detail: "Deletes and recreates one sandbox env so the next capability run can reuse a fresh local execution environment. Short alias: /sandbox rebuild <env>.",
        aliases: &["/mosaic sandbox rebuild"],
    },
    CommandSpec {
        command: "/sandbox clean",
        arg_hint: "",
        category: CommandCategory::Sandbox,
        summary: "Clean sandbox run and attachment workdirs",
        detail: "Removes sandbox run workdirs and attachment workdirs without leaving the chat transcript. Short alias: /sandbox clean.",
        aliases: &["/mosaic sandbox clean"],
    },
    CommandSpec {
        command: "/inspect last",
        arg_hint: "",
        category: CommandCategory::Inspect,
        summary: "Inspect the most recent run inline",
        detail: "Fetches the most recent run detail for the active session and renders the summary inline in the transcript. Short alias: /inspect last.",
        aliases: &["/mosaic inspect last"],
    },
    CommandSpec {
        command: "/tool",
        arg_hint: "<name> <input>",
        category: CommandCategory::Tool,
        summary: "Invoke a tool explicitly",
        detail: "Submits a real run that explicitly targets one tool and shows the capability events inline in the transcript. Short alias: /tool <name> <input>.",
        aliases: &["/mosaic tool"],
    },
    CommandSpec {
        command: "/skill",
        arg_hint: "<name> <input>",
        category: CommandCategory::Skill,
        summary: "Invoke a skill explicitly",
        detail: "Submits a real run that explicitly targets one skill and streams the result back into the active transcript. Short alias: /skill <name> <input>.",
        aliases: &["/mosaic skill"],
    },
    CommandSpec {
        command: "/workflow",
        arg_hint: "<name> <input>",
        category: CommandCategory::Workflow,
        summary: "Invoke a workflow explicitly",
        detail: "Submits a real run that explicitly targets one workflow and renders step activity inline in the transcript. Short alias: /workflow <name> <input>.",
        aliases: &["/mosaic workflow"],
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
    /// Operator approved the pending capability call with the given call_id.
    ApproveCapability(String),
    /// Operator denied the pending capability call with the given call_id.
    DenyCapability(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppMode {
    Mock,
    Interactive,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShellState {
    Idle,
    Composing,
    Commanding,
    Running,
    TranscriptOverlay,
    TurnDetailOverlay,
}

impl ShellState {
    pub fn label(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Composing => "composing",
            Self::Commanding => "command",
            Self::Running => "running",
            Self::TranscriptOverlay => "transcript",
            Self::TurnDetailOverlay => "detail",
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandSurfaceView {
    pub query: Option<String>,
    pub matches: Vec<CommandSpec>,
    pub selected: Option<CommandSpec>,
    pub selected_index: usize,
    pub skill_completion: Option<String>,
    pub completion_suffix: Option<String>,
    pub popup: CommandPopupView,
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

fn active_turn_matches_run(active_turn: &ActiveTurn, run_id: Option<&str>) -> bool {
    match (active_turn.cell.run_id.as_deref(), run_id) {
        (_, None) => true,
        (None, Some(_)) => true,
        (Some(existing), Some(incoming)) => existing == incoming,
    }
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
    pub cursor_pos: usize,
    pub transcript_len: usize,
    pub current_run_id: Option<String>,
    pub current_gateway_run_id: Option<String>,
    pub last_gateway_run_id: Option<String>,
    pub memory_summary: Option<String>,
    pub compressed_context: Option<String>,
    pub references: Vec<String>,
    pub streaming_preview: Option<String>,
    pub streaming_run_id: Option<String>,
    pub active_turn: Option<ActiveTurn>,
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
    pub selected_session: usize,
    pub bottom_pane: BottomPaneState,
    pub overlay: OverlayState,
    pub transcript_overlay_open: bool,
    pub detail_overlay_open: bool,
    pub detail_overlay_target: Option<HistoryCellKey>,
    pub transcript: TranscriptState,
    pub transcript_overlay: TranscriptState,
    pub detail_overlay: TranscriptState,
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
    transcript_overlay_cache: RefCell<Option<TranscriptOverlayCache>>,
    detail_overlay_cache: RefCell<Option<DetailOverlayCache>>,
    pub git_branch: Option<String>,
    pub tokens_in: u64,
    pub tokens_out: u64,
    pub tokens_cached: u64,
    /// Incremented on every tick; drives spinner animation for running tool calls.
    pub spinner_tick: usize,
    /// Pending approval request, if any. When set, the approval overlay is shown.
    pub pending_approval: Option<ApprovalRequest>,
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
            selected_session: 0,
            bottom_pane: BottomPaneState::default(),
            overlay: OverlayState::None,
            transcript_overlay_open: false,
            detail_overlay_open: false,
            detail_overlay_target: None,
            transcript: TranscriptState::new(),
            transcript_overlay: TranscriptState::new(),
            detail_overlay: TranscriptState::new(),
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
            transcript_overlay_cache: RefCell::new(None),
            detail_overlay_cache: RefCell::new(None),
            git_branch: None,
            tokens_in: 0,
            tokens_out: 0,
            tokens_cached: 0,
            spinner_tick: 0,
            pending_approval: None,
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
            selected_session: 0,
            bottom_pane: BottomPaneState::default(),
            overlay: OverlayState::None,
            transcript_overlay_open: false,
            detail_overlay_open: false,
            detail_overlay_target: None,
            transcript: TranscriptState::new(),
            transcript_overlay: TranscriptState::new(),
            detail_overlay: TranscriptState::new(),
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
            transcript_overlay_cache: RefCell::new(None),
            detail_overlay_cache: RefCell::new(None),
            git_branch: None,
            tokens_in: 0,
            tokens_out: 0,
            tokens_cached: 0,
            spinner_tick: 0,
            pending_approval: None,
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

    pub fn control_model(&self) -> &str {
        &self.control_model
    }

    pub fn gateway_connected(&self) -> bool {
        self.gateway_connected
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
        let cursor_pos = self.active_session().cursor_pos;
        let unread = self.active_session().unread;
        let preserved_turn = self
            .sessions
            .get(self.selected_session)
            .and_then(|existing| {
                existing
                    .active_turn
                    .as_ref()
                    .map(|active| active.cell.clone())
            })
            .or_else(|| {
                self.sessions
                    .get(self.selected_session)
                    .and_then(preserved_expandable_turn)
            });
        let run_label = session.run.status.label();
        let state = session_state_from_run_label(run_label);
        self.runtime_status = runtime_status_from_run_label(run_label).to_owned();

        if let Some(view) = self.sessions.get_mut(self.selected_session) {
            let db_entries: Vec<TimelineEntry> = session
                .transcript
                .iter()
                .filter(|m| m.role != TranscriptRole::System)
                .map(transcript_entry)
                .collect();
            let db_count = db_entries.len();

            if view.transcript_len == 0 {
                // First sync (no prior DB baseline): full replace to remove mock/system
                // entries. Re-attach any locally-submitted (Operator) messages that
                // haven't been confirmed by the DB yet so they survive the 200 ms window.
                let locally_submitted: Vec<TimelineEntry> = view
                    .timeline
                    .iter()
                    .skip(view.transcript_len)
                    .filter(|e| e.kind == TimelineKind::Operator)
                    .cloned()
                    .collect();
                view.timeline = db_entries;
                for entry in locally_submitted {
                    let in_db = view
                        .timeline
                        .iter()
                        .any(|e| e.kind == entry.kind && e.body == entry.body);
                    if !in_db {
                        view.timeline.push(entry);
                    }
                }
            } else if db_count > view.transcript_len {
                // DB has new messages: append only entries not already present in the
                // local tail (entries beyond transcript_len added by push_timeline or
                // finalize_active_turn).  Dedup by (kind, body) prevents duplicates when
                // DB confirms a locally-optimistic message.
                for entry in db_entries.iter().skip(view.transcript_len) {
                    let in_local = view
                        .timeline
                        .iter()
                        .skip(view.transcript_len)
                        .any(|e| e.kind == entry.kind && e.body == entry.body);
                    if !in_local {
                        view.timeline.push(entry.clone());
                    }
                }
            }
            // If db_count <= view.transcript_len and transcript_len != 0: no-op
            // (DB hasn't moved forward; local tail is preserved as-is).
            if runtime_status_is_busy(run_label)
                && let Some(turn) = preserved_turn
                && view.active_turn.is_none()
                && !view.timeline.iter().any(|entry| entry.phase.is_some())
            {
                view.active_turn = Some(ActiveTurn {
                    cell: turn,
                    revision: 1,
                });
            }
            view.transcript_len = db_count;
            if !runtime_status_is_busy(run_label) {
                view.streaming_preview = None;
                view.streaming_run_id = None;
                view.active_turn = None;
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
            view.cursor_pos = cursor_pos;
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
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> AppAction {
        // If the approval overlay is active, route keys to it first.
        if let Some(approval) = self.pending_approval.take() {
            match key.code {
                KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => {
                    let call_id = approval.call_id.clone();
                    return AppAction::ApproveCapability(call_id);
                }
                KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                    let call_id = approval.call_id.clone();
                    return AppAction::DenyCapability(call_id);
                }
                _ => {
                    // Any other key: put the approval back and ignore.
                    self.pending_approval = Some(approval);
                    return AppAction::Continue;
                }
            }
        }

        let event = interpret_key_event(
            key,
            self.command_menu_active(),
            self.command_menu_should_complete(),
            self.active_draft().is_empty(),
        );

        match event {
            AppEvent::Quit => AppAction::Quit,
            AppEvent::OpenHelp => {
                self.push_help(None);
                AppAction::Continue
            }
            AppEvent::ToggleTurnDetail => {
                self.toggle_latest_turn_details();
                AppAction::Continue
            }
            AppEvent::ToggleTranscriptOverlay => {
                self.toggle_transcript_overlay();
                AppAction::Continue
            }
            AppEvent::CommandNext => {
                self.select_next_command_match();
                AppAction::Continue
            }
            AppEvent::CommandPrevious => {
                self.select_previous_command_match();
                AppAction::Continue
            }
            AppEvent::CommandComplete => {
                self.complete_selected_command();
                AppAction::Continue
            }
            AppEvent::ClearDraftOrCloseOverlay => {
                if self.transcript_overlay_open {
                    self.transcript_overlay_open = false;
                    self.transcript_overlay.scroll_home();
                    self.refresh_overlay_state();
                } else if self.detail_overlay_open {
                    self.detail_overlay_open = false;
                    self.detail_overlay_target = None;
                    self.detail_overlay.scroll_home();
                    self.sync_detail_overlay_flags();
                    self.refresh_overlay_state();
                } else if self.command_menu_active() || !self.active_draft().is_empty() {
                    self.active_session_mut().draft.clear();
                    self.active_session_mut().cursor_pos = 0;
                    self.bottom_pane.reset_command_selection();
                    self.refresh_overlay_state();
                }
                AppAction::Continue
            }
            AppEvent::ScrollDown => {
                if self.transcript_overlay_open {
                    self.transcript_overlay.scroll_down(5);
                } else if self.detail_overlay_open {
                    self.detail_overlay.scroll_down(5);
                } else {
                    self.transcript.scroll_down(5);
                }
                AppAction::Continue
            }
            AppEvent::ScrollUp => {
                if self.transcript_overlay_open {
                    self.transcript_overlay.scroll_up(5);
                } else if self.detail_overlay_open {
                    self.detail_overlay.scroll_up(5);
                } else {
                    self.transcript.scroll_up(5);
                }
                AppAction::Continue
            }
            AppEvent::ScrollHome => {
                if self.transcript_overlay_open {
                    self.transcript_overlay.scroll_home();
                } else if self.detail_overlay_open {
                    self.detail_overlay.scroll_home();
                } else {
                    self.transcript.scroll_home();
                }
                AppAction::Continue
            }
            AppEvent::ScrollEnd => {
                if self.transcript_overlay_open {
                    self.transcript_overlay.scroll_end();
                } else if self.detail_overlay_open {
                    self.detail_overlay.scroll_end();
                } else {
                    self.transcript.scroll_end();
                }
                AppAction::Continue
            }
            AppEvent::SubmitComposer => self.submit_composer(),
            AppEvent::BackspaceDraft => {
                let session = self.active_session_mut();
                if session.cursor_pos > 0 {
                    let pos = session.cursor_pos - 1;
                    let byte_pos = session.draft.char_indices().nth(pos).map(|(i, _)| i);
                    if let Some(byte_pos) = byte_pos {
                        session.draft.remove(byte_pos);
                        session.cursor_pos -= 1;
                    }
                }
                self.bottom_pane.reset_command_selection();
                self.refresh_overlay_state();
                AppAction::Continue
            }
            AppEvent::InsertChar(character) => {
                let session = self.active_session_mut();
                let pos = session.cursor_pos;
                let byte_pos = session
                    .draft
                    .char_indices()
                    .nth(pos)
                    .map(|(i, _)| i)
                    .unwrap_or(session.draft.len());
                session.draft.insert(byte_pos, character);
                session.cursor_pos += 1;
                self.bottom_pane.reset_command_selection();
                self.refresh_overlay_state();
                AppAction::Continue
            }
            AppEvent::CursorLeft => {
                let session = self.active_session_mut();
                session.cursor_pos = session.cursor_pos.saturating_sub(1);
                AppAction::Continue
            }
            AppEvent::CursorRight => {
                let session = self.active_session_mut();
                let max = session.draft.chars().count();
                session.cursor_pos = (session.cursor_pos + 1).min(max);
                AppAction::Continue
            }
            AppEvent::CursorHome => {
                if self.active_draft().is_empty() {
                    if self.transcript_overlay_open {
                        self.transcript_overlay.scroll_home();
                    } else if self.detail_overlay_open {
                        self.detail_overlay.scroll_home();
                    } else {
                        self.transcript.scroll_home();
                    }
                } else {
                    self.active_session_mut().cursor_pos = 0;
                }
                AppAction::Continue
            }
            AppEvent::CursorEnd => {
                if self.active_draft().is_empty() {
                    if self.transcript_overlay_open {
                        self.transcript_overlay.scroll_end();
                    } else if self.detail_overlay_open {
                        self.detail_overlay.scroll_end();
                    } else {
                        self.transcript.scroll_end();
                    }
                } else {
                    let max = self.active_draft().chars().count();
                    self.active_session_mut().cursor_pos = max;
                }
                AppAction::Continue
            }
            AppEvent::None => AppAction::Continue,
        }
    }

    pub fn tick(&mut self) {
        self.heartbeat = self.heartbeat.wrapping_add(1);
        self.spinner_tick = self.spinner_tick.wrapping_add(1);
    }

    fn start_or_refresh_active_turn(
        &mut self,
        run_id: Option<&str>,
        phase: TurnPhase,
        title: impl Into<String>,
        body: impl Into<String>,
    ) {
        let title = title.into();
        let body = body.into();
        let timestamp = current_hhmm();
        if let Some(session) = self.sessions.get_mut(self.selected_session) {
            let reuse_existing = session
                .active_turn
                .as_ref()
                .is_some_and(|active_turn| active_turn_matches_run(active_turn, run_id));
            if !reuse_existing {
                session.active_turn = Some(ActiveTurn {
                    cell: new_active_turn_entry(&timestamp, run_id, phase, &title, &body),
                    revision: 0,
                });
            }
            let active_turn = session.active_turn.as_mut().expect("active turn to exist");
            let entry = &mut active_turn.cell;
            entry.timestamp = timestamp;
            entry.kind = TimelineKind::Agent;
            entry.block = TranscriptBlock::AssistantMessage;
            entry.actor = "assistant".to_owned();
            entry.title = title;
            if !body.is_empty() || entry.body.trim().is_empty() {
                entry.body = body.clone();
            }
            entry.run_id = run_id
                .map(str::to_owned)
                .or_else(|| entry.run_id.clone())
                .or_else(|| session.streaming_run_id.clone());
            entry.phase = Some(phase);

            if matches!(phase, TurnPhase::Streaming) {
                session.streaming_preview = Some(entry.body.clone());
                session.streaming_run_id = entry.run_id.clone();
            }
            active_turn.revision = active_turn.revision.saturating_add(1);
        }
    }

    fn append_active_turn_output(&mut self, run_id: &str, chunk: &str) {
        let timestamp = current_hhmm();
        if let Some(session) = self.sessions.get_mut(self.selected_session) {
            let reuse_existing = session
                .active_turn
                .as_ref()
                .is_some_and(|active_turn| active_turn_matches_run(active_turn, Some(run_id)));
            if !reuse_existing {
                session.active_turn = Some(ActiveTurn {
                    cell: new_active_turn_entry(
                        &timestamp,
                        Some(run_id),
                        TurnPhase::Streaming,
                        "Assistant response",
                        "",
                    ),
                    revision: 0,
                });
            }
            let active_turn = session.active_turn.as_mut().expect("active turn to exist");
            let entry = &mut active_turn.cell;
            let was_streaming = matches!(entry.phase, Some(TurnPhase::Streaming));
            entry.timestamp = timestamp;
            entry.phase = Some(TurnPhase::Streaming);
            entry.run_id = Some(run_id.to_owned());
            entry.block = TranscriptBlock::AssistantMessage;
            if !was_streaming {
                entry.body.clear();
            }
            entry.body.push_str(chunk);
            session.streaming_preview = Some(entry.body.clone());
            session.streaming_run_id = Some(run_id.to_owned());
            active_turn.revision = active_turn.revision.saturating_add(1);
        }
    }

    fn attach_turn_detail(
        &mut self,
        run_id: Option<&str>,
        phase: TurnPhase,
        kind: TranscriptDetailKind,
        title: impl Into<String>,
        body: impl Into<String>,
    ) {
        let title = title.into();
        let body = body.into();
        let turn_title = match phase {
            TurnPhase::Failed => "Assistant response",
            TurnPhase::Canceled => "Assistant response",
            _ => "Assistant response",
        };
        self.start_or_refresh_active_turn(run_id, phase, turn_title, "");

        if let Some(session) = self.sessions.get_mut(self.selected_session) {
            let Some(active_turn) = session.active_turn.as_mut() else {
                return;
            };
            let entry = &mut active_turn.cell;
            entry.timestamp = current_hhmm();
            entry.phase = Some(phase);
            if kind == TranscriptDetailKind::Failure {
                entry.block = TranscriptBlock::AssistantMessage;
            }
            entry.details.push(TranscriptDetail { kind, title, body });
            active_turn.revision = active_turn.revision.saturating_add(1);
        }
    }

    fn finalize_active_turn(
        &mut self,
        run_id: &str,
        phase: TurnPhase,
        body: impl Into<String>,
        detail: Option<(TranscriptDetailKind, String, String)>,
    ) {
        let body = body.into();
        let mut committed_index = None;
        if let Some(session) = self.sessions.get_mut(self.selected_session) {
            let timestamp = current_hhmm();
            let reuse_existing = session
                .active_turn
                .as_ref()
                .is_some_and(|active_turn| active_turn_matches_run(active_turn, Some(run_id)));
            let mut active_turn = if reuse_existing {
                session.active_turn.take().expect("active turn to exist")
            } else {
                ActiveTurn {
                    cell: new_active_turn_entry(
                        &timestamp,
                        Some(run_id),
                        phase,
                        "Assistant response",
                        &body,
                    ),
                    revision: 0,
                }
            };
            let entry = &mut active_turn.cell;
            entry.timestamp = timestamp;
            entry.phase = Some(phase);
            entry.run_id = Some(run_id.to_owned());
            entry.kind = TimelineKind::Agent;
            entry.block = TranscriptBlock::AssistantMessage;
            entry.actor = "assistant".to_owned();
            entry.title = "Assistant response".to_owned();
            if !body.is_empty() && (matches!(phase, TurnPhase::Completed) || entry.body.is_empty())
            {
                entry.body = body;
            }
            if let Some((kind, title, detail_body)) = detail {
                entry.details.push(TranscriptDetail {
                    kind,
                    title,
                    body: detail_body,
                });
            }
            session.timeline.push(entry.clone());
            trim_timeline(session);
            committed_index = Some(session.timeline.len().saturating_sub(1));
            session.streaming_preview = None;
            session.streaming_run_id = None;
            session.active_turn = None;
            // Advance transcript_len to the current timeline length so the next
            // sync_runtime_session_with_origin treats the finalized entry as
            // already-seen and won't append it again from DB (no duplicate).
            session.transcript_len = session.timeline.len();
        }
        // A new committed turn means we should scroll to show it.
        self.transcript.follow = true;
        if self.detail_overlay_open
            && matches!(self.detail_overlay_target, Some(HistoryCellKey::Active))
        {
            if let Some(index) = committed_index {
                self.detail_overlay_target = Some(HistoryCellKey::Committed(index));
            }
        }
        self.sync_detail_overlay_flags();
    }

    fn toggle_latest_turn_details(&mut self) {
        let cells = self.history_cells();
        let latest_turn_title = cells.latest_expandable_title().map(str::to_owned);
        let activity = if let Some(title) = latest_turn_title {
            self.transcript_overlay_open = false;
            self.transcript_overlay.scroll_home();
            self.detail_overlay_open = !self.detail_overlay_open;
            if self.detail_overlay_open {
                self.detail_overlay_target =
                    cells.resolve_detail_target(self.detail_overlay_target);
                self.detail_overlay.scroll_home();
            } else {
                self.detail_overlay_target = None;
            }
            self.sync_detail_overlay_flags();
            self.refresh_overlay_state();
            Some(format!(
                "{} detail overlay for {}",
                if self.detail_overlay_open {
                    "Opened"
                } else {
                    "Closed"
                },
                title
            ))
        } else {
            None
        };
        if let Some(activity) = activity {
            self.push_activity("tui", activity);
        }
    }

    fn toggle_transcript_overlay(&mut self) {
        self.detail_overlay_open = false;
        self.detail_overlay_target = None;
        self.detail_overlay.scroll_home();
        self.sync_detail_overlay_flags();
        self.transcript_overlay_open = !self.transcript_overlay_open;
        if self.transcript_overlay_open {
            self.transcript_overlay.scroll_home();
        }
        self.refresh_overlay_state();
        self.push_activity(
            "tui",
            if self.transcript_overlay_open {
                "Opened transcript overlay".to_owned()
            } else {
                "Closed transcript overlay".to_owned()
            },
        );
    }

    pub fn apply_run_event(&mut self, event: RunEvent) {
        match event {
            RunEvent::RunStarted { run_id, input } => {
                self.runtime_status = "running".to_owned();
                self.active_session_mut().state = SessionState::Active;
                self.push_activity("runtime", "Run started");
                self.start_or_refresh_active_turn(
                    Some(&run_id),
                    TurnPhase::Queued,
                    "Assistant response",
                    "Waiting for assistant output.",
                );
                self.attach_turn_detail(
                    Some(&run_id),
                    TurnPhase::Queued,
                    TranscriptDetailKind::Notice,
                    "Run started",
                    format!(
                        "run_id={}\ninput={}",
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
                self.attach_turn_detail(
                    None,
                    TurnPhase::CapabilityActive,
                    TranscriptDetailKind::Workflow,
                    format!("Workflow started: {}", name),
                    format!("steps={step_count}\nroute=workflow"),
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
                self.attach_turn_detail(
                    None,
                    TurnPhase::CapabilityActive,
                    TranscriptDetailKind::Workflow,
                    format!("Workflow step: {}", step),
                    match summary {
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
                self.attach_turn_detail(
                    None,
                    TurnPhase::WaitingOnCapability,
                    TranscriptDetailKind::Workflow,
                    format!("Workflow step finished: {}", step),
                    match summary {
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
                self.attach_turn_detail(
                    None,
                    TurnPhase::Failed,
                    TranscriptDetailKind::Failure,
                    format!("Workflow step failed: {}", step),
                    match summary {
                        Some(summary) => format!(
                            "failure={}\nworkflow={}\n{}\nerror={}\nnext={}",
                            classify_failure(Some("workflow"), None),
                            workflow,
                            summary,
                            truncate_for_timeline(&error, 180),
                            next_action_for_failure(Some("workflow"))
                        ),
                        None => format!(
                            "failure={}\nworkflow={}\nerror={}\nnext={}",
                            classify_failure(Some("workflow"), None),
                            workflow,
                            truncate_for_timeline(&error, 180),
                            next_action_for_failure(Some("workflow"))
                        ),
                    },
                );
            }
            RunEvent::WorkflowFinished { name } => {
                self.push_activity("workflow", format!("Workflow finished: {}", name));
                self.attach_turn_detail(
                    None,
                    TurnPhase::WaitingOnCapability,
                    TranscriptDetailKind::Workflow,
                    format!("Workflow finished: {}", name),
                    "route=workflow".to_owned(),
                );
            }
            RunEvent::SkillStarted { name, summary } => {
                self.push_activity("skill", &format!("Executing skill: {}", name));
                self.attach_turn_detail(
                    None,
                    TurnPhase::CapabilityActive,
                    detail_kind_from_text("skill", summary.as_deref().unwrap_or("")),
                    format!("Skill started: {}", name),
                    match summary {
                        Some(summary) => format!("Skill: {}\n{}", name, summary),
                        None => format!("Skill: {}", name),
                    },
                );
            }
            RunEvent::SkillFinished { name, summary } => {
                self.push_activity("skill", &format!("Skill finished: {}", name));
                self.attach_turn_detail(
                    None,
                    TurnPhase::WaitingOnCapability,
                    detail_kind_from_text("skill", summary.as_deref().unwrap_or("")),
                    format!("Skill finished: {}", name),
                    match summary {
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
                self.attach_turn_detail(
                    None,
                    TurnPhase::Failed,
                    TranscriptDetailKind::Failure,
                    format!("Skill failed: {}", name),
                    match summary {
                        Some(summary) => format!(
                            "failure={}\nSkill: {}\n{}\nError: {}\nnext={}",
                            classify_failure(Some("skill"), None),
                            name,
                            summary,
                            truncate_for_timeline(&error, 180),
                            next_action_for_failure(Some("skill"))
                        ),
                        None => format!(
                            "failure={}\nSkill: {}\nError: {}\nnext={}",
                            classify_failure(Some("skill"), None),
                            name,
                            truncate_for_timeline(&error, 180),
                            next_action_for_failure(Some("skill"))
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
                self.attach_turn_detail(
                    None,
                    TurnPhase::Queued,
                    TranscriptDetailKind::Provider,
                    "Provider request",
                    format!(
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
                self.attach_turn_detail(
                    None,
                    TurnPhase::Queued,
                    TranscriptDetailKind::Provider,
                    "Provider retry",
                    format!(
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
                self.attach_turn_detail(
                    None,
                    TurnPhase::Failed,
                    TranscriptDetailKind::Failure,
                    "Provider failed",
                    format!(
                        "failure={}\nprovider={} profile={} model={} kind={} error={}\nnext={}",
                        classify_failure(Some("provider"), None),
                        provider_type,
                        profile,
                        model,
                        kind,
                        truncate_for_timeline(&error, 180),
                        next_action_for_failure(Some("provider"))
                    ),
                );
            }
            RunEvent::ToolCalling {
                name,
                call_id,
                summary,
            } => {
                self.push_activity("tool", &format!("Calling tool: {}", name));
                // Push a live exec call entry onto the active turn.
                if let Some(session) = self.sessions.get_mut(self.selected_session) {
                    if let Some(active_turn) = session.active_turn.as_mut() {
                        active_turn.cell.exec_calls.push(ExecCallState {
                            call_id: call_id.clone(),
                            tool_name: name.clone(),
                            input_summary: summary
                                .as_deref()
                                .unwrap_or("")
                                .chars()
                                .take(120)
                                .collect(),
                            output_lines: Vec::new(),
                            exit_ok: None,
                            duration_label: None,
                            running: true,
                        });
                        active_turn.revision = active_turn.revision.saturating_add(1);
                    }
                }
                self.attach_turn_detail(
                    None,
                    TurnPhase::CapabilityActive,
                    detail_kind_from_text("tool", summary.as_deref().unwrap_or("")),
                    format!("Tool call: {}", name),
                    match summary {
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
                // Mark the exec call as completed.
                if let Some(session) = self.sessions.get_mut(self.selected_session) {
                    if let Some(active_turn) = session.active_turn.as_mut() {
                        if let Some(exec) = active_turn
                            .cell
                            .exec_calls
                            .iter_mut()
                            .find(|e| e.call_id == call_id)
                        {
                            exec.running = false;
                            exec.exit_ok = Some(true);
                            if let Some(ref s) = summary {
                                for line in s.lines().take(EXEC_MAX_OUTPUT_LINES) {
                                    if exec.output_lines.len() >= EXEC_MAX_OUTPUT_LINES {
                                        exec.output_lines.remove(0);
                                    }
                                    exec.output_lines.push(line.to_owned());
                                }
                            }
                        }
                        active_turn.revision = active_turn.revision.saturating_add(1);
                    }
                }
                self.attach_turn_detail(
                    None,
                    TurnPhase::WaitingOnCapability,
                    detail_kind_from_text("tool", summary.as_deref().unwrap_or("")),
                    format!("Tool finished: {}", name),
                    match summary {
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
                // Mark the exec call as failed.
                if let Some(session) = self.sessions.get_mut(self.selected_session) {
                    if let Some(active_turn) = session.active_turn.as_mut() {
                        if let Some(exec) = active_turn
                            .cell
                            .exec_calls
                            .iter_mut()
                            .find(|e| e.call_id == call_id)
                        {
                            exec.running = false;
                            exec.exit_ok = Some(false);
                            exec.output_lines
                                .push(format!("error: {}", truncate_for_timeline(&error, 120)));
                        }
                        active_turn.revision = active_turn.revision.saturating_add(1);
                    }
                }
                self.attach_turn_detail(
                    None,
                    TurnPhase::Failed,
                    TranscriptDetailKind::Failure,
                    format!("Tool failed: {}", name),
                    match summary {
                        Some(summary) => format!(
                            "failure={}\ncall_id={}\n{}\nerror={}\nnext={}",
                            classify_failure(Some("tool"), None),
                            call_id,
                            summary,
                            truncate_for_timeline(&error, 180),
                            next_action_for_failure(Some("tool"))
                        ),
                        None => format!(
                            "failure={}\ncall_id={}\nerror={}\nnext={}",
                            classify_failure(Some("tool"), None),
                            call_id,
                            truncate_for_timeline(&error, 180),
                            next_action_for_failure(Some("tool"))
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
                self.attach_turn_detail(
                    None,
                    TurnPhase::CapabilityActive,
                    detail_kind_from_text("capability", &format!("{name} {kind}")),
                    format!("Capability queued: {}", name),
                    format!(
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
                self.attach_turn_detail(
                    None,
                    TurnPhase::CapabilityActive,
                    detail_kind_from_text("capability", &name),
                    format!("Capability running: {}", name),
                    format!("job_id={}", job_id),
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
                self.attach_turn_detail(
                    None,
                    TurnPhase::WaitingOnCapability,
                    detail_kind_from_text("capability", &name),
                    format!("Capability retry: {}", name),
                    format!(
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
                self.attach_turn_detail(
                    None,
                    TurnPhase::WaitingOnCapability,
                    detail_kind_from_text("capability", &format!("{name} {summary}")),
                    format!("Capability finished: {}", name),
                    format!(
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
                self.attach_turn_detail(
                    None,
                    TurnPhase::Failed,
                    TranscriptDetailKind::Failure,
                    format!("Capability failed: {}", name),
                    format!(
                        "failure={}\njob_id={}\nerror={}\nnext={}",
                        classify_failure(None, Some("tool")),
                        job_id,
                        truncate_for_timeline(&error, 180),
                        next_action_for_failure(Some("tool"))
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
                self.attach_turn_detail(
                    None,
                    TurnPhase::Failed,
                    TranscriptDetailKind::Failure,
                    format!("Permission check failed: {}", name),
                    format!(
                        "failure={}\ncall_id={}\nreason={}\nnext={}",
                        classify_failure(Some("permission"), None),
                        call_id,
                        truncate_for_timeline(&reason, 180),
                        next_action_for_failure(Some("permission"))
                    ),
                );
            }
            RunEvent::OutputDelta {
                run_id,
                chunk,
                accumulated_chars: _,
            } => {
                self.runtime_status = "streaming".to_owned();
                self.active_session_mut().state = SessionState::Active;
                if self.is_interactive() {
                    self.append_active_turn_output(&run_id, &chunk);
                    return;
                }
                self.append_active_turn_output(&run_id, &chunk);
            }
            RunEvent::FinalAnswerReady { run_id } => {
                self.runtime_status = "streaming".to_owned();
                self.active_session_mut().state = SessionState::Active;
                self.push_activity("runtime", "Final answer ready");
                self.attach_turn_detail(
                    Some(&run_id),
                    TurnPhase::WaitingOnCapability,
                    TranscriptDetailKind::Notice,
                    "Final answer ready",
                    format!("run_id={run_id}\nWaiting for the final transcript to land."),
                );
            }
            RunEvent::RunFinished {
                run_id,
                output_preview,
            } => {
                self.runtime_status = "idle".to_owned();
                self.active_session_mut().state = SessionState::Waiting;
                self.push_activity("runtime", "Run finished");
                self.finalize_active_turn(
                    &run_id,
                    TurnPhase::Completed,
                    if output_preview.is_empty() {
                        String::new()
                    } else {
                        truncate_for_timeline(&output_preview, 600)
                    },
                    Some((
                        TranscriptDetailKind::Notice,
                        "Run finished".to_owned(),
                        format!(
                            "run_id={}\noutput_preview={}",
                            run_id,
                            truncate_for_timeline(&output_preview, 180)
                        ),
                    )),
                );
            }
            RunEvent::RunFailed {
                run_id,
                error,
                failure_kind,
                failure_origin,
            } => {
                self.runtime_status = "error".to_owned();
                self.active_session_mut().state = SessionState::Degraded;
                self.push_activity("runtime", "Run failed");
                self.finalize_active_turn(
                    &run_id,
                    TurnPhase::Failed,
                    "The run failed before the assistant finished responding.".to_owned(),
                    Some((
                        TranscriptDetailKind::Failure,
                        "Run failed".to_owned(),
                        format!(
                            "failure={}\nrun_id={}\nfailure_kind={}\nerror={}\nnext={}",
                            classify_failure(failure_origin.as_deref(), failure_kind.as_deref()),
                            run_id,
                            failure_kind.unwrap_or_else(|| "<none>".to_owned()),
                            truncate_for_timeline(&error, 180),
                            next_action_for_failure(failure_origin.as_deref())
                        ),
                    )),
                );
            }
            RunEvent::RunCanceled { run_id, reason } => {
                self.runtime_status = "canceled".to_owned();
                self.active_session_mut().state = SessionState::Degraded;
                self.push_activity("runtime", "Run canceled");
                self.finalize_active_turn(
                    &run_id,
                    TurnPhase::Canceled,
                    "The run was canceled before the assistant finished responding.".to_owned(),
                    Some((
                        TranscriptDetailKind::Notice,
                        "Run canceled".to_owned(),
                        format!(
                            "run_id={}\nreason={}",
                            run_id,
                            truncate_for_timeline(&reason, 180)
                        ),
                    )),
                );
            }
            RunEvent::TokenUsage {
                input_tokens,
                output_tokens,
                cached_tokens,
            } => {
                self.tokens_in = self.tokens_in.saturating_add(input_tokens);
                self.tokens_out = self.tokens_out.saturating_add(output_tokens);
                self.tokens_cached = self.tokens_cached.saturating_add(cached_tokens);
            }
            RunEvent::CapabilityApprovalRequired {
                call_id,
                tool_name,
                command_preview,
                risk_level,
            } => {
                self.pending_approval = Some(ApprovalRequest {
                    call_id,
                    tool_name,
                    command_preview,
                    risk_level: RiskLevel::from_str(&risk_level),
                });
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
        if let Some(active_turn) = self.active_session().active_turn.as_ref()
            && matches!(active_turn.cell.phase, Some(TurnPhase::Streaming))
        {
            return Some(active_turn.cell.body.as_str());
        }

        self.active_session().streaming_preview.as_deref()
    }

    pub fn transcript_view(&self) -> TranscriptView<'_> {
        transcript_view_for_session(
            self.active_session(),
            self.transcript.scroll,
            self.spinner_tick,
        )
    }

    pub fn history_cells(&self) -> HistoryCells {
        HistoryCells::from(&self.transcript_view())
    }

    pub fn chat_view(&self) -> ChatView {
        ChatView::new(self.history_cells(), self.transcript_scroll())
    }

    fn transcript_overlay_lines_for_cells(&self, cells: &HistoryCells) -> Vec<Line<'static>> {
        let key = cells.transcript_key();
        let mut cache = self.transcript_overlay_cache.borrow_mut();
        let should_refresh = cache.as_ref().is_none_or(|entry| entry.key != key);
        if should_refresh {
            *cache = Some(TranscriptOverlayCache {
                key,
                lines: cells.summary_lines(),
            });
        }
        cache
            .as_ref()
            .map(|entry| entry.lines.clone())
            .unwrap_or_default()
    }

    pub fn transcript_overlay_lines(&self) -> Vec<Line<'static>> {
        self.transcript_overlay_lines_for_cells(&self.history_cells())
    }

    pub fn transcript_overlay_cache_key(&self) -> Option<(usize, Option<usize>, bool)> {
        self.transcript_overlay_cache
            .borrow()
            .as_ref()
            .map(|entry| entry.key)
    }

    pub fn detail_overlay_cache_key(
        &self,
    ) -> Option<(usize, Option<usize>, bool, Option<HistoryCellKey>)> {
        self.detail_overlay_cache
            .borrow()
            .as_ref()
            .map(|entry| entry.key)
    }

    pub fn transcript_scroll(&self) -> u16 {
        self.transcript.scroll
    }

    /// Compute the number of rendered lines in the chat transcript at a given terminal width.
    /// Used by the event loop to call `sync_follow` before each draw.
    pub fn chat_total_lines(&self, width: u16) -> u16 {
        self.chat_view().lines_at_width(Some(width)).len() as u16
    }

    pub fn transcript_overlay_scroll(&self) -> u16 {
        self.transcript_overlay.scroll
    }

    pub fn detail_overlay_scroll(&self) -> u16 {
        self.detail_overlay.scroll
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
        self.bottom_pane.input_mode(self.active_draft())
    }

    pub fn shell_state(&self) -> ShellState {
        if self.overlay_state().is_turn_detail() {
            ShellState::TurnDetailOverlay
        } else if self.overlay_state().is_transcript() {
            ShellState::TranscriptOverlay
        } else if self.command_menu_active() {
            ShellState::Commanding
        } else if self.is_busy() {
            ShellState::Running
        } else if !self.active_draft().trim().is_empty() {
            ShellState::Composing
        } else {
            ShellState::Idle
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
            InputMode::Chat => "Message…",
            InputMode::Command => "Run a slash command. Tab accepts, Enter executes.",
            InputMode::Search => "Type to filter…",
        }
    }

    pub fn gateway_target_label(&self) -> &str {
        &self.gateway_target
    }

    pub fn overlay_state(&self) -> OverlayState {
        OverlayState::from_shell_state(
            self.command_query(),
            self.transcript_overlay_open,
            self.detail_overlay_open,
        )
    }

    pub fn selected_command_index(&self) -> usize {
        self.bottom_pane.command_menu_index
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
        "clear"
    }

    pub fn command_query(&self) -> Option<&str> {
        self.active_draft().trim_start().strip_prefix('/')
    }

    pub fn command_surface_view(&self) -> CommandSurfaceView {
        let query = self.command_query().map(str::to_owned);
        let selected_index = self.selected_command_index();
        let matches = matching_commands(query.as_deref().unwrap_or_default());
        let selected = matches
            .get(selected_index.min(matches.len().saturating_sub(1)))
            .copied();
        let skill_completion = self.selected_skill_completion();
        let completion_suffix = if let Some(completion) = skill_completion.as_ref() {
            completion
                .strip_prefix(self.active_draft())
                .filter(|suffix| !suffix.is_empty())
                .map(str::to_owned)
        } else if let Some(command) = selected {
            let completion = command_completion(command.command);
            completion
                .strip_prefix(self.active_draft())
                .filter(|suffix| !suffix.is_empty())
                .map(str::to_owned)
        } else {
            None
        };
        let popup = CommandPopupView::new(
            query.clone().unwrap_or_default(),
            matches.clone(),
            selected_index,
        );

        CommandSurfaceView {
            query,
            matches,
            selected,
            selected_index,
            skill_completion,
            completion_suffix,
            popup,
        }
    }

    pub fn command_matches(&self) -> Vec<CommandSpec> {
        self.command_surface_view().matches
    }

    pub fn command_popup_view(&self) -> CommandPopupView {
        self.command_surface_view().popup
    }

    pub fn selected_command_match(&self) -> Option<CommandSpec> {
        self.command_surface_view().selected
    }

    pub fn command_suggestions(&self) -> Vec<CommandSpec> {
        suggest_commands(self.command_query().unwrap_or_default())
    }

    pub fn command_completion_suffix(&self) -> Option<String> {
        self.command_surface_view().completion_suffix
    }

    fn composer_view_from_command_surface(
        &self,
        command_surface: &CommandSurfaceView,
    ) -> ComposerView {
        let busy = runtime_status_is_busy(&self.runtime_status);
        let status_label = if busy {
            "busy".to_owned()
        } else {
            "ready".to_owned()
        };
        let status_detail = if busy {
            if let Some(run_id) = self.current_run_identifier() {
                format!("send disabled · /run stop ({run_id})")
            } else {
                "send disabled · /run stop".to_owned()
            }
        } else {
            self.session_label().to_owned()
        };
        ComposerView {
            draft: self.active_draft().to_owned(),
            cursor_pos: self.active_session().cursor_pos,
            mode: self.input_mode(),
            shell_state: self.shell_state(),
            placeholder: self.composer_placeholder().to_owned(),
            completion_suffix: command_surface.completion_suffix.clone(),
            busy,
            status_label,
            status_detail,
            enter_hint: self.enter_hint(),
            escape_hint: self.escape_hint().to_owned(),
            spinner: self.task_spinner(),
        }
    }

    pub fn composer_view(&self) -> ComposerView {
        let command_surface = self.command_surface_view();
        self.composer_view_from_command_surface(&command_surface)
    }

    pub fn status_bar_view(&self) -> StatusBarView {
        let runtime_summary = self
            .active_turn_banner()
            .unwrap_or_else(|| self.operator_status());
        let runtime_label = if self.is_busy() {
            format!("{} {}", self.task_spinner(), self.runtime_status)
        } else {
            self.runtime_status.clone()
        };

        StatusBarView {
            workspace: display_workspace_path(&self.workspace_path),
            session_label: self.session_label().to_owned(),
            active_profile: self.active_profile().to_owned(),
            control_model: self.control_model().to_owned(),
            gateway_live: self.gateway_connected(),
            gateway_target: self.gateway_target_label().to_owned(),
            hide_runtime_summary: self.hide_status_summary_during_streaming(),
            shell_state_label: self.shell_state().label(),
            runtime_label,
            runtime_summary,
            git_branch: self.git_branch.clone(),
            tokens_in: self.tokens_in,
            tokens_out: self.tokens_out,
        }
    }

    /// Returns rendered lines for the approval overlay, or `None` if no approval is pending.
    pub fn approval_overlay_lines(&self) -> Option<Vec<Line<'static>>> {
        use ratatui::style::{Modifier, Style};
        use ratatui::text::{Line, Span};
        let approval = self.pending_approval.as_ref()?;
        let risk_style = Style::default()
            .fg(approval.risk_level.color())
            .add_modifier(Modifier::BOLD);
        let lines = vec![
            Line::from(vec![
                Span::styled("  ⚠ ", Style::default().fg(ratatui::style::Color::Yellow)),
                Span::styled(
                    "Capability Approval Required",
                    Style::default().add_modifier(Modifier::BOLD),
                ),
            ]),
            Line::from(vec![
                Span::raw("  tool: "),
                Span::styled(
                    approval.tool_name.clone(),
                    Style::default()
                        .fg(ratatui::style::Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw("  risk: "),
                Span::styled(approval.risk_level.label(), risk_style),
            ]),
            Line::from(vec![
                Span::raw("  "),
                Span::styled(
                    approval
                        .command_preview
                        .chars()
                        .take(80)
                        .collect::<String>(),
                    Style::default().fg(ratatui::style::Color::Gray),
                ),
            ]),
            Line::from(vec![Span::styled(
                "  [y] approve  [n / Esc] deny",
                Style::default().fg(ratatui::style::Color::DarkGray),
            )]),
        ];
        Some(lines)
    }

    pub fn detail_overlay_lines(&self) -> Option<Vec<Line<'static>>> {
        self.turn_detail_overlay_view_for_cells(&self.history_cells())
            .map(|view| view.lines)
    }

    pub fn transcript_overlay_view_model(&self) -> TranscriptOverlayView {
        TranscriptOverlayView {
            lines: self.transcript_overlay_lines_for_cells(&self.history_cells()),
            scroll: self.transcript_overlay_scroll(),
        }
    }

    pub fn turn_detail_overlay_view_model(&self) -> Option<TurnDetailOverlayView> {
        self.turn_detail_overlay_view_for_cells(&self.history_cells())
    }

    fn turn_detail_overlay_view_for_cells(
        &self,
        cells: &HistoryCells,
    ) -> Option<TurnDetailOverlayView> {
        if !self.detail_overlay_open {
            return None;
        }

        let (count, revision, streaming) = cells.transcript_key();
        let key = (count, revision, streaming, self.detail_overlay_target);
        let mut cache = self.detail_overlay_cache.borrow_mut();
        let should_refresh = cache.as_ref().is_none_or(|entry| entry.key != key);
        if should_refresh {
            *cache =
                cells
                    .detail_view(self.detail_overlay_target)
                    .map(|detail| DetailOverlayCache {
                        key,
                        title: detail.title,
                        lines: detail.lines,
                    });
        }
        let entry = cache.as_ref()?;
        Some(TurnDetailOverlayView {
            title: entry.title.clone(),
            lines: entry.lines.clone(),
            scroll: self.detail_overlay_scroll(),
        })
    }

    pub fn transcript_surface_view(&self) -> TranscriptSurfaceView {
        let cells = self.history_cells();
        TranscriptSurfaceView {
            chat: ChatView::new(cells.clone(), self.transcript_scroll()),
            transcript_overlay: TranscriptOverlayView {
                lines: self.transcript_overlay_lines_for_cells(&cells),
                scroll: self.transcript_overlay_scroll(),
            },
            turn_detail_overlay: self.turn_detail_overlay_view_for_cells(&cells),
        }
    }

    pub fn overlay_stack_view(
        &self,
        surface: &TranscriptSurfaceView,
        command_surface: &CommandSurfaceView,
    ) -> OverlayStackView {
        OverlayStackView {
            state: self.overlay_state(),
            command_popup: command_surface.popup.clone(),
            transcript: surface.transcript_overlay.clone(),
            turn_detail: surface.turn_detail_overlay.clone(),
        }
    }

    pub fn shell_snapshot(&self) -> ShellSnapshot {
        let surface = self.transcript_surface_view();
        let command_surface = self.command_surface_view();
        let status_bar = self.status_bar_view().into_chrome();
        let mut composer = self
            .composer_view_from_command_surface(&command_surface)
            .into_chrome();
        // Inject the context line (workspace/session/model info) from the status bar
        // into the composer so it appears as the top row of the input area.
        composer.context_line = status_bar.header.clone();
        // Hide cursor when a full-screen overlay is open so it does not bleed through.
        if self.transcript_overlay_open || self.detail_overlay_open {
            composer.cursor_visible = false;
        }
        ShellSnapshot {
            chrome: ShellChromeView {
                status_bar,
                composer,
            },
            overlays: self.overlay_stack_view(&surface, &command_surface),
            surface,
        }
    }

    pub fn active_turn_banner(&self) -> Option<String> {
        self.history_cells().active_banner()
    }

    pub fn hide_status_summary_during_streaming(&self) -> bool {
        self.active_session()
            .active_turn
            .as_ref()
            .is_some_and(|turn| matches!(turn.cell.phase, Some(TurnPhase::Streaming)))
            || self.active_streaming_preview().is_some()
    }

    pub fn is_busy(&self) -> bool {
        runtime_status_is_busy(&self.runtime_status)
    }

    pub fn task_spinner(&self) -> &'static str {
        const FRAMES: [&str; 4] = ["-", "\\", "|", "/"];
        FRAMES[self.pulse_frame() % FRAMES.len()]
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
                session.cursor_pos = existing.cursor_pos;
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
                if session.active_turn.is_none() {
                    session.active_turn = existing.active_turn.clone();
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

        // Save current selected session ID BEFORE replacing the list so we can
        // detect a real session switch vs. a same-session catalog refresh.
        let current_selected_id = self
            .sessions
            .get(self.selected_session)
            .map(|s| s.id.clone())
            .unwrap_or_default();

        self.sessions = sessions;
        if let Some(index) = self
            .sessions
            .iter()
            .position(|session| session.id == selected_session_id)
        {
            if selected_session_id != current_selected_id {
                // Actual session switch — reset scroll, overlays, unread count.
                self.select_session(index);
            } else {
                // Same session, just keep the index in sync (e.g. catalog reordering).
                self.selected_session = index;
            }
        }
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
                    "you",
                    "You",
                    &truncate_for_timeline(&message, 600),
                );
                self.start_or_refresh_active_turn(
                    None,
                    TurnPhase::Submitted,
                    "Assistant response",
                    format!(
                        "Waiting for session {} to reply via profile {} ({}).",
                        session_id, profile, model
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

        self.active_session_mut().draft.clear();
        self.active_session_mut().cursor_pos = 0;
        self.bottom_pane.reset_command_selection();
        self.transcript_overlay_open = false;
        self.detail_overlay_open = false;
        self.detail_overlay_target = None;
        self.transcript_overlay.scroll_home();
        self.detail_overlay.scroll_home();
        self.sync_detail_overlay_flags();
        self.refresh_overlay_state();
        self.transcript.follow = true;

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
        self.transcript.scroll_home();
        self.transcript_overlay.scroll_home();
        self.detail_overlay.scroll_home();
        self.sessions[index].unread = 0;
        self.transcript_overlay_open = false;
        self.detail_overlay_open = false;
        self.detail_overlay_target = None;
        self.sync_detail_overlay_flags();
        self.refresh_overlay_state();
    }

    fn command_menu_active(&self) -> bool {
        self.overlay_state().is_command_palette()
    }

    fn refresh_overlay_state(&mut self) {
        self.overlay = OverlayState::from_shell_state(
            self.command_query(),
            self.transcript_overlay_open,
            self.detail_overlay_open,
        );
    }

    fn sync_detail_overlay_flags(&mut self) {
        let active_target = if self.detail_overlay_open {
            self.history_cells()
                .resolve_detail_target(self.detail_overlay_target)
        } else {
            None
        };
        self.detail_overlay_target = active_target;
        if let Some(session) = self.sessions.get_mut(self.selected_session) {
            if let Some(active_turn) = session.active_turn.as_mut() {
                active_turn.cell.details_expanded =
                    matches!(active_target, Some(HistoryCellKey::Active))
                        && !active_turn.cell.details.is_empty();
            }
            for (index, entry) in session.timeline.iter_mut().enumerate() {
                entry.details_expanded = matches!(active_target, Some(HistoryCellKey::Committed(selected)) if selected == index);
            }
        }
    }

    fn command_menu_should_complete(&self) -> bool {
        if !self.command_menu_active() {
            return false;
        }

        if command_invocation_ready(self.active_draft()) {
            return false;
        }

        let surface = self.command_surface_view();

        if let Some(completion) = surface.skill_completion {
            return self.active_draft().trim_end() != completion.trim_end();
        }

        let Some(command) = surface.selected else {
            return false;
        };

        self.active_draft().trim_end() != command_completion(command.command).trim_end()
    }

    fn select_next_command_match(&mut self) {
        self.bottom_pane
            .select_next_command_match(self.command_surface_view().matches.len());
    }

    fn select_previous_command_match(&mut self) {
        self.bottom_pane
            .select_previous_command_match(self.command_surface_view().matches.len());
    }

    fn complete_selected_command(&mut self) {
        let surface = self.command_surface_view();

        if let Some(completion) = surface.skill_completion {
            self.active_session_mut().cursor_pos = completion.chars().count();
            self.active_session_mut().draft = completion;
            self.bottom_pane.reset_command_selection();
            self.refresh_overlay_state();
            return;
        }

        let Some(command) = surface.selected else {
            return;
        };

        let completion = command_completion(command.command);
        self.active_session_mut().cursor_pos = completion.chars().count();
        self.active_session_mut().draft = completion;
        self.bottom_pane.reset_command_selection();
        self.refresh_overlay_state();
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
        let selected = matches.get(
            self.bottom_pane
                .command_menu_index
                .min(matches.len().saturating_sub(1)),
        )?;
        Some(format!("{command_prefix}{} ", selected.name))
    }

    fn route_command(&mut self, command: &str) -> AppAction {
        let canonical = rewrite_command_alias(command);
        let parts = canonical.split_whitespace().collect::<Vec<_>>();
        let Some(name) = parts.first().copied() else {
            self.push_command_error("Usage: /help");
            return AppAction::Continue;
        };

        match name {
            "help" => match parts.as_slice() {
                ["help"] => {
                    self.push_help(None);
                    AppAction::Continue
                }
                ["help", category, ..] => {
                    if let Some(category) = CommandCategory::parse(category) {
                        self.push_help(Some(category));
                    } else {
                        self.push_command_error(format!(
                            "Unknown help category: {}. Use /help to browse the grouped catalog.",
                            category
                        ));
                    }
                    AppAction::Continue
                }
                _ => AppAction::Continue,
            },
            "new" => self.route_new_command(&parts[1..]),
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

    fn route_new_command(&mut self, args: &[&str]) -> AppAction {
        let session_id = if args.is_empty() {
            generate_session_id()
        } else {
            args.join(" ")
        };
        self.prepare_session_switch(&session_id, true)
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
                self.push_command_error("Usage: /model list | /model show | /model use <profile>");
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
                self.push_command_error("Usage: /profile <name> | /profile show | /profile list");
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
                self.push_command_error("Usage: /gateway status");
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
                self.push_command_error("Usage: /adapter status");
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
                self.push_command_error("Usage: /node list | /node show <id>");
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
                    "Usage: /sandbox status | /sandbox inspect <env> | /sandbox rebuild <env> | /sandbox clean",
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
                    "Usage: /session list | /session show | /session switch <id> | /new [id]",
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
                self.runtime_status = "canceling".to_owned();
                self.active_session_mut().state = SessionState::Active;
                self.push_activity("run", format!("Requested cancellation for {run_id}."));
                self.attach_turn_detail(
                    Some(&run_id),
                    TurnPhase::WaitingOnCapability,
                    TranscriptDetailKind::Notice,
                    "Run cancellation requested",
                    format!(
                        "run_id={}\nWaiting for the runtime to stop the current assistant turn.",
                        run_id
                    ),
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
                self.runtime_status = "running".to_owned();
                self.active_session_mut().state = SessionState::Active;
                self.push_activity("run", format!("Requested retry for {run_id}."));
                self.start_or_refresh_active_turn(
                    None,
                    TurnPhase::Submitted,
                    "Assistant response",
                    format!(
                        "Retry requested for run {}. Waiting for the runtime to start a new assistant turn.",
                        run_id
                    ),
                );
                self.attach_turn_detail(
                    None,
                    TurnPhase::Submitted,
                    TranscriptDetailKind::Notice,
                    "Run retry requested",
                    format!(
                        "retry_of={}\nWaiting for the runtime to start a new assistant turn.",
                        run_id
                    ),
                );
                AppAction::RetryRun(run_id)
            }
            _ => {
                self.push_command_error("Usage: /run stop | /run retry");
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
                self.push_command_error("Usage: /inspect last");
                AppAction::Continue
            }
        }
    }

    fn route_explicit_capability(&mut self, kind: &str, args: Vec<&str>) -> AppAction {
        let Some((name, input)) = split_capability_args(args) else {
            self.push_command_error(match kind {
                "tool" => "Usage: /tool <name> <input>",
                "skill" => "Usage: /skill <name> <input>",
                _ => "Usage: /workflow <name> <input>",
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
        self.start_or_refresh_active_turn(
            None,
            TurnPhase::Submitted,
            "Assistant response",
            format!(
                "Waiting for explicit {} {} to run.\ninput={}",
                kind,
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
        self.push_system_entry("Session list", body);
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
                .map(|spec| format!("{}  {}", spec.usage(), spec.summary))
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
            "input.command / opens the slash popup, typing filters, Tab completes, Enter executes"
                .to_owned(),
            "navigation     PageUp/PageDown scroll the transcript, Ctrl+C quits".to_owned(),
            "aliases        /mosaic ... remains supported as a compatibility alias, but bare slash commands are canonical inside TUI.".to_owned(),
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
        self.push_operator_card(title, body);
    }

    pub(crate) fn push_operator_card(&mut self, title: impl Into<String>, body: impl Into<String>) {
        let title = title.into();
        let body = body.into();
        self.push_timeline_block(
            TimelineKind::System,
            TranscriptBlock::OperatorResultCard,
            "control-plane",
            &title,
            &body,
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
        self.push_timeline_block(kind, kind.default_block(), actor, title, body);
    }

    fn push_timeline_block(
        &mut self,
        kind: TimelineKind,
        block: TranscriptBlock,
        actor: &str,
        title: &str,
        body: &str,
    ) {
        if let Some(session) = self.sessions.get_mut(self.selected_session) {
            session.timeline.push(TimelineEntry {
                timestamp: current_hhmm(),
                kind,
                block,
                actor: actor.to_owned(),
                title: title.to_owned(),
                body: body.to_owned(),
                run_id: None,
                phase: None,
                details: Vec::new(),
                details_expanded: false,
                exec_calls: vec![],
            });
            trim_timeline(session);
        }
        self.transcript.follow = true;
    }
}

fn new_active_turn_entry(
    timestamp: &str,
    run_id: Option<&str>,
    phase: TurnPhase,
    title: impl Into<String>,
    body: impl Into<String>,
) -> TimelineEntry {
    TimelineEntry {
        timestamp: timestamp.to_owned(),
        kind: TimelineKind::Agent,
        block: TranscriptBlock::AssistantMessage,
        actor: "assistant".to_owned(),
        title: title.into(),
        body: body.into(),
        run_id: run_id.map(str::to_owned),
        phase: Some(phase),
        details: Vec::new(),
        details_expanded: false,
        exec_calls: vec![],
    }
}

fn transcript_view_for_session<'a>(
    session: &'a SessionRecord,
    scroll: u16,
    spinner_tick: usize,
) -> TranscriptView<'a> {
    TranscriptView {
        entries: &session.timeline,
        active_entry: session.active_turn.as_ref().map(|turn| &turn.cell),
        active_revision: session.active_turn.as_ref().map(|turn| turn.revision),
        streaming_preview: session
            .active_turn
            .as_ref()
            .filter(|turn| matches!(turn.cell.phase, Some(TurnPhase::Streaming)))
            .map(|turn| turn.cell.body.as_str())
            .or(session.streaming_preview.as_deref()),
        scroll,
        spinner_tick,
    }
}

fn preserved_expandable_turn(session: &SessionRecord) -> Option<TimelineEntry> {
    let view = transcript_view_for_session(session, 0, 0);
    view.active_entry
        .filter(|entry| !entry.details.is_empty())
        .cloned()
        .or_else(|| {
            view.entries
                .iter()
                .rev()
                .find(|entry| !entry.details.is_empty())
                .cloned()
        })
}

fn trim_timeline(session: &mut SessionRecord) {
    if session.timeline.len() > 400 {
        let overflow = session.timeline.len() - 400;
        session.timeline.drain(0..overflow);
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
        cursor_pos: 0,
        transcript_len: 0,
        current_run_id: None,
        current_gateway_run_id: None,
        last_gateway_run_id: None,
        memory_summary: None,
        compressed_context: None,
        references: Vec::new(),
        streaming_preview: None,
        streaming_run_id: None,
        active_turn: None,
        timeline: Vec::new(),
    }
}

fn transcript_entry(message: &TranscriptMessage) -> TimelineEntry {
    let (kind, block, actor, title) = match message.role {
        TranscriptRole::System => (
            TimelineKind::System,
            TranscriptBlock::SystemNotice,
            "system",
            "System",
        ),
        TranscriptRole::User => (
            TimelineKind::Operator,
            TranscriptBlock::UserMessage,
            "user",
            "User",
        ),
        TranscriptRole::Assistant => (
            TimelineKind::Agent,
            TranscriptBlock::AssistantMessage,
            "assistant",
            "Assistant",
        ),
        TranscriptRole::Tool => (
            TimelineKind::Tool,
            TranscriptBlock::ExecutionCard,
            "tool",
            "Tool",
        ),
    };

    TimelineEntry {
        timestamp: message.created_at.format("%H:%M").to_string(),
        kind,
        block,
        actor: actor.to_owned(),
        title: title.to_owned(),
        body: message.content.clone(),
        run_id: None,
        phase: None,
        details: Vec::new(),
        details_expanded: false,
        exec_calls: vec![],
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

fn detail_kind_from_text(actor: &str, body: &str) -> TranscriptDetailKind {
    let actor = actor.to_ascii_lowercase();
    let body = body.to_ascii_lowercase();

    if actor.contains("workflow") {
        return TranscriptDetailKind::Workflow;
    }
    if actor.contains("skill") {
        return TranscriptDetailKind::Skill;
    }
    if actor.contains("provider") {
        return TranscriptDetailKind::Provider;
    }
    if actor.contains("tool") && !body.contains("mcp") {
        return TranscriptDetailKind::Tool;
    }
    if body.contains("mcp") {
        return TranscriptDetailKind::Mcp;
    }
    if body.contains("sandbox") {
        return TranscriptDetailKind::Sandbox;
    }
    if body.contains("node") {
        return TranscriptDetailKind::Node;
    }
    TranscriptDetailKind::Capability
}

fn classify_failure(origin: Option<&str>, fallback: Option<&str>) -> &'static str {
    match origin.unwrap_or_default() {
        "provider" => "provider failure",
        "tool" => "tool logic failure",
        "mcp" | "mcp_transport" => "MCP transport failure",
        "node" => "node execution failure",
        "sandbox" => "sandbox preparation failure",
        "workflow" | "orchestration" => "workflow/orchestration failure",
        "gateway" => "gateway/control-plane failure",
        "permission" => "permission failure",
        "skill" => "skill execution failure",
        _ => match fallback.unwrap_or_default() {
            "provider" => "provider failure",
            "tool" => "tool logic failure",
            "workflow" => "workflow/orchestration failure",
            "skill" => "skill execution failure",
            _ => "runtime failure",
        },
    }
}

fn next_action_for_failure(origin: Option<&str>) -> &'static str {
    match origin.unwrap_or_default() {
        "provider" => "/run retry or /inspect last",
        "tool" => "/inspect last or rerun /tool <name> <input>",
        "mcp" | "mcp_transport" => "/inspect last and verify MCP server health",
        "node" => "/node list, /node show <id>, or /inspect last",
        "sandbox" => "/sandbox status, /sandbox inspect <env>, or /sandbox rebuild <env>",
        "workflow" | "orchestration" => "/inspect last or rerun /workflow <name> <input>",
        "gateway" => "/gateway status or /run retry",
        "permission" => "/inspect last and review the active policy",
        "skill" => "/inspect last or rerun /skill <name> <input>",
        _ => "/inspect last",
    }
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
    format!("{} ", command.trim())
}

fn command_invocation_ready(draft: &str) -> bool {
    let command = rewrite_command_alias(draft.trim().trim_start_matches('/'));
    if command.is_empty() {
        return false;
    }

    let parts = command.split_whitespace().collect::<Vec<_>>();
    match parts.as_slice() {
        ["help"] | ["help", ..] => true,
        ["new"] | ["new", ..] => true,
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
            "Unknown command: /{}. Use /help to browse the command catalog.",
            command.trim()
        );
    }

    format!(
        "Unknown command: /{}. Did you mean {}?",
        command.trim(),
        suggestions
            .into_iter()
            .take(3)
            .map(|spec| spec.usage())
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
            let alias_searchable = command_alias_search_text(spec);
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
        || spec.aliases.iter().any(|alias| {
            let alias_tokens = command_search_tokens(alias);
            command_tokens_match_query(&alias_tokens, &query_tokens)
        })
}

fn command_search_text(command: &str) -> String {
    normalize_command_text(command)
}

fn command_alias_search_text(spec: CommandSpec) -> String {
    spec.aliases
        .iter()
        .map(|alias| command_search_text(alias))
        .collect::<Vec<_>>()
        .join(" ")
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

fn rewrite_command_alias(command: &str) -> String {
    let trimmed = command.trim().trim_start_matches('/').trim();
    if trimmed.is_empty() {
        return String::new();
    }

    let mut best: Option<(usize, String)> = None;
    for spec in LOCAL_COMMANDS {
        for alias in spec.aliases {
            let alias_completion = command_completion(alias);
            let alias_prefix = alias_completion.trim().trim_start_matches('/');
            if trimmed == alias_prefix || trimmed.starts_with(&format!("{alias_prefix} ")) {
                let matched_len = alias_prefix.len();
                let canonical_prefix = spec.command.trim_start_matches('/');
                let rewritten = if trimmed == alias_prefix {
                    canonical_prefix.to_owned()
                } else {
                    let remainder = trimmed[matched_len..].trim_start();
                    if remainder.is_empty() {
                        canonical_prefix.to_owned()
                    } else {
                        format!("{canonical_prefix} {remainder}")
                    }
                };
                if best
                    .as_ref()
                    .is_none_or(|(best_len, _)| matched_len > *best_len)
                {
                    best = Some((matched_len, rewritten));
                }
            }
        }
    }

    best.map(|(_, rewritten)| rewritten)
        .unwrap_or_else(|| trimmed.to_owned())
}

fn generate_session_id() -> String {
    use chrono::Local;

    format!("session-{}", Local::now().format("%Y%m%d-%H%M%S"))
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

    use super::{
        App, AppAction, ComposerRunRequest, InputMode, ProfileOption, SessionRecord, SessionState,
        ShellState, SkillOption, StoredSessionRecord, TimelineKind, TranscriptBlock,
        interactive_session_record, preserved_expandable_turn,
    };
    use mosaic_session_core::TranscriptRole;
    use crate::history_cell::HistoryCellKey;
    use crate::transcript::{
        ActiveTurn, TimelineEntry, TranscriptDetail, TranscriptDetailKind, TurnPhase,
    };

    #[test]
    fn switching_sessions_resets_unread_and_scroll() {
        let mut app = App::new("/tmp/mosaic".into());
        app.transcript.scroll = 8;
        app.transcript_overlay.scroll = 5;
        app.detail_overlay.scroll = 3;
        app.sessions[1].unread = 3;

        app.select_session(1);

        assert_eq!(app.transcript.scroll, 0);
        assert_eq!(app.transcript_overlay.scroll, 0);
        assert_eq!(app.detail_overlay.scroll, 0);
        assert_eq!(app.sessions[1].unread, 0);
    }

    #[test]
    fn composer_view_tracks_mode_and_busy_state() {
        let mut app = App::new("/tmp/mosaic".into());
        app.active_session_mut().draft = "/help".to_owned();
        app.runtime_status = "running".to_owned();

        let pane = app.composer_view();

        assert_eq!(pane.mode, InputMode::Command);
        assert_eq!(pane.shell_state, ShellState::Commanding);
        assert!(pane.busy);
        assert!(pane.placeholder.contains("slash"));
    }

    #[test]
    fn shell_state_transitions_between_idle_composing_running_and_overlays() {
        let mut app = App::new("/tmp/mosaic".into());
        assert_eq!(app.shell_state(), ShellState::Idle);

        app.active_session_mut().draft = "hello".to_owned();
        assert_eq!(app.shell_state(), ShellState::Composing);

        app.runtime_status = "running".to_owned();
        assert_eq!(app.shell_state(), ShellState::Running);

        app.active_session_mut().draft = "/help".to_owned();
        assert_eq!(app.shell_state(), ShellState::Commanding);

        app.active_session_mut().draft.clear();
        app.transcript_overlay_open = true;
        assert_eq!(app.shell_state(), ShellState::TranscriptOverlay);

        app.transcript_overlay_open = false;
        app.detail_overlay_open = true;
        assert_eq!(app.shell_state(), ShellState::TurnDetailOverlay);
    }

    #[test]
    fn overlay_state_tracks_slash_draft_without_stealing_input() {
        let mut app = App::new("/tmp/mosaic".into());
        app.active_session_mut().draft = "/hel".to_owned();
        assert!(app.overlay_state().is_command_palette());
        assert_eq!(app.active_draft(), "/hel");

        app.active_session_mut().draft.clear();
        assert!(!app.overlay_state().is_command_palette());
    }

    #[test]
    fn ctrl_t_toggles_transcript_overlay_without_mutating_draft() {
        let mut app = App::new("/tmp/mosaic".into());
        app.active_session_mut().draft = "draft".to_owned();

        let _ = app.handle_key(KeyEvent::new(KeyCode::Char('t'), KeyModifiers::CONTROL));
        assert!(app.overlay_state().is_transcript());
        assert_eq!(app.active_draft(), "draft");

        let _ = app.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        assert!(!app.overlay_state().is_transcript());
        assert_eq!(app.active_draft(), "draft");
    }

    #[test]
    fn overlay_scroll_keys_target_the_open_overlay_not_main_transcript() {
        let mut app = App::new("/tmp/mosaic".into());
        app.transcript.scroll = 2;
        app.transcript_overlay_open = true;
        app.refresh_overlay_state();

        let _ = app.handle_key(KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE));
        assert_eq!(app.transcript.scroll, 2);
        assert_eq!(app.transcript_overlay.scroll, 5);

        app.transcript_overlay_open = false;
        app.detail_overlay_open = true;
        app.refresh_overlay_state();
        let _ = app.handle_key(KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE));
        assert_eq!(app.detail_overlay.scroll, 5);
        assert_eq!(app.transcript.scroll, 2);
    }

    #[test]
    fn transcript_view_exposes_scroll_and_streaming_preview() {
        let mut app = App::new_interactive(
            "/tmp/mosaic".into(),
            "demo".to_owned(),
            "openai".to_owned(),
            "gpt-5.4-mini".to_owned(),
            Vec::new(),
            Vec::new(),
            false,
        );
        app.transcript.scroll = 9;
        app.active_session_mut().streaming_preview = Some("partial".to_owned());

        let view = app.transcript_view();

        assert_eq!(view.scroll, 9);
        assert_eq!(view.streaming_preview, Some("partial"));
        assert_eq!(view.entries.len(), app.visible_timeline().len());
    }

    #[test]
    fn transcript_overlay_cache_tracks_active_turn_revision() {
        let mut app = App::new("/tmp/mosaic".into());
        app.apply_run_event(RunEvent::RunStarted {
            run_id: "run-1".to_owned(),
            input: "hello".to_owned(),
        });

        let _ = app.transcript_overlay_lines();
        let first_key = app.transcript_overlay_cache_key();

        app.apply_run_event(RunEvent::OutputDelta {
            run_id: "run-1".to_owned(),
            chunk: "world".to_owned(),
            accumulated_chars: 5,
        });
        let _ = app.transcript_overlay_lines();
        let second_key = app.transcript_overlay_cache_key();

        assert_ne!(first_key, second_key);
    }

    #[test]
    fn detail_overlay_cache_tracks_active_turn_revision() {
        let mut app = App::new("/tmp/mosaic".into());
        app.apply_run_event(RunEvent::ToolCalling {
            name: "read_file".to_owned(),
            call_id: "call-1".to_owned(),
            summary: Some("target=tool:read_file".to_owned()),
        });
        app.detail_overlay_open = true;
        app.detail_overlay_target = Some(HistoryCellKey::Active);
        app.sync_detail_overlay_flags();

        let _ = app.detail_overlay_lines();
        let first_key = app.detail_overlay_cache_key();

        app.apply_run_event(RunEvent::ToolFinished {
            name: "read_file".to_owned(),
            call_id: "call-1".to_owned(),
            summary: Some("target=tool:read_file\nstatus=ok".to_owned()),
        });
        let _ = app.detail_overlay_lines();
        let second_key = app.detail_overlay_cache_key();

        assert_ne!(first_key, second_key);
    }

    #[test]
    fn detail_overlay_cache_key_tracks_selected_history_cell_target() {
        let mut app = App::new("/tmp/mosaic".into());
        let first_index = app.active_session().timeline.len();
        app.active_session_mut().timeline.push(TimelineEntry {
            timestamp: "12:00".to_owned(),
            kind: TimelineKind::Agent,
            block: TranscriptBlock::AssistantMessage,
            actor: "assistant".to_owned(),
            title: "First".to_owned(),
            body: "done".to_owned(),
            run_id: Some("run-1".to_owned()),
            phase: Some(TurnPhase::Completed),
            details: vec![TranscriptDetail {
                kind: TranscriptDetailKind::Tool,
                title: "first".to_owned(),
                body: "target=tool:first".to_owned(),
            }],
            details_expanded: false,
            exec_calls: vec![],
        });
        let second_index = app.active_session().timeline.len();
        app.active_session_mut().timeline.push(TimelineEntry {
            timestamp: "12:01".to_owned(),
            kind: TimelineKind::Agent,
            block: TranscriptBlock::AssistantMessage,
            actor: "assistant".to_owned(),
            title: "Second".to_owned(),
            body: "done".to_owned(),
            run_id: Some("run-2".to_owned()),
            phase: Some(TurnPhase::Completed),
            details: vec![TranscriptDetail {
                kind: TranscriptDetailKind::Tool,
                title: "second".to_owned(),
                body: "target=tool:second".to_owned(),
            }],
            details_expanded: false,
            exec_calls: vec![],
        });

        app.detail_overlay_open = true;
        app.detail_overlay_target = Some(HistoryCellKey::Committed(first_index));
        app.sync_detail_overlay_flags();
        let _ = app.detail_overlay_lines();
        let first_key = app.detail_overlay_cache_key();

        app.detail_overlay_target = Some(HistoryCellKey::Committed(second_index));
        app.sync_detail_overlay_flags();
        let _ = app.detail_overlay_lines();
        let second_key = app.detail_overlay_cache_key();

        assert_ne!(first_key, second_key);
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
        assert_eq!(app.escape_hint(), "clear");
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

        assert_eq!(app.active_draft(), "/gateway status ");
    }

    #[test]
    fn tab_accepts_current_command_completion() {
        let mut app = App::new("/tmp/mosaic".into());
        app.active_session_mut().draft = "/he".to_owned();

        app.handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));

        assert_eq!(app.active_draft(), "/help ");
    }

    #[test]
    fn gateway_status_command_renders_inline_card() {
        let mut app = App::new("/tmp/mosaic".into());
        app.gateway_summary = Some("Gateway ready".to_owned());
        app.gateway_detail = Some("transport=http+sse".to_owned());
        app.node_summary = Some("nodes=1".to_owned());
        app.node_detail = Some("healthy".to_owned());
        app.active_session_mut().draft = "/gateway status".to_owned();

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
        app.active_session_mut().draft = "/sandbox status".to_owned();

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
        app.active_session_mut().draft = "/sandbox rebuild python-capability-demo".to_owned();

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

        assert_eq!(app.active_draft(), "/sandbox status ");
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
        app.active_session_mut().draft = "/adapter status".to_owned();

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
        list_app.active_session_mut().draft = "/node list".to_owned();
        assert_eq!(list_app.submit_composer(), AppAction::NodeList);

        let mut show_app = App::new("/tmp/mosaic".into());
        show_app.active_session_mut().draft = "/node show headless-1".to_owned();
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
    fn entering_help_runs_inline_catalog_instead_of_stalling_on_completion() {
        let mut app = App::new("/tmp/mosaic".into());
        app.active_session_mut().draft = "/help".to_owned();

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
    fn mosaic_alias_still_routes_to_inline_help_catalog() {
        let mut app = App::new("/tmp/mosaic".into());
        app.active_session_mut().draft = "/mosaic".to_owned();

        let action = app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

        assert_eq!(action, AppAction::Continue);
        assert_eq!(
            app.active_session()
                .timeline
                .last()
                .map(|entry| entry.title.as_str()),
            Some("Mosaic command reference")
        );
    }

    #[test]
    fn new_command_creates_a_fresh_session_inline() {
        let mut app = App::new("/tmp/mosaic".into());
        app.active_session_mut().draft = "/new".to_owned();

        let action = app.submit_composer();

        assert_eq!(action, AppAction::Continue);
        assert!(app.session_label().starts_with("session-"));
        assert_eq!(
            app.active_session()
                .timeline
                .last()
                .map(|entry| entry.title.as_str()),
            Some("Session selected")
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
            Some("Session list")
        );
    }

    #[test]
    fn model_list_renders_inline_operator_card_in_the_transcript_shell() {
        let mut app = App::new_interactive(
            "/tmp/mosaic".into(),
            "demo".to_owned(),
            "openai".to_owned(),
            "gpt-5.4-mini".to_owned(),
            vec![
                ProfileOption {
                    name: "openai".to_owned(),
                    model: "gpt-5.4-mini".to_owned(),
                    provider_type: "openai".to_owned(),
                },
                ProfileOption {
                    name: "ollama".to_owned(),
                    model: "qwen3".to_owned(),
                    provider_type: "ollama".to_owned(),
                },
            ],
            Vec::new(),
            false,
        );
        app.active_session_mut().draft = "/model list".to_owned();

        let action = app.submit_composer();

        assert_eq!(action, AppAction::Continue);
        assert_eq!(
            app.active_session()
                .timeline
                .last()
                .map(|entry| entry.title.as_str()),
            Some("Runtime profiles")
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
        assert_eq!(app.activity.len(), initial_activity_len + 1);
        assert_eq!(app.active_session().timeline.len(), initial_timeline_len);
        assert_eq!(
            app.active_session()
                .active_turn
                .as_ref()
                .map(|turn| &turn.cell)
                .and_then(|entry| entry.details.last())
                .map(|detail| detail.title.as_str()),
            Some("Run started")
        );
        assert_eq!(
            app.active_session()
                .active_turn
                .as_ref()
                .map(|turn| &turn.cell)
                .and_then(|entry| entry.phase),
            Some(TurnPhase::Queued)
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
            .active_turn
            .as_ref()
            .map(|turn| &turn.cell)
            .and_then(|entry| entry.details.last())
            .map(|detail| detail.body.clone())
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
    fn output_delta_replaces_waiting_placeholder_with_streamed_body() {
        let mut app = App::new("/tmp/mosaic".into());

        app.apply_run_event(RunEvent::RunStarted {
            run_id: "run-1".to_owned(),
            input: "hello".to_owned(),
        });
        app.apply_run_event(RunEvent::OutputDelta {
            run_id: "run-1".to_owned(),
            chunk: "partial reply".to_owned(),
            accumulated_chars: 13,
        });

        let active = app
            .active_session()
            .active_turn
            .as_ref()
            .expect("active turn should exist");
        assert_eq!(active.cell.phase, Some(TurnPhase::Streaming));
        assert_eq!(active.cell.body, "partial reply");
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
            .active_turn
            .as_ref()
            .map(|turn| &turn.cell)
            .expect("active turn should exist");
        assert_eq!(last.kind, TimelineKind::Agent);
        assert_eq!(last.block, TranscriptBlock::AssistantMessage);
        assert_eq!(last.title, "Assistant response");
        assert_eq!(last.phase, Some(TurnPhase::Failed));
        let detail = last.details.last().expect("failure detail should exist");
        assert_eq!(detail.title, "Tool failed: read_file");
        assert!(detail.body.contains("permission denied"));
        assert!(detail.body.contains("next=/inspect last"));
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

        let last = app
            .active_session()
            .active_turn
            .as_ref()
            .map(|turn| &turn.cell)
            .expect("turn should exist");
        let tool_body = last
            .details
            .iter()
            .find(|detail| detail.title == "Tool call: read_file")
            .map(|detail| detail.body.clone())
            .unwrap_or_default();
        let workflow_body = last
            .details
            .iter()
            .find(|detail| detail.title == "Workflow step: fanout")
            .map(|detail| detail.body.clone())
            .unwrap_or_default();
        assert_eq!(last.block, TranscriptBlock::AssistantMessage);
        assert!(tool_body.contains("exec_target=local"));
        assert!(workflow_body.contains("orchestration_owner=workflow_engine"));
    }

    #[test]
    fn run_failed_captures_failure_origin_and_next_action() {
        let mut app = App::new("/tmp/mosaic".into());

        app.apply_run_event(RunEvent::RunFailed {
            run_id: "run-1".to_owned(),
            error: "sandbox dependencies missing".to_owned(),
            failure_kind: Some("runtime".to_owned()),
            failure_origin: Some("sandbox".to_owned()),
        });

        let last = app
            .active_session()
            .timeline
            .last()
            .expect("finalized turn should exist");
        assert_eq!(last.block, TranscriptBlock::AssistantMessage);
        assert_eq!(last.phase, Some(TurnPhase::Failed));
        let detail = last.details.last().expect("failure detail should exist");
        assert!(detail.body.contains("failure=sandbox preparation failure"));
        assert!(detail.body.contains("next=/sandbox status"));
    }

    #[test]
    fn sync_session_catalog_preserves_draft_while_streaming_preview_is_active() {
        let mut app = App::new("/tmp/mosaic".into());
        app.active_session_mut().draft = "retry later".to_owned();
        app.active_session_mut().streaming_preview = Some("partial reply".to_owned());
        let current_id = app.session_label().to_owned();

        app.sync_session_catalog(Vec::new(), &current_id);

        assert_eq!(app.active_draft(), "retry later");
        assert_eq!(app.active_streaming_preview(), Some("partial reply"));
    }

    #[test]
    fn run_canceled_renders_notice_and_clears_streaming_preview() {
        let mut app = App::new("/tmp/mosaic".into());
        app.active_session_mut().streaming_preview = Some("partial".to_owned());
        app.active_session_mut().streaming_run_id = Some("run-1".to_owned());

        app.apply_run_event(RunEvent::RunCanceled {
            run_id: "run-1".to_owned(),
            reason: "operator requested stop".to_owned(),
        });

        let last = app
            .active_session()
            .timeline
            .last()
            .expect("timeline entry should exist");
        assert_eq!(last.block, TranscriptBlock::AssistantMessage);
        assert_eq!(last.phase, Some(TurnPhase::Canceled));
        assert_eq!(
            last.details.last().map(|detail| detail.title.as_str()),
            Some("Run canceled")
        );
        assert_eq!(app.active_streaming_preview(), None);
    }

    #[test]
    fn run_retry_command_starts_submitted_live_turn_instead_of_system_card() {
        let mut app = App::new("/tmp/mosaic".into());
        app.active_session_mut().last_gateway_run_id = Some("gw-run-1".to_owned());
        app.active_session_mut().draft = "/run retry".to_owned();

        let action = app.submit_composer();

        assert_eq!(action, AppAction::RetryRun("gw-run-1".to_owned()));
        assert_eq!(app.runtime_status, "running");
        let active = app
            .active_session()
            .active_turn
            .as_ref()
            .expect("retry should create a live turn");
        assert_eq!(active.cell.phase, Some(TurnPhase::Submitted));
        assert!(
            active
                .cell
                .body
                .contains("Retry requested for run gw-run-1")
        );
        assert_eq!(
            active
                .cell
                .details
                .last()
                .map(|detail| detail.title.as_str()),
            Some("Run retry requested")
        );
        assert_ne!(
            app.active_session()
                .timeline
                .last()
                .map(|entry| entry.title.as_str()),
            Some("Run retry requested")
        );
    }

    #[test]
    fn run_stop_command_mutates_current_live_turn_in_place() {
        let mut app = App::new("/tmp/mosaic".into());
        app.active_session_mut().current_gateway_run_id = Some("gw-run-1".to_owned());
        app.active_session_mut().active_turn = Some(ActiveTurn {
            cell: TimelineEntry {
                timestamp: "12:00".to_owned(),
                kind: TimelineKind::Agent,
                block: TranscriptBlock::AssistantMessage,
                actor: "assistant".to_owned(),
                title: "Assistant response".to_owned(),
                body: "partial reply".to_owned(),
                run_id: Some("gw-run-1".to_owned()),
                phase: Some(TurnPhase::Streaming),
                details: Vec::new(),
                details_expanded: false,
                exec_calls: vec![],
            },
            revision: 1,
        });
        app.active_session_mut().draft = "/run stop".to_owned();

        let action = app.submit_composer();

        assert_eq!(action, AppAction::CancelRun("gw-run-1".to_owned()));
        assert_eq!(app.runtime_status, "canceling");
        let active = app
            .active_session()
            .active_turn
            .as_ref()
            .expect("cancel request should keep the live turn");
        assert_eq!(active.cell.phase, Some(TurnPhase::WaitingOnCapability));
        assert_eq!(active.cell.body, "partial reply");
        assert_eq!(
            active
                .cell
                .details
                .last()
                .map(|detail| detail.title.as_str()),
            Some("Run cancellation requested")
        );
        assert_ne!(
            app.active_session()
                .timeline
                .last()
                .map(|entry| entry.title.as_str()),
            Some("Run cancellation requested")
        );
    }

    #[test]
    fn run_finished_preserves_streamed_body_and_pins_detail_to_committed_cell() {
        let mut app = App::new("/tmp/mosaic".into());
        app.apply_run_event(RunEvent::RunStarted {
            run_id: "run-1".to_owned(),
            input: "hello".to_owned(),
        });
        app.apply_run_event(RunEvent::OutputDelta {
            run_id: "run-1".to_owned(),
            chunk: "partial reply".to_owned(),
            accumulated_chars: 13,
        });
        app.detail_overlay_open = true;
        app.detail_overlay_target = Some(HistoryCellKey::Active);
        app.sync_detail_overlay_flags();

        app.apply_run_event(RunEvent::RunFinished {
            run_id: "run-1".to_owned(),
            output_preview: String::new(),
        });

        let committed_index = app.active_session().timeline.len().saturating_sub(1);
        let last = app
            .active_session()
            .timeline
            .last()
            .expect("finalized turn should exist");
        assert_eq!(last.body, "partial reply");
        assert_eq!(last.phase, Some(TurnPhase::Completed));
        assert_eq!(
            app.detail_overlay_target,
            Some(HistoryCellKey::Committed(committed_index))
        );
        let lines = app
            .detail_overlay_lines()
            .expect("detail overlay should stay open on the committed turn")
            .into_iter()
            .map(|line| line.to_string())
            .collect::<Vec<_>>();
        assert!(lines.iter().any(|line| line.contains("partial reply")));
        assert!(lines.iter().any(|line| line.contains("Run finished")));
        // transcript_len is advanced to timeline.len() after finalize so the next DB sync
        // knows how many messages are already covered and won't append duplicates.
        assert_eq!(
            app.active_session().transcript_len,
            app.active_session().timeline.len(),
            "transcript_len must equal timeline.len() after finalize to prevent duplicate on next DB sync"
        );
    }

    #[test]
    fn finalize_active_turn_does_not_cause_duplicate_on_next_sync() {
        // 1. Simulate an interactive run: RunStarted → OutputDelta → RunFinished
        let mut app = App::new("/tmp/mosaic".into());
        let initial_timeline_len = app.active_session().timeline.len();

        app.apply_run_event(RunEvent::RunStarted {
            run_id: "run-1".to_owned(),
            input: "hello".to_owned(),
        });
        app.apply_run_event(RunEvent::OutputDelta {
            run_id: "run-1".to_owned(),
            chunk: "world".to_owned(),
            accumulated_chars: 5,
        });
        app.apply_run_event(RunEvent::RunFinished {
            run_id: "run-1".to_owned(),
            output_preview: String::new(),
        });

        // finalize_active_turn pushes exactly 1 entry (assistant).
        assert_eq!(
            app.active_session().timeline.len(),
            initial_timeline_len + 1,
            "finalize should push exactly one assistant entry"
        );
        // transcript_len must match timeline.len() so next DB sync knows the window.
        assert_eq!(
            app.active_session().transcript_len,
            app.active_session().timeline.len(),
            "transcript_len must equal timeline.len() after finalize"
        );

        // 2. Simulate a DB sync that returns 1 assistant message (matching local state).
        let mut stored = StoredSessionRecord::new("demo", "Session", "openai", "agent", "gpt-5.4-mini");
        stored.append_message(TranscriptRole::Assistant, "world", None);
        app.sync_runtime_session_with_origin(&stored, "Local");

        // DB has same count as transcript_len → no append, no full-replace.
        // The assistant entry already in the timeline is preserved; no duplicate.
        assert_eq!(
            app.active_session().timeline.len(),
            initial_timeline_len + 1,
            "sync after finalize must not produce a duplicate assistant entry"
        );
    }

    #[test]
    fn initial_transcript_state_has_follow_true_so_load_scrolls_to_bottom() {
        // App::new uses TranscriptState::new() which sets follow=true.
        let app = App::new("/tmp/mosaic".into());
        assert!(
            app.transcript.follow,
            "transcript must start with follow=true so the first draw pins to bottom"
        );
    }

    #[test]
    fn user_message_survives_db_sync_when_db_is_empty() {
        // Simulate: user has typed and submitted a message, but the DB poll fires
        // before the runtime has saved the message to the DB.
        let mut app = App::new("/tmp/mosaic".into());
        // Do one sync to establish a non-zero transcript_len baseline (empty DB).
        let stored_empty = StoredSessionRecord::new("demo", "Session", "openai", "agent", "gpt-5.4-mini");
        app.sync_runtime_session_with_origin(&stored_empty, "Local");
        let baseline_len = app.active_session().timeline.len();

        // submit_composer (non-interactive path) calls queue_operator_instruction which
        // pushes with TimelineKind::Operator — the same kind that transcript_entry()
        // produces for TranscriptRole::User, making dedup possible.
        app.active_session_mut().draft = "hello".to_owned();
        let _ = app.submit_composer();
        assert_eq!(app.active_session().timeline.len(), baseline_len + 1);

        // DB poll fires and DB still has no messages.
        app.sync_runtime_session_with_origin(&stored_empty, "Local");

        // The user message must still be in the timeline.
        assert_eq!(
            app.active_session().timeline.len(),
            baseline_len + 1,
            "locally-pushed user message must survive a DB sync that hasn't caught up yet"
        );
        assert!(
            app.active_session()
                .timeline
                .iter()
                .any(|e| e.body.contains("hello")),
            "the 'hello' entry must still be present after the empty sync"
        );
    }

    #[test]
    fn user_message_deduped_when_db_catches_up() {
        // When DB eventually includes the message, it must not appear twice.
        let mut app = App::new("/tmp/mosaic".into());
        let stored_empty = StoredSessionRecord::new("demo", "Session", "openai", "agent", "gpt-5.4-mini");
        app.sync_runtime_session_with_origin(&stored_empty, "Local");
        let baseline_len = app.active_session().timeline.len();

        // Submit "hello" locally (TimelineKind::Operator, body = "hello").
        app.active_session_mut().draft = "hello".to_owned();
        let _ = app.submit_composer();

        // DB catches up and now has the same message (TranscriptRole::User → Operator kind).
        let mut stored_with_message = StoredSessionRecord::new("demo", "Session", "openai", "agent", "gpt-5.4-mini");
        stored_with_message.append_message(TranscriptRole::User, "hello", None);
        app.sync_runtime_session_with_origin(&stored_with_message, "Local");

        assert_eq!(
            app.active_session().timeline.len(),
            baseline_len + 1,
            "when DB confirms the message, timeline must have exactly one copy"
        );
    }

    #[test]
    fn run_failed_preserves_partial_streamed_body_in_committed_cell() {
        let mut app = App::new("/tmp/mosaic".into());
        app.apply_run_event(RunEvent::RunStarted {
            run_id: "run-1".to_owned(),
            input: "hello".to_owned(),
        });
        app.apply_run_event(RunEvent::OutputDelta {
            run_id: "run-1".to_owned(),
            chunk: "partial reply".to_owned(),
            accumulated_chars: 13,
        });

        app.apply_run_event(RunEvent::RunFailed {
            run_id: "run-1".to_owned(),
            error: "boom".to_owned(),
            failure_kind: Some("runtime".to_owned()),
            failure_origin: Some("runtime".to_owned()),
        });

        let last = app
            .active_session()
            .timeline
            .last()
            .expect("failed turn should be committed");
        assert_eq!(last.phase, Some(TurnPhase::Failed));
        assert_eq!(last.body, "partial reply");
        assert!(
            last.details
                .last()
                .map(|detail| detail.body.contains("boom"))
                .unwrap_or(false)
        );
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
        assert_eq!(last_timeline.title, "Assistant response");
        assert_eq!(last_timeline.phase, Some(TurnPhase::Completed));
        assert!(last_timeline.body.contains("done"));
        assert_eq!(
            last_timeline
                .details
                .last()
                .map(|detail| detail.title.as_str()),
            Some("Run finished")
        );
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
        assert_eq!(last_timeline.title, "Assistant response");
        assert_eq!(last_timeline.phase, Some(TurnPhase::Failed));
        assert!(
            last_timeline
                .details
                .last()
                .map(|detail| detail.body.contains("boom"))
                .unwrap_or(false)
        );
    }

    #[test]
    fn ctrl_o_toggles_latest_turn_detail_expansion() {
        let mut app = App::new("/tmp/mosaic".into());
        app.apply_run_event(RunEvent::ToolCalling {
            name: "read_file".to_owned(),
            call_id: "call-1".to_owned(),
            summary: Some("source=builtin | exec_target=local".to_owned()),
        });

        assert_eq!(
            app.active_session()
                .active_turn
                .as_ref()
                .map(|turn| turn.cell.details_expanded),
            Some(false)
        );
        let _ = app.handle_key(KeyEvent::new(KeyCode::Char('o'), KeyModifiers::CONTROL));
        assert_eq!(
            app.active_session()
                .active_turn
                .as_ref()
                .map(|turn| turn.cell.details_expanded),
            Some(true)
        );
    }

    #[test]
    fn detail_overlay_lines_prefer_active_turn_history_cell() {
        let mut app = App::new("/tmp/mosaic".into());
        app.apply_run_event(RunEvent::ToolCalling {
            name: "read_file".to_owned(),
            call_id: "call-1".to_owned(),
            summary: Some("target=tool:read_file".to_owned()),
        });
        let _ = app.handle_key(KeyEvent::new(KeyCode::Char('o'), KeyModifiers::CONTROL));

        let lines = app
            .detail_overlay_lines()
            .expect("detail overlay should expose active turn lines")
            .into_iter()
            .map(|line| line.to_string())
            .collect::<Vec<_>>();

        assert!(
            lines
                .iter()
                .any(|line| line.contains("Tool call: read_file"))
        );
        assert!(
            lines
                .iter()
                .any(|line| line.contains("target=tool:read_file"))
        );
    }

    #[test]
    fn detail_overlay_preserves_selected_committed_cell_when_new_active_turn_appears() {
        let mut app = App::new("/tmp/mosaic".into());
        let committed_index = app.active_session().timeline.len();
        app.active_session_mut().timeline.push(TimelineEntry {
            timestamp: "12:00".to_owned(),
            kind: TimelineKind::Agent,
            block: TranscriptBlock::AssistantMessage,
            actor: "assistant".to_owned(),
            title: "Committed".to_owned(),
            body: "done".to_owned(),
            run_id: Some("run-1".to_owned()),
            phase: Some(TurnPhase::Completed),
            details: vec![TranscriptDetail {
                kind: TranscriptDetailKind::Tool,
                title: "old".to_owned(),
                body: "target=tool:old".to_owned(),
            }],
            details_expanded: false,
            exec_calls: vec![],
        });
        app.detail_overlay_open = true;
        app.detail_overlay_target = Some(HistoryCellKey::Committed(committed_index));
        app.sync_detail_overlay_flags();

        app.active_session_mut().active_turn = Some(ActiveTurn {
            cell: TimelineEntry {
                timestamp: "12:01".to_owned(),
                kind: TimelineKind::Agent,
                block: TranscriptBlock::AssistantMessage,
                actor: "assistant".to_owned(),
                title: "Active".to_owned(),
                body: "working".to_owned(),
                run_id: Some("run-2".to_owned()),
                phase: Some(TurnPhase::Streaming),
                details: vec![TranscriptDetail {
                    kind: TranscriptDetailKind::Tool,
                    title: "new".to_owned(),
                    body: "target=tool:new".to_owned(),
                }],
                details_expanded: false,
                exec_calls: vec![],
            },
            revision: 1,
        });
        app.sync_detail_overlay_flags();

        let lines = app
            .detail_overlay_lines()
            .expect("detail overlay should keep the committed target")
            .into_iter()
            .map(|line| line.to_string())
            .collect::<Vec<_>>();

        assert!(lines.iter().any(|line| line.contains("Committed")));
        assert!(lines.iter().any(|line| line.contains("target=tool:old")));
        assert!(!lines.iter().any(|line| line.contains("target=tool:new")));
        assert_eq!(
            app.detail_overlay_target,
            Some(HistoryCellKey::Committed(committed_index))
        );
    }

    #[test]
    fn preserved_expandable_turn_prefers_active_turn_then_latest_committed_detail() {
        let mut session = interactive_session_record("demo", "gpt-5.4-mini");
        session.timeline.push(TimelineEntry {
            timestamp: "12:00".to_owned(),
            kind: TimelineKind::Agent,
            block: TranscriptBlock::AssistantMessage,
            actor: "assistant".to_owned(),
            title: "Committed".to_owned(),
            body: "done".to_owned(),
            run_id: Some("run-1".to_owned()),
            phase: Some(TurnPhase::Completed),
            details: vec![TranscriptDetail {
                kind: TranscriptDetailKind::Tool,
                title: "old".to_owned(),
                body: "target=tool:old".to_owned(),
            }],
            details_expanded: false,
            exec_calls: vec![],
        });

        let committed = preserved_expandable_turn(&session).expect("committed detail should exist");
        assert_eq!(committed.title, "Committed");

        session.active_turn = Some(ActiveTurn {
            cell: TimelineEntry {
                timestamp: "12:01".to_owned(),
                kind: TimelineKind::Agent,
                block: TranscriptBlock::AssistantMessage,
                actor: "assistant".to_owned(),
                title: "Active".to_owned(),
                body: "working".to_owned(),
                run_id: Some("run-2".to_owned()),
                phase: Some(TurnPhase::Streaming),
                details: vec![TranscriptDetail {
                    kind: TranscriptDetailKind::Tool,
                    title: "new".to_owned(),
                    body: "target=tool:new".to_owned(),
                }],
                details_expanded: false,
                exec_calls: vec![],
            },
            revision: 1,
        });

        let active = preserved_expandable_turn(&session).expect("active detail should exist");
        assert_eq!(active.title, "Active");
    }

    #[test]
    fn transcript_surface_view_shares_history_snapshot_between_chat_and_overlay() {
        let mut app = App::new("/tmp/mosaic".into());
        app.apply_run_event(RunEvent::ToolCalling {
            name: "read_file".to_owned(),
            call_id: "call-1".to_owned(),
            summary: Some("target=tool:read_file".to_owned()),
        });

        let surface = app.transcript_surface_view();
        assert_eq!(surface.chat.lines(), surface.transcript_overlay.lines);
    }

    #[test]
    fn shell_snapshot_reuses_single_transcript_surface() {
        let mut app = App::new("/tmp/mosaic".into());
        app.apply_run_event(RunEvent::ToolCalling {
            name: "read_file".to_owned(),
            call_id: "call-1".to_owned(),
            summary: Some("target=tool:read_file".to_owned()),
        });

        let snapshot = app.shell_snapshot();
        assert_eq!(
            snapshot.surface.chat.lines(),
            snapshot.surface.transcript_overlay.lines
        );
        assert_eq!(snapshot.overlays.state, app.overlay_state());
        assert_eq!(
            snapshot.overlays.transcript.lines,
            snapshot.surface.transcript_overlay.lines
        );
        assert!(
            snapshot
                .chrome
                .status_bar
                .header
                .to_string()
                .contains("Mosaic")
        );
        assert!(
            snapshot
                .chrome
                .composer
                .hint_line
                .to_string()
                .contains("/ commands")
        );
    }

    #[test]
    fn shell_snapshot_cursor_hidden_when_overlay_open() {
        let mut app = App::new("/tmp/mosaic".into());
        // Normal state: cursor should be visible.
        assert!(app.shell_snapshot().chrome.composer.cursor_visible);
        // Transcript overlay open: cursor must be hidden.
        app.transcript_overlay_open = true;
        assert!(!app.shell_snapshot().chrome.composer.cursor_visible);
        app.transcript_overlay_open = false;
        // Detail overlay open: cursor must also be hidden.
        app.detail_overlay_open = true;
        assert!(!app.shell_snapshot().chrome.composer.cursor_visible);
    }

    #[test]
    fn command_surface_view_keeps_popup_selection_and_completion_in_sync() {
        let mut app = App::new("/tmp/mosaic".into());
        app.active_session_mut().draft = "/he".to_owned();

        let surface = app.command_surface_view();

        assert_eq!(surface.query.as_deref(), Some("he"));
        assert_eq!(
            surface.selected.map(|command| command.command),
            Some("/help")
        );
        assert_eq!(
            surface.popup.selected.map(|command| command.command),
            Some("/help")
        );
        assert_eq!(surface.completion_suffix.as_deref(), Some("lp "));
    }

    #[test]
    fn active_turn_details_distinguish_mcp_and_sandbox_states() {
        let mut app = App::new("/tmp/mosaic".into());
        app.apply_run_event(RunEvent::ToolCalling {
            name: "remote_fetch".to_owned(),
            call_id: "call-1".to_owned(),
            summary: Some("source=mcp_server | mcp_startup=connecting".to_owned()),
        });
        app.apply_run_event(RunEvent::CapabilityJobQueued {
            job_id: "job-1".to_owned(),
            name: "sandbox-prepare".to_owned(),
            kind: "python".to_owned(),
            risk: "medium".to_owned(),
            permission_scopes: vec!["fs.read".to_owned()],
        });

        let details = &app
            .active_session()
            .active_turn
            .as_ref()
            .map(|turn| &turn.cell)
            .expect("active turn should exist")
            .details;
        assert_eq!(
            details.first().map(|detail| detail.kind),
            Some(TranscriptDetailKind::Mcp)
        );
        assert_eq!(
            details.last().map(|detail| detail.kind),
            Some(TranscriptDetailKind::Sandbox)
        );
    }

    #[test]
    fn session_list_and_switch_stay_inside_the_transcript_shell() {
        let mut app = App::new_with_resume("/tmp/mosaic".into(), true);
        app.active_session_mut().draft = "/session list".to_owned();
        let list_action = app.submit_composer();

        assert_eq!(list_action, AppAction::Continue);
        assert_eq!(
            app.active_session()
                .timeline
                .last()
                .map(|entry| entry.title.as_str()),
            Some("Session list")
        );

        let mut switch_app = App::new_with_resume("/tmp/mosaic".into(), true);
        switch_app.active_session_mut().draft = "/session switch sess-node-007".to_owned();
        let switch_action = switch_app.submit_composer();

        assert_eq!(switch_action, AppAction::Continue);
        assert_eq!(switch_app.session_label(), "sess-node-007");
        assert_eq!(
            switch_app
                .active_session()
                .timeline
                .last()
                .map(|entry| entry.title.as_str()),
            Some("Session selected")
        );
    }

    #[test]
    fn sync_session_catalog_preserves_cursor_pos_alongside_draft() {
        let mut app = App::new_interactive(
            "/tmp/mosaic".into(),
            "demo".to_owned(),
            "openai".to_owned(),
            "gpt-5.4-mini".to_owned(),
            Vec::new(),
            Vec::new(),
            false,
        );

        for ch in "hello".chars() {
            app.handle_key(KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE));
        }
        assert_eq!(app.active_session().cursor_pos, 5);

        // Simulate a catalog refresh: fresh record always has cursor_pos=0.
        let fresh = SessionRecord {
            id: "demo".to_owned(),
            title: "Demo session".to_owned(),
            origin: "Local".to_owned(),
            modified: "00:00".to_owned(),
            created: "00:00".to_owned(),
            channel: "control".to_owned(),
            actor: None,
            thread: None,
            route: "default".to_owned(),
            runtime: "agent-runtime".to_owned(),
            model: "gpt-5.4-mini".to_owned(),
            state: SessionState::Waiting,
            unread: 0,
            draft: String::new(),
            cursor_pos: 0,
            transcript_len: 0,
            current_run_id: None,
            current_gateway_run_id: None,
            last_gateway_run_id: None,
            memory_summary: None,
            compressed_context: None,
            references: Vec::new(),
            streaming_preview: None,
            streaming_run_id: None,
            active_turn: None,
            timeline: Vec::new(),
        };
        app.sync_session_catalog(vec![fresh], "demo");

        // Both draft and cursor_pos must be preserved through the catalog refresh.
        assert_eq!(app.active_draft(), "hello");
        assert_eq!(app.active_session().cursor_pos, 5);
    }

    #[test]
    fn sync_runtime_session_preserves_cursor_pos_across_gateway_refresh() {
        let mut app = App::new_interactive(
            "/tmp/mosaic".into(),
            "demo".to_owned(),
            "openai".to_owned(),
            "gpt-5.4-mini".to_owned(),
            Vec::new(),
            Vec::new(),
            false,
        );

        for ch in "hello".chars() {
            app.handle_key(KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE));
        }
        assert_eq!(app.active_session().cursor_pos, 5, "cursor must be at end after typing");

        // Simulate a gateway sync (empty transcript, like the first 200ms poll).
        let stored = StoredSessionRecord::new("demo", "Demo session", "openai", "agent", "gpt-5.4-mini");
        app.sync_runtime_session_with_origin(&stored, "Local");

        // Draft AND cursor_pos must survive the gateway sync.
        assert_eq!(app.active_draft(), "hello", "draft must survive gateway sync");
        assert_eq!(app.active_session().cursor_pos, 5, "cursor_pos must survive gateway sync");
    }

    #[test]
    fn push_timeline_sets_transcript_follow_flag() {
        let mut app = App::new("/tmp/mosaic".into());
        app.transcript.follow = false;
        app.transcript.scroll = 3;

        app.push_system_entry("test", "should trigger follow");

        assert!(
            app.transcript.follow,
            "transcript should follow after a new entry is pushed"
        );
    }

    #[test]
    fn submit_composer_sets_follow_instead_of_scrolling_to_top() {
        let mut app = App::new("/tmp/mosaic".into());
        app.active_session_mut().draft = "test message".to_owned();
        app.transcript.scroll = 10;
        app.transcript.follow = false;

        let _ = app.submit_composer();

        assert!(
            app.transcript.follow,
            "transcript must follow new content after submit, not reset to top"
        );
    }

    #[test]
    fn sync_session_catalog_same_session_preserves_scroll_and_follow() {
        let mut app = App::new_interactive(
            "/tmp/mosaic".into(),
            "demo".to_owned(),
            "openai".to_owned(),
            "gpt-5.4-mini".to_owned(),
            Vec::new(),
            Vec::new(),
            false,
        );
        let session_id = app.active_session().id.clone();

        // Simulate user having scrolled somewhere (follow=false means user scrolled up).
        app.transcript.scroll = 8;
        app.transcript.follow = false;

        // Simulate a catalog refresh with the same session still selected.
        app.sync_session_catalog(Vec::new(), &session_id);

        assert_eq!(
            app.transcript.scroll, 8,
            "scroll must be preserved when same session is re-selected"
        );
        assert!(
            !app.transcript.follow,
            "follow flag must be preserved when same session is re-selected"
        );
    }

    #[test]
    fn sync_session_catalog_session_switch_resets_scroll() {
        let mut app = App::new_interactive(
            "/tmp/mosaic".into(),
            "demo".to_owned(),
            "openai".to_owned(),
            "gpt-5.4-mini".to_owned(),
            Vec::new(),
            Vec::new(),
            false,
        );
        app.transcript.scroll = 8;
        app.transcript.follow = false;

        // Introduce a second session and switch to it.
        let other_session = interactive_session_record("other-session", "gpt-5.4-mini");
        app.sync_session_catalog(vec![other_session], "other-session");

        assert_eq!(
            app.transcript.scroll, 0,
            "scroll must reset to 0 when switching to a different session"
        );
    }
}
