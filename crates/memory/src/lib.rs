use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Result, bail};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MemoryEntryKind {
    Summary,
    Note,
    Compression,
    CrossSession,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MemoryEntry {
    pub id: String,
    pub kind: MemoryEntryKind,
    pub content: String,
    #[serde(default)]
    pub tags: Vec<String>,
    pub created_at: DateTime<Utc>,
}

impl MemoryEntry {
    pub fn new(kind: MemoryEntryKind, content: impl Into<String>, tags: Vec<String>) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            kind,
            content: content.into(),
            tags,
            created_at: Utc::now(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SessionMemoryRecord {
    pub session_id: String,
    pub updated_at: DateTime<Utc>,
    pub summary: Option<String>,
    pub compressed_context: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub related_sessions: Vec<String>,
    #[serde(default)]
    pub entries: Vec<MemoryEntry>,
}

impl SessionMemoryRecord {
    pub fn new(session_id: impl Into<String>) -> Self {
        Self {
            session_id: session_id.into(),
            updated_at: Utc::now(),
            summary: None,
            compressed_context: None,
            tags: Vec::new(),
            related_sessions: Vec::new(),
            entries: Vec::new(),
        }
    }

    pub fn set_summary(&mut self, summary: Option<String>) {
        self.summary = summary;
        self.updated_at = Utc::now();
    }

    pub fn set_compressed_context(&mut self, compressed_context: Option<String>) {
        self.compressed_context = compressed_context;
        self.updated_at = Utc::now();
    }

    pub fn record_entry(
        &mut self,
        kind: MemoryEntryKind,
        content: impl Into<String>,
        tags: Vec<String>,
    ) {
        self.entries.push(MemoryEntry::new(kind, content, tags));
        self.updated_at = Utc::now();
    }

    pub fn link_session(&mut self, session_id: impl Into<String>) {
        let session_id = session_id.into();
        if !self.related_sessions.contains(&session_id) {
            self.related_sessions.push(session_id);
            self.updated_at = Utc::now();
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MemorySearchHit {
    pub session_id: String,
    pub kind: String,
    pub preview: String,
    #[serde(default)]
    pub tags: Vec<String>,
    pub updated_at: DateTime<Utc>,
}

pub trait MemoryStore: Send + Sync {
    fn load_session(&self, session_id: &str) -> Result<Option<SessionMemoryRecord>>;
    fn save_session(&self, record: &SessionMemoryRecord) -> Result<()>;
    fn list_sessions(&self) -> Result<Vec<SessionMemoryRecord>>;
    fn search(&self, query: &str, tag: Option<&str>) -> Result<Vec<MemorySearchHit>>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryPolicy {
    pub compression_message_threshold: usize,
    pub recent_message_window: usize,
    pub summary_char_budget: usize,
    pub note_char_budget: usize,
}

impl Default for MemoryPolicy {
    fn default() -> Self {
        Self {
            compression_message_threshold: 8,
            recent_message_window: 6,
            summary_char_budget: 320,
            note_char_budget: 220,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompressionOutcome {
    pub compressed: bool,
    pub summary: String,
    pub recent_messages: Vec<String>,
    pub original_message_count: usize,
    pub kept_recent_count: usize,
}

pub fn summarize_fragments(fragments: &[String], max_chars: usize) -> String {
    let normalized = fragments
        .iter()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();

    if normalized.is_empty() {
        return "No memory available.".to_owned();
    }

    let mut summary = String::new();
    for fragment in &normalized {
        let next = if summary.is_empty() {
            (*fragment).to_owned()
        } else {
            format!("{} | {}", summary, fragment)
        };

        if next.chars().count() > max_chars {
            break;
        }

        summary = next;
    }

    if summary.is_empty() {
        truncate_preview(normalized[0], max_chars)
    } else {
        summary
    }
}

pub fn compress_fragments(fragments: &[String], policy: &MemoryPolicy) -> CompressionOutcome {
    let compressed = fragments.len() > policy.compression_message_threshold;
    let kept_recent_count = fragments.len().min(policy.recent_message_window);
    let recent_messages = fragments[fragments.len().saturating_sub(kept_recent_count)..].to_vec();
    let summary_source = if compressed {
        &fragments[..fragments.len().saturating_sub(kept_recent_count)]
    } else {
        fragments
    };

    let summary = summarize_fragments(summary_source, policy.summary_char_budget);

    CompressionOutcome {
        compressed,
        summary,
        recent_messages,
        original_message_count: fragments.len(),
        kept_recent_count,
    }
}

#[derive(Debug, Clone)]
pub struct FileMemoryStore {
    root_dir: PathBuf,
}

impl FileMemoryStore {
    pub fn new(root_dir: impl Into<PathBuf>) -> Self {
        Self {
            root_dir: root_dir.into(),
        }
    }

    pub fn root_dir(&self) -> &Path {
        &self.root_dir
    }

    fn ensure_root_dir(&self) -> Result<()> {
        fs::create_dir_all(&self.root_dir)?;
        Ok(())
    }

    fn session_path(&self, session_id: &str) -> Result<PathBuf> {
        validate_memory_session_id(session_id)?;
        Ok(self.root_dir.join(format!("{session_id}.json")))
    }
}

impl MemoryStore for FileMemoryStore {
    fn load_session(&self, session_id: &str) -> Result<Option<SessionMemoryRecord>> {
        let path = self.session_path(session_id)?;
        if !path.exists() {
            return Ok(None);
        }

        let content = fs::read_to_string(path)?;
        Ok(Some(serde_json::from_str::<SessionMemoryRecord>(&content)?))
    }

    fn save_session(&self, record: &SessionMemoryRecord) -> Result<()> {
        self.ensure_root_dir()?;
        let path = self.session_path(&record.session_id)?;
        fs::write(path, serde_json::to_vec_pretty(record)?)?;
        Ok(())
    }

    fn list_sessions(&self) -> Result<Vec<SessionMemoryRecord>> {
        self.ensure_root_dir()?;
        let mut sessions = Vec::new();

        for entry in fs::read_dir(&self.root_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
                continue;
            }

            let content = fs::read_to_string(path)?;
            sessions.push(serde_json::from_str::<SessionMemoryRecord>(&content)?);
        }

        sessions.sort_by(|left, right| right.updated_at.cmp(&left.updated_at));
        Ok(sessions)
    }

    fn search(&self, query: &str, tag: Option<&str>) -> Result<Vec<MemorySearchHit>> {
        let query = query.trim().to_ascii_lowercase();
        let tag = tag.map(|value| value.trim().to_ascii_lowercase());
        let mut hits = Vec::new();

        for session in self.list_sessions()? {
            let mut push_hit = |kind: &str, content: &str, tags: &[String]| {
                let content_lower = content.to_ascii_lowercase();
                let tag_match = match tag.as_deref() {
                    Some(required) => tags
                        .iter()
                        .any(|value| value.eq_ignore_ascii_case(required)),
                    None => true,
                };
                let query_match = query.is_empty() || content_lower.contains(&query);

                if tag_match && query_match {
                    hits.push(MemorySearchHit {
                        session_id: session.session_id.clone(),
                        kind: kind.to_owned(),
                        preview: truncate_preview(content, 140),
                        tags: tags.to_vec(),
                        updated_at: session.updated_at,
                    });
                }
            };

            if let Some(summary) = session.summary.as_deref() {
                push_hit("summary", summary, &session.tags);
            }
            if let Some(compressed) = session.compressed_context.as_deref() {
                push_hit("compression", compressed, &session.tags);
            }
            for entry in &session.entries {
                push_hit(
                    &format!("{:?}", entry.kind).to_ascii_lowercase(),
                    &entry.content,
                    &entry.tags,
                );
            }
        }

        hits.sort_by(|left, right| right.updated_at.cmp(&left.updated_at));
        Ok(hits)
    }
}

fn validate_memory_session_id(session_id: &str) -> Result<()> {
    let trimmed = session_id.trim();
    if trimmed.is_empty() {
        bail!("memory session id must not be empty");
    }
    if trimmed.contains('/') || trimmed.contains('\\') || trimmed.contains("..") {
        bail!("memory session id must not contain path traversal or separators");
    }
    if trimmed.chars().any(char::is_whitespace) {
        bail!("memory session id must not contain whitespace");
    }
    Ok(())
}

fn truncate_preview(value: &str, max_chars: usize) -> String {
    if value.chars().count() <= max_chars {
        return value.to_owned();
    }

    let truncated: String = value.chars().take(max_chars).collect();
    format!("{truncated}...")
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
        let path = std::env::temp_dir().join(format!(
            "mosaic-memory-{label}-{}-{nanos}-{count}",
            process::id()
        ));
        fs::create_dir_all(&path).expect("temp dir should be created");
        path
    }

    #[test]
    fn summarizes_and_compresses_long_fragments() {
        let policy = MemoryPolicy {
            compression_message_threshold: 3,
            recent_message_window: 2,
            summary_char_budget: 120,
            note_char_budget: 80,
        };
        let fragments = vec![
            "system: You are helpful.".to_owned(),
            "user: explain gateway routing".to_owned(),
            "assistant: routing uses session ids".to_owned(),
            "user: also mention memory".to_owned(),
            "assistant: memory stores summaries".to_owned(),
        ];

        let compressed = compress_fragments(&fragments, &policy);

        assert!(compressed.compressed);
        assert_eq!(compressed.original_message_count, 5);
        assert_eq!(compressed.kept_recent_count, 2);
        assert_eq!(compressed.recent_messages.len(), 2);
        assert!(
            compressed.summary.contains("gateway routing")
                || compressed.summary.contains("You are helpful")
        );
    }

    #[test]
    fn file_store_roundtrips_and_searches_memory() {
        let dir = temp_dir("roundtrip");
        let store = FileMemoryStore::new(&dir);
        let mut record = SessionMemoryRecord::new("demo");
        record.tags = vec!["project".to_owned()];
        record.set_summary(Some("Gateway session summary".to_owned()));
        record.set_compressed_context(Some(
            "Earlier turns discussed memory compression".to_owned(),
        ));
        record.record_entry(
            MemoryEntryKind::Note,
            "Remember to mention cross-session references",
            vec!["project".to_owned(), "note".to_owned()],
        );
        store.save_session(&record).expect("record should save");

        let loaded = store
            .load_session("demo")
            .expect("load should succeed")
            .expect("record should exist");
        let hits = store
            .search("cross-session", Some("note"))
            .expect("search should succeed");

        assert_eq!(loaded.session_id, "demo");
        assert_eq!(loaded.entries.len(), 1);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].session_id, "demo");

        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn list_sessions_returns_latest_first() {
        let dir = temp_dir("list");
        let store = FileMemoryStore::new(&dir);
        let mut older = SessionMemoryRecord::new("older");
        older.updated_at = older.updated_at - chrono::Duration::seconds(10);
        let newer = SessionMemoryRecord::new("newer");
        store.save_session(&older).expect("older should save");
        store.save_session(&newer).expect("newer should save");

        let sessions = store.list_sessions().expect("list should succeed");
        assert_eq!(sessions[0].session_id, "newer");
        assert_eq!(sessions[1].session_id, "older");

        fs::remove_dir_all(dir).ok();
    }
}
