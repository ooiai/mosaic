use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
};

use crate::app::{CommandCategory, CommandSpec, matching_commands};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandPopupView {
    pub query: String,
    pub commands: Vec<CommandSpec>,
    pub selected: Option<CommandSpec>,
    pub selected_index: usize,
}

pub struct CommandPopupWidget {
    lines: Vec<Line<'static>>,
    key: (String, usize, usize),
    height: u16,
}

impl CommandPopupWidget {
    pub fn new(view: CommandPopupView) -> Self {
        let query = view.query;
        let commands = view.commands;
        let selected = view.selected;
        let mut lines = Vec::new();

        if let Some(command) = selected {
            lines.push(Line::from(vec![
                Span::styled(
                    format!("{} ", display_category(command.category)),
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(command.command, Style::default().fg(Color::Cyan)),
                Span::styled(
                    if command.arg_hint.is_empty() {
                        "".to_owned()
                    } else {
                        format!(" {}", command.arg_hint)
                    },
                    Style::default().fg(Color::DarkGray),
                ),
            ]));
            lines.push(Line::from(vec![Span::styled(
                command.summary,
                Style::default().fg(Color::Gray),
            )]));
            lines.push(Line::from(Span::styled(
                command.detail,
                Style::default().fg(Color::DarkGray),
            )));
            if !command.aliases.is_empty() {
                lines.push(Line::from(Span::styled(
                    format!("aliases {}", command.aliases.join(", ")),
                    Style::default().fg(Color::DarkGray),
                )));
            }
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

        let selected_index = commands.len().saturating_sub(1).min(view.selected_index);
        let window = visible_command_window(commands.len(), selected_index, 8);
        let mut last_category = None;
        for (index, command) in commands
            .iter()
            .enumerate()
            .skip(window.start)
            .take(window.len())
        {
            if last_category != Some(command.category) {
                lines.push(Line::from(vec![
                    Span::styled("  ", Style::default().fg(Color::DarkGray)),
                    Span::styled(
                        display_category(command.category),
                        Style::default()
                            .fg(Color::DarkGray)
                            .add_modifier(Modifier::BOLD),
                    ),
                ]));
                last_category = Some(command.category);
            }
            let is_selected = index == selected_index;
            let marker_style = if is_selected {
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::DarkGray)
            };
            lines.push(Line::from(vec![
                Span::styled(if is_selected { "› " } else { "  " }, marker_style),
                Span::styled(
                    format!("{:<18}", command.command),
                    if is_selected {
                        Style::default()
                            .fg(Color::Cyan)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default()
                    },
                ),
                Span::styled(
                    format!("{:<18}", command.arg_hint),
                    Style::default().fg(Color::DarkGray),
                ),
                Span::styled(command.summary, Style::default().fg(Color::Gray)),
            ]));
        }
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "↵ run  ·  Tab complete  ·  Esc close",
            Style::default().fg(Color::DarkGray),
        )));

        let rows = window.len().clamp(1, 8) as u16;
        let category_rows = commands
            .iter()
            .skip(window.start)
            .take(window.len())
            .map(|command| command.category)
            .fold(Vec::new(), |mut seen, category| {
                if seen.last().copied() != Some(category) {
                    seen.push(category);
                }
                seen
            })
            .len() as u16;
        Self {
            lines,
            key: (query.to_owned(), selected_index, commands.len()),
            height: 6 + rows + category_rows,
        }
    }

    pub fn key(&self) -> (&str, usize, usize) {
        (&self.key.0, self.key.1, self.key.2)
    }

    pub fn height(&self) -> u16 {
        self.height
    }

    pub fn paragraph(&self) -> Paragraph<'static> {
        Paragraph::new(self.lines.clone())
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" Commands ")
                    .border_style(Style::default().fg(Color::Gray)),
            )
            .wrap(Wrap { trim: false })
    }
}

fn visible_command_window(
    total: usize,
    selected: usize,
    max_rows: usize,
) -> std::ops::Range<usize> {
    if total <= max_rows {
        return 0..total;
    }
    let half = max_rows / 2;
    let mut start = selected.saturating_sub(half);
    let mut end = (start + max_rows).min(total);
    if end - start < max_rows {
        start = end.saturating_sub(max_rows);
    }
    end = (start + max_rows).min(total);
    start..end
}

impl CommandPopupView {
    pub fn new(query: String, commands: Vec<CommandSpec>, selected_index: usize) -> Self {
        let selected = commands
            .get(selected_index.min(commands.len().saturating_sub(1)))
            .copied();
        Self {
            query,
            commands,
            selected,
            selected_index,
        }
    }

    pub fn from_query(query: &str, selected_index: usize) -> Self {
        Self::new(query.to_owned(), matching_commands(query), selected_index)
    }
}

pub fn command_popup_rect(frame_area: Rect, composer_area: Rect, height: u16) -> Rect {
    let width = frame_area.width.saturating_sub(8).min(84).max(40);
    let max_x = frame_area.right().saturating_sub(width);
    let x = composer_area.x.saturating_add(2).min(max_x);
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

#[cfg(test)]
mod tests {
    use super::{CommandPopupView, CommandPopupWidget};

    #[test]
    fn command_popup_key_tracks_query_and_selection() {
        let view = CommandPopupView::from_query("sand", 1);
        let widget = CommandPopupWidget::new(view);
        assert_eq!(widget.key(), ("sand", 1, 4));
        assert!(widget.height() >= 6);
    }

    #[test]
    fn command_popup_groups_visible_commands_by_category() {
        let view = CommandPopupView::from_query("", 0);
        let widget = CommandPopupWidget::new(view);
        let text = widget
            .lines
            .iter()
            .map(|line| line.to_string())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(text.contains("UI"));
        assert!(text.contains("Session"));
        assert!(text.contains("↵ run"));
    }
}
