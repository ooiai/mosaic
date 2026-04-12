use ratatui::{
    style::{Color, Style},
    text::Span,
};

// ── Language keyword lists ────────────────────────────────────────────────

const BASH_KEYWORDS: &[&str] = &[
    "if", "then", "else", "elif", "fi", "for", "while", "do", "done", "case", "esac", "in",
    "return", "exit", "function", "select", "until",
];

const BASH_BUILTINS: &[&str] = &[
    "echo", "cd", "ls", "mkdir", "rm", "cp", "mv", "cat", "grep", "sed", "awk", "curl", "export",
    "source", "read", "printf", "test", "true", "false", "set", "unset", "shift", "exec", "eval",
    "trap", "wait", "kill", "jobs", "bg", "fg", "pushd", "popd", "pwd", "type", "which", "chmod",
    "chown", "touch", "head", "tail", "sort", "uniq", "wc", "find", "xargs", "tee", "cut", "tr",
    "basename", "dirname",
];

const RUST_KEYWORDS: &[&str] = &[
    "fn", "let", "mut", "pub", "use", "mod", "struct", "enum", "impl", "trait", "where", "for",
    "if", "else", "match", "return", "async", "await", "move", "ref", "self", "super", "crate",
    "type", "const", "static", "dyn", "unsafe", "loop", "while", "break", "continue", "in", "as",
    "extern", "true", "false",
];

const PYTHON_KEYWORDS: &[&str] = &[
    "def", "class", "import", "from", "as", "if", "elif", "else", "for", "while", "with", "try",
    "except", "finally", "raise", "return", "yield", "lambda", "pass", "break", "continue",
    "global", "nonlocal", "assert", "del", "in", "not", "and", "or", "is", "True", "False", "None",
    "async", "await",
];

// ── Tokenizer ─────────────────────────────────────────────────────────────

/// Highlight a single line of code for the given language.
///
/// Returns a `Vec<Span<'static>>` ready to embed into a `Line`.
/// If `lang` is unrecognised, returns a single `Color::Cyan` span.
pub fn highlight_code(lang: &str, code_line: &str) -> Vec<Span<'static>> {
    if code_line.is_empty() {
        return vec![Span::raw("")];
    }

    match lang.to_ascii_lowercase().as_str() {
        "bash" | "sh" | "shell" | "zsh" | "fish" => highlight_bash(code_line),
        "rust" | "rs" => highlight_rust(code_line),
        "python" | "py" => highlight_python(code_line),
        "json" => highlight_json(code_line),
        _ => vec![Span::styled(
            code_line.to_owned(),
            Style::default().fg(Color::Cyan),
        )],
    }
}

// ── Bash ─────────────────────────────────────────────────────────────────

fn highlight_bash(line: &str) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    let mut chars = line.char_indices().peekable();

    while let Some((start, ch)) = chars.next() {
        match ch {
            // Comment
            '#' => {
                let rest = line[start..].to_owned();
                spans.push(Span::styled(rest, Style::default().fg(Color::DarkGray)));
                break;
            }
            // String literals
            '"' | '\'' => {
                let quote = ch;
                let mut s = String::from(ch);
                let mut closed = false;
                for (_, c) in chars.by_ref() {
                    s.push(c);
                    if c == quote {
                        closed = true;
                        break;
                    }
                }
                let _ = closed;
                spans.push(Span::styled(s, Style::default().fg(Color::Green)));
            }
            // Variable expansion
            '$' => {
                let mut s = String::from('$');
                if let Some(&(_, '{')) = chars.peek() {
                    s.push('{');
                    chars.next();
                    for (_, c) in chars.by_ref() {
                        s.push(c);
                        if c == '}' {
                            break;
                        }
                    }
                } else {
                    for (_, c) in chars.by_ref() {
                        if c.is_alphanumeric() || c == '_' {
                            s.push(c);
                        } else {
                            // Put back the non-word char as a plain span
                            spans.push(Span::styled(s.clone(), Style::default().fg(Color::Yellow)));
                            spans.push(Span::styled(
                                c.to_string(),
                                Style::default().fg(Color::Cyan),
                            ));
                            s.clear();
                            break;
                        }
                    }
                }
                if !s.is_empty() {
                    spans.push(Span::styled(s, Style::default().fg(Color::Yellow)));
                }
            }
            // Word tokens (keywords / builtins / identifiers)
            c if c.is_alphabetic() || c == '_' => {
                let mut word = String::from(c);
                while let Some(&(_, nc)) = chars.peek() {
                    if nc.is_alphanumeric() || nc == '_' || nc == '-' {
                        word.push(nc);
                        chars.next();
                    } else {
                        break;
                    }
                }
                let style = if BASH_KEYWORDS.contains(&word.as_str()) {
                    Style::default().fg(Color::Magenta)
                } else if BASH_BUILTINS.contains(&word.as_str()) {
                    Style::default().fg(Color::LightBlue)
                } else {
                    Style::default().fg(Color::Cyan)
                };
                spans.push(Span::styled(word, style));
            }
            // Everything else
            _ => {
                spans.push(Span::styled(
                    ch.to_string(),
                    Style::default().fg(Color::Cyan),
                ));
            }
        }
    }

    spans
}

