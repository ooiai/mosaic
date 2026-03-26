use std::path::PathBuf;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use mosaic_runtime::events::RunEvent;
use mosaic_session_core::{
    SessionRecord as StoredSessionRecord, TranscriptMessage, TranscriptRole, session_route_for_id,
};

use crate::mock;

pub const LOCAL_COMMANDS: [(&str, &str); 8] = [
    ("/help", "Show local control command reference"),
    ("/logs", "Toggle activity feed visibility"),
    (
        "/gateway connect",
        "Resume gateway refresh and event streaming",
    ),
    (
        "/gateway disconnect",
        "Pause gateway refresh and event streaming",
    ),
    ("/runtime <status>", "Set the control runtime status label"),
    (
        "/session state|model",
        "Update the selected session state or model label",
    ),
    ("/model list", "Show available runtime profiles"),
    (
        "/model use <profile>",
        "Switch the real runtime profile for next turns",
    ),
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppAction {
    Continue,
    Quit,
    Submit(String),
    GatewayConnect,
    GatewayDisconnect,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppMode {
    Mock,
    Interactive,
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
        let state = match self.runtime_status.as_str() {
            "running" => SessionState::Active,
            "error" => SessionState::Degraded,
            _ => SessionState::Waiting,
        };

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
            RunEvent::RunStarted { input } => {
                self.runtime_status = "running".to_owned();
                self.push_activity("runtime", "Run started");
                if self.is_interactive() {
                    return;
                }
                self.push_timeline(
                    TimelineKind::System,
                    "runtime",
                    "Run started",
                    &format!("Input: {}", truncate_for_timeline(&input, 180)),
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
            RunEvent::FinalAnswerReady => {
                self.push_activity("runtime", "Final answer ready");
                if self.is_interactive() {
                    return;
                }
                self.push_timeline(
                    TimelineKind::Agent,
                    "runtime",
                    "Final answer ready",
                    "Assistant output is ready to present.",
                );
            }
            RunEvent::RunFinished { output_preview } => {
                self.runtime_status = "idle".to_owned();
                self.push_activity("runtime", "Run finished");
                if self.is_interactive() {
                    return;
                }
                self.push_timeline(
                    TimelineKind::Agent,
                    "runtime",
                    "Run finished",
                    &format!(
                        "Output preview: {}",
                        truncate_for_timeline(&output_preview, 180)
                    ),
                );
            }
            RunEvent::RunFailed { error } => {
                self.runtime_status = "error".to_owned();
                self.push_activity("runtime", "Run failed");
                if self.is_interactive() {
                    return;
                }
                self.push_timeline(
                    TimelineKind::System,
                    "runtime",
                    "Run failed",
                    &truncate_for_timeline(&error, 180),
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

    pub fn command_query(&self) -> Option<&str> {
        let query = self.active_draft().strip_prefix('/')?;
        let end = query.find(char::is_whitespace).unwrap_or(query.len());
        Some(&query[..end])
    }

    pub fn command_matches(&self) -> Vec<(&'static str, &'static str)> {
        matching_commands(self.command_query().unwrap_or_default())
    }

    pub fn selected_command_match(&self) -> Option<(&'static str, &'static str)> {
        let matches = self.command_matches();
        matches
            .get(self.command_menu_index.min(matches.len().saturating_sub(1)))
            .copied()
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
                self.surface = Surface::Console;
                self.show_console_history = true;
                self.focus = Focus::Composer;
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
            let command = command.trim();
            let external_action = match command {
                "gateway connect" => Some(AppAction::GatewayConnect),
                "gateway disconnect" => Some(AppAction::GatewayDisconnect),
                _ => None,
            };

            self.route_command(command);
            external_action.unwrap_or(AppAction::Continue)
        } else if self.is_interactive() {
            if self.runtime_status == "running" {
                self.push_command_error("A run is already in progress for this session");
                AppAction::Continue
            } else {
                self.push_activity(
                    "composer",
                    format!("Submitted message to session {}", self.session_label()),
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
        self.resume_scope = ResumeScope::Local;
        self.ensure_selected_session_visible();
    }

    fn command_menu_active(&self) -> bool {
        self.focus == Focus::Composer && self.command_query().is_some()
    }

    fn command_menu_should_complete(&self) -> bool {
        self.command_menu_active()
            && self.active_draft().starts_with('/')
            && !self.active_draft()[1..].chars().any(char::is_whitespace)
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
        let Some((command, _)) = self.selected_command_match() else {
            return;
        };

        self.active_session_mut().draft = format!("{command} ");
        self.command_menu_index = 0;
    }

    fn route_command(&mut self, command: &str) {
        let mut parts = command.split_whitespace();
        let Some(name) = parts.next() else {
            self.push_command_error("Usage: /help");
            return;
        };

        match name {
            "help" => self.push_help(),
            "logs" => self.toggle_logs(),
            "gateway" => self.route_gateway_command(parts.next()),
            "model" => self.route_model_command(parts.collect()),
            "runtime" => {
                let status = parts.collect::<Vec<_>>().join(" ");
                self.set_runtime_status(status.trim());
            }
            "session" => self.route_session_command(parts.collect()),
            _ => self.push_command_error(format!("Unknown command: /{name}")),
        }
    }

    fn route_model_command(&mut self, args: Vec<&str>) {
        match args.as_slice() {
            ["list"] => {
                if self.available_profiles.is_empty() {
                    self.push_command_error(
                        "No runtime profiles are available in this TUI session",
                    );
                    return;
                }

                let profiles = self
                    .available_profiles
                    .iter()
                    .map(|profile| {
                        if profile.name == self.selected_profile {
                            format!("* {} ({})", profile.name, profile.model)
                        } else {
                            format!("- {} ({})", profile.name, profile.model)
                        }
                    })
                    .collect::<Vec<_>>()
                    .join("\n");

                self.push_activity("model", "Listed available runtime profiles.");
                self.push_system_entry("Runtime profiles", profiles);
            }
            ["use", rest @ ..] if !rest.is_empty() => self.set_active_profile(&rest.join(" ")),
            _ => self.push_command_error("Usage: /model list | /model use <profile>"),
        }
    }

    fn route_gateway_command(&mut self, action: Option<&str>) {
        match action {
            Some("connect") => {
                self.gateway_connected = true;
                self.push_activity("gateway", "Gateway link marked connected in the TUI.");
                self.push_system_entry(
                    "Gateway link updated",
                    "Gateway refresh and event streaming were resumed by operator command.",
                );
            }
            Some("disconnect") => {
                self.gateway_connected = false;
                self.push_activity("gateway", "Gateway link marked disconnected in the TUI.");
                self.push_system_entry(
                    "Gateway link updated",
                    "Gateway refresh and event streaming were paused by operator command.",
                );
            }
            _ => self.push_command_error("Usage: /gateway connect|disconnect"),
        }
    }

    fn set_runtime_status(&mut self, status: &str) {
        if status.is_empty() {
            self.push_command_error("Usage: /runtime <status>");
            return;
        }

        self.runtime_status = status.to_owned();
        self.push_activity("runtime", format!("Runtime status set to {status}."));
        self.push_system_entry(
            "Runtime status updated",
            format!("Control-plane runtime status is now {status}."),
        );
    }

    fn route_session_command(&mut self, args: Vec<&str>) {
        match args.as_slice() {
            ["state", "active"] => self.set_session_state(SessionState::Active),
            ["state", "waiting"] => self.set_session_state(SessionState::Waiting),
            ["state", "degraded"] => self.set_session_state(SessionState::Degraded),
            ["model", rest @ ..] if !rest.is_empty() => self.set_session_model(&rest.join(" ")),
            _ => self.push_command_error(
                "Usage: /session state <active|waiting|degraded> | /session model <name>",
            ),
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
            format!("Selected session now targets {model}."),
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
        self.active_session_mut().model = option.model.clone();
        self.push_activity(
            "model",
            format!("Interactive runtime profile switched to {}.", option.name),
        );
        self.push_system_entry(
            "Runtime profile updated",
            format!(
                "Future turns will use profile {} (type={}, model={}).",
                option.name, option.provider_type, option.model
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
        self.push_activity("command", "Displayed local control command reference.");
        self.push_system_entry(
            "Local command reference",
            "Available commands:\n/help\n/logs\n/gateway connect\n/gateway disconnect\n/runtime <status>\n/session state <active|waiting|degraded>\n/session model <name>\n/model list\n/model use <profile>",
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

pub fn matching_commands(query: &str) -> Vec<(&'static str, &'static str)> {
    let trimmed = query.trim().trim_start_matches('/').to_ascii_lowercase();
    LOCAL_COMMANDS
        .into_iter()
        .filter(|(command, _)| {
            if trimmed.is_empty() {
                return true;
            }

            let searchable = command.trim_start_matches('/').to_ascii_lowercase();
            searchable.starts_with(&trimmed)
                || searchable
                    .split_whitespace()
                    .any(|token| token.starts_with(&trimmed))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use mosaic_runtime::events::RunEvent;

    use super::{App, AppAction, Focus, ResumeScope, SessionState, Surface, TimelineKind};

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
            error: "boom".to_owned(),
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
