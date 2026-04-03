use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
};

use crate::transcript::{
    TranscriptBlock, TranscriptDetailKind, TranscriptView, TurnPhase, body_lines,
    current_timestamp_label,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HistoryCellKey {
    Placeholder,
    Active,
    Committed(usize),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoryCell {
    pub key: HistoryCellKey,
    pub title: String,
    pub phase: Option<TurnPhase>,
    pub detail_summary: Option<String>,
    pub summary_lines: Vec<Line<'static>>,
    pub detail_lines: Vec<Line<'static>>,
    pub expandable: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoryCells {
    pub committed: Vec<HistoryCell>,
    pub active: Option<HistoryCell>,
    pub active_revision: Option<usize>,
    pub streaming_preview: Option<Vec<Line<'static>>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoryDetailView {
    pub key: HistoryCellKey,
    pub title: String,
    pub lines: Vec<Line<'static>>,
}

impl HistoryCells {
    pub fn transcript_key(&self) -> (usize, Option<usize>, bool) {
        (
            self.committed.len(),
            self.active_revision,
            self.streaming_preview.is_some(),
        )
    }

    pub fn summary_lines(&self) -> Vec<Line<'static>> {
        let mut lines = Vec::new();
        for cell in &self.committed {
            lines.extend(cell.summary_lines.clone());
        }
        if let Some(active) = &self.active {
            lines.extend(active.summary_lines.clone());
        }
        if self.active.is_none()
            && let Some(preview) = &self.streaming_preview
        {
            lines.extend(preview.clone());
        }
        lines
    }

    pub fn latest_expandable(&self) -> Option<&HistoryCell> {
        if let Some(active) = &self.active
            && active.expandable
        {
            return Some(active);
        }

        self.committed.iter().rev().find(|cell| cell.expandable)
    }

    pub fn latest_expandable_key(&self) -> Option<HistoryCellKey> {
        self.latest_expandable().map(|cell| cell.key)
    }

    pub fn cell_by_key(&self, key: HistoryCellKey) -> Option<&HistoryCell> {
        match key {
            HistoryCellKey::Placeholder => None,
            HistoryCellKey::Active => self.active.as_ref(),
            HistoryCellKey::Committed(index) => self.committed.get(index),
        }
    }

    pub fn latest_expandable_title(&self) -> Option<&str> {
        self.latest_expandable().map(|cell| cell.title.as_str())
    }

    pub fn resolve_detail_target(
        &self,
        requested: Option<HistoryCellKey>,
    ) -> Option<HistoryCellKey> {
        requested
            .filter(|key| self.cell_by_key(*key).is_some_and(|cell| cell.expandable))
            .or_else(|| self.latest_expandable_key())
    }

    pub fn detail_view(&self, requested: Option<HistoryCellKey>) -> Option<HistoryDetailView> {
        let key = self.resolve_detail_target(requested)?;
        let cell = self.cell_by_key(key)?;
        Some(HistoryDetailView {
            key,
            title: cell.title.clone(),
            lines: cell.detail_lines.clone(),
        })
    }

    pub fn active_banner(&self) -> Option<String> {
        let turn = self.latest_expandable()?;
        let phase = turn.phase?;
        if !phase.is_active() && !matches!(phase, TurnPhase::Failed | TurnPhase::Canceled) {
            return None;
        }
        let detail_summary = turn
            .detail_summary
            .clone()
            .unwrap_or_else(|| "waiting for runtime detail".to_owned());
        Some(format!("{} · {}", phase.label(), detail_summary))
    }
}

pub fn build_history_cells<'a>(transcript: &'a TranscriptView<'a>) -> HistoryCells {
    if transcript.entries.is_empty()
        && transcript.active_entry.is_none()
        && transcript.streaming_preview.is_none()
    {
        return HistoryCells {
            committed: vec![empty_state_cell()],
            active: None,
            active_revision: transcript.active_revision,
            streaming_preview: None,
        };
    }

    let committed = transcript
        .entries
        .iter()
        .enumerate()
        .map(|(index, entry)| {
            build_history_cell_with_key(entry, false, HistoryCellKey::Committed(index))
        })
        .collect::<Vec<_>>();
    let active = transcript
        .active_entry
        .map(|entry| build_history_cell_with_key(entry, true, HistoryCellKey::Active));

    let has_active_streaming_turn = transcript
        .active_entry
        .is_some_and(|entry| matches!(entry.phase, Some(TurnPhase::Streaming)))
        || transcript
            .entries
            .iter()
            .any(|entry| matches!(entry.phase, Some(TurnPhase::Streaming)));
    let streaming_preview = if transcript.active_entry.is_none() && !has_active_streaming_turn {
        transcript
            .streaming_preview
            .map(|preview| streaming_preview_lines(transcript.entries, preview))
    } else {
        None
    };

    HistoryCells {
        committed,
        active,
        active_revision: transcript.active_revision,
        streaming_preview,
    }
}

pub fn transcript_lines<'a>(transcript: &'a TranscriptView<'a>) -> Vec<Line<'a>> {
    build_history_cells(transcript).summary_lines()
}

fn empty_state_cell() -> HistoryCell {
    let mut lines = Vec::new();
    lines.push(Line::from(vec![Span::styled(
        "Mosaic local shell",
        Style::default()
            .fg(Color::White)
            .add_modifier(Modifier::BOLD),
    )]));
    lines.push(Line::from(vec![
        Span::styled("Tip", Style::default().fg(Color::DarkGray)),
        Span::styled("  /", Style::default().fg(Color::Yellow)),
        Span::raw(" opens commands  ·  Enter sends  ·  Ctrl+O opens detail"),
    ]));
    lines.push(Line::from(vec![
        Span::styled("     ", Style::default().fg(Color::DarkGray)),
        Span::raw("Ctrl+T opens transcript overlay"),
    ]));
    lines.push(Line::from(""));
    HistoryCell {
        key: HistoryCellKey::Placeholder,
        title: "Mosaic local shell".to_owned(),
        phase: None,
        detail_summary: None,
        summary_lines: lines.clone(),
        detail_lines: lines,
        expandable: false,
    }
}

fn streaming_preview_lines(
    entries: &[crate::transcript::TimelineEntry],
    preview: &str,
) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::styled(
            format!("{}  ", current_timestamp_label(entries)),
            Style::default().fg(Color::DarkGray),
        ),
        Span::styled(
            "assistant",
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("  [streaming]", Style::default().fg(Color::DarkGray)),
    ]));
    for line in preview.lines() {
        lines.push(Line::from(vec![
            Span::raw("  "),
            Span::raw(line.to_owned()),
        ]));
    }
    lines.push(Line::from(""));
    lines
}

