use std::{collections::HashMap, sync::Arc};

use anyhow::Result;
use async_trait::async_trait;
use mosaic_tool_core::ToolRegistry;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SkillManifest {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
    pub tools: Vec<String>,
    pub system_prompt: Option<String>,
    #[serde(default)]
    pub steps: Vec<SkillStep>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum SkillStep {
    Think,
    CallTool { tool: String },
    Summarize,
}

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

#[derive(Default)]
pub struct SkillRegistry {
    skills: HashMap<String, Arc<dyn Skill>>,
}

impl SkillRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, skill: Arc<dyn Skill>) {
        self.skills.insert(skill.name().to_owned(), skill);
    }

    pub fn get(&self, name: &str) -> Option<Arc<dyn Skill>> {
        self.skills.get(name).cloned()
    }
}

pub struct SummarizeSkill;

#[async_trait]
impl Skill for SummarizeSkill {
    fn name(&self) -> &str {
        "summarize"
    }

    async fn execute(&self, input: serde_json::Value, _ctx: &SkillContext) -> Result<SkillOutput> {
        let text = input
            .get("text")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default();

        Ok(SkillOutput {
            content: format!("summary: {text}"),
            structured: Some(input),
        })
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use futures::executor::block_on;
    use mosaic_tool_core::ToolRegistry;

    use super::{SkillContext, SkillRegistry, SummarizeSkill};

    #[test]
    fn summarize_skill_is_registered_and_returns_summary_text() {
        let mut registry = SkillRegistry::new();
        registry.register(Arc::new(SummarizeSkill));

        let skill = registry
            .get("summarize")
            .expect("summarize skill should be present");
        let output = block_on(skill.execute(
            serde_json::json!({ "text": "Rust futures" }),
            &SkillContext {
                tools: Arc::new(ToolRegistry::new()),
            },
        ))
        .expect("skill should succeed");

        assert_eq!(output.content, "summary: Rust futures");
    }
}
