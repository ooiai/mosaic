use std::time::Instant;

use reqwest::{Client, StatusCode};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use mosaic_core::config::ProfileConfig;
use mosaic_core::error::{MosaicError, Result};
use mosaic_core::provider::{
    ChatMessage, ChatRequest, ChatResponse, ChatRole, ModelInfo, Provider, ProviderHealth,
};

#[derive(Debug, Clone)]
pub struct OpenAiCompatibleProvider {
    client: Option<Client>,
    base_url: String,
    api_key: String,
    api_key_env: String,
    mock_mode: bool,
}

impl OpenAiCompatibleProvider {
    /// Creates a provider from a profile, deferring auth errors to request time.
    /// This allows the TUI to launch even before an API key is configured.
    pub fn from_profile(profile: &ProfileConfig) -> Result<Self> {
        let base_url = profile.provider.base_url.clone();
        let api_key_env = profile.provider.api_key_env.clone();
        let api_key = if base_url.starts_with("mock://") {
            "mock-key".to_string()
        } else {
            std::env::var(&api_key_env).unwrap_or_default()
        };
        Self::new_with_env(base_url, api_key, api_key_env)
    }

    pub fn new(base_url: String, api_key: String) -> Result<Self> {
        Self::new_with_env(base_url, api_key, String::new())
    }

    fn new_with_env(base_url: String, api_key: String, api_key_env: String) -> Result<Self> {
        let normalized = base_url.trim_end_matches('/').to_string();
        let mock_mode = normalized.starts_with("mock://");
        let client = if mock_mode {
            None
        } else {
            Some(
                Client::builder()
                    .timeout(std::time::Duration::from_secs(60))
                    .build()
                    .map_err(|err| {
                        MosaicError::Network(format!("failed to initialize HTTP client: {err}"))
                    })?,
            )
        };
        Ok(Self {
            client,
            base_url: normalized,
            api_key,
            api_key_env,
            mock_mode,
        })
    }

    fn check_auth(&self) -> Result<()> {
        if !self.mock_mode && self.api_key.trim().is_empty() {
            let env = if self.api_key_env.is_empty() {
                "an API key environment variable".to_string()
            } else {
                format!("environment variable {}", self.api_key_env)
            };
            return Err(MosaicError::Auth(format!("{env} is required")));
        }
        Ok(())
    }

    fn endpoint(&self, path: &str) -> String {
        let normalized_path = path.trim_start_matches('/');
        if self.base_url.ends_with("/v1") && normalized_path.starts_with("v1/") {
            let without_v1 = normalized_path.trim_start_matches("v1/");
            return format!("{}/{}", self.base_url, without_v1);
        }
        format!("{}/{}", self.base_url, normalized_path)
    }

    async fn request_json<T: for<'de> Deserialize<'de>>(
        &self,
        request: reqwest::RequestBuilder,
    ) -> Result<T> {
        let response = request.send().await.map_err(|err| {
            if err.is_timeout() {
                MosaicError::Network("request timed out".to_string())
            } else {
                MosaicError::Network(err.to_string())
            }
        })?;
        let status = response.status();
        if !status.is_success() {
            let text = response
                .text()
                .await
                .unwrap_or_else(|_| "<failed to read error body>".to_string());
            return Err(match status {
                StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => {
                    MosaicError::Auth(format!("provider rejected API key: {text}"))
                }
                _ => MosaicError::Network(format!("provider request failed ({status}): {text}")),
            });
        }
        response.json::<T>().await.map_err(|err| {
            MosaicError::Network(format!("failed to parse provider response: {err}"))
        })
    }
}

#[async_trait::async_trait]
impl Provider for OpenAiCompatibleProvider {
    async fn list_models(&self) -> Result<Vec<ModelInfo>> {
        if self.mock_mode {
            let model = self
                .base_url
                .trim_start_matches("mock://")
                .trim()
                .to_string();
            let model = if model.is_empty() {
                "mock-model".to_string()
            } else {
                model
            };
            return Ok(vec![ModelInfo {
                id: model,
                owned_by: Some("mock".to_string()),
            }]);
        }
        self.check_auth()?;
        let endpoint = self.endpoint("/v1/models");
        let req = self
            .client
            .as_ref()
            .ok_or_else(|| MosaicError::Network("HTTP client is not initialized".to_string()))?
            .get(endpoint)
            .bearer_auth(&self.api_key);
        let payload: ModelsResponse = self.request_json(req).await?;
        let models = payload
            .data
            .into_iter()
            .map(|item| ModelInfo {
                id: item.id,
                owned_by: item.owned_by,
            })
            .collect();
        Ok(models)
    }

