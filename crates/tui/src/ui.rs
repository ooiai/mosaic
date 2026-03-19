use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
};

use crate::app::{App, ResumeScope, Surface, TimelineEntry, TimelineKind, matching_commands};

pub fn render(frame: &mut Frame<'_>, app: &App) {
    match app.surface {
        Surface::Console => render_console(frame, app),
        Surface::Resume => render_resume(frame, app),
    }

    if app.show_help_overlay {
        render_help_overlay(frame);
    }
}

fn render_console(frame: &mut Frame<'_>, app: &App) {
    let outer = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(11),
            Constraint::Min(8),
            Constraint::Length(1),
            Constraint::Length(3),
            Constraint::Length(1),
        ])
        .split(frame.area());

    render_welcome(frame, app, outer[0]);
    render_console_stream(frame, app, outer[1]);
    render_workspace_line(frame, app, outer[2]);
    render_composer(frame, app, outer[3]);
    render_footer(frame, app, outer[4]);

    if app.command_query().is_some() {
        render_command_palette(frame, app, outer[3]);
    }
}

fn render_resume(frame: &mut Frame<'_>, app: &App) {
    let outer = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2),
            Constraint::Length(5),
            Constraint::Min(10),
            Constraint::Length(1),
        ])
        .split(frame.area());

    let status = Paragraph::new(vec![
        Line::from(vec![
            Span::styled("* ", Style::default().fg(Color::Cyan)),
            Span::raw(
                "Experimental local mode is enabled. Runtime, tool, and node feeds remain mock-driven.",
            ),
        ]),
        Line::from(vec![
            Span::styled("* ", Style::default().fg(Color::Blue)),
            Span::raw("Use Tab to cycle scope tabs and / to filter the resume list."),
        ]),
    ]);
    frame.render_widget(status, outer[0]);

    let visible = app.visible_session_indices();
    let active_resume = visible
        .iter()
        .copied()
        .find(|index| *index == app.selected_session)
        .or_else(|| visible.first().copied());
    let other_sessions = visible
        .iter()
        .copied()
        .filter(|index| Some(*index) != active_resume)
        .collect::<Vec<_>>();

    let mut lines = vec![
        Line::from(""),
        Line::from(vec![Span::styled(
            "Select a session to resume:",
            Style::default().add_modifier(Modifier::BOLD),
        )]),
        Line::from(""),
        resume_tabs_line(app.resume_scope),
        Line::from(""),
    ];

    if app.resume_search || !app.resume_query.is_empty() {
        lines.push(Line::from(vec![
            Span::styled("Search ", Style::default().fg(Color::DarkGray)),
            Span::raw(app.resume_query.as_str()),
        ]));
        lines.push(Line::from(""));
    }

    if visible.is_empty() {
        lines.push(Line::from(vec![Span::styled(
            "No sessions match the current filter.",
            Style::default().fg(Color::Yellow),
        )]));
    } else {
        if let Some(index) = active_resume {
            lines.push(Line::from(vec![Span::styled(
                "This branch",
                Style::default()
                    .fg(Color::Magenta)
                    .add_modifier(Modifier::BOLD),
            )]));
            lines.push(Line::from(vec![Span::styled(
                "  ------------------------------",
                Style::default().fg(Color::DarkGray),
            )]));
            lines.push(resume_table_header());
            lines.push(resume_row_line(app, index));
            lines.push(Line::from(""));
        }

        if !other_sessions.is_empty() {
            lines.push(Line::from(vec![Span::styled(
                "Other sessions",
                Style::default()
                    .fg(Color::Magenta)
                    .add_modifier(Modifier::BOLD),
            )]));
            lines.push(Line::from(vec![Span::styled(
                "  ------------------------------",
                Style::default().fg(Color::DarkGray),
            )]));
            lines.push(resume_table_header());
            for index in other_sessions {
                lines.push(resume_row_line(app, index));
            }
        }
    }

    let spacer = Paragraph::new("");
    frame.render_widget(spacer, outer[1]);

    let widget = Paragraph::new(lines).wrap(Wrap { trim: false });
    frame.render_widget(widget, outer[2]);

    let footer = Paragraph::new(Line::from(vec![
        Span::raw("up/down to navigate · "),
        Span::raw("Enter to select · "),
        Span::raw("Esc to cancel · "),
        Span::raw("/ to search"),
    ]));
    frame.render_widget(footer, outer[3]);
}

