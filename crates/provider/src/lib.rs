use anyhow::{Result, anyhow};
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};

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

#[async_trait]
pub trait LlmProvider: Send + Sync {
    async fn complete(
        &self,
        messages: &[Message],
        tools: Option<&[ToolDefinition]>,
    ) -> Result<CompletionResponse>;
}

pub struct MockProvider;

#[async_trait]
impl LlmProvider for MockProvider {
    async fn complete(
        &self,
        messages: &[Message],
        tools: Option<&[ToolDefinition]>,
    ) -> Result<CompletionResponse> {
        let last = messages.last().ok_or_else(|| anyhow!("no messages"))?;

        match last.role {
            Role::User => {
                let content = last.content.to_lowercase();

                if content.contains("time") && tool_is_available(tools, "time_now") {
                    return Ok(CompletionResponse {
                        message: None,
                        tool_calls: vec![ToolCall {
                            id: "call_mock_time_now".to_owned(),
                            name: "time_now".to_owned(),
                            arguments: serde_json::json!({}),
                        }],
                        finish_reason: Some("tool_calls".to_owned()),
                    });
                }

                if (content.contains("read") || content.contains("file"))
                    && tool_is_available(tools, "read_file")
                {
                    return Ok(CompletionResponse {
                        message: None,
                        tool_calls: vec![ToolCall {
                            id: "call_mock_read_file".to_owned(),
                            name: "read_file".to_owned(),
                            arguments: serde_json::json!({
                                "path": "README.md"
                            }),
                        }],
                        finish_reason: Some("tool_calls".to_owned()),
                    });
                }

                Ok(CompletionResponse {
                    message: Some(Message {
                        role: Role::Assistant,
                        content: format!("mock response: {}", last.content),
                        tool_call_id: None,
                    }),
                    tool_calls: vec![],
                    finish_reason: Some("stop".to_owned()),
                })
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

                Ok(CompletionResponse {
                    message: Some(Message {
                        role: Role::Assistant,
                        content: reply,
                        tool_call_id: None,
                    }),
                    tool_calls: vec![],
                    finish_reason: Some("stop".to_owned()),
                })
            }
            _ => Ok(CompletionResponse {
                message: Some(Message {
                    role: Role::Assistant,
                    content: "mock response".to_owned(),
                    tool_call_id: None,
                }),
                tool_calls: vec![],
                finish_reason: Some("stop".to_owned()),
            }),
        }
    }
}

pub struct OpenAiCompatibleProvider {
    client: Client,
    base_url: String,
    api_key: String,
    model: String,
}

impl OpenAiCompatibleProvider {
    pub fn new(base_url: String, api_key: String, model: String) -> Self {
        Self {
            client: Client::new(),
            base_url,
            api_key,
            model,
        }
    }
}

