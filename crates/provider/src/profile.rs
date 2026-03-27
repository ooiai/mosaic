use std::{collections::BTreeMap, env, sync::Arc};

use anyhow::{Result, anyhow, bail};
use mosaic_config::{MosaicConfig, ProviderProfileConfig};
use serde::{Deserialize, Serialize};

use crate::{
    capabilities::{
        ModelCapabilities, ScheduledProfile, SchedulingIntent, SchedulingRequest, budget_rank,
        infer_model_capabilities,
    },
    types::LlmProvider,
    vendors::build_provider_from_profile,
};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProviderProfile {
    pub name: String,
    pub provider_type: String,
    pub model: String,
    pub base_url: Option<String>,
    pub api_key_env: Option<String>,
    pub capabilities: ModelCapabilities,
}

impl ProviderProfile {
    pub fn api_key_present(&self) -> bool {
        self.api_key_env
            .as_deref()
            .is_some_and(|env_var| env::var(env_var).is_ok())
    }
}

#[derive(Debug, Clone)]
pub struct ProviderProfileRegistry {
    active_profile: String,
    profiles: BTreeMap<String, ProviderProfile>,
}

impl ProviderProfileRegistry {
    pub fn from_config(config: &MosaicConfig) -> Result<Self> {
        if config.profiles.is_empty() {
            bail!("no provider profiles configured");
        }

        let profiles = config
            .profiles
            .iter()
            .map(|(name, profile)| (name.clone(), provider_profile_from_config(name, profile)))
            .collect::<BTreeMap<_, _>>();

        if !profiles.contains_key(&config.active_profile) {
            bail!(
                "active profile '{}' does not exist in the provider registry",
                config.active_profile
            );
        }

        Ok(Self {
            active_profile: config.active_profile.clone(),
            profiles,
        })
    }

    pub fn active_profile_name(&self) -> &str {
        &self.active_profile
    }

    pub fn active_profile(&self) -> &ProviderProfile {
        self.profiles
            .get(&self.active_profile)
            .expect("active profile should exist in registry")
    }

    pub fn list(&self) -> Vec<&ProviderProfile> {
        self.profiles.values().collect()
    }

    pub fn get(&self, name: &str) -> Option<&ProviderProfile> {
        self.profiles.get(name)
    }

    pub fn resolve(&self, name: Option<&str>) -> Result<ProviderProfile> {
        let name = name.unwrap_or(&self.active_profile);
        self.profiles
            .get(name)
            .cloned()
            .ok_or_else(|| anyhow!("unknown provider profile: {name}"))
    }

    pub fn build_provider(&self, name: Option<&str>) -> Result<Arc<dyn LlmProvider>> {
        let profile = self.resolve(name)?;
        build_provider_from_profile(&profile)
    }

    pub fn schedule(&self, request: SchedulingRequest) -> Result<ScheduledProfile> {
        if let Some(requested_profile) = request.requested_profile.as_deref() {
            let profile = self.resolve(Some(requested_profile))?;
            if request.requires_tools && !profile.capabilities.supports_tools {
                bail!(
                    "profile '{}' does not support tool-enabled runs",
                    profile.name
                );
            }
            return Ok(ScheduledProfile {
                profile,
                reason: "requested_profile".to_owned(),
            });
        }

        let mut candidates = self
            .list()
            .into_iter()
            .filter(|profile| !request.requires_tools || profile.capabilities.supports_tools)
            .cloned()
            .collect::<Vec<_>>();

        if candidates.is_empty() {
            bail!("no provider profiles satisfy the requested runtime constraints");
        }

        if let Some(channel) = request.channel.as_deref() {
            if let Some(profile) = channel_policy_profile(&candidates, channel) {
                return Ok(ScheduledProfile {
                    profile,
                    reason: format!("channel_policy:{}", channel),
                });
            }
        }

        let active = self.active_profile().clone();
        let active_available = candidates.iter().any(|profile| profile.name == active.name);
        let long_context = request.estimated_context_chars
            > active.capabilities.context_window_chars.saturating_mul(3) / 4;

        let selected = match request.intent {
            SchedulingIntent::Summary => candidates
                .into_iter()
                .min_by_key(|profile| {
                    (
                        budget_rank(&profile.capabilities.budget_tier),
                        profile.capabilities.context_window_chars,
                    )
                })
                .expect("summary scheduling should have candidates"),
            SchedulingIntent::WorkflowStep => {
                if long_context {
                    candidates
                        .into_iter()
                        .max_by_key(|profile| profile.capabilities.context_window_chars)
                        .expect("workflow scheduling should have candidates")
                } else if active_available {
                    active.clone()
                } else {
                    candidates.remove(0)
                }
            }
            SchedulingIntent::InteractiveRun => {
                if long_context {
                    candidates
                        .into_iter()
                        .max_by_key(|profile| profile.capabilities.context_window_chars)
                        .expect("interactive scheduling should have candidates")
                } else if active_available {
                    active.clone()
                } else {
                    candidates.remove(0)
                }
            }
        };

        let reason = if selected.name == active.name {
            "active_profile"
        } else if long_context {
            "expanded_context_window"
        } else if matches!(request.intent, SchedulingIntent::Summary) {
            "summary_budget_policy"
        } else {
            "capability_policy"
        };

        Ok(ScheduledProfile {
            profile: selected,
            reason: reason.to_owned(),
        })
    }
}

fn channel_policy_profile(
    candidates: &[ProviderProfile],
    channel: &str,
) -> Option<ProviderProfile> {
    candidates
        .iter()
        .find(|profile| profile.name == channel)
        .cloned()
}

pub(crate) fn provider_profile_from_config(
    name: &str,
    config: &ProviderProfileConfig,
) -> ProviderProfile {
    ProviderProfile {
        name: name.to_owned(),
        provider_type: config.provider_type.clone(),
        model: config.model.clone(),
        base_url: config.base_url.clone(),
        api_key_env: config.api_key_env.clone(),
        capabilities: infer_model_capabilities(&config.provider_type, &config.model),
    }
}
