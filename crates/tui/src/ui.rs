use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph, Wrap},
};

use crate::app::{App, ResumeScope, Surface, TimelineEntry, TimelineKind, matching_commands};

pub fn render(frame: &mut Frame<'_>, app: &App) {
    match app.surface {
        Surface::Console => render_console(frame, app),
        Surface::Resume => render_resume(frame, app),
    }

    if app.show_help_overlay {
        render_help_overlay(frame, app);
    }
}

fn render_console(frame: &mut Frame<'_>, app: &App) {
    let palette_height = command_palette_height(app);
    let mut constraints = vec![
        Constraint::Length(11),
        Constraint::Min(8),
        Constraint::Length(1),
        Constraint::Length(3),
    ];
    if palette_height > 0 {
        constraints.push(Constraint::Length(palette_height));
    }
    constraints.push(Constraint::Length(1));

    let outer = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(frame.area());

    let footer_index = outer.len() - 1;
    let palette_area = if palette_height > 0 {
        Some(outer[footer_index - 1])
    } else {
        None
    };

    render_welcome(frame, app, outer[0]);
    render_console_stream(frame, app, outer[1]);
    render_workspace_line(frame, app, outer[2]);
    render_composer(frame, app, outer[3]);
    if let Some(area) = palette_area {
        render_command_palette(frame, app, area);
    }
    render_footer(frame, app, outer[footer_index]);
}

fn render_resume(frame: &mut Frame<'_>, app: &App) {
    let outer = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(9),
            Constraint::Min(10),
            Constraint::Length(1),
        ])
        .split(frame.area());

    let status = if app.is_interactive() {
        Paragraph::new(vec![Line::from(vec![
            Span::styled("• ", Style::default().fg(Color::Cyan)),
            Span::raw(
                "Interactive local session mode is enabled. Enter resumes the selected conversation.",
            ),
        ])])
    } else {
        Paragraph::new(vec![Line::from(vec![
            Span::styled("• ", Style::default().fg(Color::Cyan)),
            Span::raw(
                "Experimental local mode is enabled. Runtime, tool, and node feeds remain mock-driven.",
            ),
        ])])
    };
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
                "  └────────────────────────────",
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
                "  └────────────────────────────",
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
        Span::raw("↑↓ to navigate · "),
        Span::raw("Enter to select · "),
        Span::raw("Esc to cancel · "),
        Span::raw("/ to search"),
    ]));
    frame.render_widget(footer, outer[3]);
}

fn render_welcome(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let mut detail_height = if app.command_query().is_some() { 2 } else { 1 };
    if app.extension_summary.is_some() {
        detail_height += 1;
    }
    if app.extension_policy_summary.is_some() {
        detail_height += 1;
    }
    if !app.extension_errors.is_empty() {
        detail_height += 1;
    }
    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(7),
            Constraint::Length(detail_height),
            Constraint::Min(1),
        ])
        .split(area);

    let card_width = area.width.min(62);
    let card_area = Rect {
        x: area.x,
        y: area.y,
        width: card_width,
        height: sections[0].height,
    };
    let card = Paragraph::new(vec![
        mosaic_mark_top(),
        mosaic_mark_bottom(),
        Line::from(""),
        Line::from(vec![
            Span::styled("Tip: ", Style::default().fg(Color::DarkGray)),
            Span::styled("/help", Style::default().fg(Color::Cyan)),
            Span::styled(
                " Show local commands and shortcuts.",
                Style::default().fg(Color::DarkGray),
            ),
        ]),
        Line::from(vec![Span::styled(
            "Mosaic uses AI, so always check for mistakes.",
            Style::default().fg(Color::DarkGray),
        )]),
    ])
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::Magenta)),
    )
    .wrap(Wrap { trim: false });
    frame.render_widget(card, card_area);

    let mut detail_lines = if app.is_interactive() {
        vec![Line::from(vec![
            Span::styled("• ", Style::default().fg(Color::Cyan)),
            Span::raw(
                "Interactive mode is enabled. Messages are sent to the real local runtime and stored in the current session.",
            ),
        ])]
    } else {
        vec![Line::from(vec![
            Span::styled("• ", Style::default().fg(Color::Cyan)),
            Span::raw(
                "Experimental mode is enabled. These features are not stable, may have bugs, and may be removed in the future.",
            ),
        ])]
    };
    if app.is_interactive() {
        detail_lines.push(Line::from(vec![
            Span::styled("• ", Style::default().fg(Color::Blue)),
            Span::raw(format!(
                "Current profile: {}  model: {}  session: {}",
                app.active_profile(),
                app.control_model,
                app.session_label()
            )),
        ]));
    } else if app.command_query().is_some() {
        detail_lines.push(Line::from(vec![
            Span::styled("• ", Style::default().fg(Color::Blue)),
            Span::raw("Environment loaded: 1 custom instruction, 1 MCP server, 3 skills, 2 agents"),
        ]));
    }
    let details = Paragraph::new(detail_lines);
    frame.render_widget(details, sections[1]);
}

