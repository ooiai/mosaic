use super::*;
use std::collections::BTreeMap;
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use futures::StreamExt;
use mosaic_config::{LoadConfigOptions, MosaicConfig, ProviderProfileConfig, load_mosaic_config};
use mosaic_extension_core::load_extension_set;
use mosaic_inspect::RouteMode;
use mosaic_mcp_core::{McpServerManager, McpServerSpec};
use mosaic_memory::{FileMemoryStore, MemoryPolicy};
use mosaic_provider::MockProvider;
use mosaic_scheduler_core::FileCronStore;
use mosaic_session_core::{SessionStore, TranscriptRole};
use mosaic_skill_core::SummarizeSkill;
use mosaic_workflow::{Workflow, WorkflowStep, WorkflowStepKind};
use tokio::sync::oneshot;

pub(crate) fn telegram_env_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

fn mock_profile_config() -> ProviderProfileConfig {
    ProviderProfileConfig {
        provider_type: "mock".to_owned(),
        model: "mock".to_owned(),
        base_url: None,
        api_key_env: None,
        transport: Default::default(),
        attachments: Default::default(),
        vendor: Default::default(),
    }
}

#[derive(Default)]
pub(crate) struct MemorySessionStore {
    sessions: Mutex<BTreeMap<String, SessionRecord>>,
}

impl SessionStore for MemorySessionStore {
    fn load(&self, id: &str) -> Result<Option<SessionRecord>> {
        Ok(self
            .sessions
            .lock()
            .expect("session lock should not be poisoned")
            .get(id)
            .cloned())
    }

    fn save(&self, session: &SessionRecord) -> Result<()> {
        self.sessions
            .lock()
            .expect("session lock should not be poisoned")
            .insert(session.id.clone(), session.clone());
        Ok(())
    }

    fn list(&self) -> Result<Vec<SessionSummary>> {
        Ok(self
            .sessions
            .lock()
            .expect("session lock should not be poisoned")
            .values()
            .map(SessionRecord::summary)
            .collect())
    }
}

fn test_cron_store() -> Arc<dyn CronStore> {
    Arc::new(FileCronStore::new(
        std::env::temp_dir().join(format!("mosaic-gateway-tests-cron-{}", Uuid::new_v4())),
    ))
}

fn test_node_store() -> Arc<FileNodeStore> {
    Arc::new(FileNodeStore::new(
        std::env::temp_dir().join(format!("mosaic-gateway-tests-nodes-{}", Uuid::new_v4())),
    ))
}

fn gateway() -> GatewayHandle {
    let mut config = MosaicConfig::default();
    config.active_profile = "demo-provider".to_owned();
    config
        .profiles
        .insert("demo-provider".to_owned(), mock_profile_config());
    let profiles =
        ProviderProfileRegistry::from_config(&config).expect("profile registry should build");
    let mut tools = ToolRegistry::new();
    tools.register(Arc::new(mosaic_tool_core::TimeNowTool::new()));
    let mut skills = SkillRegistry::new();
    skills.register_native(Arc::new(SummarizeSkill));
    let mut workflows = WorkflowRegistry::new();
    workflows.register(Workflow {
        name: "quick_brief".to_owned(),
        description: Some("Summarize input with one skill step".to_owned()),
        visibility: mosaic_tool_core::CapabilityExposure::default(),
        steps: vec![WorkflowStep {
            name: "summarize".to_owned(),
            kind: WorkflowStepKind::Skill {
                skill: "summarize".to_owned(),
                input: "{{input}}".to_owned(),
            },
        }],
    });
    let workspace_root = std::env::temp_dir().join("mosaic-gateway-tests-workspace");
    let sandbox = crate::build_sandbox_manager(&workspace_root, &config)
        .expect("sandbox manager should build");

    GatewayHandle::new_local(
        Handle::current(),
        GatewayRuntimeComponents {
            config_snapshot: config.clone(),
            profiles: Arc::new(profiles),
            provider_override: Some(Arc::new(MockProvider)),
            session_store: Arc::new(MemorySessionStore::default()),
            memory_store: Arc::new(FileMemoryStore::new(
                std::env::temp_dir().join("mosaic-gateway-tests-memory"),
            )),
            memory_policy: MemoryPolicy::default(),
            runtime_policy: config.runtime.clone(),
            attachments: config.attachments.clone(),
            sandbox,
            telegram: config.telegram.clone(),
            app_name: None,
            tools: Arc::new(tools),
            skills: Arc::new(skills),
            workflows: Arc::new(workflows),
            node_store: test_node_store(),
            mcp_manager: None,
            cron_store: test_cron_store(),
            workspace_root,
            runs_dir: std::env::temp_dir(),
            audit_root: std::env::temp_dir().join("mosaic-gateway-tests-audit"),
            extensions: Vec::new(),
            policies: PolicyConfig::default(),
            deployment: config.deployment.clone(),
            auth: config.auth.clone(),
            audit: config.audit.clone(),
            observability: config.observability.clone(),
        },
    )
}

