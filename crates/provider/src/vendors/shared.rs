use std::{env, future::Future, time::Duration};

use anyhow::{Result, anyhow, bail};
use mosaic_config::ProviderType;
use reqwest::Client;
use serde::{Deserialize, de::DeserializeOwned};
use tokio::time::sleep;
use tracing::warn;

use crate::{
    errors::{ProviderError, ProviderErrorKind},
    profile::ProviderProfile,
    types::{
        CompletionResponse, Message, ProviderAttempt, ProviderCompletion,
        ProviderTransportMetadata, Role, ToolCall, ToolDefinition,
    },
};

pub(crate) const DEFAULT_MAX_RETRIES: u8 = 2;
pub(crate) const DEFAULT_TIMEOUT_MS: u64 = 45_000;
pub(crate) const ANTHROPIC_TIMEOUT_MS: u64 = 60_000;
pub(crate) const OLLAMA_TIMEOUT_MS: u64 = 90_000;
pub(crate) const TOOL_CALL_SHADOW_PREFIX: &str = "__mosaic_tool_calls__:";
pub(crate) const AZURE_CHAT_COMPLETIONS_API_VERSION: &str = "2024-10-21";
pub(crate) const ANTHROPIC_VERSION_HEADER: &str = "2023-06-01";

#[derive(Clone)]
pub(crate) struct OpenAiStyleProvider {
    pub(crate) client: Client,
    pub(crate) provider_type: String,
    pub(crate) profile_name: String,
    pub(crate) model: String,
    pub(crate) base_url: String,
    pub(crate) auth: RequestAuth,
    pub(crate) metadata: ProviderTransportMetadata,
    pub(crate) endpoint: OpenAiStyleEndpoint,
    pub(crate) api_version: Option<String>,
    pub(crate) request_headers: Vec<(String, String)>,
}

#[derive(Clone, Copy)]
pub(crate) enum OpenAiStyleEndpoint {
    Standard,
    Azure,
    Ollama,
}

#[derive(Clone)]
pub(crate) enum RequestAuth {
    None,
    Bearer(String),
    ApiKey(String),
}

impl OpenAiStyleProvider {
    pub(crate) fn metadata(&self) -> ProviderTransportMetadata {
        self.metadata.clone()
    }

    pub(crate) async fn complete(
        &self,
        messages: &[Message],
        tools: Option<&[ToolDefinition]>,
    ) -> std::result::Result<ProviderCompletion, ProviderError> {
        let url = match self.endpoint {
            OpenAiStyleEndpoint::Standard => openai_chat_completions_url(&self.base_url),
            OpenAiStyleEndpoint::Azure => azure_chat_completions_url(
                &self.base_url,
                &self.model,
                self.api_version
                    .as_deref()
                    .unwrap_or(AZURE_CHAT_COMPLETIONS_API_VERSION),
            ),
            OpenAiStyleEndpoint::Ollama => ollama_chat_completions_url(&self.base_url),
        };
        let body = build_openai_style_body(
            &self.model,
            messages,
            tools,
            !matches!(self.endpoint, OpenAiStyleEndpoint::Azure),
        );
        let request = JsonRequest {
            url,
            auth: self.auth.clone(),
            headers: self.request_headers.clone(),
            body,
        };
        let metadata = self.metadata();

        let (response, attempts) = execute_with_retry(&metadata, || async {
            send_json_request::<ApiChatCompletionResponse>(
                &self.client,
                &request,
                &self.provider_type,
                &self.profile_name,
                &self.model,
            )
            .await
        })
        .await?;

        Ok(ProviderCompletion {
            response: parse_openai_style_response(
                response,
                &self.provider_type,
                &self.profile_name,
                &self.model,
            )?,
            attempts,
        })
    }
}

pub(crate) fn build_http_client(timeout_ms: u64) -> Client {
    Client::builder()
        .timeout(Duration::from_millis(timeout_ms))
        .build()
        .expect("provider HTTP client should build")
}

