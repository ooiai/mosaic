use std::{collections::BTreeMap, env, sync::Arc};

use anyhow::{Result, anyhow, bail};
use mosaic_config::{
    MosaicConfig, ProviderAttachmentRoutingConfig, ProviderProfileConfig, ProviderType,
    ProviderUsage, parse_provider_type,
};
use serde::{Deserialize, Serialize};

use crate::{
    capabilities::{
        ModelCapabilities, ScheduledProfile, SchedulingIntent, SchedulingRequest, budget_rank,
        infer_model_capabilities,
    },
    types::LlmProvider,
    vendors::build_provider_from_profile,
    vendors::shared::{
        ANTHROPIC_TIMEOUT_MS, ANTHROPIC_VERSION_HEADER, AZURE_CHAT_COMPLETIONS_API_VERSION,
        DEFAULT_MAX_RETRIES, DEFAULT_TIMEOUT_MS, OLLAMA_TIMEOUT_MS,
    },
};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProviderProfile {
    pub name: String,
    pub provider_type: String,
    pub usage: ProviderUsage,
    pub model: String,
    pub base_url: Option<String>,
    pub api_key_env: Option<String>,
    pub timeout_ms: u64,
    pub max_retries: u8,
    pub retry_backoff_ms: u64,
    #[serde(default)]
    pub custom_headers: BTreeMap<String, String>,
    #[serde(default)]
    pub allow_custom_headers: bool,
    pub azure_api_version: Option<String>,
    pub anthropic_version: Option<String>,
    pub attachment_routing: ProviderAttachmentRoutingConfig,
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
            .map(|(name, profile)| {
                (
                    name.clone(),
                    provider_profile_from_config(config, name, profile),
                )
            })
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
            if request.requires_vision && !profile.capabilities.supports_vision {
                bail!(
                    "profile '{}' does not support multimodal attachment runs",
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
            .filter(|profile| !request.requires_vision || profile.capabilities.supports_vision)
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
    root_config: &MosaicConfig,
    name: &str,
    profile_config: &ProviderProfileConfig,
) -> ProviderProfile {
    let provider_type = parse_provider_type(&profile_config.provider_type);
    let timeout_ms = profile_config
        .transport
        .timeout_ms
        .or(root_config.provider_defaults.timeout_ms)
        .unwrap_or_else(|| default_timeout_ms(provider_type));
    let max_retries = profile_config
        .transport
        .max_retries
        .or(root_config.provider_defaults.max_retries)
        .unwrap_or_else(|| default_max_retries(provider_type));
    let retry_backoff_ms = profile_config
        .transport
        .retry_backoff_ms
        .or(root_config.provider_defaults.retry_backoff_ms)
        .unwrap_or_else(|| default_retry_backoff_ms(provider_type));
    let allow_custom_headers = profile_config.vendor.allow_custom_headers;
    let custom_headers = if allow_custom_headers {
        profile_config.transport.custom_headers.clone()
    } else {
        BTreeMap::new()
    };
    let azure_api_version = match provider_type {
        Some(ProviderType::Azure) => Some(
            profile_config
                .vendor
                .azure_api_version
                .clone()
                .unwrap_or_else(|| AZURE_CHAT_COMPLETIONS_API_VERSION.to_owned()),
        ),
        _ => None,
    };
    let anthropic_version = match provider_type {
        Some(ProviderType::Anthropic) => Some(
            profile_config
                .vendor
                .anthropic_version
                .clone()
                .unwrap_or_else(|| ANTHROPIC_VERSION_HEADER.to_owned()),
        ),
        _ => None,
    };

    ProviderProfile {
        name: name.to_owned(),
        provider_type: profile_config.provider_type.clone(),
        usage: provider_type
            .map(ProviderType::usage)
            .unwrap_or(ProviderUsage::Compatibility),
        model: profile_config.model.clone(),
        base_url: profile_config.base_url.clone(),
        api_key_env: profile_config.api_key_env.clone(),
        timeout_ms,
        max_retries,
        retry_backoff_ms,
        custom_headers,
        allow_custom_headers,
        azure_api_version,
        anthropic_version,
        attachment_routing: ProviderAttachmentRoutingConfig {
            mode: profile_config.attachments.mode,
            processor: profile_config.attachments.processor.clone(),
        },
        capabilities: infer_model_capabilities(
            &profile_config.provider_type,
            &profile_config.model,
        ),
    }
}

fn default_timeout_ms(provider_type: Option<ProviderType>) -> u64 {
    match provider_type {
        Some(ProviderType::Anthropic) => ANTHROPIC_TIMEOUT_MS,
        Some(ProviderType::Ollama) => OLLAMA_TIMEOUT_MS,
        Some(ProviderType::Mock) => 0,
        _ => DEFAULT_TIMEOUT_MS,
    }
}

fn default_max_retries(provider_type: Option<ProviderType>) -> u8 {
    match provider_type {
        Some(ProviderType::Mock) => 0,
        Some(ProviderType::Ollama) => 1,
        _ => DEFAULT_MAX_RETRIES,
    }
}

fn default_retry_backoff_ms(provider_type: Option<ProviderType>) -> u64 {
    match provider_type {
        Some(ProviderType::Mock) => 0,
        _ => 150,
    }
}
