use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use mosaic_config::{AppConfig, MosaicConfig, SkillConfig, ToolConfig};
use mosaic_skill_core::{SkillMetadata, SkillRegistry, SkillSourceKind};
use mosaic_tool_core::{
    CapabilityExposure, CapabilityInvocationMode, CapabilityVisibility, ToolMetadata, ToolRegistry,
    ToolSource,
};
use mosaic_workflow::{WorkflowMetadata, WorkflowRegistry};
use serde::{Deserialize, Serialize};

use crate::{
    APP_EXTENSION_NAME, BUILTIN_EXTENSION_NAME, ExtensionStatus, WORKSPACE_EXTENSION_NAME,
};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CapabilitySourceBreakdown {
    pub source_kind: String,
    pub count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct CapabilityVisibilitySummary {
    pub visible: usize,
    pub restricted: usize,
    pub hidden: usize,
    pub conversational: usize,
    pub explicit_only: usize,
    pub hidden_invocation: usize,
    pub attachment_capable: usize,
    pub channel_scoped: usize,
    pub profile_count: usize,
    pub telegram_bot_count: usize,
    pub bot_scoped_bindings: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct CapabilityInventorySummary {
    pub total_capabilities: usize,
    pub total_tools: usize,
    pub total_skills: usize,
    pub total_workflows: usize,
    pub total_mcp_servers: usize,
    #[serde(default)]
    pub source_breakdown: Vec<CapabilitySourceBreakdown>,
    #[serde(default)]
    pub visibility: CapabilityVisibilitySummary,
}

pub fn summarize_planned_capabilities(
    config: &MosaicConfig,
    app_config: Option<&AppConfig>,
    workspace_root: &Path,
) -> CapabilityInventorySummary {
    let planned = crate::planning::plan_extensions(config, app_config, workspace_root);
    let mut summary = base_inventory_visibility(config);
    let mut source_breakdown = BTreeMap::<String, usize>::new();

    for extension in planned
        .into_iter()
        .filter(|extension| extension.status.active && extension.status.error.is_none())
    {
        for tool in &extension.tools {
            summary.total_tools += 1;
            increment_source(
                &mut source_breakdown,
                planned_tool_source_kind(&extension.status.name),
            );
            observe_tool_config(tool, &mut summary.visibility);
        }

        for skill in &extension.skills {
            summary.total_skills += 1;
            increment_source(&mut source_breakdown, planned_skill_source_kind(skill));
            observe_skill_config(skill, &mut summary.visibility);
        }

        for workflow in &extension.workflows {
            summary.total_workflows += 1;
            increment_source(
                &mut source_breakdown,
                planned_workflow_source_kind(&extension.status.name),
            );
            observe_exposure(&workflow.visibility, &mut summary.visibility);
        }

        if let Some(mcp) = &extension.mcp {
            let mut servers = BTreeSet::new();
            for server in &mcp.servers {
                servers.insert(server.name.clone());
            }
            summary.total_mcp_servers += servers.len();
        }
    }

    finalize_inventory(summary, source_breakdown)
}

pub fn summarize_loaded_capabilities(
    tools: &ToolRegistry,
    skills: &SkillRegistry,
    workflows: &WorkflowRegistry,
    extensions: &[ExtensionStatus],
    config: &MosaicConfig,
) -> CapabilityInventorySummary {
    let mut summary = base_inventory_visibility(config);
    let mut source_breakdown = BTreeMap::<String, usize>::new();

    for tool in tools.iter() {
        let metadata = tool.metadata();
        summary.total_tools += 1;
        increment_source(&mut source_breakdown, loaded_tool_source_kind(metadata));
        observe_exposure(&metadata.exposure, &mut summary.visibility);
    }

    for skill in skills.iter() {
        let metadata = skill.metadata();
        summary.total_skills += 1;
        increment_source(&mut source_breakdown, loaded_skill_source_kind(metadata));
        observe_exposure(&metadata.exposure, &mut summary.visibility);
    }

    for workflow in workflows.iter() {
        let metadata = &workflow.metadata;
        summary.total_workflows += 1;
        increment_source(&mut source_breakdown, loaded_workflow_source_kind(metadata));
        observe_exposure(&metadata.exposure, &mut summary.visibility);
    }

    let mut servers = BTreeSet::new();
    for extension in extensions {
        for server in &extension.mcp_servers {
            servers.insert(server.clone());
        }
    }
    summary.total_mcp_servers = servers.len();

    finalize_inventory(summary, source_breakdown)
}

fn finalize_inventory(
    mut summary: CapabilityInventorySummary,
    source_breakdown: BTreeMap<String, usize>,
) -> CapabilityInventorySummary {
    summary.total_capabilities =
        summary.total_tools + summary.total_skills + summary.total_workflows;
    summary.source_breakdown = source_breakdown
        .into_iter()
        .map(|(source_kind, count)| CapabilitySourceBreakdown { source_kind, count })
        .collect();
    summary
}

fn base_inventory_visibility(config: &MosaicConfig) -> CapabilityInventorySummary {
    CapabilityInventorySummary {
        total_capabilities: 0,
        total_tools: 0,
        total_skills: 0,
        total_workflows: 0,
        total_mcp_servers: 0,
        source_breakdown: Vec::new(),
        visibility: CapabilityVisibilitySummary {
            profile_count: config.profiles.len(),
            telegram_bot_count: config
                .telegram
                .bots
                .values()
                .filter(|bot| bot.enabled)
                .count(),
            bot_scoped_bindings: config
                .telegram
                .bots
                .values()
                .filter(|bot| bot.enabled)
                .map(|bot| {
                    bot.allowed_tools.len() + bot.allowed_skills.len() + bot.allowed_workflows.len()
                })
                .sum(),
            ..CapabilityVisibilitySummary::default()
        },
    }
}

fn increment_source(source_breakdown: &mut BTreeMap<String, usize>, source_kind: &'static str) {
    *source_breakdown.entry(source_kind.to_owned()).or_insert(0) += 1;
}

fn observe_tool_config(tool: &ToolConfig, summary: &mut CapabilityVisibilitySummary) {
    observe_visibility(tool.visibility, summary);
    observe_invocation_mode(tool.invocation_mode, summary);
    if tool.accepts_attachments {
        summary.attachment_capable += 1;
    }
    if !tool.allowed_channels.is_empty() {
        summary.channel_scoped += 1;
    }
}

fn observe_skill_config(skill: &SkillConfig, summary: &mut CapabilityVisibilitySummary) {
    observe_visibility(skill.visibility, summary);
    observe_invocation_mode(skill.invocation_mode, summary);
    if skill.accepts_attachments {
        summary.attachment_capable += 1;
    }
    if !skill.allowed_channels.is_empty() {
        summary.channel_scoped += 1;
    }
}

fn observe_exposure(exposure: &CapabilityExposure, summary: &mut CapabilityVisibilitySummary) {
    observe_visibility(exposure.visibility, summary);
    observe_invocation_mode(exposure.invocation_mode, summary);
    if exposure.accepts_attachments {
        summary.attachment_capable += 1;
    }
    if !exposure.allowed_channels.is_empty() {
        summary.channel_scoped += 1;
    }
}

fn observe_visibility(visibility: CapabilityVisibility, summary: &mut CapabilityVisibilitySummary) {
    match visibility {
        CapabilityVisibility::Visible => summary.visible += 1,
        CapabilityVisibility::Restricted => summary.restricted += 1,
        CapabilityVisibility::Hidden => summary.hidden += 1,
    }
}

fn observe_invocation_mode(
    invocation_mode: CapabilityInvocationMode,
    summary: &mut CapabilityVisibilitySummary,
) {
    match invocation_mode {
        CapabilityInvocationMode::Conversational => summary.conversational += 1,
        CapabilityInvocationMode::ExplicitOnly => summary.explicit_only += 1,
        CapabilityInvocationMode::Hidden => summary.hidden_invocation += 1,
    }
}

fn planned_tool_source_kind(extension_name: &str) -> &'static str {
    if extension_name == BUILTIN_EXTENSION_NAME {
        "builtin"
    } else if matches!(
        extension_name,
        WORKSPACE_EXTENSION_NAME | APP_EXTENSION_NAME
    ) {
        "workspace_config"
    } else {
        "extension"
    }
}

fn planned_workflow_source_kind(extension_name: &str) -> &'static str {
    planned_tool_source_kind(extension_name)
}