fn gateway_with_filtered_workflow() -> GatewayHandle {
    let mut config = MosaicConfig::default();
    config.active_profile = "demo-provider".to_owned();
    config
        .profiles
        .insert("demo-provider".to_owned(), mock_profile_config());
    let profiles =
        ProviderProfileRegistry::from_config(&config).expect("profile registry should build");
    let mut tools = ToolRegistry::new();
    tools.register(Arc::new(mosaic_tool_core::TimeNowTool::new()));
    let mut skills = SkillRegistry::new();
    skills.register_native(Arc::new(SummarizeSkill));
    let mut workflows = WorkflowRegistry::new();
    workflows.register(Workflow {
        name: "quick_brief".to_owned(),
        description: Some("Summarize input with one skill step".to_owned()),
        visibility: mosaic_tool_core::CapabilityExposure::default(),
        steps: vec![WorkflowStep {
            name: "summarize".to_owned(),
            kind: WorkflowStepKind::Skill {
                skill: "summarize".to_owned(),
                input: "{{input}}".to_owned(),
            },
        }],
    });
    workflows.register_with_metadata(
        Workflow {
            name: "telegram_only".to_owned(),
            description: Some("Only available from telegram".to_owned()),
            visibility: mosaic_tool_core::CapabilityExposure::default(),
            steps: vec![WorkflowStep {
                name: "summarize".to_owned(),
                kind: WorkflowStepKind::Skill {
                    skill: "summarize".to_owned(),
                    input: "{{input}}".to_owned(),
                },
            }],
        },
        mosaic_workflow::WorkflowMetadata::new("telegram_only").with_exposure(
            mosaic_tool_core::CapabilityExposure::new("workspace_config")
                .with_invocation_mode(mosaic_tool_core::CapabilityInvocationMode::ExplicitOnly)
                .with_allowed_channels(vec!["telegram".to_owned()]),
        ),
    );
    let workspace_root = std::env::temp_dir().join("mosaic-gateway-tests-workspace-filtered");
    let sandbox = crate::build_sandbox_manager(&workspace_root, &config)
        .expect("sandbox manager should build");

    GatewayHandle::new_local(
        Handle::current(),
        GatewayRuntimeComponents {
            config_snapshot: config.clone(),
            profiles: Arc::new(profiles),
            provider_override: Some(Arc::new(MockProvider)),
            session_store: Arc::new(MemorySessionStore::default()),
            memory_store: Arc::new(FileMemoryStore::new(
                std::env::temp_dir().join("mosaic-gateway-tests-memory-filtered"),
            )),
            memory_policy: MemoryPolicy::default(),
            runtime_policy: config.runtime.clone(),
            attachments: config.attachments.clone(),
            sandbox,
            telegram: config.telegram.clone(),
            app_name: None,
            tools: Arc::new(tools),
            skills: Arc::new(skills),
            workflows: Arc::new(workflows),
            node_store: test_node_store(),
            mcp_manager: None,
            cron_store: test_cron_store(),
            workspace_root,
            runs_dir: std::env::temp_dir(),
            audit_root: std::env::temp_dir().join("mosaic-gateway-tests-audit-filtered"),
            extensions: Vec::new(),
            policies: PolicyConfig::default(),
            deployment: config.deployment.clone(),
            auth: config.auth.clone(),
            audit: config.audit.clone(),
            observability: config.observability.clone(),
        },
    )
}

fn mcp_script_path() -> String {
    format!(
        "{}/../../scripts/mock_mcp_server.py",
        env!("CARGO_MANIFEST_DIR")
    )
}

fn extension_workspace_dir(name: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "mosaic-gateway-extension-tests-{}-{}",
        name,
        Uuid::new_v4()
    ))
}

fn write_extension_workspace_config(root: &std::path::Path, version_pin: &str) {
    let mosaic_dir = root.join(".mosaic");
    std::fs::create_dir_all(mosaic_dir.join("extensions"))
        .expect("extension test directories should be created");
    std::fs::write(
        mosaic_dir.join("config.yaml"),
        format!(
            "extensions:
  manifests:
    - path: .mosaic/extensions/demo-extension.yaml
      version_pin: {}
policies:
  allow_exec: true
  allow_webhook: true
  allow_cron: true
  allow_mcp: true
  hot_reload_enabled: true
",
            version_pin,
        ),
    )
    .expect("workspace config should be written");
}

fn write_extension_manifest(
    root: &std::path::Path,
    version: &str,
    workflow_name: &str,
    tool_name: &str,
) {
    std::fs::write(
        root.join(".mosaic/extensions/demo-extension.yaml"),
        format!(
            "name: demo.extension
version: {}
description: demo extension
tools: []
skills: []
workflows:
  - name: {}
    description: demo workflow
    steps:
      - name: ask_time
        kind: prompt
        prompt: What time is it?
        tools:
          - {}
",
            version, workflow_name, tool_name,
        ),
    )
    .expect("extension manifest should be written");
}

fn extension_gateway(root: &std::path::Path) -> GatewayHandle {
    let workspace_config_path = root.join(".mosaic/config.yaml");
    let loaded = load_mosaic_config(&LoadConfigOptions {
        cwd: root.to_path_buf(),
        user_config_path: None,
        workspace_config_path: Some(workspace_config_path.clone()),
        overrides: Default::default(),
    })
    .expect("workspace config should load");
    let profiles = ProviderProfileRegistry::from_config(&loaded.config)
        .expect("profile registry should build");
    let cron_store: Arc<dyn CronStore> = Arc::new(FileCronStore::new(root.join(".mosaic/cron")));
    let extension_set = load_extension_set(&loaded.config, None, root, cron_store.clone())
        .expect("extension set should load");
    let sandbox =
        crate::build_sandbox_manager(root, &loaded.config).expect("sandbox manager should build");

    GatewayHandle::new_local_with_reload_source(
        Handle::current(),
        GatewayRuntimeComponents {
            config_snapshot: loaded.config.clone(),
            profiles: Arc::new(profiles),
            provider_override: Some(Arc::new(MockProvider)),
            session_store: Arc::new(MemorySessionStore::default()),
            memory_store: Arc::new(FileMemoryStore::new(root.join(".mosaic/memory"))),
            memory_policy: MemoryPolicy::default(),
            runtime_policy: loaded.config.runtime.clone(),
            attachments: loaded.config.attachments.clone(),
            sandbox,
            telegram: loaded.config.telegram.clone(),
            app_name: None,
            tools: Arc::new(extension_set.tools),
            skills: Arc::new(extension_set.skills),
            workflows: Arc::new(extension_set.workflows),
            node_store: Arc::new(FileNodeStore::new(root.join(".mosaic/nodes"))),
            mcp_manager: extension_set.mcp_manager,
            cron_store,
            workspace_root: root.to_path_buf(),
            runs_dir: root.join(".mosaic/runs"),
            audit_root: root.join(".mosaic/audit"),
            extensions: extension_set.extensions.clone(),
            policies: extension_set.policies.clone(),
            deployment: loaded.config.deployment.clone(),
            auth: loaded.config.auth.clone(),
            audit: loaded.config.audit.clone(),
            observability: loaded.config.observability.clone(),
        },
        Some(GatewayReloadSource {
            workspace_root: root.to_path_buf(),
            workspace_config_path,
            user_config_path: None,
            app_config: None,
        }),
    )
}

