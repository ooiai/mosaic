use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
};

use crate::{
    chat_widget::{ChatWidget, TranscriptSurfaceView},
    composer::{ComposerChromeView, ComposerWidget},
    overlays::{OverlayStack, OverlayStackView},
    status_bar::StatusBarChromeView,
};

pub struct ShellChromeView {
    pub status_bar: StatusBarChromeView,
    pub composer: ComposerChromeView,
}

pub struct ShellSnapshot {
    pub chrome: ShellChromeView,
    pub surface: TranscriptSurfaceView,
    pub overlays: OverlayStackView,
}

pub struct ShellView {
    chat_area: Rect,
    composer_area: Rect,
    chat: ChatWidget,
    composer: ComposerWidget,
    overlays: OverlayStack,
}

impl ShellView {
    pub fn new(snapshot: ShellSnapshot, frame_area: Rect) -> Self {
        let outer = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(8),
                Constraint::Length(4),
            ])
            .split(frame_area);

        Self {
            chat_area: outer[0],
            composer_area: outer[1],
            chat: ChatWidget::new(snapshot.surface.chat.clone()),
            composer: ComposerWidget::from_chrome(snapshot.chrome.composer),
            overlays: OverlayStack::from_view(snapshot.overlays, frame_area, outer[1]),
        }
    }

    pub fn render(self, frame: &mut Frame<'_>) {
        self.chat.render(frame, self.chat_area);
        self.composer.render(frame, self.composer_area);
        self.overlays.render(frame);
    }
}