fn render_welcome(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(7),
            Constraint::Length(2),
            Constraint::Min(1),
        ])
        .split(area);

    let card_width = area.width.min(76);
    let card_area = Rect {
        x: area.x,
        y: area.y,
        width: card_width,
        height: sections[0].height,
    };
    let card = Paragraph::new(vec![
        Line::from(vec![
            Span::styled("[ ] [ ]", Style::default().fg(Color::Cyan)),
            Span::raw("  "),
            Span::styled(
                "Mosaic Copilot ".to_owned(),
                Style::default().add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("v{}", env!("CARGO_PKG_VERSION")),
                Style::default().fg(Color::DarkGray),
            ),
        ]),
        Line::from(vec![
            Span::styled("[_][__]", Style::default().fg(Color::Magenta)),
            Span::raw("  "),
            Span::raw("Describe a task to get started."),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("[__][__]", Style::default().fg(Color::Magenta)),
            Span::raw("  "),
            Span::styled("Tip: ", Style::default().fg(Color::DarkGray)),
            Span::styled("/help", Style::default().fg(Color::Cyan)),
            Span::raw(" Show local commands and shortcuts."),
        ]),
        Line::from(vec![Span::raw(
            "Mosaic uses AI, so always check for mistakes.",
        )]),
    ])
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Magenta)),
    )
    .wrap(Wrap { trim: false });
    frame.render_widget(card, card_area);

    let details = Paragraph::new(vec![
        Line::from(vec![
            Span::styled("o ", Style::default().fg(Color::Cyan)),
            Span::raw(
                "Experimental mode is enabled. These features are not stable, may have bugs, and may be removed in the future.",
            ),
        ]),
        Line::from(vec![
            Span::styled("o ", Style::default().fg(Color::Blue)),
            Span::raw(format!(
                "Environment loaded: 1 custom instruction, 1 MCP server, 3 skills, 2 agents in {}.",
                app.workspace_name
            )),
        ]),
    ]);
    frame.render_widget(details, sections[1]);
}

fn render_console_stream(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let lines = if app.show_console_history {
        console_lines(app)
    } else {
        vec![
            Line::from(""),
            Line::from(vec![Span::styled(
                "No active conversation on this route yet.",
                Style::default().fg(Color::DarkGray),
            )]),
            Line::from(vec![Span::styled(
                "Send a task, open the resume browser, or start with a slash command.",
                Style::default().fg(Color::DarkGray),
            )]),
        ]
    };
    let widget = Paragraph::new(lines)
        .scroll((app.timeline_scroll, 0))
        .wrap(Wrap { trim: false });
    frame.render_widget(widget, area);
}

fn render_workspace_line(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let left = format!(
        "{} [/_{}*]",
        display_workspace_path(&app.workspace_path),
        app.workspace_name
    );
    let right = format!("{} ({}) (1x)", app.control_model, app.runtime_status);

    let widget = Paragraph::new(Line::from(vec![
        Span::raw(&left),
        Span::raw(pad_between(area.width, left.len(), right.len())),
        Span::styled(&right, Style::default().fg(Color::DarkGray)),
    ]));
    frame.render_widget(widget, area);
}

fn render_composer(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let draft = app.active_draft();
    let placeholder =
        "Type @ to mention files, # for issues/PRs, / for commands, or ? for shortcuts";
    let input = if draft.is_empty() { placeholder } else { draft };
    let input_style = if draft.is_empty() {
        Style::default().fg(Color::DarkGray)
    } else {
        Style::default()
    };

    let widget = Paragraph::new(Line::from(vec![
        Span::styled("> ", Style::default().fg(Color::Cyan)),
        Span::styled(input, input_style),
    ]))
    .block(
        Block::default()
            .borders(Borders::TOP | Borders::BOTTOM)
            .border_style(Style::default().fg(Color::DarkGray)),
    )
    .wrap(Wrap { trim: false });
    frame.render_widget(widget, area);

    if app.focus == crate::app::Focus::Composer && !app.show_help_overlay {
        let max_cursor_x = area.right().saturating_sub(2);
        let cursor_offset = if draft.is_empty() {
            0
        } else {
            draft.chars().count() as u16
        };
        let cursor_x = area
            .x
            .saturating_add(3)
            .saturating_add(cursor_offset)
            .min(max_cursor_x);
        let cursor_y = area.y.saturating_add(1);
        frame.set_cursor_position((cursor_x, cursor_y));
    }
}

