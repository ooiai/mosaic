use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Result, anyhow, bail};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::{
    Skill, SkillContext, SkillOutput,
    manifest::template::{input_text, render_template},
};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct MarkdownSkillFrontmatter {
    pub name: Option<String>,
    pub description: Option<String>,
    pub version: Option<String>,
    #[serde(default)]
    pub allowed_tools: Vec<String>,
    #[serde(default)]
    pub allowed_channels: Vec<String>,
    #[serde(default)]
    pub invocation_mode: Option<mosaic_tool_core::CapabilityInvocationMode>,
    #[serde(default)]
    pub accepts_attachments: bool,
    #[serde(default)]
    pub runtime_requirements: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct MarkdownSkillPack {
    name: String,
    source_path: PathBuf,
    body: String,
    frontmatter: MarkdownSkillFrontmatter,
    templates_dir: Option<PathBuf>,
    references_dir: Option<PathBuf>,
    scripts_dir: Option<PathBuf>,
}

impl MarkdownSkillPack {
    pub fn load_from_dir(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let source_dir = if path.is_file() {
            path.parent()
                .ok_or_else(|| anyhow!("markdown skill pack file has no parent directory"))?
                .to_path_buf()
        } else {
            path.to_path_buf()
        };

        let skill_md_path = if path.is_file() {
            path.to_path_buf()
        } else {
            source_dir.join("SKILL.md")
        };

        if !skill_md_path.exists() {
            bail!(
                "markdown skill pack '{}' is missing SKILL.md",
                source_dir.display()
            );
        }

        let content = fs::read_to_string(&skill_md_path)?;
        let (frontmatter, body) = parse_skill_markdown(&content)?;
        let name = frontmatter
            .name
            .clone()
            .unwrap_or_else(|| {
                source_dir
                    .file_name()
                    .and_then(|value| value.to_str())
                    .unwrap_or("markdown_skill")
                    .to_owned()
            })
            .trim()
            .to_owned();

        if name.is_empty() {
            bail!(
                "markdown skill pack '{}' resolved to an empty skill name",
                source_dir.display()
            );
        }

        Ok(Self {
            name,
            source_path: source_dir.clone(),
            body,
            frontmatter,
            templates_dir: optional_dir(source_dir.join("templates")),
            references_dir: optional_dir(source_dir.join("references")),
            scripts_dir: optional_dir(source_dir.join("scripts")),
        })
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn source_path(&self) -> &Path {
        &self.source_path
    }

    pub fn description(&self) -> Option<&str> {
        self.frontmatter.description.as_deref()
    }

    pub fn version(&self) -> Option<&str> {
        self.frontmatter.version.as_deref()
    }

    pub fn allowed_tools(&self) -> &[String] {
        &self.frontmatter.allowed_tools
    }

    pub fn allowed_channels(&self) -> &[String] {
        &self.frontmatter.allowed_channels
    }

    pub fn invocation_mode(&self) -> Option<mosaic_tool_core::CapabilityInvocationMode> {
        self.frontmatter.invocation_mode
    }

    pub fn accepts_attachments(&self) -> bool {
        self.frontmatter.accepts_attachments
    }

    pub fn runtime_requirements(&self) -> &[String] {
        &self.frontmatter.runtime_requirements
    }

    pub fn body(&self) -> &str {
        &self.body
    }

    pub fn templates_dir(&self) -> Option<&Path> {
        self.templates_dir.as_deref()
    }

    pub fn references_dir(&self) -> Option<&Path> {
        self.references_dir.as_deref()
    }

    pub fn scripts_dir(&self) -> Option<&Path> {
        self.scripts_dir.as_deref()
    }
}

#[async_trait]
impl Skill for MarkdownSkillPack {
    fn name(&self) -> &str {
        &self.name
    }

    async fn execute(&self, input: serde_json::Value, _ctx: &SkillContext) -> Result<SkillOutput> {
        let base_input = input_text(&input);
        let rendered = render_template(&self.body, &base_input, &base_input, &HashMap::new());
        let content = if rendered.trim().is_empty() {
            base_input.clone()
        } else {
            rendered.trim().to_owned()
        };

        Ok(SkillOutput {
            content,
            structured: Some(serde_json::json!({
                "input": input,
                "source_path": self.source_path.display().to_string(),
                "skill_md": self.source_path.join("SKILL.md").display().to_string(),
                "templates_dir": self.templates_dir.as_ref().map(|path| path.display().to_string()),
                "references_dir": self.references_dir.as_ref().map(|path| path.display().to_string()),
                "scripts_dir": self.scripts_dir.as_ref().map(|path| path.display().to_string()),
                "runtime_requirements": self.frontmatter.runtime_requirements,
            })),
        })
    }
}

fn parse_skill_markdown(content: &str) -> Result<(MarkdownSkillFrontmatter, String)> {
    let normalized = content.replace("\r\n", "\n");
    if let Some(rest) = normalized.strip_prefix("---\n") {
        let Some(split_idx) = rest.find("\n---\n") else {
            bail!("markdown skill pack frontmatter is missing closing '---'");
        };
        let frontmatter_raw = &rest[..split_idx];
        let body = &rest[(split_idx + 5)..];
        let frontmatter = if frontmatter_raw.trim().is_empty() {
            MarkdownSkillFrontmatter::default()
        } else {
            serde_yaml::from_str(frontmatter_raw)?
        };
        Ok((frontmatter, body.trim().to_owned()))
    } else {
        Ok((
            MarkdownSkillFrontmatter::default(),
            normalized.trim().to_owned(),
        ))
    }
}

fn optional_dir(path: PathBuf) -> Option<PathBuf> {
    path.is_dir().then_some(path)
}
