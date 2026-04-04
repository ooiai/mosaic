use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};
use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
};

pub fn render_markdown(text: &str, width: u16) -> Vec<Line<'static>> {
    let mut lines: Vec<Line<'static>> = Vec::new();
    let mut current_spans: Vec<Span<'static>> = Vec::new();

    let mut bold = false;
    let mut italic = false;
    let mut in_code_block = false;
    let mut code_lang = String::new();
    let mut code_block_lines: Vec<String> = Vec::new();
    let mut list_depth: usize = 0;
    let mut ordered_list_counter: Vec<u64> = Vec::new();
    let mut heading_level: Option<u8> = None;

    let options = Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TABLES;
    let parser = Parser::new_ext(text, options);

    let indent_width = width.saturating_sub(4).max(10) as usize;

    for event in parser {
        match event {
            Event::Start(Tag::Heading { level, .. }) => {
                flush_spans(&mut current_spans, &mut lines);
                heading_level = Some(level as u8);
            }
            Event::End(TagEnd::Heading(_)) => {
                let spans = std::mem::take(&mut current_spans);
                if !spans.is_empty() {
                    let level = heading_level.unwrap_or(1);
                    let prefix = match level {
                        1 => "# ",
                        2 => "## ",
                        _ => "### ",
                    };
                    let (fg, modifier) = match level {
                        1 => (Color::White, Modifier::BOLD),
                        2 => (Color::LightBlue, Modifier::BOLD),
                        _ => (Color::Cyan, Modifier::BOLD),
                    };
                    let mut header_spans = vec![Span::styled(
                        prefix,
                        Style::default().fg(fg).add_modifier(modifier),
                    )];
                    for span in spans {
                        header_spans.push(Span::styled(
                            span.content.into_owned(),
                            Style::default().fg(fg).add_modifier(modifier),
                        ));
                    }
                    lines.push(Line::from(header_spans));
                    lines.push(Line::from(""));
                }
                heading_level = None;
            }
            Event::Start(Tag::Paragraph) => {}
            Event::End(TagEnd::Paragraph) => {
                flush_spans(&mut current_spans, &mut lines);
                lines.push(Line::from(""));
            }
            Event::Start(Tag::CodeBlock(kind)) => {
                flush_spans(&mut current_spans, &mut lines);
                in_code_block = true;
                code_lang = match kind {
                    pulldown_cmark::CodeBlockKind::Fenced(lang) => lang.to_string(),
                    pulldown_cmark::CodeBlockKind::Indented => String::new(),
                };
                code_block_lines.clear();
            }
            Event::End(TagEnd::CodeBlock) => {
                in_code_block = false;
                for code_line in &code_block_lines {
                    lines.push(Line::from(vec![
                        Span::styled("  ", Style::default()),
                        Span::styled(code_line.clone(), Style::default().fg(Color::Cyan)),
                    ]));
                }
                if !code_block_lines.is_empty() {
                    lines.push(Line::from(""));
                }
                code_block_lines.clear();
                code_lang.clear();
            }
            Event::Start(Tag::List(start_num)) => {
                flush_spans(&mut current_spans, &mut lines);
                list_depth += 1;
                if let Some(n) = start_num {
                    ordered_list_counter.push(n);
                } else {
                    ordered_list_counter.push(0);
                }
            }
            Event::End(TagEnd::List(_)) => {
                flush_spans(&mut current_spans, &mut lines);
                list_depth = list_depth.saturating_sub(1);
                ordered_list_counter.pop();
                if list_depth == 0 {
                    lines.push(Line::from(""));
                }
            }
            Event::Start(Tag::Item) => {
                flush_spans(&mut current_spans, &mut lines);
                if let Some(counter) = ordered_list_counter.last_mut() {
                    if *counter > 0 {
                        *counter += 1;
                    }
                }
            }
            Event::End(TagEnd::Item) => {
                let spans = std::mem::take(&mut current_spans);
                let indent = "  ".repeat(list_depth.saturating_sub(1));
                let bullet = if let Some(&counter) = ordered_list_counter.last() {
                    if counter > 0 {
                        format!("{}{}. ", indent, counter - 1)
                    } else {
                        format!("{}· ", indent)
                    }
                } else {
                    format!("{}· ", indent)
                };
                let mut item_spans = vec![Span::styled(bullet, Style::default().fg(Color::Yellow))];
                item_spans.extend(spans);
                lines.push(Line::from(item_spans));
            }
            Event::Start(Tag::Strong) => {
                bold = true;
            }
            Event::End(TagEnd::Strong) => {
                bold = false;
            }
            Event::Start(Tag::Emphasis) => {
                italic = true;
            }
            Event::End(TagEnd::Emphasis) => {
                italic = false;
            }
            Event::Start(Tag::Link {
                dest_url, title, ..
            }) => {
                let _ = dest_url;
                let _ = title;
            }
            Event::End(TagEnd::Link) => {}
            Event::Start(Tag::BlockQuote(_)) => {
                flush_spans(&mut current_spans, &mut lines);
            }
            Event::End(TagEnd::BlockQuote(_)) => {
                flush_spans(&mut current_spans, &mut lines);
                lines.push(Line::from(""));
            }
            Event::Start(Tag::HtmlBlock) | Event::End(TagEnd::HtmlBlock) => {}
            Event::Start(_) | Event::End(_) => {}
            Event::Text(text) => {
                let text: String = String::from(text);
                if in_code_block {
                    for line in text.lines() {
                        code_block_lines.push(line.to_string());
                    }
                } else {
                    let style = inline_style(bold, italic);
                    let parts: Vec<String> =
                        text.split('\n').map(|s: &str| s.to_string()).collect();
                    for (i, part) in parts.iter().enumerate() {
                        if !part.is_empty() {
                            current_spans.push(Span::styled(part.clone(), style));
                        }
                        if i < parts.len() - 1 {
                            flush_spans(&mut current_spans, &mut lines);
                        }
                    }
                }
            }
            Event::Code(code) => {
                current_spans.push(Span::styled(
                    format!("`{}`", code),
                    Style::default()
                        .fg(Color::LightGreen)
                        .add_modifier(Modifier::BOLD),
                ));
            }
            Event::Html(_) | Event::InlineHtml(_) => {}
            Event::SoftBreak => {
                if !current_spans.is_empty() {
                    current_spans.push(Span::raw(" "));
                }
            }
            Event::HardBreak => {
                flush_spans(&mut current_spans, &mut lines);
            }
            Event::Rule => {
                flush_spans(&mut current_spans, &mut lines);
                let rule: String = "─".repeat(indent_width.min(60));
                lines.push(Line::from(vec![Span::styled(
                    rule,
                    Style::default().fg(Color::DarkGray),
                )]));
                lines.push(Line::from(""));
            }
            Event::TaskListMarker(checked) => {
                let marker = if checked { "[x] " } else { "[ ] " };
                current_spans.push(Span::styled(marker, Style::default().fg(Color::Yellow)));
            }
            Event::FootnoteReference(_) | Event::DisplayMath(_) | Event::InlineMath(_) => {}
        }
    }

    flush_spans(&mut current_spans, &mut lines);

    while lines
        .last()
        .map(|l: &Line| l.spans.is_empty() || l.spans.iter().all(|s| s.content.is_empty()))
        .unwrap_or(false)
    {
        lines.pop();
    }

    lines
}

