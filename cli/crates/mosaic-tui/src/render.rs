use std::time::{Duration, SystemTime, UNIX_EPOCH};

use chrono::{DateTime, Utc};
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap};

use crate::commands::{CommandSuggestion, CommandSuggestionSource, command_suggestions};
use crate::state::TuiState;
use crate::{TuiFocus, TuiStartupContext};

const MOSAIC_TUI_TITLE: &str = concat!("Mosaic CLI v", env!("CARGO_PKG_VERSION"));
const STARTUP_PLACEHOLDER: &str =
    "Type @ to mention files, # for issues/PRs, / for commands, or ? for shortcuts";

pub(crate) fn render(
    frame: &mut ratatui::Frame,
    state: &TuiState,
    agent_id: Option<&str>,
    profile_name: &str,
    model_name: &str,
    policy_summary: &str,
    cwd: &str,
    startup_context: &TuiStartupContext,
) {
    let area = frame.area();
    let compact = area.width < 90;
    let input_height = if state.input.lines().count() > 1 {
        4
    } else {
        3
    };
    let startup_surface = state.show_startup_surface();
    let command_palette_visible = should_show_command_palette(state);
    let palette_items = if command_palette_visible {
        command_suggestions(
            &state.input,
            &state.agents,
            &state.sessions,
            state.active_session_id.as_deref(),
        )
    } else {
        Vec::new()
    };
    let palette_height = command_palette_height(
        area.height,
        state.input.lines().count() as u16,
        palette_items.len() as u16,
    );

    let input_area = if startup_surface {
        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints(if command_palette_visible {
                vec![
                    Constraint::Min(1),
                    Constraint::Length(1),
                    Constraint::Length(1),
                    Constraint::Length(input_height),
                    Constraint::Length(palette_height),
                ]
            } else {
                vec![
                    Constraint::Min(1),
                    Constraint::Length(1),
                    Constraint::Length(1),
                    Constraint::Length(input_height),
                    Constraint::Length(1),
                ]
            })
            .split(area);

        render_startup_main_canvas(frame, state, layout[0], compact);
        render_startup_environment(frame, layout[1], startup_context);
        render_startup_location(
            frame,
            layout[2],
            cwd,
            startup_context.git_branch.as_deref(),
            profile_name,
            agent_id,
            model_name,
            state.running,
        );
        render_input(frame, state, layout[3], true);
        if command_palette_visible {
            render_command_palette(frame, state, &palette_items, layout[4]);
        } else {
            render_startup_footer(frame, layout[4], startup_context.pending_requests);
        }
        layout[3]
    } else {
        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints(if command_palette_visible {
                vec![
                    Constraint::Min(1),
                    Constraint::Length(1),
                    Constraint::Length(input_height),
                    Constraint::Length(palette_height),
                ]
            } else {
                vec![
                    Constraint::Min(1),
                    Constraint::Length(1),
                    Constraint::Length(input_height),
                    Constraint::Length(1),
                ]
            })
            .split(area);

        render_standard_main_canvas(frame, state, layout[0]);
        render_context_strip(
            frame,
            state,
            layout[1],
            cwd,
            profile_name,
            agent_id,
            model_name,
            policy_summary,
        );
        render_input(frame, state, layout[2], false);
        if command_palette_visible {
            render_command_palette(frame, state, &palette_items, layout[3]);
        } else {
            render_footer_chrome(frame, layout[3], compact, state.focus, state.show_inspector);
        }
        layout[2]
    };

    if state.show_help {
        render_help_overlay(frame, area);
    }

    if state.show_agent_picker {
        render_agent_picker(frame, state, area);
    }

    if state.show_session_picker {
        render_session_picker(frame, state, area);
    }

    if state.focus == TuiFocus::Input
        && !state.show_help
        && !state.show_agent_picker
        && !state.show_session_picker
    {
        frame.set_cursor_position(input_cursor(input_area, &state.input));
    }
}

fn render_standard_main_canvas(frame: &mut ratatui::Frame, state: &TuiState, area: Rect) {
    match state.focus {
        TuiFocus::Sessions => render_resume_surface(frame, state, area),
        TuiFocus::Inspector if state.show_inspector => render_inspector(frame, state, area),
        _ => render_messages(frame, state, area),
    }
}

fn render_startup_main_canvas(
    frame: &mut ratatui::Frame,
    state: &TuiState,
    area: Rect,
    compact: bool,
) {
    match state.focus {
        TuiFocus::Sessions => render_resume_surface(frame, state, area),
        TuiFocus::Inspector if state.show_inspector => render_inspector(frame, state, area),
        _ => render_welcome(frame, area, compact),
    }
}