fn render_console_stream(frame: &mut Frame<'_>, app: &App, area: Rect) {
    if app.show_console_history {
        let widget = Paragraph::new(console_lines(app))
            .scroll((app.timeline_scroll, 0))
            .wrap(Wrap { trim: false });
        frame.render_widget(widget, area);
    } else {
        let mut info_height = 1;
        if app.extension_summary.is_some() {
            info_height += 1;
        }
        if app.extension_policy_summary.is_some() {
            info_height += 1;
        }
        if !app.extension_errors.is_empty() {
            info_height += 1;
        }

        let sections = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(1), Constraint::Length(info_height)])
            .split(area);

        frame.render_widget(Paragraph::new(""), sections[0]);
        frame.render_widget(startup_environment_line(app), sections[1]);
    }
}

fn render_workspace_line(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let path = display_workspace_path(&app.workspace_path);
    let badge = format!(" [/_{}*]", app.workspace_name);
    let right = format!(
        "{} ({}) (1x)",
        display_control_model(&app.control_model),
        display_runtime_label(&app.runtime_status)
    );

    let widget = Paragraph::new(Line::from(vec![
        Span::styled(&path, Style::default().fg(Color::Gray)),
        Span::styled(&badge, Style::default().fg(Color::DarkGray)),
        Span::raw(pad_between(
            area.width,
            path.len() + badge.len(),
            right.len(),
        )),
        Span::styled(&right, Style::default().fg(Color::DarkGray)),
    ]));
    frame.render_widget(widget, area);
}

fn startup_environment_line<'a>(app: &'a App) -> Paragraph<'a> {
    let mut lines = vec![if app.is_interactive() {
        Line::from(vec![
            Span::styled("◦ ", Style::default().fg(Color::Magenta)),
            Span::styled(
                format!(
                    "Session {} ready on profile {} ({})",
                    app.session_label(),
                    app.active_profile(),
                    app.control_model
                ),
                Style::default().fg(Color::DarkGray),
            ),
        ])
    } else {
        Line::from(vec![
            Span::styled("◦ ", Style::default().fg(Color::Magenta)),
            Span::styled(
                "Loading environment: 1 custom instruction, 3 skills",
                Style::default().fg(Color::DarkGray),
            ),
        ])
    }];

    if let Some(summary) = app.extension_summary.as_deref() {
        lines.push(Line::from(vec![
            Span::styled("◦ ", Style::default().fg(Color::Cyan)),
            Span::styled(summary, Style::default().fg(Color::DarkGray)),
        ]));
    }

    if let Some(summary) = app.extension_policy_summary.as_deref() {
        lines.push(Line::from(vec![
            Span::styled("◦ ", Style::default().fg(Color::Blue)),
            Span::styled(summary, Style::default().fg(Color::DarkGray)),
        ]));
    }

    if let Some(error) = app.extension_errors.first() {
        lines.push(Line::from(vec![
            Span::styled("◦ ", Style::default().fg(Color::Yellow)),
            Span::styled(
                format!("Extension issue: {}", error),
                Style::default().fg(Color::Yellow),
            ),
        ]));
    }

    Paragraph::new(lines)
}