pub(crate) fn resolve_api_key(profile: &ProviderProfile, required: bool) -> Result<Option<String>> {
    match profile.api_key_env.as_deref() {
        Some(env_var) => env::var(env_var).map(Some).map_err(|_| {
            anyhow!(
                "profile '{}' expects environment variable {} to be set",
                profile.name,
                env_var
            )
        }),
        None if required => bail!("profile '{}' is missing api_key_env", profile.name),
        None => Ok(None),
    }
}

pub(crate) fn resolve_base_url(
    profile: &ProviderProfile,
    provider_type: ProviderType,
) -> Result<String> {
    profile
        .base_url
        .clone()
        .or_else(|| provider_type.default_base_url().map(str::to_owned))
        .ok_or_else(|| anyhow!("profile '{}' is missing base_url", profile.name))
}

pub(crate) struct JsonRequest {
    pub(crate) url: String,
    pub(crate) auth: RequestAuth,
    pub(crate) headers: Vec<(String, String)>,
    pub(crate) body: serde_json::Value,
}

pub(crate) async fn send_json_request<T: DeserializeOwned>(
    client: &Client,
    request: &JsonRequest,
    provider_type: &str,
    profile_name: &str,
    model: &str,
) -> std::result::Result<T, ProviderError> {
    let mut builder = client.post(request.url.clone());

    builder = match &request.auth {
        RequestAuth::None => builder,
        RequestAuth::Bearer(token) => builder.bearer_auth(token),
        RequestAuth::ApiKey(token) => builder.header("api-key", token),
    };

    for (key, value) in &request.headers {
        builder = builder.header(key, value);
    }

    let response = builder
        .json(&request.body)
        .send()
        .await
        .map_err(|error| translate_reqwest_error(provider_type, profile_name, model, error))?;

    let status = response.status();
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        return Err(translate_status_error(
            provider_type,
            profile_name,
            model,
            status.as_u16(),
            &body,
        ));
    }

    response
        .json::<T>()
        .await
        .map_err(|error| translate_decode_error(provider_type, profile_name, model, error))
}

pub(crate) async fn execute_with_retry<T, F, Fut>(
    metadata: &ProviderTransportMetadata,
    mut operation: F,
) -> std::result::Result<(T, Vec<ProviderAttempt>), ProviderError>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = std::result::Result<T, ProviderError>>,
{
    let max_attempts = metadata.max_retries.saturating_add(1);
    let mut attempts = Vec::new();

    for attempt in 1..=max_attempts {
        match operation().await {
            Ok(value) => {
                attempts.push(ProviderAttempt {
                    attempt,
                    max_attempts,
                    status: "success".to_owned(),
                    error_kind: None,
                    status_code: None,
                    retryable: false,
                    message: None,
                });
                return Ok((value, attempts));
            }
            Err(error) => {
                let retryable = error.retryable && attempt < max_attempts;
                attempts.push(ProviderAttempt {
                    attempt,
                    max_attempts,
                    status: if retryable { "retry" } else { "failed" }.to_owned(),
                    error_kind: Some(error.kind_label().to_owned()),
                    status_code: error.status_code,
                    retryable: error.retryable,
                    message: Some(error.public_message().to_owned()),
                });
                if retryable {
                    warn!(
                        provider_type = metadata.provider_type,
                        attempt,
                        max_attempts,
                        error_kind = error.kind_label(),
                        retryable = error.retryable,
                        "retrying provider request"
                    );
                    sleep(Duration::from_millis(
                        metadata.retry_backoff_ms.saturating_mul(attempt as u64),
                    ))
                    .await;
                    continue;
                }

                return Err(error.with_attempts(attempts));
            }
        }
    }

    unreachable!("provider retry loop should always return")
}

fn translate_reqwest_error(
    provider_type: &str,
    profile_name: &str,
    model: &str,
    error: reqwest::Error,
) -> ProviderError {
    let kind = if error.is_timeout() {
        ProviderErrorKind::Timeout
    } else {
        ProviderErrorKind::Transport
    };

    ProviderError::new(
        kind,
        provider_type,
        profile_name,
        model,
        error.to_string(),
        error.status().map(|status| status.as_u16()),
        matches!(
            kind,
            ProviderErrorKind::Timeout | ProviderErrorKind::Transport
        ),
    )
}

