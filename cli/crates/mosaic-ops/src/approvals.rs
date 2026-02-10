use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use mosaic_core::error::{MosaicError, Result};

const CURRENT_APPROVAL_POLICY_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalMode {
    Deny,
    Confirm,
    Allowlist,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApprovalPolicy {
    pub version: u32,
    pub mode: ApprovalMode,
    #[serde(default)]
    pub allowlist: Vec<String>,
}

#[derive(Debug, Clone)]
pub enum ApprovalDecision {
    Auto { approved_by: String },
    NeedsConfirmation { reason: String },
    Deny { reason: String },
}

#[derive(Debug, Clone)]
pub struct ApprovalStore {
    path: PathBuf,
}

impl Default for ApprovalPolicy {
    fn default() -> Self {
        Self {
            version: CURRENT_APPROVAL_POLICY_VERSION,
            mode: ApprovalMode::Confirm,
            allowlist: Vec::new(),
        }
    }
}

impl ApprovalPolicy {
    pub fn normalize(&mut self) {
        let mut deduped = BTreeSet::new();
        for prefix in self.allowlist.drain(..) {
            let normalized = normalize_prefix(&prefix);
            if !normalized.is_empty() {
                deduped.insert(normalized);
            }
        }
        self.allowlist = deduped.into_iter().collect();
    }

    pub fn matches_allowlist(&self, command: &str) -> bool {
        let normalized = command.trim().to_lowercase();
        self.allowlist.iter().any(|prefix| {
            normalized == *prefix
                || normalized.starts_with(&format!("{prefix} "))
                || normalized.starts_with(&format!("{prefix}/"))
        })
    }

    pub fn validate(&self) -> Result<()> {
        if self.version != CURRENT_APPROVAL_POLICY_VERSION {
            return Err(MosaicError::Config(format!(
                "unsupported approvals policy version {}, expected {}",
                self.version, CURRENT_APPROVAL_POLICY_VERSION
            )));
        }
        Ok(())
    }
}

impl ApprovalStore {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn load_or_default(&self) -> Result<ApprovalPolicy> {
        if !self.path.exists() {
            return Ok(ApprovalPolicy::default());
        }

        let raw = std::fs::read_to_string(&self.path)?;
        let mut policy: ApprovalPolicy = toml::from_str(&raw).map_err(|err| {
            MosaicError::Config(format!(
                "invalid approvals policy {}: {err}",
                self.path.display()
            ))
        })?;
        policy.validate()?;
        policy.normalize();
        Ok(policy)
    }

    pub fn save(&self, policy: &ApprovalPolicy) -> Result<()> {
        let mut policy = policy.clone();
        policy.version = CURRENT_APPROVAL_POLICY_VERSION;
        policy.normalize();
        policy.validate()?;
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let rendered = toml::to_string_pretty(&policy)?;
        std::fs::write(&self.path, rendered)?;
        Ok(())
    }

    pub fn set_mode(&self, mode: ApprovalMode) -> Result<ApprovalPolicy> {
        let mut policy = self.load_or_default()?;
        policy.mode = mode;
        self.save(&policy)?;
        Ok(policy)
    }

    pub fn add_allowlist(&self, prefix: &str) -> Result<ApprovalPolicy> {
        let mut policy = self.load_or_default()?;
        let normalized = normalize_prefix(prefix);
        if normalized.is_empty() {
            return Err(MosaicError::Validation(
                "allowlist prefix cannot be empty".to_string(),
            ));
        }
        policy.allowlist.push(normalized);
        self.save(&policy)?;
        self.load_or_default()
    }

    pub fn remove_allowlist(&self, prefix: &str) -> Result<ApprovalPolicy> {
        let mut policy = self.load_or_default()?;
        let normalized = normalize_prefix(prefix);
        policy.allowlist.retain(|item| item != &normalized);
        self.save(&policy)?;
        self.load_or_default()
    }
}

pub fn evaluate_approval(command: &str, policy: &ApprovalPolicy) -> ApprovalDecision {
    match policy.mode {
        ApprovalMode::Deny => ApprovalDecision::Deny {
            reason: "approval mode is set to deny".to_string(),
        },
        ApprovalMode::Confirm => ApprovalDecision::NeedsConfirmation {
            reason: "approval mode requires confirmation".to_string(),
        },
        ApprovalMode::Allowlist => {
            if policy.matches_allowlist(command) {
                ApprovalDecision::Auto {
                    approved_by: "approval_allowlist".to_string(),
                }
            } else {
                ApprovalDecision::NeedsConfirmation {
                    reason: "command is not in approvals allowlist".to_string(),
                }
            }
        }
    }
}

fn normalize_prefix(prefix: &str) -> String {
    prefix.trim().to_lowercase()
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::*;

    #[test]
    fn allowlist_mode_matches_prefix() {
        let policy = ApprovalPolicy {
            version: CURRENT_APPROVAL_POLICY_VERSION,
            mode: ApprovalMode::Allowlist,
            allowlist: vec!["cargo test".to_string()],
        };
        assert!(policy.matches_allowlist("cargo test --workspace"));
        assert!(!policy.matches_allowlist("cargo run"));
    }

    #[test]
    fn store_round_trip() {
        let temp = tempdir().expect("tempdir");
        let path = temp.path().join("approvals.toml");
        let store = ApprovalStore::new(path);
        let policy = store.set_mode(ApprovalMode::Allowlist).expect("set mode");
        assert_eq!(policy.mode, ApprovalMode::Allowlist);

        let policy = store.add_allowlist("cargo test").expect("add allowlist");
        assert_eq!(policy.allowlist, vec!["cargo test".to_string()]);

        let policy = store
            .remove_allowlist("cargo test")
            .expect("remove allowlist");
        assert!(policy.allowlist.is_empty());
    }
}