fn mosaic_mark_top() -> Line<'static> {
    let colors = [Color::Cyan, Color::Magenta, Color::Blue];
    Line::from(vec![
        tile_span('M', colors[0], true),
        Span::raw(" "),
        tile_span('O', colors[1], true),
        Span::raw(" "),
        tile_span('S', colors[2], true),
        Span::raw(" "),
        Span::styled(
            "Mosaic Copilot ".to_owned(),
            Style::default().add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!("v{}", env!("CARGO_PKG_VERSION")),
            Style::default().fg(Color::DarkGray),
        ),
    ])
}

fn mosaic_mark_bottom() -> Line<'static> {
    let colors = [Color::Magenta, Color::Blue, Color::Cyan];
    Line::from(vec![
        tile_span('A', colors[0], false),
        Span::raw(" "),
        tile_span('I', colors[1], false),
        Span::raw(" "),
        tile_span('C', colors[2], false),
        Span::raw(" "),
        Span::styled(
            "Describe a task to get started.",
            Style::default().fg(Color::Gray),
        ),
    ])
}

fn tile_span(ch: char, color: Color, top: bool) -> Span<'static> {
    let tile = if top {
        format!("╭{}╮", ch)
    } else {
        format!("╰{}╯", ch)
    };
    Span::styled(tile, Style::default().fg(color))
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
        Span::styled("› ", Style::default().fg(Color::Cyan)),
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
    let sections = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(10), Constraint::Length(8)])
        .split(area);
    let hints = "shift+tab switch mode";
    let request_count = if app.show_console_history {
        app.active_session().unread
    } else {
        0
    };
    let left = Paragraph::new(Line::from(vec![Span::styled(
        hints,
        Style::default().fg(Color::DarkGray),
    )]));
    frame.render_widget(left, sections[0]);

    let right = Paragraph::new(Line::from(vec![Span::styled(
        format!("{request_count} reqs."),
        Style::default().fg(Color::DarkGray),
    )]));
    frame.render_widget(right, sections[1]);
}

fn command_palette_height(app: &App) -> u16 {
    if app.command_query().is_none() {
        return 0;
    }

    let rows = matching_commands(app.command_query().unwrap_or_default())
        .len()
        .clamp(1, 8) as u16;
    rows + 1
}

fn render_command_palette(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let commands = matching_commands(app.command_query().unwrap_or_default());

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
                Span::styled(if selected { "▎ " } else { "│ " }, style),
                Span::styled(format!("{:<25}", command), style),
                Span::styled(description.to_string(), Style::default().fg(Color::Gray)),
            ]));
        }
    }

    let widget = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::LEFT | Borders::BOTTOM)
                .border_style(Style::default().fg(Color::DarkGray)),
        )
        .wrap(Wrap { trim: false });

    frame.render_widget(Clear, area);
    frame.render_widget(widget, area);
}