pub fn build_history_cell(entry: &crate::transcript::TimelineEntry, active: bool) -> HistoryCell {
    build_history_cell_with_key(
        entry,
        active,
        if active {
            HistoryCellKey::Active
        } else {
            HistoryCellKey::Committed(0)
        },
    )
}

fn build_history_cell_with_key(
    entry: &crate::transcript::TimelineEntry,
    active: bool,
    key: HistoryCellKey,
) -> HistoryCell {
    let (label, label_style, body_style) = cell_identity(entry);

    let lead = if active { "● " } else { "· " };
    let lead_style = if active {
        Style::default()
            .fg(Color::Green)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let mut header = vec![
        Span::styled(lead, lead_style),
        Span::styled(
            format!("{}  ", entry.timestamp),
            Style::default().fg(Color::DarkGray),
        ),
        Span::styled(format!("[{}]", label), label_style),
    ];
    if let Some(phase) = entry.phase {
        header.push(Span::styled("  ", Style::default().fg(Color::DarkGray)));
        header.push(Span::styled(
            format!("[{}]", phase.label()),
            Style::default().fg(Color::DarkGray),
        ));
    }
    if active {
        header.push(Span::styled("  ", Style::default().fg(Color::DarkGray)));
        header.push(Span::styled(
            "live",
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ));
    }
    header.push(Span::styled("  ·  ", Style::default().fg(Color::DarkGray)));
    header.push(Span::styled(
        entry.title.clone(),
        Style::default().add_modifier(Modifier::BOLD),
    ));
    if !entry.actor.is_empty() && entry.actor != label {
        header.push(Span::styled("  ", Style::default().fg(Color::DarkGray)));
        header.push(Span::styled(
            format!("· {}", entry.actor),
            Style::default().fg(Color::DarkGray),
        ));
    }

    let mut summary_lines = vec![Line::from(header)];
    for line in body_lines(entry) {
        summary_lines.push(Line::from(vec![
            Span::styled("  ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                "│ ",
                Style::default().fg(if active {
                    Color::Green
                } else {
                    Color::DarkGray
                }),
            ),
            Span::styled(line, body_style),
        ]));
    }

    if !entry.details.is_empty() {
        summary_lines.push(detail_summary_line(entry, active));
        if let Some(preview) = detail_preview_line(entry, active) {
            summary_lines.push(preview);
        }
        summary_lines.push(Line::from(vec![
            Span::styled("  ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                if entry.details_expanded {
                    "detail overlay open · Esc closes"
                } else {
                    "Ctrl+O detail"
                },
                Style::default().fg(Color::DarkGray),
            ),
        ]));
    }

    summary_lines.push(Line::from(""));

    HistoryCell {
        key,
        title: entry.title.clone(),
        phase: entry.phase,
        detail_summary: entry
            .details
            .last()
            .map(|detail| format!("{} · {}", detail.kind.label(), detail.title)),
        summary_lines,
        detail_lines: build_detail_lines(entry, label, label_style, body_style),
        expandable: !entry.details.is_empty(),
    }
}