fn planned_skill_source_kind(skill: &SkillConfig) -> &'static str {
    match skill.skill_type.as_str() {
        "builtin" => "native_skill",
        "manifest" => "manifest_skill",
        "markdown_pack" => "markdown_skill_pack",
        _ => "manifest_skill",
    }
}

fn loaded_tool_source_kind(metadata: &ToolMetadata) -> &'static str {
    match &metadata.source {
        ToolSource::Mcp { .. } => "mcp",
        ToolSource::Builtin => {
            if metadata.extension.is_some() {
                match metadata.exposure.source.as_str() {
                    "workspace_config" | "app_config" => "workspace_config",
                    value if value.starts_with("manifest:") => "extension",
                    _ => "extension",
                }
            } else {
                match metadata.exposure.source.as_str() {
                    "workspace_config" | "app_config" => "workspace_config",
                    value if value.starts_with("manifest:") => "extension",
                    _ => "builtin",
                }
            }
        }
    }
}

fn loaded_skill_source_kind(metadata: &SkillMetadata) -> &'static str {
    match metadata.source_kind {
        SkillSourceKind::Native => "native_skill",
        SkillSourceKind::Manifest => "manifest_skill",
        SkillSourceKind::MarkdownPack => "markdown_skill_pack",
    }
}

fn loaded_workflow_source_kind(metadata: &WorkflowMetadata) -> &'static str {
    if metadata.extension.is_some() {
        match metadata.exposure.source.as_str() {
            "workspace_config" | "app_config" => "workspace_config",
            value if value.starts_with("manifest:") => "extension",
            _ => "extension",
        }
    } else {
        match metadata.exposure.source.as_str() {
            "workspace_config" | "app_config" => "workspace_config",
            value if value.starts_with("manifest:") => "extension",
            _ => "builtin",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mosaic_config::{AttachmentConfig, AttachmentPolicyConfig, ExtensionsConfig, PolicyConfig};
    use mosaic_workflow::Workflow;

    fn config() -> MosaicConfig {
        let mut config = MosaicConfig::default();
        config.active_profile = "mock".to_owned();
        config.telegram.bots.insert(
            "ops".to_owned(),
            mosaic_config::TelegramBotConfig {
                enabled: true,
                bot_token_env: "BOT".to_owned(),
                webhook_secret_token_env: None,
                webhook_path: None,
                route_key: None,
                default_profile: Some("mock".to_owned()),
                allowed_tools: vec!["read_file".to_owned()],
                allowed_skills: vec!["summarize".to_owned()],
                allowed_workflows: vec!["ops_flow".to_owned()],
                attachments: None,
            },
        );
        config.policies = PolicyConfig::default();
        config.attachments = AttachmentConfig {
            policy: AttachmentPolicyConfig::default(),
            routing: Default::default(),
        };
        config.extensions = ExtensionsConfig::default();
        config
    }

    #[test]
    fn planned_summary_counts_sources_and_visibility() {
        let mut config = config();
        config.tools.push(mosaic_config::ToolConfig {
            tool_type: "builtin".to_owned(),
            name: "read_file".to_owned(),
            visibility: mosaic_tool_core::CapabilityVisibility::Restricted,
            invocation_mode: mosaic_tool_core::CapabilityInvocationMode::ExplicitOnly,
            required_policy: Some("ops.read".to_owned()),
            allowed_channels: vec!["telegram".to_owned()],
            accepts_attachments: false,
            sandbox: None,
        });
        config.skills.push(mosaic_config::SkillConfig {
            skill_type: "markdown_pack".to_owned(),
            name: "operator_note".to_owned(),
            path: Some("examples/skills/operator-note".to_owned()),
            description: None,
            input_schema: serde_json::json!({"type":"object"}),
            tools: vec!["read_file".to_owned()],
            system_prompt: None,
            steps: Vec::new(),
            visibility: mosaic_tool_core::CapabilityVisibility::Visible,
            invocation_mode: mosaic_tool_core::CapabilityInvocationMode::Conversational,
            required_policy: None,
            allowed_channels: vec!["telegram".to_owned()],
            accepts_attachments: true,
            runtime_requirements: vec!["python:venv".to_owned()],
            sandbox: None,
        });
        config.workflows.push(Workflow {
            name: "ops_flow".to_owned(),
            description: None,
            visibility: mosaic_tool_core::CapabilityExposure::new("workspace_config"),
            steps: Vec::new(),
        });

        let summary = summarize_planned_capabilities(
            &config,
            None,
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .parent()
                .expect("crate parent"),
        );

        assert!(summary.total_tools >= 1);
        assert_eq!(summary.total_skills, 2); // builtin summarize + markdown pack
        assert_eq!(summary.total_workflows, 1);
        assert!(
            summary
                .source_breakdown
                .iter()
                .any(|entry| entry.source_kind == "markdown_skill_pack")
        );
        assert!(summary.visibility.channel_scoped >= 2);
        assert_eq!(summary.visibility.telegram_bot_count, 1);
        assert_eq!(summary.visibility.bot_scoped_bindings, 3);
    }
}
