use std::fs::{self};
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::privacy::append_sanitized_jsonl;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandAudit {
    pub id: String,
    pub ts: DateTime<Utc>,
    pub session_id: String,
    pub command: String,
    pub cwd: String,
    pub approved_by: String,
    pub exit_code: i32,
    pub duration_ms: u128,
}

#[derive(Debug, Clone)]
pub struct AuditStore {
    audit_dir: PathBuf,
    audit_log_path: PathBuf,
}

impl AuditStore {
    pub fn new(audit_dir: PathBuf, audit_log_path: PathBuf) -> Self {
        Self {
            audit_dir,
            audit_log_path,
        }
    }

    pub fn ensure_dirs(&self) -> Result<()> {
        fs::create_dir_all(&self.audit_dir)?;
        Ok(())
    }

    pub fn append_command(&self, entry: &CommandAudit) -> Result<()> {
        self.ensure_dirs()?;
        append_sanitized_jsonl(&self.audit_log_path, entry, "command audit persistence")
    }

    pub fn path(&self) -> &Path {
        &self.audit_log_path
    }
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;
    use uuid::Uuid;

    use super::*;

    #[test]
    fn append_command_redacts_secret_like_command() {
        let temp = tempdir().expect("tempdir");
        let store = AuditStore::new(
            temp.path().join("audit"),
            temp.path().join("audit").join("commands.jsonl"),
        );
        store
            .append_command(&CommandAudit {
                id: Uuid::new_v4().to_string(),
                ts: Utc::now(),
                session_id: "s1".to_string(),
                command: "echo sk-live-secret-12345678901234567890".to_string(),
                cwd: temp.path().display().to_string(),
                approved_by: "flag_yes".to_string(),
                exit_code: 0,
                duration_ms: 1,
            })
            .expect("append");
        let raw = std::fs::read_to_string(store.path()).expect("read audit log");
        assert!(raw.contains("[REDACTED_OPENAI_KEY]"));
    }

    #[test]
    fn append_command_blocks_private_key_material() {
        let temp = tempdir().expect("tempdir");
        let store = AuditStore::new(
            temp.path().join("audit"),
            temp.path().join("audit").join("commands.jsonl"),
        );
        let err = store
            .append_command(&CommandAudit {
                id: Uuid::new_v4().to_string(),
                ts: Utc::now(),
                session_id: "s1".to_string(),
                command: "cat <<'EOF'\n-----BEGIN PRIVATE KEY-----\nEOF".to_string(),
                cwd: temp.path().display().to_string(),
                approved_by: "flag_yes".to_string(),
                exit_code: 0,
                duration_ms: 1,
            })
            .expect_err("should block");
        assert!(err.to_string().contains("private key material"));
    }
}
