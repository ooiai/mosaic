use anyhow::{Result, bail};
use mosaic_config::{ProviderType, parse_provider_type};
use mosaic_tool_core::{CapabilityInvocationMode, CapabilityVisibility, ToolMetadata};
use serde::{Deserialize, Serialize};

use crate::{profile::ProviderProfile, types::ToolDefinition};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ModelCapabilities {
    pub supports_tools: bool,
    pub supports_sessions: bool,
    pub family: String,
    pub context_window_chars: usize,
    pub budget_tier: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SchedulingIntent {
    InteractiveRun,
    Summary,
    WorkflowStep,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SchedulingRequest {
    pub requested_profile: Option<String>,
    pub channel: Option<String>,
    pub intent: SchedulingIntent,
    pub estimated_context_chars: usize,
    pub requires_tools: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ScheduledProfile {
    pub profile: ProviderProfile,
    pub reason: String,
}

pub fn validate_step_tools_support(profile: &ProviderProfile, tool_names: &[String]) -> Result<()> {
    if !tool_names.is_empty() && !profile.capabilities.supports_tools {
        bail!(
            "profile '{}' does not support tool-enabled workflow steps",
            profile.name
        );
    }

    Ok(())
}

pub fn tool_is_visible_to_model(metadata: &ToolMetadata) -> bool {
    metadata.capability.authorized
        && metadata.capability.healthy
        && metadata.exposure.visibility == CapabilityVisibility::Visible
        && metadata.exposure.invocation_mode == CapabilityInvocationMode::Conversational
}

pub fn tool_definition_from_metadata(metadata: &ToolMetadata) -> ToolDefinition {
    ToolDefinition {
        name: metadata.name.clone(),
        description: metadata.description.clone(),
        input_schema: metadata.input_schema.clone(),
    }
}

pub(crate) fn infer_model_capabilities(provider_type: &str, model: &str) -> ModelCapabilities {
    let provider_type = parse_provider_type(provider_type);
    let family = if model == "mock" {
        "mock".to_owned()
    } else if model.starts_with("gpt-5.4") {
        "gpt-5.4".to_owned()
    } else if model.starts_with("gpt-") {
        model.split('-').take(2).collect::<Vec<_>>().join("-")
    } else if model.starts_with("claude") {
        "claude".to_owned()
    } else {
        provider_type
            .map(ProviderType::as_str)
            .unwrap_or("custom")
            .to_owned()
    };

    let (context_window_chars, budget_tier) = infer_context_budget(provider_type, model);

    ModelCapabilities {
        supports_tools: matches!(
            provider_type,
            Some(
                ProviderType::Mock
                    | ProviderType::OpenAi
                    | ProviderType::Azure
                    | ProviderType::Anthropic
                    | ProviderType::Ollama
                    | ProviderType::OpenAiCompatible
            )
        ),
        supports_sessions: true,
        family,
        context_window_chars,
        budget_tier: budget_tier.to_owned(),
    }
}

fn infer_context_budget(provider_type: Option<ProviderType>, model: &str) -> (usize, &'static str) {
    if model == "mock" {
        (64_000, "medium")
    } else if model.starts_with("claude") || matches!(provider_type, Some(ProviderType::Anthropic))
    {
        (180_000, "large")
    } else if model.contains("mini") {
        (32_000, "small")
    } else if model.starts_with("gpt-5.4") {
        (128_000, "large")
    } else if model.starts_with("gpt-") {
        (64_000, "medium")
    } else if matches!(provider_type, Some(ProviderType::Ollama)) {
        (24_000, "small")
    } else {
        (24_000, "small")
    }
}

pub(crate) fn budget_rank(value: &str) -> usize {
    match value {
        "small" => 0,
        "medium" => 1,
        "large" => 2,
        _ => 3,
    }
}
