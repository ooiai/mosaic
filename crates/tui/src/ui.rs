use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
};

use crate::app::{App, CommandCategory, InputMode, TimelineEntry, TimelineKind, matching_commands};

pub fn render(frame: &mut Frame<'_>, app: &App) {
    render_console(frame, app);
}

fn render_console(frame: &mut Frame<'_>, app: &App) {
    let outer = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2),
            Constraint::Min(8),
            Constraint::Length(3),
            Constraint::Length(1),
        ])
        .split(frame.area());

    render_header(frame, app, outer[0]);
    render_transcript(frame, app, outer[1]);
    render_composer(frame, app, outer[2]);
    render_footer(frame, outer[3]);

    if app.command_query().is_some() {
        let popup = command_popup_rect(frame.area(), outer[2], command_popup_height(app));
        render_command_palette(frame, app, popup);
    }
}

fn render_header(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Length(1)])
        .split(area);

    let workspace = display_workspace_path(&app.workspace_path);
    let gateway = if app.gateway_connected {
        "gateway: live"
    } else {
        "gateway: paused"
    };
    let header = Paragraph::new(Line::from(vec![
        Span::styled(workspace, Style::default().fg(Color::Gray)),
        Span::styled("  •  ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            format!("session {}", app.session_label()),
            Style::default().add_modifier(Modifier::BOLD),
        ),
        Span::styled("  •  ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            format!("profile {}", app.active_profile()),
            Style::default().fg(Color::Cyan),
        ),
        Span::styled("  •  ", Style::default().fg(Color::DarkGray)),
        Span::styled(gateway, Style::default().fg(Color::DarkGray)),
    ]));
    frame.render_widget(header, sections[0]);

    let subheader = Paragraph::new(Line::from(vec![
        Span::styled("model ", Style::default().fg(Color::DarkGray)),
        Span::raw(app.control_model.as_str()),
        Span::styled("  •  ", Style::default().fg(Color::DarkGray)),
        Span::styled("status ", Style::default().fg(Color::DarkGray)),
        Span::styled(app.operator_status(), Style::default().fg(Color::Gray)),
    ]));
    frame.render_widget(subheader, sections[1]);
}

fn render_transcript(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let widget = Paragraph::new(console_lines(app))
        .block(
            Block::default()
                .borders(Borders::TOP | Borders::BOTTOM)
                .title("Conversation"),
        )
        .scroll((app.timeline_scroll, 0))
        .wrap(Wrap { trim: false });
    frame.render_widget(widget, area);
}

fn render_composer(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let draft = app.active_draft();
    let mode = app.input_mode();
    let badge_style = match mode {
        InputMode::Chat => Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
        InputMode::Command => Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD),
        InputMode::Search => Style::default()
            .fg(Color::Green)
            .add_modifier(Modifier::BOLD),
    };
    let badge_text = format!("[{}]", mode.label());
    let prompt_style = if draft.is_empty() {
        Style::default().fg(Color::DarkGray)
    } else {
        Style::default()
    };

    let mut spans = vec![
        Span::styled(badge_text.as_str(), badge_style),
        Span::raw(" "),
        Span::styled(
            if draft.is_empty() {
                app.composer_placeholder().to_owned()
            } else {
                draft.to_owned()
            },
            prompt_style,
        ),
    ];
    if !draft.is_empty() {
        if let Some(suffix) = app.command_completion_suffix() {
            spans.push(Span::styled(suffix, Style::default().fg(Color::DarkGray)));
        }
    }

    let widget = Paragraph::new(Line::from(spans))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Composer")
                .border_style(Style::default().fg(Color::DarkGray)),
        )
        .wrap(Wrap { trim: false });
    frame.render_widget(widget, area);

    let max_cursor_x = area.right().saturating_sub(2);
    let cursor_offset = if draft.is_empty() {
        0
    } else {
        draft.chars().count() as u16
    };
    let cursor_x = area
        .x
        .saturating_add(badge_text.chars().count() as u16 + 2)
        .saturating_add(cursor_offset)
        .min(max_cursor_x);
    let cursor_y = area.y.saturating_add(1);
    frame.set_cursor_position((cursor_x, cursor_y));
}

