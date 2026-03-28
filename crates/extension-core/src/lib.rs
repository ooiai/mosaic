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
    CapabilityExposure, CronRegisterTool, EchoTool, ExecTool, ReadFileTool, TimeNowTool, Tool,
    ToolCompatibility, ToolMetadata, ToolRegistry, WebhookTool,
};
use mosaic_workflow::{Workflow, WorkflowCompatibility, WorkflowMetadata, WorkflowRegistry};
use serde::{Deserialize, Serialize};

mod builtin;
mod loading;
mod planning;
mod validation;

const BUILTIN_EXTENSION_NAME: &str = "builtin.core";
const BUILTIN_EXTENSION_VERSION: &str = "1.0.0";
const WORKSPACE_EXTENSION_NAME: &str = "workspace.inline";
const WORKSPACE_EXTENSION_VERSION: &str = "0.1.0";
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
    validation::validate_extension_set(config, app_config, workspace_root)
}

pub fn load_extension_set(
    config: &MosaicConfig,
    app_config: Option<&AppConfig>,
    workspace_root: &Path,
    cron_store: Arc<dyn CronStore>,
) -> Result<LoadedExtensionSet> {
    loading::load_extension_set(config, app_config, workspace_root, cron_store)
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

fn exposure_from_tool_config(tool: &ToolConfig, source: &str) -> CapabilityExposure {
    CapabilityExposure::new(source)
        .with_visibility(tool.visibility)
        .with_invocation_mode(tool.invocation_mode)
        .with_required_policy(tool.required_policy.clone())
        .with_allowed_channels(tool.allowed_channels.clone())
}

fn exposure_from_skill_config(skill: &SkillConfig, source: &str) -> CapabilityExposure {
    CapabilityExposure::new(source)
        .with_visibility(skill.visibility)
        .with_invocation_mode(skill.invocation_mode)
        .with_required_policy(skill.required_policy.clone())
        .with_allowed_channels(skill.allowed_channels.clone())
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
