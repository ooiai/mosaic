use std::sync::Arc;

use futures::executor::block_on;
use mosaic_tool_core::{EchoTool, ToolRegistry};

use crate::{ManifestSkillStep, SkillContext, SkillManifest, SkillRegistry, SummarizeSkill};

#[test]
fn summarize_skill_is_registered_and_returns_summary_text() {
    let mut registry = SkillRegistry::new();
    registry.register(Arc::new(SummarizeSkill));
    let ctx = SkillContext {
        tools: Arc::new(ToolRegistry::new()),
    };

    let output = block_on(
        registry
            .get("summarize")
            .expect("summarize skill should be registered")
            .execute(serde_json::json!({ "text": "Explain Mosaic." }), &ctx),
    )
    .expect("skill should execute");

    assert_eq!(output.content, "summary: Explain Mosaic.");
}

#[test]
fn manifest_skill_registers_and_executes_steps() {
    let mut tools = ToolRegistry::new();
    tools.register(Arc::new(EchoTool::new()));

    let mut registry = SkillRegistry::new();
    registry.register_manifest(SkillManifest {
        name: "compose".to_owned(),
        description: "Compose several steps".to_owned(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "text": { "type": "string" }
            }
        }),
        tools: vec!["echo".to_owned()],
        system_prompt: None,
        steps: vec![
            ManifestSkillStep::Echo {
                name: "draft".to_owned(),
                input: Some("Draft: {{input}}".to_owned()),
            },
            ManifestSkillStep::Summarize {
                name: "summary".to_owned(),
                input: Some("{{steps.draft}}".to_owned()),
            },
        ],
    });

    let ctx = SkillContext {
        tools: Arc::new(tools),
    };

    let output = block_on(
        registry
            .get("compose")
            .expect("manifest skill should be registered")
            .execute(serde_json::json!({ "text": "Mosaic" }), &ctx),
    )
    .expect("manifest skill should execute");

    assert_eq!(output.content, "summary: Draft: Mosaic");
    assert_eq!(
        output.structured.as_ref().expect("structured output")["input"],
        "Mosaic"
    );
}