fn render_footer(frame: &mut Frame<'_>, area: Rect) {
    let footer = Paragraph::new(Line::from(vec![Span::styled(
        "/ commands · Tab accept · ↑↓ select · Enter send · PgUp/PgDn scroll · Ctrl+C quit",
        Style::default().fg(Color::DarkGray),
    )]));
    frame.render_widget(footer, area);
}

fn command_popup_height(app: &App) -> u16 {
    let commands = matching_commands(app.command_query().unwrap_or_default());
    let rows = commands.len().clamp(1, 8) as u16;
    5 + rows
}

fn command_popup_rect(frame_area: Rect, composer_area: Rect, height: u16) -> Rect {
    let width = frame_area.width.saturating_sub(6).min(92).max(32);
    let x = frame_area.x + frame_area.width.saturating_sub(width) / 2;
    let y = composer_area
        .y
        .saturating_sub(height.saturating_add(1))
        .max(frame_area.y + 2);
    Rect {
        x,
        y,
        width,
        height,
    }
}

fn render_command_palette(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let commands = matching_commands(app.command_query().unwrap_or_default());
    let selected = app.selected_command_match();
    let mut lines = Vec::new();

    if let Some(command) = selected {
        lines.push(Line::from(vec![
            Span::styled(
                format!("{} ", display_category(command.category)),
                Style::default()
                    .fg(Color::Magenta)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(command.usage, Style::default().fg(Color::Cyan)),
        ]));
        lines.push(Line::from(Span::styled(
            command.summary,
            Style::default().fg(Color::Gray),
        )));
        lines.push(Line::from(Span::styled(
            command.detail,
            Style::default().fg(Color::DarkGray),
        )));
    } else {
        lines.push(Line::from(Span::styled(
            "No matching commands.",
            Style::default().fg(Color::Yellow),
        )));
        lines.push(Line::from(Span::styled(
            "Keep typing to filter the command list.",
            Style::default().fg(Color::DarkGray),
        )));
        lines.push(Line::from(""));
    }
    lines.push(Line::from(""));

    let selected_index = app
        .command_matches()
        .len()
        .saturating_sub(1)
        .min(app.command_menu_index);
    for (index, command) in commands.iter().enumerate() {
        let is_selected = index == selected_index;
        let marker_style = if is_selected {
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::DarkGray)
        };
        lines.push(Line::from(vec![
            Span::styled(if is_selected { "▸ " } else { "  " }, marker_style),
            Span::styled(
                format!("{:<24}", command.command),
                if is_selected {
                    Style::default().add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                },
            ),
            Span::styled(
                format!("{:<10}", display_category(command.category)),
                Style::default().fg(Color::DarkGray),
            ),
            Span::styled(command.summary, Style::default().fg(Color::Gray)),
        ]));
    }

    let widget = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Commands")
                .border_style(Style::default().fg(Color::DarkGray)),
        )
        .wrap(Wrap { trim: false });

    frame.render_widget(Clear, area);
    frame.render_widget(widget, area);
}

fn console_lines(app: &App) -> Vec<Line<'_>> {
    let mut lines = Vec::new();
    if app.visible_timeline().is_empty() && app.active_streaming_preview().is_none() {
        lines.push(Line::from(vec![Span::styled(
            "No messages yet.",
            Style::default().add_modifier(Modifier::BOLD),
        )]));
        lines.push(Line::from(vec![
            Span::styled("Tip ", Style::default().fg(Color::DarkGray)),
            Span::raw("Type a message to talk to the active session or start with "),
            Span::styled("/", Style::default().fg(Color::Yellow)),
            Span::raw(" to browse commands."),
        ]));
        return lines;
    }

    for entry in app.visible_timeline() {
        lines.extend(entry_lines(entry));
    }

    if let Some(preview) = app.active_streaming_preview() {
        lines.push(Line::from(""));
        lines.push(Line::from(vec![
            Span::styled(
                format!("[{}] ", current_timestamp_label(app)),
                Style::default().fg(Color::DarkGray),
            ),
            Span::styled(
                "Assistant · streaming",
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
        ]));
        for line in preview.lines() {
            lines.push(Line::from(vec![Span::raw("  "), Span::raw(line)]));
        }
    }

    lines
}

