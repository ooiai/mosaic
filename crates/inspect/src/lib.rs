use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolTrace {
    pub call_id: Option<String>,
    pub name: String,
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
    pub session_id: Option<String>,
    pub started_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
    pub input: String,
    pub output: Option<String>,
    pub effective_profile: Option<EffectiveProfileTrace>,
    #[serde(default)]
    pub tool_calls: Vec<ToolTrace>,
    #[serde(default)]
    pub skill_calls: Vec<SkillTrace>,
    pub error: Option<String>,
}

impl RunTrace {
    pub fn new(input: String) -> Self {
        Self {
            run_id: Uuid::new_v4().to_string(),
            session_id: None,
            started_at: Utc::now(),
            finished_at: None,
            input,
            output: None,
            effective_profile: None,
            tool_calls: vec![],
            skill_calls: vec![],
            error: None,
        }
    }

    pub fn bind_session(&mut self, session_id: impl Into<String>) {
        self.session_id = Some(session_id.into());
    }

    pub fn bind_effective_profile(&mut self, profile: EffectiveProfileTrace) {
        self.effective_profile = Some(profile);
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
        trace.tool_calls.push(ToolTrace {
            call_id: Some("call-1".to_owned()),
            name: "echo".to_owned(),
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

        fs::remove_file(path).ok();
        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn trace_summary_reports_status_counts_and_duration() {
        let started_at = Utc::now();
        let finished_at = started_at + Duration::milliseconds(18);

        let trace = RunTrace {
            run_id: "run-1".to_owned(),
            session_id: Some("session-1".to_owned()),
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
            tool_calls: vec![ToolTrace {
                call_id: Some("call-1".to_owned()),
                name: "echo".to_owned(),
                input: serde_json::json!({ "text": "hello" }),
                output: Some("hello".to_owned()),
                started_at,
                finished_at: Some(started_at + Duration::milliseconds(3)),
            }],
            skill_calls: vec![],
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
    }

    #[test]
    fn trace_status_reports_failure_when_error_exists() {
        let mut trace = RunTrace::new("hello".to_owned());
        trace.finish_err("boom".to_owned());

        assert_eq!(trace.status(), "failed");
        assert_eq!(trace.summary().status, "failed");
        assert!(trace.duration_ms().is_some());
    }
}
