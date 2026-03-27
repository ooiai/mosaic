use std::sync::Arc;

use mosaic_skill_core::{
    ManifestSkillStep, SkillContext, SkillManifest, SkillRegistry, SummarizeSkill,
};
use mosaic_tool_core::ToolRegistry;

#[tokio::test]
async fn skill_registry_executes_native_and_manifest_skills() {
    let mut registry = SkillRegistry::new();
    registry.register_native(Arc::new(SummarizeSkill));
    registry.register_manifest(SkillManifest {
        name: "echo-chain".to_owned(),
        description: "Echo then summarize".to_owned(),
        input_schema: serde_json::json!({"type": "object"}),
        tools: vec![],
        system_prompt: None,
        steps: vec![
            ManifestSkillStep::Echo {
                name: "echo".to_owned(),
                input: Some("{{input}}".to_owned()),
            },
            ManifestSkillStep::Summarize {
                name: "summary".to_owned(),
                input: Some("{{steps.echo.output}}".to_owned()),
            },
        ],
    });

    let ctx = SkillContext {
        tools: Arc::new(ToolRegistry::new()),
    };
    let native = registry
        .get("summarize")
        .expect("native summarize skill should exist")
        .execute(serde_json::json!({"text": "longer text"}), &ctx)
        .await
        .expect("native summarize should execute");
    let manifest = registry
        .get("echo-chain")
        .expect("manifest skill should exist")
        .execute(serde_json::json!({"text": "pipeline"}), &ctx)
        .await
        .expect("manifest skill should execute");

    assert!(native.content.starts_with("summary:"));
    assert!(manifest.content.starts_with("summary:"));
}