    async fn chat(&self, request: ChatRequest) -> Result<ChatResponse> {
        if self.mock_mode {
            maybe_capture_mock_chat_request(&request);
            let content = std::env::var("MOSAIC_MOCK_CHAT_RESPONSE")
                .unwrap_or_else(|_| "mock-answer".to_string());
            return Ok(ChatResponse { content });
        }
        self.check_auth()?;
        let endpoint = self.endpoint("/v1/chat/completions");
        let messages = request
            .messages
            .into_iter()
            .map(OpenAiMessage::from_chat_message)
            .collect::<Vec<_>>();
        let req = self
            .client
            .as_ref()
            .ok_or_else(|| MosaicError::Network("HTTP client is not initialized".to_string()))?
            .post(endpoint)
            .bearer_auth(&self.api_key)
            .json(&ChatCompletionRequest {
                model: request.model,
                temperature: request.temperature,
                messages,
            });
        let payload: ChatCompletionResponse = self.request_json(req).await?;
        let choice =
            payload.choices.into_iter().next().ok_or_else(|| {
                MosaicError::Network("provider returned no chat choices".to_string())
            })?;
        Ok(ChatResponse {
            content: content_to_text(choice.message.content),
        })
    }

    async fn health(&self) -> Result<ProviderHealth> {
        let started = Instant::now();
        let ok = self.list_models().await.is_ok();
        let latency_ms = Some(started.elapsed().as_millis());
        let detail = if ok {
            "provider reachable".to_string()
        } else {
            "provider request failed".to_string()
        };
        Ok(ProviderHealth {
            ok,
            latency_ms,
            detail,
        })
    }
}

fn maybe_capture_mock_chat_request(request: &ChatRequest) {
    let Ok(path) = std::env::var("MOSAIC_MOCK_CHAT_CAPTURE_PATH") else {
        return;
    };
    let messages = request
        .messages
        .iter()
        .map(|message| {
            json!({
                "role": match message.role {
                    ChatRole::System => "system",
                    ChatRole::User => "user",
                    ChatRole::Assistant => "assistant",
                },
                "content": message.content,
            })
        })
        .collect::<Vec<_>>();
    let payload = json!({
        "model": request.model,
        "temperature": request.temperature,
        "messages": messages,
    });
    if let Ok(raw) = serde_json::to_vec_pretty(&payload) {
        let _ = std::fs::write(path, raw);
    }
}

#[derive(Debug, Deserialize)]
struct ModelsResponse {
    data: Vec<ModelItem>,
}

#[derive(Debug, Deserialize)]
struct ModelItem {
    id: String,
    owned_by: Option<String>,
}

#[derive(Debug, Serialize)]
struct ChatCompletionRequest {
    model: String,
    temperature: f32,
    messages: Vec<OpenAiMessage>,
}

#[derive(Debug, Serialize, Deserialize)]
struct OpenAiMessage {
    role: String,
    content: Value,
}

impl OpenAiMessage {
    fn from_chat_message(message: ChatMessage) -> Self {
        let role = match message.role {
            ChatRole::System => "system",
            ChatRole::User => "user",
            ChatRole::Assistant => "assistant",
        }
        .to_string();
        Self {
            role,
            content: Value::String(message.content),
        }
    }
}

#[derive(Debug, Deserialize)]
struct ChatCompletionResponse {
    choices: Vec<Choice>,
}

#[derive(Debug, Deserialize)]
struct Choice {
    message: OpenAiMessage,
}

fn content_to_text(content: Value) -> String {
    match content {
        Value::String(text) => text,
        Value::Array(parts) => parts
            .into_iter()
            .filter_map(|part| {
                part.get("text")
                    .and_then(|v| v.as_str())
                    .map(ToOwned::to_owned)
            })
            .collect::<Vec<_>>()
            .join("\n"),
        other => other.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn content_to_text_handles_array_parts() {
        let value = serde_json::json!([
            { "type": "text", "text": "a" },
            { "type": "text", "text": "b" }
        ]);
        assert_eq!(content_to_text(value), "a\nb");
    }

    #[test]
    fn endpoint_avoids_duplicate_v1_when_base_url_already_has_v1() {
        let provider = OpenAiCompatibleProvider {
            client: None,
            base_url: "https://example.openai.azure.com/openai/v1".to_string(),
            api_key: "mock".to_string(),
            api_key_env: String::new(),
            mock_mode: false,
        };
        assert_eq!(
            provider.endpoint("/v1/models"),
            "https://example.openai.azure.com/openai/v1/models"
        );
        assert_eq!(
            provider.endpoint("/v1/chat/completions"),
            "https://example.openai.azure.com/openai/v1/chat/completions"
        );
    }

    #[test]
    fn endpoint_keeps_v1_when_base_url_without_v1() {
        let provider = OpenAiCompatibleProvider {
            client: None,
            base_url: "https://api.openai.com".to_string(),
            api_key: "mock".to_string(),
            api_key_env: String::new(),
            mock_mode: false,
        };
        assert_eq!(
            provider.endpoint("/v1/models"),
            "https://api.openai.com/v1/models"
        );
    }
}
