use std::sync::Arc;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum RunEvent {
    RunStarted {
        run_id: String,
        input: String,
    },
    WorkflowStarted {
        name: String,
        step_count: usize,
    },
    WorkflowStepStarted {
        workflow: String,
        step: String,
        kind: String,
        summary: Option<String>,
    },
    WorkflowStepFinished {
        workflow: String,
        step: String,
        summary: Option<String>,
    },
    WorkflowStepFailed {
        workflow: String,
        step: String,
        error: String,
        summary: Option<String>,
    },
    WorkflowFinished {
        name: String,
    },
    SkillStarted {
        name: String,
        summary: Option<String>,
    },
    SkillFinished {
        name: String,
        summary: Option<String>,
    },
    SkillFailed {
        name: String,
        error: String,
        summary: Option<String>,
    },
    ProviderRequest {
        provider_type: String,
        profile: String,
        model: String,
        tool_count: usize,
        message_count: usize,
        max_attempts: u8,
    },
    ProviderRetry {
        provider_type: String,
        profile: String,
        model: String,
        attempt: u8,
        max_attempts: u8,
        kind: String,
        status_code: Option<u16>,
        retryable: bool,
        error: String,
    },
    ProviderFailed {
        provider_type: String,
        profile: String,
        model: String,
        kind: String,
        status_code: Option<u16>,
        retryable: bool,
        error: String,
    },
    ToolCalling {
        name: String,
        call_id: String,
        summary: Option<String>,
    },
    ToolFinished {
        name: String,
        call_id: String,
        summary: Option<String>,
    },
    ToolFailed {
        name: String,
        call_id: String,
        error: String,
        summary: Option<String>,
    },
    CapabilityJobQueued {
        job_id: String,
        name: String,
        kind: String,
        risk: String,
        permission_scopes: Vec<String>,
    },
    /// Emitted when a capability call needs operator approval before proceeding.
    CapabilityApprovalRequired {
        call_id: String,
        tool_name: String,
        command_preview: String,
        risk_level: String,
    },
    CapabilityJobStarted {
        job_id: String,
        name: String,
    },
    CapabilityJobRetried {
        job_id: String,
        name: String,
        attempt: u8,
        error: String,
    },
    CapabilityJobFinished {
        job_id: String,
        name: String,
        status: String,
        summary: String,
    },
    CapabilityJobFailed {
        job_id: String,
        name: String,
        error: String,
    },
    PermissionCheckFailed {
        name: String,
        call_id: String,
        reason: String,
    },
    OutputDelta {
        run_id: String,
        chunk: String,
        accumulated_chars: usize,
    },
    FinalAnswerReady {
        run_id: String,
    },
    RunFinished {
        run_id: String,
        output_preview: String,
    },
    RunFailed {
        run_id: String,
        error: String,
        failure_kind: Option<String>,
        failure_origin: Option<String>,
    },
    RunCanceled {
        run_id: String,
        reason: String,
    },
    TokenUsage {
        input_tokens: u64,
        output_tokens: u64,
        cached_tokens: u64,
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

#[derive(Default)]
pub struct CompositeEventSink {
    sinks: Vec<SharedRunEventSink>,
}

impl CompositeEventSink {
    pub fn new() -> Self {
        Self { sinks: Vec::new() }
    }

    pub fn with_sink(mut self, sink: SharedRunEventSink) -> Self {
        self.sinks.push(sink);
        self
    }

    pub fn push(&mut self, sink: SharedRunEventSink) {
        self.sinks.push(sink);
    }

    pub fn len(&self) -> usize {
        self.sinks.len()
    }

    pub fn is_empty(&self) -> bool {
        self.sinks.is_empty()
    }
}

impl RunEventSink for CompositeEventSink {
    fn emit(&self, event: RunEvent) {
        for sink in &self.sinks {
            sink.emit(event.clone());
        }
    }
}

pub fn shared_noop_event_sink() -> SharedRunEventSink {
    Arc::new(NoopEventSink)
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use super::*;

    #[derive(Default)]
    struct VecEventSink {
        events: Arc<Mutex<Vec<RunEvent>>>,
    }

    impl VecEventSink {
        fn new() -> Self {
            Self {
                events: Arc::new(Mutex::new(Vec::new())),
            }
        }

        fn snapshot(&self) -> Vec<RunEvent> {
            self.events
                .lock()
                .expect("events lock should not be poisoned")
                .clone()
        }
    }

    impl RunEventSink for VecEventSink {
        fn emit(&self, event: RunEvent) {
            self.events
                .lock()
                .expect("events lock should not be poisoned")
                .push(event);
        }
    }

    #[test]
    fn composite_sink_broadcasts_events_to_all_children() {
        let sink_a = Arc::new(VecEventSink::new());
        let sink_b = Arc::new(VecEventSink::new());

        let composite = CompositeEventSink::new()
            .with_sink(sink_a.clone())
            .with_sink(sink_b.clone());

        composite.emit(RunEvent::RunStarted {
            run_id: "run-1".to_owned(),
            input: "hello".to_owned(),
        });

        let events_a = sink_a.snapshot();
        let events_b = sink_b.snapshot();

        assert_eq!(events_a.len(), 1);
        assert_eq!(events_b.len(), 1);

        match &events_a[0] {
            RunEvent::RunStarted { run_id, input } => {
                assert_eq!(run_id, "run-1");
                assert_eq!(input, "hello");
            }
            other => panic!("unexpected event: {other:?}"),
        }

        match &events_b[0] {
            RunEvent::RunStarted { run_id, input } => {
                assert_eq!(run_id, "run-1");
                assert_eq!(input, "hello");
            }
            other => panic!("unexpected event: {other:?}"),
        }
    }

    #[test]
    fn composite_sink_reports_empty_state() {
        let composite = CompositeEventSink::new();

        assert!(composite.is_empty());
        assert_eq!(composite.len(), 0);
    }

    #[test]
    fn composite_sink_tracks_sink_count() {
        let sink_a = Arc::new(VecEventSink::new());
        let sink_b = Arc::new(VecEventSink::new());

        let composite = CompositeEventSink::new()
            .with_sink(sink_a)
            .with_sink(sink_b);

        assert!(!composite.is_empty());
        assert_eq!(composite.len(), 2);
    }
}