fn render_welcome(frame: &mut ratatui::Frame, area: Rect, compact: bool) {
    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(if compact { 9 } else { 8 }),
            Constraint::Length(1),
            Constraint::Length(2),
            Constraint::Min(1),
        ])
        .margin(1)
        .split(area);

    let card_width = sections[0]
        .width
        .min(if compact { sections[0].width } else { 94 });
    let card_area = Rect {
        x: sections[0].x,
        y: sections[0].y,
        width: card_width,
        height: sections[0].height,
    };
    render_welcome_card(frame, card_area);
    render_startup_notice(frame, sections[2]);
}

fn render_welcome_card(frame: &mut ratatui::Frame, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Magenta));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let sections = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(6), Constraint::Min(1)])
        .margin(1)
        .split(inner);

    let icon = Paragraph::new(vec![
        Line::from(vec![
            Span::styled("[]", Style::default().fg(Color::Cyan)),
            Span::raw(" "),
            Span::styled("[]", Style::default().fg(Color::Cyan)),
        ]),
        Line::from(vec![
            Span::styled("[", Style::default().fg(Color::Magenta)),
            Span::styled("::", Style::default().fg(Color::LightGreen)),
            Span::styled("]", Style::default().fg(Color::Magenta)),
        ]),
    ]);
    frame.render_widget(icon, sections[0]);

    let content = vec![
        Line::styled(
            MOSAIC_TUI_TITLE,
            Style::default()
                .fg(Color::LightMagenta)
                .add_modifier(Modifier::BOLD),
        ),
        Line::styled(
            "Describe a task to get started.",
            Style::default().fg(Color::Gray),
        ),
        Line::raw(""),
        Line::from(vec![
            Span::styled("Tip: ", Style::default().fg(Color::Gray)),
            Span::styled(
                "/agent",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" switch the active agent before sending your first prompt."),
        ]),
        Line::styled(
            "Mosaic uses AI, so always check for mistakes.",
            Style::default().fg(Color::Gray),
        ),
    ];
    frame.render_widget(
        Paragraph::new(content).wrap(Wrap { trim: false }),
        sections[1],
    );
}

fn render_startup_notice(frame: &mut ratatui::Frame, area: Rect) {
    frame.render_widget(notice_paragraph(), area);
}

fn render_resume_surface(frame: &mut ratatui::Frame, state: &TuiState, area: Rect) {
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(1)])
        .split(area);
    frame.render_widget(notice_paragraph(), layout[0]);

    let content_height = resume_surface_height(state.sessions.len(), layout[1].height);
    let body = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(content_height)])
        .split(layout[1]);
    let content = body[1];
    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(4),
            Constraint::Length(1),
        ])
        .split(content);

    frame.render_widget(
        Paragraph::new("Select a session to resume:")
            .style(Style::default().add_modifier(Modifier::BOLD)),
        sections[0],
    );
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("Sessions: ", Style::default().fg(Color::Gray)),
            Span::styled(
                "Local",
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("   "),
            Span::styled("Remote", Style::default().fg(Color::Gray)),
            Span::raw("   "),
            Span::styled("All", Style::default().fg(Color::Gray)),
            Span::styled(" (tab to cycle)", Style::default().fg(Color::Gray)),
        ])),
        sections[1],
    );
    render_resume_session_tables(frame, state, sections[2]);
    frame.render_widget(
        Paragraph::new("↑↓ to navigate • Enter to select • Esc to cancel • / to search")
            .style(Style::default().fg(Color::Gray)),
        sections[3],
    );
}

fn render_resume_session_tables(frame: &mut ratatui::Frame, state: &TuiState, area: Rect) {
    if state.sessions.is_empty() {
        frame.render_widget(
            Paragraph::new(vec![
                Line::raw("<no sessions yet>"),
                Line::styled(
                    "Press Ctrl+N to start a fresh conversation",
                    Style::default().fg(Color::Gray),
                ),
            ]),
            area,
        );
        return;
    }

    let selected = state.selected_session.min(state.sessions.len() - 1);
    let selected_entry = &state.sessions[selected];
    let other_entries = state
        .sessions
        .iter()
        .enumerate()
        .filter(|(index, _)| *index != selected)
        .take(area.height.saturating_sub(8) as usize)
        .collect::<Vec<_>>();
    let other_height = if other_entries.is_empty() {
        0
    } else {
        other_entries.len() as u16 + 2
    };
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints(if other_entries.is_empty() {
            vec![Constraint::Length(3), Constraint::Min(1)]
        } else {
            vec![
                Constraint::Length(3),
                Constraint::Length(other_height),
                Constraint::Min(1),
            ]
        })
        .split(area);

    render_session_group(
        frame,
        "This branch",
        &[(selected + 1, selected_entry)],
        Some(0),
        layout[0],
    );
    if !other_entries.is_empty() {
        render_session_group(
            frame,
            "Other sessions",
            &other_entries
                .iter()
                .map(|(index, entry)| (*index + 1, *entry))
                .collect::<Vec<_>>(),
            None,
            layout[1],
        );
    }
}

