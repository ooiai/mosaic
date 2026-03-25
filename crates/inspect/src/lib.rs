use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::Result;
use chrono::{DateTime, Utc};
use mosaic_tool_core::ToolSource;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolTrace {
    pub call_id: Option<String>,
    pub name: String,
    #[serde(default)]
    pub source: ToolSource,
    pub input: serde_json::Value,
    pub output: Option<String>,
    pub started_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
}

impl ToolTrace {
    pub fn duration_ms(&self) -> Option<i64> {
        self.finished_at.map(|finished| {
            finished
                .signed_duration_since(self.started_at)
                .num_milliseconds()
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SkillTrace {
    pub name: String,
    pub input: serde_json::Value,
    pub output: Option<String>,
    pub started_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IngressTrace {
    pub kind: String,
    pub channel: Option<String>,
    pub source: Option<String>,
    pub remote_addr: Option<String>,
    pub display_name: Option<String>,
    pub gateway_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EffectiveProfileTrace {
    pub profile: String,
    pub provider_type: String,
    pub model: String,
    pub api_key_env: Option<String>,
    pub api_key_present: bool,
}

impl SkillTrace {
    pub fn duration_ms(&self) -> Option<i64> {
        self.finished_at.map(|finished| {
            finished
                .signed_duration_since(self.started_at)
                .num_milliseconds()
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WorkflowStepTrace {
    pub name: String,
    pub kind: String,
    pub input: String,
    pub output: Option<String>,
    pub started_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
    pub error: Option<String>,
}

impl WorkflowStepTrace {
    pub fn duration_ms(&self) -> Option<i64> {
        self.finished_at.map(|finished| {
            finished
                .signed_duration_since(self.started_at)
                .num_milliseconds()
        })
    }

    pub fn status(&self) -> &'static str {
        if self.error.is_some() {
            "failed"
        } else if self.finished_at.is_some() {
            "success"
        } else {
            "running"
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MemoryReadTrace {
    pub session_id: String,
    pub source: String,
    pub preview: String,
    #[serde(default)]
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MemoryWriteTrace {
    pub session_id: String,
    pub kind: String,
    pub preview: String,
    #[serde(default)]
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CompressionTrace {
    pub original_message_count: usize,
    pub kept_recent_count: usize,
    pub summary_preview: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ModelSelectionTrace {
    pub scope: String,
    pub requested_profile: Option<String>,
    pub selected_profile: String,
    pub selected_model: String,
    pub reason: String,
    pub context_window_chars: usize,
    pub budget_tier: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RunSummary {
    pub status: String,
    pub tool_calls: usize,
    pub skill_calls: usize,
    pub duration_ms: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RunTrace {
    pub run_id: String,
    pub gateway_run_id: Option<String>,
    pub correlation_id: Option<String>,
    pub session_id: Option<String>,
    pub session_route: Option<String>,
    pub ingress: Option<IngressTrace>,
    pub workflow_name: Option<String>,
    pub started_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
    pub input: String,
    pub output: Option<String>,
    pub effective_profile: Option<EffectiveProfileTrace>,
    #[serde(default)]
    pub model_selections: Vec<ModelSelectionTrace>,
    #[serde(default)]
    pub memory_reads: Vec<MemoryReadTrace>,
    #[serde(default)]
    pub memory_writes: Vec<MemoryWriteTrace>,
    pub compression: Option<CompressionTrace>,
    #[serde(default)]
    pub tool_calls: Vec<ToolTrace>,
    #[serde(default)]
    pub skill_calls: Vec<SkillTrace>,
    #[serde(default)]
    pub step_traces: Vec<WorkflowStepTrace>,
    pub error: Option<String>,
}

impl RunTrace {
    pub fn new(input: String) -> Self {
        Self {
            run_id: Uuid::new_v4().to_string(),
            gateway_run_id: None,
            correlation_id: None,
            session_id: None,
            session_route: None,
            ingress: None,
            workflow_name: None,
            started_at: Utc::now(),
            finished_at: None,
            input,
            output: None,
            effective_profile: None,
            model_selections: vec![],
            memory_reads: vec![],
            memory_writes: vec![],
            compression: None,
            tool_calls: vec![],
            skill_calls: vec![],
            step_traces: vec![],
            error: None,
        }
    }

    pub fn bind_session(&mut self, session_id: impl Into<String>) {
        self.session_id = Some(session_id.into());
    }

    pub fn bind_gateway_context(
        &mut self,
        gateway_run_id: impl Into<String>,
        correlation_id: impl Into<String>,
        session_route: impl Into<String>,
    ) {
        self.gateway_run_id = Some(gateway_run_id.into());
        self.correlation_id = Some(correlation_id.into());
        self.session_route = Some(session_route.into());
    }

    pub fn bind_ingress(&mut self, ingress: IngressTrace) {
        self.ingress = Some(ingress);
    }

    pub fn bind_workflow(&mut self, workflow_name: impl Into<String>) {
        self.workflow_name = Some(workflow_name.into());
    }

    pub fn bind_effective_profile(&mut self, profile: EffectiveProfileTrace) {
        self.effective_profile = Some(profile);
    }

    pub fn add_model_selection(&mut self, trace: ModelSelectionTrace) {
        self.model_selections.push(trace);
    }

    pub fn add_memory_read(&mut self, trace: MemoryReadTrace) {
        self.memory_reads.push(trace);
    }

    pub fn add_memory_write(&mut self, trace: MemoryWriteTrace) {
        self.memory_writes.push(trace);
    }

    pub fn bind_compression(&mut self, trace: CompressionTrace) {
        self.compression = Some(trace);
    }

    pub fn finish_ok(&mut self, output: String) {
        self.finished_at = Some(Utc::now());
        self.output = Some(output);
        self.error = None;
    }

    pub fn finish_err(&mut self, error: String) {
        self.finished_at = Some(Utc::now());
        self.error = Some(error);
    }

    pub fn duration_ms(&self) -> Option<i64> {
        self.finished_at.map(|finished| {
            finished
                .signed_duration_since(self.started_at)
                .num_milliseconds()
        })
    }

    pub fn status(&self) -> &'static str {
        if self.error.is_some() {
            "failed"
        } else if self.finished_at.is_some() {
            "success"
        } else {
            "running"
        }
    }

    pub fn summary(&self) -> RunSummary {
        RunSummary {
            status: self.status().to_owned(),
            tool_calls: self.tool_calls.len(),
            skill_calls: self.skill_calls.len(),
            duration_ms: self.duration_ms(),
        }
    }

    pub fn save_to_default_dir(&self) -> Result<PathBuf> {
        self.save_to_dir(PathBuf::from(".mosaic/runs"))
    }

    pub fn save_to_dir(&self, dir: impl AsRef<Path>) -> Result<PathBuf> {
        let dir = dir.as_ref();
        fs::create_dir_all(dir)?;
        let path = dir.join(format!("{}.json", self.run_id));
        fs::write(&path, serde_json::to_vec_pretty(self)?)?;
        Ok(path)
    }
}

#[cfg(test)]
mod tests {
    use std::{
        process,
        sync::atomic::{AtomicU64, Ordering},
        time::{SystemTime, UNIX_EPOCH},
    };

    use chrono::Duration;

    use super::*;

    static COUNTER: AtomicU64 = AtomicU64::new(0);

    fn temp_dir(label: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock should be after epoch")
            .as_nanos();
        let count = COUNTER.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!(
            "mosaic-inspect-{label}-{}-{nanos}-{count}",
            process::id()
        ))
    }

    #[test]
    fn saves_trace_to_a_custom_directory() {
        let dir = temp_dir("trace");
        let mut trace = RunTrace::new("hello".to_owned());
        trace.bind_ingress(IngressTrace {
            kind: "remote_operator".to_owned(),
            channel: Some("cli".to_owned()),
            source: Some("mosaic-cli".to_owned()),
            remote_addr: None,
            display_name: None,
            gateway_url: Some("http://127.0.0.1:8080".to_owned()),
        });
        trace.add_memory_read(MemoryReadTrace {
            session_id: "demo".to_owned(),
            source: "session_summary".to_owned(),
            preview: "Stored summary".to_owned(),
            tags: vec![],
        });
        trace.add_memory_write(MemoryWriteTrace {
            session_id: "demo".to_owned(),
            kind: "summary".to_owned(),
            preview: "New summary".to_owned(),
            tags: vec!["session".to_owned()],
        });
        trace.bind_compression(CompressionTrace {
            original_message_count: 12,
            kept_recent_count: 6,
            summary_preview: "Compressed older turns".to_owned(),
        });
        trace.tool_calls.push(ToolTrace {
            call_id: Some("call-1".to_owned()),
            name: "echo".to_owned(),
            source: ToolSource::Builtin,
            input: serde_json::json!({ "text": "hello" }),
            output: Some("hello".to_owned()),
            started_at: Utc::now(),
            finished_at: Some(Utc::now()),
        });
        trace.finish_ok("world".to_owned());

        let path = trace.save_to_dir(&dir).expect("trace should save");
        let content = fs::read_to_string(&path).expect("saved trace should be readable");
        let loaded: RunTrace = serde_json::from_str(&content).expect("trace should deserialize");

        assert_eq!(loaded.input, "hello");
        assert_eq!(loaded.output.as_deref(), Some("world"));
        assert_eq!(loaded.tool_calls[0].call_id.as_deref(), Some("call-1"));
        assert_eq!(
            loaded
                .ingress
                .as_ref()
                .and_then(|ingress| ingress.gateway_url.as_deref()),
            Some("http://127.0.0.1:8080")
        );
        assert_eq!(loaded.memory_reads.len(), 1);
        assert!(loaded.compression.is_some());

        fs::remove_file(path).ok();
        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn trace_summary_reports_status_counts_and_duration() {
        let started_at = Utc::now();
        let finished_at = started_at + Duration::milliseconds(18);

        let trace = RunTrace {
            run_id: "run-1".to_owned(),
            gateway_run_id: Some("gateway-run-1".to_owned()),
            correlation_id: Some("corr-1".to_owned()),
            session_id: Some("session-1".to_owned()),
            session_route: Some("gateway.local/session-1".to_owned()),
            ingress: None,
            workflow_name: Some("research_brief".to_owned()),
            started_at,
            finished_at: Some(finished_at),
            input: "hello".to_owned(),
            output: Some("world".to_owned()),
            effective_profile: Some(EffectiveProfileTrace {
                profile: "mock".to_owned(),
                provider_type: "mock".to_owned(),
                model: "mock".to_owned(),
                api_key_env: None,
                api_key_present: false,
            }),
            model_selections: vec![ModelSelectionTrace {
                scope: "run".to_owned(),
                requested_profile: None,
                selected_profile: "mock".to_owned(),
                selected_model: "mock".to_owned(),
                reason: "active_profile".to_owned(),
                context_window_chars: 4000,
                budget_tier: "debug".to_owned(),
            }],
            memory_reads: vec![],
            memory_writes: vec![],
            compression: None,
            tool_calls: vec![ToolTrace {
                call_id: Some("call-1".to_owned()),
                name: "echo".to_owned(),
                source: ToolSource::Builtin,
                input: serde_json::json!({ "text": "hello" }),
                output: Some("hello".to_owned()),
                started_at,
                finished_at: Some(started_at + Duration::milliseconds(3)),
            }],
            skill_calls: vec![],
            step_traces: vec![WorkflowStepTrace {
                name: "draft".to_owned(),
                kind: "prompt".to_owned(),
                input: "hello".to_owned(),
                output: Some("world".to_owned()),
                started_at,
                finished_at: Some(started_at + Duration::milliseconds(9)),
                error: None,
            }],
            error: None,
        };

        let summary = trace.summary();

        assert_eq!(trace.status(), "success");
        assert_eq!(trace.duration_ms(), Some(18));
        assert_eq!(summary.status, "success");
        assert_eq!(summary.tool_calls, 1);
        assert_eq!(summary.skill_calls, 0);
        assert_eq!(summary.duration_ms, Some(18));
        assert_eq!(trace.tool_calls[0].duration_ms(), Some(3));
        assert_eq!(trace.step_traces[0].duration_ms(), Some(9));
        assert_eq!(trace.step_traces[0].status(), "success");
        assert_eq!(trace.model_selections.len(), 1);
    }

    #[test]
    fn trace_status_reports_failure_when_error_exists() {
        let mut trace = RunTrace::new("hello".to_owned());
        trace.finish_err("boom".to_owned());

        assert_eq!(trace.status(), "failed");
        assert_eq!(trace.summary().status, "failed");
        assert!(trace.duration_ms().is_some());
    }

    #[test]
    fn bind_ingress_updates_trace_metadata() {
        let mut trace = RunTrace::new("hello".to_owned());
        trace.bind_ingress(IngressTrace {
            kind: "webchat".to_owned(),
            channel: Some("webchat".to_owned()),
            source: Some("browser".to_owned()),
            remote_addr: Some("127.0.0.1".to_owned()),
            display_name: Some("guest".to_owned()),
            gateway_url: None,
        });

        assert_eq!(
            trace.ingress.as_ref().map(|ingress| ingress.kind.as_str()),
            Some("webchat")
        );
        assert_eq!(
            trace
                .ingress
                .as_ref()
                .and_then(|ingress| ingress.display_name.as_deref()),
            Some("guest")
        );
    }
}
