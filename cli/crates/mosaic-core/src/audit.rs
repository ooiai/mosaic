use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::error::Result;

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
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.audit_log_path)?;
        let line = serde_json::to_string(entry)?;
        file.write_all(line.as_bytes())?;
        file.write_all(b"\n")?;
        Ok(())
    }

    pub fn path(&self) -> &Path {
        &self.audit_log_path
    }
}
