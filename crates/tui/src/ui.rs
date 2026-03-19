use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
};

use crate::app::{App, ResumeScope, Surface, TimelineEntry, TimelineKind};

const COMMANDS: [(&str, &str); 6] = [
    ("/help", "Show local control command reference"),
    ("/logs", "Toggle activity feed visibility"),
    ("/gateway connect", "Mark the mock gateway as connected"),
    (
        "/gateway disconnect",
        "Mark the mock gateway as disconnected",
    ),
    ("/runtime <status>", "Set the control runtime status label"),
    (
        "/session state|model",
        "Update the selected session state or model label",
    ),
];

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
            Constraint::Length(8),
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

    lines.push(Line::from(vec![
        Span::styled("#   ", Style::default().fg(Color::DarkGray)),
        Span::styled("Type     ", Style::default().fg(Color::DarkGray)),
        Span::styled("Modified  ", Style::default().fg(Color::DarkGray)),
        Span::styled("Created   ", Style::default().fg(Color::DarkGray)),
        Span::styled("Summary", Style::default().fg(Color::DarkGray)),
    ]));

    if visible.is_empty() {
        lines.push(Line::from(vec![Span::styled(
            "No sessions match the current filter.",
            Style::default().fg(Color::Yellow),
        )]));
    } else {
        for index in visible {
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

            lines.push(Line::from(vec![
                Span::styled(prefix, row_style),
                Span::styled(
                    format!("{:<2}  ", index + 1),
                    Style::default().fg(Color::DarkGray),
                ),
                Span::styled(format!("{:<8} ", session.origin), row_style),
                Span::styled(format!("{:<9} ", session.modified), row_style),
                Span::styled(format!("{:<9} ", session.created), row_style),
                Span::styled(session.title.as_str(), row_style),
            ]));
        }
    }

    let widget = Paragraph::new(lines).wrap(Wrap { trim: false });
    frame.render_widget(widget, outer[1]);

    let footer = Paragraph::new(Line::from(vec![
        Span::raw("j/k navigate  "),
        Span::raw("Enter select  "),
        Span::raw("Esc cancel  "),
        Span::raw("/ search"),
    ]));
    frame.render_widget(footer, outer[2]);
}

fn render_welcome(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(5),
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
    let session = app.active_session();
    let card = Paragraph::new(vec![
        Line::from(vec![
            Span::styled(
                format!("Mosaic Control Plane · {} ", app.workspace_name),
                Style::default().add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                env!("CARGO_PKG_VERSION"),
                Style::default().fg(Color::DarkGray),
            ),
        ]),
        Line::from("Describe a task to get started."),
        Line::from(""),
        Line::from(vec![
            Span::styled("Tip: ", Style::default().fg(Color::DarkGray)),
            Span::styled("/help", Style::default().fg(Color::Cyan)),
            Span::raw(" commands, "),
            Span::styled("r", Style::default().fg(Color::Cyan)),
            Span::raw(" resume browser, "),
            Span::styled("F1", Style::default().fg(Color::Cyan)),
            Span::raw(" shortcuts"),
        ]),
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
            Span::styled("* ", Style::default().fg(Color::Cyan)),
            Span::raw(
                "Experimental local mode is enabled. Runtime, tool, and node feeds remain mock-driven.",
            ),
        ]),
        Line::from(vec![
            Span::styled("* ", Style::default().fg(Color::Blue)),
            Span::raw(format!(
                "Environment loaded: 1 custom instruction, 3 skills, {} sessions, route {}.",
                app.sessions.len(),
                session.route
            )),
        ]),
    ]);
    frame.render_widget(details, sections[1]);
}

fn render_console_stream(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let lines = console_lines(app);
    let widget = Paragraph::new(lines)
        .scroll((app.timeline_scroll, 0))
        .wrap(Wrap { trim: false });
    frame.render_widget(widget, area);
}

fn render_workspace_line(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let session = app.active_session();
    let left = format!("{} [{}]", app.workspace_path, session.route);
    let right = format!(
        "{} ({})  ctrl:{} {} {}",
        session.model,
        session.runtime,
        app.control_model,
        app.runtime_status,
        app.heartbeat_symbol()
    );

    let widget = Paragraph::new(Line::from(vec![
        Span::raw(&left),
        Span::raw(pad_between(area.width, left.len(), right.len())),
        Span::styled(&right, Style::default().fg(Color::DarkGray)),
    ]));
    frame.render_widget(widget, area);
}

fn render_composer(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let draft = app.active_draft();
    let placeholder = "Type @ for files, / for commands, or ? for shortcuts";
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
    let hints = "Tab focus  r resume  F1 help  Ctrl+L activity";
    let summary = format!(
        "{} events  focus:{}  gateway:{}",
        app.activity.len(),
        app.focus.label(),
        if app.gateway_connected { "up" } else { "down" }
    );
    let widget = Paragraph::new(Line::from(vec![
        Span::styled(hints, Style::default().fg(Color::DarkGray)),
        Span::raw(pad_between(area.width, hints.len(), summary.len())),
        Span::styled(&summary, Style::default().fg(Color::DarkGray)),
    ]));
    frame.render_widget(widget, area);
}

fn render_command_palette(frame: &mut Frame<'_>, app: &App, composer_area: Rect) {
    let commands = matching_commands(app.command_query().unwrap_or_default());
    let height = commands.len().min(7) as u16 + 2;
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
            let style = if index == 0 {
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            lines.push(Line::from(vec![
                Span::styled(format!("{:<25}", command), style),
                Span::styled(description.to_string(), Style::default().fg(Color::Gray)),
            ]));
        }
    }

    let widget = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray))
                .title("Commands"),
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
        Line::from("  Tab / Shift+Tab  cycle focus or resume scope tabs"),
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

fn matching_commands(query: &str) -> Vec<(&'static str, &'static str)> {
    let trimmed = query.trim().to_ascii_lowercase();
    COMMANDS
        .into_iter()
        .filter(|(command, _)| {
            trimmed.is_empty() || command.to_ascii_lowercase().contains(&trimmed)
        })
        .collect()
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

        assert!(screen.contains("Mosaic Control Plane"));
        assert!(screen.contains("Describe a task to get started."));
        assert!(screen.contains("Environment loaded"));
        assert!(screen.contains("/tmp/mosaic"));
        assert!(screen.contains("r resume"));
    }

    #[test]
    fn resume_surface_renders_session_table() {
        let app = App::new_with_resume("/tmp/mosaic".into(), true);
        let screen = render_to_text(&app, 140, 32);

        assert!(screen.contains("Select a session to resume:"));
        assert!(screen.contains("Sessions Local"));
        assert!(screen.contains("Modified"));
        assert!(screen.contains("Gateway routing soak"));
    }

    #[test]
    fn command_palette_appears_for_slash_input() {
        let mut app = App::new("/tmp/mosaic".into());
        app.focus = Focus::Composer;
        app.active_session_mut().draft = "/gate".to_owned();

        let screen = render_to_text(&app, 140, 32);

        assert!(screen.contains("Commands"));
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