pub fn cell_lines(entry: &crate::transcript::TimelineEntry, active: bool) -> Vec<Line<'static>> {
    build_history_cell(entry, active).summary_lines
}

pub fn detail_lines(entry: &crate::transcript::TimelineEntry) -> Vec<Line<'static>> {
    build_history_cell(entry, false).detail_lines
}

fn build_detail_lines(
    entry: &crate::transcript::TimelineEntry,
    label: &'static str,
    label_style: Style,
    body_style: Style,
) -> Vec<Line<'static>> {
    let mut lines = vec![Line::from(vec![
        Span::styled(
            format!("{}  ", entry.timestamp),
            Style::default().fg(Color::DarkGray),
        ),
        Span::styled(label, label_style),
        Span::styled("  ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            entry.title.clone(),
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ),
    ])];

    if !entry.actor.is_empty() && entry.actor != label {
        lines.push(Line::from(vec![
            Span::styled("actor ", Style::default().fg(Color::DarkGray)),
            Span::styled(entry.actor.clone(), Style::default().fg(Color::Gray)),
        ]));
    }
    if let Some(phase) = entry.phase {
        lines.push(Line::from(vec![
            Span::styled("phase ", Style::default().fg(Color::DarkGray)),
            Span::styled(phase.label(), Style::default().fg(Color::Yellow)),
        ]));
    }
    if let Some(run_id) = &entry.run_id {
        lines.push(Line::from(vec![
            Span::styled("run ", Style::default().fg(Color::DarkGray)),
            Span::raw(run_id.clone()),
        ]));
    }

    let body = body_lines(entry);
    if !body.is_empty() {
        lines.push(Line::from(""));
        for line in body {
            lines.push(Line::from(vec![
                Span::styled("│ ", Style::default().fg(Color::DarkGray)),
                Span::styled(line, body_style),
            ]));
        }
    }

    if !entry.details.is_empty() {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "attached execution detail",
            Style::default()
                .fg(Color::Gray)
                .add_modifier(Modifier::BOLD),
        )));
        for detail in &entry.details {
            lines.push(Line::from(vec![
                Span::styled("• ", Style::default().fg(Color::DarkGray)),
                Span::styled(
                    detail.kind.label(),
                    Style::default().fg(detail_kind_color(detail.kind)),
                ),
                Span::styled("  ", Style::default().fg(Color::DarkGray)),
                Span::raw(detail.title.clone()),
            ]));
            for body_line in detail.body.lines().filter(|line| !line.trim().is_empty()) {
                lines.push(Line::from(vec![
                    Span::styled("    ", Style::default().fg(Color::DarkGray)),
                    Span::styled(body_line.to_owned(), Style::default().fg(Color::Gray)),
                ]));
            }
            lines.push(Line::from(""));
        }
    }

    lines.push(Line::from(vec![Span::styled(
        "Esc closes  ·  Ctrl+O toggles detail",
        Style::default().fg(Color::DarkGray),
    )]));
    lines
}

