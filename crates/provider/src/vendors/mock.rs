use async_trait::async_trait;

use crate::{
    ProviderAttempt, ProviderCompletion, ProviderError, ProviderErrorKind,
    ProviderTransportMetadata,
    types::{CompletionResponse, LlmProvider, Message, Role, ToolCall, ToolDefinition},
};

use super::shared::{infer_tool_name_from_call_id, preview_text, resolve_tool_name};

pub struct MockProvider;

impl MockProvider {
    fn success_attempt() -> Vec<ProviderAttempt> {
        vec![ProviderAttempt {
            attempt: 1,
            max_attempts: 1,
            status: "success".to_owned(),
            error_kind: None,
            status_code: None,
            retryable: false,
            message: None,
        }]
    }
}

#[async_trait]
impl LlmProvider for MockProvider {
    fn metadata(&self) -> ProviderTransportMetadata {
        ProviderTransportMetadata {
            provider_type: "mock".to_owned(),
            base_url: None,
            timeout_ms: 0,
            max_retries: 0,
            retry_backoff_ms: 0,
            api_version: None,
            version_header: None,
            custom_header_keys: Vec::new(),
            supports_tool_call_shadow_messages: false,
        }
    }

    async fn complete(
        &self,
        messages: &[Message],
        tools: Option<&[ToolDefinition]>,
    ) -> std::result::Result<ProviderCompletion, ProviderError> {
        let last = messages.last().ok_or_else(|| {
            ProviderError::new(
                ProviderErrorKind::InvalidRequest,
                "mock",
                "mock",
                "mock",
                "no messages",
                None,
                false,
            )
        })?;

        let response = match last.role {
            Role::User => {
                let content = last.content.to_lowercase();
                let time_tool = resolve_tool_name(tools, "time_now");
                let read_file_tool = resolve_tool_name(tools, "read_file");

                if content.contains("time") {
                    if let Some(tool_name) = time_tool {
                        CompletionResponse {
                            message: None,
                            tool_calls: vec![ToolCall {
                                id: format!("call_mock_{}", tool_name.replace('.', "_")),
                                name: tool_name,
                                arguments: serde_json::json!({}),
                            }],
                            finish_reason: Some("tool_calls".to_owned()),
                        }
                    } else {
                        CompletionResponse {
                            message: Some(Message {
                                role: Role::Assistant,
                                content: format!("mock response: {}", last.content),
                                tool_call_id: None,
                            }),
                            tool_calls: vec![],
                            finish_reason: Some("stop".to_owned()),
                        }
                    }
                } else if content.contains("read") || content.contains("file") {
                    if let Some(tool_name) = read_file_tool {
                        CompletionResponse {
                            message: None,
                            tool_calls: vec![ToolCall {
                                id: format!("call_mock_{}", tool_name.replace('.', "_")),
                                name: tool_name,
                                arguments: serde_json::json!({ "path": "README.md" }),
                            }],
                            finish_reason: Some("tool_calls".to_owned()),
                        }
                    } else {
                        CompletionResponse {
                            message: Some(Message {
                                role: Role::Assistant,
                                content: format!("mock response: {}", last.content),
                                tool_call_id: None,
                            }),
                            tool_calls: vec![],
                            finish_reason: Some("stop".to_owned()),
                        }
                    }
                } else {
                    CompletionResponse {
                        message: Some(Message {
                            role: Role::Assistant,
                            content: format!("mock response: {}", last.content),
                            tool_call_id: None,
                        }),
                        tool_calls: vec![],
                        finish_reason: Some("stop".to_owned()),
                    }
                }
            }
            Role::Tool => {
                let reply = match infer_tool_name_from_call_id(last.tool_call_id.as_deref()) {
                    Some("time_now") => format!("The current time is: {}", last.content),
                    Some("read_file") => {
                        let preview = preview_text(&last.content, 220);
                        format!("I read the file successfully. Preview:\n{}", preview)
                    }
                    _ => format!("Tool returned:\n{}", last.content),
                };

                CompletionResponse {
                    message: Some(Message {
                        role: Role::Assistant,
                        content: reply,
                        tool_call_id: None,
                    }),
                    tool_calls: vec![],
                    finish_reason: Some("stop".to_owned()),
                }
            }
            _ => CompletionResponse {
                message: Some(Message {
                    role: Role::Assistant,
                    content: "mock response".to_owned(),
                    tool_call_id: None,
                }),
                tool_calls: vec![],
                finish_reason: Some("stop".to_owned()),
            },
        };

        Ok(ProviderCompletion {
            response,
            attempts: Self::success_attempt(),
        })
    }
}