async fn spawn_http_gateway(gateway: GatewayHandle) -> Result<(String, oneshot::Sender<()>)> {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let addr = listener.local_addr()?;
    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    let router = http_router(gateway);
    tokio::spawn(async move {
        let _ = axum::serve(listener, router)
            .with_graceful_shutdown(async {
                let _ = shutdown_rx.await;
            })
            .await;
    });

    Ok((format!("http://{addr}"), shutdown_tx))
}

async fn read_first_sse_frame(response: reqwest::Response) -> Result<String> {
    let mut stream = response.bytes_stream();
    let mut buffer = String::new();

    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if let Some(delimiter) = buffer.find(
                "

",
            ) {
                let frame = buffer[..delimiter].to_owned();
                return Ok::<String, anyhow::Error>(frame);
            }

            let Some(chunk) = stream.next().await else {
                return Err(anyhow::anyhow!("event stream closed before first frame"));
            };
            buffer.push_str(&String::from_utf8_lossy(&chunk?));
        }
    })
    .await
    .map_err(|_| anyhow::anyhow!("timed out waiting for SSE frame"))?
}

#[tokio::test]
async fn submit_run_routes_session_and_persists_gateway_metadata() {
    let gateway = gateway();
    let submitted = gateway
        .submit_run(GatewayRunRequest {
            system: Some("You are helpful.".to_owned()),
            input: "hello gateway".to_owned(),
            tool: None,
            skill: None,
            workflow: None,
            session_id: Some("demo".to_owned()),
            profile: None,
            ingress: None,
        })
        .expect("submit should succeed");

    let result = submitted.wait().await.expect("run should succeed");
    let session = gateway
        .load_session("demo")
        .expect("load should succeed")
        .expect("session should exist");

    assert_eq!(result.trace.session_id.as_deref(), Some("demo"));
    assert_eq!(
        result.trace.gateway_run_id.as_deref(),
        Some(result.gateway_run_id.as_str())
    );
    assert_eq!(
        session.gateway.last_gateway_run_id.as_deref(),
        Some(result.gateway_run_id.as_str())
    );
    assert_eq!(
        session.run.current_gateway_run_id.as_deref(),
        Some(result.gateway_run_id.as_str())
    );
    assert_eq!(
        session.run.current_correlation_id.as_deref(),
        Some(result.correlation_id.as_str())
    );
    assert_eq!(session.run.status, RunLifecycleStatus::Success);
    assert_eq!(session.gateway.route, "gateway.local/demo");
    assert!(
        session
            .transcript
            .iter()
            .any(|message| matches!(message.role, TranscriptRole::Assistant))
    );
}

#[tokio::test]
async fn subscribe_broadcasts_submitted_runtime_and_session_events() {
    let gateway = gateway();
    let mut receiver = gateway.subscribe();
    let submitted = gateway
        .submit_command(GatewayCommand::SubmitRun(GatewayRunRequest {
            system: Some("Use tools when needed.".to_owned()),
            input: "What time is it now?".to_owned(),
            tool: None,
            skill: None,
            workflow: None,
            session_id: Some("clock".to_owned()),
            profile: None,
            ingress: None,
        }))
        .expect("submit should succeed");
    let run_id = submitted.gateway_run_id().to_owned();
    let _ = submitted.wait().await.expect("run should succeed");

    let mut saw_submitted = false;
    let mut saw_runtime = false;
    let mut saw_run_updated = false;
    let mut saw_session_updated = false;
    let mut saw_completed = false;

    while let Ok(envelope) = receiver.try_recv() {
        if envelope.gateway_run_id != run_id {
            continue;
        }

        match envelope.event {
            GatewayEvent::InboundReceived { .. } => {}
            GatewayEvent::RunSubmitted { .. } => saw_submitted = true,
            GatewayEvent::Runtime(_) => saw_runtime = true,
            GatewayEvent::RunUpdated { .. } => saw_run_updated = true,
            GatewayEvent::SessionUpdated { .. } => saw_session_updated = true,
            GatewayEvent::RunCompleted { .. } => saw_completed = true,
            GatewayEvent::OutboundDelivered { .. } => {}
            GatewayEvent::OutboundFailed { .. } => {}
            GatewayEvent::RunFailed { .. } => {}
            GatewayEvent::CapabilityJobUpdated { .. } => {}
            GatewayEvent::CronUpdated { .. } => {}
            GatewayEvent::ExtensionsReloaded { .. } => {}
            GatewayEvent::ExtensionReloadFailed { .. } => {}
        }
    }

    assert!(saw_submitted);
    assert!(saw_runtime);
    assert!(saw_run_updated);
    assert!(saw_session_updated);
    assert!(saw_completed);
}

