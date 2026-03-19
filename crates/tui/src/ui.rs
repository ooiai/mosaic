use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap},
};

use crate::app::{App, Focus, SessionState, TimelineEntry, TimelineKind};

pub fn render(frame: &mut Frame<'_>, app: &App) {
    let outer = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(10),
            Constraint::Length(5),
        ])
        .split(frame.area());

    render_status_bar(frame, app, outer[0]);

    let body = body_layout(app, outer[1]);
    render_sessions(frame, app, body.sessions);
    render_main_panel(frame, app, body.main);

    if let Some(observability) = body.observability {
        render_observability(frame, app, observability);
    }

    render_composer(frame, app, outer[2]);
}

fn render_status_bar(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let session = app.active_session();
    let status = Line::from(vec![
        Span::styled(
            " MOSAIC ",
            Style::default().fg(Color::Black).bg(Color::Cyan),
        ),
        Span::raw("  "),
        Span::styled("workspace ", Style::default().fg(Color::DarkGray)),
        Span::raw(&app.workspace_name),
        Span::raw("  "),
        Span::styled("session ", Style::default().fg(Color::DarkGray)),
        Span::raw(&session.id),
        Span::raw("  "),
        Span::styled("model ", Style::default().fg(Color::DarkGray)),
        Span::raw(&app.control_model),
        Span::raw("  "),
        Span::styled("runtime ", Style::default().fg(Color::DarkGray)),
        Span::raw(format!("{} {}", app.runtime_status, app.heartbeat_symbol())),
        Span::raw("  "),
        Span::styled("gateway ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            if app.gateway_connected {
                "connected"
            } else {
                "disconnected"
            },
            Style::default().fg(if app.gateway_connected {
                Color::Green
            } else {
                Color::Red
            }),
        ),
        Span::raw("  "),
        Span::styled("focus ", Style::default().fg(Color::DarkGray)),
        Span::raw(app.focus.label()),
    ]);

    let footer = Line::from(vec![
        Span::styled("path ", Style::default().fg(Color::DarkGray)),
        Span::raw(&app.workspace_path),
        Span::raw("  "),
        Span::styled("keys ", Style::default().fg(Color::DarkGray)),
        Span::raw("Tab cycle  i compose  Ctrl+L logs  /help commands  q quit"),
    ]);

    let widget = Paragraph::new(vec![status, footer]).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray))
            .title("Control Plane"),
    );

    frame.render_widget(widget, area);
}

fn render_sessions(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let items = app
        .sessions
        .iter()
        .enumerate()
        .map(|(index, session)| {
            let selected = index == app.selected_session;
            let state_style = match session.state {
                SessionState::Active => Style::default().fg(Color::Green),
                SessionState::Waiting => Style::default().fg(Color::Yellow),
                SessionState::Degraded => Style::default().fg(Color::Red),
            };

            let mut lines = vec![
                Line::from(Span::styled(
                    session.title.as_str(),
                    Style::default().add_modifier(Modifier::BOLD),
                )),
                Line::from(vec![
                    Span::styled(
                        session.state.label(),
                        state_style.add_modifier(Modifier::BOLD),
                    ),
                    Span::raw("  "),
                    Span::raw(session.channel.as_str()),
                    Span::raw("  "),
                    Span::styled(session.route.as_str(), Style::default().fg(Color::DarkGray)),
                ]),
            ];

            let unread_style = if session.unread > 0 {
                Style::default().fg(Color::Cyan)
            } else {
                Style::default().fg(Color::DarkGray)
            };

            let mut status_line = vec![Span::styled(
                format!("{} new events", session.unread),
                unread_style,
            )];

            if !session.draft.is_empty() {
                status_line.push(Span::raw("  "));
                status_line.push(Span::styled(
                    "draft saved",
                    Style::default().fg(Color::Yellow),
                ));
            }

            lines.push(Line::from(status_line));

            let item_style = if selected {
                Style::default().bg(Color::DarkGray)
            } else {
                Style::default()
            };

            ListItem::new(lines).style(item_style)
        })
        .collect::<Vec<_>>();

    let widget = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(border_style(app.focus == Focus::Sessions))
            .title("Sessions"),
    );

    frame.render_widget(widget, area);
}