#[async_trait]
impl LlmProvider for OpenAiCompatibleProvider {
    async fn complete(
        &self,
        messages: &[Message],
        tools: Option<&[ToolDefinition]>,
    ) -> Result<CompletionResponse> {
        let url = format!("{}/chat/completions", self.base_url.trim_end_matches('/'));

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

        let body = if let Some(req_tools) = req_tools.filter(|defs| !defs.is_empty()) {
            serde_json::json!({
                "model": self.model,
                "messages": req_messages,
                "tools": req_tools,
                "tool_choice": "auto",
            })
        } else {
            serde_json::json!({
                "model": self.model,
                "messages": req_messages,
            })
        };

        let response: ApiChatCompletionResponse = self
            .client
            .post(url)
            .bearer_auth(&self.api_key)
            .json(&body)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;

        let choice = response
            .choices
            .into_iter()
            .next()
            .ok_or_else(|| anyhow!("empty choices in provider response"))?;

        let tool_calls = choice
            .message
            .tool_calls
            .unwrap_or_default()
            .into_iter()
            .map(|tool_call| {
                let arguments =
                    serde_json::from_str::<serde_json::Value>(&tool_call.function.arguments)
                        .unwrap_or_else(|_| serde_json::json!({}));

                ToolCall {
                    id: tool_call.id,
                    name: tool_call.function.name,
                    arguments,
                }
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
}

fn role_to_api(role: &Role) -> &'static str {
    match role {
        Role::System => "system",
        Role::User => "user",
        Role::Assistant => "assistant",
        Role::Tool => "tool",
    }
}

fn tool_is_available(tools: Option<&[ToolDefinition]>, name: &str) -> bool {
    tools
        .map(|defs| defs.iter().any(|tool| tool.name == name))
        .unwrap_or(false)
}

fn infer_tool_name_from_call_id(call_id: Option<&str>) -> Option<&'static str> {
    match call_id {
        Some(id) if id.contains("time_now") => Some("time_now"),
        Some(id) if id.contains("read_file") => Some("read_file"),
        _ => None,
    }
}

fn preview_text(value: &str, limit: usize) -> String {
    if value.chars().count() <= limit {
        return value.to_string();
    }

    let truncated: String = value.chars().take(limit).collect();
    format!("{truncated}...")
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

#[cfg(test)]
mod tests {
    use futures::executor::block_on;

    use super::{LlmProvider, Message, MockProvider, Role, ToolDefinition};

    fn time_tool_definition() -> ToolDefinition {
        ToolDefinition {
            name: "time_now".to_owned(),
            description: "Return the current UTC timestamp".to_owned(),
            input_schema: serde_json::json!({ "type": "object", "properties": {} }),
        }
    }

    fn read_file_tool_definition() -> ToolDefinition {
        ToolDefinition {
            name: "read_file".to_owned(),
            description: "Read a UTF-8 text file from disk".to_owned(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": { "path": { "type": "string" } },
                "required": ["path"],
            }),
        }
    }

    #[test]
    fn mock_provider_replies_to_the_last_message_when_no_tool_is_needed() {
        let response = block_on(MockProvider.complete(
            &[
                Message {
                    role: Role::System,
                    content: "system".to_owned(),
                    tool_call_id: None,
                },
                Message {
                    role: Role::User,
                    content: "hello".to_owned(),
                    tool_call_id: None,
                },
            ],
            None,
        ))
        .expect("mock provider should succeed");

        assert_eq!(
            response.message.expect("message should exist").content,
            "mock response: hello"
        );
        assert_eq!(response.finish_reason.as_deref(), Some("stop"));
        assert!(response.tool_calls.is_empty());
    }

    #[test]
    fn mock_provider_emits_time_now_tool_call_when_tool_is_available() {
        let tools = vec![time_tool_definition()];

        let response = block_on(MockProvider.complete(
            &[Message {
                role: Role::User,
                content: "What time is it now?".to_owned(),
                tool_call_id: None,
            }],
            Some(&tools),
        ))
        .expect("mock provider should succeed");

        assert!(response.message.is_none());
        assert_eq!(response.finish_reason.as_deref(), Some("tool_calls"));
        assert_eq!(response.tool_calls.len(), 1);
        assert_eq!(response.tool_calls[0].name, "time_now");
    }

    #[test]
    fn mock_provider_emits_read_file_tool_call_when_tool_is_available() {
        let tools = vec![read_file_tool_definition()];

        let response = block_on(MockProvider.complete(
            &[Message {
                role: Role::User,
                content: "Read a file for me.".to_owned(),
                tool_call_id: None,
            }],
            Some(&tools),
        ))
        .expect("mock provider should succeed");

        assert!(response.message.is_none());
        assert_eq!(response.tool_calls.len(), 1);
        assert_eq!(response.tool_calls[0].name, "read_file");
        assert_eq!(
            response.tool_calls[0].arguments,
            serde_json::json!({ "path": "README.md" })
        );
    }

    #[test]
    fn mock_provider_finalizes_after_tool_output() {
        let response = block_on(MockProvider.complete(
            &[Message {
                role: Role::Tool,
                content: "2026-03-20T12:00:00Z".to_owned(),
                tool_call_id: Some("call_mock_time_now".to_owned()),
            }],
            None,
        ))
        .expect("mock provider should succeed");

        assert_eq!(
            response.message.expect("message should exist").content,
            "The current time is: 2026-03-20T12:00:00Z"
        );
    }

    #[test]
    fn mock_provider_uses_file_preview_after_read_file_tool_output() {
        let response = block_on(MockProvider.complete(
            &[Message {
                role: Role::Tool,
                content: "abcdefghijklmnopqrstuvwxyz".repeat(12),
                tool_call_id: Some("call_mock_read_file".to_owned()),
            }],
            Some(&[read_file_tool_definition()]),
        ))
        .expect("mock provider should succeed");

        let message = response.message.expect("message should exist");
        assert!(
            message
                .content
                .starts_with("I read the file successfully. Preview:\n")
        );
        assert!(message.content.ends_with("..."));
    }

    #[test]
    fn mock_provider_falls_back_when_requested_tool_is_unavailable() {
        let response = block_on(MockProvider.complete(
            &[Message {
                role: Role::User,
                content: "What time is it?".to_owned(),
                tool_call_id: None,
            }],
            Some(&[read_file_tool_definition()]),
        ))
        .expect("mock provider should succeed");

        assert!(response.tool_calls.is_empty());
        assert_eq!(
            response.message.expect("message should exist").content,
            "mock response: What time is it?"
        );
    }
}