fn translate_status_error(
    provider_type: &str,
    profile_name: &str,
    model: &str,
    status_code: u16,
    body: &str,
) -> ProviderError {
    let (kind, retryable) = match status_code {
        401 | 403 => (ProviderErrorKind::Auth, false),
        408 | 504 => (ProviderErrorKind::Timeout, true),
        429 => (ProviderErrorKind::RateLimited, true),
        400..=499 => (ProviderErrorKind::InvalidRequest, false),
        500..=599 => (ProviderErrorKind::Unavailable, true),
        _ => (ProviderErrorKind::Unknown, false),
    };

    ProviderError::new(
        kind,
        provider_type,
        profile_name,
        model,
        if body.trim().is_empty() {
            format!("upstream returned status {status_code}")
        } else {
            format!("status {status_code}: {}", truncate_preview(body, 240))
        },
        Some(status_code),
        retryable,
    )
}

fn translate_decode_error(
    provider_type: &str,
    profile_name: &str,
    model: &str,
    error: reqwest::Error,
) -> ProviderError {
    ProviderError::new(
        ProviderErrorKind::Response,
        provider_type,
        profile_name,
        model,
        error.to_string(),
        None,
        false,
    )
}

fn openai_chat_completions_url(base_url: &str) -> String {
    let trimmed = base_url.trim_end_matches('/');
    if trimmed.ends_with("/chat/completions") {
        trimmed.to_owned()
    } else {
        format!("{trimmed}/chat/completions")
    }
}

fn azure_chat_completions_url(base_url: &str, deployment: &str, api_version: &str) -> String {
    let trimmed = base_url.trim_end_matches('/');
    format!("{trimmed}/openai/deployments/{deployment}/chat/completions?api-version={api_version}")
}

fn ollama_chat_completions_url(base_url: &str) -> String {
    let trimmed = base_url.trim_end_matches('/');
    if trimmed.ends_with("/chat/completions") {
        trimmed.to_owned()
    } else if trimmed.ends_with("/v1") {
        format!("{trimmed}/chat/completions")
    } else {
        format!("{trimmed}/v1/chat/completions")
    }
}

fn build_openai_style_body(
    model: &str,
    messages: &[Message],
    tools: Option<&[ToolDefinition]>,
    include_model: bool,
) -> serde_json::Value {
    let req_messages: Vec<serde_json::Value> = messages
        .iter()
        .map(|message| match message.role {
            Role::Tool => serde_json::json!({
                "role": role_to_api(&message.role),
                "content": message.content,
                "tool_call_id": message.tool_call_id,
            }),
            _ => serde_json::json!({
                "role": role_to_api(&message.role),
                "content": message.content,
            }),
        })
        .collect();

    let req_tools = tools.map(|defs| {
        defs.iter()
            .map(|tool| {
                serde_json::json!({
                    "type": "function",
                    "function": {
                        "name": tool.name,
                        "description": tool.description,
                        "parameters": tool.input_schema,
                    }
                })
            })
            .collect::<Vec<_>>()
    });

    match (include_model, req_tools.filter(|defs| !defs.is_empty())) {
        (true, Some(req_tools)) => serde_json::json!({
            "model": model,
            "messages": req_messages,
            "tools": req_tools,
            "tool_choice": "auto",
        }),
        (true, None) => serde_json::json!({
            "model": model,
            "messages": req_messages,
        }),
        (false, Some(req_tools)) => serde_json::json!({
            "messages": req_messages,
            "tools": req_tools,
            "tool_choice": "auto",
        }),
        (false, None) => serde_json::json!({
            "messages": req_messages,
        }),
    }
}