fn render_main_panel(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(5), Constraint::Min(5)])
        .split(area);

    let session = app.active_session();
    let summary = Paragraph::new(vec![
        Line::from(vec![
            Span::styled(
                session.title.as_str(),
                Style::default().add_modifier(Modifier::BOLD),
            ),
            Span::raw("  "),
            Span::styled(session.id.as_str(), Style::default().fg(Color::DarkGray)),
        ]),
        Line::from(vec![
            Span::styled("route ", Style::default().fg(Color::DarkGray)),
            Span::raw(session.route.as_str()),
            Span::raw("  "),
            Span::styled("runtime ", Style::default().fg(Color::DarkGray)),
            Span::raw(session.runtime.as_str()),
            Span::raw("  "),
            Span::styled("model ", Style::default().fg(Color::DarkGray)),
            Span::raw(session.model.as_str()),
        ]),
        Line::from(vec![
            Span::styled("channel ", Style::default().fg(Color::DarkGray)),
            Span::raw(session.channel.as_str()),
            Span::raw("  "),
            Span::styled("state ", Style::default().fg(Color::DarkGray)),
            Span::raw(session.state.label()),
            Span::raw("  "),
            Span::styled("timeline ", Style::default().fg(Color::DarkGray)),
            Span::raw("local mock control"),
        ]),
    ])
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(border_style(false))
            .title("Session Header"),
    );

    frame.render_widget(summary, sections[0]);

    let timeline_lines = timeline_lines(&session.timeline);
    let timeline = Paragraph::new(timeline_lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(border_style(app.focus == Focus::Timeline))
                .title("Task / Conversation Timeline"),
        )
        .scroll((app.timeline_scroll, 0))
        .wrap(Wrap { trim: false });

    frame.render_widget(timeline, sections[1]);
}

fn render_observability(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let lines = app
        .activity
        .iter()
        .flat_map(|entry| {
            [
                Line::from(vec![
                    Span::styled(
                        format!("[{}] ", entry.timestamp),
                        Style::default().fg(Color::DarkGray),
                    ),
                    Span::styled(
                        entry.scope.as_str(),
                        Style::default()
                            .fg(Color::Cyan)
                            .add_modifier(Modifier::BOLD),
                    ),
                ]),
                Line::from(entry.message.as_str()),
                Line::from(""),
            ]
        })
        .collect::<Vec<_>>();

    let title = if area.width < 22 {
        "Obs"
    } else {
        "Observability"
    };

    let widget = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(border_style(app.focus == Focus::Observability))
                .title(title),
        )
        .scroll((app.observability_scroll, 0))
        .wrap(Wrap { trim: false });

    frame.render_widget(widget, area);
}

fn render_composer(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let lines = vec![
        Line::from(vec![
            Span::styled("> ", Style::default().fg(Color::Cyan)),
            Span::raw(app.active_draft()),
        ]),
        Line::from(Span::styled(
            "Type an operator instruction or /help for local control commands.",
            Style::default().fg(Color::DarkGray),
        )),
    ];

    let widget = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(border_style(app.focus == Focus::Composer))
                .title("Composer"),
        )
        .wrap(Wrap { trim: false });

    frame.render_widget(widget, area);

    if app.focus == Focus::Composer {
        let max_cursor_x = area.right().saturating_sub(2);
        let cursor_x = area
            .x
            .saturating_add(3)
            .saturating_add(app.active_draft().chars().count() as u16)
            .min(max_cursor_x);
        let cursor_y = area.y.saturating_add(1);
        frame.set_cursor_position((cursor_x, cursor_y));
    }
}

fn timeline_lines(entries: &[TimelineEntry]) -> Vec<Line<'_>> {
    let mut lines = Vec::with_capacity(entries.len() * 3);

    for entry in entries {
        let kind_style = match entry.kind {
            TimelineKind::Operator => Style::default().fg(Color::Cyan),
            TimelineKind::Agent => Style::default().fg(Color::Green),
            TimelineKind::Tool => Style::default().fg(Color::Yellow),
            TimelineKind::System => Style::default().fg(Color::Magenta),
        };

        lines.push(Line::from(vec![
            Span::styled(
                format!("[{}] ", entry.timestamp),
                Style::default().fg(Color::DarkGray),
            ),
            Span::styled(
                format!("{} ", entry.kind.label()),
                kind_style.add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("{} ", entry.actor),
                Style::default().fg(Color::Gray),
            ),
            Span::styled(
                entry.title.as_str(),
                Style::default().add_modifier(Modifier::BOLD),
            ),
        ]));
        lines.push(Line::from(entry.body.as_str()));
        lines.push(Line::from(""));
    }

    lines
}

fn border_style(focused: bool) -> Style {
    if focused {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::DarkGray)
    }
}

struct BodyLayout {
    sessions: Rect,
    main: Rect,
    observability: Option<Rect>,
}

fn body_layout(app: &App, area: Rect) -> BodyLayout {
    if app.show_observability {
        let chunks = if area.width >= 120 {
            Layout::default()
                .direction(Direction::Horizontal)
                .constraints([
                    Constraint::Length(30),
                    Constraint::Min(40),
                    Constraint::Length(36),
                ])
                .split(area)
        } else {
            Layout::default()
                .direction(Direction::Horizontal)
                .constraints([
                    Constraint::Percentage(24),
                    Constraint::Percentage(48),
                    Constraint::Percentage(28),
                ])
                .split(area)
        };

        BodyLayout {
            sessions: chunks[0],
            main: chunks[1],
            observability: Some(chunks[2]),
        }
    } else {
        let chunks = if area.width >= 96 {
            Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Length(30), Constraint::Min(40)])
                .split(area)
        } else {
            Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(28), Constraint::Percentage(72)])
                .split(area)
        };

        BodyLayout {
            sessions: chunks[0],
            main: chunks[1],
            observability: None,
        }
    }
}
