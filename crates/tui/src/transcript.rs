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

/// Maximum number of output lines retained per exec call.
pub const EXEC_MAX_OUTPUT_LINES: usize = 12;

/// Spinner frames for running tool calls.
pub const SPINNER_FRAMES: &[&str] = &["⠋", "⠙", "⠸", "⠴", "⠦", "⠇"];

/// Returns the spinner character for a given tick counter.
pub fn spinner_frame(tick: usize) -> &'static str {
    SPINNER_FRAMES[tick % SPINNER_FRAMES.len()]
}

/// Live state for a single tool call within a turn.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecCallState {
    pub call_id: String,
    pub tool_name: String,
    /// First 120 chars of the call summary/input.
    pub input_summary: String,
    /// Last `EXEC_MAX_OUTPUT_LINES` lines of tool output.
    pub output_lines: Vec<String>,
    /// `None` = still running, `Some(true)` = success, `Some(false)` = failure.
    pub exit_ok: Option<bool>,
    /// Approximate duration label, set on completion.
    pub duration_label: Option<String>,
    pub running: bool,
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
    /// Live tool/exec calls attached to this turn.
    pub exec_calls: Vec<ExecCallState>,
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
    /// When `true`, the view follows new content by scrolling to the bottom.
    /// Set to `false` when the operator manually scrolls up.
    pub follow: bool,
}

impl TranscriptState {
    pub fn new() -> Self {
        Self {
            scroll: 0,
            follow: true,
        }
    }

    pub fn scroll_down(&mut self, amount: u16) {
        self.follow = false;
        self.scroll = self.scroll.saturating_add(amount);
    }

    pub fn scroll_up(&mut self, amount: u16) {
        self.follow = false;
        self.scroll = self.scroll.saturating_sub(amount);
    }

    pub fn scroll_home(&mut self) {
        self.follow = false;
        self.scroll = 0;
    }

    pub fn scroll_end(&mut self) {
        self.follow = true;
        self.scroll = self.scroll.saturating_add(20);
    }

    /// Update scroll to follow new content given total lines and visible height.
    /// Only takes effect when `follow == true`.
    pub fn sync_follow(&mut self, total_lines: u16, visible_height: u16) {
        let max_scroll = total_lines.saturating_sub(visible_height);
        if self.follow {
            self.scroll = max_scroll;
        } else {
            // Clamp manual scroll so it can never go past the last line.
            self.scroll = self.scroll.min(max_scroll);
        }
    }
}

pub struct TranscriptView<'a> {
    pub entries: &'a [TimelineEntry],
    pub active_entry: Option<&'a TimelineEntry>,
    pub active_revision: Option<usize>,
    pub streaming_preview: Option<&'a str>,
    pub scroll: u16,
    /// Current spinner tick for animating running tool calls.
    pub spinner_tick: usize,
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

#[cfg(test)]
mod tests {
    use super::TranscriptState;

    #[test]
    fn scroll_down_is_clamped_to_content_height() {
        let mut state = TranscriptState::new();
        // Manually scroll so follow=false
        state.scroll_down(3);
        assert!(!state.follow);
        // Try to scroll way past the content (total=10 lines, visible=5 → max_scroll=5)
        state.scroll_down(100);
        state.sync_follow(10, 5);
        assert_eq!(state.scroll, 5, "scroll must be clamped to max_scroll");
    }

    #[test]
    fn sync_follow_clamps_on_content_shrink() {
        let mut state = TranscriptState::new();
        state.scroll_down(20); // scroll=20, follow=false
        // Content is now only 8 lines tall, visible=5 → max_scroll=3
        state.sync_follow(8, 5);
        assert_eq!(
            state.scroll, 3,
            "scroll must be clamped when content shrinks"
        );
    }

    #[test]
    fn sync_follow_true_always_pins_to_bottom() {
        let mut state = TranscriptState::new();
        state.scroll = 99;
        state.follow = true;
        state.sync_follow(20, 5);
        assert_eq!(state.scroll, 15);
    }
}