fn render_session_group(
    frame: &mut ratatui::Frame,
    title: &str,
    entries: &[(usize, &mosaic_core::session::SessionSummary)],
    highlight_index: Option<usize>,
    area: Rect,
) {
    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(1)])
        .split(area);
    frame.render_widget(
        Paragraph::new(Line::styled(
            {
                let prefix = format!("── {} ", title);
                let dashes = area
                    .width
                    .saturating_sub(prefix.chars().count() as u16) as usize;
                format!("{prefix}{}", "─".repeat(dashes))
            },
            Style::default()
                .fg(Color::LightMagenta)
                .add_modifier(Modifier::BOLD),
        )),
        sections[0],
    );

    let width = sections[1].width.saturating_sub(2);
    let mut items = Vec::with_capacity(entries.len() + 1);
    items.push(ListItem::new(Line::styled(
        compose_session_header(width),
        Style::default()
            .fg(Color::Gray)
            .add_modifier(Modifier::BOLD),
    )));
    items.extend(entries.iter().map(|(row_number, entry)| {
        ListItem::new(compose_session_row_line(width, *row_number, entry))
    }));

    let mut list_state = ListState::default();
    if let Some(highlight) = highlight_index {
        list_state.select(Some(highlight + 1));
    }

    let list = List::new(items).highlight_symbol("› ").highlight_style(
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    );
    frame.render_stateful_widget(list, sections[1], &mut list_state);
}

fn compose_session_header(width: u16) -> String {
    compose_fixed_columns(
        width as usize,
        &[
            ("#", 4),
            ("Type", 8),
            ("Modified", 12),
            ("Created", 12),
            ("Summary", usize::MAX),
        ],
    )
}

fn compose_session_row_line(
    width: u16,
    row_number: usize,
    entry: &mosaic_core::session::SessionSummary,
) -> Line<'static> {
    let summary = entry.title.clone().unwrap_or_else(|| {
        entry.runtime.as_ref().map_or_else(
            || format!("{} events", entry.event_count),
            |runtime| {
                format!(
                    "{} {}",
                    runtime.profile_name,
                    runtime.agent_id.as_deref().unwrap_or("<none>")
                )
            },
        )
    });
    let age = format_relative_timestamp(entry.last_updated);
    let created = format_relative_timestamp(entry.created_at);
    let row = compose_fixed_columns(
        width as usize,
        &[
            (&format!("{row_number}."), 4),
            ("Local", 8),
            (&age, 12),
            (&created, 12),
            (&summary, usize::MAX),
        ],
    );
    Line::raw(row)
}

fn render_messages(frame: &mut ratatui::Frame, state: &TuiState, area: Rect) {
    let activity_height = if state.running || !state.inspector.is_empty() {
        let desired = (state.inspector.len().min(4) as u16)
            .saturating_add(if state.running { 3 } else { 2 })
            .max(4);
        desired.min(area.height.saturating_sub(3).max(1))
    } else {
        0
    };
    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints(if activity_height > 0 {
            vec![
                Constraint::Length(1),
                Constraint::Min(1),
                Constraint::Length(activity_height),
            ]
        } else {
            vec![Constraint::Length(1), Constraint::Min(1)]
        })
        .margin(1)
        .split(area);

    let heading_left = if state.running {
        format!("{} {}", spinner_frame(), animated_waiting_copy())
    } else {
        "Conversation".to_string()
    };
    let heading_right = format!(
        "session={}",
        state.active_session_id.as_deref().unwrap_or("<new>")
    );
    frame.render_widget(
        Paragraph::new(compose_split_line(
            sections[0].width,
            &heading_left,
            &heading_right,
        ))
        .style(Style::default().fg(Color::DarkGray)),
        sections[0],
    );

    let mut lines = Vec::new();
    for entry in &state.messages {
        let (role_label, role_style, detail_style) = match entry.role.as_str() {
            "user" => (
                "You",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
                Style::default().fg(Color::White),
            ),
            "assistant" => (
                "Mosaic",
                Style::default()
                    .fg(Color::LightGreen)
                    .add_modifier(Modifier::BOLD),
                Style::default().fg(Color::White),
            ),
            _ => (
                "System",
                Style::default()
                    .fg(Color::Gray)
                    .add_modifier(Modifier::BOLD),
                Style::default().fg(Color::Gray),
            ),
        };
        lines.push(Line::styled(role_label, role_style));
        for chunk in entry.text.split('\n') {
            lines.push(Line::from(vec![
                Span::styled("  ", Style::default().fg(Color::DarkGray)),
                Span::styled(chunk.to_string(), detail_style),
            ]));
        }
        lines.push(Line::raw(""));
    }

    if lines.is_empty() {
        lines.extend([
            Line::styled(
                "No conversation yet.",
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
            Line::styled(
                "Type a prompt below, or use / to open the command assistant.",
                Style::default().fg(Color::Gray),
            ),
        ]);
    }

    if state.running {
        lines.push(Line::raw(""));
        lines.push(Line::styled(
            "Mosaic",
            Style::default()
                .fg(Color::LightGreen)
                .add_modifier(Modifier::BOLD),
        ));
        lines.push(Line::from(vec![
            Span::styled("  ", Style::default().fg(Color::DarkGray)),
            Span::styled(spinner_frame(), Style::default().fg(Color::LightGreen)),
            Span::raw(" "),
            Span::styled(animated_waiting_copy(), Style::default().fg(Color::Gray)),
        ]));
        if let Some(entry) = state.inspector.last() {
            lines.push(Line::from(vec![
                Span::styled("  ", Style::default().fg(Color::DarkGray)),
                Span::styled("recent activity: ", Style::default().fg(Color::DarkGray)),
                Span::styled(
                    truncate_text(&entry.detail, sections[1].width as usize),
                    Style::default().fg(Color::DarkGray),
                ),
            ]));
        }
    }

    frame.render_widget(
        Paragraph::new(lines).wrap(Wrap { trim: false }),
        sections[1],
    );

    if activity_height > 0 {
        render_activity_panel(frame, state, sections[2]);
    }
}

fn render_input(frame: &mut ratatui::Frame, state: &TuiState, area: Rect, startup_surface: bool) {
    let prompt_style = Style::default()
        .fg(if state.running {
            Color::LightGreen
        } else if startup_surface {
            Color::White
        } else {
            Color::Cyan
        })
        .add_modifier(Modifier::BOLD);
    let prompt = prompt_prefix(startup_surface, state.running);
    let input_hint = if state.running {
        animated_waiting_copy()
    } else {
        compose_input_placeholder(startup_surface, false).to_string()
    };
    let lines = if state.input.is_empty() {
        vec![Line::from(vec![
            Span::styled(prompt.clone(), prompt_style),
            Span::styled(input_hint, Style::default().fg(Color::Gray)),
        ])]
    } else {
        state
            .input
            .split('\n')
            .enumerate()
            .map(|(index, line)| {
                Line::from(vec![
                    Span::styled(
                        if index == 0 {
                            prompt.clone()
                        } else {
                            "  ".to_string()
                        },
                        prompt_style,
                    ),
                    Span::raw(line.to_string()),
                ])
            })
            .collect::<Vec<_>>()
    };

    frame.render_widget(
        Paragraph::new(lines)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(border_style(state.focus == TuiFocus::Input)),
            )
            .wrap(Wrap { trim: false }),
        area,
    );
}

