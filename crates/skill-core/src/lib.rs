use std::{collections::HashMap, sync::Arc};

use anyhow::{Result, anyhow};
use async_trait::async_trait;
use mosaic_tool_core::ToolRegistry;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SkillManifest {
    pub name: String,
    pub description: String,
    #[serde(default = "default_input_schema")]
    pub input_schema: serde_json::Value,
    #[serde(default)]
    pub tools: Vec<String>,
    pub system_prompt: Option<String>,
    #[serde(default)]
    pub steps: Vec<ManifestSkillStep>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ManifestSkillStep {
    Echo {
        name: String,
        input: Option<String>,
    },
    Summarize {
        name: String,
        input: Option<String>,
    },
    Tool {
        name: String,
        tool: String,
        #[serde(default)]
        input: serde_json::Value,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SkillCapabilities {
    #[serde(default)]
    pub declared_tools: Vec<String>,
    pub manifest_backed: bool,
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

pub enum RegisteredSkill {
    Native(Arc<dyn Skill>),
    Manifest(ManifestSkill),
}

impl RegisteredSkill {
    pub fn capabilities(&self) -> SkillCapabilities {
        match self {
            Self::Native(_) => SkillCapabilities {
                declared_tools: Vec::new(),
                manifest_backed: false,
            },
            Self::Manifest(skill) => SkillCapabilities {
                declared_tools: skill.manifest.tools.clone(),
                manifest_backed: true,
            },
        }
    }

    pub async fn execute(
        &self,
        input: serde_json::Value,
        ctx: &SkillContext,
    ) -> Result<SkillOutput> {
        match self {
            Self::Native(skill) => skill.execute(input, ctx).await,
            Self::Manifest(skill) => skill.execute(input, ctx).await,
        }
    }
}

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

    async fn execute(&self, input: serde_json::Value, ctx: &SkillContext) -> Result<SkillOutput> {
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

#[derive(Default)]
pub struct SkillRegistry {
    skills: HashMap<String, RegisteredSkill>,
}

impl SkillRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, skill: Arc<dyn Skill>) {
        self.register_native(skill);
    }

    pub fn register_native(&mut self, skill: Arc<dyn Skill>) {
        self.skills
            .insert(skill.name().to_owned(), RegisteredSkill::Native(skill));
    }

    pub fn register_manifest(&mut self, manifest: SkillManifest) {
        self.skills.insert(
            manifest.name.clone(),
            RegisteredSkill::Manifest(ManifestSkill::new(manifest)),
        );
    }

    pub fn get(&self, name: &str) -> Option<&RegisteredSkill> {
        self.skills.get(name)
    }

    pub async fn execute(
        &self,
        name: &str,
        input: serde_json::Value,
        ctx: &SkillContext,
    ) -> Result<SkillOutput> {
        let skill = self
            .get(name)
            .ok_or_else(|| anyhow!("skill not found: {name}"))?;
        skill.execute(input, ctx).await
    }
}

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

impl ManifestSkillStep {
    pub fn name(&self) -> &str {
        match self {
            Self::Echo { name, .. } => name,
            Self::Summarize { name, .. } => name,
            Self::Tool { name, .. } => name,
        }
    }
}

fn render_template(
    template: &str,
    input: &str,
    current: &str,
    step_outputs: &HashMap<String, String>,
) -> String {
    let mut rendered = template
        .replace("{{input}}", input)
        .replace("{{current}}", current);

    for (step_name, output) in step_outputs {
        rendered = rendered.replace(&format!("{{{{steps.{step_name}.output}}}}"), output);
    }

    rendered
}

fn input_text(input: &serde_json::Value) -> String {
    input
        .get("text")
        .and_then(serde_json::Value::as_str)
        .map(str::to_owned)
        .unwrap_or_else(|| match input {
            serde_json::Value::String(text) => text.clone(),
            _ => input.to_string(),
        })
}

fn default_input_schema() -> serde_json::Value {
    serde_json::json!({ "type": "object" })
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use futures::executor::block_on;
    use mosaic_tool_core::ToolRegistry;

    use super::{ManifestSkillStep, SkillContext, SkillManifest, SkillRegistry, SummarizeSkill};

    #[test]
    fn summarize_skill_is_registered_and_returns_summary_text() {
        let mut registry = SkillRegistry::new();
        registry.register(Arc::new(SummarizeSkill));

        let output = block_on(registry.execute(
            "summarize",
            serde_json::json!({ "text": "Rust futures" }),
            &SkillContext {
                tools: Arc::new(ToolRegistry::new()),
            },
        ))
        .expect("skill should succeed");

        assert_eq!(output.content, "summary: Rust futures");
        assert!(registry.get("summarize").is_some());
    }

    #[test]
    fn manifest_skill_registers_and_executes_steps() {
        let mut registry = SkillRegistry::new();
        registry.register_manifest(SkillManifest {
            name: "brief".to_owned(),
            description: "Build a brief".to_owned(),
            input_schema: serde_json::json!({ "type": "object" }),
            tools: Vec::new(),
            system_prompt: None,
            steps: vec![
                ManifestSkillStep::Echo {
                    name: "draft".to_owned(),
                    input: Some("draft: {{input}}".to_owned()),
                },
                ManifestSkillStep::Summarize {
                    name: "summary".to_owned(),
                    input: Some("{{steps.draft.output}}".to_owned()),
                },
            ],
        });

        let output = block_on(registry.execute(
            "brief",
            serde_json::json!({ "text": "Rust async" }),
            &SkillContext {
                tools: Arc::new(ToolRegistry::new()),
            },
        ))
        .expect("manifest skill should succeed");

        assert_eq!(output.content, "summary: draft: Rust async");
        assert!(
            registry
                .get("brief")
                .expect("manifest skill should be registered")
                .capabilities()
                .manifest_backed
        );
    }
}
