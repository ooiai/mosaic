use std::path::PathBuf;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::mock;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppAction {
    Continue,
    Quit,
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

    pub fn previous(self) -> Self {
        match self {
            Self::Sessions => Self::Observability,
            Self::Timeline => Self::Sessions,
            Self::Composer => Self::Timeline,
            Self::Observability => Self::Composer,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Sessions => "sessions",
            Self::Timeline => "timeline",
            Self::Composer => "composer",
            Self::Observability => "observability",
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
    pub route: String,
    pub runtime: String,
    pub model: String,
    pub state: SessionState,
    pub unread: usize,
    pub draft: String,
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
    pub workspace_name: String,
    pub workspace_path: String,
    pub sessions: Vec<SessionRecord>,
    pub activity: Vec<ActivityEntry>,
    pub surface: Surface,
    pub selected_session: usize,
    pub resume_scope: ResumeScope,
    pub resume_query: String,
    pub resume_search: bool,
    pub focus: Focus,
    pub show_observability: bool,
    pub show_help_overlay: bool,
    pub timeline_scroll: u16,
    pub observability_scroll: u16,
    pub gateway_connected: bool,
    pub runtime_status: String,
    pub control_model: String,
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
            resume_scope: ResumeScope::All,
            resume_query: String::new(),
            resume_search: false,
            focus: Focus::Composer,
            show_observability: true,
            show_help_overlay: false,
            timeline_scroll: 0,
            observability_scroll: 0,
            gateway_connected: true,
            runtime_status: "warm".to_owned(),
            control_model: "gpt-5.4-control".to_owned(),
            heartbeat: 0,
        }
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
                self.focus = self.rewind_focus();
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
            _ => {
                self.handle_focus_key(key);
                AppAction::Continue
            }
        }
    }

    pub fn tick(&mut self) {
        self.heartbeat = self.heartbeat.wrapping_add(1);
    }

    pub fn heartbeat_symbol(&self) -> &'static str {
        const FRAMES: [&str; 4] = ["|", "/", "-", "\\"];
        FRAMES[self.heartbeat % FRAMES.len()]
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
        self.active_draft().strip_prefix('/')
    }

    pub fn visible_session_indices(&self) -> Vec<usize> {
        self.sessions
            .iter()
            .enumerate()
            .filter_map(|(index, session)| self.session_visible_in_resume(session).then_some(index))
            .collect()
    }

    fn handle_focus_key(&mut self, key: KeyEvent) {
        match self.focus {
            Focus::Sessions => self.handle_sessions_key(key.code),
            Focus::Timeline => self.handle_timeline_key(key.code),
            Focus::Composer => self.handle_composer_key(key),
            Focus::Observability => self.handle_observability_key(key.code),
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

    fn handle_composer_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Enter => self.submit_composer(),
            KeyCode::Backspace => {
                self.active_session_mut().draft.pop();
            }
            KeyCode::Char(character)
                if !key.modifiers.contains(KeyModifiers::CONTROL)
                    && !key.modifiers.contains(KeyModifiers::ALT) =>
            {
                self.active_session_mut().draft.push(character);
            }
            _ => {}
        }
    }

    fn submit_composer(&mut self) {
        let message = self.active_draft().trim().to_owned();
        if message.is_empty() {
            return;
        }

        if let Some(command) = message.strip_prefix('/') {
            self.route_command(command.trim());
        } else {
            self.queue_operator_instruction(&message);
        }

        self.active_session_mut().draft.clear();
        self.timeline_scroll = 0;
    }

    fn queue_operator_instruction(&mut self, message: &str) {
        let session_label = self.session_label().to_owned();

        self.active_session_mut().timeline.push(TimelineEntry {
            timestamp: "now".to_owned(),
            kind: TimelineKind::Operator,
            actor: "operator".to_owned(),
            title: "Queued operator instruction".to_owned(),
            body: message.to_owned(),
        });

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
        self.resume_scope = ResumeScope::All;
        self.ensure_selected_session_visible();
    }

    fn rewind_focus(&self) -> Focus {
        let previous = self.focus.previous();
        if !self.show_observability && previous == Focus::Observability {
            previous.previous()
        } else {
            previous
        }
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
            "runtime" => {
                let status = parts.collect::<Vec<_>>().join(" ");
                self.set_runtime_status(status.trim());
            }
            "session" => self.route_session_command(parts.collect()),
            _ => self.push_command_error(format!("Unknown command: /{name}")),
        }
    }

    fn route_gateway_command(&mut self, action: Option<&str>) {
        match action {
            Some("connect") => {
                self.gateway_connected = true;
                self.push_activity("gateway", "Gateway marked connected in the local TUI.");
                self.push_system_entry(
                    "Gateway link updated",
                    "Local mock transport was marked connected by operator command.",
                );
            }
            Some("disconnect") => {
                self.gateway_connected = false;
                self.push_activity("gateway", "Gateway marked disconnected in the local TUI.");
                self.push_system_entry(
                    "Gateway link updated",
                    "Local mock transport was marked disconnected by operator command.",
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
            "Available commands:\n/help\n/logs\n/gateway connect\n/gateway disconnect\n/runtime <status>\n/session state <active|waiting|degraded>\n/session model <name>",
        );
    }

    fn push_command_error(&mut self, message: impl Into<String>) {
        let message = message.into();
        self.push_activity("command", format!("Rejected command: {message}"));
        self.push_system_entry("Command rejected", message);
    }

    fn push_system_entry(&mut self, title: impl Into<String>, body: impl Into<String>) {
        self.active_session_mut().timeline.push(TimelineEntry {
            timestamp: "now".to_owned(),
            kind: TimelineKind::System,
            actor: "control-plane".to_owned(),
            title: title.into(),
            body: body.into(),
        });
    }

    fn push_activity(&mut self, scope: impl Into<String>, message: impl Into<String>) {
        self.activity.push(ActivityEntry {
            timestamp: "now".to_owned(),
            scope: scope.into(),
            message: message.into(),
        });
    }
}

#[cfg(test)]
mod tests {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

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
}
