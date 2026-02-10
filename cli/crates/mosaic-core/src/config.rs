use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{MosaicError, Result};
use crate::state::StateMode;

pub const CURRENT_CONFIG_VERSION: u32 = 1;
pub const DEFAULT_PROFILE: &str = "default";
pub const DEFAULT_MODEL: &str = "gpt-4o-mini";
pub const DEFAULT_BASE_URL: &str = "https://api.openai.com";
pub const DEFAULT_API_KEY_ENV: &str = "OPENAI_API_KEY";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ProviderKind {
    #[serde(rename = "openai_compatible", alias = "open_ai_compatible")]
    OpenAiCompatible,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunGuardMode {
    ConfirmDangerous,
    AllConfirm,
    Unrestricted,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    pub kind: ProviderKind,
    pub base_url: String,
    pub api_key_env: String,
    pub model: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfig {
    pub temperature: f32,
    pub max_turns: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunToolConfig {
    pub guard_mode: RunGuardMode,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolsConfig {
    pub enabled: bool,
    pub run: RunToolConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileConfig {
    pub provider: ProviderConfig,
    pub agent: AgentConfig,
    pub tools: ToolsConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateConfig {
    pub mode: StateMode,
    pub project_dir: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigFile {
    pub version: u32,
    pub active_profile: String,
    pub state: StateConfig,
    pub profiles: BTreeMap<String, ProfileConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolvedConfig {
    pub profile_name: String,
    pub profile: ProfileConfig,
    pub state: StateConfig,
}

impl Default for ProviderConfig {
    fn default() -> Self {
        Self {
            kind: ProviderKind::OpenAiCompatible,
            base_url: DEFAULT_BASE_URL.to_string(),
            api_key_env: DEFAULT_API_KEY_ENV.to_string(),
            model: DEFAULT_MODEL.to_string(),
        }
    }
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            temperature: 0.2,
            max_turns: 8,
        }
    }
}

impl Default for RunToolConfig {
    fn default() -> Self {
        Self {
            guard_mode: RunGuardMode::ConfirmDangerous,
        }
    }
}

impl Default for ToolsConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            run: RunToolConfig::default(),
        }
    }
}

impl Default for ProfileConfig {
    fn default() -> Self {
        Self {
            provider: ProviderConfig::default(),
            agent: AgentConfig::default(),
            tools: ToolsConfig::default(),
        }
    }
}

impl StateConfig {
    pub fn from_mode(mode: StateMode) -> Self {
        Self {
            mode,
            project_dir: ".mosaic".to_string(),
        }
    }
}

impl ConfigFile {
    pub fn default_for_mode(mode: StateMode) -> Self {
        let mut profiles = BTreeMap::new();
        profiles.insert(DEFAULT_PROFILE.to_string(), ProfileConfig::default());
        Self {
            version: CURRENT_CONFIG_VERSION,
            active_profile: DEFAULT_PROFILE.to_string(),
            state: StateConfig::from_mode(mode),
            profiles,
        }
    }

    pub fn validate(&self) -> Result<()> {
        if self.version != CURRENT_CONFIG_VERSION {
            return Err(MosaicError::Config(format!(
                "unsupported config version {}, expected {}",
                self.version, CURRENT_CONFIG_VERSION
            )));
        }
        if self.profiles.is_empty() {
            return Err(MosaicError::Validation(
                "at least one profile must be configured".to_string(),
            ));
        }
        if self.resolve_profile(Some(&self.active_profile)).is_err() {
            return Err(MosaicError::Validation(format!(
                "active_profile '{}' does not exist",
                self.active_profile
            )));
        }
        for (name, profile) in &self.profiles {
            profile.validate().map_err(|err| {
                MosaicError::Validation(format!("profile '{name}' validation failed: {err}"))
            })?;
        }
        Ok(())
    }

    pub fn resolve_profile(&self, requested: Option<&str>) -> Result<ResolvedConfig> {
        let profile_name = requested
            .map(ToOwned::to_owned)
            .unwrap_or_else(|| self.active_profile.clone());
        let profile = self
            .profiles
            .get(&profile_name)
            .ok_or_else(|| MosaicError::Config(format!("profile '{profile_name}' not found")))?;
        Ok(ResolvedConfig {
            profile_name,
            profile: profile.clone(),
            state: self.state.clone(),
        })
    }
}

impl ProfileConfig {
    pub fn validate(&self) -> Result<()> {
        if self.provider.base_url.trim().is_empty() {
            return Err(MosaicError::Validation(
                "provider.base_url cannot be empty".to_string(),
            ));
        }
        if self.provider.api_key_env.trim().is_empty() {
            return Err(MosaicError::Validation(
                "provider.api_key_env cannot be empty".to_string(),
            ));
        }
        if self.provider.model.trim().is_empty() {
            return Err(MosaicError::Validation(
                "provider.model cannot be empty".to_string(),
            ));
        }
        if !(0.0..=2.0).contains(&self.agent.temperature) {
            return Err(MosaicError::Validation(
                "agent.temperature must be in [0.0, 2.0]".to_string(),
            ));
        }
        if self.agent.max_turns == 0 {
            return Err(MosaicError::Validation(
                "agent.max_turns must be greater than 0".to_string(),
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct ConfigManager {
    path: PathBuf,
}

impl ConfigManager {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn exists(&self) -> bool {
        self.path.exists()
    }

    pub fn load(&self) -> Result<ConfigFile> {
        let raw = fs::read_to_string(&self.path).map_err(|err| {
            if err.kind() == std::io::ErrorKind::NotFound {
                MosaicError::Config(format!(
                    "config file not found at {}. run `mosaic setup` first",
                    self.path.display()
                ))
            } else {
                MosaicError::Io(err.to_string())
            }
        })?;
        let parsed: ConfigFile = toml::from_str(&raw)?;
        parsed.validate()?;
        Ok(parsed)
    }

    pub fn load_or_default(&self, mode: StateMode) -> Result<ConfigFile> {
        if self.exists() {
            return self.load();
        }
        Ok(ConfigFile::default_for_mode(mode))
    }

    pub fn save(&self, config: &ConfigFile) -> Result<()> {
        config.validate()?;
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }
        let raw = toml::to_string_pretty(config)?;
        fs::write(&self.path, raw)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::*;
    use crate::state::StateMode;

    #[test]
    fn default_config_is_valid() {
        let config = ConfigFile::default_for_mode(StateMode::Xdg);
        assert!(config.validate().is_ok());
        assert!(config.profiles.contains_key(DEFAULT_PROFILE));
    }

    #[test]
    fn save_and_load_round_trip() {
        let temp = tempdir().unwrap();
        let path = temp.path().join("config.toml");
        let manager = ConfigManager::new(path);
        let config = ConfigFile::default_for_mode(StateMode::Project);
        manager.save(&config).unwrap();
        let loaded = manager.load().unwrap();
        assert_eq!(loaded.version, CURRENT_CONFIG_VERSION);
        assert_eq!(loaded.state.mode, StateMode::Project);
    }

    #[test]
    fn resolve_profile_uses_active_by_default() {
        let mut config = ConfigFile::default_for_mode(StateMode::Xdg);
        config.active_profile = "default".to_string();
        let resolved = config.resolve_profile(None).unwrap();
        assert_eq!(resolved.profile_name, "default");
    }
}
