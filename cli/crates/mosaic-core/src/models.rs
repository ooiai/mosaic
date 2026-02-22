use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{MosaicError, Result};

const CURRENT_MODELS_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelsConfig {
    pub version: u32,
    #[serde(default)]
    pub profiles: BTreeMap<String, ModelProfileConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ModelProfileConfig {
    #[serde(default)]
    pub aliases: BTreeMap<String, String>,
    #[serde(default)]
    pub fallbacks: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct ModelRoutingStore {
    path: PathBuf,
}

impl Default for ModelsConfig {
    fn default() -> Self {
        Self {
            version: CURRENT_MODELS_VERSION,
            profiles: BTreeMap::new(),
        }
    }
}

impl ModelsConfig {
    pub fn normalize(&mut self) {
        let mut normalized_profiles = BTreeMap::new();
        for (name, mut profile) in std::mem::take(&mut self.profiles) {
            let normalized_name = name.trim().to_string();
            if normalized_name.is_empty() {
                continue;
            }
            profile.normalize();
            normalized_profiles.insert(normalized_name, profile);
        }
        self.profiles = normalized_profiles;
    }

    pub fn validate(&self) -> Result<()> {
        if self.version != CURRENT_MODELS_VERSION {
            return Err(MosaicError::Config(format!(
                "unsupported models config version {}, expected {}",
                self.version, CURRENT_MODELS_VERSION
            )));
        }
        Ok(())
    }
}

impl ModelProfileConfig {
    pub fn normalize(&mut self) {
        let mut aliases = BTreeMap::new();
        for (alias, target) in std::mem::take(&mut self.aliases) {
            let normalized_alias = normalize_alias(&alias);
            let normalized_target = normalize_model_ref(&target);
            if normalized_alias.is_empty() || normalized_target.is_empty() {
                continue;
            }
            aliases.insert(normalized_alias, normalized_target);
        }
        self.aliases = aliases;
        self.fallbacks = normalize_model_list(std::mem::take(&mut self.fallbacks));
    }

    pub fn resolve_model_ref(&self, model_ref: &str) -> String {
        let normalized = normalize_alias(model_ref);
        if let Some(target) = self.aliases.get(&normalized) {
            return target.clone();
        }
        normalize_model_ref(model_ref)
    }
}

impl ModelRoutingStore {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn load_or_default(&self) -> Result<ModelsConfig> {
        if !self.path.exists() {
            return Ok(ModelsConfig::default());
        }
        let raw = std::fs::read_to_string(&self.path)?;
        let mut config: ModelsConfig = toml::from_str(&raw).map_err(|err| {
            MosaicError::Config(format!(
                "invalid models config {}: {err}",
                self.path.display()
            ))
        })?;
        config.validate()?;
        config.normalize();
        Ok(config)
    }

    pub fn save(&self, config: &ModelsConfig) -> Result<()> {
        let mut config = config.clone();
        config.version = CURRENT_MODELS_VERSION;
        config.normalize();
        config.validate()?;
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let rendered = toml::to_string_pretty(&config)?;
        std::fs::write(&self.path, rendered)?;
        Ok(())
    }

    pub fn profile(&self, profile_name: &str) -> Result<ModelProfileConfig> {
        let profile_name = normalize_profile_name(profile_name)?;
        let config = self.load_or_default()?;
        Ok(config
            .profiles
            .get(&profile_name)
            .cloned()
            .unwrap_or_default())
    }

    pub fn set_alias(
        &self,
        profile_name: &str,
        alias: &str,
        model_ref: &str,
    ) -> Result<ModelProfileConfig> {
        let profile_name = normalize_profile_name(profile_name)?;
        let alias = normalize_alias(alias);
        if alias.is_empty() {
            return Err(MosaicError::Validation("alias cannot be empty".to_string()));
        }
        let model_ref = normalize_model_ref(model_ref);
        if model_ref.is_empty() {
            return Err(MosaicError::Validation(
                "model reference cannot be empty".to_string(),
            ));
        }

        let mut config = self.load_or_default()?;
        let profile = config.profiles.entry(profile_name.clone()).or_default();
        profile.aliases.insert(alias, model_ref);
        profile.normalize();
        self.save(&config)?;
        self.profile(profile_name.as_str())
    }

    pub fn remove_alias(&self, profile_name: &str, alias: &str) -> Result<ModelProfileConfig> {
        let profile_name = normalize_profile_name(profile_name)?;
        let alias = normalize_alias(alias);
        if alias.is_empty() {
            return Err(MosaicError::Validation("alias cannot be empty".to_string()));
        }

        let mut config = self.load_or_default()?;
        let profile = config.profiles.entry(profile_name.clone()).or_default();
        profile.aliases.remove(&alias);
        profile.normalize();
        self.save(&config)?;
        self.profile(profile_name.as_str())
    }

    pub fn clear_aliases(&self, profile_name: &str) -> Result<ModelProfileConfig> {
        let profile_name = normalize_profile_name(profile_name)?;
        let mut config = self.load_or_default()?;
        let profile = config.profiles.entry(profile_name.clone()).or_default();
        profile.aliases.clear();
        profile.normalize();
        self.save(&config)?;
        self.profile(profile_name.as_str())
    }

    pub fn add_fallback(&self, profile_name: &str, model_ref: &str) -> Result<ModelProfileConfig> {
        let profile_name = normalize_profile_name(profile_name)?;
        let model_ref = normalize_model_ref(model_ref);
        if model_ref.is_empty() {
            return Err(MosaicError::Validation(
                "fallback model cannot be empty".to_string(),
            ));
        }
        let mut config = self.load_or_default()?;
        let profile = config.profiles.entry(profile_name.clone()).or_default();
        profile.fallbacks.push(model_ref);
        profile.normalize();
        self.save(&config)?;
        self.profile(profile_name.as_str())
    }

    pub fn remove_fallback(
        &self,
        profile_name: &str,
        model_ref: &str,
    ) -> Result<ModelProfileConfig> {
        let profile_name = normalize_profile_name(profile_name)?;
        let model_ref = normalize_model_ref(model_ref);
        if model_ref.is_empty() {
            return Err(MosaicError::Validation(
                "fallback model cannot be empty".to_string(),
            ));
        }
        let mut config = self.load_or_default()?;
        let profile = config.profiles.entry(profile_name.clone()).or_default();
        profile.fallbacks.retain(|item| item != &model_ref);
        profile.normalize();
        self.save(&config)?;
        self.profile(profile_name.as_str())
    }

    pub fn clear_fallbacks(&self, profile_name: &str) -> Result<ModelProfileConfig> {
        let profile_name = normalize_profile_name(profile_name)?;
        let mut config = self.load_or_default()?;
        let profile = config.profiles.entry(profile_name.clone()).or_default();
        profile.fallbacks.clear();
        profile.normalize();
        self.save(&config)?;
        self.profile(profile_name.as_str())
    }
}

fn normalize_profile_name(value: &str) -> Result<String> {
    let normalized = value.trim().to_string();
    if normalized.is_empty() {
        return Err(MosaicError::Validation(
            "profile name cannot be empty".to_string(),
        ));
    }
    Ok(normalized)
}

fn normalize_alias(value: &str) -> String {
    value.trim().to_ascii_lowercase()
}

fn normalize_model_ref(value: &str) -> String {
    value.trim().to_string()
}

fn normalize_model_list(values: Vec<String>) -> Vec<String> {
    let mut seen = BTreeSet::new();
    let mut out = Vec::new();
    for value in values {
        let normalized = normalize_model_ref(&value);
        if normalized.is_empty() {
            continue;
        }
        if seen.insert(normalized.clone()) {
            out.push(normalized);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::*;

    #[test]
    fn profile_resolves_aliases_case_insensitive() {
        let mut profile = ModelProfileConfig::default();
        profile
            .aliases
            .insert("primary".to_string(), "gpt-4o-mini".to_string());
        profile.normalize();
        assert_eq!(profile.resolve_model_ref("PRIMARY"), "gpt-4o-mini");
        assert_eq!(profile.resolve_model_ref("gpt-4.1"), "gpt-4.1");
    }

    #[test]
    fn store_round_trip_aliases_and_fallbacks() {
        let temp = tempdir().expect("tempdir");
        let store = ModelRoutingStore::new(temp.path().join("models.toml"));

        let profile = store
            .set_alias("default", "fast", "gpt-4o-mini")
            .expect("set alias");
        assert_eq!(
            profile.aliases.get("fast").expect("fast alias"),
            "gpt-4o-mini"
        );

        let profile = store
            .add_fallback("default", "gpt-4.1-mini")
            .expect("add fallback");
        assert_eq!(profile.fallbacks, vec!["gpt-4.1-mini".to_string()]);

        let profile = store
            .add_fallback("default", "gpt-4.1-mini")
            .expect("add duplicate fallback");
        assert_eq!(profile.fallbacks, vec!["gpt-4.1-mini".to_string()]);

        let profile = store.remove_alias("default", "FAST").expect("remove alias");
        assert!(profile.aliases.is_empty());

        let profile = store.clear_fallbacks("default").expect("clear fallbacks");
        assert!(profile.fallbacks.is_empty());
    }
}
