use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use anyhow::{Context, Result, anyhow, bail};
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
    pub template: Option<String>,
    #[serde(default)]
    pub references: Vec<String>,
    #[serde(default)]
    pub script: Option<String>,
    #[serde(default)]
    pub script_runtime: Option<MarkdownScriptRuntime>,
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

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MarkdownScriptRuntime {
    Auto,
    Python,
    Node,
    Shell,
}

impl MarkdownScriptRuntime {
    pub fn label(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Python => "python",
            Self::Node => "node",
            Self::Shell => "shell",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MarkdownSkillAssetRecord {
    pub name: String,
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MarkdownScriptExecutionRecord {
    pub name: String,
    pub path: String,
    pub runtime: String,
    #[serde(default)]
    pub output_mode: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MarkdownSkillExecutionRecord {
    pub pack_name: String,
    pub pack_path: String,
    pub skill_md: String,
    #[serde(default)]
    pub template: Option<MarkdownSkillAssetRecord>,
    #[serde(default)]
    pub references: Vec<MarkdownSkillAssetRecord>,
    #[serde(default)]
    pub script: Option<MarkdownScriptExecutionRecord>,
    #[serde(default)]
    pub accepts_attachments: bool,
    #[serde(default)]
    pub attachment_count: usize,
    #[serde(default)]
    pub attachment_summary: Option<String>,
    #[serde(default)]
    pub sandbox_env_id: Option<String>,
    #[serde(default)]
    pub sandbox_env_kind: Option<String>,
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

        if let Some(template) = frontmatter.template.as_deref() {
            ensure_relative_file(&source_dir.join("templates"), template, "template")?;
        }
        for reference in &frontmatter.references {
            ensure_relative_file(&source_dir.join("references"), reference, "reference")?;
        }
        if let Some(script) = frontmatter.script.as_deref() {
            ensure_relative_file(&source_dir.join("scripts"), script, "script")?;
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

    pub fn template(&self) -> Option<&str> {
        self.frontmatter.template.as_deref()
    }

    pub fn references(&self) -> &[String] {
        &self.frontmatter.references
    }

    pub fn script(&self) -> Option<&str> {
        self.frontmatter.script.as_deref()
    }

    pub fn script_runtime(&self) -> Option<MarkdownScriptRuntime> {
        self.frontmatter.script_runtime
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

    async fn execute(&self, input: serde_json::Value, ctx: &SkillContext) -> Result<SkillOutput> {
        let base_input = input_text(&input);
        let attachment_summary = summarize_attachments(&input);
        let reference_assets = self.load_references()?;
        let template_asset = self.load_template()?;
        let base_template = merged_template_body(&self.body, template_asset.as_ref());
        let rendered = render_markdown_skill_template(
            &base_template,
            &base_input,
            &attachment_summary,
            &reference_assets,
        );
        let (content, script_record, script_structured) =
            self.execute_script_if_configured(&input, &rendered, &reference_assets, ctx)?;

        let execution = MarkdownSkillExecutionRecord {
            pack_name: self.name.clone(),
            pack_path: self.source_path.display().to_string(),
            skill_md: self.source_path.join("SKILL.md").display().to_string(),
            template: template_asset.as_ref().map(|asset| asset.record()),
            references: reference_assets
                .iter()
                .map(LoadedReference::record)
                .collect(),
            script: script_record,
            accepts_attachments: self.frontmatter.accepts_attachments,
            attachment_count: attachment_summary.count,
            attachment_summary: attachment_summary.summary.clone(),
            sandbox_env_id: ctx.sandbox.as_ref().map(|sandbox| sandbox.env_id.clone()),
            sandbox_env_kind: ctx
                .sandbox
                .as_ref()
                .map(|sandbox| sandbox.kind.label().to_owned()),
        };

        Ok(SkillOutput {
            content: if content.trim().is_empty() {
                base_input.clone()
            } else {
                content.trim().to_owned()
            },
            structured: Some(serde_json::json!({
                "input": input,
                "source_path": execution.pack_path,
                "skill_md": execution.skill_md,
                "templates_dir": self.templates_dir.as_ref().map(|path| path.display().to_string()),
                "references_dir": self.references_dir.as_ref().map(|path| path.display().to_string()),
                "scripts_dir": self.scripts_dir.as_ref().map(|path| path.display().to_string()),
                "runtime_requirements": self.frontmatter.runtime_requirements,
                "markdown_pack": execution,
                "script_structured": script_structured,
            })),
        })
    }
}

#[derive(Debug, Clone)]
struct LoadedTemplate {
    name: String,
    path: PathBuf,
    content: String,
}

impl LoadedTemplate {
    fn record(&self) -> MarkdownSkillAssetRecord {
        MarkdownSkillAssetRecord {
            name: self.name.clone(),
            path: self.path.display().to_string(),
        }
    }
}

#[derive(Debug, Clone)]
struct LoadedReference {
    name: String,
    path: PathBuf,
    key: String,
    content: String,
}

impl LoadedReference {
    fn record(&self) -> MarkdownSkillAssetRecord {
        MarkdownSkillAssetRecord {
            name: self.name.clone(),
            path: self.path.display().to_string(),
        }
    }
}

#[derive(Debug, Clone)]
struct AttachmentSummary {
    count: usize,
    summary: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ScriptResponse {
    #[serde(default)]
    content: Option<String>,
    #[serde(default)]
    output_mode: Option<String>,
    #[serde(default)]
    structured: Option<serde_json::Value>,
}

impl MarkdownSkillPack {
    fn load_template(&self) -> Result<Option<LoadedTemplate>> {
        let Some(template_name) = self.template() else {
            return Ok(None);
        };
        let base = self.templates_dir.as_ref().ok_or_else(|| {
            anyhow!(
                "markdown skill pack '{}' has no templates/ directory",
                self.name
            )
        })?;
        let path = ensure_relative_file(base, template_name, "template")?;
        let content = fs::read_to_string(&path)
            .with_context(|| format!("failed to read template '{}'", path.display()))?;
        Ok(Some(LoadedTemplate {
            name: template_name.to_owned(),
            path,
            content,
        }))
    }

    fn load_references(&self) -> Result<Vec<LoadedReference>> {
        if self.references().is_empty() {
            return Ok(Vec::new());
        }
        let base = self.references_dir.as_ref().ok_or_else(|| {
            anyhow!(
                "markdown skill pack '{}' declares references but has no references/ directory",
                self.name
            )
        })?;
        self.references()
            .iter()
            .map(|name| {
                let path = ensure_relative_file(base, name, "reference")?;
                let content = fs::read_to_string(&path)
                    .with_context(|| format!("failed to read reference '{}'", path.display()))?;
                Ok(LoadedReference {
                    name: name.clone(),
                    key: normalize_reference_key(name),
                    path,
                    content: content.trim().to_owned(),
                })
            })
            .collect()
    }

    fn execute_script_if_configured(
        &self,
        input: &serde_json::Value,
        rendered: &str,
        references: &[LoadedReference],
        ctx: &SkillContext,
    ) -> Result<(
        String,
        Option<MarkdownScriptExecutionRecord>,
        Option<serde_json::Value>,
    )> {
        let Some(script_name) = self.script() else {
            return Ok((rendered.trim().to_owned(), None, None));
        };
        let sandbox = ctx.sandbox.as_ref().ok_or_else(|| {
            anyhow!(
                "markdown skill '{}' requires a sandbox env before helper scripts can run",
                self.name
            )
        })?;
        let base = self.scripts_dir.as_ref().ok_or_else(|| {
            anyhow!(
                "markdown skill pack '{}' declares a script but has no scripts/ directory",
                self.name
            )
        })?;
        let script_path = ensure_relative_file(base, script_name, "script")?;
        let runtime = self
            .script_runtime()
            .unwrap_or_else(|| infer_script_runtime(&script_path, sandbox.kind));
        let script_payload = serde_json::json!({
            "input": input,
            "rendered_prompt": rendered,
            "references": references.iter().map(|reference| serde_json::json!({
                "name": reference.name,
                "key": reference.key,
                "path": reference.path.display().to_string(),
                "content": reference.content,
            })).collect::<Vec<_>>(),
            "pack": {
                "name": self.name,
                "path": self.source_path.display().to_string(),
            },
            "sandbox": {
                "env_id": sandbox.env_id,
                "kind": sandbox.kind.label(),
                "env_dir": sandbox.env_dir.display().to_string(),
                "workdir": sandbox.workdir.display().to_string(),
                "dependency_spec": sandbox.dependency_spec,
                "status": sandbox.status,
            },
        });

        let stdout = run_helper_script(
            runtime,
            &script_path,
            &self.source_path,
            sandbox,
            &script_payload,
        )?;
        let trimmed = stdout.trim();
        if trimmed.is_empty() {
            return Ok((
                rendered.trim().to_owned(),
                Some(MarkdownScriptExecutionRecord {
                    name: script_name.to_owned(),
                    path: script_path.display().to_string(),
                    runtime: runtime.label().to_owned(),
                    output_mode: Some("empty".to_owned()),
                }),
                None,
            ));
        }

        if let Ok(response) = serde_json::from_str::<ScriptResponse>(trimmed) {
            return Ok((
                response
                    .content
                    .unwrap_or_else(|| rendered.trim().to_owned()),
                Some(MarkdownScriptExecutionRecord {
                    name: script_name.to_owned(),
                    path: script_path.display().to_string(),
                    runtime: runtime.label().to_owned(),
                    output_mode: response.output_mode.or(Some("json".to_owned())),
                }),
                response.structured,
            ));
        }

        Ok((
            trimmed.to_owned(),
            Some(MarkdownScriptExecutionRecord {
                name: script_name.to_owned(),
                path: script_path.display().to_string(),
                runtime: runtime.label().to_owned(),
                output_mode: Some("text".to_owned()),
            }),
            None,
        ))
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

fn ensure_relative_file(base: &Path, relative: &str, label: &str) -> Result<PathBuf> {
    if relative.trim().is_empty() {
        bail!("markdown skill {} path cannot be empty", label);
    }
    let base = base
        .canonicalize()
        .with_context(|| format!("failed to resolve {} directory '{}'", label, base.display()))?;
    let candidate = base.join(relative);
    if !candidate.exists() {
        bail!(
            "markdown skill {} '{}' does not exist under '{}'",
            label,
            relative,
            base.display()
        );
    }
    let canonical = candidate
        .canonicalize()
        .with_context(|| format!("failed to resolve {} '{}'", label, candidate.display()))?;
    if !canonical.starts_with(&base) {
        bail!(
            "markdown skill {} '{}' must stay inside '{}'",
            label,
            relative,
            base.display()
        );
    }
    if !canonical.is_file() {
        bail!(
            "markdown skill {} '{}' must point to a file",
            label,
            canonical.display()
        );
    }
    Ok(canonical)
}

fn merged_template_body(body: &str, template: Option<&LoadedTemplate>) -> String {
    match template {
        Some(template) if body.trim().is_empty() => template.content.clone(),
        Some(template) if template.content.trim().is_empty() => body.to_owned(),
        Some(template) => format!("{}\n\n{}", body.trim(), template.content.trim()),
        None => body.to_owned(),
    }
}

fn render_markdown_skill_template(
    template: &str,
    input: &str,
    attachments: &AttachmentSummary,
    references: &[LoadedReference],
) -> String {
    let mut rendered = render_template(template, input, input, &HashMap::new());
    rendered = rendered.replace("{{attachments.count}}", &attachments.count.to_string());
    rendered = rendered.replace(
        "{{attachments.summary}}",
        attachments.summary.as_deref().unwrap_or("none"),
    );
    for reference in references {
        rendered = rendered.replace(
            &format!("{{{{references.{}}}}}", reference.key),
            &reference.content,
        );
    }
    rendered
}

fn summarize_attachments(input: &serde_json::Value) -> AttachmentSummary {
    let attachments = input
        .get("attachments")
        .and_then(|value| value.as_array())
        .cloned()
        .unwrap_or_default();
    let summary = attachments
        .iter()
        .map(|attachment| {
            let kind = attachment
                .get("kind")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("other");
            let filename = attachment
                .get("filename")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("unnamed");
            format!("{kind}:{filename}")
        })
        .collect::<Vec<_>>();
    AttachmentSummary {
        count: attachments.len(),
        summary: (!summary.is_empty()).then(|| summary.join(", ")),
    }
}

fn normalize_reference_key(name: &str) -> String {
    name.rsplit_once('.')
        .map(|(stem, _)| stem)
        .unwrap_or(name)
        .chars()
        .map(|ch| match ch {
            'a'..='z' | '0'..='9' => ch,
            'A'..='Z' => ch.to_ascii_lowercase(),
            _ => '_',
        })
        .collect()
}

fn infer_script_runtime(
    path: &Path,
    sandbox_kind: mosaic_sandbox_core::SandboxKind,
) -> MarkdownScriptRuntime {
    match path.extension().and_then(|value| value.to_str()) {
        Some("py") => MarkdownScriptRuntime::Python,
        Some("js") | Some("mjs") | Some("cjs") => MarkdownScriptRuntime::Node,
        Some("sh") => MarkdownScriptRuntime::Shell,
        _ => match sandbox_kind {
            mosaic_sandbox_core::SandboxKind::Python => MarkdownScriptRuntime::Python,
            mosaic_sandbox_core::SandboxKind::Node => MarkdownScriptRuntime::Node,
            _ => MarkdownScriptRuntime::Shell,
        },
    }
}

fn run_helper_script(
    runtime: MarkdownScriptRuntime,
    script_path: &Path,
    pack_dir: &Path,
    sandbox: &crate::SkillSandboxContext,
    payload: &serde_json::Value,
) -> Result<String> {
    let execution_env = sandbox_execution_env(sandbox);
    let (program, args) = resolve_script_command(runtime, script_path, sandbox)?;
    let mut command = Command::new(program);
    command.args(args);
    command.current_dir(&sandbox.workdir);
    command.stdin(Stdio::piped());
    command.stdout(Stdio::piped());
    command.stderr(Stdio::piped());
    command.env("MOSAIC_SKILL_PACK_DIR", pack_dir);
    command.env("MOSAIC_SANDBOX_ENV_ID", &sandbox.env_id);
    command.env("MOSAIC_SANDBOX_WORKDIR", &sandbox.workdir);
    if let Some(node_path) = execution_env.node_path.as_ref() {
        command.env("NODE_PATH", node_path);
    }
    if let Some(path_prefix) = execution_env.path_prefix.as_ref() {
        let current_path = std::env::var_os("PATH").unwrap_or_default();
        let mut joined = std::ffi::OsString::new();
        joined.push(path_prefix);
        joined.push(if cfg!(windows) { ";" } else { ":" });
        joined.push(current_path);
        command.env("PATH", joined);
    }

    let mut child = command.spawn().with_context(|| {
        format!(
            "failed to start markdown skill helper script '{}'",
            script_path.display()
        )
    })?;
    if let Some(mut stdin) = child.stdin.take() {
        use std::io::Write;
        let body = serde_json::to_vec_pretty(payload)?;
        stdin.write_all(&body)?;
    }
    let output = child.wait_with_output().with_context(|| {
        format!(
            "failed to wait for markdown skill helper script '{}'",
            script_path.display()
        )
    })?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
        bail!(
            "markdown skill helper script '{}' failed: {}",
            script_path.display(),
            if stderr.is_empty() {
                format!("exit status {}", output.status)
            } else {
                stderr
            }
        );
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

struct SandboxExecutionEnv {
    path_prefix: Option<std::ffi::OsString>,
    node_path: Option<String>,
}

fn sandbox_execution_env(sandbox: &crate::SkillSandboxContext) -> SandboxExecutionEnv {
    let mut path_prefix = None;
    let mut node_path = None;
    match sandbox.kind {
        mosaic_sandbox_core::SandboxKind::Python => {
            for runtime_dir in [sandbox.env_dir.join("venv"), sandbox.env_dir.join("uv")] {
                let bin_dir = if cfg!(windows) {
                    runtime_dir.join("Scripts")
                } else {
                    runtime_dir.join("bin")
                };
                if bin_dir.is_dir() {
                    path_prefix = Some(bin_dir.into_os_string());
                    break;
                }
            }
        }
        mosaic_sandbox_core::SandboxKind::Node => {
            let node_modules = sandbox.env_dir.join("node_modules");
            if node_modules.is_dir() {
                node_path = Some(node_modules.display().to_string());
                let bin_dir = node_modules.join(".bin");
                if bin_dir.is_dir() {
                    path_prefix = Some(bin_dir.into_os_string());
                }
            }
        }
        mosaic_sandbox_core::SandboxKind::Shell | mosaic_sandbox_core::SandboxKind::Processor => {}
    }

    SandboxExecutionEnv {
        path_prefix,
        node_path,
    }
}

fn resolve_script_command(
    runtime: MarkdownScriptRuntime,
    script_path: &Path,
    sandbox: &crate::SkillSandboxContext,
) -> Result<(String, Vec<String>)> {
    let python_venv = sandbox.env_dir.join("venv");
    let python_uv = sandbox.env_dir.join("uv");
    let script = script_path.display().to_string();
    match runtime {
        MarkdownScriptRuntime::Auto => {
            unreachable!("auto runtime must be resolved before execution")
        }
        MarkdownScriptRuntime::Python => {
            let python = if python_venv.exists() {
                python_binary(&python_venv)
            } else if python_uv.exists() {
                python_binary(&python_uv)
            } else {
                PathBuf::from("python3")
            };
            Ok((python.display().to_string(), vec![script]))
        }
        MarkdownScriptRuntime::Node => Ok(("node".to_owned(), vec![script])),
        MarkdownScriptRuntime::Shell => Ok(("sh".to_owned(), vec![script])),
    }
}

fn python_binary(runtime_dir: &Path) -> PathBuf {
    let unix = runtime_dir.join("bin/python");
    if unix.exists() {
        unix
    } else {
        runtime_dir.join("Scripts/python.exe")
    }
}
