use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::{CapabilityAudit, ToolMetadata};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolResult {
    pub content: String,
    pub structured: Option<serde_json::Value>,
    #[serde(default)]
    pub is_error: bool,
    pub audit: Option<CapabilityAudit>,
}

impl ToolResult {
    pub fn ok(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            structured: None,
            is_error: false,
            audit: None,
        }
    }
}

#[async_trait]
pub trait Tool: Send + Sync {
    fn metadata(&self) -> &ToolMetadata;
    async fn call(&self, input: serde_json::Value) -> Result<ToolResult>;
}