// ── Rust ──────────────────────────────────────────────────────────────────

fn highlight_rust(line: &str) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    let mut chars = line.char_indices().peekable();

    while let Some((start, ch)) = chars.next() {
        match ch {
            // Line comment
            '/' if matches!(chars.peek(), Some(&(_, '/'))) => {
                let rest = line[start..].to_owned();
                spans.push(Span::styled(rest, Style::default().fg(Color::DarkGray)));
                break;
            }
            // String literal
            '"' => {
                let mut s = String::from('"');
                let mut escaped = false;
                for (_, c) in chars.by_ref() {
                    s.push(c);
                    if escaped {
                        escaped = false;
                    } else if c == '\\' {
                        escaped = true;
                    } else if c == '"' {
                        break;
                    }
                }
                spans.push(Span::styled(s, Style::default().fg(Color::Green)));
            }
            // Char literal / lifetime
            '\'' => {
                // Peek to detect lifetime vs char
                let mut s = String::from('\'');
                let peek_start = start + 1;
                if peek_start < line.len() {
                    // Read the rest as lifetime: 'ident
                    let rest = &line[peek_start..];
                    let word_end = rest
                        .find(|c: char| !c.is_alphanumeric() && c != '_')
                        .unwrap_or(rest.len());
                    if word_end > 0 && word_end <= 32 {
                        let word = &rest[..word_end];
                        s.push_str(word);
                        for _ in 0..word.chars().count() {
                            chars.next();
                        }
                        // If followed by ' it's a char, otherwise a lifetime
                        if chars.peek().map(|&(_, c)| c) == Some('\'') {
                            s.push('\'');
                            chars.next();
                            spans.push(Span::styled(s, Style::default().fg(Color::Green)));
                        } else {
                            spans.push(Span::styled(s, Style::default().fg(Color::LightYellow)));
                        }
                        continue;
                    }
                }
                spans.push(Span::styled(s, Style::default().fg(Color::Cyan)));
            }
            // Number
            c if c.is_ascii_digit() => {
                let mut n = String::from(c);
                while let Some(&(_, nc)) = chars.peek() {
                    if nc.is_alphanumeric() || nc == '_' || nc == '.' {
                        n.push(nc);
                        chars.next();
                    } else {
                        break;
                    }
                }
                spans.push(Span::styled(n, Style::default().fg(Color::Yellow)));
            }
            // Word tokens (keywords / type names / identifiers)
            c if c.is_alphabetic() || c == '_' => {
                let mut word = String::from(c);
                while let Some(&(_, nc)) = chars.peek() {
                    if nc.is_alphanumeric() || nc == '_' {
                        word.push(nc);
                        chars.next();
                    } else {
                        break;
                    }
                }
                let style = if RUST_KEYWORDS.contains(&word.as_str()) {
                    Style::default().fg(Color::Magenta)
                } else if word
                    .chars()
                    .next()
                    .map(|c| c.is_uppercase())
                    .unwrap_or(false)
                {
                    Style::default().fg(Color::LightBlue)
                } else {
                    Style::default().fg(Color::Cyan)
                };
                spans.push(Span::styled(word, style));
            }
            _ => {
                spans.push(Span::styled(
                    ch.to_string(),
                    Style::default().fg(Color::Cyan),
                ));
            }
        }
    }

    spans
}

// ── Python ────────────────────────────────────────────────────────────────