fn render_help_overlay(frame: &mut Frame<'_>, app: &App) {
    let area = centered_rect(frame.area(), 74, 24);
    let mut lines = vec![
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
        Line::from("  q                 quit when the composer is not focused"),
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
        Line::from("  /model list"),
        Line::from("  /model use <profile>"),
    ];

    if app.is_interactive() {
        lines.push(Line::from(""));
        lines.push(Line::from(vec![Span::styled(
            "Current Runtime",
            Style::default().add_modifier(Modifier::BOLD),
        )]));
        lines.push(Line::from(format!(
            "  session={}  profile={}  model={}",
            app.session_label(),
            app.active_profile(),
            app.control_model
        )));
    }

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
    ];

    if let Some(summary) = session.memory_summary.as_deref() {
        lines.push(Line::from(vec![
            Span::styled("Summary ", Style::default().fg(Color::DarkGray)),
            Span::raw(truncate_console_value(summary, 180)),
        ]));
    }

    if let Some(compressed) = session.compressed_context.as_deref() {
        lines.push(Line::from(vec![
            Span::styled("Compressed ", Style::default().fg(Color::DarkGray)),
            Span::raw(truncate_console_value(compressed, 180)),
        ]));
    }

    if !session.references.is_empty() {
        lines.push(Line::from(vec![
            Span::styled("References ", Style::default().fg(Color::DarkGray)),
            Span::raw(truncate_console_value(&session.references.join(", "), 180)),
        ]));
    }

    lines.push(Line::from(""));

    for entry in &session.timeline {
        lines.extend(stream_entry_lines(app, entry));
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

fn stream_entry_lines<'a>(app: &'a App, entry: &'a TimelineEntry) -> Vec<Line<'a>> {
    let phase = entry.title.trim();
    let animated = if phase.eq_ignore_ascii_case("thinking") {
        Some((
            ["·", "∙", "•", "∙"][app.pulse_frame()],
            Style::default().fg(Color::Cyan),
        ))
    } else if phase.eq_ignore_ascii_case("working") {
        Some((
            ["∙", "•", "●", "•"][app.pulse_frame()],
            Style::default().fg(Color::Yellow),
        ))
    } else {
        None
    };

    if let Some((marker, style)) = animated {
        vec![
            Line::from(vec![
                Span::styled(format!("{marker} "), style.add_modifier(Modifier::BOLD)),
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
    } else {
        let (marker, style) = match entry.kind {
            TimelineKind::Operator => (">", Style::default().fg(Color::Cyan)),
            TimelineKind::Agent => ("+", Style::default().fg(Color::Green)),
            TimelineKind::Tool => ("!", Style::default().fg(Color::Yellow)),
            TimelineKind::System => ("o", Style::default().fg(Color::Magenta)),
        };

        vec![
            Line::from(vec![
                Span::styled(format!("{marker} "), style.add_modifier(Modifier::BOLD)),
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
        Span::styled("Sessions:", Style::default().fg(Color::DarkGray)),
        Span::raw(" "),
        tab(ResumeScope::Local),
        Span::raw("  "),
        tab(ResumeScope::Remote),
        Span::raw("  "),
        tab(ResumeScope::All),
        Span::styled("  (tab to cycle)", Style::default().fg(Color::DarkGray)),
    ])
}

fn resume_table_header() -> Line<'static> {
    Line::from(vec![
        Span::styled("    #   ", Style::default().fg(Color::DarkGray)),
        Span::styled("Type     ", Style::default().fg(Color::DarkGray)),
        Span::styled("Modified  ", Style::default().fg(Color::DarkGray)),
        Span::styled("Created   ", Style::default().fg(Color::DarkGray)),
        Span::styled("Summary", Style::default().fg(Color::DarkGray)),
    ])
}

fn resume_row_line(app: &App, index: usize) -> Line<'_> {
    let session = &app.sessions[index];
    let selected = index == app.selected_session;
    let prefix = if selected { "▏ " } else { "  " };
    let marker = if selected { "> " } else { "  " };
    let row_style = if selected {
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    };

    Line::from(vec![
        Span::styled(prefix, row_style),
        Span::styled(marker, row_style),
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

fn display_control_model(model: &str) -> &str {
    model.strip_suffix("-control").unwrap_or(model)
}

fn display_runtime_label(status: &str) -> &str {
    if status == "warm" { "xhigh" } else { status }
}

fn truncate_console_value(value: &str, limit: usize) -> String {
    if value.chars().count() <= limit {
        return value.to_owned();
    }

    let truncated: String = value.chars().take(limit).collect();
    format!("{truncated}...")
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

    fn line_index(screen: &str, needle: &str) -> Option<usize> {
        screen.lines().position(|line| line.contains(needle))
    }

    fn line_with<'a>(screen: &'a str, needle: &str) -> Option<&'a str> {
        screen.lines().find(|line| line.contains(needle))
    }

    #[test]
    fn startup_canvas_renders_welcome_environment_and_footer() {
        let app = App::new("/tmp/mosaic".into());
        let screen = render_to_text(&app, 140, 32);

        assert!(screen.contains("Mosaic Copilot"));
        assert!(screen.contains("v0.1.0"));
        assert!(screen.contains("╭M╮"));
        assert!(screen.contains("╭O╮"));
        assert!(screen.contains("╭S╮"));
        assert!(screen.contains("╰A╯"));
        assert!(screen.contains("╰I╯"));
        assert!(screen.contains("╰C╯"));
        assert!(screen.contains("Describe a task to get started."));
        assert!(screen.contains("Loading environment"));
        assert!(screen.contains("[/_mosaic*]"));
        assert!(screen.contains("shift+tab switch mode"));
        assert!(screen.contains("0 reqs."));
    }

    #[test]
    fn startup_canvas_renders_extension_status_lines() {
        let mut app = App::new("/tmp/mosaic".into());
        app.extension_summary = Some("Extensions builtin.core@1.0.0".to_owned());
        app.extension_policy_summary =
            Some("Policies exec=true webhook=true cron=true mcp=true hot_reload=true".to_owned());
        app.extension_errors = vec!["demo.extension: missing_tool".to_owned()];

        let screen = render_to_text(&app, 140, 32);

        assert!(screen.contains("builtin.core@1.0.0"));
        assert!(
            screen.contains("Policies exec=true webhook=true cron=true mcp=true hot_reload=true")
        );
        assert!(screen.contains("missing_tool"));
    }

    #[test]
    fn resume_surface_renders_session_table() {
        let app = App::new_with_resume("/tmp/mosaic".into(), true);
        let screen = render_to_text(&app, 140, 32);

        assert!(screen.contains("Select a session to resume:"));
        assert!(screen.contains("Sessions: Local"));
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

        assert!(screen.contains("▎ /gateway connect"));
        assert!(screen.contains("/gateway connect"));
        assert!(screen.contains("/gateway disconnect"));
        assert!(screen.contains(
            "Environment loaded: 1 custom instruction, 1 MCP server, 3 skills, 2 agents"
        ));

        let composer_line = line_index(&screen, "› /gate").expect("composer line should render");
        let palette_line =
            line_index(&screen, "▎ /gateway connect").expect("palette line should render");
        assert!(palette_line > composer_line);
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
        let with_feed = render_to_text(&app, 140, 40);
        app.show_observability = false;
        let without_feed = render_to_text(&app, 140, 40);

        assert!(with_feed.contains("Activity Feed"));
        assert!(!without_feed.contains("Activity Feed"));
    }

    #[test]
    fn thinking_and_working_markers_animate() {
        let mut app = App::new("/tmp/mosaic".into());
        app.show_console_history = true;

        let first = render_to_text(&app, 140, 32);
        app.tick();
        let second = render_to_text(&app, 140, 32);

        let first_thinking = line_with(&first, "Thinking").expect("thinking line should render");
        let second_thinking = line_with(&second, "Thinking").expect("thinking line should render");
        let first_working = line_with(&first, "Working").expect("working line should render");
        let second_working = line_with(&second, "Working").expect("working line should render");

        assert_ne!(first_thinking, second_thinking);
        assert_ne!(first_working, second_working);
    }

    #[test]
    fn resume_surface_keeps_console_overlay_hidden() {
        let app = App::new_with_resume("/tmp/mosaic".into(), true);
        let screen = render_to_text(&app, 140, 32);

        assert!(!screen.contains("Activity Feed"));
        assert!(matches!(app.surface, Surface::Resume));
    }
}
