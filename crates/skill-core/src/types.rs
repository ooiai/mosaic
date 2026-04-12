use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use mosaic_sandbox_core::{SandboxKind, SandboxScope};
use mosaic_tool_core::ToolRegistry;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SkillOutput {
    pub content: String,
    pub structured: Option<serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillSandboxContext {
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

pub struct SkillContext {
    pub tools: Arc<ToolRegistry>,
    pub sandbox: Option<SkillSandboxContext>,
}

#[async_trait]
pub trait Skill: Send + Sync {
    fn name(&self) -> &str;

    async fn execute(&self, input: serde_json::Value, ctx: &SkillContext) -> Result<SkillOutput>;
}