#[tokio::test]
async fn gateway_lists_nodes_and_persists_session_affinity() {
    let gateway = gateway();
    let node_store = gateway.snapshot_components().node_store.clone();
    node_store
        .register_node(&NodeRegistration::new(
            "node-a",
            "Headless Node",
            "file-bus",
            "headless",
            vec![NodeCapabilityDeclaration {
                name: "read_file".to_owned(),
                kind: mosaic_tool_core::CapabilityKind::File,
                permission_scopes: vec![mosaic_tool_core::PermissionScope::LocalRead],
                risk: mosaic_tool_core::ToolRiskLevel::Low,
            }],
        ))
        .expect("node registration should persist");

    let nodes = gateway.list_nodes().expect("nodes should list");
    assert_eq!(nodes.len(), 1);
    assert_eq!(nodes[0].node_id, "node-a");

    gateway
        .attach_node("node-a", Some("demo"))
        .expect("node affinity should persist");
    assert_eq!(
        gateway
            .node_affinity(Some("demo"))
            .expect("node affinity should load"),
        Some("node-a".to_owned())
    );
    assert_eq!(
        gateway
            .node_capabilities("node-a")
            .expect("node capabilities should load")
            .len(),
        1
    );
}

#[tokio::test]
async fn gateway_node_store_prefers_online_nodes_when_stale_nodes_exist() {
    let gateway = gateway();
    let node_store = gateway.snapshot_components().node_store.clone();
    let mut stale = NodeRegistration::new(
        "node-stale",
        "Stale Node",
        "file-bus",
        "headless",
        vec![NodeCapabilityDeclaration {
            name: "read_file".to_owned(),
            kind: mosaic_tool_core::CapabilityKind::File,
            permission_scopes: vec![mosaic_tool_core::PermissionScope::LocalRead],
            risk: mosaic_tool_core::ToolRiskLevel::Low,
        }],
    );
    stale.last_heartbeat_at = Utc::now() - chrono::Duration::seconds(60);
    let fresh = NodeRegistration::new(
        "node-fresh",
        "Fresh Node",
        "file-bus",
        "headless",
        vec![NodeCapabilityDeclaration {
            name: "read_file".to_owned(),
            kind: mosaic_tool_core::CapabilityKind::File,
            permission_scopes: vec![mosaic_tool_core::PermissionScope::LocalRead],
            risk: mosaic_tool_core::ToolRiskLevel::Low,
        }],
    );
    node_store
        .register_node(&stale)
        .expect("stale node should persist");
    node_store
        .register_node(&fresh)
        .expect("fresh node should persist");

    let selection = node_store
        .select_node("read_file", None)
        .expect("node selection should succeed")
        .expect("node selection should exist");
    assert_eq!(selection.registration.node_id, "node-fresh");
}

#[tokio::test]
async fn gateway_handle_keeps_mcp_manager_owned_by_components() {
    let mut config = MosaicConfig::default();
    config.active_profile = "demo-provider".to_owned();
    config
        .profiles
        .insert("demo-provider".to_owned(), mock_profile_config());
    let profiles =
        ProviderProfileRegistry::from_config(&config).expect("profile registry should build");
    let mut tools = ToolRegistry::new();
    tools.register(Arc::new(mosaic_tool_core::TimeNowTool::new()));
    let manager = Arc::new(
        McpServerManager::start(&[McpServerSpec {
            name: "filesystem".to_owned(),
            command: "python3".to_owned(),
            args: vec![mcp_script_path(), "filesystem".to_owned()],
        }])
        .expect("MCP manager should start"),
    );
    let workspace_root = std::env::temp_dir().join("mosaic-gateway-tests-workspace");
    let sandbox = crate::build_sandbox_manager(&workspace_root, &config)
        .expect("sandbox manager should build");

    let gateway = GatewayHandle::new_local(
        Handle::current(),
        GatewayRuntimeComponents {
            config_snapshot: config.clone(),
            profiles: Arc::new(profiles),
            provider_override: Some(Arc::new(MockProvider)),
            session_store: Arc::new(MemorySessionStore::default()),
            memory_store: Arc::new(FileMemoryStore::new(
                std::env::temp_dir().join("mosaic-gateway-tests-memory"),
            )),
            memory_policy: MemoryPolicy::default(),
            runtime_policy: config.runtime.clone(),
            attachments: config.attachments.clone(),
            sandbox,
            telegram: config.telegram.clone(),
            app_name: None,
            tools: Arc::new(tools),
            skills: Arc::new(SkillRegistry::new()),
            workflows: Arc::new(WorkflowRegistry::new()),
            node_store: test_node_store(),
            mcp_manager: Some(manager),
            cron_store: test_cron_store(),
            workspace_root,
            runs_dir: std::env::temp_dir(),
            audit_root: std::env::temp_dir().join("mosaic-gateway-tests-audit"),
            extensions: Vec::new(),
            policies: PolicyConfig::default(),
            deployment: config.deployment.clone(),
            auth: config.auth.clone(),
            audit: config.audit.clone(),
            observability: config.observability.clone(),
        },
    );

    assert_eq!(
        gateway
            .snapshot_components()
            .mcp_manager
            .as_ref()
            .map(|manager| manager.server_count()),
        Some(1)
    );
    assert_eq!(
        gateway
            .snapshot_components()
            .mcp_manager
            .as_ref()
            .expect("MCP manager should be retained")
            .list_servers(),
        vec!["filesystem".to_owned()]
    );
}

#[tokio::test]
async fn http_health_reports_gateway_state() {
    let gateway = gateway();
    let (base_url, shutdown) = spawn_http_gateway(gateway)
        .await
        .expect("http gateway should start");

    let response = reqwest::get(format!("{base_url}/health"))
        .await
        .expect("health request should succeed");
    assert_eq!(response.status(), StatusCode::OK);
    let health: HealthResponse = response.json().await.expect("health should deserialize");

    assert_eq!(health.status, "ok");
    assert_eq!(health.active_profile, "demo-provider");
    assert_eq!(health.transport, "http+sse");

    let _ = shutdown.send(());
}

