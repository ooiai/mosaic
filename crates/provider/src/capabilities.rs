use anyhow::{Result, bail};
use mosaic_config::{AttachmentRouteModeConfig, ProviderType, parse_provider_type};
use mosaic_tool_core::{CapabilityInvocationMode, CapabilityVisibility, ToolMetadata};
use serde::{Deserialize, Serialize};

use crate::{profile::ProviderProfile, types::ToolDefinition};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ModelCapabilities {
    pub supports_tools: bool,
    pub supports_sessions: bool,
    pub supports_vision: bool,
    pub supports_documents: bool,
    pub supports_audio: bool,
    pub supports_video: bool,
    pub preferred_attachment_mode: AttachmentRouteModeConfig,
    pub family: String,
    pub context_window_chars: usize,
    pub budget_tier: String,
}

impl ModelCapabilities {
    pub fn infer(provider_type: &str, model: &str) -> Self {
        infer_model_capabilities(provider_type, model)
    }
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
    #[serde(default)]
    pub requires_vision: bool,
    #[serde(default)]
    pub requires_documents: bool,
    #[serde(default)]
    pub requires_audio: bool,
    #[serde(default)]
    pub requires_video: bool,
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
        supports_vision: infer_vision_support(provider_type, model),
        supports_documents: infer_document_support(provider_type, model),
        supports_audio: infer_audio_support(provider_type, model),
        supports_video: infer_video_support(provider_type, model),
        preferred_attachment_mode: infer_preferred_attachment_mode(provider_type, model),
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

fn infer_vision_support(provider_type: Option<ProviderType>, model: &str) -> bool {
    if model == "mock" {
        return true;
    }

    let normalized = model.to_ascii_lowercase();
    if normalized.contains("vision") || normalized.contains("llava") {
        return true;
    }

    match provider_type {
        Some(ProviderType::OpenAi | ProviderType::Azure | ProviderType::OpenAiCompatible) => {
            normalized.starts_with("gpt-")
        }
        Some(ProviderType::Anthropic) => normalized.starts_with("claude"),
        Some(ProviderType::Ollama) => normalized.contains("vision") || normalized.contains("llava"),
        Some(ProviderType::Mock) => true,
        None => false,
    }
}

fn infer_document_support(provider_type: Option<ProviderType>, model: &str) -> bool {
    if model == "mock" {
        return true;
    }

    let normalized = model.to_ascii_lowercase();
    if normalized.contains("document") || normalized.contains("pdf") {
        return true;
    }

    match provider_type {
        Some(ProviderType::OpenAi | ProviderType::Azure | ProviderType::OpenAiCompatible) => {
            normalized.starts_with("gpt-")
        }
        Some(ProviderType::Anthropic) => normalized.starts_with("claude"),
        Some(ProviderType::Mock) => true,
        Some(ProviderType::Ollama) | None => false,
    }
}

fn infer_audio_support(provider_type: Option<ProviderType>, model: &str) -> bool {
    if model == "mock" {
        return false;
    }

    let normalized = model.to_ascii_lowercase();
    match provider_type {
        Some(ProviderType::OpenAi | ProviderType::Azure | ProviderType::OpenAiCompatible) => {
            normalized.contains("audio")
        }
        Some(ProviderType::Anthropic) => normalized.contains("audio"),
        Some(ProviderType::Mock | ProviderType::Ollama) | None => false,
    }
}

fn infer_video_support(provider_type: Option<ProviderType>, model: &str) -> bool {
    if model == "mock" {
        return false;
    }

    let normalized = model.to_ascii_lowercase();
    match provider_type {
        Some(ProviderType::OpenAi | ProviderType::Azure | ProviderType::OpenAiCompatible) => {
            normalized.contains("video")
        }
        Some(ProviderType::Anthropic) => normalized.contains("video"),
        Some(ProviderType::Mock | ProviderType::Ollama) | None => false,
    }
}

fn infer_preferred_attachment_mode(
    provider_type: Option<ProviderType>,
    model: &str,
) -> AttachmentRouteModeConfig {
    if infer_vision_support(provider_type, model) || infer_document_support(provider_type, model) {
        AttachmentRouteModeConfig::ProviderNative
    } else {
        AttachmentRouteModeConfig::SpecializedProcessor
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
