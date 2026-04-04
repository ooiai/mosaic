use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
};

/// Returns `true` if the given text looks like a unified diff.
pub fn is_diff(text: &str) -> bool {
    let mut lines = text.lines();
    // Check the first few lines for classic diff markers
    for line in lines.by_ref().take(6) {
        if line.starts_with("diff --git")
            || line.starts_with("--- a/")
            || line.starts_with("+++ b/")
            || line.starts_with("--- /")
            || line.starts_with("+++ /")
        {
            return true;
        }
        if line.starts_with("@@ -") && line.contains(" @@") {
            return true;
        }
    }
    false
}

/// Render a unified diff patch as colored ratatui `Line`s.
///
/// Added lines (`+`) are green, removed lines (`-`) are red, hunk headers
/// (`@@`) are cyan, file headers (`diff`/`---`/`+++`) are bold white, and
/// context lines are dark gray.  Each line is truncated at `width - 2`
/// characters to avoid overflow inside the cell.
pub fn render_diff(patch: &str, width: u16) -> Vec<Line<'static>> {
    let max_len = (width.saturating_sub(2)) as usize;
    let mut lines = Vec::new();

    for raw_line in patch.lines() {
        let truncated = if raw_line.len() > max_len && max_len > 0 {
            raw_line[..max_len].to_owned()
        } else {
            raw_line.to_owned()
        };

        let line = if raw_line.starts_with("diff ")
            || raw_line.starts_with("index ")
            || raw_line.starts_with("--- ")
            || raw_line.starts_with("+++ ")
            || raw_line.starts_with("new file")
            || raw_line.starts_with("deleted file")
            || raw_line.starts_with("rename ")
            || raw_line.starts_with("similarity ")
        {
            // File-level header
            Line::from(Span::styled(
                truncated,
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ))
        } else if raw_line.starts_with("@@ ") {
            // Hunk header
            Line::from(Span::styled(
                truncated,
                Style::default().fg(Color::Cyan),
            ))
        } else if raw_line.starts_with('+') {
            // Added line
            Line::from(Span::styled(
                truncated,
                Style::default().fg(Color::Green),
            ))
        } else if raw_line.starts_with('-') {
            // Removed line
            Line::from(Span::styled(
                truncated,
                Style::default().fg(Color::Red),
            ))
        } else if raw_line.starts_with('\\') {
            // "No newline at end of file" marker — treat as context
            Line::from(Span::styled(
                truncated,
                Style::default().fg(Color::DarkGray),
            ))
        } else {
            // Context line (starts with space) or unknown
            Line::from(Span::styled(
                truncated,
                Style::default().fg(Color::DarkGray),
            ))
        };

        lines.push(line);
    }

    lines
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::style::Color;

    fn sample_diff() -> &'static str {
        "diff --git a/foo.rs b/foo.rs\n\
         index abc..def 100644\n\
         --- a/foo.rs\n\
         +++ b/foo.rs\n\
         @@ -1,3 +1,4 @@\n\
          fn main() {\n\
         -    println!(\"hello\");\n\
         +    println!(\"hello world\");\n\
         +    println!(\"extra\");\n\
          }"
    }

    #[test]
    fn added_line_is_green() {
        let lines = render_diff(sample_diff(), 120);
        let added = lines
            .iter()
            .find(|l| l.spans.iter().any(|s| s.content.starts_with('+')))
            .expect("should have an added line");
        assert!(
            added.spans.iter().any(|s| s.style.fg == Some(Color::Green)),
            "added line should be green"
        );
    }

    #[test]
    fn removed_line_is_red() {
        let lines = render_diff(sample_diff(), 120);
        let removed = lines
            .iter()
            .find(|l| l.spans.iter().any(|s| s.content.starts_with('-') && !s.content.starts_with("---")))
            .expect("should have a removed line");
        assert!(
            removed.spans.iter().any(|s| s.style.fg == Some(Color::Red)),
            "removed line should be red"
        );
    }

    #[test]
    fn hunk_header_is_cyan() {
        let lines = render_diff(sample_diff(), 120);
        let hunk = lines
            .iter()
            .find(|l| l.spans.iter().any(|s| s.content.starts_with("@@ ")))
            .expect("should have a hunk header");
        assert!(
            hunk.spans.iter().any(|s| s.style.fg == Some(Color::Cyan)),
            "hunk header should be cyan"
        );
    }

    #[test]
    fn file_header_is_bold() {
        let lines = render_diff(sample_diff(), 120);
        let header = lines
            .iter()
            .find(|l| l.spans.iter().any(|s| s.content.starts_with("diff ")))
            .expect("should have a diff header");
        assert!(
            header
                .spans
                .iter()
                .any(|s| s.style.add_modifier.contains(Modifier::BOLD)),
            "file header should be bold"
        );
    }

    #[test]
    fn context_line_is_dark_gray() {
        let lines = render_diff(sample_diff(), 120);
        // Context lines start with " " (space)
        let ctx = lines
            .iter()
            .find(|l| l.spans.iter().any(|s| s.content.starts_with(" fn")))
            .expect("should have a context line");
        assert!(
            ctx.spans.iter().any(|s| s.style.fg == Some(Color::DarkGray)),
            "context line should be dark gray"
        );
    }

    #[test]
    fn long_line_is_truncated_at_width() {
        let long_diff = "+".to_owned() + &"a".repeat(200);
        let lines = render_diff(&long_diff, 80);
        assert!(!lines.is_empty());
        let content: String = lines[0].spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(
            content.len() <= 78,
            "line should be truncated to width-2=78, got {}",
            content.len()
        );
    }

    #[test]
    fn is_diff_recognizes_git_diff() {
        assert!(is_diff("diff --git a/foo b/foo\n--- a/foo\n+++ b/foo"));
    }

    #[test]
    fn is_diff_recognizes_hunk_header() {
        assert!(is_diff("@@ -1,3 +1,4 @@ fn foo() {"));
    }

    #[test]
    fn is_diff_rejects_plain_text() {
        assert!(!is_diff("Hello world\nThis is a normal message."));
    }

    #[test]
    fn is_diff_recognizes_plus_plus_plus() {
        assert!(is_diff("+++ b/src/main.rs\n@@ -1 +1 @@\n+fn main() {}"));
    }
}
