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

fn default_compatibility_schema() -> u32 {
    1
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SkillCapabilities {
    #[serde(default)]
    pub declared_tools: Vec<String>,
    pub manifest_backed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SkillCompatibility {
    #[serde(default = "default_compatibility_schema")]
    pub schema_version: u32,
}

impl Default for SkillCompatibility {
    fn default() -> Self {
        Self {
            schema_version: default_compatibility_schema(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SkillMetadata {
    pub name: String,
    pub extension: Option<String>,
    pub version: Option<String>,
    #[serde(default)]
    pub declared_tools: Vec<String>,
    pub manifest_backed: bool,
    #[serde(default)]
    pub compatibility: SkillCompatibility,
}

impl SkillMetadata {
    pub fn native(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            extension: None,
            version: None,
            declared_tools: Vec::new(),
            manifest_backed: false,
            compatibility: SkillCompatibility::default(),
        }
    }

    pub fn manifest(manifest: &SkillManifest) -> Self {
        Self {
            name: manifest.name.clone(),
            extension: None,
            version: None,
            declared_tools: manifest.tools.clone(),
            manifest_backed: true,
            compatibility: SkillCompatibility::default(),
        }
    }

    pub fn with_extension(
        mut self,
        extension: impl Into<String>,
        version: impl Into<String>,
    ) -> Self {
        self.extension = Some(extension.into());
        self.version = Some(version.into());
        self
    }

    pub fn with_compatibility(mut self, compatibility: SkillCompatibility) -> Self {
        self.compatibility = compatibility;
        self
    }

    pub fn capabilities(&self) -> SkillCapabilities {
        SkillCapabilities {
            declared_tools: self.declared_tools.clone(),
            manifest_backed: self.manifest_backed,
        }
    }

    pub fn is_compatible_with_schema(&self, schema_version: u32) -> bool {
        self.compatibility.schema_version == schema_version
    }
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

enum RegisteredSkillImpl {
    Native(Arc<dyn Skill>),
    Manifest(ManifestSkill),
}

pub struct RegisteredSkill {
    implementation: RegisteredSkillImpl,
    metadata: SkillMetadata,
}

impl RegisteredSkill {
    pub fn metadata(&self) -> &SkillMetadata {
        &self.metadata
    }

    pub fn capabilities(&self) -> SkillCapabilities {
        self.metadata.capabilities()
    }

    pub async fn execute(
        &self,
        input: serde_json::Value,
        ctx: &SkillContext,
    ) -> Result<SkillOutput> {
        match &self.implementation {
            RegisteredSkillImpl::Native(skill) => skill.execute(input, ctx).await,
            RegisteredSkillImpl::Manifest(skill) => skill.execute(input, ctx).await,
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
        let metadata = SkillMetadata::native(skill.name().to_owned());
        self.register_native_with_metadata(skill, metadata);
    }

    pub fn register_native_with_metadata(
        &mut self,
        skill: Arc<dyn Skill>,
        metadata: SkillMetadata,
    ) {
        self.skills.insert(
            metadata.name.clone(),
            RegisteredSkill {
                implementation: RegisteredSkillImpl::Native(skill),
                metadata,
            },
        );
    }

    pub fn register_manifest(&mut self, manifest: SkillManifest) {
        let metadata = SkillMetadata::manifest(&manifest);
        self.register_manifest_with_metadata(manifest, metadata);
    }

    pub fn register_manifest_with_metadata(
        &mut self,
        manifest: SkillManifest,
        metadata: SkillMetadata,
    ) {
        self.skills.insert(
            metadata.name.clone(),
            RegisteredSkill {
                implementation: RegisteredSkillImpl::Manifest(ManifestSkill::new(manifest)),
                metadata,
            },
        );
    }

    pub fn get(&self, name: &str) -> Option<&RegisteredSkill> {
        self.skills.get(name)
    }

    pub fn unregister(&mut self, name: &str) -> Option<RegisteredSkill> {
        self.skills.remove(name)
    }

    pub fn list(&self) -> Vec<String> {
        self.skills.keys().cloned().collect()
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
