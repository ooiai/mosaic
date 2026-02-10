use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use mosaic_core::error::{MosaicError, Result};

const CURRENT_SANDBOX_POLICY_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SandboxProfile {
    Restricted,
    Standard,
    Elevated,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxPolicy {
    pub version: u32,
    pub profile: SandboxProfile,
}

#[derive(Debug, Clone, Serialize)]
pub struct SandboxProfileInfo {
    pub profile: SandboxProfile,
    pub description: String,
    pub blocked_examples: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct SandboxStore {
    path: PathBuf,
}

impl Default for SandboxPolicy {
    fn default() -> Self {
        Self {
            version: CURRENT_SANDBOX_POLICY_VERSION,
            profile: SandboxProfile::Standard,
        }
    }
}

impl SandboxPolicy {
    pub fn validate(&self) -> Result<()> {
        if self.version != CURRENT_SANDBOX_POLICY_VERSION {
            return Err(MosaicError::Config(format!(
                "unsupported sandbox policy version {}, expected {}",
                self.version, CURRENT_SANDBOX_POLICY_VERSION
            )));
        }
        Ok(())
    }
}

impl SandboxStore {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn load_or_default(&self) -> Result<SandboxPolicy> {
        if !self.path.exists() {
            return Ok(SandboxPolicy::default());
        }
        let raw = std::fs::read_to_string(&self.path)?;
        let policy: SandboxPolicy = toml::from_str(&raw).map_err(|err| {
            MosaicError::Config(format!(
                "invalid sandbox policy {}: {err}",
                self.path.display()
            ))
        })?;
        policy.validate()?;
        Ok(policy)
    }

    pub fn save(&self, policy: &SandboxPolicy) -> Result<()> {
        let mut policy = policy.clone();
        policy.version = CURRENT_SANDBOX_POLICY_VERSION;
        policy.validate()?;
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let rendered = toml::to_string_pretty(&policy)?;
        std::fs::write(&self.path, rendered)?;
        Ok(())
    }

    pub fn set_profile(&self, profile: SandboxProfile) -> Result<SandboxPolicy> {
        let mut policy = self.load_or_default()?;
        policy.profile = profile;
        self.save(&policy)?;
        Ok(policy)
    }
}

pub fn evaluate_sandbox(command: &str, profile: SandboxProfile) -> Option<String> {
    if profile != SandboxProfile::Restricted {
        return None;
    }

    let normalized = command.trim().to_lowercase();
    let blocked_patterns = [
        "curl ",
        "wget ",
        "ssh ",
        "scp ",
        "nc ",
        "ncat ",
        "telnet ",
        "docker ",
        "kubectl ",
        "sudo ",
        "brew install",
        "apt-get",
    ];
    if blocked_patterns
        .iter()
        .any(|pattern| normalized.starts_with(pattern) || normalized.contains(pattern))
    {
        return Some("sandbox profile 'restricted' blocks network/system commands".to_string());
    }

    None
}

pub fn profile_info(profile: SandboxProfile) -> SandboxProfileInfo {
    match profile {
        SandboxProfile::Restricted => SandboxProfileInfo {
            profile,
            description:
                "Disallow network/system-impacting shell commands and require local-only execution"
                    .to_string(),
            blocked_examples: vec![
                "curl https://...".to_string(),
                "ssh user@host".to_string(),
                "docker build .".to_string(),
            ],
        },
        SandboxProfile::Standard => SandboxProfileInfo {
            profile,
            description:
                "Allow standard local development commands; high-risk actions still need approval"
                    .to_string(),
            blocked_examples: Vec::new(),
        },
        SandboxProfile::Elevated => SandboxProfileInfo {
            profile,
            description: "Least restrictive profile for trusted controlled environments"
                .to_string(),
            blocked_examples: Vec::new(),
        },
    }
}

pub fn list_profiles() -> Vec<SandboxProfileInfo> {
    vec![
        profile_info(SandboxProfile::Restricted),
        profile_info(SandboxProfile::Standard),
        profile_info(SandboxProfile::Elevated),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn restricted_profile_blocks_network_commands() {
        let reason = evaluate_sandbox("curl https://example.com", SandboxProfile::Restricted);
        assert!(reason.is_some());
    }

    #[test]
    fn standard_profile_allows_network_commands() {
        let reason = evaluate_sandbox("curl https://example.com", SandboxProfile::Standard);
        assert!(reason.is_none());
    }
}
