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
    AnthropicResponse, JsonRequest, RequestAuth, build_anthropic_body, build_http_client,
    execute_with_retry, parse_anthropic_response, resolve_api_key, resolve_base_url,
    send_json_request, tool_call_shadow_content,
};

pub struct AnthropicProvider {
    client: reqwest::Client,
    profile_name: String,
    base_url: String,
    api_key: String,
    model: String,
    metadata: ProviderTransportMetadata,
    request_headers: Vec<(String, String)>,
}

impl AnthropicProvider {
    pub fn new(
        profile_name: String,
        base_url: String,
        api_key: String,
        model: String,
        timeout_ms: u64,
        max_retries: u8,
        retry_backoff_ms: u64,
        anthropic_version: String,
        custom_headers: std::collections::BTreeMap<String, String>,
    ) -> Self {
        let custom_header_keys = custom_headers.keys().cloned().collect::<Vec<_>>();
        let mut request_headers = custom_headers.into_iter().collect::<Vec<_>>();
        request_headers.push(("anthropic-version".to_owned(), anthropic_version.clone()));
        let metadata = ProviderTransportMetadata {
            provider_type: ProviderType::Anthropic.as_str().to_owned(),
            base_url: Some(base_url.clone()),
            timeout_ms,
            max_retries,
            retry_backoff_ms,
            api_version: None,
            version_header: Some(anthropic_version.clone()),
            custom_header_keys,
            supports_tool_call_shadow_messages: true,
        };
        Self {
            client: build_http_client(metadata.timeout_ms),
            profile_name,
            base_url,
            api_key,
            model,
            metadata,
            request_headers,
        }
    }

    pub(crate) fn from_profile(profile: &ProviderProfile) -> Result<Self> {
        Ok(Self::new(
            profile.name.clone(),
            resolve_base_url(profile, ProviderType::Anthropic)?,
            resolve_api_key(profile, true)?.expect("anthropic requires api key"),
            profile.model.clone(),
            profile.timeout_ms,
            profile.max_retries,
            profile.retry_backoff_ms,
            profile
                .anthropic_version
                .clone()
                .expect("anthropic profiles should carry version"),
            profile.custom_headers.clone(),
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
            headers: self.request_headers.clone(),
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