#[tokio::test]
async fn http_runs_and_sessions_handlers_return_gateway_state() {
    let gateway = gateway();
    let (base_url, shutdown) = spawn_http_gateway(gateway)
        .await
        .expect("http gateway should start");
    let client = reqwest::Client::new();

    let response = client
        .post(format!("{base_url}/runs"))
        .json(&RunSubmission {
            system: Some("You are helpful.".to_owned()),
            input: "hello over http".to_owned(),
            tool: None,
            skill: None,
            workflow: None,
            session_id: Some("http-demo".to_owned()),
            profile: None,
            ingress: None,
        })
        .send()
        .await
        .expect("run request should succeed");
    assert_eq!(response.status(), StatusCode::OK);
    let run: RunResponse = response
        .json()
        .await
        .expect("run response should deserialize");

    assert_eq!(run.trace.session_id.as_deref(), Some("http-demo"));
    assert_eq!(
        run.trace
            .ingress
            .as_ref()
            .map(|ingress| ingress.kind.as_str()),
        Some("remote_operator")
    );
    assert_eq!(
        run.trace
            .ingress
            .as_ref()
            .and_then(|ingress| ingress.channel.as_deref()),
        Some("api")
    );

    let sessions_response = client
        .get(format!("{base_url}/sessions"))
        .send()
        .await
        .expect("session list should succeed");
    assert_eq!(sessions_response.status(), StatusCode::OK);
    let sessions: Vec<SessionSummaryDto> = sessions_response
        .json()
        .await
        .expect("sessions should deserialize");
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].id, "http-demo");
    assert_eq!(sessions[0].session_route, "gateway.channel/api/http-demo");
    assert_eq!(sessions[0].channel_context.channel.as_deref(), Some("api"));

    let detail_response = client
        .get(format!("{base_url}/sessions/http-demo"))
        .send()
        .await
        .expect("session detail should succeed");
    assert_eq!(detail_response.status(), StatusCode::OK);
    let detail: SessionDetailDto = detail_response
        .json()
        .await
        .expect("session detail should deserialize");
    assert_eq!(detail.id, "http-demo");
    assert_eq!(detail.gateway.route, "gateway.channel/api/http-demo");
    assert_eq!(detail.channel_context.channel.as_deref(), Some("api"));
    assert_eq!(detail.transcript.len(), 3);

    let _ = shutdown.send(());
}

#[tokio::test]
async fn http_webchat_ingress_sets_webchat_trace_metadata() {
    let gateway = gateway();
    let (base_url, shutdown) = spawn_http_gateway(gateway)
        .await
        .expect("http gateway should start");
    let client = reqwest::Client::new();

    let response = client
        .post(format!("{base_url}/ingress/webchat"))
        .json(&InboundMessage {
            session_id: None,
            input: "hello from browser".to_owned(),
            profile: None,
            display_name: Some("guest".to_owned()),
            actor_id: Some("guest-1".to_owned()),
            conversation_id: None,
            thread_id: Some("room-7".to_owned()),
            thread_title: Some("Launch Room".to_owned()),
            reply_target: Some("webchat:guest-1".to_owned()),
            message_id: None,
            received_at: None,
            raw_event_id: None,
            ingress: None,
        })
        .send()
        .await
        .expect("webchat request should succeed");
    assert_eq!(response.status(), StatusCode::OK);
    let run: RunResponse = response
        .json()
        .await
        .expect("run response should deserialize");

    assert!(
        run.trace
            .session_id
            .as_deref()
            .is_some_and(|id| id.starts_with("webchat-"))
    );
    assert_eq!(
        run.trace
            .ingress
            .as_ref()
            .map(|ingress| ingress.kind.as_str()),
        Some("webchat_http")
    );
    assert_eq!(
        run.trace
            .ingress
            .as_ref()
            .and_then(|ingress| ingress.display_name.as_deref()),
        Some("guest")
    );

    let _ = shutdown.send(());
}