fn cell_identity(entry: &crate::transcript::TimelineEntry) -> (&'static str, Style, Style) {
    match entry.block {
        TranscriptBlock::UserMessage => (
            "you",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
            Style::default(),
        ),
        TranscriptBlock::AssistantMessage => (
            "assistant",
            Style::default()
                .fg(match entry.phase {
                    Some(TurnPhase::Failed) => Color::Red,
                    Some(TurnPhase::Canceled) => Color::Yellow,
                    Some(TurnPhase::Submitted) | Some(TurnPhase::Queued) => Color::Magenta,
                    Some(TurnPhase::CapabilityActive) => Color::Yellow,
                    _ => Color::Green,
                })
                .add_modifier(Modifier::BOLD),
            Style::default(),
        ),
        TranscriptBlock::SystemNotice => (
            "notice",
            Style::default()
                .fg(Color::Magenta)
                .add_modifier(Modifier::BOLD),
            Style::default().fg(Color::Gray),
        ),
        TranscriptBlock::OperatorResultCard => (
            "result",
            Style::default()
                .fg(Color::Blue)
                .add_modifier(Modifier::BOLD),
            Style::default().fg(Color::Gray),
        ),
        TranscriptBlock::ExecutionCard => (
            "exec",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
            Style::default().fg(Color::Gray),
        ),
        TranscriptBlock::FailureCard => (
            "failure",
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            Style::default().fg(Color::Gray),
        ),
    }
}

fn detail_summary_line(entry: &crate::transcript::TimelineEntry, active: bool) -> Line<'static> {
    let accent = if active {
        Color::Green
    } else {
        Color::DarkGray
    };
    let mut spans = vec![
        Span::styled("  ", Style::default().fg(Color::DarkGray)),
        Span::styled("│ ", Style::default().fg(accent)),
    ];
    let preview_len = entry.details.len().min(3);
    for (index, detail) in entry.details.iter().rev().take(preview_len).enumerate() {
        if index > 0 {
            spans.push(Span::styled("  ", Style::default().fg(Color::DarkGray)));
        }
        spans.extend(detail_chip(detail.kind, &detail.title));
    }
    if entry.details.len() > preview_len {
        spans.push(Span::styled("  ", Style::default().fg(Color::DarkGray)));
        spans.push(Span::styled(
            format!("+{}", entry.details.len() - preview_len),
            Style::default().fg(Color::DarkGray),
        ));
    }
    Line::from(spans)
}

fn detail_preview_line(
    entry: &crate::transcript::TimelineEntry,
    active: bool,
) -> Option<Line<'static>> {
    let accent = if active {
        Color::Green
    } else {
        Color::DarkGray
    };
    let detail = entry.details.last()?;
    let lines = detail
        .body
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>();
    let body = lines
        .iter()
        .find(|line| {
            line.starts_with("route=")
                || line.starts_with("failure=")
                || line.starts_with("next=")
                || line.starts_with("target=")
        })
        .copied()
        .or_else(|| lines.first().copied())?;
    Some(Line::from(vec![
        Span::styled("  ", Style::default().fg(Color::DarkGray)),
        Span::styled("│ ", Style::default().fg(accent)),
        Span::styled(
            truncate_inline(body, 88),
            Style::default().fg(Color::DarkGray),
        ),
    ]))
}

fn detail_chip(kind: TranscriptDetailKind, title: &str) -> Vec<Span<'static>> {
    let label = kind.label();
    let color = detail_kind_color(kind);
    vec![
        Span::styled("[", Style::default().fg(Color::DarkGray)),
        Span::styled(label, Style::default().fg(color)),
        Span::styled("]", Style::default().fg(Color::DarkGray)),
        Span::styled(truncate_inline(title, 36), Style::default().fg(Color::Gray)),
    ]
}

fn detail_kind_color(kind: TranscriptDetailKind) -> Color {
    match kind {
        TranscriptDetailKind::Notice => Color::Blue,
        TranscriptDetailKind::Provider => Color::Magenta,
        TranscriptDetailKind::Tool => Color::Yellow,
        TranscriptDetailKind::Mcp => Color::LightYellow,
        TranscriptDetailKind::Skill => Color::Green,
        TranscriptDetailKind::Workflow => Color::Cyan,
        TranscriptDetailKind::Capability => Color::Yellow,
        TranscriptDetailKind::Sandbox => Color::LightMagenta,
        TranscriptDetailKind::Node => Color::LightCyan,
        TranscriptDetailKind::Failure => Color::Red,
    }
}