fn render_startup_environment(
    frame: &mut ratatui::Frame,
    area: Rect,
    startup_context: &TuiStartupContext,
) {
    let line = Line::from(vec![
        Span::styled("• ", Style::default().fg(Color::White)),
        Span::styled(
            compose_startup_environment_line(
                startup_context.custom_instruction_count,
                startup_context.mcp_server_count,
                startup_context.skill_count,
                startup_context.agent_count,
            ),
            Style::default().fg(Color::LightMagenta),
        ),
    ]);
    frame.render_widget(Paragraph::new(line), area);
}

fn render_startup_location(
    frame: &mut ratatui::Frame,
    area: Rect,
    cwd: &str,
    branch: Option<&str>,
    profile_name: &str,
    agent_id: Option<&str>,
    model_name: &str,
    running: bool,
) {
    let left = compose_startup_location_line(cwd, branch);
    let right = compose_runtime_badge(model_name, profile_name, agent_id, running);
    frame.render_widget(
        Paragraph::new(compose_split_line(area.width, &left, &right))
            .style(Style::default().fg(Color::Gray)),
        area,
    );
}

fn render_context_strip(
    frame: &mut ratatui::Frame,
    state: &TuiState,
    area: Rect,
    cwd: &str,
    profile_name: &str,
    agent_id: Option<&str>,
    model_name: &str,
    _policy_summary: &str,
) {
    let left = display_cwd(cwd);
    let right = compose_runtime_badge(model_name, profile_name, agent_id, state.running);
    frame.render_widget(
        Paragraph::new(compose_split_line(area.width, &left, &right))
            .style(Style::default().fg(Color::Gray)),
        area,
    );
}

fn render_activity_panel(frame: &mut ratatui::Frame, state: &TuiState, area: Rect) {
    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(1)])
        .split(area);
    frame.render_widget(
        Paragraph::new(Line::styled(
            "activity",
            Style::default()
                .fg(Color::LightMagenta)
                .add_modifier(Modifier::BOLD),
        )),
        sections[0],
    );

    let mut lines = Vec::new();
    if state.running {
        lines.push(Line::from(vec![
            Span::styled(spinner_frame(), Style::default().fg(Color::LightGreen)),
            Span::raw(" "),
            Span::styled(animated_waiting_copy(), Style::default().fg(Color::Gray)),
        ]));
    }
    for entry in state.inspector.iter().rev().take(4).rev() {
        lines.push(Line::from(vec![
            Span::styled(
                format!("{} ", activity_icon(&entry.kind)),
                Style::default().fg(activity_color(&entry.kind)),
            ),
            Span::styled(
                format!("{}:", entry.kind),
                Style::default()
                    .fg(activity_color(&entry.kind))
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" "),
            Span::styled(
                truncate_text(&entry.detail, area.width as usize),
                Style::default().fg(Color::Gray),
            ),
        ]));
    }
    frame.render_widget(
        Paragraph::new(lines).wrap(Wrap { trim: false }),
        sections[1],
    );
}

