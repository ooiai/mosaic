use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use mosaic_core::error::{MosaicError, Result};
use mosaic_core::privacy::append_sanitized_jsonl;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemEvent {
    pub ts: DateTime<Utc>,
    pub name: String,
    pub data: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PresenceSnapshot {
    pub ts: DateTime<Utc>,
    pub pid: u32,
    pub cwd: String,
    pub hostname: String,
}

#[derive(Debug, Clone)]
pub struct SystemEventStore {
    path: PathBuf,
}

impl SystemEventStore {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn append_event(&self, name: &str, data: Value) -> Result<SystemEvent> {
        let name = name.trim();
        if name.is_empty() {
            return Err(MosaicError::Validation(
                "system event name cannot be empty".to_string(),
            ));
        }

        let event = SystemEvent {
            ts: Utc::now(),
            name: name.to_string(),
            data,
        };
        append_sanitized_jsonl(&self.path, &event, "system event persistence")?;
        Ok(event)
    }

    pub fn read_tail(&self, tail: usize) -> Result<Vec<SystemEvent>> {
        if !self.path.exists() {
            return Ok(Vec::new());
        }
        let raw = std::fs::read_to_string(&self.path)?;
        let mut events = raw
            .lines()
            .filter(|line| !line.trim().is_empty())
            .map(|line| {
                serde_json::from_str::<SystemEvent>(line).map_err(|err| {
                    MosaicError::Validation(format!(
                        "invalid system event format {}: {err}",
                        self.path.display()
                    ))
                })
            })
            .collect::<Result<Vec<_>>>()?;
        if events.len() > tail {
            let keep_from = events.len() - tail;
            events = events.split_off(keep_from);
        }
        Ok(events)
    }
}

pub fn system_events_path(data_dir: &Path) -> PathBuf {
    data_dir.join("system-events.jsonl")
}

pub fn snapshot_presence(cwd: &Path) -> PresenceSnapshot {
    let hostname = std::env::var("HOSTNAME")
        .or_else(|_| std::env::var("COMPUTERNAME"))
        .unwrap_or_else(|_| "unknown-host".to_string());
    PresenceSnapshot {
        ts: Utc::now(),
        pid: std::process::id(),
        cwd: cwd.display().to_string(),
        hostname,
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use tempfile::tempdir;

    use super::*;

    #[test]
    fn append_and_read_events() {
        let temp = tempdir().expect("tempdir");
        let store = SystemEventStore::new(temp.path().join("events.jsonl"));
        store
            .append_event("startup", json!({"ok": true}))
            .expect("append");
        let events = store.read_tail(10).expect("read tail");
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].name, "startup");
    }

    #[test]
    fn append_event_redacts_secret_like_content() {
        let temp = tempdir().expect("tempdir");
        let store = SystemEventStore::new(temp.path().join("events.jsonl"));
        store
            .append_event(
                "secret_probe",
                json!({"token": "sk-live-secret-value-1234567890"}),
            )
            .expect("append");
        let raw = std::fs::read_to_string(store.path()).expect("read events");
        assert!(raw.contains("\"token\":\"[REDACTED]\""));
    }

    #[test]
    fn append_event_blocks_private_key_content() {
        let temp = tempdir().expect("tempdir");
        let store = SystemEventStore::new(temp.path().join("events.jsonl"));
        let err = store
            .append_event(
                "secret_probe",
                json!({"text":"-----BEGIN OPENSSH PRIVATE KEY-----"}),
            )
            .expect_err("should block");
        assert!(err.to_string().contains("private key material"));
    }
}
