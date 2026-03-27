use anyhow::Result;
use async_trait::async_trait;
use mosaic_config::ProviderType;

use crate::{
    ProviderProfile,
    types::{
        LlmProvider, Message, ProviderCompletion, ProviderTransportMetadata, ToolCall,
        ToolDefinition,
    },
};

use super::shared::{
    ANTHROPIC_TIMEOUT_MS, ANTHROPIC_VERSION_HEADER, AnthropicResponse, JsonRequest, RequestAuth,
    build_anthropic_body, build_http_client, execute_with_retry, parse_anthropic_response,
    resolve_api_key, resolve_base_url, send_json_request, tool_call_shadow_content,
};

pub struct AnthropicProvider {
    client: reqwest::Client,
    profile_name: String,
    base_url: String,
    api_key: String,
    model: String,
    metadata: ProviderTransportMetadata,
}

impl AnthropicProvider {
    pub fn new(profile_name: String, base_url: String, api_key: String, model: String) -> Self {
        let metadata = ProviderTransportMetadata {
            provider_type: ProviderType::Anthropic.as_str().to_owned(),
            base_url: Some(base_url.clone()),
            timeout_ms: ANTHROPIC_TIMEOUT_MS,
            max_retries: super::shared::DEFAULT_MAX_RETRIES,
            supports_tool_call_shadow_messages: true,
        };
        Self {
            client: build_http_client(metadata.timeout_ms),
            profile_name,
            base_url,
            api_key,
            model,
            metadata,
        }
    }

    pub(crate) fn from_profile(profile: &ProviderProfile) -> Result<Self> {
        Ok(Self::new(
            profile.name.clone(),
            resolve_base_url(profile, ProviderType::Anthropic)?,
            resolve_api_key(profile, true)?.expect("anthropic requires api key"),
            profile.model.clone(),
        ))
    }
}

#[async_trait]
impl LlmProvider for AnthropicProvider {
    fn metadata(&self) -> ProviderTransportMetadata {
        self.metadata.clone()
    }

    fn tool_call_shadow_message(&self, tool_calls: &[ToolCall]) -> Option<Message> {
        Some(Message {
            role: crate::Role::Assistant,
            content: tool_call_shadow_content(tool_calls),
            tool_call_id: None,
        })
    }

    async fn complete(
        &self,
        messages: &[Message],
        tools: Option<&[ToolDefinition]>,
    ) -> std::result::Result<ProviderCompletion, crate::ProviderError> {
        let url = {
            let trimmed = self.base_url.trim_end_matches('/');
            if trimmed.ends_with("/messages") {
                trimmed.to_owned()
            } else {
                format!("{trimmed}/messages")
            }
        };
        let body = build_anthropic_body(&self.model, messages, tools);
        let request = JsonRequest {
            url,
            auth: RequestAuth::ApiKey(self.api_key.clone()),
            headers: vec![(
                "anthropic-version".to_owned(),
                ANTHROPIC_VERSION_HEADER.to_owned(),
            )],
            body,
        };
        let metadata = self.metadata();

        let (response, attempts) = execute_with_retry(&metadata, || async {
            send_json_request::<AnthropicResponse>(
                &self.client,
                &request,
                &metadata.provider_type,
                &self.profile_name,
                &self.model,
            )
            .await
        })
        .await?;

        Ok(ProviderCompletion {
            response: parse_anthropic_response(
                response,
                &metadata.provider_type,
                &self.profile_name,
                &self.model,
            )?,
            attempts,
        })
    }
}