pub(crate) fn compose_status_line(
    detail: &str,
    profile_name: &str,
    agent_id: Option<&str>,
    session_id: Option<&str>,
    policy_summary: &str,
) -> String {
    format!(
        "{} | profile={} | agent={} | session={} | policy={}",
        detail,
        profile_name,
        agent_id.unwrap_or("<none>"),
        session_id.unwrap_or("<new>"),
        policy_summary
    )
}

pub(crate) fn compose_startup_environment_line(
    custom_instruction_count: usize,
    mcp_server_count: usize,
    skill_count: usize,
    agent_count: usize,
) -> String {
    let mut parts = vec![format!(
        "{} custom instruction{}",
        custom_instruction_count,
        plural_suffix(custom_instruction_count)
    )];
    if mcp_server_count > 0 {
        parts.push(format!(
            "{} MCP server{}",
            mcp_server_count,
            plural_suffix(mcp_server_count)
        ));
    }
    parts.push(format!(
        "{} skill{}",
        skill_count,
        plural_suffix(skill_count)
    ));
    parts.push(format!(
        "{} agent{}",
        agent_count,
        plural_suffix(agent_count)
    ));
    format!("Environment loaded: {}", parts.join(", "))
}

pub(crate) fn compose_startup_location_line(cwd: &str, branch: Option<&str>) -> String {
    let cwd = display_cwd(cwd);
    match branch {
        Some(branch) if !branch.is_empty() => format!("{cwd} [{branch}]"),
        _ => cwd,
    }
}

#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn compose_header_line(
    compact: bool,
    focus: TuiFocus,
    _profile_name: &str,
    _agent_id: Option<&str>,
    _policy_summary: &str,
) -> String {
    if compact {
        format!("{MOSAIC_TUI_TITLE} | /help /agent /status")
    } else {
        format!(
            "{MOSAIC_TUI_TITLE} | focus={} | /help /agent /session /status",
            focus_label(focus)
        )
    }
}

pub(crate) fn compose_shortcuts_line(compact: bool, show_inspector: bool) -> String {
    if compact {
        "Tab focus | Shift+Tab reverse | ↑↓ command select | ? help".to_string()
    } else {
        format!(
            "Tab focus | Shift+Tab reverse | ↑↓ command select | Ctrl+A agents | Ctrl+S sessions | Ctrl+N new | Ctrl+R refresh | Ctrl+I inspector={} | q quit",
            if show_inspector { "on" } else { "off" }
        )
    }
}

fn render_footer_chrome(
    frame: &mut ratatui::Frame,
    area: Rect,
    compact: bool,
    focus: TuiFocus,
    show_inspector: bool,
) {
    let left = compose_shortcuts_line(compact, show_inspector);
    let right = format!("focus={}", focus_label(focus));
    frame.render_widget(
        Paragraph::new(compose_split_line(area.width, &left, &right))
            .style(Style::default().fg(Color::DarkGray)),
        area,
    );
}

fn render_startup_footer(frame: &mut ratatui::Frame, area: Rect, pending_requests: usize) {
    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(1), Constraint::Length(8)])
        .split(area);
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(
                "shift+tab",
                Style::default()
                    .fg(Color::Gray)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" switch mode", Style::default().fg(Color::Gray)),
        ])),
        columns[0],
    );
    frame.render_widget(
        Paragraph::new(format!("{pending_requests} reqs."))
            .style(Style::default().fg(Color::Gray))
            .alignment(Alignment::Right),
        columns[1],
    );
}

fn should_show_command_palette(state: &TuiState) -> bool {
    state.focus == TuiFocus::Input
        && !state.show_help
        && !state.show_agent_picker
        && !state.show_session_picker
        && state.input.trim_start().starts_with('/')
}

fn render_command_palette(
    frame: &mut ratatui::Frame,
    state: &TuiState,
    items: &[CommandSuggestion],
    area: Rect,
) {
    let width = area.width.saturating_sub(4);
    let rows = if items.is_empty() {
        vec![ListItem::new(Line::styled(
            "No matching slash commands. Try /help, /status, /models, /skills, /session, or /agents",
            Style::default().fg(Color::Gray),
        ))]
    } else {
        items
            .iter()
            .map(|item| ListItem::new(command_palette_item_lines(width, item)))
            .collect::<Vec<_>>()
    };

    let mut list_state = ListState::default();
    if !items.is_empty() {
        list_state.select(Some(state.command_palette_index.min(items.len() - 1)));
    }
    frame.render_widget(Clear, area);
    let block = Block::default()
        .title("command assistant")
        .borders(Borders::TOP | Borders::LEFT | Borders::RIGHT)
        .border_style(Style::default().fg(Color::DarkGray));
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(inner);
    let list = List::new(rows)
        .highlight_symbol("› ")
        .highlight_style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        );
    frame.render_stateful_widget(list, sections[0], &mut list_state);
    frame.render_widget(
        Paragraph::new(command_assistant_footer(
            sections[1].width,
            items.get(
                state
                    .command_palette_index
                    .min(items.len().saturating_sub(1)),
            ),
        ))
        .style(Style::default().fg(Color::DarkGray)),
        sections[1],
    );
}