fn render_footer(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let hints = "shift+tab switch mode";
    let request_count = if app.show_console_history {
        app.active_session().unread
    } else {
        0
    };
    let summary = format!("{request_count} reqs.");
    let widget = Paragraph::new(Line::from(vec![
        Span::styled(hints, Style::default().fg(Color::DarkGray)),
        Span::raw(pad_between(area.width, hints.len(), summary.len())),
        Span::styled(&summary, Style::default().fg(Color::DarkGray)),
    ]));
    frame.render_widget(widget, area);
}

fn render_command_palette(frame: &mut Frame<'_>, app: &App, composer_area: Rect) {
    let commands = matching_commands(app.command_query().unwrap_or_default());
    let height = commands.len().min(8) as u16 + 2;
    let area = Rect {
        x: composer_area.x,
        y: composer_area.y.saturating_sub(height),
        width: composer_area.width,
        height,
    };

    let mut lines = Vec::new();
    if commands.is_empty() {
        lines.push(Line::from(vec![Span::styled(
            "No matching local commands.",
            Style::default().fg(Color::Yellow),
        )]));
    } else {
        for (index, (command, description)) in commands.iter().enumerate() {
            let selected = index == app.command_menu_index.min(commands.len().saturating_sub(1));
            let style = if selected {
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            lines.push(Line::from(vec![
                Span::styled(if selected { "| " } else { "  " }, style),
                Span::styled(format!("{:<25}", command), style),
                Span::styled(description.to_string(), Style::default().fg(Color::Gray)),
            ]));
        }
    }

    let widget = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::LEFT | Borders::TOP | Borders::BOTTOM)
                .border_style(Style::default().fg(Color::DarkGray)),
        )
        .wrap(Wrap { trim: false });

    frame.render_widget(Clear, area);
    frame.render_widget(widget, area);
}

fn render_help_overlay(frame: &mut Frame<'_>) {
    let area = centered_rect(frame.area(), 74, 20);
    let lines = vec![
        Line::from(vec![Span::styled(
            "Keyboard",
            Style::default().add_modifier(Modifier::BOLD),
        )]),
        Line::from("  Tab               cycle focus inside the console"),
        Line::from("  Shift+Tab         open the resume browser from the console"),
        Line::from("  j / k / arrows    move through the active list or stream"),
        Line::from("  r                 open the resume browser"),
        Line::from("  Enter             send the current draft or select a session"),
        Line::from("  Ctrl+L            show or hide the activity feed"),
        Line::from("  F1                open or close this help"),
        Line::from("  Esc               leave help, resume, or search"),
        Line::from("  Ctrl+C            quit the TUI"),
        Line::from(""),
        Line::from(vec![Span::styled(
            "Local Commands",
            Style::default().add_modifier(Modifier::BOLD),
        )]),
        Line::from("  /help"),
        Line::from("  /logs"),
        Line::from("  /gateway connect | /gateway disconnect"),
        Line::from("  /runtime <status>"),
        Line::from("  /session state <active|waiting|degraded>"),
        Line::from("  /session model <name>"),
    ];

    let widget = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan))
                .title("Operator Help"),
        )
        .wrap(Wrap { trim: false });

    frame.render_widget(Clear, area);
    frame.render_widget(widget, area);
}

