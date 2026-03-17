use std::cmp::Reverse;
use std::fs::{self, File};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use crate::error::{MosaicError, Result};
use crate::privacy::append_sanitized_jsonl;

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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub runtime: Option<SessionRuntimeMetadata>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionRuntimeMetadata {
    pub agent_id: Option<String>,
    pub profile_name: String,
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
        append_sanitized_jsonl(&path, event, "session event persistence")
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
            let created_at = events.first().map(|event| event.ts);
            let title = Self::extract_session_title(&events);
            let runtime = Self::latest_runtime_metadata_from_events(&events);
            sessions.push(SessionSummary {
                session_id,
                event_count: events.len(),
                last_updated,
                created_at,
                title,
                runtime,
            });
        }
        sessions.sort_by_key(|summary| Reverse(summary.last_updated));
        Ok(sessions)
    }

    pub fn latest_runtime_metadata(
        &self,
        session_id: &str,
    ) -> Result<Option<SessionRuntimeMetadata>> {
        let path = self.session_path(session_id);
        if !path.exists() {
            return Ok(None);
        }
        let events = Self::read_events_from_path(&path)?;
        Ok(Self::latest_runtime_metadata_from_events(&events))
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

    pub fn build_runtime_metadata_event(
        session_id: &str,
        metadata: &SessionRuntimeMetadata,
    ) -> SessionEvent {
        Self::build_event(
            session_id,
            EventKind::System,
            serde_json::json!({
                "category": "runtime_metadata",
                "agent_id": metadata.agent_id,
                "profile_name": metadata.profile_name,
            }),
        )
    }

    pub fn latest_runtime_metadata_from_events(
        events: &[SessionEvent],
    ) -> Option<SessionRuntimeMetadata> {
        events.iter().rev().find_map(Self::parse_runtime_metadata)
    }

    fn extract_session_title(events: &[SessionEvent]) -> Option<String> {
        events
            .iter()
            .find(|event| event.kind == EventKind::User)
            .and_then(|event| event.payload.get("text")?.as_str().map(str::to_string))
            .map(|text| {
                let single_line = text.lines().next().unwrap_or("").trim().to_string();
                if single_line.chars().count() > 60 {
                    format!("{}…", single_line.chars().take(60).collect::<String>())
                } else {
                    single_line
                }
            })
            .filter(|s| !s.is_empty())
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

    fn parse_runtime_metadata(event: &SessionEvent) -> Option<SessionRuntimeMetadata> {
        if event.kind != EventKind::System {
            return None;
        }
        if event.payload.get("category")?.as_str()? != "runtime_metadata" {
            return None;
        }
        let profile_name = event.payload.get("profile_name")?.as_str()?.trim();
        if profile_name.is_empty() {
            return None;
        }
        let agent_id = event
            .payload
            .get("agent_id")
            .and_then(|value| value.as_str())
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned);
        Some(SessionRuntimeMetadata {
            agent_id,
            profile_name: profile_name.to_string(),
        })
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

    #[test]
    fn latest_runtime_metadata_reads_last_runtime_event() {
        let temp = tempdir().unwrap();
        let store = SessionStore::new(temp.path().join("sessions"));
        let sid = store.create_session_id();
        store
            .append_event(&SessionStore::build_runtime_metadata_event(
                &sid,
                &SessionRuntimeMetadata {
                    agent_id: Some("writer".to_string()),
                    profile_name: "default".to_string(),
                },
            ))
            .unwrap();
        store
            .append_event(&SessionStore::build_event(
                &sid,
                EventKind::User,
                json!({ "text": "hello" }),
            ))
            .unwrap();
        store
            .append_event(&SessionStore::build_runtime_metadata_event(
                &sid,
                &SessionRuntimeMetadata {
                    agent_id: Some("editor".to_string()),
                    profile_name: "default".to_string(),
                },
            ))
            .unwrap();

        let runtime = store.latest_runtime_metadata(&sid).unwrap();
        assert_eq!(
            runtime,
            Some(SessionRuntimeMetadata {
                agent_id: Some("editor".to_string()),
                profile_name: "default".to_string(),
            })
        );
    }

    #[test]
    fn list_sessions_includes_runtime_summary() {
        let temp = tempdir().unwrap();
        let store = SessionStore::new(temp.path().join("sessions"));
        let sid = store.create_session_id();
        store
            .append_event(&SessionStore::build_runtime_metadata_event(
                &sid,
                &SessionRuntimeMetadata {
                    agent_id: Some("writer".to_string()),
                    profile_name: "default".to_string(),
                },
            ))
            .unwrap();
        store
            .append_event(&SessionStore::build_event(
                &sid,
                EventKind::Assistant,
                json!({ "text": "done" }),
            ))
            .unwrap();

        let sessions = store.list_sessions().unwrap();
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].session_id, sid);
        assert_eq!(
            sessions[0].runtime,
            Some(SessionRuntimeMetadata {
                agent_id: Some("writer".to_string()),
                profile_name: "default".to_string(),
            })
        );
    }

    #[test]
    fn append_event_redacts_secret_like_payload_before_persist() {
        let temp = tempdir().unwrap();
        let store = SessionStore::new(temp.path().join("sessions"));
        let sid = store.create_session_id();
        let event = SessionStore::build_event(
            &sid,
            EventKind::User,
            json!({
                "text": "token=sk-test-secret-value-1234567890123",
                "api_key": "raw-value"
            }),
        );
        store.append_event(&event).unwrap();

        let saved = std::fs::read_to_string(store.session_path(&sid)).unwrap();
        assert!(saved.contains("token=[REDACTED]"));
        assert!(saved.contains("\"api_key\":\"[REDACTED]\""));
    }

    #[test]
    fn append_event_blocks_private_key_material() {
        let temp = tempdir().unwrap();
        let store = SessionStore::new(temp.path().join("sessions"));
        let sid = store.create_session_id();
        let event = SessionStore::build_event(
            &sid,
            EventKind::User,
            json!({
                "text": "-----BEGIN OPENSSH PRIVATE KEY-----"
            }),
        );
        let err = store.append_event(&event).unwrap_err();
        assert!(err.to_string().contains("private key material"));
    }
}