#[tokio::test]
async fn channel_explicit_tool_skill_and_workflow_commands_route_through_gateway_model() {
    let gateway = gateway();

    let tool = gateway
        .submit_webchat_message(InboundMessage {
            session_id: Some("tool-demo".to_owned()),
            input: "/mosaic tool time_now".to_owned(),
            profile: Some("demo-provider".to_owned()),
            display_name: Some("guest".to_owned()),
            actor_id: Some("guest-1".to_owned()),
            conversation_id: Some("webchat:tool".to_owned()),
            thread_id: None,
            thread_title: None,
            reply_target: Some("webchat:tool".to_owned()),
            message_id: Some("message-tool".to_owned()),
            received_at: None,
            raw_event_id: None,
            ingress: None,
        })
        .expect("tool command should submit")
        .wait()
        .await
        .expect("tool command should succeed");
    assert_eq!(
        tool.trace
            .route_decision
            .as_ref()
            .map(|route| route.route_mode),
        Some(RouteMode::Tool)
    );
    assert_eq!(
        tool.trace
            .route_decision
            .as_ref()
            .and_then(|route| route.selected_tool.as_deref()),
        Some("time_now")
    );
    assert_eq!(
        tool.trace
            .ingress
            .as_ref()
            .and_then(|ingress| ingress.control_command.as_deref()),
        Some("tool")
    );
    assert_eq!(
        tool.trace
            .ingress
            .as_ref()
            .and_then(|ingress| ingress.original_text.as_deref()),
        Some("/mosaic tool time_now")
    );

    let skill = gateway
        .submit_webchat_message(InboundMessage {
            session_id: Some("skill-demo".to_owned()),
            input: "/mosaic skill summarize summarize this please".to_owned(),
            profile: Some("demo-provider".to_owned()),
            display_name: Some("guest".to_owned()),
            actor_id: Some("guest-1".to_owned()),
            conversation_id: Some("webchat:skill".to_owned()),
            thread_id: None,
            thread_title: None,
            reply_target: Some("webchat:skill".to_owned()),
            message_id: Some("message-skill".to_owned()),
            received_at: None,
            raw_event_id: None,
            ingress: None,
        })
        .expect("skill command should submit")
        .wait()
        .await
        .expect("skill command should succeed");
    assert_eq!(
        skill
            .trace
            .route_decision
            .as_ref()
            .map(|route| route.route_mode),
        Some(RouteMode::Skill)
    );
    assert_eq!(
        skill
            .trace
            .route_decision
            .as_ref()
            .and_then(|route| route.selected_skill.as_deref()),
        Some("summarize")
    );
    assert_eq!(
        skill
            .trace
            .ingress
            .as_ref()
            .and_then(|ingress| ingress.control_command.as_deref()),
        Some("skill")
    );
    assert_eq!(
        skill
            .trace
            .ingress
            .as_ref()
            .and_then(|ingress| ingress.original_text.as_deref()),
        Some("/mosaic skill summarize summarize this please")
    );
    assert!(skill.output.starts_with("summary:"));

    let workflow = gateway
        .submit_webchat_message(InboundMessage {
            session_id: Some("workflow-demo".to_owned()),
            input: "/mosaic workflow quick_brief operator handoff".to_owned(),
            profile: Some("demo-provider".to_owned()),
            display_name: Some("guest".to_owned()),
            actor_id: Some("guest-1".to_owned()),
            conversation_id: Some("webchat:workflow".to_owned()),
            thread_id: None,
            thread_title: None,
            reply_target: Some("webchat:workflow".to_owned()),
            message_id: Some("message-workflow".to_owned()),
            received_at: None,
            raw_event_id: None,
            ingress: None,
        })
        .expect("workflow command should submit")
        .wait()
        .await
        .expect("workflow command should succeed");
    assert_eq!(
        workflow
            .trace
            .route_decision
            .as_ref()
            .map(|route| route.route_mode),
        Some(RouteMode::Workflow)
    );
    assert_eq!(
        workflow
            .trace
            .route_decision
            .as_ref()
            .and_then(|route| route.selected_workflow.as_deref()),
        Some("quick_brief")
    );
    assert_eq!(
        workflow
            .trace
            .ingress
            .as_ref()
            .and_then(|ingress| ingress.control_command.as_deref()),
        Some("workflow")
    );
    assert_eq!(
        workflow
            .trace
            .ingress
            .as_ref()
            .and_then(|ingress| ingress.original_text.as_deref()),
        Some("/mosaic workflow quick_brief operator handoff")
    );
    assert!(workflow.output.starts_with("summary:"));
}

#[tokio::test]
async fn mosaic_root_and_help_render_grouped_dynamic_catalogs() {
    let gateway = gateway();

    let catalog = gateway
        .submit_webchat_message(InboundMessage {
            session_id: Some("catalog-demo".to_owned()),
            input: "/mosaic".to_owned(),
            profile: Some("demo-provider".to_owned()),
            display_name: Some("guest".to_owned()),
            actor_id: Some("guest-1".to_owned()),
            conversation_id: Some("webchat:catalog".to_owned()),
            thread_id: None,
            thread_title: None,
            reply_target: Some("webchat:catalog".to_owned()),
            message_id: Some("message-catalog".to_owned()),
            received_at: None,
            raw_event_id: None,
            ingress: None,
        })
        .expect("catalog command should submit")
        .wait()
        .await
        .expect("catalog command should succeed");

    assert!(
        catalog
            .output
            .contains("Mosaic commands available in this conversation.")
    );
    assert!(catalog.output.contains("\nSession\n"));
    assert!(catalog.output.contains("\nRuntime\n"));
    assert!(catalog.output.contains("\nTools\n"));
    assert!(catalog.output.contains("\nSkills\n"));
    assert!(catalog.output.contains("\nWorkflows\n"));
    assert!(catalog.output.contains("\nGateway\n"));
    assert_eq!(
        catalog
            .trace
            .route_decision
            .as_ref()
            .and_then(|route| route.selected_category.as_deref()),
        None
    );
    assert_eq!(
        catalog
            .trace
            .ingress
            .as_ref()
            .and_then(|ingress| ingress.control_command.as_deref()),
        Some("catalog")
    );
    assert!(
        catalog
            .trace
            .route_decision
            .as_ref()
            .and_then(|route| route.catalog_scope.as_deref())
            .is_some_and(|scope| scope.contains("channel=webchat"))
    );

    let tools = gateway
        .submit_webchat_message(InboundMessage {
            session_id: Some("catalog-demo".to_owned()),
            input: "/mosaic help tools".to_owned(),
            profile: Some("demo-provider".to_owned()),
            display_name: Some("guest".to_owned()),
            actor_id: Some("guest-1".to_owned()),
            conversation_id: Some("webchat:catalog".to_owned()),
            thread_id: None,
            thread_title: None,
            reply_target: Some("webchat:catalog".to_owned()),
            message_id: Some("message-tools".to_owned()),
            received_at: None,
            raw_event_id: None,
            ingress: None,
        })
        .expect("help tools should submit")
        .wait()
        .await
        .expect("help tools should succeed");

    assert!(
        tools
            .output
            .contains("Tools commands available in this conversation.")
    );
    assert!(tools.output.contains("/mosaic tool time_now"));
    assert!(!tools.output.contains("\nSkills\n"));
    assert_eq!(
        tools
            .trace
            .route_decision
            .as_ref()
            .and_then(|route| route.selected_category.as_deref()),
        Some("tools")
    );
}

#[tokio::test]
async fn mosaic_help_workflows_respects_channel_visibility() {
    let gateway = gateway_with_filtered_workflow();
    let components = gateway.snapshot_components();
    let webchat = crate::command_catalog::build_command_catalog(
        &components,
        &crate::command_catalog::ChannelCommandContext {
            channel: "webchat".to_owned(),
            bot_name: None,
            session_id: Some("workflow-help".to_owned()),
            profile: "demo-provider".to_owned(),
        },
        Some(crate::command_catalog::ChannelCommandCategory::Workflows),
    )
    .render();
    let telegram = crate::command_catalog::build_command_catalog(
        &components,
        &crate::command_catalog::ChannelCommandContext {
            channel: "telegram".to_owned(),
            bot_name: None,
            session_id: Some("telegram-1".to_owned()),
            profile: "demo-provider".to_owned(),
        },
        Some(crate::command_catalog::ChannelCommandCategory::Workflows),
    )
    .render();

    assert!(webchat.contains("/mosaic workflow quick_brief <input>"));
    assert!(!webchat.contains("telegram_only"));
    assert!(telegram.contains("telegram_only"));
}

