mod anthropic;
mod azure;
mod mock;
mod ollama;
mod openai;
mod openai_compatible;
pub(crate) mod shared;

use std::sync::Arc;

use anyhow::{Result, anyhow};
use mosaic_config::{ProviderType, parse_provider_type};

use crate::{profile::ProviderProfile, types::LlmProvider};

pub use anthropic::AnthropicProvider;
pub use azure::AzureProvider;
pub use mock::MockProvider;
pub use ollama::OllamaProvider;
pub use openai::OpenAiProvider;
pub use openai_compatible::OpenAiCompatibleProvider;

pub fn build_provider_from_profile(profile: &ProviderProfile) -> Result<Arc<dyn LlmProvider>> {
    let provider_type = parse_provider_type(&profile.provider_type)
        .ok_or_else(|| anyhow!("unsupported provider type: {}", profile.provider_type))?;

    let provider: Arc<dyn LlmProvider> = match provider_type {
        ProviderType::Mock => Arc::new(MockProvider),
        ProviderType::OpenAi => Arc::new(OpenAiProvider::from_profile(profile)?),
        ProviderType::Azure => Arc::new(AzureProvider::from_profile(profile)?),
        ProviderType::Anthropic => Arc::new(AnthropicProvider::from_profile(profile)?),
        ProviderType::Ollama => Arc::new(OllamaProvider::from_profile(profile)?),
        ProviderType::OpenAiCompatible => {
            Arc::new(OpenAiCompatibleProvider::from_profile(profile)?)
        }
    };

    Ok(provider)
}