fn inline_style(bold: bool, italic: bool) -> Style {
    let mut style = Style::default().fg(Color::White);
    if bold {
        style = style.add_modifier(Modifier::BOLD);
    }
    if italic {
        style = style.add_modifier(Modifier::ITALIC);
    }
    style
}

fn flush_spans(spans: &mut Vec<Span<'static>>, lines: &mut Vec<Line<'static>>) {
    if !spans.is_empty() {
        let taken = std::mem::take(spans);
        lines.push(Line::from(taken));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::style::Modifier;

    #[test]
    fn renders_heading_with_bold() {
        let lines = render_markdown("# Hello World", 80);
        assert!(!lines.is_empty());
        let first = &lines[0];
        assert!(
            first
                .spans
                .iter()
                .any(|s| s.style.add_modifier.contains(Modifier::BOLD))
        );
    }

    #[test]
    fn renders_bold_inline() {
        let lines = render_markdown("This is **bold** text", 80);
        assert!(!lines.is_empty());
        let has_bold = lines[0]
            .spans
            .iter()
            .any(|s| s.style.add_modifier.contains(Modifier::BOLD));
        assert!(has_bold, "bold text should have BOLD modifier");
    }

    #[test]
    fn renders_inline_code() {
        let lines = render_markdown("Use `cargo test` to run tests", 80);
        assert!(!lines.is_empty());
        let has_code = lines[0]
            .spans
            .iter()
            .any(|s| s.content.contains("cargo test"));
        assert!(has_code);
    }

    #[test]
    fn renders_fenced_code_block() {
        let md = "```rust\nfn main() {}\n```";
        let lines = render_markdown(md, 80);
        assert!(!lines.is_empty());
        let has_code = lines
            .iter()
            .any(|l| l.spans.iter().any(|s| s.content.contains("fn main")));
        assert!(has_code, "fenced code block should appear in output");
        let code_line = lines
            .iter()
            .find(|l| l.spans.iter().any(|s| s.content.contains("fn main")))
            .unwrap();
        assert!(
            code_line
                .spans
                .iter()
                .any(|s| s.style.fg == Some(Color::Cyan))
        );
    }

    #[test]
    fn renders_bullet_list() {
        let md = "- item one\n- item two";
        let lines = render_markdown(md, 80);
        assert!(lines.len() >= 2);
        let has_bullet = lines
            .iter()
            .any(|l| l.spans.iter().any(|s| s.content.contains('·')));
        assert!(has_bullet, "bullet list should have bullet marker");
    }

    #[test]
    fn empty_input_returns_empty() {
        let lines = render_markdown("", 80);
        assert!(lines.is_empty());
    }
}
