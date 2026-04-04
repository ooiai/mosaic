use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

use crate::{app::ShellState, bottom_pane::InputMode};

pub struct ComposerView {
    pub draft: String,
    pub cursor_pos: usize,
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
    pub busy: bool,
    pub cursor_pos: usize,
    pub cursor_visible: bool,
    pub draft_text: String,
    pub placeholder: String,
    pub completion_suffix: Option<String>,
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
                "❯",
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
            && let Some(ref suffix) = self.completion_suffix
        {
            prompt_spans.push(Span::styled(
                suffix.clone(),
                Style::default().fg(Color::DarkGray),
            ));
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
            Span::styled("   / commands  ·  Esc ", Style::default().fg(Color::DarkGray)),
            Span::styled(self.escape_hint, Style::default().fg(Color::DarkGray)),
        ]);

        ComposerChromeView {
            prompt_line,
            status_line,
            busy: self.busy,
            cursor_pos: self.cursor_pos,
            cursor_visible: true,
            draft_text: self.draft,
            placeholder: self.placeholder,
            completion_suffix: self.completion_suffix,
        }
    }
}

pub struct ComposerWidget {
    status_line: Line<'static>,
    draft_text: String,
    placeholder: String,
    completion_suffix: Option<String>,
    cursor_pos: usize,
    cursor_visible: bool,
    busy: bool,
}

impl ComposerWidget {
    pub fn new(view: ComposerView) -> Self {
        Self::from_chrome(view.into_chrome())
    }

    pub fn from_chrome(chrome: ComposerChromeView) -> Self {
        Self {
            status_line: chrome.status_line,
            draft_text: chrome.draft_text,
            placeholder: chrome.placeholder,
            completion_suffix: chrome.completion_suffix,
            cursor_pos: chrome.cursor_pos,
            cursor_visible: chrome.cursor_visible,
            busy: chrome.busy,
        }
    }

    pub fn render(self, frame: &mut ratatui::Frame<'_>, area: Rect) {
        // "❯ " prefix is 2 chars; leave 2 chars margin from right edge.
        let prefix_len: usize = 2;
        let visible_width = (area.width as usize).saturating_sub(prefix_len + 2).max(1);

        // Compute horizontal scroll so cursor is always in view.
        let draft_len = self.draft_text.chars().count();
        let cursor_pos = self.cursor_pos.min(draft_len);
        let scroll = if cursor_pos >= visible_width {
            cursor_pos - visible_width + 1
        } else {
            0
        };

        // Build the visible portion of the draft.
        let (prompt_text, prompt_style) = if self.draft_text.is_empty() {
            (self.placeholder, Style::default().fg(Color::DarkGray))
        } else {
            let visible: String = self
                .draft_text
                .chars()
                .skip(scroll)
                .take(visible_width)
                .collect();
            (visible, Style::default())
        };

        let mut prompt_spans = vec![
            Span::styled(
                "❯",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" "),
            Span::styled(prompt_text, prompt_style),
        ];
        // Only show completion suffix when cursor is at end of draft and no scroll offset.
        if !self.draft_text.is_empty() && scroll == 0 && cursor_pos == draft_len {
            if let Some(suffix) = self.completion_suffix {
                prompt_spans.push(Span::styled(suffix, Style::default().fg(Color::DarkGray)));
            }
        }

        let paragraph = Paragraph::new(vec![
            Line::from(prompt_spans),
            self.status_line,
        ])
        .block(Block::default().borders(Borders::TOP).border_style(
            if self.busy {
                Style::default().fg(Color::Yellow)
            } else {
                Style::default().fg(Color::DarkGray)
            },
        ));

        frame.render_widget(paragraph, area);

        if !self.cursor_visible {
            return;
        }

        let max_cursor_x = area.right().saturating_sub(2);
        let cursor_x = area
            .x
            .saturating_add(prefix_len as u16)
            .saturating_add((cursor_pos - scroll) as u16)
            .min(max_cursor_x);
        // area.y is the TOP border row; content starts at area.y + 1.
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
            cursor_pos: 3,
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

        assert_eq!(chrome.cursor_pos, 3);
        assert!(chrome.busy);
        assert!(chrome.prompt_line.to_string().contains("/help"));
        assert!(chrome.status_line.to_string().contains("busy"));
        assert!(chrome.status_line.to_string().contains("/ commands"));
        assert!(!chrome.status_line.to_string().contains("Ctrl+C quit"));
    }
}
