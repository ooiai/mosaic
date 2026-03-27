use std::collections::HashMap;

use anyhow::{Result, anyhow};

use super::{
    manifest::ManifestSkillStep,
    template::{input_text, render_template},
};
use crate::{SkillContext, SkillManifest, SkillOutput};

pub struct ManifestSkill {
    manifest: SkillManifest,
}

impl ManifestSkill {
    pub fn new(manifest: SkillManifest) -> Self {
        Self { manifest }
    }

    pub fn manifest(&self) -> &SkillManifest {
        &self.manifest
    }

    pub(crate) async fn execute(
        &self,
        input: serde_json::Value,
        ctx: &SkillContext,
    ) -> Result<SkillOutput> {
        let base_input = input_text(&input);
        let mut state = base_input.clone();
        let mut step_outputs = HashMap::new();

        for step in &self.manifest.steps {
            let output = match step {
                ManifestSkillStep::Echo { name: _, input } => {
                    let template = input.as_deref().unwrap_or("{{input}}");
                    render_template(template, &base_input, &state, &step_outputs)
                }
                ManifestSkillStep::Summarize { name: _, input } => {
                    let template = input.as_deref().unwrap_or("{{current}}");
                    let rendered = render_template(template, &base_input, &state, &step_outputs);
                    format!("summary: {rendered}")
                }
                ManifestSkillStep::Tool {
                    name: _,
                    tool,
                    input,
                } => {
                    let tool_impl = ctx
                        .tools
                        .get(tool)
                        .ok_or_else(|| anyhow!("manifest skill tool not found: {tool}"))?;
                    let result = tool_impl.call(input.clone()).await?;
                    result.content
                }
            };

            state = output.clone();
            step_outputs.insert(step.name().to_owned(), output);
        }

        Ok(SkillOutput {
            content: state,
            structured: Some(serde_json::json!({
                "input": base_input,
                "steps": step_outputs,
            })),
        })
    }
}