#[tokio::test]
async fn channel_session_and_profile_commands_bind_follow_up_messages() {
    let gateway = gateway();

    let session_switch = gateway
        .submit_webchat_message(InboundMessage {
            session_id: Some("webchat-default".to_owned()),
            input: "/mosaic session new ops".to_owned(),
            profile: Some("demo-provider".to_owned()),
            display_name: Some("guest".to_owned()),
            actor_id: Some("guest-1".to_owned()),
            conversation_id: Some("webchat:binding".to_owned()),
            thread_id: None,
            thread_title: None,
            reply_target: Some("webchat:binding".to_owned()),
            message_id: Some("message-session-new".to_owned()),
            received_at: None,
            raw_event_id: None,
            ingress: None,
        })
        .expect("session new should submit")
        .wait()
        .await
        .expect("session new should succeed");
    assert!(
        session_switch
            .output
            .contains("conversation bound to new session ops")
    );
    assert_eq!(session_switch.trace.session_id.as_deref(), Some("ops"));

    let profile_switch = gateway
        .submit_webchat_message(InboundMessage {
            session_id: Some("webchat-default".to_owned()),
            input: "/mosaic profile demo-provider".to_owned(),
            profile: Some("demo-provider".to_owned()),
            display_name: Some("guest".to_owned()),
            actor_id: Some("guest-1".to_owned()),
            conversation_id: Some("webchat:binding".to_owned()),
            thread_id: None,
            thread_title: None,
            reply_target: Some("webchat:binding".to_owned()),
            message_id: Some("message-profile".to_owned()),
            received_at: None,
            raw_event_id: None,
            ingress: None,
        })
        .expect("profile should submit")
        .wait()
        .await
        .expect("profile should succeed");
    assert!(
        profile_switch
            .output
            .contains("conversation profile set to demo-provider")
    );

    let follow_up = gateway
        .submit_webchat_message(InboundMessage {
            session_id: Some("webchat-default".to_owned()),
            input: "hello after binding".to_owned(),
            profile: None,
            display_name: Some("guest".to_owned()),
            actor_id: Some("guest-1".to_owned()),
            conversation_id: Some("webchat:binding".to_owned()),
            thread_id: None,
            thread_title: None,
            reply_target: Some("webchat:binding".to_owned()),
            message_id: Some("message-follow-up".to_owned()),
            received_at: None,
            raw_event_id: None,
            ingress: None,
        })
        .expect("follow-up should submit")
        .wait()
        .await
        .expect("follow-up should succeed");

    assert_eq!(follow_up.trace.session_id.as_deref(), Some("ops"));
    assert_eq!(
        follow_up
            .trace
            .effective_profile
            .as_ref()
            .map(|profile| profile.profile.as_str()),
        Some("demo-provider")
    );
}

