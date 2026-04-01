use anyhow::Result;
use async_trait::async_trait;

use crate::{Tool, ToolContext, ToolMetadata, ToolResult};

pub struct EchoTool {
    meta: ToolMetadata,
}

impl EchoTool {
    pub fn new() -> Self {
        Self {
            meta: ToolMetadata::builtin(
                "echo",
                "Echo input as output",
                serde_json::json!({
                    "type": "object",
                    "properties": {
                        "text": { "type": "string" }
                    },
                    "required": ["text"]
                }),
            ),
        }
    }
}

impl Default for EchoTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for EchoTool {
    fn metadata(&self) -> &ToolMetadata {
        &self.meta
    }

    async fn call(&self, input: serde_json::Value, _ctx: &ToolContext) -> Result<ToolResult> {
        let text = input
            .get("text")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_owned();

        Ok(ToolResult {
            content: text,
            structured: Some(input),
            is_error: false,
            audit: None,
        })
    }
}
