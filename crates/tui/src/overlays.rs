use crate::command_popup::{CommandPopupView, CommandPopupWidget, command_popup_rect};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OverlayState {
    #[default]
    None,
    CommandPalette,
    Transcript,
    TurnDetail,
}

impl OverlayState {
    pub fn from_shell_state(query: Option<&str>, transcript_open: bool, detail_open: bool) -> Self {
        if query.is_some() {
            Self::CommandPalette
        } else if transcript_open {
            Self::Transcript
        } else if detail_open {
            Self::TurnDetail
        } else {
            Self::None
        }
    }

    pub fn is_command_palette(self) -> bool {
        matches!(self, Self::CommandPalette)
    }

    pub fn is_turn_detail(self) -> bool {
        matches!(self, Self::TurnDetail)
    }

    pub fn is_transcript(self) -> bool {
        matches!(self, Self::Transcript)
    }
}

use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TurnDetailOverlayView {
    pub title: String,
    pub lines: Vec<Line<'static>>,
    pub scroll: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TranscriptOverlayView {
    pub lines: Vec<Line<'static>>,
    pub scroll: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OverlayStackView {
    pub state: OverlayState,
    pub command_popup: CommandPopupView,
    pub transcript: TranscriptOverlayView,
    pub turn_detail: Option<TurnDetailOverlayView>,
}

pub struct TurnDetailOverlayWidget {
    title: String,
    lines: Vec<Line<'static>>,
    scroll: u16,
}

impl TurnDetailOverlayWidget {
    pub fn new(view: TurnDetailOverlayView) -> Self {
        Self {
            title: view.title,
            lines: view.lines,
            scroll: view.scroll,
        }
    }

    pub fn paragraph(&self) -> Paragraph<'static> {
        Paragraph::new(self.lines.clone())
            .block(
                Block::default()
                    .title(format!(" Turn detail · {} ", self.title))
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Gray)),
            )
            .scroll((self.scroll, 0))
            .wrap(Wrap { trim: false })
    }
}

pub struct TranscriptOverlayWidget {
    lines: Vec<Line<'static>>,
    scroll: u16,
}

impl TranscriptOverlayWidget {
    pub fn new(view: TranscriptOverlayView) -> Self {
        let mut lines = view.lines;
        lines.push(Line::from(vec![Span::styled(
            "Esc closes  ·  Ctrl+T toggles transcript view",
            Style::default().fg(Color::DarkGray),
        )]));
        Self {
            lines,
            scroll: view.scroll,
        }
    }

    pub fn paragraph(&self) -> Paragraph<'static> {
        Paragraph::new(self.lines.clone())
            .block(
                Block::default()
                    .title(" Transcript ")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Gray)),
            )
            .scroll((self.scroll, 0))
            .wrap(Wrap { trim: false })
    }
}

pub struct OverlayStack {
    command_popup: Option<(Rect, Paragraph<'static>)>,
    transcript_overlay: Option<(Rect, Paragraph<'static>)>,
    turn_detail_overlay: Option<(Rect, Paragraph<'static>)>,
}

impl OverlayStack {
    pub fn from_view(view: OverlayStackView, frame_area: Rect, composer_area: Rect) -> Self {
        let command_popup = if view.state.is_command_palette() {
            let widget = CommandPopupWidget::new(view.command_popup);
            let popup = command_popup_rect(frame_area, composer_area, widget.height());
            Some((popup, widget.paragraph()))
        } else {
            None
        };
        let transcript_overlay = if view.state.is_transcript() {
            Some((
                transcript_overlay_rect(frame_area),
                TranscriptOverlayWidget::new(view.transcript).paragraph(),
            ))
        } else {
            None
        };
        let turn_detail_overlay = if view.state.is_turn_detail() {
            view.turn_detail
                .map(TurnDetailOverlayWidget::new)
                .map(|widget| (turn_detail_rect(frame_area), widget.paragraph()))
        } else {
            None
        };

        Self {
            command_popup,
            transcript_overlay,
            turn_detail_overlay,
        }
    }

    pub fn render(self, frame: &mut Frame<'_>) {
        if let Some((area, widget)) = self.command_popup {
            frame.render_widget(Clear, area);
            frame.render_widget(widget, area);
        } else if let Some((area, widget)) = self.transcript_overlay {
            frame.render_widget(Clear, area);
            frame.render_widget(widget, area);
        } else if let Some((area, widget)) = self.turn_detail_overlay {
            frame.render_widget(Clear, area);
            frame.render_widget(widget, area);
        }
    }
}

pub fn turn_detail_rect(area: Rect) -> Rect {
    let width = area.width.saturating_sub(12).clamp(48, 110);
    let height = area.height.saturating_sub(8).clamp(12, 28);
    Rect {
        x: area.x + (area.width.saturating_sub(width)) / 2,
        y: area.y + (area.height.saturating_sub(height)) / 2,
        width,
        height,
    }
}

pub fn transcript_overlay_rect(area: Rect) -> Rect {
    let width = area.width.saturating_sub(8).clamp(52, 132);
    let height = area.height.saturating_sub(4).clamp(14, 36);
    Rect {
        x: area.x + (area.width.saturating_sub(width)) / 2,
        y: area.y + (area.height.saturating_sub(height)) / 2,
        width,
        height,
    }
}

#[cfg(test)]
mod tests {
    use super::{OverlayStack, OverlayStackView, OverlayState, TranscriptOverlayView};
    use crate::command_popup::CommandPopupView;

    #[test]
    fn overlay_stack_from_view_prefers_transcript_when_requested() {
        let view = OverlayStackView {
            state: OverlayState::Transcript,
            command_popup: CommandPopupView::from_query("", 0),
            transcript: TranscriptOverlayView {
                lines: vec![],
                scroll: 0,
            },
            turn_detail: None,
        };

        let stack = OverlayStack::from_view(
            view,
            ratatui::layout::Rect::new(0, 0, 100, 40),
            ratatui::layout::Rect::new(0, 30, 100, 4),
        );

        assert!(stack.command_popup.is_none());
        assert!(stack.transcript_overlay.is_some());
        assert!(stack.turn_detail_overlay.is_none());
    }
}