#[tokio::test]
async fn telegram_inbound_run_records_outbound_delivery_audit_and_incident_trace() {
    let _env_guard = telegram_env_lock()
        .lock()
        .expect("telegram env lock should not be poisoned");
    let delivery_requests = Arc::new(Mutex::new(Vec::<serde_json::Value>::new()));
    let api = axum::Router::new().route(
        "/bottest-token/sendMessage",
        axum::routing::post({
            let delivery_requests = delivery_requests.clone();
            move |axum::Json(payload): axum::Json<serde_json::Value>| {
                let delivery_requests = delivery_requests.clone();
                async move {
                    delivery_requests
                        .lock()
                        .expect("delivery requests lock should not be poisoned")
                        .push(payload);
                    axum::Json(serde_json::json!({
                        "ok": true,
                        "result": { "message_id": 88 }
                    }))
                }
            }
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("telegram api listener should bind");
    let addr = listener.local_addr().expect("listener addr should resolve");
    tokio::spawn(async move {
        let _ = axum::serve(listener, api).await;
    });

    unsafe {
        std::env::set_var("MOSAIC_TELEGRAM_BOT_TOKEN", "test-token");
        std::env::set_var("MOSAIC_TELEGRAM_API_BASE_URL", format!("http://{addr}"));
    }

    let gateway = gateway();
    let bot = crate::auth::resolved_telegram_bot_by_name(&gateway.snapshot_components(), None)
        .expect("default telegram bot should resolve");
    let mut receiver = gateway.subscribe();
    let submitted = gateway
        .submit_telegram_update(
            &bot,
            TelegramUpdate {
                update_id: 9001,
                message: Some(mosaic_channel_telegram::TelegramMessage {
                    message_id: 11,
                    text: Some("hello telegram".to_owned()),
                    caption: None,
                    photo: vec![],
                    document: None,
                    message_thread_id: Some(7),
                    chat: mosaic_channel_telegram::TelegramChat {
                        id: -10042,
                        chat_type: "supergroup".to_owned(),
                        title: Some("Control Room".to_owned()),
                        username: None,
                        first_name: None,
                        last_name: None,
                    },
                    from: Some(mosaic_channel_telegram::TelegramUser {
                        id: 17,
                        username: Some("operator17".to_owned()),
                        first_name: "Real".to_owned(),
                        last_name: Some("Operator".to_owned()),
                    }),
                }),
                edited_message: None,
                channel_post: None,
            },
        )
        .expect("telegram submit should succeed");
    let result = submitted.wait().await.expect("telegram run should succeed");

    let session = gateway
        .load_session("telegram-default--10042-7")
        .expect("session load should succeed")
        .expect("telegram session should exist");
    assert_eq!(result.trace.outbound_deliveries.len(), 1);
    assert_eq!(
        result.trace.outbound_deliveries[0].result.status.label(),
        "delivered"
    );
    assert_eq!(
        result.trace.outbound_deliveries[0]
            .result
            .provider_message_id
            .as_deref(),
        Some("88")
    );
    assert_eq!(
        session.channel_context.conversation_id.as_deref(),
        Some("telegram:bot:default:chat:-10042")
    );
    assert_eq!(
        session.channel_context.last_delivery_status.as_deref(),
        Some("delivered")
    );

    let requests = delivery_requests
        .lock()
        .expect("delivery requests lock should not be poisoned");
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0]["chat_id"], -10042);
    assert_eq!(requests[0]["message_thread_id"], 7);
    assert_eq!(requests[0]["reply_to_message_id"], 11);
    drop(requests);

    let audit_events = gateway.audit_events(20);
    assert!(
        audit_events
            .iter()
            .any(|event| event.kind == "channel.inbound_received")
    );
    assert!(
        audit_events
            .iter()
            .any(|event| event.kind == "channel.outbound_delivered")
    );

    let mut saw_inbound = false;
    let mut saw_outbound = false;
    while let Ok(envelope) = receiver.try_recv() {
        if envelope.gateway_run_id != result.gateway_run_id {
            continue;
        }
        match envelope.event {
            GatewayEvent::InboundReceived { .. } => saw_inbound = true,
            GatewayEvent::OutboundDelivered { .. } => saw_outbound = true,
            _ => {}
        }
    }
    assert!(saw_inbound);
    assert!(saw_outbound);

    let (bundle, _) = gateway
        .incident_bundle(&result.gateway_run_id)
        .expect("incident bundle should build");
    assert_eq!(bundle.trace.outbound_deliveries.len(), 1);
    assert!(
        bundle
            .audit_events
            .iter()
            .any(|event| event.kind == "channel.outbound_delivered")
    );

    unsafe {
        std::env::remove_var("MOSAIC_TELEGRAM_BOT_TOKEN");
        std::env::remove_var("MOSAIC_TELEGRAM_API_BASE_URL");
    }
}

#[tokio::test]
async fn reload_extensions_swaps_workflow_registry_and_broadcasts_event() {
    let root = extension_workspace_dir("reload-ok");
    write_extension_workspace_config(&root, "0.1.0");
    write_extension_manifest(&root, "0.1.0", "demo_flow_alpha", "time_now");
    let gateway = extension_gateway(&root);
    let mut receiver = gateway.subscribe();

    assert!(
        gateway
            .snapshot_components()
            .workflows
            .get("demo_flow_alpha")
            .is_some()
    );

    write_extension_manifest(&root, "0.1.0", "demo_flow_beta", "time_now");
    gateway
        .reload_extensions()
        .expect("extension reload should succeed");

    let components = gateway.snapshot_components();
    assert!(components.workflows.get("demo_flow_beta").is_some());
    assert!(components.workflows.get("demo_flow_alpha").is_none());
    assert!(
        components
            .extensions
            .iter()
            .any(|extension| extension.name == "demo.extension")
    );

    let mut saw_reloaded = false;
    while let Ok(envelope) = receiver.try_recv() {
        if matches!(envelope.event, GatewayEvent::ExtensionsReloaded { .. }) {
            saw_reloaded = true;
            break;
        }
    }
    assert!(saw_reloaded);

    std::fs::remove_dir_all(root).ok();
}

#[tokio::test]
async fn reload_extensions_rolls_back_on_failure_and_broadcasts_error() {
    let root = extension_workspace_dir("reload-fail");
    write_extension_workspace_config(&root, "0.1.0");
    write_extension_manifest(&root, "0.1.0", "demo_flow_alpha", "time_now");
    let gateway = extension_gateway(&root);
    let mut receiver = gateway.subscribe();

    write_extension_manifest(&root, "0.1.0", "demo_flow_broken", "missing_tool");
    let error = gateway
        .reload_extensions()
        .expect_err("extension reload should fail");
    assert!(error.to_string().contains("missing_tool"));

    let components = gateway.snapshot_components();
    assert!(components.workflows.get("demo_flow_alpha").is_some());
    assert!(components.workflows.get("demo_flow_broken").is_none());

    let mut saw_failure = false;
    while let Ok(envelope) = receiver.try_recv() {
        if matches!(envelope.event, GatewayEvent::ExtensionReloadFailed { .. }) {
            saw_failure = true;
            break;
        }
    }
    assert!(saw_failure);

    std::fs::remove_dir_all(root).ok();
}

#[tokio::test]
async fn http_events_stream_emits_gateway_envelopes() {
    let gateway = gateway();
    let (base_url, shutdown) = spawn_http_gateway(gateway.clone())
        .await
        .expect("http gateway should start");
    let client = reqwest::Client::new();
    let events_response = client
        .get(format!("{base_url}/events"))
        .send()
        .await
        .expect("event stream request should succeed");
    assert_eq!(events_response.status(), StatusCode::OK);

    let wait_handle = gateway
        .submit_run(GatewayRunRequest {
            system: Some("You are helpful.".to_owned()),
            input: "hello events".to_owned(),
            tool: None,
            skill: None,
            workflow: None,
            session_id: Some("events-demo".to_owned()),
            profile: None,
            ingress: None,
        })
        .expect("run submission should succeed");
    let frame = read_first_sse_frame(events_response)
        .await
        .expect("first SSE frame should arrive");
    let _ = wait_handle.wait().await.expect("run should succeed");

    assert!(frame.contains("data:"));
    assert!(frame.contains("RunUpdated") || frame.contains("RunSubmitted"));
    assert!(frame.contains("events-demo"));

    let _ = shutdown.send(());
}
