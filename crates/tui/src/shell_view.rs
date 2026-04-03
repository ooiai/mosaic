use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
};

use crate::{
    chat_widget::{ChatWidget, TranscriptSurfaceView},
    composer::{ComposerChromeView, ComposerWidget},
    overlays::{OverlayStack, OverlayStackView},
    status_bar::{StatusBarChromeView, StatusBarWidget},
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
    header_area: Rect,
    chat_area: Rect,
    composer_area: Rect,
    header: StatusBarWidget,
    chat: ChatWidget,
    composer: ComposerWidget,
    overlays: OverlayStack,
}

impl ShellView {
    pub fn new(snapshot: ShellSnapshot, frame_area: Rect) -> Self {
        let outer = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(2),
                Constraint::Min(8),
                Constraint::Length(4),
            ])
            .split(frame_area);

        Self {
            header_area: outer[0],
            chat_area: outer[1],
            composer_area: outer[2],
            header: StatusBarWidget::from_chrome(snapshot.chrome.status_bar),
            chat: ChatWidget::new(snapshot.surface.chat.clone()),
            composer: ComposerWidget::from_chrome(snapshot.chrome.composer),
            overlays: OverlayStack::from_view(snapshot.overlays, frame_area, outer[2]),
        }
    }

    pub fn render(self, frame: &mut Frame<'_>) {
        self.header.render(frame, self.header_area);
        self.chat.render(frame, self.chat_area);
        self.composer.render(frame, self.composer_area);
        self.overlays.render(frame);
    }
}