pub(crate) fn build_anthropic_body(
    model: &str,
    messages: &[Message],
    tools: Option<&[ToolDefinition]>,
) -> serde_json::Value {
    let system = messages
        .iter()
        .filter(|message| matches!(message.role, Role::System))
        .map(|message| message.content.clone())
        .collect::<Vec<_>>()
        .join("\n\n");

    let mut body = serde_json::json!({
        "model": model,
        "max_tokens": 2048,
        "messages": anthropic_messages_from_conversation(messages),
    });

    if !system.is_empty() {
        body["system"] = serde_json::Value::String(system);
    }

    if let Some(req_tools) = tools.filter(|defs| !defs.is_empty()) {
        body["tools"] = serde_json::Value::Array(
            req_tools
                .iter()
                .map(|tool| {
                    serde_json::json!({
                        "name": tool.name,
                        "description": tool.description,
                        "input_schema": tool.input_schema,
                    })
                })
                .collect(),
        );
    }

    body
}

fn anthropic_messages_from_conversation(messages: &[Message]) -> Vec<serde_json::Value> {
    let mut request_messages = Vec::new();

    for message in messages {
        match message.role {
            Role::System => {}
            Role::User => push_anthropic_message(
                &mut request_messages,
                "user",
                serde_json::json!({
                    "type": "text",
                    "text": message.content,
                }),
            ),
            Role::Assistant => {
                if let Some(tool_calls) = parse_tool_call_shadow(&message.content) {
                    for tool_call in tool_calls {
                        push_anthropic_message(
                            &mut request_messages,
                            "assistant",
                            serde_json::json!({
                                "type": "tool_use",
                                "id": tool_call.id,
                                "name": tool_call.name,
                                "input": tool_call.arguments,
                            }),
                        );
                    }
                } else {
                    push_anthropic_message(
                        &mut request_messages,
                        "assistant",
                        serde_json::json!({
                            "type": "text",
                            "text": message.content,
                        }),
                    );
                }
            }
            Role::Tool => push_anthropic_message(
                &mut request_messages,
                "user",
                serde_json::json!({
                    "type": "tool_result",
                    "tool_use_id": message.tool_call_id.clone().unwrap_or_else(|| "unknown".to_owned()),
                    "content": message.content,
                }),
            ),
        }
    }

    request_messages
}

fn push_anthropic_message(
    messages: &mut Vec<serde_json::Value>,
    role: &str,
    block: serde_json::Value,
) {
    if let Some(last) = messages.last_mut() {
        if last.get("role").and_then(|value| value.as_str()) == Some(role) {
            last.get_mut("content")
                .and_then(|value| value.as_array_mut())
                .expect("anthropic message content should be an array")
                .push(block);
            return;
        }
    }

    messages.push(serde_json::json!({
        "role": role,
        "content": [block],
    }));
}

pub(crate) fn tool_call_shadow_content(tool_calls: &[ToolCall]) -> String {
    format!(
        "{TOOL_CALL_SHADOW_PREFIX}{}",
        serde_json::to_string(tool_calls).unwrap_or_else(|_| "[]".to_owned())
    )
}

fn parse_tool_call_shadow(content: &str) -> Option<Vec<ToolCall>> {
    let payload = content.strip_prefix(TOOL_CALL_SHADOW_PREFIX)?;
    serde_json::from_str(payload).ok()
}

fn parse_openai_style_response(
    response: ApiChatCompletionResponse,
    provider_type: &str,
    profile_name: &str,
    model: &str,
) -> std::result::Result<CompletionResponse, ProviderError> {
    let choice = response.choices.into_iter().next().ok_or_else(|| {
        ProviderError::new(
            ProviderErrorKind::Response,
            provider_type,
            profile_name,
            model,
            "empty choices in provider response",
            None,
            false,
        )
    })?;

    let tool_calls = choice
        .message
        .tool_calls
        .unwrap_or_default()
        .into_iter()
        .map(|tool_call| ToolCall {
            id: tool_call.id,
            name: tool_call.function.name,
            arguments: serde_json::from_str::<serde_json::Value>(&tool_call.function.arguments)
                .unwrap_or_else(|_| serde_json::json!({})),
        })
        .collect::<Vec<_>>();

    let message = if tool_calls.is_empty() {
        Some(Message {
            role: Role::Assistant,
            content: choice.message.content.unwrap_or_default(),
            tool_call_id: None,
        })
    } else {
        None
    };

    Ok(CompletionResponse {
        message,
        tool_calls,
        finish_reason: choice.finish_reason,
    })
}

