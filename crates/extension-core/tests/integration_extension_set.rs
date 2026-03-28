use std::{path::PathBuf, sync::Arc};

use mosaic_config::{
    ExtensionManifestRef, ExtensionsConfig, MosaicConfig, SkillConfig, ToolConfig,
};
use mosaic_extension_core::{load_extension_set, validate_extension_set};
use mosaic_scheduler_core::FileCronStore;
use mosaic_tool_core::{CapabilityInvocationMode, CapabilityVisibility};
use mosaic_workflow::{Workflow, WorkflowStep, WorkflowStepKind};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("extension-core crate should live under crates/")
        .parent()
        .expect("repo root should exist")
        .to_path_buf()
}

#[test]
fn extension_set_loads_example_manifest_into_public_registries() {
    let root = repo_root();
    for path in [
        "examples/extensions/time-and-summary.yaml",
        "examples/extensions/telegram-e2e.yaml",
    ] {
        let mut config = MosaicConfig::default();
        config.extensions = ExtensionsConfig {
            manifests: vec![ExtensionManifestRef {
                path: path.to_owned(),
                version_pin: Some("0.1.0".to_owned()),
                enabled: true,
            }],
        };

        let report = validate_extension_set(&config, None, &root);
        assert!(
            report.is_ok(),
            "extension validation issues for {path}: {:?}",
            report.issues
        );

        let loaded = load_extension_set(
            &config,
            None,
            &root,
            Arc::new(FileCronStore::new(root.join(".tmp-extension-cron"))),
        )
        .expect("extension set should load");

        assert!(loaded.skills.get("summarize_notes").is_some());
        assert!(loaded.workflows.get("summarize_operator_note").is_some());
    }
}

#[test]
fn workspace_config_and_manifest_capabilities_merge_into_final_registry() {
    let root = repo_root();
    let mut config = MosaicConfig::default();
    config.tools = vec![ToolConfig {
        tool_type: "builtin".to_owned(),
        name: "time_now".to_owned(),
        visibility: CapabilityVisibility::Restricted,
        invocation_mode: CapabilityInvocationMode::ExplicitOnly,
        required_policy: Some("workspace_override".to_owned()),
        allowed_channels: vec!["telegram".to_owned()],
    }];
    config.skills = vec![SkillConfig {
        skill_type: "manifest".to_owned(),
        name: "workspace_summary".to_owned(),
        description: Some("Summarize text from workspace config".to_owned()),
        input_schema: serde_json::json!({ "type": "object" }),
        tools: Vec::new(),
        system_prompt: Some("Summarize briefly.".to_owned()),
        steps: Vec::new(),
        visibility: CapabilityVisibility::Visible,
        invocation_mode: CapabilityInvocationMode::Conversational,
        required_policy: None,
        allowed_channels: vec!["webchat".to_owned()],
    }];
    config.workflows = vec![Workflow {
        name: "workspace_flow".to_owned(),
        description: Some("Workflow from workspace config".to_owned()),
        visibility: mosaic_tool_core::CapabilityExposure::new("workspace_config")
            .with_visibility(CapabilityVisibility::Visible)
            .with_invocation_mode(CapabilityInvocationMode::Conversational),
        steps: vec![WorkflowStep {
            name: "summarize".to_owned(),
            kind: WorkflowStepKind::Skill {
                skill: "workspace_summary".to_owned(),
                input: "{{input}}".to_owned(),
            },
        }],
    }];
    config.extensions = ExtensionsConfig {
        manifests: vec![ExtensionManifestRef {
            path: "examples/extensions/time-and-summary.yaml".to_owned(),
            version_pin: Some("0.1.0".to_owned()),
            enabled: true,
        }],
    };

    let loaded = load_extension_set(
        &config,
        None,
        &root,
        Arc::new(FileCronStore::new(root.join(".tmp-extension-cron-merge"))),
    )
    .expect("extension set should load");

    let time_now = loaded
        .tools
        .get("time_now")
        .expect("time_now should be present");
    assert_eq!(time_now.metadata().exposure.source, "workspace_config");
    assert_eq!(
        time_now.metadata().exposure.invocation_mode,
        CapabilityInvocationMode::ExplicitOnly
    );
    assert_eq!(
        time_now.metadata().exposure.allowed_channels,
        vec!["telegram".to_owned()]
    );
    assert!(loaded.skills.get("workspace_summary").is_some());
    assert!(loaded.workflows.get("workspace_flow").is_some());
    assert!(loaded.skills.get("summarize_notes").is_some());
    assert!(loaded.workflows.get("summarize_operator_note").is_some());
}
