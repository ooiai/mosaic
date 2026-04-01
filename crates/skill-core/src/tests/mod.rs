use std::{
    fs, process,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::{SystemTime, UNIX_EPOCH},
};

use futures::executor::block_on;
use mosaic_tool_core::{EchoTool, ToolRegistry};

use crate::{
    ManifestSkillStep, MarkdownSkillPack, SkillContext, SkillManifest, SkillRegistry,
    SkillSourceKind, SummarizeSkill,
};

static COUNTER: AtomicU64 = AtomicU64::new(0);

fn temp_dir(label: &str) -> std::path::PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock should be after epoch")
        .as_nanos();
    let count = COUNTER.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
        "mosaic-skill-core-{label}-{}-{nanos}-{count}",
        process::id()
    ));
    fs::create_dir_all(&path).expect("temp dir should exist");
    path
}

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

#[test]
fn markdown_skill_pack_loads_and_executes_as_template_skill() {
    let dir = temp_dir("markdown-pack");
    fs::write(
        dir.join("SKILL.md"),
        r#"---
name: operator_note
description: Render an operator note template
version: 0.1.0
allowed_tools:
  - read_file
allowed_channels:
  - telegram
invocation_mode: explicit_only
accepts_attachments: true
runtime_requirements:
  - python
---
Operator note:
{{input}}
"#,
    )
    .expect("skill pack should be written");
    fs::create_dir_all(dir.join("templates")).expect("templates dir should exist");

    let pack = MarkdownSkillPack::load_from_dir(&dir).expect("markdown skill should load");
    assert_eq!(pack.name(), "operator_note");
    assert_eq!(pack.version(), Some("0.1.0"));
    assert_eq!(pack.allowed_tools(), &["read_file".to_owned()]);
    assert_eq!(pack.allowed_channels(), &["telegram".to_owned()]);
    assert_eq!(pack.runtime_requirements(), &["python".to_owned()]);

    let metadata = crate::SkillMetadata::markdown_pack(&pack);
    assert_eq!(metadata.source_kind, SkillSourceKind::MarkdownPack);
    assert_eq!(metadata.skill_version.as_deref(), Some("0.1.0"));

    let mut registry = SkillRegistry::new();
    registry.register_markdown_pack(pack);
    let ctx = SkillContext {
        tools: Arc::new(ToolRegistry::new()),
    };

    let output = block_on(
        registry
            .get("operator_note")
            .expect("markdown skill should be registered")
            .execute(serde_json::json!({ "text": "Check the alerts." }), &ctx),
    )
    .expect("markdown skill should execute");

    assert_eq!(output.content, "Operator note:\nCheck the alerts.");

    fs::remove_dir_all(dir).ok();
}
