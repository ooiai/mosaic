use mosaic_agent::AgentEvent;
use mosaic_core::error::Result;
use mosaic_core::session::{EventKind, SessionEvent, SessionStore, SessionSummary};

use crate::{
    TuiAgentOption, TuiFocus, TuiLocalCommand, TuiLocalCommandOutput, short_id, summarize_json,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ChatLine {
    pub(crate) role: String,
    pub(crate) text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct InspectorLine {
    pub(crate) kind: String,
    pub(crate) detail: String,
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

#[derive(Debug, Clone)]
pub struct TuiState {
    pub(crate) focus: TuiFocus,
    pub(crate) startup_visible: bool,
    pub(crate) show_inspector: bool,
    pub(crate) show_help: bool,
    pub(crate) show_agent_picker: bool,
    pub(crate) show_session_picker: bool,
    pub(crate) input: String,
    pub(crate) command_palette_index: usize,
    pub(crate) running: bool,
    pub(crate) status: String,
    pub(crate) sessions: Vec<SessionSummary>,
    pub(crate) selected_session: usize,
    pub(crate) agents: Vec<TuiAgentOption>,
    pub(crate) selected_agent: usize,
    pub(crate) active_session_id: Option<String>,
    pub(crate) messages: Vec<ChatLine>,
    pub(crate) inspector: Vec<InspectorLine>,
}

impl TuiState {
    pub(crate) fn new(
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
            startup_visible: true,
            show_inspector,
            show_help: false,
            show_agent_picker: false,
            show_session_picker: false,
            input: String::new(),
            command_palette_index: 0,
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

    pub(crate) fn reduce(&mut self, action: TuiAction) {
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
                self.startup_visible = true;
                self.command_palette_index = 0;
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

    pub(crate) fn refresh_sessions(&mut self, store: &SessionStore) -> Result<()> {
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

    pub(crate) fn load_session(
        &mut self,
        store: &SessionStore,
        session_id: Option<String>,
    ) -> Result<()> {
        self.active_session_id = session_id;
        self.messages.clear();
        self.inspector.clear();
        self.command_palette_index = 0;
        if let Some(id) = self.active_session_id.clone() {
            let events = store.read_events(&id)?;
            self.apply_persisted_events(&events);
            self.status = format!("resumed session={id}");
        } else {
            self.status = "new session".to_string();
        }
        Ok(())
    }

    pub(crate) fn dismiss_startup_surface(&mut self) {
        self.startup_visible = false;
    }

    pub(crate) fn show_startup_surface(&self) -> bool {
        self.startup_visible && self.focus == TuiFocus::Input
    }

    pub(crate) fn reset_command_palette_selection(&mut self) {
        self.command_palette_index = 0;
    }

    pub(crate) fn select_next_command(&mut self, item_count: usize) {
        if item_count == 0 {
            self.command_palette_index = 0;
            return;
        }
        self.command_palette_index = (self.command_palette_index + 1) % item_count;
    }

    pub(crate) fn select_prev_command(&mut self, item_count: usize) {
        if item_count == 0 {
            self.command_palette_index = 0;
            return;
        }
        if self.command_palette_index == 0 {
            self.command_palette_index = item_count - 1;
        } else {
            self.command_palette_index -= 1;
        }
    }

    pub(crate) fn apply_local_command_output(
        &mut self,
        command: TuiLocalCommand,
        output: TuiLocalCommandOutput,
    ) {
        self.dismiss_startup_surface();
        self.messages.push(ChatLine {
            role: "user".to_string(),
            text: command.slash_name().to_string(),
        });
        self.messages.push(ChatLine {
            role: "system".to_string(),
            text: if output.body.trim().is_empty() {
                output.title
            } else {
                format!("{}\n{}", output.title, output.body)
            },
        });
        self.push_inspector_line("local_command", output.inspector_detail);
        self.status = output.status;
    }

    pub(crate) fn apply_local_command_error(
        &mut self,
        command: TuiLocalCommand,
        error: impl Into<String>,
    ) {
        let error = error.into();
        self.dismiss_startup_surface();
        self.messages.push(ChatLine {
            role: "user".to_string(),
            text: command.slash_name().to_string(),
        });
        self.messages.push(ChatLine {
            role: "system".to_string(),
            text: format!("{} failed\n{}", command.slash_name(), error),
        });
        self.push_inspector_line(
            "error",
            format!("local command {} failed: {}", command.slash_name(), error),
        );
        self.status = format!("error: {error}");
    }

    pub(crate) fn push_inspector_line(
        &mut self,
        kind: impl Into<String>,
        detail: impl Into<String>,
    ) {
        self.inspector.push(InspectorLine {
            kind: kind.into(),
            detail: detail.into(),
        });
        self.trim_inspector();
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
                    self.push_inspector_line(
                        "tool_call",
                        format!("{name} {}", summarize_json(event.payload.get("args"))),
                    );
                }
                EventKind::ToolResult => {
                    let name = event
                        .payload
                        .get("name")
                        .and_then(|value| value.as_str())
                        .unwrap_or("tool");
                    self.push_inspector_line(
                        "tool_result",
                        format!("{name} {}", summarize_json(event.payload.get("result"))),
                    );
                }
                EventKind::Error => {
                    self.push_inspector_line("error", summarize_json(event.payload.get("message")));
                }
                EventKind::System => {}
            }
        }
    }

    pub(crate) fn apply_agent_event(&mut self, event: AgentEvent) {
        self.startup_visible = false;
        self.command_palette_index = 0;
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
                self.push_inspector_line(
                    "tool_call",
                    format!(
                        "[{}] {name} {}",
                        short_id(&session_id),
                        summarize_json(Some(&args))
                    ),
                );
                self.status = format!("running tool={name}");
            }
            AgentEvent::ToolResult {
                session_id,
                name,
                result,
            } => {
                self.active_session_id = Some(session_id);
                self.push_inspector_line(
                    "tool_result",
                    format!(
                        "[{}] {name} {}",
                        short_id(self.active_session_id.as_deref().unwrap_or_default()),
                        summarize_json(Some(&result))
                    ),
                );
            }
            AgentEvent::Error {
                session_id,
                message,
            } => {
                self.active_session_id = Some(session_id);
                self.push_inspector_line(
                    "error",
                    format!(
                        "[{}] {message}",
                        short_id(self.active_session_id.as_deref().unwrap_or_default())
                    ),
                );
            }
        }
    }

    fn trim_inspector(&mut self) {
        const MAX_INSPECTOR: usize = 200;
        if self.inspector.len() > MAX_INSPECTOR {
            let overflow = self.inspector.len() - MAX_INSPECTOR;
            self.inspector.drain(0..overflow);
        }
    }
}
