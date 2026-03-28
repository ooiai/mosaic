use std::{
    fs,
    path::PathBuf,
    process,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::{SystemTime, UNIX_EPOCH},
};

use mosaic_config::MosaicConfig;
use mosaic_memory::{FileMemoryStore, MemoryPolicy};
use mosaic_node_protocol::FileNodeStore;
use mosaic_provider::ProviderProfileRegistry;
use mosaic_runtime::{AgentRuntime, RunRequest, RuntimeContext, events::shared_noop_event_sink};
use mosaic_session_core::{FileSessionStore, SessionStore};
use mosaic_skill_core::{SkillRegistry, SummarizeSkill};
use mosaic_tool_core::{TimeNowTool, ToolRegistry};
use mosaic_workflow::WorkflowRegistry;

static COUNTER: AtomicU64 = AtomicU64::new(0);

fn temp_dir(label: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock should be after epoch")
        .as_nanos();
    let count = COUNTER.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "mosaic-runtime-integration-{label}-{}-{nanos}-{count}",
        process::id()
    ))
}

#[tokio::test]
async fn runtime_executes_mock_provider_tool_loop_against_real_session_and_memory_stores() {
    let root = temp_dir("runtime");
    fs::create_dir_all(&root).expect("temp root should exist");

    let mut tools = ToolRegistry::new();
    tools.register(Arc::new(TimeNowTool::new()));

    let mut skills = SkillRegistry::new();
    skills.register_native(Arc::new(SummarizeSkill));

    let ctx = RuntimeContext {
        profiles: Arc::new(
            ProviderProfileRegistry::from_config(&MosaicConfig::default())
                .expect("provider registry should build"),
        ),
        provider_override: None,
        session_store: Arc::new(FileSessionStore::new(root.join("sessions"))),
        memory_store: Arc::new(FileMemoryStore::new(root.join("memory"))),
        memory_policy: MemoryPolicy::default(),
        runtime_policy: MosaicConfig::default().runtime,
        tools: Arc::new(tools),
        skills: Arc::new(skills),
        workflows: Arc::new(WorkflowRegistry::new()),
        node_router: Some(Arc::new(FileNodeStore::new(root.join("nodes")))),
        active_extensions: vec![],
        event_sink: shared_noop_event_sink(),
    };

    let runtime = AgentRuntime::new(ctx);
    let result = runtime
        .run(RunRequest {
            run_id: None,
            system: None,
            input: "What time is it right now?".to_owned(),
            tool: None,
            skill: None,
            workflow: None,
            session_id: Some("demo".to_owned()),
            profile: Some("mock".to_owned()),
            ingress: None,
        })
        .await
        .expect("runtime run should succeed");

    assert!(result.output.contains("current time"));
    assert_eq!(result.trace.tool_calls.len(), 1);
    let session = FileSessionStore::new(root.join("sessions"))
        .load("demo")
        .expect("session load should succeed")
        .expect("session should exist");
    assert!(session.transcript.len() >= 2);

    fs::remove_dir_all(root).ok();
}
