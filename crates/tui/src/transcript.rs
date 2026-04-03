#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimelineKind {
    Operator,
    Agent,
    Tool,
    System,
}

impl TimelineKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Operator => "operator",
            Self::Agent => "agent",
            Self::Tool => "tool",
            Self::System => "system",
        }
    }

    pub fn default_block(self) -> TranscriptBlock {
        match self {
            Self::Operator => TranscriptBlock::UserMessage,
            Self::Agent => TranscriptBlock::AssistantMessage,
            Self::Tool => TranscriptBlock::ExecutionCard,
            Self::System => TranscriptBlock::SystemNotice,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TranscriptBlock {
    UserMessage,
    AssistantMessage,
    SystemNotice,
    OperatorResultCard,
    ExecutionCard,
    FailureCard,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TurnPhase {
    Submitted,
    Queued,
    Streaming,
    CapabilityActive,
    WaitingOnCapability,
    Failed,
    Canceled,
    Completed,
}

impl TurnPhase {
    pub fn label(self) -> &'static str {
        match self {
            Self::Submitted => "submitted",
            Self::Queued => "queued",
            Self::Streaming => "streaming",
            Self::CapabilityActive => "capability-active",
            Self::WaitingOnCapability => "waiting-on-capability",
            Self::Failed => "failed",
            Self::Canceled => "canceled",
            Self::Completed => "completed",
        }
    }

    pub fn is_active(self) -> bool {
        matches!(
            self,
            Self::Submitted
                | Self::Queued
                | Self::Streaming
                | Self::CapabilityActive
                | Self::WaitingOnCapability
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TranscriptDetailKind {
    Notice,
    Provider,
    Tool,
    Mcp,
    Skill,
    Workflow,
    Capability,
    Sandbox,
    Node,
    Failure,
}

impl TranscriptDetailKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Notice => "notice",
            Self::Provider => "provider",
            Self::Tool => "tool",
            Self::Mcp => "mcp",
            Self::Skill => "skill",
            Self::Workflow => "workflow",
            Self::Capability => "capability",
            Self::Sandbox => "sandbox",
            Self::Node => "node",
            Self::Failure => "failure",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TranscriptDetail {
    pub kind: TranscriptDetailKind,
    pub title: String,
    pub body: String,
}

#[derive(Debug, Clone)]
pub struct TranscriptCell {
    pub timestamp: String,
    pub kind: TimelineKind,
    pub block: TranscriptBlock,
    pub actor: String,
    pub title: String,
    pub body: String,
    pub run_id: Option<String>,
    pub phase: Option<TurnPhase>,
    pub details: Vec<TranscriptDetail>,
    pub details_expanded: bool,
}

pub type TimelineEntry = TranscriptCell;

#[derive(Debug, Clone)]
pub struct ActiveTurn {
    pub cell: TimelineEntry,
    pub revision: usize,
}

#[derive(Debug, Clone, Default)]
pub struct TranscriptState {
    pub scroll: u16,
}

impl TranscriptState {
    pub fn scroll_down(&mut self, amount: u16) {
        self.scroll = self.scroll.saturating_add(amount);
    }

    pub fn scroll_up(&mut self, amount: u16) {
        self.scroll = self.scroll.saturating_sub(amount);
    }

    pub fn scroll_home(&mut self) {
        self.scroll = 0;
    }

    pub fn scroll_end(&mut self) {
        self.scroll = self.scroll.saturating_add(20);
    }
}

pub struct TranscriptView<'a> {
    pub entries: &'a [TimelineEntry],
    pub active_entry: Option<&'a TimelineEntry>,
    pub active_revision: Option<usize>,
    pub streaming_preview: Option<&'a str>,
    pub scroll: u16,
}

pub fn body_lines(entry: &TimelineEntry) -> Vec<String> {
    if entry.body.is_empty() {
        return Vec::new();
    }

    let mut lines = entry
        .body
        .lines()
        .map(str::trim_end)
        .filter(|line| !line.is_empty())
        .map(str::to_owned)
        .collect::<Vec<_>>();
    let limit = match entry.block {
        TranscriptBlock::UserMessage | TranscriptBlock::AssistantMessage => usize::MAX,
        TranscriptBlock::SystemNotice => 3,
        TranscriptBlock::OperatorResultCard => 4,
        TranscriptBlock::ExecutionCard => 3,
        TranscriptBlock::FailureCard => 4,
    };

    if lines.len() > limit {
        let next_action = matches!(entry.block, TranscriptBlock::FailureCard)
            .then(|| lines.iter().find(|line| line.starts_with("next=")).cloned())
            .flatten();
        lines.truncate(limit);
        if let Some(next_action) = next_action {
            if !lines.iter().any(|line| line == &next_action) {
                if !lines.is_empty() {
                    lines.pop();
                }
                lines.push(next_action);
            }
        } else {
            lines.push("… use /inspect last for deeper detail".to_owned());
        }
    }

    lines
}

pub fn current_timestamp_label(entries: &[TimelineEntry]) -> String {
    entries
        .last()
        .map(|entry| entry.timestamp.clone())
        .unwrap_or_else(|| "now".to_owned())
}
