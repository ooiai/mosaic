use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use mosaic_tool_core::{Tool, ToolMetadata, ToolResult};

#[async_trait]
pub trait McpTransport: Send + Sync {
    async fn request(&self, method: &str, params: serde_json::Value) -> Result<serde_json::Value>;
}

pub struct McpClient {
    transport: Arc<dyn McpTransport>,
}

impl McpClient {
    pub fn new(transport: Arc<dyn McpTransport>) -> Self {
        Self { transport }
    }

    pub async fn list_tools(&self) -> Result<Vec<serde_json::Value>> {
        let value = self
            .transport
            .request("tools/list", serde_json::json!({}))
            .await?;

        Ok(value.as_array().cloned().unwrap_or_default())
    }

    pub async fn call_tool(
        &self,
        name: &str,
        args: serde_json::Value,
    ) -> Result<serde_json::Value> {
        self.transport
            .request(
                "tools/call",
                serde_json::json!({
                    "name": name,
                    "arguments": args,
                }),
            )
            .await
    }
}

pub struct McpToolAdapter {
    pub metadata: ToolMetadata,
    pub client: Arc<McpClient>,
    pub remote_name: String,
}

#[async_trait]
impl Tool for McpToolAdapter {
    fn metadata(&self) -> &ToolMetadata {
        &self.metadata
    }

    async fn call(&self, input: serde_json::Value) -> Result<ToolResult> {
        let value = self.client.call_tool(&self.remote_name, input).await?;
        Ok(ToolResult {
            content: value.to_string(),
            structured: Some(value),
        })
    }
}