pub(crate) fn parse_anthropic_response(
    response: AnthropicResponse,
    provider_type: &str,
    profile_name: &str,
    model: &str,
) -> std::result::Result<CompletionResponse, ProviderError> {
    let mut text_blocks = Vec::new();
    let mut tool_calls = Vec::new();

    for block in response.content {
        match block.kind.as_str() {
            "text" => {
                if let Some(text) = block.text {
                    text_blocks.push(text);
                }
            }
            "tool_use" => {
                tool_calls.push(ToolCall {
                    id: block.id.unwrap_or_else(|| "tool_use_missing_id".to_owned()),
                    name: block
                        .name
                        .unwrap_or_else(|| "tool_use_missing_name".to_owned()),
                    arguments: block.input.unwrap_or_else(|| serde_json::json!({})),
                });
            }
            _ => {}
        }
    }

    if text_blocks.is_empty() && tool_calls.is_empty() {
        return Err(ProviderError::new(
            ProviderErrorKind::Response,
            provider_type,
            profile_name,
            model,
            "anthropic response did not include text or tool_use content",
            None,
            false,
        ));
    }

    Ok(CompletionResponse {
        message: if tool_calls.is_empty() {
            Some(Message {
                role: Role::Assistant,
                content: text_blocks.join("\n"),
                tool_call_id: None,
            })
        } else {
            None
        },
        tool_calls,
        finish_reason: response.stop_reason,
    })
}

fn role_to_api(role: &Role) -> &'static str {
    match role {
        Role::System => "system",
        Role::User => "user",
        Role::Assistant => "assistant",
        Role::Tool => "tool",
    }
}

pub(crate) fn resolve_tool_name(
    tools: Option<&[ToolDefinition]>,
    canonical: &str,
) -> Option<String> {
    let defs = tools?;

    defs.iter()
        .find(|tool| tool.name == canonical)
        .map(|tool| tool.name.clone())
        .or_else(|| {
            defs.iter()
                .find(|tool| tool.name.ends_with(&format!(".{canonical}")))
                .map(|tool| tool.name.clone())
        })
}

pub(crate) fn infer_tool_name_from_call_id(call_id: Option<&str>) -> Option<&'static str> {
    match call_id {
        Some(id) if id.contains("time_now") => Some("time_now"),
        Some(id) if id.contains("read_file") => Some("read_file"),
        _ => None,
    }
}

pub(crate) fn preview_text(value: &str, limit: usize) -> String {
    if value.chars().count() <= limit {
        return value.to_string();
    }

    let truncated: String = value.chars().take(limit).collect();
    format!("{truncated}...")
}

fn truncate_preview(value: &str, limit: usize) -> String {
    preview_text(value, limit)
}

#[derive(Debug, Deserialize)]
struct ApiChatCompletionResponse {
    choices: Vec<ApiChoice>,
}

#[derive(Debug, Deserialize)]
struct ApiChoice {
    message: ApiMessage,
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ApiMessage {
    content: Option<String>,
    tool_calls: Option<Vec<ApiToolCall>>,
}

#[derive(Debug, Deserialize)]
struct ApiToolCall {
    id: String,
    function: ApiFunctionCall,
}

#[derive(Debug, Deserialize)]
struct ApiFunctionCall {
    name: String,
    arguments: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct AnthropicResponse {
    pub(crate) content: Vec<AnthropicContentBlock>,
    pub(crate) stop_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct AnthropicContentBlock {
    #[serde(rename = "type")]
    pub(crate) kind: String,
    pub(crate) text: Option<String>,
    pub(crate) id: Option<String>,
    pub(crate) name: Option<String>,
    pub(crate) input: Option<serde_json::Value>,
}