fn highlight_python(line: &str) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    let mut chars = line.char_indices().peekable();

    while let Some((start, ch)) = chars.next() {
        match ch {
            '#' => {
                let rest = line[start..].to_owned();
                spans.push(Span::styled(rest, Style::default().fg(Color::DarkGray)));
                break;
            }
            '@' => {
                // Decorator
                let mut s = String::from('@');
                while let Some(&(_, nc)) = chars.peek() {
                    if nc.is_alphanumeric() || nc == '_' || nc == '.' {
                        s.push(nc);
                        chars.next();
                    } else {
                        break;
                    }
                }
                spans.push(Span::styled(s, Style::default().fg(Color::LightYellow)));
            }
            '"' | '\'' => {
                let quote = ch;
                let mut s = String::from(ch);
                // Check for triple quote
                let is_triple = matches!(
                    (chars.peek().map(|&(_, c)| c), {
                        // peek second
                        let rest = &line[start + ch.len_utf8()..];
                        rest.chars().nth(1)
                    }),
                    (Some(q1), Some(q2)) if q1 == quote && q2 == quote
                );
                if is_triple {
                    // Just consume everything to EOL for single-line triple
                    s.push_str(&line[start + 1..]);
                    // Skip remaining chars
                    for _ in chars.by_ref() {}
                } else {
                    let mut escaped = false;
                    for (_, c) in chars.by_ref() {
                        s.push(c);
                        if escaped {
                            escaped = false;
                        } else if c == '\\' {
                            escaped = true;
                        } else if c == quote {
                            break;
                        }
                    }
                }
                spans.push(Span::styled(s, Style::default().fg(Color::Green)));
            }
            c if c.is_ascii_digit() => {
                let mut n = String::from(c);
                while let Some(&(_, nc)) = chars.peek() {
                    if nc.is_alphanumeric() || nc == '_' || nc == '.' {
                        n.push(nc);
                        chars.next();
                    } else {
                        break;
                    }
                }
                spans.push(Span::styled(n, Style::default().fg(Color::Yellow)));
            }
            c if c.is_alphabetic() || c == '_' => {
                let mut word = String::from(c);
                while let Some(&(_, nc)) = chars.peek() {
                    if nc.is_alphanumeric() || nc == '_' {
                        word.push(nc);
                        chars.next();
                    } else {
                        break;
                    }
                }
                let style = if PYTHON_KEYWORDS.contains(&word.as_str()) {
                    Style::default().fg(Color::Magenta)
                } else {
                    Style::default().fg(Color::Cyan)
                };
                spans.push(Span::styled(word, style));
            }
            _ => {
                spans.push(Span::styled(
                    ch.to_string(),
                    Style::default().fg(Color::Cyan),
                ));
            }
        }
    }

    spans
}

// ── JSON ──────────────────────────────────────────────────────────────────

