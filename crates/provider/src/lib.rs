use anyhow::Result;
use async_trait::async_trait;
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
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: serde_json::Value,
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
    async fn complete(&self, messages: &[Message]) -> Result<CompletionResponse>;
}

pub struct MockProvider;

#[async_trait]
impl LlmProvider for MockProvider {
    async fn complete(&self, messages: &[Message]) -> Result<CompletionResponse> {
        let last = messages
            .last()
            .map(|message| message.content.clone())
            .unwrap_or_default();

        Ok(CompletionResponse {
            message: Some(Message {
                role: Role::Assistant,
                content: format!("mock response: {last}"),
            }),
            tool_calls: vec![],
            finish_reason: Some("stop".to_owned()),
        })
    }
}

#[cfg(test)]
mod tests {
    use futures::executor::block_on;

    use super::{LlmProvider, Message, MockProvider, Role};

    #[test]
    fn mock_provider_replies_to_the_last_message() {
        let response = block_on(MockProvider.complete(&[
            Message {
                role: Role::System,
                content: "system".to_owned(),
            },
            Message {
                role: Role::User,
                content: "hello".to_owned(),
            },
        ]))
        .expect("mock provider should succeed");

        assert_eq!(
            response.message.expect("message should exist").content,
            "mock response: hello"
        );
        assert_eq!(response.finish_reason.as_deref(), Some("stop"));
        assert!(response.tool_calls.is_empty());
    }
}