fn render_inspector(frame: &mut ratatui::Frame, state: &TuiState, area: Rect) {
    let items = if state.inspector.is_empty() {
        vec![ListItem::new("No tool events.")]
    } else {
        state
            .inspector
            .iter()
            .rev()
            .take(40)
            .map(|entry| ListItem::new(format!("{}: {}", entry.kind, entry.detail)))
            .collect::<Vec<_>>()
    };

    frame.render_widget(
        List::new(items).block(
            Block::default()
                .title("inspector")
                .borders(Borders::ALL)
                .border_style(border_style(state.focus == TuiFocus::Inspector)),
        ),
        area,
    );
}

fn render_help_overlay(frame: &mut ratatui::Frame, area: Rect) {
    let popup = centered_rect(75, 70, area);
    let help = vec![
        Line::from("Keyboard shortcuts"),
        Line::from(""),
        Line::from("Plain input             ask/chat directly in the current session"),
        Line::from("Enter                  send message / execute local command"),
        Line::from("Ctrl+J                 insert newline"),
        Line::from("Tab / Shift+Tab        switch focus"),
        Line::from("↑ / ↓                  move command selection"),
        Line::from("Tab                    autocomplete selected slash command"),
        Line::from("Ctrl+A                 open agent picker"),
        Line::from("Ctrl+S                 open session picker"),
        Line::from("Ctrl+N                 new session"),
        Line::from("Ctrl+R                 refresh sessions"),
        Line::from("Ctrl+I                 toggle inspector"),
        Line::from("/help                  open the fullscreen help overlay"),
        Line::from("/agents                open agent picker in input"),
        Line::from("/agent ID              switch active agent in input"),
        Line::from("/session ID            resume a session by id"),
        Line::from("/clear, /new           start a fresh session"),
        Line::from("/status                print active runtime summary"),
        Line::from("/models /skills /docs  inspect local config, skills, and docs"),
        Line::from("/logs /doctor          show logs and run diagnostics in place"),
        Line::from("/memory /knowledge     inspect memory and knowledge state locally"),
        Line::from("/plugins               inspect installed plugins locally"),
        Line::from("?                      toggle this help"),
        Line::from("q / Ctrl+C             quit"),
        Line::from(""),
        Line::from(
            "Focus changes what the main canvas shows: conversation, sessions, or inspector.",
        ),
    ];

    frame.render_widget(Clear, popup);
    frame.render_widget(
        Paragraph::new(help)
            .block(
                Block::default()
                    .title("help")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Cyan)),
            )
            .wrap(Wrap { trim: false }),
        popup,
    );
}

fn render_agent_picker(frame: &mut ratatui::Frame, state: &TuiState, area: Rect) {
    let popup = centered_rect(55, 55, area);
    let items = state
        .agents
        .iter()
        .map(|entry| {
            let mut details = vec![format!("profile={}", entry.profile_name)];
            if entry.is_default {
                details.push("default".to_string());
            }
            if !entry.route_keys.is_empty() {
                details.push(format!("routes={}", entry.route_keys.join(",")));
            }
            ListItem::new(vec![
                Line::raw(format!("{} ({})", entry.id, entry.name)),
                Line::styled(
                    format!("  {}", details.join(" | ")),
                    Style::default().fg(Color::Gray),
                ),
            ])
        })
        .collect::<Vec<_>>();

    frame.render_widget(Clear, popup);
    let list = List::new(items)
        .block(
            Block::default()
                .title("agent picker")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan)),
        )
        .highlight_style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        );
    let mut stateful = ListState::default();
    if !state.agents.is_empty() {
        stateful.select(Some(state.selected_agent.min(state.agents.len() - 1)));
    }
    frame.render_stateful_widget(list, popup, &mut stateful);
}

fn render_session_picker(frame: &mut ratatui::Frame, state: &TuiState, area: Rect) {
    frame.render_widget(Clear, area);
    render_resume_surface(frame, state, area);
}

fn notice_paragraph() -> Paragraph<'static> {
    Paragraph::new(vec![Line::from(vec![
        Span::styled("• ", Style::default().fg(Color::LightBlue)),
        Span::styled("🧪 ", Style::default().fg(Color::LightGreen)),
        Span::styled(
            "Experimental mode is enabled. These features are not stable, may have bugs, and may be removed in the future.",
            Style::default().fg(Color::Gray),
        ),
    ])])
    .wrap(Wrap { trim: false })
}

