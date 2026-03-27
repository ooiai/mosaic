use anyhow::Result;
use async_trait::async_trait;
use mosaic_config::ProviderType;

use crate::{
    ProviderProfile,
    types::{LlmProvider, Message, ProviderCompletion, ProviderTransportMetadata, ToolDefinition},
};

use super::shared::{
    OLLAMA_TIMEOUT_MS, OpenAiStyleEndpoint, OpenAiStyleProvider, RequestAuth, build_http_client,
    resolve_api_key, resolve_base_url,
};

pub struct OllamaProvider {
    inner: OpenAiStyleProvider,
}

impl OllamaProvider {
    pub fn new(
        profile_name: String,
        base_url: String,
        api_key: Option<String>,
        model: String,
    ) -> Self {
        let metadata = ProviderTransportMetadata {
            provider_type: ProviderType::Ollama.as_str().to_owned(),
            base_url: Some(base_url.clone()),
            timeout_ms: OLLAMA_TIMEOUT_MS,
            max_retries: 1,
            supports_tool_call_shadow_messages: false,
        };
        Self {
            inner: OpenAiStyleProvider {
                client: build_http_client(metadata.timeout_ms),
                provider_type: ProviderType::Ollama.as_str().to_owned(),
                profile_name,
                model,
                base_url,
                auth: api_key
                    .map(RequestAuth::Bearer)
                    .unwrap_or(RequestAuth::None),
                metadata,
                endpoint: OpenAiStyleEndpoint::Ollama,
            },
        }
    }

    pub(crate) fn from_profile(profile: &ProviderProfile) -> Result<Self> {
        Ok(Self::new(
            profile.name.clone(),
            resolve_base_url(profile, ProviderType::Ollama)?,
            resolve_api_key(profile, false)?,
            profile.model.clone(),
        ))
    }
}

#[async_trait]
impl LlmProvider for OllamaProvider {
    fn metadata(&self) -> ProviderTransportMetadata {
        self.inner.metadata()
    }

    async fn complete(
        &self,
        messages: &[Message],
        tools: Option<&[ToolDefinition]>,
    ) -> std::result::Result<ProviderCompletion, crate::ProviderError> {
        self.inner.complete(messages, tools).await
    }
}