fn entry_lines(entry: &TimelineEntry) -> Vec<Line<'_>> {
    let (label, style) = match entry.kind {
        TimelineKind::Operator => (
            "You".to_owned(),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        TimelineKind::Agent => (
            entry.title.clone(),
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ),
        TimelineKind::Tool => (
            entry.title.clone(),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        TimelineKind::System => (
            entry.title.clone(),
            Style::default()
                .fg(Color::Magenta)
                .add_modifier(Modifier::BOLD),
        ),
    };

    let mut lines = vec![Line::from(vec![
        Span::styled(
            format!("[{}] ", entry.timestamp),
            Style::default().fg(Color::DarkGray),
        ),
        Span::styled(label, style),
        Span::styled(
            format!("  {}", entry.actor),
            Style::default().fg(Color::DarkGray),
        ),
    ])];

    if entry.body.is_empty() {
        lines.push(Line::from(vec![
            Span::raw("  "),
            Span::styled("—", Style::default().fg(Color::DarkGray)),
        ]));
    } else {
        for line in entry.body.lines() {
            lines.push(Line::from(vec![Span::raw("  "), Span::raw(line)]));
        }
    }
    lines.push(Line::from(""));
    lines
}

fn display_category(category: CommandCategory) -> &'static str {
    match category {
        CommandCategory::Gateway => "Gateway",
        CommandCategory::Adapter => "Adapter",
        CommandCategory::Node => "Node",
        CommandCategory::Session => "Session",
        CommandCategory::Model => "Model",
        CommandCategory::Run => "Run",
        CommandCategory::Sandbox => "Sandbox",
        CommandCategory::Tool => "Tool",
        CommandCategory::Skill => "Skill",
        CommandCategory::Workflow => "Workflow",
        CommandCategory::Inspect => "Inspect",
        CommandCategory::Ui => "UI",
    }
}

fn display_workspace_path(path: &str) -> String {
    std::env::var("HOME")
        .ok()
        .and_then(|home| path.strip_prefix(&home).map(|suffix| format!("~{suffix}")))
        .unwrap_or_else(|| path.to_owned())
}

fn current_timestamp_label(app: &App) -> String {
    app.visible_timeline()
        .last()
        .map(|entry| entry.timestamp.clone())
        .unwrap_or_else(|| "now".to_owned())
}

#[cfg(test)]
mod tests {
    use ratatui::{Terminal, backend::TestBackend};

    use super::render;
    use crate::app::App;

    fn render_to_text(app: &App, width: u16, height: u16) -> String {
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).expect("terminal should initialize");
        terminal
            .draw(|frame| render(frame, app))
            .expect("render should succeed");

        let buffer = terminal.backend().buffer();
        let area = buffer.area;
        let mut text = String::new();
        for y in 0..area.height {
            for x in 0..area.width {
                let cell = buffer.cell((x, y)).expect("cell should exist");
                text.push_str(cell.symbol());
            }
            text.push('\n');
        }
        text
    }

    #[test]
    fn chat_shell_renders_single_column_header_transcript_and_footer() {
        let app = App::new("/tmp/mosaic".into());
        let screen = render_to_text(&app, 120, 28);

        assert!(screen.contains("Conversation"));
        assert!(screen.contains("Composer"));
        assert!(screen.contains("session sess-gateway-001"));
        assert!(screen.contains("/ commands · Tab accept"));
    }

    #[test]
    fn slash_input_renders_command_popup() {
        let mut app = App::new("/tmp/mosaic".into());
        app.active_session_mut().draft = "/m".to_owned();

        let screen = render_to_text(&app, 120, 28);

        assert!(screen.contains("Commands"));
        assert!(screen.contains("/model list"));
        assert!(screen.contains("Model"));
    }

    #[test]
    fn transcript_renders_streaming_preview_inline() {
        let mut app = App::new_interactive(
            "/tmp/mosaic".into(),
            "demo".to_owned(),
            "openai".to_owned(),
            "gpt-5.4-mini".to_owned(),
            Vec::new(),
            Vec::new(),
            false,
        );
        app.active_session_mut().streaming_preview = Some("partial assistant reply".to_owned());

        let screen = render_to_text(&app, 120, 28);

        assert!(screen.contains("Assistant · streaming"));
        assert!(screen.contains("partial assistant reply"));
    }
}
