use std::cmp::Reverse;
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use crate::error::{MosaicError, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventKind {
    User,
    Assistant,
    ToolCall,
    ToolResult,
    System,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionEvent {
    pub id: String,
    pub ts: DateTime<Utc>,
    pub session_id: String,
    #[serde(rename = "type")]
    pub kind: EventKind,
    pub payload: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionSummary {
    pub session_id: String,
    pub event_count: usize,
    pub last_updated: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone)]
pub struct SessionStore {
    sessions_dir: PathBuf,
}

impl SessionStore {
    pub fn new(sessions_dir: PathBuf) -> Self {
        Self { sessions_dir }
    }

    pub fn sessions_dir(&self) -> &Path {
        &self.sessions_dir
    }

    pub fn ensure_dirs(&self) -> Result<()> {
        fs::create_dir_all(&self.sessions_dir)?;
        Ok(())
    }

    pub fn create_session_id(&self) -> String {
        Uuid::new_v4().to_string()
    }

    pub fn session_path(&self, session_id: &str) -> PathBuf {
        self.sessions_dir.join(format!("{session_id}.jsonl"))
    }

    pub fn append_event(&self, event: &SessionEvent) -> Result<()> {
        self.ensure_dirs()?;
        let path = self.session_path(&event.session_id);
        let mut file = OpenOptions::new().create(true).append(true).open(path)?;
        let line = serde_json::to_string(event)?;
        file.write_all(line.as_bytes())?;
        file.write_all(b"\n")?;
        Ok(())
    }

    pub fn read_events(&self, session_id: &str) -> Result<Vec<SessionEvent>> {
        let path = self.session_path(session_id);
        if !path.exists() {
            return Err(MosaicError::Config(format!(
                "session '{session_id}' was not found"
            )));
        }
        Self::read_events_from_path(&path)
    }

    pub fn list_sessions(&self) -> Result<Vec<SessionSummary>> {
        self.ensure_dirs()?;
        let mut sessions = Vec::new();
        for entry in fs::read_dir(&self.sessions_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|v| v.to_str()) != Some("jsonl") {
                continue;
            }
            let session_id = path
                .file_stem()
                .and_then(|v| v.to_str())
                .ok_or_else(|| MosaicError::Validation("invalid session file name".to_string()))?
                .to_string();
            let events = Self::read_events_from_path(&path)?;
            let last_updated = events.last().map(|event| event.ts);
            sessions.push(SessionSummary {
                session_id,
                event_count: events.len(),
                last_updated,
            });
        }
        sessions.sort_by_key(|summary| Reverse(summary.last_updated));
        Ok(sessions)
    }

    pub fn latest_session_id(&self) -> Result<Option<String>> {
        let sessions = self.list_sessions()?;
        Ok(sessions.first().map(|summary| summary.session_id.clone()))
    }

    pub fn clear_session(&self, session_id: &str) -> Result<()> {
        let path = self.session_path(session_id);
        if !path.exists() {
            return Err(MosaicError::Config(format!(
                "session '{session_id}' was not found"
            )));
        }
        fs::remove_file(path)?;
        Ok(())
    }

    pub fn clear_all(&self) -> Result<usize> {
        self.ensure_dirs()?;
        let mut count = 0usize;
        for entry in fs::read_dir(&self.sessions_dir)? {
            let path = entry?.path();
            if path.extension().and_then(|v| v.to_str()) == Some("jsonl") {
                fs::remove_file(path)?;
                count += 1;
            }
        }
        Ok(count)
    }

    pub fn build_event(session_id: &str, kind: EventKind, payload: Value) -> SessionEvent {
        SessionEvent {
            id: Uuid::new_v4().to_string(),
            ts: Utc::now(),
            session_id: session_id.to_string(),
            kind,
            payload,
        }
    }

    fn read_events_from_path(path: &Path) -> Result<Vec<SessionEvent>> {
        let file = File::open(path)?;
        let reader = BufReader::new(file);
        let mut events = Vec::new();
        for line in reader.lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            let event: SessionEvent = serde_json::from_str(&line).map_err(|err| {
                MosaicError::Validation(format!("invalid session event format: {err}"))
            })?;
            events.push(event);
        }
        Ok(events)
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use tempfile::tempdir;

    use super::*;

    #[test]
    fn append_and_read_session_events() {
        let temp = tempdir().unwrap();
        let store = SessionStore::new(temp.path().join("sessions"));
        let sid = store.create_session_id();
        let e1 = SessionStore::build_event(&sid, EventKind::User, json!({ "text": "hello" }));
        let e2 = SessionStore::build_event(&sid, EventKind::Assistant, json!({ "text": "hi" }));
        store.append_event(&e1).unwrap();
        store.append_event(&e2).unwrap();
        let events = store.read_events(&sid).unwrap();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].kind, EventKind::User);
        assert_eq!(events[1].kind, EventKind::Assistant);
    }

    #[test]
    fn clear_all_sessions() {
        let temp = tempdir().unwrap();
        let store = SessionStore::new(temp.path().join("sessions"));
        let sid1 = store.create_session_id();
        let sid2 = store.create_session_id();
        store
            .append_event(&SessionStore::build_event(
                &sid1,
                EventKind::System,
                json!({}),
            ))
            .unwrap();
        store
            .append_event(&SessionStore::build_event(
                &sid2,
                EventKind::System,
                json!({}),
            ))
            .unwrap();
        let removed = store.clear_all().unwrap();
        assert_eq!(removed, 2);
    }
}
