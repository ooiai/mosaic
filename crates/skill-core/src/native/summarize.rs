use anyhow::Result;
use async_trait::async_trait;

use crate::manifest::template::input_text;
use crate::{Skill, SkillContext, SkillOutput};

pub struct SummarizeSkill;

#[async_trait]
impl Skill for SummarizeSkill {
    fn name(&self) -> &str {
        "summarize"
    }

    async fn execute(&self, input: serde_json::Value, _ctx: &SkillContext) -> Result<SkillOutput> {
        let text = input_text(&input);
        Ok(SkillOutput {
            content: format!("summary: {text}"),
            structured: Some(input),
        })
    }
}
