use std::{
    fs,
    net::TcpListener,
    path::{Path, PathBuf},
    process,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use mosaic_config::{
    AuditConfig, AuthConfig, DeploymentConfig, MosaicConfig, ObservabilityConfig, PolicyConfig,
    ProviderProfileConfig,
};
use mosaic_control_protocol::RunSubmission;
use mosaic_gateway::{GatewayHandle, GatewayRuntimeComponents, serve_http_with_shutdown};
use mosaic_memory::{FileMemoryStore, MemoryPolicy};
use mosaic_node_protocol::FileNodeStore;
use mosaic_provider::ProviderProfileRegistry;
use mosaic_sandbox_core::{SandboxManager, SandboxSettings};
use mosaic_scheduler_core::FileCronStore;
use mosaic_sdk::GatewayClient;
use mosaic_session_core::FileSessionStore;
use mosaic_skill_core::{SkillRegistry, SummarizeSkill};
use mosaic_tool_core::{ReadFileTool, TimeNowTool, ToolRegistry};
use mosaic_workflow::WorkflowRegistry;
use tokio::{runtime::Handle, sync::oneshot, time::timeout};

static COUNTER: AtomicU64 = AtomicU64::new(0);

fn temp_dir(label: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock should be after epoch")
        .as_nanos();
    let count = COUNTER.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "mosaic-sdk-real-{label}-{}-{nanos}-{count}",
        process::id()
    ))
}

fn free_port() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").expect("ephemeral port should bind");
    let port = listener
        .local_addr()
        .expect("listener addr should exist")
        .port();
    drop(listener);
    port
}

fn build_components(root: &Path) -> GatewayRuntimeComponents {
    let mut config = MosaicConfig::default();
    config.active_profile = "demo-provider".to_owned();
    config.profiles.insert(
        "demo-provider".to_owned(),
        ProviderProfileConfig {
            provider_type: "mock".to_owned(),
            model: "mock".to_owned(),
            base_url: None,
            api_key_env: None,
            transport: Default::default(),
            attachments: Default::default(),
            vendor: Default::default(),
        },
    );
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
    let sandbox = Arc::new({
        let manager = SandboxManager::new(&workspace_root, SandboxSettings::default());
        manager
            .ensure_layout()
            .expect("sandbox layout should be created");
        manager
    });

    GatewayRuntimeComponents {
        config_snapshot: config.clone(),
        profiles: Arc::new(
            ProviderProfileRegistry::from_config(&config).expect("provider registry should build"),
        ),
        provider_override: None,
        session_store: Arc::new(FileSessionStore::new(
            workspace_root.join(".mosaic/sessions"),
        )),
        memory_store: Arc::new(FileMemoryStore::new(workspace_root.join(".mosaic/memory"))),
        memory_policy: MemoryPolicy::default(),
        runtime_policy: config.runtime.clone(),
        attachments: config.attachments.clone(),
        sandbox,
        telegram: config.telegram.clone(),
        app_name: None,
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

#[tokio::test]
async fn real_gateway_http_chain_runs_through_sdk_when_enabled() {
    if std::env::var("MOSAIC_REAL_TESTS").ok().as_deref() != Some("1") {
        eprintln!("skipping real gateway SDK test: set MOSAIC_REAL_TESTS=1");
        return;
    }

    let root = temp_dir("gateway");
    fs::create_dir_all(&root).expect("temp root should exist");
    let gateway = GatewayHandle::new_local(Handle::current(), build_components(&root));
    let port = free_port();
    let addr = format!("127.0.0.1:{port}")
        .parse()
        .expect("socket addr should parse");
    let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
    let server_gateway = gateway.clone();
    let server = tokio::spawn(async move {
        serve_http_with_shutdown(server_gateway, addr, async move {
            let _ = shutdown_rx.await;
        })
        .await
        .expect("gateway server should run");
    });
    tokio::time::sleep(Duration::from_millis(150)).await;

    let client = GatewayClient::new(format!("http://127.0.0.1:{port}"));
    let _health = client.health().await.expect("health should succeed");
    let mut events = client
        .subscribe_events()
        .await
        .expect("event stream should connect");

    let run = client
        .submit_run(RunSubmission {
            system: None,
            input: "What time is it?".to_owned(),
            tool: None,
            skill: None,
            workflow: None,
            session_id: Some("sdk-real".to_owned()),
            profile: Some("demo-provider".to_owned()),
            ingress: None,
        })
        .await
        .expect("run submission should succeed");
    assert!(run.output.contains("current time"));

    let sessions = client.list_sessions().await.expect("sessions should list");
    assert!(sessions.iter().any(|session| session.id == "sdk-real"));

    let envelope = timeout(Duration::from_secs(10), events.next_event())
        .await
        .expect("event stream should yield within timeout")
        .expect("event stream should decode")
        .expect("event should exist");
    assert_eq!(envelope.session_id.as_deref(), Some("sdk-real"));

    drop(events);
    drop(client);
    let _ = shutdown_tx.send(());
    server.await.expect("server task should join");
    fs::remove_dir_all(root).ok();
}