fn console_lines(app: &App) -> Vec<Line<'_>> {
    let session = app.active_session();
    let mut lines = vec![
        Line::from(vec![
            Span::styled("Session ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!("{} ({})", session.title, session.id),
                Style::default().add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::styled("Route ", Style::default().fg(Color::DarkGray)),
            Span::raw(session.route.as_str()),
            Span::raw("  "),
            Span::styled("Channel ", Style::default().fg(Color::DarkGray)),
            Span::raw(session.channel.as_str()),
            Span::raw("  "),
            Span::styled("Runtime ", Style::default().fg(Color::DarkGray)),
            Span::raw(session.runtime.as_str()),
            Span::raw("  "),
            Span::styled("State ", Style::default().fg(Color::DarkGray)),
            Span::raw(session.state.label()),
        ]),
        Line::from(""),
    ];

    for entry in &session.timeline {
        lines.extend(stream_entry_lines(entry));
    }

    if app.show_observability {
        lines.push(Line::from(""));
        lines.push(Line::from(vec![Span::styled(
            "Activity Feed",
            Style::default().add_modifier(Modifier::BOLD),
        )]));
        lines.push(Line::from(""));

        for entry in app.activity.iter().rev().take(5).rev() {
            lines.push(Line::from(vec![
                Span::styled(
                    format!("[{}] ", entry.timestamp),
                    Style::default().fg(Color::DarkGray),
                ),
                Span::styled(
                    format!("{:<7}", entry.scope),
                    Style::default().fg(Color::Cyan),
                ),
                Span::raw(entry.message.as_str()),
            ]));
        }
    }

    lines
}

fn stream_entry_lines(entry: &TimelineEntry) -> Vec<Line<'_>> {
    let (marker, style) = match entry.kind {
        TimelineKind::Operator => (">", Style::default().fg(Color::Cyan)),
        TimelineKind::Agent => ("+", Style::default().fg(Color::Green)),
        TimelineKind::Tool => ("!", Style::default().fg(Color::Yellow)),
        TimelineKind::System => ("o", Style::default().fg(Color::Magenta)),
    };

    vec![
        Line::from(vec![
            Span::styled(format!("{} ", marker), style.add_modifier(Modifier::BOLD)),
            Span::styled(
                entry.title.as_str(),
                Style::default().add_modifier(Modifier::BOLD),
            ),
            Span::raw("  "),
            Span::styled(
                entry.timestamp.as_str(),
                Style::default().fg(Color::DarkGray),
            ),
        ]),
        Line::from(vec![
            Span::styled("  actor ", Style::default().fg(Color::DarkGray)),
            Span::styled(entry.actor.as_str(), Style::default().fg(Color::Gray)),
            Span::raw("  "),
            Span::styled("phase ", Style::default().fg(Color::DarkGray)),
            Span::styled(entry.kind.label(), Style::default().fg(Color::DarkGray)),
            Span::raw("  "),
            Span::styled(entry.body.as_str(), Style::default().fg(Color::Gray)),
        ]),
        Line::from(""),
    ]
}

fn resume_tabs_line(scope: ResumeScope) -> Line<'static> {
    let tab = |value: ResumeScope| {
        if value == scope {
            Span::styled(
                value.label().to_string(),
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )
        } else {
            Span::styled(
                value.label().to_string(),
                Style::default().fg(Color::DarkGray),
            )
        }
    };

    Line::from(vec![
        Span::styled("Sessions ", Style::default().fg(Color::DarkGray)),
        tab(ResumeScope::Local),
        Span::raw("  "),
        tab(ResumeScope::Remote),
        Span::raw("  "),
        tab(ResumeScope::All),
        Span::styled("  (Tab to cycle)", Style::default().fg(Color::DarkGray)),
    ])
}

fn resume_table_header() -> Line<'static> {
    Line::from(vec![
        Span::styled("#   ", Style::default().fg(Color::DarkGray)),
        Span::styled("Type     ", Style::default().fg(Color::DarkGray)),
        Span::styled("Modified  ", Style::default().fg(Color::DarkGray)),
        Span::styled("Created   ", Style::default().fg(Color::DarkGray)),
        Span::styled("Summary", Style::default().fg(Color::DarkGray)),
    ])
}

fn resume_row_line(app: &App, index: usize) -> Line<'_> {
    let session = &app.sessions[index];
    let selected = index == app.selected_session;
    let prefix = if selected { "> " } else { "  " };
    let row_style = if selected {
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    };

    Line::from(vec![
        Span::styled(prefix, row_style),
        Span::styled(
            format!("{:<2}  ", index + 1),
            Style::default().fg(Color::DarkGray),
        ),
        Span::styled(format!("{:<8} ", session.origin), row_style),
        Span::styled(format!("{:<9} ", session.modified), row_style),
        Span::styled(format!("{:<9} ", session.created), row_style),
        Span::styled(session.title.as_str(), row_style),
    ])
}