fn highlight_json(line: &str) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    let mut chars = line.char_indices().peekable();

    // Detect if we're on a key line (simplified: first quoted string before ':')
    // We use a simple left-to-right scan.
    let mut seen_colon = false;

    while let Some((_start, ch)) = chars.next() {
        match ch {
            '"' => {
                let mut s = String::from('"');
                let mut escaped = false;
                for (_, c) in chars.by_ref() {
                    s.push(c);
                    if escaped {
                        escaped = false;
                    } else if c == '\\' {
                        escaped = true;
                    } else if c == '"' {
                        break;
                    }
                }
                // After the closing quote, peek for ':'
                let peek = chars.peek().map(|&(_, c)| c);
                let peek2 = {
                    let rest = &line[_start..];
                    // rough: check if the next non-space char is ':'
                    rest.chars().skip(s.len()).find(|c| !c.is_whitespace()) == Some(':')
                };
                let color = if !seen_colon && peek2 {
                    Color::LightBlue // key
                } else {
                    Color::Green // value
                };
                if peek == Some(':') {
                    seen_colon = true;
                }
                spans.push(Span::styled(s, Style::default().fg(color)));
            }
            ':' => {
                seen_colon = true;
                spans.push(Span::styled(":", Style::default().fg(Color::DarkGray)));
            }
            '{' | '}' | '[' | ']' | ',' => {
                spans.push(Span::styled(
                    ch.to_string(),
                    Style::default().fg(Color::DarkGray),
                ));
            }
            // Numbers
            c if c.is_ascii_digit()
                || (c == '-'
                    && chars
                        .peek()
                        .map(|&(_, nc)| nc.is_ascii_digit())
                        .unwrap_or(false)) =>
            {
                let mut n = String::from(c);
                while let Some(&(_, nc)) = chars.peek() {
                    if nc.is_ascii_digit()
                        || nc == '.'
                        || nc == 'e'
                        || nc == 'E'
                        || nc == '+'
                        || nc == '-'
                    {
                        n.push(nc);
                        chars.next();
                    } else {
                        break;
                    }
                }
                spans.push(Span::styled(n, Style::default().fg(Color::Yellow)));
            }
            // Booleans / null
            't' | 'f' | 'n' => {
                let mut word = String::from(ch);
                while let Some(&(_, nc)) = chars.peek() {
                    if nc.is_alphabetic() {
                        word.push(nc);
                        chars.next();
                    } else {
                        break;
                    }
                }
                let color = match word.as_str() {
                    "true" | "false" | "null" => Color::Magenta,
                    _ => Color::Cyan,
                };
                spans.push(Span::styled(word, Style::default().fg(color)));
            }
            _ => {
                spans.push(Span::styled(
                    ch.to_string(),
                    Style::default().fg(Color::Cyan),
                ));
            }
        }
    }

    spans
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::style::Color;

    fn spans_text(spans: &[Span]) -> String {
        spans.iter().map(|s| s.content.as_ref()).collect()
    }

    fn spans_color_for(spans: &[Span], text: &str) -> Option<Color> {
        spans
            .iter()
            .find(|s| s.content.as_ref() == text)
            .and_then(|s| s.style.fg)
    }

    // ── Bash tests ────────────────────────────────────────────────────────

    #[test]
    fn bash_keyword_is_magenta() {
        let spans = highlight_code("bash", "if true");
        assert_eq!(
            spans_color_for(&spans, "if"),
            Some(Color::Magenta),
            "bash keyword 'if' should be Magenta"
        );
    }

    #[test]
    fn bash_builtin_is_light_blue() {
        let spans = highlight_code("bash", "echo hello");
        assert_eq!(
            spans_color_for(&spans, "echo"),
            Some(Color::LightBlue),
            "bash builtin 'echo' should be LightBlue"
        );
    }

    #[test]
    fn bash_variable_is_yellow() {
        let spans = highlight_code("bash", "echo $HOME");
        assert!(
            spans
                .iter()
                .any(|s| s.style.fg == Some(Color::Yellow) && s.content.contains("HOME")),
            "bash variable $HOME should be yellow"
        );
    }

    #[test]
    fn bash_comment_is_dark_gray() {
        let spans = highlight_code("bash", "# this is a comment");
        assert!(
            spans.iter().any(|s| s.style.fg == Some(Color::DarkGray)),
            "bash comment should be DarkGray"
        );
    }

    // ── Rust tests ────────────────────────────────────────────────────────

    #[test]
    fn rust_keyword_is_magenta() {
        let spans = highlight_code("rust", "fn main() {}");
        assert_eq!(
            spans_color_for(&spans, "fn"),
            Some(Color::Magenta),
            "rust keyword 'fn' should be Magenta"
        );
    }

    #[test]
    fn rust_string_is_green() {
        let spans = highlight_code("rust", r#"let s = "hello";"#);
        assert!(
            spans
                .iter()
                .any(|s| s.style.fg == Some(Color::Green) && s.content.contains("hello")),
            "rust string should be Green"
        );
    }

    #[test]
    fn rust_comment_is_dark_gray() {
        let spans = highlight_code("rust", "// this is a comment");
        assert!(
            spans.iter().any(|s| s.style.fg == Some(Color::DarkGray)),
            "rust comment should be DarkGray"
        );
    }

    // ── Python tests ──────────────────────────────────────────────────────

    #[test]
    fn python_keyword_is_magenta() {
        let spans = highlight_code("python", "def foo():");
        assert_eq!(
            spans_color_for(&spans, "def"),
            Some(Color::Magenta),
            "python keyword 'def' should be Magenta"
        );
    }

    // ── JSON tests ────────────────────────────────────────────────────────

    #[test]
    fn json_boolean_is_magenta() {
        let spans = highlight_code("json", "true");
        assert!(
            spans.iter().any(|s| s.style.fg == Some(Color::Magenta)),
            "json boolean 'true' should be Magenta"
        );
    }

    #[test]
    fn json_null_is_magenta() {
        let spans = highlight_code("json", "null");
        assert!(
            spans.iter().any(|s| s.style.fg == Some(Color::Magenta)),
            "json null should be Magenta"
        );
    }

    // ── Unknown language ──────────────────────────────────────────────────

    #[test]
    fn unknown_language_returns_cyan_span() {
        let spans = highlight_code("cobol", "MOVE x TO y");
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].style.fg, Some(Color::Cyan));
    }

    #[test]
    fn empty_line_returns_empty_span() {
        let spans = highlight_code("rust", "");
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].content.as_ref(), "");
    }
}
