use std::path::PathBuf;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::mock;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppAction {
    Continue,
    Quit,
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
    pub channel: String,
    pub route: String,
    pub runtime: String,
    pub model: String,
    pub state: SessionState,
    pub unread: usize,
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
    pub selected_session: usize,
    pub focus: Focus,
    pub composer: String,
    pub show_observability: bool,
    pub timeline_scroll: u16,
    pub observability_scroll: u16,
    pub gateway_connected: bool,
    pub runtime_status: String,
    pub control_model: String,
    heartbeat: usize,
}

impl App {
    pub fn new(workspace_path: PathBuf) -> Self {
        let workspace_name = workspace_path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("workspace")
            .to_owned();

        Self {
            workspace_name,
            workspace_path: workspace_path.display().to_string(),
            sessions: mock::sessions(),
            activity: mock::activity_feed(),
            selected_session: 0,
            focus: Focus::Sessions,
            composer: String::new(),
            show_observability: true,
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

    fn handle_focus_key(&mut self, key: KeyEvent) {
        match self.focus {
            Focus::Sessions => self.handle_sessions_key(key.code),
            Focus::Timeline => self.handle_timeline_key(key.code),
            Focus::Composer => self.handle_composer_key(key),
            Focus::Observability => self.handle_observability_key(key.code),
        }
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
                self.composer.pop();
            }
            KeyCode::Char(character)
                if !key.modifiers.contains(KeyModifiers::CONTROL)
                    && !key.modifiers.contains(KeyModifiers::ALT) =>
            {
                self.composer.push(character);
            }
            _ => {}
        }
    }

    fn submit_composer(&mut self) {
        let message = self.composer.trim().to_owned();
        if message.is_empty() {
            return;
        }

        self.active_session_mut().timeline.push(TimelineEntry {
            timestamp: "now".to_owned(),
            kind: TimelineKind::Operator,
            actor: "operator".to_owned(),
            title: "Queued operator instruction".to_owned(),
            body: message.clone(),
        });

        self.activity.push(ActivityEntry {
            timestamp: "now".to_owned(),
            scope: "composer".to_owned(),
            message: format!("Buffered command for {}", self.session_label()),
        });

        self.composer.clear();
        self.timeline_scroll = 0;
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

    fn advance_focus(&self) -> Focus {
        let next = self.focus.next();
        if !self.show_observability && next == Focus::Observability {
            next.next()
        } else {
            next
        }
    }

    fn rewind_focus(&self) -> Focus {
        let previous = self.focus.previous();
        if !self.show_observability && previous == Focus::Observability {
            previous.previous()
        } else {
            previous
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{App, Focus, TimelineKind};

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
        app.composer = "Trace gateway route".to_owned();

        app.submit_composer();

        assert_eq!(app.composer, "");
        assert_eq!(app.active_session().timeline.len(), initial_len + 1);
        assert_eq!(
            app.active_session().timeline.last().map(|entry| entry.kind),
            Some(TimelineKind::Operator)
        );
    }
}
