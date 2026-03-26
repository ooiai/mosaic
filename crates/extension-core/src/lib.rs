use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::Arc,
};

use anyhow::{Result, bail};
use async_trait::async_trait;
use mosaic_config::{
    AppConfig, CURRENT_SCHEMA_VERSION, ExtensionManifest, ExtensionManifestRef, McpConfig,
    MosaicConfig, PolicyConfig, SkillConfig, ToolConfig, load_extension_manifest_from_file,
};
use mosaic_mcp_core::{McpRegisteredTool, McpServerManager, McpServerSpec};
use mosaic_scheduler_core::CronStore;
use mosaic_skill_core::{
    SkillCompatibility, SkillManifest, SkillMetadata, SkillRegistry, SummarizeSkill,
};
use mosaic_tool_core::{
    CronRegisterTool, EchoTool, ExecTool, ReadFileTool, TimeNowTool, Tool, ToolCompatibility,
    ToolMetadata, ToolRegistry, WebhookTool,
};
use mosaic_workflow::{Workflow, WorkflowCompatibility, WorkflowMetadata, WorkflowRegistry};
use serde::{Deserialize, Serialize};

const BUILTIN_EXTENSION_NAME: &str = "builtin.core";
const BUILTIN_EXTENSION_VERSION: &str = "1.0.0";
const APP_EXTENSION_NAME: &str = "app.inline";
const APP_EXTENSION_VERSION: &str = "0.1.0";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExtensionStatus {
    pub name: String,
    pub version: String,
    pub source: String,
    pub enabled: bool,
    pub active: bool,
    pub tools: Vec<String>,
    pub skills: Vec<String>,
    pub workflows: Vec<String>,
    pub mcp_servers: Vec<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExtensionValidationIssue {
    pub extension: Option<String>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExtensionValidationReport {
    pub policies: PolicyConfig,
    pub extensions: Vec<ExtensionStatus>,
    pub issues: Vec<ExtensionValidationIssue>,
}

impl ExtensionValidationReport {
    pub fn is_ok(&self) -> bool {
        self.issues.is_empty()
    }
}

pub struct LoadedExtensionSet {
    pub tools: ToolRegistry,
    pub skills: SkillRegistry,
    pub workflows: WorkflowRegistry,
    pub mcp_manager: Option<Arc<McpServerManager>>,
    pub extensions: Vec<ExtensionStatus>,
    pub policies: PolicyConfig,
}

struct PlannedExtension {
    status: ExtensionStatus,
    schema_version: u32,
    tools: Vec<ToolConfig>,
    skills: Vec<SkillConfig>,
    workflows: Vec<Workflow>,
    mcp: Option<McpConfig>,
}

struct ExtensionWrappedTool {
    inner: Arc<dyn Tool>,
    metadata: ToolMetadata,
}

#[async_trait]
impl Tool for ExtensionWrappedTool {
    fn metadata(&self) -> &ToolMetadata {
        &self.metadata
    }

    async fn call(&self, input: serde_json::Value) -> Result<mosaic_tool_core::ToolResult> {
        self.inner.call(input).await
    }
}

pub fn validate_extension_set(
    config: &MosaicConfig,
    app_config: Option<&AppConfig>,
    workspace_root: &Path,
) -> ExtensionValidationReport {
    let mut planned = plan_extensions(config, app_config, workspace_root);
    let mut issues = Vec::new();
    let mut tool_names = BTreeMap::<String, String>::new();
    let mut skill_names = BTreeMap::<String, String>::new();
    let mut workflow_names = BTreeMap::<String, String>::new();

    for extension in &planned {
        if let Some(error) = extension.status.error.clone() {
            issues.push(ExtensionValidationIssue {
                extension: Some(extension.status.name.clone()),
                message: error,
            });
            continue;
        }

        if !extension.status.active {
            continue;
        }

        if extension.schema_version != CURRENT_SCHEMA_VERSION {
            issues.push(ExtensionValidationIssue {
                extension: Some(extension.status.name.clone()),
                message: format!(
                    "schema_version {} is not compatible with runtime schema {}",
                    extension.schema_version, CURRENT_SCHEMA_VERSION
                ),
            });
        }

        for tool in &extension.tools {
            if let Some(message) = tool_policy_issue(tool, &config.policies) {
                issues.push(ExtensionValidationIssue {
                    extension: Some(extension.status.name.clone()),
                    message,
                });
            }
            if let Some(previous) =
                tool_names.insert(tool.name.clone(), extension.status.name.clone())
            {
                issues.push(ExtensionValidationIssue {
                    extension: Some(extension.status.name.clone()),
                    message: format!("tool '{}' collides with extension {}", tool.name, previous),
                });
            }
        }

        if let Some(mcp) = &extension.mcp {
            if !config.policies.allow_mcp && !mcp.servers.is_empty() {
                issues.push(ExtensionValidationIssue {
                    extension: Some(extension.status.name.clone()),
                    message: "policy blocks MCP extensions".to_owned(),
                });
            }
        }

        for skill in &extension.skills {
            if let Some(previous) =
                skill_names.insert(skill.name.clone(), extension.status.name.clone())
            {
                issues.push(ExtensionValidationIssue {
                    extension: Some(extension.status.name.clone()),
                    message: format!(
                        "skill '{}' collides with extension {}",
                        skill.name, previous
                    ),
                });
            }
        }

        for workflow in &extension.workflows {
            if let Some(previous) =
                workflow_names.insert(workflow.name.clone(), extension.status.name.clone())
            {
                issues.push(ExtensionValidationIssue {
                    extension: Some(extension.status.name.clone()),
                    message: format!(
                        "workflow '{}' collides with extension {}",
                        workflow.name, previous
                    ),
                });
            }
        }
    }

    for extension in &planned {
        if !extension.status.active || extension.status.error.is_some() {
            continue;
        }

        for skill in &extension.skills {
            if skill.skill_type == "manifest" {
                for tool in &skill.tools {
                    if !tool_names.contains_key(tool) {
                        issues.push(ExtensionValidationIssue {
                            extension: Some(extension.status.name.clone()),
                            message: format!(
                                "manifest skill '{}' references unknown tool '{}'",
                                skill.name, tool
                            ),
                        });
                    }
                }
            }
        }

        for workflow in &extension.workflows {
            for step in &workflow.steps {
                match &step.kind {
                    mosaic_workflow::WorkflowStepKind::Prompt { tools, .. } => {
                        for tool in tools {
                            if !tool_names.contains_key(tool) {
                                issues.push(ExtensionValidationIssue {
                                    extension: Some(extension.status.name.clone()),
                                    message: format!(
                                        "workflow '{}' references unknown tool '{}'",
                                        workflow.name, tool
                                    ),
                                });
                            }
                        }
                    }
                    mosaic_workflow::WorkflowStepKind::Skill { skill, .. } => {
                        if !skill_names.contains_key(skill) {
                            issues.push(ExtensionValidationIssue {
                                extension: Some(extension.status.name.clone()),
                                message: format!(
                                    "workflow '{}' references unknown skill '{}'",
                                    workflow.name, skill
                                ),
                            });
                        }
                    }
                }
            }
        }
    }

    planned.iter_mut().for_each(|planned| {
        if planned.status.error.is_none() {
            let relevant = issues
                .iter()
                .any(|issue| issue.extension.as_deref() == Some(planned.status.name.as_str()));
            if relevant {
                planned.status.error = Some("validation failed".to_owned());
            }
        }
    });

    ExtensionValidationReport {
        policies: config.policies.clone(),
        extensions: planned.into_iter().map(|planned| planned.status).collect(),
        issues,
    }
}

pub fn load_extension_set(
    config: &MosaicConfig,
    app_config: Option<&AppConfig>,
    workspace_root: &Path,
    cron_store: Arc<dyn CronStore>,
) -> Result<LoadedExtensionSet> {
    let report = validate_extension_set(config, app_config, workspace_root);
    if !report.is_ok() {
        let message = report
            .issues
            .iter()
            .map(|issue| match &issue.extension {
                Some(extension) => format!("{}: {}", extension, issue.message),
                None => issue.message.clone(),
            })
            .collect::<Vec<_>>()
            .join("; ");
        bail!(message);
    }

    let planned = plan_extensions(config, app_config, workspace_root)
        .into_iter()
        .filter(|extension| extension.status.active && extension.status.error.is_none())
        .collect::<Vec<_>>();

    let mut tools = ToolRegistry::new();
    let mut skills = SkillRegistry::new();
    let mut workflows = WorkflowRegistry::new();
    let mut server_specs = Vec::new();
    let mut server_origins = BTreeMap::new();

    for extension in &planned {
        for tool in &extension.tools {
            let built = build_builtin_tool(
                tool,
                workspace_root,
                cron_store.clone(),
                &extension.status.name,
                &extension.status.version,
                extension.schema_version,
            )?;
            tools.register(built);
        }

        for skill in &extension.skills {
            register_skill(
                &mut skills,
                skill,
                &extension.status.name,
                &extension.status.version,
                extension.schema_version,
            )?;
        }

        for workflow in &extension.workflows {
            workflows.register_with_metadata(
                workflow.clone(),
                WorkflowMetadata::new(workflow.name.clone())
                    .with_extension(
                        extension.status.name.clone(),
                        extension.status.version.clone(),
                    )
                    .with_compatibility(WorkflowCompatibility {
                        schema_version: extension.schema_version,
                    }),
            );
        }

        if let Some(mcp) = &extension.mcp {
            for server in &mcp.servers {
                server_specs.push(McpServerSpec {
                    name: server.name.clone(),
                    command: server.command.clone(),
                    args: server.args.clone(),
                });
                server_origins.insert(
                    server.name.clone(),
                    (
                        extension.status.name.clone(),
                        extension.status.version.clone(),
                        extension.schema_version,
                    ),
                );
            }
        }
    }

    let mcp_manager = if server_specs.is_empty() {
        None
    } else {
        let manager = Arc::new(McpServerManager::start(&server_specs)?);
        let before = tools.list().len();
        let registered = manager.register_tools(&mut tools)?;
        if tools.list().len() != before + registered.len() {
            bail!("MCP tool registration collided with an existing tool name");
        }
        apply_mcp_extension_metadata(&mut tools, &registered, &server_origins);
        Some(manager)
    };

    Ok(LoadedExtensionSet {
        tools,
        skills,
        workflows,
        mcp_manager,
        extensions: report.extensions,
        policies: report.policies,
    })
}

fn plan_extensions(
    config: &MosaicConfig,
    app_config: Option<&AppConfig>,
    workspace_root: &Path,
) -> Vec<PlannedExtension> {
    let mut planned = Vec::new();

    if let Some(app_config) = app_config {
        planned.push(PlannedExtension {
            status: ExtensionStatus {
                name: APP_EXTENSION_NAME.to_owned(),
                version: APP_EXTENSION_VERSION.to_owned(),
                source: "app_config".to_owned(),
                enabled: true,
                active: true,
                tools: app_config
                    .tools
                    .iter()
                    .map(|tool| tool.name.clone())
                    .collect(),
                skills: app_config
                    .skills
                    .iter()
                    .map(|skill| skill.name.clone())
                    .collect(),
                workflows: app_config
                    .workflows
                    .iter()
                    .map(|workflow| workflow.name.clone())
                    .collect(),
                mcp_servers: app_config
                    .mcp
                    .as_ref()
                    .map(|mcp| {
                        mcp.servers
                            .iter()
                            .map(|server| server.name.clone())
                            .collect()
                    })
                    .unwrap_or_default(),
                error: None,
            },
            schema_version: CURRENT_SCHEMA_VERSION,
            tools: app_config.tools.clone(),
            skills: app_config.skills.clone(),
            workflows: app_config.workflows.clone(),
            mcp: app_config.mcp.clone(),
        });
    } else {
        planned.push(PlannedExtension {
            status: ExtensionStatus {
                name: BUILTIN_EXTENSION_NAME.to_owned(),
                version: BUILTIN_EXTENSION_VERSION.to_owned(),
                source: "builtin".to_owned(),
                enabled: true,
                active: true,
                tools: builtin_tool_configs(&config.policies)
                    .into_iter()
                    .map(|tool| tool.name)
                    .collect(),
                skills: vec!["summarize".to_owned()],
                workflows: Vec::new(),
                mcp_servers: Vec::new(),
                error: None,
            },
            schema_version: CURRENT_SCHEMA_VERSION,
            tools: builtin_tool_configs(&config.policies),
            skills: vec![SkillConfig {
                skill_type: "builtin".to_owned(),
                name: "summarize".to_owned(),
                description: None,
                input_schema: serde_json::json!({ "type": "object" }),
                tools: Vec::new(),
                system_prompt: None,
                steps: Vec::new(),
            }],
            workflows: Vec::new(),
            mcp: None,
        });
    }

    for manifest_ref in &config.extensions.manifests {
        planned.push(load_manifest_extension(manifest_ref, workspace_root));
    }

    planned
}

fn load_manifest_extension(
    manifest_ref: &ExtensionManifestRef,
    workspace_root: &Path,
) -> PlannedExtension {
    let resolved = resolve_manifest_path(workspace_root, &manifest_ref.path);
    if !manifest_ref.enabled {
        return PlannedExtension {
            status: ExtensionStatus {
                name: resolved
                    .file_stem()
                    .and_then(|value| value.to_str())
                    .unwrap_or("disabled.extension")
                    .to_owned(),
                version: manifest_ref
                    .version_pin
                    .clone()
                    .unwrap_or_else(|| "disabled".to_owned()),
                source: resolved.display().to_string(),
                enabled: false,
                active: false,
                tools: Vec::new(),
                skills: Vec::new(),
                workflows: Vec::new(),
                mcp_servers: Vec::new(),
                error: None,
            },
            schema_version: CURRENT_SCHEMA_VERSION,
            tools: Vec::new(),
            skills: Vec::new(),
            workflows: Vec::new(),
            mcp: None,
        };
    }

    match load_extension_manifest_from_file(&resolved) {
        Ok(manifest) => manifest_planned(manifest, manifest_ref, resolved),
        Err(err) => PlannedExtension {
            status: ExtensionStatus {
                name: manifest_ref.path.clone(),
                version: manifest_ref
                    .version_pin
                    .clone()
                    .unwrap_or_else(|| "unknown".to_owned()),
                source: resolved.display().to_string(),
                enabled: true,
                active: false,
                tools: Vec::new(),
                skills: Vec::new(),
                workflows: Vec::new(),
                mcp_servers: Vec::new(),
                error: Some(err.to_string()),
            },
            schema_version: CURRENT_SCHEMA_VERSION,
            tools: Vec::new(),
            skills: Vec::new(),
            workflows: Vec::new(),
            mcp: None,
        },
    }
}

fn manifest_planned(
    manifest: ExtensionManifest,
    manifest_ref: &ExtensionManifestRef,
    resolved: PathBuf,
) -> PlannedExtension {
    let error = match manifest_ref.version_pin.as_deref() {
        Some(expected) if expected != manifest.version => Some(format!(
            "version pin {} does not match manifest version {}",
            expected, manifest.version
        )),
        _ => None,
    };

    PlannedExtension {
        status: ExtensionStatus {
            name: manifest.name.clone(),
            version: manifest.version.clone(),
            source: resolved.display().to_string(),
            enabled: true,
            active: error.is_none(),
            tools: manifest
                .tools
                .iter()
                .map(|tool| tool.name.clone())
                .collect(),
            skills: manifest
                .skills
                .iter()
                .map(|skill| skill.name.clone())
                .collect(),
            workflows: manifest
                .workflows
                .iter()
                .map(|workflow| workflow.name.clone())
                .collect(),
            mcp_servers: manifest
                .mcp
                .as_ref()
                .map(|mcp| {
                    mcp.servers
                        .iter()
                        .map(|server| server.name.clone())
                        .collect()
                })
                .unwrap_or_default(),
            error,
        },
        schema_version: manifest.schema_version,
        tools: manifest.tools,
        skills: manifest.skills,
        workflows: manifest.workflows,
        mcp: manifest.mcp,
    }
}

fn resolve_manifest_path(workspace_root: &Path, path: &str) -> PathBuf {
    let candidate = PathBuf::from(path);
    if candidate.is_absolute() {
        candidate
    } else {
        workspace_root.join(candidate)
    }
}

fn builtin_tool_configs(policies: &PolicyConfig) -> Vec<ToolConfig> {
    let mut tools = vec![
        ToolConfig {
            tool_type: "builtin".to_owned(),
            name: "echo".to_owned(),
        },
        ToolConfig {
            tool_type: "builtin".to_owned(),
            name: "read_file".to_owned(),
        },
        ToolConfig {
            tool_type: "builtin".to_owned(),
            name: "time_now".to_owned(),
        },
    ];

    if policies.allow_cron {
        tools.push(ToolConfig {
            tool_type: "builtin".to_owned(),
            name: "cron_register".to_owned(),
        });
    }
    if policies.allow_exec {
        tools.push(ToolConfig {
            tool_type: "builtin".to_owned(),
            name: "exec_command".to_owned(),
        });
    }
    if policies.allow_webhook {
        tools.push(ToolConfig {
            tool_type: "builtin".to_owned(),
            name: "webhook_call".to_owned(),
        });
    }

    tools
}

fn tool_policy_issue(tool: &ToolConfig, policies: &PolicyConfig) -> Option<String> {
    match tool.name.as_str() {
        "exec_command" if !policies.allow_exec => Some("policy blocks exec_command".to_owned()),
        "webhook_call" if !policies.allow_webhook => Some("policy blocks webhook_call".to_owned()),
        "cron_register" if !policies.allow_cron => Some("policy blocks cron_register".to_owned()),
        _ => None,
    }
}

fn build_builtin_tool(
    tool: &ToolConfig,
    workspace_root: &Path,
    cron_store: Arc<dyn CronStore>,
    extension_name: &str,
    extension_version: &str,
    schema_version: u32,
) -> Result<Arc<dyn Tool>> {
    let roots = vec![workspace_root.to_path_buf()];
    let inner: Arc<dyn Tool> = match (tool.tool_type.as_str(), tool.name.as_str()) {
        ("builtin", "cron_register") => Arc::new(CronRegisterTool::new(cron_store)),
        ("builtin", "echo") => Arc::new(EchoTool::new()),
        ("builtin", "exec_command") => Arc::new(ExecTool::new(roots.clone())),
        ("builtin", "read_file") => Arc::new(ReadFileTool::new_with_allowed_roots(roots)),
        ("builtin", "time_now") => Arc::new(TimeNowTool::new()),
        ("builtin", "webhook_call") => Arc::new(WebhookTool::new()),
        ("builtin", other) => bail!("unsupported builtin tool: {}", other),
        (other, _) => bail!("unsupported tool type: {}", other),
    };

    Ok(wrap_tool(
        inner,
        extension_name,
        extension_version,
        schema_version,
    ))
}

fn wrap_tool(
    inner: Arc<dyn Tool>,
    extension_name: &str,
    extension_version: &str,
    schema_version: u32,
) -> Arc<dyn Tool> {
    let metadata = inner
        .metadata()
        .clone()
        .with_extension(extension_name.to_owned(), extension_version.to_owned())
        .with_compatibility(ToolCompatibility { schema_version });
    Arc::new(ExtensionWrappedTool { inner, metadata })
}

fn register_skill(
    registry: &mut SkillRegistry,
    skill: &SkillConfig,
    extension_name: &str,
    extension_version: &str,
    schema_version: u32,
) -> Result<()> {
    let compatibility = SkillCompatibility { schema_version };
    match (skill.skill_type.as_str(), skill.name.as_str()) {
        ("builtin", "summarize") => registry.register_native_with_metadata(
            Arc::new(SummarizeSkill),
            SkillMetadata::native("summarize")
                .with_extension(extension_name.to_owned(), extension_version.to_owned())
                .with_compatibility(compatibility),
        ),
        ("manifest", _) => registry.register_manifest_with_metadata(
            SkillManifest {
                name: skill.name.clone(),
                description: skill
                    .description
                    .clone()
                    .unwrap_or_else(|| format!("manifest skill {}", skill.name)),
                input_schema: skill.input_schema.clone(),
                tools: skill.tools.clone(),
                system_prompt: skill.system_prompt.clone(),
                steps: skill.steps.clone(),
            },
            SkillMetadata::manifest(&SkillManifest {
                name: skill.name.clone(),
                description: skill
                    .description
                    .clone()
                    .unwrap_or_else(|| format!("manifest skill {}", skill.name)),
                input_schema: skill.input_schema.clone(),
                tools: skill.tools.clone(),
                system_prompt: skill.system_prompt.clone(),
                steps: skill.steps.clone(),
            })
            .with_extension(extension_name.to_owned(), extension_version.to_owned())
            .with_compatibility(compatibility),
        ),
        ("builtin", other) => bail!("unsupported builtin skill: {}", other),
        (other, _) => bail!("unsupported skill type: {}", other),
    }
    Ok(())
}

fn apply_mcp_extension_metadata(
    tools: &mut ToolRegistry,
    registered: &[McpRegisteredTool],
    origins: &BTreeMap<String, (String, String, u32)>,
) {
    for registration in registered {
        let Some((extension_name, extension_version, schema_version)) =
            origins.get(&registration.server_name)
        else {
            continue;
        };
        let Some(existing) = tools.get(&registration.qualified_name) else {
            continue;
        };
        tools.register(wrap_tool(
            existing,
            extension_name,
            extension_version,
            *schema_version,
        ));
    }
}

#[cfg(test)]
mod tests {
    use std::{
        fs, process,
        sync::atomic::{AtomicU64, Ordering},
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::*;

    static COUNTER: AtomicU64 = AtomicU64::new(0);

    fn temp_dir(label: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock should be after epoch")
            .as_nanos();
        let count = COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "mosaic-extension-core-{label}-{}-{nanos}-{count}",
            process::id()
        ));
        fs::create_dir_all(&path).expect("temp dir should exist");
        path
    }

    fn repo_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("extension-core crate should live under crates/")
            .parent()
            .expect("repo root should exist")
            .to_path_buf()
    }

    #[test]
    fn example_extension_manifest_validates_from_repo() {
        let root = repo_root();
        let mut config = MosaicConfig::default();
        config.extensions.manifests.push(ExtensionManifestRef {
            path: "examples/extensions/time-and-summary.yaml".to_owned(),
            version_pin: Some("0.1.0".to_owned()),
            enabled: true,
        });

        let report = validate_extension_set(&config, None, &root);
        assert!(
            report.is_ok(),
            "example extension should validate: {:?}",
            report.issues
        );
    }

    #[test]
    fn validation_reports_version_pin_mismatch() {
        let dir = temp_dir("version-pin");
        let manifest = dir.join("demo-extension.yaml");
        fs::write(
            &manifest,
            "name: demo.extension
version: 0.2.0
description: demo
tools: []
skills: []
workflows: []
",
        )
        .expect("manifest should be written");

        let mut config = MosaicConfig::default();
        config.extensions.manifests.push(ExtensionManifestRef {
            path: manifest.display().to_string(),
            version_pin: Some("0.3.0".to_owned()),
            enabled: true,
        });

        let report = validate_extension_set(&config, None, &dir);
        assert!(!report.is_ok());
        assert!(
            report
                .issues
                .iter()
                .any(|issue| issue.message.contains("version pin"))
        );

        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn load_extension_set_applies_policy_filtered_builtins() {
        let mut config = MosaicConfig::default();
        config.policies.allow_exec = false;
        config.policies.allow_webhook = false;

        let loaded = load_extension_set(
            &config,
            None,
            Path::new("."),
            Arc::new(mosaic_scheduler_core::FileCronStore::new(
                std::env::temp_dir().join("mosaic-extension-core-cron"),
            )),
        )
        .expect("extension set should load");

        let tool_names = loaded.tools.list();
        assert!(!tool_names.contains(&"exec_command".to_owned()));
        assert!(!tool_names.contains(&"webhook_call".to_owned()));
        assert!(tool_names.contains(&"time_now".to_owned()));
    }
}
