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
    pub git_branch: Option<String>,
    pub tokens_in: u64,
    pub tokens_out: u64,
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
            let mut spans = vec![
                Span::styled(self.shell_state_label, Style::default().fg(Color::Cyan)),
                Span::styled("  ·  ", Style::default().fg(Color::DarkGray)),
                Span::styled(self.runtime_label, Style::default().fg(Color::Green)),
                Span::styled("  ·  ", Style::default().fg(Color::DarkGray)),
                Span::styled(self.runtime_summary, Style::default().fg(Color::Gray)),
            ];
            if let Some(branch) = self.git_branch {
                spans.push(Span::styled("  ·  ", Style::default().fg(Color::DarkGray)));
                spans.push(Span::styled(
                    format!("⎇ {}", branch),
                    Style::default().fg(Color::Cyan),
                ));
            }
            if self.tokens_in > 0 || self.tokens_out > 0 {
                spans.push(Span::styled("  ·  ", Style::default().fg(Color::DarkGray)));
                spans.push(Span::styled(
                    format!(
                        "{} in / {} out",
                        fmt_tokens(self.tokens_in),
                        fmt_tokens(self.tokens_out)
                    ),
                    Style::default().fg(Color::DarkGray),
                ));
            }
            Some(Line::from(spans))
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

pub(crate) fn fmt_tokens(n: u64) -> String {
    if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.1}k", n as f64 / 1_000.0)
    } else {
        format!("{}", n)
    }
}

#[cfg(test)]
mod tests {
    use super::{StatusBarView, fmt_tokens};

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
            git_branch: None,
            tokens_in: 0,
            tokens_out: 0,
        }
        .into_chrome();

        assert!(chrome.header.to_string().contains("Mosaic"));
        assert!(chrome.subheader.is_none());
    }

    #[test]
    fn status_bar_shows_git_branch() {
        let chrome = StatusBarView {
            workspace: "~/mosaic".to_owned(),
            session_label: "session".to_owned(),
            active_profile: "gpt".to_owned(),
            control_model: "gpt-5.4".to_owned(),
            gateway_live: true,
            gateway_target: "local".to_owned(),
            hide_runtime_summary: false,
            shell_state_label: "idle",
            runtime_label: "idle".to_owned(),
            runtime_summary: String::new(),
            git_branch: Some("main".to_owned()),
            tokens_in: 0,
            tokens_out: 0,
        }
        .into_chrome();
        let sub = chrome.subheader.unwrap();
        let text: String = sub.spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(
            text.contains("main"),
            "subheader should contain branch name"
        );
    }

    #[test]
    fn status_bar_shows_token_counts() {
        let chrome = StatusBarView {
            workspace: "~/mosaic".to_owned(),
            session_label: "session".to_owned(),
            active_profile: "gpt".to_owned(),
            control_model: "gpt-5.4".to_owned(),
            gateway_live: true,
            gateway_target: "local".to_owned(),
            hide_runtime_summary: false,
            shell_state_label: "idle",
            runtime_label: "idle".to_owned(),
            runtime_summary: String::new(),
            git_branch: None,
            tokens_in: 1200,
            tokens_out: 400,
        }
        .into_chrome();
        let sub = chrome.subheader.unwrap();
        let text: String = sub.spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(text.contains("in"), "subheader should show token counts");
        assert!(text.contains("out"), "subheader should show token out");
    }

    #[test]
    fn fmt_tokens_formats_correctly() {
        assert_eq!(fmt_tokens(0), "0");
        assert_eq!(fmt_tokens(999), "999");
        assert_eq!(fmt_tokens(1000), "1.0k");
        assert_eq!(fmt_tokens(1234), "1.2k");
        assert_eq!(fmt_tokens(1_000_000), "1.0M");
    }
}
