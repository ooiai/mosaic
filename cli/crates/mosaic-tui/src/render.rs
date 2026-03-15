use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap};

use crate::state::TuiState;
use crate::{TuiFocus, TuiStartupContext, short_id};

const MOSAIC_TUI_TITLE: &str = concat!("Mosaic CLI v", env!("CARGO_PKG_VERSION"));
const STARTUP_PLACEHOLDER: &str =
    "Type @ to mention files, # for issues/PRs, / for commands, or ? for shortcuts";

pub(crate) fn render(
    frame: &mut ratatui::Frame,
    state: &TuiState,
    agent_id: Option<&str>,
    profile_name: &str,
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
    let input_area = if startup_surface {
        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(1),
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Length(input_height),
                Constraint::Length(1),
            ])
            .split(area);

        render_startup_main_canvas(frame, state, layout[0], compact);
        render_startup_environment(frame, layout[1], startup_context);
        render_startup_location(frame, layout[2], cwd, startup_context.git_branch.as_deref());
        render_input(frame, state, layout[3], true);
        render_startup_footer(frame, layout[4], startup_context.pending_requests);
        layout[3]
    } else {
        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(1),
                Constraint::Length(1),
                Constraint::Length(input_height),
                Constraint::Length(1),
            ])
            .split(area);

        render_standard_main_canvas(frame, state, layout[0]);
        render_context_strip(
            frame,
            state,
            layout[1],
            cwd,
            profile_name,
            agent_id,
            policy_summary,
        );
        render_input(frame, state, layout[2], false);
        render_footer_chrome(frame, layout[3], compact, state.focus, state.show_inspector);
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

    if should_show_command_palette(state) {
        render_command_palette(frame, state, area, input_area);
    }

    if state.focus == TuiFocus::Input {
        frame.set_cursor_position(input_cursor(input_area, &state.input));
    }
}

fn render_standard_main_canvas(frame: &mut ratatui::Frame, state: &TuiState, area: Rect) {
    match state.focus {
        TuiFocus::Sessions => render_sessions(frame, state, area),
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
        TuiFocus::Sessions => render_sessions(frame, state, area),
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
    let notice = Paragraph::new(vec![Line::from(vec![
        Span::styled("• ", Style::default().fg(Color::LightBlue)),
        Span::styled("experimental mode", Style::default().fg(Color::LightGreen)),
        Span::styled(
            " is enabled. These features are not stable, may have bugs, and may be removed in the future.",
            Style::default().fg(Color::Gray),
        ),
    ])])
    .wrap(Wrap { trim: false });
    frame.render_widget(notice, area);
}

fn render_messages(frame: &mut ratatui::Frame, state: &TuiState, area: Rect) {
    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(2), Constraint::Min(1)])
        .margin(1)
        .split(area);

    let heading = Paragraph::new(Line::styled(
        format!(
            "conversation | session={}",
            state.active_session_id.as_deref().unwrap_or("<new>")
        ),
        Style::default().fg(Color::DarkGray),
    ));
    frame.render_widget(heading, sections[0]);

    let mut lines = Vec::new();
    for entry in &state.messages {
        let role_style = match entry.role.as_str() {
            "user" => Style::default().fg(Color::Cyan),
            "assistant" => Style::default().fg(Color::LightGreen),
            _ => Style::default().fg(Color::Gray),
        };
        lines.push(Line::from(vec![
            Span::styled(
                format!("{} ", entry.role),
                role_style.add_modifier(Modifier::BOLD),
            ),
            Span::raw(entry.text.clone()),
        ]));
        lines.push(Line::raw(""));
    }

    let paragraph = Paragraph::new(lines).wrap(Wrap { trim: false });
    frame.render_widget(paragraph, sections[1]);
}

fn render_sessions(frame: &mut ratatui::Frame, state: &TuiState, area: Rect) {
    let items = if state.sessions.is_empty() {
        vec![ListItem::new(vec![
            Line::raw("<no sessions yet>"),
            Line::styled(
                "  Press Ctrl+N to start a fresh conversation",
                Style::default().fg(Color::Gray),
            ),
        ])]
    } else {
        state
            .sessions
            .iter()
            .map(|entry| {
                let marker = if state
                    .active_session_id
                    .as_ref()
                    .is_some_and(|active| active == &entry.session_id)
                {
                    "*"
                } else {
                    " "
                };
                let runtime = entry.runtime.as_ref().map_or_else(
                    || "<unknown> / <none>".to_string(),
                    |runtime| {
                        format!(
                            "{} / {}",
                            runtime.profile_name,
                            runtime.agent_id.as_deref().unwrap_or("<none>")
                        )
                    },
                );
                ListItem::new(vec![
                    Line::raw(format!(
                        "{marker} {} ({})",
                        short_id(&entry.session_id),
                        entry.event_count
                    )),
                    Line::styled(format!("  {runtime}"), Style::default().fg(Color::Gray)),
                ])
            })
            .collect::<Vec<_>>()
    };

    let list = List::new(items)
        .block(
            Block::default()
                .title("session browser")
                .borders(Borders::ALL)
                .border_style(border_style(state.focus == TuiFocus::Sessions)),
        )
        .highlight_style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        );
    let mut list_state = ListState::default();
    if !state.sessions.is_empty() {
        list_state.select(Some(state.selected_session.min(state.sessions.len() - 1)));
    }
    frame.render_stateful_widget(list, area, &mut list_state);
}

