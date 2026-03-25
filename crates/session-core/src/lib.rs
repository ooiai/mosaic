use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Result, bail};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum TranscriptRole {
    System,
    User,
    Assistant,
    Tool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TranscriptMessage {
    pub role: TranscriptRole,
    pub content: String,
    pub tool_call_id: Option<String>,
    pub created_at: DateTime<Utc>,
}

impl TranscriptMessage {
    pub fn new(
        role: TranscriptRole,
        content: impl Into<String>,
        tool_call_id: Option<String>,
    ) -> Self {
        Self {
            role,
            content: content.into(),
            tool_call_id,
            created_at: Utc::now(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct SessionMemoryMetadata {
    pub latest_summary: Option<String>,
    pub compressed_context: Option<String>,
    pub last_memory_write_at: Option<DateTime<Utc>>,
    pub memory_entry_count: usize,
    pub compression_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SessionReference {
    pub session_id: String,
    pub reason: String,
    pub created_at: DateTime<Utc>,
}

impl SessionReference {
    pub fn new(session_id: impl Into<String>, reason: impl Into<String>) -> Self {
        Self {
            session_id: session_id.into(),
            reason: reason.into(),
            created_at: Utc::now(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SessionRecord {
    pub id: String,
    pub title: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub provider_profile: String,
    pub provider_type: String,
    pub model: String,
    pub last_run_id: Option<String>,
    #[serde(default)]
    pub gateway: SessionGatewayMetadata,
    #[serde(default)]
    pub memory: SessionMemoryMetadata,
    #[serde(default)]
    pub references: Vec<SessionReference>,
    #[serde(default)]
    pub transcript: Vec<TranscriptMessage>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct SessionGatewayMetadata {
    pub route: String,
    pub last_gateway_run_id: Option<String>,
    pub last_correlation_id: Option<String>,
}

impl SessionRecord {
    pub fn new(
        id: impl Into<String>,
        title: impl Into<String>,
        provider_profile: impl Into<String>,
        provider_type: impl Into<String>,
        model: impl Into<String>,
    ) -> Self {
        let id = id.into();
        let now = Utc::now();

        Self {
            id: id.clone(),
            title: title.into(),
            created_at: now,
            updated_at: now,
            provider_profile: provider_profile.into(),
            provider_type: provider_type.into(),
            model: model.into(),
            last_run_id: None,
            gateway: SessionGatewayMetadata {
                route: session_route_for_id(&id),
                last_gateway_run_id: None,
                last_correlation_id: None,
            },
            memory: SessionMemoryMetadata::default(),
            references: Vec::new(),
            transcript: Vec::new(),
        }
    }

    pub fn append_message(
        &mut self,
        role: TranscriptRole,
        content: impl Into<String>,
        tool_call_id: Option<String>,
    ) {
        self.transcript
            .push(TranscriptMessage::new(role, content, tool_call_id));
        self.updated_at = Utc::now();
    }

    pub fn set_runtime_binding(
        &mut self,
        provider_profile: impl Into<String>,
        provider_type: impl Into<String>,
        model: impl Into<String>,
    ) {
        self.provider_profile = provider_profile.into();
        self.provider_type = provider_type.into();
        self.model = model.into();
        self.updated_at = Utc::now();
    }

    pub fn set_last_run_id(&mut self, run_id: impl Into<String>) {
        self.last_run_id = Some(run_id.into());
        self.updated_at = Utc::now();
    }

    pub fn set_gateway_binding(
        &mut self,
        route: impl Into<String>,
        gateway_run_id: impl Into<String>,
        correlation_id: impl Into<String>,
    ) {
        self.gateway.route = route.into();
        self.gateway.last_gateway_run_id = Some(gateway_run_id.into());
        self.gateway.last_correlation_id = Some(correlation_id.into());
        self.updated_at = Utc::now();
    }

    pub fn set_memory_state(
        &mut self,
        latest_summary: Option<String>,
        compressed_context: Option<String>,
        memory_entry_count: usize,
        increment_compression_count: bool,
    ) {
        self.memory.latest_summary = latest_summary;
        self.memory.compressed_context = compressed_context;
        self.memory.memory_entry_count = memory_entry_count;
        self.memory.last_memory_write_at = Some(Utc::now());
        if increment_compression_count {
            self.memory.compression_count += 1;
        }
        self.updated_at = Utc::now();
    }

    pub fn record_reference(&mut self, session_id: impl Into<String>, reason: impl Into<String>) {
        let session_id = session_id.into();
        if self
            .references
            .iter()
            .any(|reference| reference.session_id == session_id)
        {
            return;
        }

        self.references
            .push(SessionReference::new(session_id, reason));
        self.updated_at = Utc::now();
    }

    pub fn summary(&self) -> SessionSummary {
        SessionSummary {
            id: self.id.clone(),
            title: self.title.clone(),
            updated_at: self.updated_at,
            provider_profile: self.provider_profile.clone(),
            provider_type: self.provider_type.clone(),
            model: self.model.clone(),
            session_route: self.effective_gateway_route(),
            last_gateway_run_id: self.gateway.last_gateway_run_id.clone(),
            last_correlation_id: self.gateway.last_correlation_id.clone(),
            message_count: self.transcript.len(),
            last_message_preview: self
                .transcript
                .last()
                .map(|message| truncate_preview(&message.content, 120)),
            memory_summary_preview: self
                .memory
                .latest_summary
                .as_deref()
                .map(|summary| truncate_preview(summary, 120)),
            reference_count: self.references.len(),
        }
    }

    fn effective_gateway_route(&self) -> String {
        if self.gateway.route.is_empty() {
            session_route_for_id(&self.id)
        } else {
            self.gateway.route.clone()
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SessionSummary {
    pub id: String,
    pub title: String,
    pub updated_at: DateTime<Utc>,
    pub provider_profile: String,
    pub provider_type: String,
    pub model: String,
    pub session_route: String,
    pub last_gateway_run_id: Option<String>,
    pub last_correlation_id: Option<String>,
    pub message_count: usize,
    pub last_message_preview: Option<String>,
    pub memory_summary_preview: Option<String>,
    pub reference_count: usize,
}

pub trait SessionStore: Send + Sync {
    fn load(&self, id: &str) -> Result<Option<SessionRecord>>;
    fn save(&self, session: &SessionRecord) -> Result<()>;
    fn list(&self) -> Result<Vec<SessionSummary>>;
}

#[derive(Debug, Clone)]
pub struct FileSessionStore {
    root_dir: PathBuf,
}

impl FileSessionStore {
    pub fn new(root_dir: impl Into<PathBuf>) -> Self {
        Self {
            root_dir: root_dir.into(),
        }
    }

    pub fn root_dir(&self) -> &Path {
        &self.root_dir
    }

    pub fn load_or_create(
        &self,
        id: &str,
        title: impl Into<String>,
        provider_profile: impl Into<String>,
        provider_type: impl Into<String>,
        model: impl Into<String>,
    ) -> Result<SessionRecord> {
        if let Some(session) = self.load(id)? {
            return Ok(session);
        }

        Ok(SessionRecord::new(
            id,
            title,
            provider_profile,
            provider_type,
            model,
        ))
    }

    fn ensure_root_dir(&self) -> Result<()> {
        fs::create_dir_all(&self.root_dir)?;
        Ok(())
    }

    fn session_path(&self, id: &str) -> Result<PathBuf> {
        validate_session_id(id)?;
        Ok(self.root_dir.join(format!("{id}.json")))
    }
}

impl SessionStore for FileSessionStore {
    fn load(&self, id: &str) -> Result<Option<SessionRecord>> {
        let path = self.session_path(id)?;

        if !path.exists() {
            return Ok(None);
        }

        let content = fs::read_to_string(&path)?;
        let session = normalize_loaded_session(serde_json::from_str::<SessionRecord>(&content)?);
        Ok(Some(session))
    }

    fn save(&self, session: &SessionRecord) -> Result<()> {
        self.ensure_root_dir()?;
        let path = self.session_path(&session.id)?;
        fs::write(path, serde_json::to_vec_pretty(session)?)?;
        Ok(())
    }

    fn list(&self) -> Result<Vec<SessionSummary>> {
        self.ensure_root_dir()?;
        let mut sessions = Vec::new();

        for entry in fs::read_dir(&self.root_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
                continue;
            }

            let content = fs::read_to_string(&path)?;
            let session =
                normalize_loaded_session(serde_json::from_str::<SessionRecord>(&content)?);
            sessions.push(session.summary());
        }

        sessions.sort_by(|left, right| right.updated_at.cmp(&left.updated_at));
        Ok(sessions)
    }
}

pub fn validate_session_id(id: &str) -> Result<()> {
    let trimmed = id.trim();
    if trimmed.is_empty() {
        bail!("session id must not be empty");
    }

    let path = Path::new(trimmed);
    if path.is_absolute() {
        bail!("session id must not be an absolute path");
    }

    let mut components = path.components();
    let Some(std::path::Component::Normal(_)) = components.next() else {
        bail!("session id must not contain path traversal or separators");
    };

    if components.next().is_some() {
        bail!("session id must not contain path traversal or separators");
    }

    if trimmed.chars().any(char::is_whitespace) {
        bail!("session id must not contain whitespace");
    }

    Ok(())
}

fn truncate_preview(value: &str, limit: usize) -> String {
    if value.chars().count() <= limit {
        return value.to_owned();
    }

    let truncated: String = value.chars().take(limit).collect();
    format!("{truncated}...")
}

fn normalize_loaded_session(mut session: SessionRecord) -> SessionRecord {
    if session.gateway.route.is_empty() {
        session.gateway.route = session_route_for_id(&session.id);
    }

    session
}

pub fn session_title_from_input(input: &str) -> String {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return "Untitled session".to_owned();
    }

    truncate_preview(trimmed, 48)
}

pub fn session_route_for_id(id: &str) -> String {
    format!("gateway.local/{id}")
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
            "mosaic-session-core-{label}-{}-{nanos}-{count}",
            process::id()
        ));
        fs::create_dir_all(&path).expect("temp dir should be created");
        path
    }

    #[test]
    fn session_records_append_messages_and_create_summaries() {
        let mut session = SessionRecord::new("demo", "Demo", "mock", "mock", "mock");
        session.append_message(TranscriptRole::User, "hello", None);
        session.append_message(TranscriptRole::Assistant, "world", None);
        session.set_last_run_id("run-1");
        session.set_memory_state(
            Some("Summary for demo".to_owned()),
            Some("Compressed demo context".to_owned()),
            2,
            true,
        );
        session.record_reference("related", "explicit_session_reference");

        let summary = session.summary();

        assert_eq!(summary.id, "demo");
        assert_eq!(summary.message_count, 2);
        assert_eq!(summary.last_message_preview.as_deref(), Some("world"));
        assert_eq!(summary.session_route, session_route_for_id("demo"));
        assert_eq!(
            summary.memory_summary_preview.as_deref(),
            Some("Summary for demo")
        );
        assert_eq!(summary.reference_count, 1);
        assert_eq!(session.last_run_id.as_deref(), Some("run-1"));
        assert_eq!(session.memory.compression_count, 1);
    }

    #[test]
    fn summary_backfills_missing_gateway_route_for_legacy_sessions() {
        let mut session = SessionRecord::new("demo", "Demo", "mock", "mock", "mock");
        session.gateway.route.clear();

        assert_eq!(
            session.summary().session_route,
            session_route_for_id("demo")
        );
    }

    #[test]
    fn file_session_store_roundtrips_sessions_and_lists_summaries() {
        let dir = temp_dir("roundtrip");
        let store = FileSessionStore::new(&dir);

        let mut session = SessionRecord::new("demo", "Demo", "mock", "mock", "mock");
        session.append_message(TranscriptRole::User, "hello", None);
        session.set_memory_state(Some("Stored summary".to_owned()), None, 1, false);
        store.save(&session).expect("session should save");

        let loaded = store
            .load("demo")
            .expect("load should succeed")
            .expect("session should exist");
        let summaries = store.list().expect("list should succeed");

        assert_eq!(loaded.id, "demo");
        assert_eq!(loaded.transcript.len(), 1);
        assert_eq!(
            loaded.memory.latest_summary.as_deref(),
            Some("Stored summary")
        );
        assert_eq!(summaries.len(), 1);
        assert_eq!(summaries[0].id, "demo");

        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn session_id_validation_rejects_paths_and_whitespace() {
        for invalid in ["", "../demo", "two words", "/tmp/demo", "nested/demo"] {
            let err = validate_session_id(invalid).expect_err("invalid session ids should fail");
            assert!(!err.to_string().is_empty());
        }
    }

    #[test]
    fn load_or_create_returns_existing_session_when_present() {
        let dir = temp_dir("load-or-create");
        let store = FileSessionStore::new(&dir);

        let session = SessionRecord::new("demo", "Demo", "mock", "mock", "mock");
        store.save(&session).expect("session should save");

        let loaded = store
            .load_or_create("demo", "Other", "mock", "mock", "mock")
            .expect("load_or_create should succeed");

        assert_eq!(loaded.title, "Demo");

        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn session_title_is_derived_from_user_input() {
        assert_eq!(session_title_from_input("  hello world  "), "hello world");
        assert_eq!(session_title_from_input("   "), "Untitled session");
        assert!(session_title_from_input(&"x".repeat(80)).ends_with("..."));
    }
}
