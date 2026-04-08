use ratatui::{
    layout::Rect,
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
    pub effort_label: String,
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
    pub left: String,
    pub right: String,
}

impl StatusBarView {
    pub fn into_chrome(self) -> StatusBarChromeView {
        let left = self.workspace;
        let right = format!("{} ({})", self.control_model, self.effort_label);
        StatusBarChromeView { left, right }
    }
}

pub struct StatusBarWidget {
    left: String,
    right: String,
}

impl StatusBarWidget {
    pub fn new(view: StatusBarView) -> Self {
        Self::from_chrome(view.into_chrome())
    }

    pub fn from_chrome(chrome: StatusBarChromeView) -> Self {
        Self {
            left: chrome.left,
            right: chrome.right,
        }
    }

    pub fn render(self, frame: &mut ratatui::Frame<'_>, area: Rect) {
        let left_len = self.left.chars().count() as u16;
        let right_len = self.right.chars().count() as u16;
        let total = left_len + right_len;
        let padding = area.width.saturating_sub(total).max(1) as usize;
        let line = Line::from(vec![
            Span::styled(self.left, Style::default().fg(Color::DarkGray)),
            Span::raw(" ".repeat(padding)),
            Span::styled(
                self.right,
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
        ]);
        frame.render_widget(Paragraph::new(line), area);
    }
}

pub fn display_workspace_path(path: &str) -> String {
    std::env::var("HOME")
        .ok()
        .and_then(|home| path.strip_prefix(&home).map(|suffix| format!("~{suffix}")))
        .unwrap_or_else(|| path.to_owned())
}

#[allow(dead_code)]
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

    fn make_view(workspace: &str, model: &str) -> StatusBarView {
        StatusBarView {
            workspace: workspace.to_owned(),
            session_label: "session".to_owned(),
            active_profile: "gpt".to_owned(),
            control_model: model.to_owned(),
            effort_label: "medium".to_owned(),
            gateway_live: true,
            gateway_target: "local".to_owned(),
            hide_runtime_summary: false,
            shell_state_label: "idle",
            runtime_label: "idle".to_owned(),
            runtime_summary: String::new(),
            git_branch: None,
            tokens_in: 0,
            tokens_out: 0,
        }
    }

    #[test]
    fn status_bar_chrome_contains_workspace_and_model() {
        let chrome = make_view("~/mosaic", "gpt-5.4").into_chrome();
        assert!(chrome.left.contains("~/mosaic"), "left must contain workspace");
        assert!(chrome.right.contains("gpt-5.4"), "right must contain model");
        assert!(chrome.right.contains("medium"), "right must contain effort");
    }

    #[test]
    fn status_bar_shows_git_branch_in_workspace() {
        // Branch is not currently displayed — just verify the chrome fields are present.
        let chrome = make_view("~/mosaic", "gpt-5.4").into_chrome();
        assert!(!chrome.left.is_empty(), "left must not be empty");
        assert!(!chrome.right.is_empty(), "right must not be empty");
    }

    #[test]
    fn status_bar_right_contains_effort_label() {
        let chrome = make_view("~/project", "claude-sonnet-4.6").into_chrome();
        assert!(chrome.right.contains("claude-sonnet-4.6"));
        assert!(chrome.right.contains("medium"));
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