fn render_input(frame: &mut ratatui::Frame, state: &TuiState, area: Rect, startup_surface: bool) {
    let prompt_style = Style::default()
        .fg(if state.running {
            Color::Yellow
        } else {
            if startup_surface {
                Color::White
            } else {
                Color::Cyan
            }
        })
        .add_modifier(Modifier::BOLD);
    let lines = if state.input.is_empty() {
        vec![Line::from(vec![
            Span::styled(if startup_surface { "❯ " } else { "> " }, prompt_style),
            Span::styled(
                compose_input_placeholder(startup_surface, state.running),
                Style::default().fg(Color::Gray),
            ),
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
                            if startup_surface { "❯ " } else { "> " }
                        } else {
                            "  "
                        },
                        prompt_style,
                    ),
                    Span::raw(line.to_string()),
                ])
            })
            .collect::<Vec<_>>()
    };

    let widget = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(border_style(state.focus == TuiFocus::Input)),
        )
        .wrap(Wrap { trim: false });
    frame.render_widget(widget, area);
}

fn render_startup_environment(
    frame: &mut ratatui::Frame,
    area: Rect,
    startup_context: &TuiStartupContext,
) {
    let line = Line::from(vec![
        Span::styled("◎ ", Style::default().fg(Color::LightMagenta)),
        Span::styled(
            compose_startup_environment_line(
                startup_context.custom_instruction_count,
                startup_context.skill_count,
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
) {
    let line = compose_startup_location_line(cwd, branch);
    frame.render_widget(
        Paragraph::new(line).style(Style::default().fg(Color::Gray)),
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
    policy_summary: &str,
) {
    let left = display_cwd(cwd);
    let right = compose_status_line(
        &state.status,
        profile_name,
        agent_id,
        state.active_session_id.as_deref(),
        policy_summary,
    );
    let line = compose_split_line(area.width, &left, &right);
    frame.render_widget(
        Paragraph::new(line).style(Style::default().fg(Color::Gray)),
        area,
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
    skill_count: usize,
) -> String {
    format!(
        "Loading environment: {} custom instruction{}, {} skill{}",
        custom_instruction_count,
        plural_suffix(custom_instruction_count),
        skill_count,
        plural_suffix(skill_count)
    )
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
        format!("{MOSAIC_TUI_TITLE} | /agent /session /new /status")
    } else {
        format!(
            "{MOSAIC_TUI_TITLE} | focus={} | /agent /session /new /status",
            focus_label(focus)
        )
    }
}

pub(crate) fn compose_shortcuts_line(compact: bool, show_inspector: bool) -> String {
    if compact {
        "Tab focus | Shift+Tab reverse | Ctrl+A agents | ? help".to_string()
    } else {
        format!(
            "Tab focus | Shift+Tab reverse | Ctrl+A agents | Ctrl+S sessions | Ctrl+N new | Ctrl+R refresh | Ctrl+I inspector={} | q quit",
            if show_inspector { "on" } else { "off" }
        )
    }
}

pub(crate) fn command_palette_items(input: &str) -> Vec<(&'static str, &'static str)> {
    const COMMANDS: [(&str, &str); 6] = [
        ("/agents", "open the agent picker"),
        ("/agent <id>", "switch the active agent"),
        ("/session", "show the current session id"),
        ("/session <id>", "resume a session by id"),
        ("/new", "start a fresh session"),
        ("/status", "print the active runtime summary"),
    ];

    let trimmed = input.trim();
    if !trimmed.starts_with('/') {
        return Vec::new();
    }
    let token = trimmed.split_whitespace().next().unwrap_or(trimmed);
    COMMANDS
        .into_iter()
        .filter(|(command, _)| token == "/" || command.starts_with(token))
        .collect()
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
    let footer = Paragraph::new(compose_split_line(area.width, &left, &right))
        .style(Style::default().fg(Color::DarkGray));
    frame.render_widget(footer, area);
}

fn render_startup_footer(frame: &mut ratatui::Frame, area: Rect, pending_requests: usize) {
    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(1), Constraint::Length(8)])
        .split(area);
    let left = Paragraph::new(Line::from(vec![
        Span::styled(
            "shift+tab",
            Style::default()
                .fg(Color::Gray)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" switch mode", Style::default().fg(Color::Gray)),
    ]));
    let right = Paragraph::new(format!("{pending_requests} reqs."))
        .style(Style::default().fg(Color::Gray))
        .alignment(Alignment::Right);
    frame.render_widget(left, columns[0]);
    frame.render_widget(right, columns[1]);
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
    area: Rect,
    input_area: Rect,
) {
    let items = command_palette_items(&state.input);
    let rows = if items.is_empty() {
        vec![ListItem::new(vec![
            Line::raw("No matching slash commands."),
            Line::styled(
                "  Try /agent, /agents, /session, /new, or /status",
                Style::default().fg(Color::Gray),
            ),
        ])]
    } else {
        items
            .into_iter()
            .map(|(command, description)| {
                ListItem::new(vec![
                    Line::styled(command, Style::default().fg(Color::Cyan)),
                    Line::styled(format!("  {description}"), Style::default().fg(Color::Gray)),
                ])
            })
            .collect::<Vec<_>>()
    };

    let desired_height = if rows.len() <= 1 {
        5
    } else {
        (rows.len() as u16).saturating_mul(2).saturating_add(2)
    };
    let popup_height = desired_height.min(area.height.saturating_sub(4)).max(5);
    let popup_width = area.width.saturating_sub(4).min(76);
    let popup_y = if input_area.y > area.y.saturating_add(popup_height) {
        input_area.y.saturating_sub(popup_height.saturating_add(1))
    } else {
        area.y.saturating_add(1)
    };
    let popup = Rect {
        x: area.x.saturating_add(2),
        y: popup_y,
        width: popup_width,
        height: popup_height,
    };

    frame.render_widget(Clear, popup);
    let list = List::new(rows).block(
        Block::default()
            .title("slash commands")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan)),
    );
    frame.render_widget(list, popup);
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

    let block = Block::default()
        .title("inspector")
        .borders(Borders::ALL)
        .border_style(border_style(state.focus == TuiFocus::Inspector));
    let list = List::new(items).block(block);
    frame.render_widget(list, area);
}

fn render_help_overlay(frame: &mut ratatui::Frame, area: Rect) {
    let popup = centered_rect(75, 70, area);
    let help = vec![
        Line::from("Keyboard shortcuts"),
        Line::from(""),
        Line::from("Enter                  send message"),
        Line::from("Ctrl+J                 insert newline"),
        Line::from("Tab / Shift+Tab        switch focus"),
        Line::from("Ctrl+A                 open agent picker"),
        Line::from("Ctrl+S                 open session picker"),
        Line::from("Ctrl+N                 new session"),
        Line::from("Ctrl+R                 refresh sessions"),
        Line::from("Ctrl+I                 toggle inspector"),
        Line::from("/agents                open agent picker in input"),
        Line::from("/agent ID              switch active agent in input"),
        Line::from("/session ID            resume a session by id"),
        Line::from("/new                   start a fresh session"),
        Line::from("/status                print active runtime summary"),
        Line::from("?                      toggle this help"),
        Line::from("q / Ctrl+C             quit"),
        Line::from(""),
        Line::from(
            "Focus changes what the main canvas shows: conversation, sessions, or inspector.",
        ),
    ];

    frame.render_widget(Clear, popup);
    let widget = Paragraph::new(help)
        .block(
            Block::default()
                .title("help")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan)),
        )
        .wrap(Wrap { trim: false });
    frame.render_widget(widget, popup);
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
    let popup = centered_rect(62, 60, area);
    let items = if state.sessions.is_empty() {
        vec![ListItem::new("<no sessions>")]
    } else {
        state
            .sessions
            .iter()
            .map(|entry| {
                let runtime = entry.runtime.as_ref().map_or_else(
                    || "<unknown> / <none>".to_string(),
                    |runtime| {
                        format!(
                            "{} / {}",
                            runtime.profile_name,
                            runtime.agent_id.as_deref().unwrap_or("<none>")
                        )
                    },
                );
                ListItem::new(vec![
                    Line::raw(format!(
                        "{} ({})",
                        short_id(&entry.session_id),
                        entry.event_count
                    )),
                    Line::styled(format!("  {runtime}"), Style::default().fg(Color::Gray)),
                ])
            })
            .collect::<Vec<_>>()
    };

    frame.render_widget(Clear, popup);
    let list = List::new(items)
        .block(
            Block::default()
                .title("session picker")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan)),
        )
        .highlight_style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        );
    let mut stateful = ListState::default();
    if !state.sessions.is_empty() {
        stateful.select(Some(state.selected_session.min(state.sessions.len() - 1)));
    }
    frame.render_stateful_widget(list, popup, &mut stateful);
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
