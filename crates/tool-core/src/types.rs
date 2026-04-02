use anyhow::Result;
use async_trait::async_trait;
use mosaic_sandbox_core::{SandboxKind, SandboxScope};
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolSandboxContext {
    pub env_id: String,
    pub kind: SandboxKind,
    pub scope: SandboxScope,
    pub env_dir: std::path::PathBuf,
    pub workdir: std::path::PathBuf,
    pub dependency_spec: Vec<String>,
    pub prepared: bool,
    pub reused: bool,
    pub selection_reason: String,
    pub status: String,
}

#[derive(Debug, Clone, Default)]
pub struct ToolContext {
    pub sandbox: Option<ToolSandboxContext>,
}

#[async_trait]
pub trait Tool: Send + Sync {
    fn metadata(&self) -> &ToolMetadata;
    async fn call(&self, input: serde_json::Value, ctx: &ToolContext) -> Result<ToolResult>;
}
