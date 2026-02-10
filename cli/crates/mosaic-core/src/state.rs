use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{MosaicError, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StateMode {
    Xdg,
    Project,
}

#[derive(Debug, Clone)]
pub struct StatePaths {
    pub mode: StateMode,
    pub root_dir: PathBuf,
    pub config_path: PathBuf,
    pub data_dir: PathBuf,
    pub policy_dir: PathBuf,
    pub approvals_policy_path: PathBuf,
    pub sandbox_policy_path: PathBuf,
    pub system_events_path: PathBuf,
    pub sessions_dir: PathBuf,
    pub audit_dir: PathBuf,
    pub audit_log_path: PathBuf,
}

impl StatePaths {
    pub fn resolve(mode: StateMode, cwd: &Path, project_dir_name: &str) -> Result<Self> {
        match mode {
            StateMode::Xdg => Self::resolve_xdg(),
            StateMode::Project => Ok(Self::resolve_project(cwd, project_dir_name)),
        }
    }

    fn resolve_xdg() -> Result<Self> {
        let config_home = dirs::config_dir().ok_or_else(|| {
            MosaicError::Config("unable to resolve XDG config directory".to_string())
        })?;
        let data_home = dirs::data_dir().ok_or_else(|| {
            MosaicError::Config("unable to resolve XDG data directory".to_string())
        })?;

        let root_dir = config_home.join("mosaic");
        let config_path = root_dir.join("config.toml");
        let data_dir = data_home.join("mosaic");
        let policy_dir = root_dir.join("policy");
        let approvals_policy_path = policy_dir.join("approvals.toml");
        let sandbox_policy_path = policy_dir.join("sandbox.toml");
        let system_events_path = data_dir.join("system-events.jsonl");
        let sessions_dir = data_dir.join("sessions");
        let audit_dir = data_dir.join("audit");
        let audit_log_path = audit_dir.join("commands.jsonl");

        Ok(Self {
            mode: StateMode::Xdg,
            root_dir,
            config_path,
            data_dir,
            policy_dir,
            approvals_policy_path,
            sandbox_policy_path,
            system_events_path,
            sessions_dir,
            audit_dir,
            audit_log_path,
        })
    }

    fn resolve_project(cwd: &Path, project_dir_name: &str) -> Self {
        let root_dir = cwd.join(project_dir_name);
        let config_path = root_dir.join("config.toml");
        let data_dir = root_dir.join("data");
        let policy_dir = root_dir.join("policy");
        let approvals_policy_path = policy_dir.join("approvals.toml");
        let sandbox_policy_path = policy_dir.join("sandbox.toml");
        let system_events_path = data_dir.join("system-events.jsonl");
        let sessions_dir = data_dir.join("sessions");
        let audit_dir = data_dir.join("audit");
        let audit_log_path = audit_dir.join("commands.jsonl");

        Self {
            mode: StateMode::Project,
            root_dir,
            config_path,
            data_dir,
            policy_dir,
            approvals_policy_path,
            sandbox_policy_path,
            system_events_path,
            sessions_dir,
            audit_dir,
            audit_log_path,
        }
    }

    pub fn ensure_dirs(&self) -> Result<()> {
        if let Some(parent) = self.config_path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::create_dir_all(&self.data_dir)?;
        fs::create_dir_all(&self.policy_dir)?;
        fs::create_dir_all(&self.sessions_dir)?;
        fs::create_dir_all(&self.audit_dir)?;
        Ok(())
    }

    pub fn is_writable(&self) -> Result<()> {
        self.ensure_dirs()?;
        let probe = self.data_dir.join(".write_probe");
        fs::write(&probe, b"ok")?;
        fs::remove_file(&probe)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::*;

    #[test]
    fn project_mode_uses_workspace_dir() {
        let temp = tempdir().unwrap();
        let paths = StatePaths::resolve(StateMode::Project, temp.path(), ".mosaic").unwrap();
        assert!(paths.config_path.ends_with(".mosaic/config.toml"));
        assert!(paths.sessions_dir.ends_with(".mosaic/data/sessions"));
    }
}
