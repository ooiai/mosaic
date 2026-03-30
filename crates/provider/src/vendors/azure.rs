use anyhow::Result;
use async_trait::async_trait;
use mosaic_config::ProviderType;

use crate::{
    ProviderProfile,
    types::{LlmProvider, Message, ProviderCompletion, ProviderTransportMetadata, ToolDefinition},
};

use super::shared::{
    OpenAiStyleEndpoint, OpenAiStyleProvider, RequestAuth, build_http_client, resolve_api_key,
    resolve_base_url,
};

pub struct AzureProvider {
    inner: OpenAiStyleProvider,
}

impl AzureProvider {
    pub fn new(
        profile_name: String,
        base_url: String,
        api_key: String,
        deployment: String,
        timeout_ms: u64,
        max_retries: u8,
        retry_backoff_ms: u64,
        api_version: String,
        custom_headers: std::collections::BTreeMap<String, String>,
    ) -> Self {
        let metadata = ProviderTransportMetadata {
            provider_type: ProviderType::Azure.as_str().to_owned(),
            base_url: Some(base_url.clone()),
            timeout_ms,
            max_retries,
            retry_backoff_ms,
            api_version: Some(api_version.clone()),
            version_header: None,
            custom_header_keys: custom_headers.keys().cloned().collect(),
            supports_tool_call_shadow_messages: false,
            supports_vision: deployment.starts_with("gpt-"),
        };
        Self {
            inner: OpenAiStyleProvider {
                client: build_http_client(metadata.timeout_ms),
                provider_type: ProviderType::Azure.as_str().to_owned(),
                profile_name,
                model: deployment,
                base_url,
                auth: RequestAuth::ApiKey(api_key),
                metadata,
                endpoint: OpenAiStyleEndpoint::Azure,
                api_version: Some(api_version),
                request_headers: custom_headers.into_iter().collect(),
            },
        }
    }

    pub(crate) fn from_profile(profile: &ProviderProfile) -> Result<Self> {
        Ok(Self::new(
            profile.name.clone(),
            resolve_base_url(profile, ProviderType::Azure)?,
            resolve_api_key(profile, true)?.expect("azure requires api key"),
            profile.model.clone(),
            profile.timeout_ms,
            profile.max_retries,
            profile.retry_backoff_ms,
            profile
                .azure_api_version
                .clone()
                .expect("azure profiles should carry api_version"),
            profile.custom_headers.clone(),
        ))
    }
}

#[async_trait]
impl LlmProvider for AzureProvider {
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
