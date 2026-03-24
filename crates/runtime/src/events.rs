use std::sync::Arc;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum RunEvent {
    RunStarted {
        input: String,
    },
    SkillStarted {
        name: String,
    },
    SkillFinished {
        name: String,
    },
    SkillFailed {
        name: String,
        error: String,
    },
    ProviderRequest {
        tool_count: usize,
        message_count: usize,
    },
    ToolCalling {
        name: String,
        call_id: String,
    },
    ToolFinished {
        name: String,
        call_id: String,
    },
    ToolFailed {
        name: String,
        call_id: String,
        error: String,
    },
    FinalAnswerReady,
    RunFinished {
        output_preview: String,
    },
    RunFailed {
        error: String,
    },
}

pub trait RunEventSink: Send + Sync {
    fn emit(&self, event: RunEvent);
}

pub type SharedRunEventSink = Arc<dyn RunEventSink>;

#[derive(Debug, Default)]
pub struct NoopEventSink;

impl RunEventSink for NoopEventSink {
    fn emit(&self, _event: RunEvent) {}
}
