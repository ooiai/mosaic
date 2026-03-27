use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::errors::ProviderError;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum Role {
    System,
    User,
    Assistant,
    Tool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Message {
    pub role: Role,
    pub content: String,
    pub tool_call_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CompletionResponse {
    pub message: Option<Message>,
    #[serde(default)]
    pub tool_calls: Vec<ToolCall>,
    pub finish_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProviderTransportMetadata {
    pub provider_type: String,
    pub base_url: Option<String>,
    pub timeout_ms: u64,
    pub max_retries: u8,
    pub retry_backoff_ms: u64,
    #[serde(default)]
    pub api_version: Option<String>,
    #[serde(default)]
    pub version_header: Option<String>,
    #[serde(default)]
    pub custom_header_keys: Vec<String>,
    pub supports_tool_call_shadow_messages: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProviderAttempt {
    pub attempt: u8,
    pub max_attempts: u8,
    pub status: String,
    pub error_kind: Option<String>,
    pub status_code: Option<u16>,
    pub retryable: bool,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProviderCompletion {
    pub response: CompletionResponse,
    #[serde(default)]
    pub attempts: Vec<ProviderAttempt>,
}

#[async_trait]
pub trait LlmProvider: Send + Sync {
    fn metadata(&self) -> ProviderTransportMetadata;

    fn tool_call_shadow_message(&self, _tool_calls: &[ToolCall]) -> Option<Message> {
        None
    }

    async fn complete(
        &self,
        messages: &[Message],
        tools: Option<&[ToolDefinition]>,
    ) -> std::result::Result<ProviderCompletion, ProviderError>;
}
