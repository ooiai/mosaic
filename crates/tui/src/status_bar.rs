use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusBarView {
    pub workspace: String,
    pub session_label: String,
    pub active_profile: String,
    pub control_model: String,
    pub gateway_live: bool,
    pub gateway_target: String,
    pub hide_runtime_summary: bool,
    pub shell_state_label: &'static str,
    pub runtime_label: String,
    pub runtime_summary: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusBarChromeView {
    pub header: Line<'static>,
    pub subheader: Option<Line<'static>>,
}

impl StatusBarView {
    pub fn into_chrome(self) -> StatusBarChromeView {
        let (gateway_dot, gateway_label, gateway_style) = if self.gateway_live {
            (
                "●",
                format!("live {}", self.gateway_target),
                Style::default().fg(Color::Green),
            )
        } else {
            (
                "●",
                format!("paused {}", self.gateway_target),
                Style::default().fg(Color::Yellow),
            )
        };

        let header = Line::from(vec![
            Span::styled(
                "Mosaic",
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("  ", Style::default().fg(Color::DarkGray)),
            Span::styled(self.workspace, Style::default().fg(Color::Gray)),
            Span::styled("  ", Style::default().fg(Color::DarkGray)),
            Span::styled(self.session_label, Style::default().fg(Color::Cyan)),
            Span::styled("  ·  ", Style::default().fg(Color::DarkGray)),
            Span::styled(self.active_profile, Style::default().fg(Color::Yellow)),
            Span::styled("/", Style::default().fg(Color::DarkGray)),
            Span::styled(self.control_model, Style::default().fg(Color::Gray)),
            Span::styled("  ", Style::default().fg(Color::DarkGray)),
            Span::styled(gateway_dot, gateway_style.add_modifier(Modifier::BOLD)),
            Span::raw(" "),
            Span::styled(gateway_label, Style::default().fg(Color::DarkGray)),
        ]);

        let subheader = if self.hide_runtime_summary {
            None
        } else {
            Some(Line::from(vec![
                Span::styled(self.shell_state_label, Style::default().fg(Color::Cyan)),
                Span::styled("  ·  ", Style::default().fg(Color::DarkGray)),
                Span::styled(self.runtime_label, Style::default().fg(Color::Green)),
                Span::styled("  ·  ", Style::default().fg(Color::DarkGray)),
                Span::styled(self.runtime_summary, Style::default().fg(Color::Gray)),
            ]))
        };

        StatusBarChromeView { header, subheader }
    }
}

pub struct StatusBarWidget {
    header: Paragraph<'static>,
    subheader: Paragraph<'static>,
}

impl StatusBarWidget {
    pub fn new(view: StatusBarView) -> Self {
        Self::from_chrome(view.into_chrome())
    }

    pub fn from_chrome(chrome: StatusBarChromeView) -> Self {
        let header = Paragraph::new(chrome.header);
        let subheader = chrome
            .subheader
            .map(Paragraph::new)
            .unwrap_or_else(|| Paragraph::new(""));

        Self { header, subheader }
    }

    pub fn render(self, frame: &mut ratatui::Frame<'_>, area: Rect) {
        let sections = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(1), Constraint::Length(1)])
            .split(area);
        frame.render_widget(self.header, sections[0]);
        frame.render_widget(self.subheader, sections[1]);
    }
}

pub fn display_workspace_path(path: &str) -> String {
    std::env::var("HOME")
        .ok()
        .and_then(|home| path.strip_prefix(&home).map(|suffix| format!("~{suffix}")))
        .unwrap_or_else(|| path.to_owned())
}

#[cfg(test)]
mod tests {
    use super::StatusBarView;

    #[test]
    fn status_bar_chrome_hides_subheader_when_runtime_summary_is_suppressed() {
        let chrome = StatusBarView {
            workspace: "~/mosaic".to_owned(),
            session_label: "session".to_owned(),
            active_profile: "gpt".to_owned(),
            control_model: "gpt-5.4".to_owned(),
            gateway_live: true,
            gateway_target: "local".to_owned(),
            hide_runtime_summary: true,
            shell_state_label: "running",
            runtime_label: "running".to_owned(),
            runtime_summary: "tool call".to_owned(),
        }
        .into_chrome();

        assert!(chrome.header.to_string().contains("Mosaic"));
        assert!(chrome.subheader.is_none());
    }
}
