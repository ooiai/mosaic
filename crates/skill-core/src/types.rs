use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use mosaic_tool_core::ToolRegistry;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SkillOutput {
    pub content: String,
    pub structured: Option<serde_json::Value>,
}

pub struct SkillContext {
    pub tools: Arc<ToolRegistry>,
}

#[async_trait]
pub trait Skill: Send + Sync {
    fn name(&self) -> &str;

    async fn execute(&self, input: serde_json::Value, ctx: &SkillContext) -> Result<SkillOutput>;
}
