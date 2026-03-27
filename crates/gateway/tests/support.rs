use std::{
    fs,
    net::TcpListener,
    path::{Path, PathBuf},
    process,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::{SystemTime, UNIX_EPOCH},
};

use mosaic_config::{
    AuditConfig, AuthConfig, DeploymentConfig, MosaicConfig, ObservabilityConfig, PolicyConfig,
};
use mosaic_gateway::GatewayRuntimeComponents;
use mosaic_memory::{FileMemoryStore, MemoryPolicy};
use mosaic_node_protocol::FileNodeStore;
use mosaic_provider::ProviderProfileRegistry;
use mosaic_scheduler_core::FileCronStore;
use mosaic_session_core::FileSessionStore;
use mosaic_skill_core::{SkillRegistry, SummarizeSkill};
use mosaic_tool_core::{ReadFileTool, TimeNowTool, ToolRegistry};
use mosaic_workflow::WorkflowRegistry;

static COUNTER: AtomicU64 = AtomicU64::new(0);

pub fn temp_dir(label: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock should be after epoch")
        .as_nanos();
    let count = COUNTER.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "mosaic-gateway-integration-{label}-{}-{nanos}-{count}",
        process::id()
    ))
}

#[allow(dead_code)]
pub fn free_port() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").expect("ephemeral port should bind");
    let port = listener
        .local_addr()
        .expect("listener addr should exist")
        .port();
    drop(listener);
    port
}

pub fn build_components(root: &Path) -> GatewayRuntimeComponents {
    let workspace_root = root.to_path_buf();
    let runs_dir = workspace_root.join(".mosaic/runs");
    let audit_root = workspace_root.join(".mosaic/audit");
    fs::create_dir_all(&runs_dir).expect("runs dir should exist");
    fs::create_dir_all(&audit_root).expect("audit dir should exist");

    let mut tools = ToolRegistry::new();
    tools.register(Arc::new(TimeNowTool::new()));
    tools.register(Arc::new(ReadFileTool::new_with_allowed_roots(vec![
        workspace_root.clone(),
    ])));

    let mut skills = SkillRegistry::new();
    skills.register_native(Arc::new(SummarizeSkill));

    GatewayRuntimeComponents {
        profiles: Arc::new(
            ProviderProfileRegistry::from_config(&MosaicConfig::default())
                .expect("provider registry should build"),
        ),
        provider_override: None,
        session_store: Arc::new(FileSessionStore::new(
            workspace_root.join(".mosaic/sessions"),
        )),
        memory_store: Arc::new(FileMemoryStore::new(workspace_root.join(".mosaic/memory"))),
        memory_policy: MemoryPolicy::default(),
        tools: Arc::new(tools),
        skills: Arc::new(skills),
        workflows: Arc::new(WorkflowRegistry::new()),
        node_store: Arc::new(FileNodeStore::new(workspace_root.join(".mosaic/nodes"))),
        mcp_manager: None,
        cron_store: Arc::new(FileCronStore::new(workspace_root.join(".mosaic/cron"))),
        workspace_root,
        runs_dir,
        audit_root,
        extensions: vec![],
        policies: PolicyConfig::default(),
        deployment: DeploymentConfig::default(),
        auth: AuthConfig::default(),
        audit: AuditConfig::default(),
        observability: ObservabilityConfig::default(),
    }
}