fn centered_rect(width_percent: u16, height_percent: u16, area: Rect) -> Rect {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - height_percent) / 2),
            Constraint::Percentage(height_percent),
            Constraint::Percentage((100 - height_percent) / 2),
        ])
        .split(area);
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - width_percent) / 2),
            Constraint::Percentage(width_percent),
            Constraint::Percentage((100 - width_percent) / 2),
        ])
        .split(vertical[1])[1]
}

fn border_style(active: bool) -> Style {
    if active {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::DarkGray)
    }
}

fn focus_label(focus: TuiFocus) -> &'static str {
    match focus {
        TuiFocus::Messages => "messages",
        TuiFocus::Input => "input",
        TuiFocus::Sessions => "sessions",
        TuiFocus::Inspector => "inspector",
    }
}

fn input_cursor(area: Rect, input: &str) -> (u16, u16) {
    let rows = input.split('\n').collect::<Vec<_>>();
    let row_offset = rows.len().saturating_sub(1) as u16;
    let col_offset = rows
        .last()
        .map(|line| line.chars().count() as u16)
        .unwrap_or(0);
    let max_x = area.x + area.width.saturating_sub(2);
    let max_y = area.y + area.height.saturating_sub(2);
    let x = area
        .x
        .saturating_add(3)
        .saturating_add(col_offset)
        .min(max_x);
    let y = area
        .y
        .saturating_add(1)
        .saturating_add(row_offset)
        .min(max_y);
    (x, y)
}

pub(crate) fn compose_input_placeholder(startup_surface: bool, running: bool) -> &'static str {
    if running {
        "Waiting for the current run to finish..."
    } else if startup_surface {
        STARTUP_PLACEHOLDER
    } else {
        "Type / for commands, Ctrl+A for agents, Ctrl+S for sessions, or ? for shortcuts"
    }
}

pub(crate) fn display_cwd(cwd: &str) -> String {
    if let Ok(home) = std::env::var("HOME") {
        if cwd.starts_with(&home) {
            return cwd.replacen(&home, "~", 1);
        }
    }
    cwd.to_string()
}

fn plural_suffix(count: usize) -> &'static str {
    if count == 1 { "" } else { "s" }
}

fn compose_split_line(width: u16, left: &str, right: &str) -> String {
    let width = width as usize;
    if width == 0 {
        return String::new();
    }

    let mut left = left.replace('\n', " ");
    let mut right = right.replace('\n', " ");
    let right_len = right.chars().count();
    if right_len >= width {
        return truncate_text(&right, width);
    }

    let left_budget = width.saturating_sub(right_len + 1);
    left = truncate_text(&left, left_budget);
    let left_len = left.chars().count();
    if left_len + right_len + 1 >= width {
        return format!("{left} {right}");
    }

    let spaces = " ".repeat(width - left_len - right_len);
    right = truncate_text(&right, width.saturating_sub(left_len + spaces.len()));
    format!("{left}{spaces}{right}")
}

fn truncate_text(text: &str, max_width: usize) -> String {
    let current = text.chars().count();
    if current <= max_width {
        return text.to_string();
    }
    if max_width <= 3 {
        return ".".repeat(max_width);
    }
    let prefix = text.chars().take(max_width - 3).collect::<String>();
    format!("{prefix}...")
}

fn command_palette_height(area_height: u16, input_lines: u16, item_count: u16) -> u16 {
    let rows = item_count.clamp(3, 8);
    let desired = (input_lines + rows + 4).clamp(6, 14);
    let max_height = area_height.saturating_sub(3).max(1);
    desired.min(max_height).max(1)
}

fn command_palette_item_lines(width: u16, item: &CommandSuggestion) -> Vec<Line<'static>> {
    let available = width as usize;
    // Split available width: ~40% for the command name, ~60% for description
    let label_width = (available * 2 / 5).max(16);
    let desc_width = available.saturating_sub(label_width + 2);

    let label_cell = truncate_text(&item.label, label_width);
    let label_padded = format!(
        "{:<width$}",
        label_cell,
        width = label_width
    );
    let desc_cell = truncate_text(&item.description, desc_width);

    let line_text = format!("{label_padded}  {desc_cell}");
    vec![Line::from(vec![Span::styled(
        line_text,
        Style::default().fg(command_suggestion_color(&item.source)),
    )])]
}

fn command_assistant_footer(width: u16, selected: Option<&CommandSuggestion>) -> String {
    let default = "↑↓ navigate • Tab complete • Enter accept".to_string();
    let Some(selected) = selected else {
        return default;
    };
    if let Some(shell_hint) = &selected.shell_hint {
        return compose_split_line(
            width,
            "↑↓ navigate • Tab complete",
            &format!("shell: {shell_hint}"),
        );
    }
    let helper = match selected.source {
        CommandSuggestionSource::Agent => "Enter to switch agent",
        CommandSuggestionSource::Session => "Enter to resume session",
        CommandSuggestionSource::Local => "Enter to use command",
        CommandSuggestionSource::Shell => "Run from shell",
    };
    format!("↑↓ navigate • Tab complete • {helper}")
}

