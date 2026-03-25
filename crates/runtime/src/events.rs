use std::sync::Arc;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum RunEvent {
    RunStarted {
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
    },
    WorkflowStepFinished {
        workflow: String,
        step: String,
    },
    WorkflowStepFailed {
        workflow: String,
        step: String,
        error: String,
    },
    WorkflowFinished {
        name: String,
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
            input: "hello".to_owned(),
        });

        let events_a = sink_a.snapshot();
        let events_b = sink_b.snapshot();

        assert_eq!(events_a.len(), 1);
        assert_eq!(events_b.len(), 1);

        match &events_a[0] {
            RunEvent::RunStarted { input } => assert_eq!(input, "hello"),
            other => panic!("unexpected event: {other:?}"),
        }

        match &events_b[0] {
            RunEvent::RunStarted { input } => assert_eq!(input, "hello"),
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
