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
    pub name: String,
    pub input: serde_json::Value,
    pub output: Option<String>,
    pub started_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SkillTrace {
    pub name: String,
    pub input: serde_json::Value,
    pub output: Option<String>,
    pub started_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RunTrace {
    pub run_id: String,
    pub started_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
    pub input: String,
    pub output: Option<String>,
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
            started_at: Utc::now(),
            finished_at: None,
            input,
            output: None,
            tool_calls: vec![],
            skill_calls: vec![],
            error: None,
        }
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
        trace.finish_ok("world".to_owned());

        let path = trace.save_to_dir(&dir).expect("trace should save");
        let content = fs::read_to_string(&path).expect("saved trace should be readable");
        let loaded: RunTrace = serde_json::from_str(&content).expect("trace should deserialize");

        assert_eq!(loaded.input, "hello");
        assert_eq!(loaded.output.as_deref(), Some("world"));

        fs::remove_file(path).ok();
        fs::remove_dir_all(dir).ok();
    }
}