fn command_suggestion_badge(source: &CommandSuggestionSource) -> &'static str {
    match source {
        CommandSuggestionSource::Local => "local",
        CommandSuggestionSource::Shell => "shell",
        CommandSuggestionSource::Agent => "agent",
        CommandSuggestionSource::Session => "session",
    }
}

fn command_suggestion_color(source: &CommandSuggestionSource) -> Color {
    match source {
        CommandSuggestionSource::Local => Color::Cyan,
        CommandSuggestionSource::Shell => Color::LightBlue,
        CommandSuggestionSource::Agent => Color::LightGreen,
        CommandSuggestionSource::Session => Color::LightMagenta,
    }
}

fn compose_runtime_badge(
    model_name: &str,
    profile_name: &str,
    agent_id: Option<&str>,
    running: bool,
) -> String {
    // Show model name when available; fall back to profile name
    let base = if !model_name.is_empty() {
        model_name.to_string()
    } else {
        profile_name.replace('_', "-")
    };
    let mut label = if let Some(agent_id) = agent_id.filter(|a| !a.is_empty()) {
        format!("{base} ({agent_id})")
    } else {
        base
    };
    if running {
        label = format!("{} {}{}", spinner_frame(), label, animated_ellipsis());
    }
    label
}

fn prompt_prefix(startup_surface: bool, running: bool) -> String {
    if running {
        format!("{} ", spinner_frame())
    } else if startup_surface {
        "❯ ".to_string()
    } else {
        "> ".to_string()
    }
}

fn spinner_frame() -> &'static str {
    const FRAMES: [&str; 8] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧"];
    let elapsed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::from_millis(0));
    let frame = ((elapsed.as_millis() / 120) as usize) % FRAMES.len();
    FRAMES[frame]
}

fn animated_ellipsis() -> &'static str {
    match animation_phase(4) {
        0 => "",
        1 => ".",
        2 => "..",
        _ => "...",
    }
}

fn animated_waiting_copy() -> String {
    const PHRASES: [&str; 4] = [
        "Mosaic is thinking",
        "Mosaic is planning",
        "Mosaic is checking context",
        "Mosaic is preparing a reply",
    ];
    format!(
        "{}{}",
        PHRASES[animation_phase(PHRASES.len())],
        animated_ellipsis()
    )
}

fn activity_icon(kind: &str) -> &'static str {
    match kind {
        "tool_call" => "→",
        "tool_result" => "✓",
        "error" => "!",
        "agent" => "●",
        _ => "•",
    }
}

fn activity_color(kind: &str) -> Color {
    match kind {
        "tool_call" => Color::LightBlue,
        "tool_result" => Color::LightGreen,
        "error" => Color::LightRed,
        "agent" => Color::Cyan,
        _ => Color::Gray,
    }
}

fn animation_phase(frame_count: usize) -> usize {
    let elapsed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::from_millis(0));
    ((elapsed.as_millis() / 240) as usize) % frame_count.max(1)
}

fn resume_surface_height(session_count: usize, available_height: u16) -> u16 {
    let desired = session_count.min(8) as u16 + 8;
    desired.max(8).min(available_height.max(1))
}

fn compose_fixed_columns(total_width: usize, columns: &[(&str, usize)]) -> String {
    if total_width == 0 {
        return String::new();
    }

    let mut rendered = String::new();
    let mut remaining = total_width;
    for (index, (text, width)) in columns.iter().enumerate() {
        if remaining == 0 {
            break;
        }
        let column_width = if index == columns.len() - 1 || *width == usize::MAX {
            remaining
        } else {
            (*width).min(remaining)
        };
        let cell = truncate_text(text, column_width);
        rendered.push_str(&cell);
        let padding = column_width.saturating_sub(cell.chars().count());
        rendered.push_str(&" ".repeat(padding));
        remaining = remaining.saturating_sub(column_width);
    }
    rendered
}

fn format_relative_timestamp(timestamp: Option<DateTime<Utc>>) -> String {
    let Some(timestamp) = timestamp else {
        return "--".to_string();
    };
    let delta = Utc::now().signed_duration_since(timestamp);
    if delta.num_minutes() < 1 {
        "now".to_string()
    } else if delta.num_hours() < 1 {
        format!("{}m ago", delta.num_minutes())
    } else if delta.num_days() < 1 {
        format!("{}h ago", delta.num_hours())
    } else if delta.num_days() < 7 {
        format!("{}d ago", delta.num_days())
    } else if delta.num_weeks() < 4 {
        format!("{}w ago", delta.num_weeks())
    } else {
        timestamp.format("%Y-%m-%d").to_string()
    }
}
