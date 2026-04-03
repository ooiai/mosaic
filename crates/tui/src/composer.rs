use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

use crate::{app::ShellState, bottom_pane::InputMode};

pub struct ComposerView {
    pub draft: String,
    pub mode: InputMode,
    pub shell_state: ShellState,
    pub placeholder: String,
    pub completion_suffix: Option<String>,
    pub busy: bool,
    pub status_label: String,
    pub status_detail: String,
    pub enter_hint: String,
    pub escape_hint: String,
    pub spinner: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComposerChromeView {
    pub prompt_line: Line<'static>,
    pub status_line: Line<'static>,
    pub hint_line: Line<'static>,
    pub busy: bool,
    pub cursor_offset: u16,
    pub cursor_visible: bool,
}

impl ComposerView {
    pub fn into_chrome(self) -> ComposerChromeView {
        let busy_label = if self.busy {
            format!("{} {}", self.spinner, self.status_label)
        } else {
            self.status_label.clone()
        };
        let badge_style = match self.mode {
            InputMode::Chat => Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
            InputMode::Command => Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
            InputMode::Search => Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        };
        let badge_text = format!("[{}]", self.mode.label());
        let prompt_style = if self.draft.is_empty() {
            Style::default().fg(Color::DarkGray)
        } else {
            Style::default()
        };

        let mut prompt_spans = vec![
            Span::styled(
                "›",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" "),
            Span::styled(
                if self.draft.is_empty() {
                    self.placeholder.clone()
                } else {
                    self.draft.clone()
                },
                prompt_style,
            ),
        ];
        if !self.draft.is_empty()
            && let Some(suffix) = self.completion_suffix
        {
            prompt_spans.push(Span::styled(suffix, Style::default().fg(Color::DarkGray)));
        }

        let prompt_line = Line::from(prompt_spans);
        let status_line = Line::from(vec![
            Span::styled(badge_text, badge_style),
            Span::styled("  ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                self.shell_state.label(),
                shell_state_style(self.shell_state),
            ),
            Span::styled("  ·  ", Style::default().fg(Color::DarkGray)),
            Span::styled(busy_label, Style::default().fg(Color::Gray)),
            Span::styled("  ·  ", Style::default().fg(Color::DarkGray)),
            Span::styled(self.status_detail, Style::default().fg(Color::DarkGray)),
        ]);
        let hint_line = Line::from(vec![Span::styled(
            format!(
                "{}  ·  / commands  ·  Tab complete  ·  Ctrl+O detail  ·  Ctrl+T transcript  ·  Esc {}",
                self.enter_hint, self.escape_hint
            ),
            Style::default().fg(Color::DarkGray),
        )]);

        let cursor_offset = if self.draft.is_empty() {
            0
        } else {
            self.draft.chars().count() as u16
        };

        ComposerChromeView {
            prompt_line,
            status_line,
            hint_line,
            busy: self.busy,
            cursor_offset,
            cursor_visible: true,
        }
    }
}

pub struct ComposerWidget {
    paragraph: Paragraph<'static>,
    cursor_offset: u16,
    cursor_visible: bool,
}

impl ComposerWidget {
    pub fn new(view: ComposerView) -> Self {
        Self::from_chrome(view.into_chrome())
    }

    pub fn from_chrome(chrome: ComposerChromeView) -> Self {
        let paragraph =
            Paragraph::new(vec![
                chrome.prompt_line,
                chrome.status_line,
                chrome.hint_line,
            ])
            .block(Block::default().borders(Borders::TOP).border_style(
                if chrome.busy {
                    Style::default().fg(Color::Yellow)
                } else {
                    Style::default().fg(Color::DarkGray)
                },
            ));

        Self {
            paragraph,
            cursor_offset: chrome.cursor_offset,
            cursor_visible: chrome.cursor_visible,
        }
    }

    pub fn render(self, frame: &mut ratatui::Frame<'_>, area: Rect) {
        frame.render_widget(self.paragraph, area);

        if !self.cursor_visible {
            return;
        }

        let max_cursor_x = area.right().saturating_sub(2);
        let cursor_x = area
            .x
            .saturating_add(2)
            .saturating_add(self.cursor_offset)
            .min(max_cursor_x);
        let cursor_y = area.y.saturating_add(1);
        frame.set_cursor_position((cursor_x, cursor_y));
    }
}

fn shell_state_style(state: ShellState) -> Style {
    match state {
        ShellState::Idle => Style::default().fg(Color::DarkGray),
        ShellState::Composing => Style::default().fg(Color::Cyan),
        ShellState::Commanding => Style::default().fg(Color::Yellow),
        ShellState::Running => Style::default().fg(Color::Green),
        ShellState::TranscriptOverlay | ShellState::TurnDetailOverlay => {
            Style::default().fg(Color::Magenta)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::ComposerView;
    use crate::{app::ShellState, bottom_pane::InputMode};

    #[test]
    fn composer_chrome_preserves_completion_suffix_and_busy_state() {
        let chrome = ComposerView {
            draft: "/he".to_owned(),
            mode: InputMode::Command,
            shell_state: ShellState::Commanding,
            placeholder: "message".to_owned(),
            completion_suffix: Some("lp".to_owned()),
            busy: true,
            status_label: "busy".to_owned(),
            status_detail: "send disabled".to_owned(),
            enter_hint: "Enter run".to_owned(),
            escape_hint: "close".to_owned(),
            spinner: "|",
        }
        .into_chrome();

        assert_eq!(chrome.cursor_offset, 3);
        assert!(chrome.busy);
        assert!(chrome.prompt_line.to_string().contains("/help"));
        assert!(chrome.status_line.to_string().contains("busy"));
        assert!(chrome.hint_line.to_string().contains("Tab complete"));
        assert!(!chrome.hint_line.to_string().contains("Ctrl+C quit"));
    }
}
