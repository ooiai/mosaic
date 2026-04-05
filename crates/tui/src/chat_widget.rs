use ratatui::{
    Frame,
    layout::Rect,
    text::Line,
    widgets::{Paragraph, Wrap},
};

use crate::{
    history_cell::{HistoryCell, HistoryCells, build_history_cells},
    overlays::{TranscriptOverlayView, TurnDetailOverlayView},
    transcript::TranscriptView,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatView {
    pub transcript_key: (usize, Option<usize>, bool),
    pub committed_cells: Vec<HistoryCell>,
    pub active_cell: Option<HistoryCell>,
    pub streaming_preview: Option<Vec<Line<'static>>>,
    pub scroll: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TranscriptSurfaceView {
    pub chat: ChatView,
    pub transcript_overlay: TranscriptOverlayView,
    pub turn_detail_overlay: Option<TurnDetailOverlayView>,
}

impl ChatView {
    pub fn new(cells: HistoryCells, scroll: u16) -> Self {
        let transcript_key = cells.transcript_key();
        Self {
            transcript_key,
            committed_cells: cells.committed,
            active_cell: cells.active,
            streaming_preview: cells.streaming_preview,
            scroll,
        }
    }

    pub fn lines(&self) -> Vec<Line<'static>> {
        self.lines_at_width(None)
    }

    /// Produce display lines using width-adaptive rendering when `width` is given.
    pub fn lines_at_width(&self, width: Option<u16>) -> Vec<Line<'static>> {
        let mut lines = Vec::new();
        for cell in &self.committed_cells {
            if let Some(w) = width {
                lines.extend(cell.render_lines(w));
            } else {
                lines.extend(cell.summary_lines.clone());
            }
        }
        if let Some(active) = &self.active_cell {
            if let Some(w) = width {
                lines.extend(active.render_lines(w));
            } else {
                lines.extend(active.summary_lines.clone());
            }
        }
        if self.active_cell.is_none()
            && let Some(preview) = &self.streaming_preview
        {
            lines.extend(preview.clone());
        }
        lines
    }

    pub fn has_active_cell(&self) -> bool {
        self.active_cell.is_some()
    }
}

impl TranscriptSurfaceView {
    pub fn new(
        cells: HistoryCells,
        chat_scroll: u16,
        transcript_overlay_scroll: u16,
        turn_detail_overlay: Option<TurnDetailOverlayView>,
    ) -> Self {
        let transcript_lines = cells.summary_lines();
        Self {
            chat: ChatView::new(cells, chat_scroll),
            transcript_overlay: TranscriptOverlayView {
                lines: transcript_lines,
                scroll: transcript_overlay_scroll,
            },
            turn_detail_overlay,
        }
    }
}

pub struct ChatWidget {
    view: ChatView,
}

impl ChatWidget {
    pub fn new(view: ChatView) -> Self {
        Self { view }
    }

    pub fn lines(&self) -> Vec<Line<'static>> {
        self.view.lines()
    }

    pub fn transcript_key(&self) -> (usize, Option<usize>, bool) {
        self.view.transcript_key
    }

    pub fn has_active_cell(&self) -> bool {
        self.view.has_active_cell()
    }

    pub fn paragraph(&self) -> Paragraph<'static> {
        Paragraph::new(self.lines())
            .scroll((self.view.scroll, 0))
            .wrap(Wrap { trim: false })
    }

    pub fn paragraph_at_width(&self, width: u16) -> Paragraph<'static> {
        Paragraph::new(self.view.lines_at_width(Some(width)))
            .scroll((self.view.scroll, 0))
            .wrap(Wrap { trim: false })
    }

    pub fn render(self, frame: &mut Frame<'_>, area: Rect) {
        frame.render_widget(self.paragraph_at_width(area.width), area);
    }
}

impl<'a> From<&TranscriptView<'a>> for ChatView {
    fn from(transcript: &TranscriptView<'a>) -> Self {
        Self::new(build_history_cells(transcript), transcript.scroll)
    }
}

#[cfg(test)]
mod tests {
    use super::{ChatView, ChatWidget};
    use crate::transcript::{
        TimelineEntry, TimelineKind, TranscriptBlock, TranscriptView, TurnPhase,
    };
    use ratatui::text::Line;

    #[test]
    fn chat_widget_transcript_key_tracks_active_revision() {
        let entry = TimelineEntry {
            timestamp: "12:00".to_owned(),
            kind: TimelineKind::Agent,
            block: TranscriptBlock::AssistantMessage,
            actor: "assistant".to_owned(),
            title: "Working".to_owned(),
            body: "hello".to_owned(),
            run_id: Some("run-1".to_owned()),
            phase: Some(TurnPhase::Streaming),
            details: Vec::new(),
            details_expanded: false,
            exec_calls: vec![],
        };
        let transcript = TranscriptView {
            entries: &[],
            active_entry: Some(&entry),
            active_revision: Some(7),
            streaming_preview: None,
            scroll: 0,
            spinner_tick: 0,
        };

        let widget = ChatWidget::new(ChatView::from(&transcript));
        assert_eq!(widget.transcript_key(), (0, Some(7), false));
        assert!(widget.has_active_cell());
        // Title is not displayed for AssistantMessage blocks; check body renders.
        assert!(
            widget
                .lines()
                .iter()
                .any(|line: &Line<'_>| line.to_string().contains("hello"))
        );
    }

    #[test]
    fn chat_view_keeps_committed_active_and_preview_as_cell_state() {
        let committed = TimelineEntry {
            timestamp: "11:59".to_owned(),
            kind: TimelineKind::Operator,
            block: TranscriptBlock::UserMessage,
            actor: "you".to_owned(),
            title: "You".to_owned(),
            body: "hello".to_owned(),
            run_id: None,
            phase: None,
            details: Vec::new(),
            details_expanded: false,
            exec_calls: vec![],
        };
        let active = TimelineEntry {
            timestamp: "12:00".to_owned(),
            kind: TimelineKind::Agent,
            block: TranscriptBlock::AssistantMessage,
            actor: "assistant".to_owned(),
            title: "Working".to_owned(),
            body: "reply".to_owned(),
            run_id: Some("run-1".to_owned()),
            phase: Some(TurnPhase::Streaming),
            details: Vec::new(),
            details_expanded: false,
            exec_calls: vec![],
        };
        let transcript = TranscriptView {
            entries: &[committed],
            active_entry: Some(&active),
            active_revision: Some(7),
            streaming_preview: None,
            scroll: 0,
            spinner_tick: 0,
        };

        let view = ChatView::from(&transcript);
        assert_eq!(view.committed_cells.len(), 1);
        assert!(view.active_cell.is_some());
        assert!(view.streaming_preview.is_none());
        assert!(
            view.lines()
                .iter()
                .any(|line| line.to_string().contains("hello"))
        );
        // Title is not displayed for AssistantMessage; check body text renders.
        assert!(
            view.lines()
                .iter()
                .any(|line| line.to_string().contains("reply"))
        );
    }
}