fn truncate_inline(value: &str, max: usize) -> String {
    let mut chars = value.chars();
    let truncated = chars.by_ref().take(max).collect::<String>();
    if chars.next().is_some() {
        format!("{truncated}…")
    } else {
        truncated
    }
}

#[cfg(test)]
mod tests {
    use super::{HistoryCellKey, HistoryCells, build_history_cell, cell_lines, detail_lines};
    use crate::transcript::{
        TimelineEntry, TimelineKind, TranscriptBlock, TranscriptDetail, TranscriptDetailKind,
        TranscriptView, TurnPhase,
    };

    fn sample_entry() -> TimelineEntry {
        TimelineEntry {
            timestamp: "12:00".to_owned(),
            kind: TimelineKind::Agent,
            block: TranscriptBlock::AssistantMessage,
            actor: "assistant".to_owned(),
            title: "Working".to_owned(),
            body: "first line\nsecond line".to_owned(),
            run_id: Some("run-123".to_owned()),
            phase: Some(TurnPhase::Streaming),
            details: Vec::new(),
            details_expanded: false,
        }
    }

    #[test]
    fn active_cell_renders_live_marker_and_rail() {
        let lines = cell_lines(&sample_entry(), true);
        let rendered = lines
            .into_iter()
            .map(|line| line.to_string())
            .collect::<Vec<_>>();
        assert!(rendered.iter().any(|line| line.contains("● ")));
        assert!(rendered.iter().any(|line| line.contains("live")));
        assert!(rendered.iter().any(|line| line.contains("│ first line")));
    }

    #[test]
    fn history_cell_builds_summary_and_detail_views() {
        let cell = build_history_cell(&sample_entry(), true);
        let summary = cell
            .summary_lines
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>();
        let detail = cell
            .detail_lines
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>();
        assert!(summary.iter().any(|line| line.contains("live")));
        assert!(detail.iter().any(|line| line.contains("phase streaming")));
        assert!(detail.iter().any(|line| line.contains("run run-123")));
    }

    #[test]
    fn detail_lines_render_execution_detail_with_phase_and_run_id() {
        let mut entry = sample_entry();
        entry.details.push(TranscriptDetail {
            kind: TranscriptDetailKind::Tool,
            title: "time_now".to_owned(),
            body: "target=tool:time_now\nstatus=ok".to_owned(),
        });

        let rendered = detail_lines(&entry)
            .into_iter()
            .map(|line| line.to_string())
            .collect::<Vec<_>>();
        assert!(rendered.iter().any(|line| line.contains("phase streaming")));
        assert!(rendered.iter().any(|line| line.contains("run run-123")));
        assert!(
            rendered
                .iter()
                .any(|line| line.contains("attached execution detail"))
        );
        assert!(rendered.iter().any(|line| line.contains("time_now")));
    }

    #[test]
    fn latest_expandable_prefers_active_cell() {
        let mut committed = sample_entry();
        committed.details.push(TranscriptDetail {
            kind: TranscriptDetailKind::Tool,
            title: "old".to_owned(),
            body: "target=tool:old".to_owned(),
        });
        let mut active = sample_entry();
        active.title = "Active".to_owned();
        active.details.push(TranscriptDetail {
            kind: TranscriptDetailKind::Skill,
            title: "new".to_owned(),
            body: "target=skill:new".to_owned(),
        });
        let view = TranscriptView {
            entries: &[committed],
            active_entry: Some(&active),
            active_revision: Some(3),
            streaming_preview: None,
            scroll: 0,
        };

        let cells = HistoryCells::from(&view);
        let latest = cells
            .latest_expandable()
            .expect("active cell should be expandable");
        let rendered = latest
            .detail_lines
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>();
        assert!(rendered.iter().any(|line| line.contains("Active")));
        assert!(rendered.iter().any(|line| line.contains("new")));
        assert_eq!(cells.latest_expandable_key(), Some(HistoryCellKey::Active));
    }
}

impl<'a> From<&TranscriptView<'a>> for HistoryCells {
    fn from(transcript: &TranscriptView<'a>) -> Self {
        build_history_cells(transcript)
    }
}