fn centered_rect(area: Rect, width: u16, height: u16) -> Rect {
    let width = width.min(area.width.saturating_sub(2)).max(1);
    let height = height.min(area.height.saturating_sub(2)).max(1);

    Rect {
        x: area.x + area.width.saturating_sub(width) / 2,
        y: area.y + area.height.saturating_sub(height) / 2,
        width,
        height,
    }
}

fn display_workspace_path(path: &str) -> String {
    std::env::var("HOME")
        .ok()
        .and_then(|home| path.strip_prefix(&home).map(|suffix| format!("~{suffix}")))
        .unwrap_or_else(|| path.to_owned())
}

fn pad_between(width: u16, left_len: usize, right_len: usize) -> String {
    let total = left_len + right_len;
    let padding = width as usize;
    if padding > total + 1 {
        " ".repeat(padding - total)
    } else {
        " ".to_owned()
    }
}

#[cfg(test)]
mod tests {
    use ratatui::{Terminal, backend::TestBackend};

    use super::render;
    use crate::app::{App, Focus, Surface};

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
    fn startup_canvas_renders_welcome_environment_and_footer() {
        let app = App::new("/tmp/mosaic".into());
        let screen = render_to_text(&app, 140, 32);

        assert!(screen.contains("Mosaic Copilot"));
        assert!(screen.contains("v0.1.0"));
        assert!(screen.contains("[ ] [ ]"));
        assert!(screen.contains("Describe a task to get started."));
        assert!(screen.contains("Environment loaded"));
        assert!(screen.contains("No active conversation on this route yet."));
        assert!(screen.contains("[/_mosaic*]"));
        assert!(screen.contains("shift+tab switch mode"));
        assert!(screen.contains("0 reqs."));
    }

    #[test]
    fn resume_surface_renders_session_table() {
        let app = App::new_with_resume("/tmp/mosaic".into(), true);
        let screen = render_to_text(&app, 140, 32);

        assert!(screen.contains("Select a session to resume:"));
        assert!(screen.contains("Sessions Local"));
        assert!(screen.contains("This branch"));
        assert!(screen.contains("Other sessions"));
        assert!(screen.contains("macOS node prep"));
        assert!(screen.contains("Modified"));
        assert!(screen.contains("Gateway routing soak"));
    }

    #[test]
    fn command_palette_appears_for_slash_input() {
        let mut app = App::new("/tmp/mosaic".into());
        app.focus = Focus::Composer;
        app.active_session_mut().draft = "/gate".to_owned();

        let screen = render_to_text(&app, 140, 32);

        assert!(screen.contains("| /gateway connect"));
        assert!(screen.contains("/gateway connect"));
        assert!(screen.contains("/gateway disconnect"));
    }

    #[test]
    fn help_overlay_renders_shortcuts_and_local_commands() {
        let mut app = App::new("/tmp/mosaic".into());
        app.show_help_overlay = true;

        let screen = render_to_text(&app, 140, 32);

        assert!(screen.contains("Operator Help"));
        assert!(screen.contains("r                 open the resume browser"));
        assert!(screen.contains("/session model <name>"));
    }

    #[test]
    fn observability_toggle_changes_console_feed() {
        let mut app = App::new("/tmp/mosaic".into());
        app.show_console_history = true;
        let with_feed = render_to_text(&app, 140, 32);
        app.show_observability = false;
        let without_feed = render_to_text(&app, 140, 32);

        assert!(with_feed.contains("Activity Feed"));
        assert!(!without_feed.contains("Activity Feed"));
    }

    #[test]
    fn resume_surface_keeps_console_overlay_hidden() {
        let app = App::new_with_resume("/tmp/mosaic".into(), true);
        let screen = render_to_text(&app, 140, 32);

        assert!(!screen.contains("Activity Feed"));
        assert!(matches!(app.surface, Surface::Resume));
    }
}
