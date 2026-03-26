use std::{
    collections::{BTreeMap, VecDeque},
    convert::Infallible,
    env,
    fs::{self, OpenOptions},
    io::Write,
    net::SocketAddr,
    path::{Path as FsPath, PathBuf},
    sync::{Arc, Mutex},
    time::Duration,
};

use anyhow::{Result, anyhow, bail};
use axum::{
    Json, Router,
    extract::{Path as AxumPath, Query, State},
    http::{HeaderMap, StatusCode},
    response::sse::{Event as SseEvent, KeepAlive, Sse},
    routing::{get, post},
};
use chrono::Utc;
use futures::stream;
use mosaic_channel_telegram::{TelegramUpdate, normalize_update as normalize_telegram_update};
use mosaic_config::{
    AppConfig, AuditConfig, AuthConfig, DeploymentConfig, LoadConfigOptions, ObservabilityConfig,
    PolicyConfig, load_mosaic_config,
};
use mosaic_control_protocol::{
    AdapterStatusDto, CapabilityJobDto, CronRegistrationDto, CronRegistrationRequest,
    ErrorResponse, EventStreamEnvelope, ExecJobRequest, ExtensionPolicyDto, ExtensionStatusDto,
    GatewayAuditEventDto, GatewayEvent, HealthResponse, InboundMessage, IncidentBundleDto,
    MetricsResponse, ReadinessResponse, ReplayWindowResponse, RunResponse, RunSubmission,
    SessionChannelDto, SessionDetailDto, SessionGatewayDto, SessionSummaryDto,
    TranscriptMessageDto, TranscriptRoleDto, WebhookJobRequest,
};
use mosaic_extension_core::{
    ExtensionStatus, ExtensionValidationReport, load_extension_set, validate_extension_set,
};
use mosaic_inspect::{ExtensionTrace, GovernanceTrace, IngressTrace, RunTrace};
use mosaic_mcp_core::McpServerManager;
use mosaic_memory::{MemoryPolicy, MemoryStore};
use mosaic_node_protocol::{FileNodeStore, NodeCapabilityDeclaration, NodeRegistration};
use mosaic_provider::{LlmProvider, ProviderProfileRegistry, public_error_message};
use mosaic_runtime::events::{RunEvent, RunEventSink, SharedRunEventSink};
use mosaic_runtime::{AgentRuntime, RunError, RunRequest, RunResult, RuntimeContext};
use mosaic_scheduler_core::{CronRegistration, CronStore};
use mosaic_session_core::{
    SessionChannelMetadata, SessionRecord, SessionStore, SessionSummary, TranscriptRole,
    session_route_for_id,
};
use mosaic_skill_core::SkillRegistry;
use mosaic_tool_core::ToolRegistry;
use mosaic_workflow::WorkflowRegistry;
use serde::{Deserialize, Serialize};
use tokio::runtime::Handle;
use tokio::sync::broadcast;
use tracing::{info, warn};
use uuid::Uuid;

pub use mosaic_control_protocol::{
    GatewayEvent as ProtocolGatewayEvent, HealthResponse as GatewayHealthResponse,
};

const DEFAULT_AUDIT_QUERY_LIMIT: usize = 50;
const DEFAULT_REPLAY_QUERY_LIMIT: usize = 50;

#[derive(Clone)]
pub struct GatewayRuntimeComponents {
    pub profiles: Arc<ProviderProfileRegistry>,
    pub provider_override: Option<Arc<dyn LlmProvider>>,
    pub session_store: Arc<dyn SessionStore>,
    pub memory_store: Arc<dyn MemoryStore>,
    pub memory_policy: MemoryPolicy,
    pub tools: Arc<ToolRegistry>,
    pub skills: Arc<SkillRegistry>,
    pub workflows: Arc<WorkflowRegistry>,
    pub node_store: Arc<FileNodeStore>,
    pub mcp_manager: Option<Arc<McpServerManager>>,
    pub cron_store: Arc<dyn CronStore>,
    pub workspace_root: PathBuf,
    pub runs_dir: PathBuf,
    pub audit_root: PathBuf,
    pub extensions: Vec<ExtensionStatus>,
    pub policies: PolicyConfig,
    pub deployment: DeploymentConfig,
    pub auth: AuthConfig,
    pub audit: AuditConfig,
    pub observability: ObservabilityConfig,
}

impl GatewayRuntimeComponents {
    pub fn runtime_context(&self, event_sink: SharedRunEventSink) -> RuntimeContext {
        RuntimeContext {
            profiles: self.profiles.clone(),
            provider_override: self.provider_override.clone(),
            session_store: self.session_store.clone(),
            memory_store: self.memory_store.clone(),
            memory_policy: self.memory_policy.clone(),
            tools: self.tools.clone(),
            skills: self.skills.clone(),
            workflows: self.workflows.clone(),
            node_router: Some(self.node_store.clone()),
            active_extensions: self.extensions.iter().map(extension_trace).collect(),
            event_sink,
        }
    }
}

#[derive(Debug, Clone)]
pub struct GatewayReloadSource {
    pub workspace_root: PathBuf,
    pub workspace_config_path: PathBuf,
    pub user_config_path: Option<PathBuf>,
    pub app_config: Option<AppConfig>,
}

#[derive(Debug, Clone)]
pub struct GatewayExtensionReloadResult {
    pub extensions: Vec<ExtensionStatus>,
    pub policies: PolicyConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum GatewayCommand {
    SubmitRun(GatewayRunRequest),
}

pub type GatewayRunRequest = RunSubmission;
pub type GatewayEventEnvelope = EventStreamEnvelope;

#[derive(Debug)]
pub struct GatewayRunResult {
    pub gateway_run_id: String,
    pub correlation_id: String,
    pub session_route: String,
    pub output: String,
    pub trace: RunTrace,
    pub trace_path: PathBuf,
    pub session_summary: Option<SessionSummary>,
}

#[derive(Debug)]
pub struct GatewayRunError {
    source: anyhow::Error,
    trace: RunTrace,
    trace_path: PathBuf,
    gateway_run_id: String,
    correlation_id: String,
    session_route: String,
}

impl GatewayRunError {
    pub fn trace(&self) -> &RunTrace {
        &self.trace
    }

    pub fn trace_path(&self) -> &PathBuf {
        &self.trace_path
    }

    pub fn into_parts(self) -> (anyhow::Error, RunTrace, PathBuf) {
        (self.source, self.trace, self.trace_path)
    }

    pub fn gateway_run_id(&self) -> &str {
        &self.gateway_run_id
    }

    pub fn correlation_id(&self) -> &str {
        &self.correlation_id
    }

    pub fn session_route(&self) -> &str {
        &self.session_route
    }
}

impl std::fmt::Display for GatewayRunError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.source)
    }
}

impl std::error::Error for GatewayRunError {}

#[derive(Debug, Default)]
struct GatewayMetricsState {
    completed_runs_total: u64,
    failed_runs_total: u64,
    capability_jobs_total: u64,
    audit_events_total: u64,
    auth_denials_total: u64,
    broadcast_lag_events_total: u64,
}

#[derive(Debug)]
struct GatewayReplayWindow {
    capacity: usize,
    dropped_events_total: u64,
    events: VecDeque<GatewayEventEnvelope>,
}

impl GatewayReplayWindow {
    fn new(capacity: usize) -> Self {
        Self {
            capacity: capacity.max(1),
            dropped_events_total: 0,
            events: VecDeque::new(),
        }
    }

    fn push(&mut self, envelope: GatewayEventEnvelope) {
        if self.events.len() == self.capacity {
            self.events.pop_front();
            self.dropped_events_total += 1;
        }
        self.events.push_back(envelope);
    }

    fn snapshot(&self, limit: usize) -> ReplayWindowResponse {
        let mut events = self
            .events
            .iter()
            .rev()
            .take(limit.max(1))
            .cloned()
            .collect::<Vec<_>>();
        events.reverse();
        ReplayWindowResponse {
            capacity: self.capacity,
            dropped_events_total: self.dropped_events_total,
            events,
        }
    }
}

#[derive(Debug)]
struct GatewayAuditLog {
    root: PathBuf,
    capacity: usize,
    events: Mutex<VecDeque<GatewayAuditEventDto>>,
}

impl GatewayAuditLog {
    fn new(root: PathBuf, capacity: usize) -> Self {
        Self {
            root,
            capacity: capacity.max(1),
            events: Mutex::new(VecDeque::new()),
        }
    }

    fn ready(&self) -> bool {
        fs::create_dir_all(&self.root).is_ok()
    }

    fn append(&self, event: GatewayAuditEventDto) {
        if fs::create_dir_all(&self.root).is_ok() {
            if let Ok(mut file) = OpenOptions::new()
                .create(true)
                .append(true)
                .open(self.root.join("events.jsonl"))
            {
                let _ = writeln!(
                    file,
                    "{}",
                    serde_json::to_string(&event).unwrap_or_default()
                );
            }
        }

        if let Ok(mut events) = self.events.lock() {
            if events.len() == self.capacity {
                events.pop_front();
            }
            events.push_back(event);
        }
    }

    fn recent(&self, limit: usize) -> Vec<GatewayAuditEventDto> {
        self.events
            .lock()
            .expect("audit events lock should not be poisoned")
            .iter()
            .rev()
            .take(limit.max(1))
            .cloned()
            .collect()
    }

    fn incident_events_for(&self, trace: &RunTrace) -> Vec<GatewayAuditEventDto> {
        let path = self.root.join("events.jsonl");
        let content = fs::read_to_string(path).unwrap_or_default();
        content
            .lines()
            .filter_map(|line| serde_json::from_str::<GatewayAuditEventDto>(line).ok())
            .filter(|event| {
                matches_identifier(
                    event.gateway_run_id.as_deref(),
                    trace.gateway_run_id.as_deref(),
                ) || matches_identifier(
                    event.correlation_id.as_deref(),
                    trace.correlation_id.as_deref(),
                ) || matches_identifier(event.session_id.as_deref(), trace.session_id.as_deref())
            })
            .collect()
    }

    fn save_incident_bundle(&self, bundle: &IncidentBundleDto) -> Result<PathBuf> {
        let dir = self.root.join("incidents");
        fs::create_dir_all(&dir)?;
        let path = dir.join(format!("{}.json", bundle.identifier));
        fs::write(&path, serde_json::to_string_pretty(bundle)?)?;
        Ok(path)
    }
}

#[derive(Clone)]
pub struct GatewayHandle {
    inner: Arc<GatewayState>,
}

struct GatewayState {
    runtime_handle: Handle,
    components: Mutex<GatewayRuntimeComponents>,
    reload_source: Option<GatewayReloadSource>,
    events: broadcast::Sender<GatewayEventEnvelope>,
    capability_jobs: Arc<Mutex<BTreeMap<String, CapabilityJobDto>>>,
    replay_window: Mutex<GatewayReplayWindow>,
    audit_log: Arc<GatewayAuditLog>,
    metrics: Mutex<GatewayMetricsState>,
}

impl GatewayState {
    fn snapshot_components(&self) -> GatewayRuntimeComponents {
        self.components
            .lock()
            .expect("gateway components lock should not be poisoned")
            .clone()
    }
}

impl GatewayHandle {
    pub fn new_local(runtime_handle: Handle, components: GatewayRuntimeComponents) -> Self {
        Self::new_local_with_reload_source(runtime_handle, components, None)
    }

    pub fn new_local_with_reload_source(
        runtime_handle: Handle,
        components: GatewayRuntimeComponents,
        reload_source: Option<GatewayReloadSource>,
    ) -> Self {
        let (events, _) = broadcast::channel(256);
        let replay_capacity = components.audit.event_replay_window.max(1);
        let audit_capacity = replay_capacity.max(256);
        let audit_root = components.audit_root.clone();
        Self {
            inner: Arc::new(GatewayState {
                runtime_handle,
                components: Mutex::new(components),
                reload_source,
                events,
                capability_jobs: Arc::new(Mutex::new(BTreeMap::new())),
                replay_window: Mutex::new(GatewayReplayWindow::new(replay_capacity)),
                audit_log: Arc::new(GatewayAuditLog::new(audit_root, audit_capacity)),
                metrics: Mutex::new(GatewayMetricsState::default()),
            }),
        }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<GatewayEventEnvelope> {
        self.inner.events.subscribe()
    }

    fn snapshot_components(&self) -> GatewayRuntimeComponents {
        self.inner.snapshot_components()
    }

    pub fn auth_mode(&self) -> String {
        operator_auth_mode(&self.snapshot_components().auth)
    }

    pub fn health(&self) -> HealthResponse {
        let components = self.snapshot_components();
        HealthResponse {
            status: "ok".to_owned(),
            active_profile: components.profiles.active_profile_name().to_owned(),
            session_count: components
                .session_store
                .list()
                .map(|sessions| sessions.len())
                .unwrap_or(0),
            transport: "http+sse".to_owned(),
            deployment_profile: components.deployment.profile.clone(),
            auth_mode: operator_auth_mode(&components.auth),
            event_replay_window: components.audit.event_replay_window,
        }
    }

    pub fn readiness(&self) -> ReadinessResponse {
        let components = self.snapshot_components();
        let session_store_ready = components.session_store.list().is_ok();
        let audit_ready = self.inner.audit_log.ready();
        let auth_ready = auth_state_ready(&components.deployment, &components.auth);
        let replay_window = self
            .inner
            .replay_window
            .lock()
            .expect("replay window lock should not be poisoned");
        ReadinessResponse {
            status: if session_store_ready && audit_ready && auth_ready {
                "ready".to_owned()
            } else {
                "degraded".to_owned()
            },
            transport: "http+sse".to_owned(),
            deployment_profile: components.deployment.profile.clone(),
            auth_mode: operator_auth_mode(&components.auth),
            session_store_ready,
            audit_ready,
            extension_count: components.extensions.len(),
            session_count: components
                .session_store
                .list()
                .map(|sessions| sessions.len())
                .unwrap_or(0),
            replay_events_buffered: replay_window.events.len(),
            event_replay_window: replay_window.capacity,
            slow_consumer_lag_threshold: components.observability.slow_consumer_lag_threshold,
        }
    }

    pub fn metrics(&self) -> MetricsResponse {
        let components = self.snapshot_components();
        let metrics = self
            .inner
            .metrics
            .lock()
            .expect("gateway metrics lock should not be poisoned");
        let replay_window = self
            .inner
            .replay_window
            .lock()
            .expect("replay window lock should not be poisoned");
        MetricsResponse {
            transport: "http+sse".to_owned(),
            deployment_profile: components.deployment.profile.clone(),
            auth_mode: operator_auth_mode(&components.auth),
            session_count: components
                .session_store
                .list()
                .map(|sessions| sessions.len())
                .unwrap_or(0),
            capability_job_count: self
                .inner
                .capability_jobs
                .lock()
                .expect("capability jobs lock should not be poisoned")
                .len(),
            completed_runs_total: metrics.completed_runs_total,
            failed_runs_total: metrics.failed_runs_total,
            capability_jobs_total: metrics.capability_jobs_total,
            audit_events_total: metrics.audit_events_total,
            auth_denials_total: metrics.auth_denials_total,
            broadcast_lag_events_total: metrics.broadcast_lag_events_total,
            replay_events_buffered: replay_window.events.len(),
            event_replay_window: replay_window.capacity,
        }
    }

    pub fn replay_window(&self, limit: usize) -> ReplayWindowResponse {
        self.inner
            .replay_window
            .lock()
            .expect("replay window lock should not be poisoned")
            .snapshot(limit)
    }

    pub fn audit_events(&self, limit: usize) -> Vec<GatewayAuditEventDto> {
        self.inner.audit_log.recent(limit)
    }

    pub fn incident_bundle(&self, identifier: &str) -> Result<(IncidentBundleDto, PathBuf)> {
        let components = self.snapshot_components();
        let trace = load_incident_trace(&components.runs_dir, identifier)?;
        let bundle = IncidentBundleDto {
            identifier: identifier.to_owned(),
            generated_at: Utc::now(),
            deployment_profile: components.deployment.profile.clone(),
            auth_mode: operator_auth_mode(&components.auth),
            redaction_policy: if components.audit.redact_inputs {
                "inputs_redacted".to_owned()
            } else {
                "full_inputs".to_owned()
            },
            audit_events: self.inner.audit_log.incident_events_for(&trace),
            metrics: self.metrics(),
            trace,
        };
        let path = self.inner.audit_log.save_incident_bundle(&bundle)?;
        Ok((bundle, path))
    }

    pub fn list_extensions(&self) -> Vec<ExtensionStatus> {
        self.snapshot_components().extensions
    }

    pub fn extension_policies(&self) -> PolicyConfig {
        self.snapshot_components().policies
    }

    pub fn validate_extensions(&self) -> Result<ExtensionValidationReport> {
        let Some(source) = self.inner.reload_source.clone() else {
            let components = self.snapshot_components();
            return Ok(ExtensionValidationReport {
                policies: components.policies,
                extensions: components.extensions,
                issues: Vec::new(),
            });
        };

        let loaded = load_mosaic_config(&LoadConfigOptions {
            cwd: source.workspace_root.clone(),
            user_config_path: source.user_config_path.clone(),
            workspace_config_path: Some(source.workspace_config_path.clone()),
            overrides: Default::default(),
        })?;

        Ok(validate_extension_set(
            &loaded.config,
            source.app_config.as_ref(),
            &source.workspace_root,
        ))
    }

    pub fn reload_extensions(&self) -> Result<GatewayExtensionReloadResult> {
        let source = self.inner.reload_source.clone().ok_or_else(|| {
            anyhow!("gateway was not initialized with an extension reload source")
        })?;
        let loaded = load_mosaic_config(&LoadConfigOptions {
            cwd: source.workspace_root.clone(),
            user_config_path: source.user_config_path.clone(),
            workspace_config_path: Some(source.workspace_config_path.clone()),
            overrides: Default::default(),
        })?;

        if !loaded.config.policies.hot_reload_enabled {
            bail!("extension hot reload is disabled by policy");
        }

        let current = self.snapshot_components();
        let extension_set = match load_extension_set(
            &loaded.config,
            source.app_config.as_ref(),
            &source.workspace_root,
            current.cron_store.clone(),
        ) {
            Ok(extension_set) => extension_set,
            Err(err) => {
                self.emit(GatewayEventEnvelope {
                    gateway_run_id: "extensions.reload".to_owned(),
                    correlation_id: "extensions.reload".to_owned(),
                    session_id: None,
                    session_route: "gateway.local/extensions".to_owned(),
                    emitted_at: Utc::now(),
                    event: GatewayEvent::ExtensionReloadFailed {
                        error: err.to_string(),
                    },
                });
                return Err(err);
            }
        };

        let profiles = Arc::new(ProviderProfileRegistry::from_config(&loaded.config)?);
        let updated = GatewayRuntimeComponents {
            profiles,
            provider_override: current.provider_override.clone(),
            session_store: current.session_store.clone(),
            memory_store: current.memory_store.clone(),
            memory_policy: current.memory_policy.clone(),
            tools: Arc::new(extension_set.tools),
            skills: Arc::new(extension_set.skills),
            workflows: Arc::new(extension_set.workflows),
            node_store: current.node_store.clone(),
            mcp_manager: extension_set.mcp_manager,
            cron_store: current.cron_store.clone(),
            workspace_root: current.workspace_root.clone(),
            runs_dir: current.runs_dir.clone(),
            audit_root: current.audit_root.clone(),
            extensions: extension_set.extensions.clone(),
            policies: extension_set.policies.clone(),
            deployment: current.deployment.clone(),
            auth: current.auth.clone(),
            audit: current.audit.clone(),
            observability: current.observability.clone(),
        };

        *self
            .inner
            .components
            .lock()
            .expect("gateway components lock should not be poisoned") = updated;

        self.emit(GatewayEventEnvelope {
            gateway_run_id: "extensions.reload".to_owned(),
            correlation_id: "extensions.reload".to_owned(),
            session_id: None,
            session_route: "gateway.local/extensions".to_owned(),
            emitted_at: Utc::now(),
            event: GatewayEvent::ExtensionsReloaded {
                extensions: extension_set
                    .extensions
                    .iter()
                    .map(extension_status_dto)
                    .collect(),
                policies: extension_policy_dto(&extension_set.policies),
            },
        });

        Ok(GatewayExtensionReloadResult {
            extensions: extension_set.extensions,
            policies: extension_set.policies,
        })
    }

    pub fn list_sessions(&self) -> Result<Vec<SessionSummary>> {
        self.snapshot_components().session_store.list()
    }

    pub fn list_adapter_statuses(&self) -> Vec<AdapterStatusDto> {
        let auth = self.snapshot_components().auth;
        vec![
            webchat_adapter_status(&auth),
            telegram_adapter_status(&auth),
        ]
    }

    pub fn load_session(&self, id: &str) -> Result<Option<SessionRecord>> {
        self.snapshot_components().session_store.load(id)
    }

    pub fn list_capability_jobs(&self) -> Vec<CapabilityJobDto> {
        let mut jobs = self
            .inner
            .capability_jobs
            .lock()
            .expect("capability jobs lock should not be poisoned")
            .values()
            .cloned()
            .collect::<Vec<_>>();
        jobs.sort_by(|left, right| right.started_at.cmp(&left.started_at));
        jobs
    }

    pub fn list_nodes(&self) -> Result<Vec<NodeRegistration>> {
        self.snapshot_components().node_store.list_nodes()
    }

    pub fn node_capabilities(&self, node_id: &str) -> Result<Vec<NodeCapabilityDeclaration>> {
        self.snapshot_components()
            .node_store
            .node_capabilities(node_id)
    }

    pub fn node_affinity(&self, session_id: Option<&str>) -> Result<Option<String>> {
        Ok(self
            .snapshot_components()
            .node_store
            .affinity_for_session(session_id)?
            .map(|record| record.node_id))
    }

    pub fn attach_node(&self, node_id: &str, session_id: Option<&str>) -> Result<()> {
        let components = self.snapshot_components();
        if components.node_store.load_node(node_id)?.is_none() {
            bail!("node not found: {}", node_id);
        }

        match session_id {
            Some(session_id) => components.node_store.attach_session(session_id, node_id),
            None => components.node_store.attach_default(node_id),
        }
    }

    pub fn list_cron_registrations(&self) -> Result<Vec<CronRegistration>> {
        self.snapshot_components().cron_store.list()
    }

    pub fn register_cron(&self, request: CronRegistrationRequest) -> Result<CronRegistration> {
        let registration = CronRegistration {
            id: request.id,
            schedule: request.schedule,
            input: request.input,
            session_id: request.session_id,
            profile: request.profile,
            skill: request.skill,
            workflow: request.workflow,
            created_at: Utc::now(),
            last_triggered_at: None,
        };
        self.snapshot_components().cron_store.save(&registration)?;
        self.emit(GatewayEventEnvelope {
            gateway_run_id: format!("cron-{}", registration.id),
            correlation_id: format!("cron-{}", registration.id),
            session_id: registration.session_id.clone(),
            session_route: registration
                .session_id
                .as_deref()
                .map(session_route_for_id)
                .unwrap_or_else(|| "gateway.local/cron".to_owned()),
            emitted_at: Utc::now(),
            event: GatewayEvent::CronUpdated {
                registration: cron_registration_dto(&registration),
            },
        });
        Ok(registration)
    }

    pub fn active_profile_name(&self) -> String {
        self.snapshot_components()
            .profiles
            .active_profile_name()
            .to_owned()
    }

    pub async fn run_exec_job(&self, request: ExecJobRequest) -> Result<CapabilityJobDto> {
        execute_capability_tool(
            self.inner.clone(),
            "exec_command",
            request.session_id,
            serde_json::json!({
                "command": request.command,
                "args": request.args,
                "cwd": request.cwd,
            }),
        )
        .await
    }

    pub async fn run_webhook_job(&self, request: WebhookJobRequest) -> Result<CapabilityJobDto> {
        execute_capability_tool(
            self.inner.clone(),
            "webhook_call",
            request.session_id,
            serde_json::json!({
                "url": request.url,
                "method": request.method,
                "body": request.body,
                "headers": request.headers,
            }),
        )
        .await
    }

    pub fn submit_telegram_update(&self, update: TelegramUpdate) -> Result<GatewaySubmittedRun> {
        let normalized = normalize_telegram_update(update)?;
        let ingress = normalized.ingress();
        self.submit_run(RunSubmission {
            system: None,
            input: normalized.input,
            skill: None,
            workflow: None,
            session_id: Some(normalized.session_id),
            profile: None,
            ingress: Some(ingress),
        })
    }

    pub async fn trigger_cron(&self, id: &str) -> Result<GatewayRunResult> {
        let mut registration = self
            .snapshot_components()
            .cron_store
            .load(id)?
            .ok_or_else(|| anyhow!("cron not found: {id}"))?;
        registration.mark_triggered();
        self.snapshot_components().cron_store.save(&registration)?;
        self.emit(GatewayEventEnvelope {
            gateway_run_id: format!("cron-trigger-{}", registration.id),
            correlation_id: format!("cron-trigger-{}", registration.id),
            session_id: registration.session_id.clone(),
            session_route: registration
                .session_id
                .as_deref()
                .map(session_route_for_id)
                .unwrap_or_else(|| "gateway.local/cron".to_owned()),
            emitted_at: Utc::now(),
            event: GatewayEvent::CronUpdated {
                registration: cron_registration_dto(&registration),
            },
        });

        self.submit_run(RunSubmission {
            system: None,
            input: registration.input.clone(),
            skill: registration.skill.clone(),
            workflow: registration.workflow.clone(),
            session_id: registration.session_id.clone(),
            profile: registration.profile.clone(),
            ingress: Some(IngressTrace {
                kind: "cron".to_owned(),
                channel: Some("cron".to_owned()),
                source: Some(registration.id.clone()),
                remote_addr: None,
                display_name: None,
                actor_id: None,
                thread_id: None,
                thread_title: None,
                reply_target: None,
                gateway_url: None,
            }),
        })?
        .wait()
        .await
        .map_err(|err| anyhow!(err.to_string()))
    }

    pub fn submit_command(&self, command: GatewayCommand) -> Result<GatewaySubmittedRun> {
        match command {
            GatewayCommand::SubmitRun(request) => self.submit_run(request),
        }
    }

    pub fn submit_run(&self, request: GatewayRunRequest) -> Result<GatewaySubmittedRun> {
        let gateway_run_id = Uuid::new_v4().to_string();
        let correlation_id = Uuid::new_v4().to_string();
        let components = self.snapshot_components();
        let session_route =
            self.resolve_session_route(request.session_id.as_deref(), request.ingress.as_ref())?;
        let resolved_profile = request
            .profile
            .clone()
            .unwrap_or_else(|| components.profiles.active_profile_name().to_owned());
        let meta = GatewayRunMeta {
            gateway_run_id: gateway_run_id.clone(),
            correlation_id: correlation_id.clone(),
            session_id: request.session_id.clone(),
            session_route: session_route.clone(),
            ingress: request.ingress.clone(),
        };

        self.record_audit_event(
            "run.submitted",
            "accepted",
            redact_audit_input(&request.input, components.audit.redact_inputs),
            request.session_id.clone(),
            Some(gateway_run_id.clone()),
            Some(correlation_id.clone()),
            request.ingress.as_ref(),
            Some(session_route.clone()),
            components.audit.redact_inputs,
        );
        self.emit(meta.envelope(GatewayEvent::RunSubmitted {
            input: request.input.clone(),
            profile: resolved_profile,
            ingress: request.ingress.clone(),
        }));

        let state = self.inner.clone();
        let join = self.inner.runtime_handle.spawn(async move {
            let event_sink: SharedRunEventSink = Arc::new(GatewayRunEventSink {
                state: state.clone(),
                meta: meta.clone(),
            });
            let components = state
                .components
                .lock()
                .expect("gateway components lock should not be poisoned")
                .clone();
            let runtime = AgentRuntime::new(components.runtime_context(event_sink));
            let run_request = RunRequest {
                system: request.system,
                input: request.input,
                skill: request.skill,
                workflow: request.workflow,
                session_id: meta.session_id.clone(),
                profile: request.profile,
                ingress: meta.ingress.clone(),
            };

            finalize_run(state, meta, runtime.run(run_request).await)
        });

        Ok(GatewaySubmittedRun {
            gateway_run_id,
            correlation_id,
            session_id: request.session_id,
            session_route,
            join,
        })
    }

    fn emit(&self, envelope: GatewayEventEnvelope) {
        broadcast_envelope(self.inner.as_ref(), envelope);
    }

    fn record_audit_event(
        &self,
        kind: &str,
        outcome: &str,
        summary: String,
        session_id: Option<String>,
        gateway_run_id: Option<String>,
        correlation_id: Option<String>,
        ingress: Option<&IngressTrace>,
        target: Option<String>,
        redacted: bool,
    ) {
        record_audit_event(
            self.inner.as_ref(),
            kind,
            outcome,
            summary,
            session_id,
            gateway_run_id,
            correlation_id,
            ingress,
            target,
            redacted,
        );
    }

    fn resolve_session_route(
        &self,
        session_id: Option<&str>,
        ingress: Option<&IngressTrace>,
    ) -> Result<String> {
        match session_id {
            Some(id) => {
                if let Some(session) = self.load_session(id)? {
                    let current = if session.gateway.route.is_empty() {
                        session_route_for_id(id)
                    } else {
                        session.gateway.route
                    };
                    if current != session_route_for_id(id) || ingress.is_none() {
                        return Ok(current);
                    }
                }

                Ok(ingress_route(Some(id), ingress).unwrap_or_else(|| session_route_for_id(id)))
            }
            None => Ok(ingress_route(None, ingress)
                .unwrap_or_else(|| "gateway.local/ephemeral".to_owned())),
        }
    }
}

pub struct GatewaySubmittedRun {
    gateway_run_id: String,
    correlation_id: String,
    session_id: Option<String>,
    session_route: String,
    join: tokio::task::JoinHandle<Result<GatewayRunResult, GatewayRunError>>,
}

impl GatewaySubmittedRun {
    pub fn gateway_run_id(&self) -> &str {
        &self.gateway_run_id
    }

    pub fn correlation_id(&self) -> &str {
        &self.correlation_id
    }

    pub fn session_id(&self) -> Option<&str> {
        self.session_id.as_deref()
    }

    pub fn session_route(&self) -> &str {
        &self.session_route
    }

    pub async fn wait(self) -> Result<GatewayRunResult, GatewayRunError> {
        match self.join.await {
            Ok(result) => result,
            Err(err) => Err(GatewayRunError {
                source: anyhow!("gateway task join failure: {err}"),
                trace: RunTrace::new("<join-failure>".to_owned()),
                trace_path: PathBuf::new(),
                gateway_run_id: self.gateway_run_id,
                correlation_id: self.correlation_id,
                session_route: self.session_route,
            }),
        }
    }
}

#[derive(Debug, Clone)]
struct GatewayRunMeta {
    gateway_run_id: String,
    correlation_id: String,
    session_id: Option<String>,
    session_route: String,
    ingress: Option<IngressTrace>,
}

impl GatewayRunMeta {
    fn envelope(&self, event: GatewayEvent) -> GatewayEventEnvelope {
        GatewayEventEnvelope {
            gateway_run_id: self.gateway_run_id.clone(),
            correlation_id: self.correlation_id.clone(),
            session_id: self.session_id.clone(),
            session_route: self.session_route.clone(),
            emitted_at: Utc::now(),
            event,
        }
    }
}

struct GatewayRunEventSink {
    state: Arc<GatewayState>,
    meta: GatewayRunMeta,
}

impl RunEventSink for GatewayRunEventSink {
    fn emit(&self, event: RunEvent) {
        if let Some(job) =
            update_runtime_capability_job(&self.state.capability_jobs, &self.meta, &event)
        {
            if job.status == "queued" {
                increment_metric(self.state.as_ref(), |metrics| {
                    metrics.capability_jobs_total += 1
                });
            }
            maybe_record_capability_job_audit(
                self.state.as_ref(),
                &job,
                self.meta.ingress.as_ref(),
            );
            broadcast_envelope(
                self.state.as_ref(),
                self.meta
                    .envelope(GatewayEvent::CapabilityJobUpdated { job }),
            );
        }
        broadcast_envelope(
            self.state.as_ref(),
            self.meta.envelope(GatewayEvent::Runtime(event)),
        );
    }
}

fn update_runtime_capability_job(
    jobs: &Arc<Mutex<BTreeMap<String, CapabilityJobDto>>>,
    meta: &GatewayRunMeta,
    event: &RunEvent,
) -> Option<CapabilityJobDto> {
    let mut jobs = jobs
        .lock()
        .expect("capability jobs lock should not be poisoned");

    match event {
        RunEvent::CapabilityJobQueued {
            job_id,
            name,
            kind,
            risk,
            permission_scopes,
        } => {
            let job = CapabilityJobDto {
                id: job_id.clone(),
                name: name.clone(),
                kind: kind.clone(),
                risk: risk.clone(),
                permission_scopes: permission_scopes.clone(),
                status: "queued".to_owned(),
                summary: None,
                target: None,
                session_id: meta.session_id.clone(),
                gateway_run_id: Some(meta.gateway_run_id.clone()),
                correlation_id: Some(meta.correlation_id.clone()),
                started_at: Utc::now(),
                finished_at: None,
                error: None,
            };
            jobs.insert(job_id.clone(), job.clone());
            Some(job)
        }
        RunEvent::CapabilityJobStarted { job_id, name } => {
            let job = jobs
                .entry(job_id.clone())
                .or_insert_with(|| CapabilityJobDto {
                    id: job_id.clone(),
                    name: name.clone(),
                    kind: "unknown".to_owned(),
                    risk: "unknown".to_owned(),
                    permission_scopes: Vec::new(),
                    status: "running".to_owned(),
                    summary: None,
                    target: None,
                    session_id: meta.session_id.clone(),
                    gateway_run_id: Some(meta.gateway_run_id.clone()),
                    correlation_id: Some(meta.correlation_id.clone()),
                    started_at: Utc::now(),
                    finished_at: None,
                    error: None,
                });
            job.status = "running".to_owned();
            job.error = None;
            Some(job.clone())
        }
        RunEvent::CapabilityJobRetried {
            job_id,
            name,
            attempt,
            error,
        } => {
            let job = jobs
                .entry(job_id.clone())
                .or_insert_with(|| CapabilityJobDto {
                    id: job_id.clone(),
                    name: name.clone(),
                    kind: "unknown".to_owned(),
                    risk: "unknown".to_owned(),
                    permission_scopes: Vec::new(),
                    status: "retrying".to_owned(),
                    summary: None,
                    target: None,
                    session_id: meta.session_id.clone(),
                    gateway_run_id: Some(meta.gateway_run_id.clone()),
                    correlation_id: Some(meta.correlation_id.clone()),
                    started_at: Utc::now(),
                    finished_at: None,
                    error: None,
                });
            job.status = "retrying".to_owned();
            job.summary = Some(format!("retry attempt {}", attempt));
            job.error = Some(error.clone());
            Some(job.clone())
        }
        RunEvent::CapabilityJobFinished {
            job_id,
            name,
            status,
            summary,
        } => {
            let job = jobs
                .entry(job_id.clone())
                .or_insert_with(|| CapabilityJobDto {
                    id: job_id.clone(),
                    name: name.clone(),
                    kind: "unknown".to_owned(),
                    risk: "unknown".to_owned(),
                    permission_scopes: Vec::new(),
                    status: status.clone(),
                    summary: None,
                    target: None,
                    session_id: meta.session_id.clone(),
                    gateway_run_id: Some(meta.gateway_run_id.clone()),
                    correlation_id: Some(meta.correlation_id.clone()),
                    started_at: Utc::now(),
                    finished_at: None,
                    error: None,
                });
            job.status = status.clone();
            job.summary = Some(summary.clone());
            job.finished_at = Some(Utc::now());
            job.error = None;
            Some(job.clone())
        }
        RunEvent::CapabilityJobFailed {
            job_id,
            name,
            error,
        } => {
            let job = jobs
                .entry(job_id.clone())
                .or_insert_with(|| CapabilityJobDto {
                    id: job_id.clone(),
                    name: name.clone(),
                    kind: "unknown".to_owned(),
                    risk: "unknown".to_owned(),
                    permission_scopes: Vec::new(),
                    status: "failed".to_owned(),
                    summary: None,
                    target: None,
                    session_id: meta.session_id.clone(),
                    gateway_run_id: Some(meta.gateway_run_id.clone()),
                    correlation_id: Some(meta.correlation_id.clone()),
                    started_at: Utc::now(),
                    finished_at: None,
                    error: None,
                });
            job.status = "failed".to_owned();
            job.error = Some(error.clone());
            job.finished_at = Some(Utc::now());
            Some(job.clone())
        }
        _ => None,
    }
}

async fn execute_capability_tool(
    state: Arc<GatewayState>,
    tool_name: &str,
    session_id: Option<String>,
    input: serde_json::Value,
) -> Result<CapabilityJobDto> {
    let tool = state
        .snapshot_components()
        .tools
        .get(tool_name)
        .ok_or_else(|| anyhow!("tool not found: {}", tool_name))?;
    let metadata = tool.metadata().clone();
    let mut job = CapabilityJobDto {
        id: Uuid::new_v4().to_string(),
        name: metadata.name.clone(),
        kind: metadata.capability.kind.label().to_owned(),
        risk: metadata.capability.risk.label().to_owned(),
        permission_scopes: metadata
            .capability
            .permission_scopes
            .iter()
            .map(|scope| scope.label().to_owned())
            .collect(),
        status: "queued".to_owned(),
        summary: None,
        target: None,
        session_id,
        gateway_run_id: None,
        correlation_id: None,
        started_at: Utc::now(),
        finished_at: None,
        error: None,
    };
    job = store_and_broadcast_capability_job(&state, job);

    if !metadata.capability.authorized {
        job.status = "failed".to_owned();
        job.error = Some(format!(
            "tool '{}' is not authorized for execution",
            metadata.name
        ));
        job.finished_at = Some(Utc::now());
        store_and_broadcast_capability_job(&state, job.clone());
        bail!(
            job.error
                .clone()
                .unwrap_or_else(|| "capability execution failed".to_owned())
        );
    }
    if !metadata.capability.healthy {
        job.status = "failed".to_owned();
        job.error = Some(format!("tool '{}' is not healthy", metadata.name));
        job.finished_at = Some(Utc::now());
        store_and_broadcast_capability_job(&state, job.clone());
        bail!(
            job.error
                .clone()
                .unwrap_or_else(|| "capability execution failed".to_owned())
        );
    }

    job.status = "running".to_owned();
    job = store_and_broadcast_capability_job(&state, job);

    let attempts = usize::from(metadata.capability.execution.retry_limit) + 1;
    let timeout = Duration::from_millis(metadata.capability.execution.timeout_ms.max(1));
    for attempt in 1..=attempts {
        match tokio::time::timeout(timeout, tool.call(input.clone())).await {
            Ok(Ok(result)) if !result.is_error => {
                job.status = "success".to_owned();
                job.summary = result
                    .audit
                    .as_ref()
                    .map(|audit| audit.side_effect_summary.clone())
                    .or_else(|| Some(truncate_preview(&result.content, 180)));
                job.target = result.audit.as_ref().and_then(|audit| audit.target.clone());
                job.finished_at = Some(Utc::now());
                job.error = None;
                return Ok(store_and_broadcast_capability_job(&state, job));
            }
            Ok(Ok(result)) => {
                let error = result.content.clone();
                if attempt < attempts {
                    job.status = "retrying".to_owned();
                    job.summary = Some(format!("retry attempt {}", attempt));
                    job.error = Some(error);
                    job = store_and_broadcast_capability_job(&state, job);
                    continue;
                }
                job.status = "failed".to_owned();
                job.summary = result
                    .audit
                    .as_ref()
                    .map(|audit| audit.side_effect_summary.clone())
                    .or_else(|| Some(truncate_preview(&result.content, 180)));
                job.target = result.audit.as_ref().and_then(|audit| audit.target.clone());
                job.error = Some(error.clone());
                job.finished_at = Some(Utc::now());
                store_and_broadcast_capability_job(&state, job.clone());
                bail!(error);
            }
            Ok(Err(err)) => {
                if attempt < attempts {
                    job.status = "retrying".to_owned();
                    job.summary = Some(format!("retry attempt {}", attempt));
                    job.error = Some(err.to_string());
                    job = store_and_broadcast_capability_job(&state, job);
                    continue;
                }
                job.status = "failed".to_owned();
                job.error = Some(err.to_string());
                job.finished_at = Some(Utc::now());
                store_and_broadcast_capability_job(&state, job.clone());
                return Err(err);
            }
            Err(_) => {
                let error = format!(
                    "tool '{}' timed out after {}ms",
                    metadata.name,
                    metadata.capability.execution.timeout_ms.max(1)
                );
                if attempt < attempts {
                    job.status = "retrying".to_owned();
                    job.summary = Some(format!("retry attempt {}", attempt));
                    job.error = Some(error.clone());
                    job = store_and_broadcast_capability_job(&state, job);
                    continue;
                }
                job.status = "failed".to_owned();
                job.error = Some(error.clone());
                job.finished_at = Some(Utc::now());
                store_and_broadcast_capability_job(&state, job.clone());
                bail!(error);
            }
        }
    }

    unreachable!("capability execution should return success or failure")
}

fn store_and_broadcast_capability_job(
    state: &GatewayState,
    job: CapabilityJobDto,
) -> CapabilityJobDto {
    state
        .capability_jobs
        .lock()
        .expect("capability jobs lock should not be poisoned")
        .insert(job.id.clone(), job.clone());

    if job.status == "queued" {
        increment_metric(state, |metrics| metrics.capability_jobs_total += 1);
    }
    maybe_record_capability_job_audit(state, &job, None);
    broadcast_envelope(
        state,
        GatewayEventEnvelope {
            gateway_run_id: job
                .gateway_run_id
                .clone()
                .unwrap_or_else(|| format!("capability-{}", job.id)),
            correlation_id: job
                .correlation_id
                .clone()
                .unwrap_or_else(|| format!("capability-{}", job.id)),
            session_id: job.session_id.clone(),
            session_route: job
                .session_id
                .as_deref()
                .map(session_route_for_id)
                .unwrap_or_else(|| "gateway.local/capabilities".to_owned()),
            emitted_at: Utc::now(),
            event: GatewayEvent::CapabilityJobUpdated { job: job.clone() },
        },
    );

    job
}

fn finalize_run(
    state: Arc<GatewayState>,
    meta: GatewayRunMeta,
    outcome: std::result::Result<RunResult, RunError>,
) -> Result<GatewayRunResult, GatewayRunError> {
    match outcome {
        Ok(result) => finalize_success(state, meta, result),
        Err(err) => finalize_failure(state, meta, err),
    }
}

fn finalize_success(
    state: Arc<GatewayState>,
    meta: GatewayRunMeta,
    result: RunResult,
) -> Result<GatewayRunResult, GatewayRunError> {
    let components = state.snapshot_components();
    let mut trace = result.trace;
    trace.bind_gateway_context(
        meta.gateway_run_id.clone(),
        meta.correlation_id.clone(),
        meta.session_route.clone(),
    );
    trace.bind_governance(GovernanceTrace {
        deployment_profile: components.deployment.profile.clone(),
        workspace_name: components.deployment.workspace_name.clone(),
        auth_mode: operator_auth_mode(&components.auth),
        audit_retention_days: components.audit.retention_days,
        event_replay_window: components.audit.event_replay_window,
        redact_inputs: components.audit.redact_inputs,
    });
    let session_summary = update_gateway_session_metadata(&state, &meta);
    let trace_path = trace
        .save_to_dir(&components.runs_dir)
        .map_err(|err| GatewayRunError {
            source: err,
            trace: trace.clone(),
            trace_path: PathBuf::new(),
            gateway_run_id: meta.gateway_run_id.clone(),
            correlation_id: meta.correlation_id.clone(),
            session_route: meta.session_route.clone(),
        })?;

    increment_metric(state.as_ref(), |metrics| metrics.completed_runs_total += 1);
    let output_preview = truncate_preview(&result.output, 120);
    info!(
        gateway_run_id = %meta.gateway_run_id,
        correlation_id = %meta.correlation_id,
        session_route = %meta.session_route,
        trace_run_id = %trace.run_id,
        "gateway finalized run successfully"
    );
    record_audit_event(
        state.as_ref(),
        "run.completed",
        "success",
        output_preview.clone(),
        meta.session_id.clone(),
        Some(meta.gateway_run_id.clone()),
        Some(meta.correlation_id.clone()),
        meta.ingress.as_ref(),
        Some(trace_path.display().to_string()),
        false,
    );
    broadcast_envelope(
        state.as_ref(),
        meta.envelope(GatewayEvent::RunCompleted { output_preview }),
    );
    if let Some(summary) = session_summary.clone() {
        broadcast_envelope(
            state.as_ref(),
            meta.envelope(GatewayEvent::SessionUpdated {
                summary: session_summary_dto(&summary),
            }),
        );
    }

    Ok(GatewayRunResult {
        gateway_run_id: meta.gateway_run_id,
        correlation_id: meta.correlation_id,
        session_route: meta.session_route,
        output: result.output,
        trace,
        trace_path,
        session_summary,
    })
}

fn finalize_failure(
    state: Arc<GatewayState>,
    meta: GatewayRunMeta,
    err: RunError,
) -> Result<GatewayRunResult, GatewayRunError> {
    let components = state.snapshot_components();
    let (source, mut trace) = err.into_parts();
    trace.bind_gateway_context(
        meta.gateway_run_id.clone(),
        meta.correlation_id.clone(),
        meta.session_route.clone(),
    );
    trace.bind_governance(GovernanceTrace {
        deployment_profile: components.deployment.profile.clone(),
        workspace_name: components.deployment.workspace_name.clone(),
        auth_mode: operator_auth_mode(&components.auth),
        audit_retention_days: components.audit.retention_days,
        event_replay_window: components.audit.event_replay_window,
        redact_inputs: components.audit.redact_inputs,
    });
    let session_summary = update_gateway_session_metadata(&state, &meta);
    let trace_path = trace
        .save_to_dir(&components.runs_dir)
        .map_err(|save_err| GatewayRunError {
            source: save_err,
            trace: trace.clone(),
            trace_path: PathBuf::new(),
            gateway_run_id: meta.gateway_run_id.clone(),
            correlation_id: meta.correlation_id.clone(),
            session_route: meta.session_route.clone(),
        })?;

    increment_metric(state.as_ref(), |metrics| metrics.failed_runs_total += 1);
    let public_error = public_error_message(&source);
    warn!(
        gateway_run_id = %meta.gateway_run_id,
        correlation_id = %meta.correlation_id,
        session_route = %meta.session_route,
        trace_run_id = %trace.run_id,
        error = %public_error,
        "gateway finalized run with failure"
    );
    record_audit_event(
        state.as_ref(),
        "run.failed",
        "failed",
        public_error.clone(),
        meta.session_id.clone(),
        Some(meta.gateway_run_id.clone()),
        Some(meta.correlation_id.clone()),
        meta.ingress.as_ref(),
        Some(trace_path.display().to_string()),
        false,
    );
    broadcast_envelope(
        state.as_ref(),
        meta.envelope(GatewayEvent::RunFailed {
            error: public_error.clone(),
        }),
    );
    if let Some(summary) = session_summary {
        broadcast_envelope(
            state.as_ref(),
            meta.envelope(GatewayEvent::SessionUpdated {
                summary: session_summary_dto(&summary),
            }),
        );
    }

    Err(GatewayRunError {
        source,
        trace,
        trace_path,
        gateway_run_id: meta.gateway_run_id,
        correlation_id: meta.correlation_id,
        session_route: meta.session_route,
    })
}

fn update_gateway_session_metadata(
    state: &GatewayState,
    meta: &GatewayRunMeta,
) -> Option<SessionSummary> {
    let session_id = meta.session_id.as_deref()?;
    let components = state.snapshot_components();
    let mut session = components.session_store.load(session_id).ok()??;
    session.set_gateway_binding(
        meta.session_route.clone(),
        meta.gateway_run_id.clone(),
        meta.correlation_id.clone(),
    );
    components.session_store.save(&session).ok()?;
    Some(session.summary())
}

#[derive(Debug, Deserialize, Default)]
struct LimitQuery {
    limit: Option<usize>,
}

enum SecretStatus {
    Disabled,
    Ready(String),
    MissingEnv(String),
}

fn increment_metric(state: &GatewayState, update: impl FnOnce(&mut GatewayMetricsState)) {
    let mut metrics = state
        .metrics
        .lock()
        .expect("gateway metrics lock should not be poisoned");
    update(&mut metrics);
}

fn broadcast_envelope(state: &GatewayState, envelope: GatewayEventEnvelope) {
    state
        .replay_window
        .lock()
        .expect("replay window lock should not be poisoned")
        .push(envelope.clone());
    let _ = state.events.send(envelope);
}

fn configured_secret_env_name(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}

fn secret_status(env_name: Option<&str>) -> SecretStatus {
    match configured_secret_env_name(env_name) {
        Some(name) => match env::var(name) {
            Ok(value) => SecretStatus::Ready(value),
            Err(_) => SecretStatus::MissingEnv(name.to_owned()),
        },
        None => SecretStatus::Disabled,
    }
}

fn operator_auth_mode(auth: &AuthConfig) -> String {
    match secret_status(auth.operator_token_env.as_deref()) {
        SecretStatus::Disabled => "disabled".to_owned(),
        SecretStatus::Ready(_) | SecretStatus::MissingEnv(_) => "required".to_owned(),
    }
}

fn auth_state_ready(deployment: &DeploymentConfig, auth: &AuthConfig) -> bool {
    match secret_status(auth.operator_token_env.as_deref()) {
        SecretStatus::Disabled => deployment.profile != "production",
        SecretStatus::Ready(_) => true,
        SecretStatus::MissingEnv(_) => false,
    }
}

fn header_token<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    headers.get(name).and_then(|value| value.to_str().ok())
}

fn bearer_token(headers: &HeaderMap) -> Option<&str> {
    header_token(headers, "authorization")?.strip_prefix("Bearer ")
}

fn ingress_actor(ingress: Option<&IngressTrace>) -> Option<String> {
    ingress.and_then(|ingress| {
        ingress
            .display_name
            .clone()
            .or_else(|| ingress.actor_id.clone())
            .or_else(|| ingress.source.clone())
    })
}

fn redact_audit_input(input: &str, redact: bool) -> String {
    if redact {
        "<redacted>".to_owned()
    } else {
        truncate_preview(input, 160)
    }
}

fn record_audit_event(
    state: &GatewayState,
    kind: &str,
    outcome: &str,
    summary: String,
    session_id: Option<String>,
    gateway_run_id: Option<String>,
    correlation_id: Option<String>,
    ingress: Option<&IngressTrace>,
    target: Option<String>,
    redacted: bool,
) {
    let event = GatewayAuditEventDto {
        id: Uuid::new_v4().to_string(),
        kind: kind.to_owned(),
        outcome: outcome.to_owned(),
        summary,
        actor: ingress_actor(ingress),
        session_id,
        gateway_run_id,
        correlation_id,
        channel: ingress.and_then(|ingress| ingress.channel.clone()),
        target,
        emitted_at: Utc::now(),
        redacted,
    };
    state.audit_log.append(event);
    increment_metric(state, |metrics| metrics.audit_events_total += 1);
}

fn maybe_record_capability_job_audit(
    state: &GatewayState,
    job: &CapabilityJobDto,
    ingress: Option<&IngressTrace>,
) {
    if matches!(job.status.as_str(), "queued" | "success" | "failed") {
        record_audit_event(
            state,
            "capability.job",
            &job.status,
            job.summary.clone().unwrap_or_else(|| job.name.clone()),
            job.session_id.clone(),
            job.gateway_run_id.clone(),
            job.correlation_id.clone(),
            ingress,
            job.target.clone(),
            false,
        );
    }
}

fn record_auth_denial(gateway: &GatewayHandle, surface: &str) {
    increment_metric(gateway.inner.as_ref(), |metrics| {
        metrics.auth_denials_total += 1
    });
    gateway.record_audit_event(
        "auth.denied",
        "denied",
        format!("authorization denied for {surface}"),
        None,
        None,
        None,
        None,
        Some(surface.to_owned()),
        false,
    );
}

fn authorize_control_request(
    gateway: &GatewayHandle,
    headers: &HeaderMap,
    surface: &str,
) -> Result<(), (StatusCode, Json<ErrorResponse>)> {
    let components = gateway.snapshot_components();
    match secret_status(components.auth.operator_token_env.as_deref()) {
        SecretStatus::Disabled => Ok(()),
        SecretStatus::Ready(secret) => {
            if bearer_token(headers) == Some(secret.as_str()) {
                Ok(())
            } else {
                record_auth_denial(gateway, surface);
                Err((
                    StatusCode::UNAUTHORIZED,
                    Json(ErrorResponse {
                        error: "operator authorization required".to_owned(),
                    }),
                ))
            }
        }
        SecretStatus::MissingEnv(name) => Err((
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ErrorResponse {
                error: format!("configured operator token env is missing: {name}"),
            }),
        )),
    }
}

fn authorize_shared_secret_request(
    gateway: &GatewayHandle,
    headers: &HeaderMap,
    env_name: Option<&str>,
    header_name: &str,
    surface: &str,
) -> Result<(), (StatusCode, Json<ErrorResponse>)> {
    match secret_status(env_name) {
        SecretStatus::Disabled => Ok(()),
        SecretStatus::Ready(secret) => {
            if header_token(headers, header_name) == Some(secret.as_str()) {
                Ok(())
            } else {
                record_auth_denial(gateway, surface);
                Err((
                    StatusCode::UNAUTHORIZED,
                    Json(ErrorResponse {
                        error: format!("{surface} shared secret required"),
                    }),
                ))
            }
        }
        SecretStatus::MissingEnv(name) => Err((
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ErrorResponse {
                error: format!("configured shared secret env is missing: {name}"),
            }),
        )),
    }
}

fn matches_identifier(left: Option<&str>, right: Option<&str>) -> bool {
    matches!((left, right), (Some(left), Some(right)) if left == right)
}

fn load_incident_trace(runs_dir: &FsPath, identifier: &str) -> Result<RunTrace> {
    for entry in fs::read_dir(runs_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
            continue;
        }
        let content = fs::read_to_string(&path)?;
        let trace: RunTrace = serde_json::from_str(&content)?;
        if trace.run_id == identifier
            || trace.gateway_run_id.as_deref() == Some(identifier)
            || trace.correlation_id.as_deref() == Some(identifier)
        {
            return Ok(trace);
        }
    }

    bail!("incident trace not found: {}", identifier)
}

fn webchat_adapter_status(auth: &AuthConfig) -> AdapterStatusDto {
    let shared_secret = secret_status(auth.webchat_shared_secret_env.as_deref());
    let (status, detail) = match shared_secret {
        SecretStatus::Disabled => (
            "ok",
            "WebChat ingress is ready without an additional shared secret.",
        ),
        SecretStatus::Ready(_) => (
            "ok",
            "WebChat ingress is ready and protected by X-Mosaic-Shared-Secret.",
        ),
        SecretStatus::MissingEnv(_) => (
            "error",
            "WebChat ingress secret is configured but the environment variable is missing.",
        ),
    };
    AdapterStatusDto {
        name: "webchat".to_owned(),
        channel: "webchat".to_owned(),
        transport: "http".to_owned(),
        ingress_path: "/ingress/webchat".to_owned(),
        outbound_ready: true,
        status: status.to_owned(),
        detail: detail.to_owned(),
    }
}

fn telegram_adapter_status(auth: &AuthConfig) -> AdapterStatusDto {
    let outbound_ready =
        env::var("MOSAIC_TELEGRAM_BOT_TOKEN").is_ok() || env::var("TELEGRAM_BOT_TOKEN").is_ok();
    let shared_secret = secret_status(auth.telegram_secret_token_env.as_deref());
    let (status, detail) = match (outbound_ready, shared_secret) {
        (_, SecretStatus::MissingEnv(_)) => (
            "error",
            "Telegram webhook secret is configured but the environment variable is missing.",
        ),
        (true, SecretStatus::Ready(_)) => (
            "ok",
            "Telegram webhook ingress and replies are ready with secret-token verification.",
        ),
        (true, SecretStatus::Disabled) => ("ok", "Telegram webhook ingress and replies are ready."),
        (false, SecretStatus::Ready(_)) => (
            "warning",
            "Telegram ingress is protected, but outbound replies still need TELEGRAM_BOT_TOKEN or MOSAIC_TELEGRAM_BOT_TOKEN.",
        ),
        (false, SecretStatus::Disabled) => (
            "warning",
            "Telegram ingress is ready, but outbound replies need TELEGRAM_BOT_TOKEN or MOSAIC_TELEGRAM_BOT_TOKEN.",
        ),
    };
    AdapterStatusDto {
        name: "telegram".to_owned(),
        channel: "telegram".to_owned(),
        transport: "http-webhook".to_owned(),
        ingress_path: "/ingress/telegram".to_owned(),
        outbound_ready,
        status: status.to_owned(),
        detail: detail.to_owned(),
    }
}

fn ingress_route(session_id: Option<&str>, ingress: Option<&IngressTrace>) -> Option<String> {
    let ingress = ingress?;
    let channel = ingress.channel.as_deref().unwrap_or(ingress.kind.as_str());
    let target = ingress
        .reply_target
        .as_deref()
        .or(ingress.actor_id.as_deref())
        .or(session_id)?;
    let mut route = format!(
        "gateway.channel/{}/{}",
        route_segment(channel),
        route_segment(target)
    );
    if let Some(thread_id) = ingress.thread_id.as_deref() {
        route.push_str("/thread/");
        route.push_str(&route_segment(thread_id));
    }
    Some(route)
}

fn route_segment(value: &str) -> String {
    let mut normalized = String::with_capacity(value.len());
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.') {
            normalized.push(ch);
        } else {
            normalized.push('-');
        }
    }
    normalized.trim_matches('-').to_owned()
}

fn truncate_preview(value: &str, limit: usize) -> String {
    if value.chars().count() <= limit {
        return value.to_owned();
    }

    let truncated: String = value.chars().take(limit).collect();
    format!("{truncated}...")
}

fn extension_trace(status: &ExtensionStatus) -> ExtensionTrace {
    ExtensionTrace {
        name: status.name.clone(),
        version: status.version.clone(),
        source: status.source.clone(),
        enabled: status.enabled,
        active: status.active,
        error: status.error.clone(),
    }
}

fn extension_status_dto(status: &ExtensionStatus) -> ExtensionStatusDto {
    ExtensionStatusDto {
        name: status.name.clone(),
        version: status.version.clone(),
        source: status.source.clone(),
        enabled: status.enabled,
        active: status.active,
        tools: status.tools.clone(),
        skills: status.skills.clone(),
        workflows: status.workflows.clone(),
        mcp_servers: status.mcp_servers.clone(),
        error: status.error.clone(),
    }
}

fn extension_policy_dto(policy: &PolicyConfig) -> ExtensionPolicyDto {
    ExtensionPolicyDto {
        allow_exec: policy.allow_exec,
        allow_webhook: policy.allow_webhook,
        allow_cron: policy.allow_cron,
        allow_mcp: policy.allow_mcp,
        hot_reload_enabled: policy.hot_reload_enabled,
    }
}

pub fn session_summary_dto(summary: &SessionSummary) -> SessionSummaryDto {
    SessionSummaryDto {
        id: summary.id.clone(),
        title: summary.title.clone(),
        updated_at: summary.updated_at,
        provider_profile: summary.provider_profile.clone(),
        provider_type: summary.provider_type.clone(),
        model: summary.model.clone(),
        session_route: summary.session_route.clone(),
        channel_context: session_channel_dto(&summary.channel_context),
        last_gateway_run_id: summary.last_gateway_run_id.clone(),
        last_correlation_id: summary.last_correlation_id.clone(),
        message_count: summary.message_count,
        last_message_preview: summary.last_message_preview.clone(),
        memory_summary_preview: summary.memory_summary_preview.clone(),
        reference_count: summary.reference_count,
    }
}

fn session_channel_dto(metadata: &SessionChannelMetadata) -> SessionChannelDto {
    SessionChannelDto {
        ingress_kind: metadata.ingress_kind.clone(),
        channel: metadata.channel.clone(),
        source: metadata.source.clone(),
        actor_id: metadata.actor_id.clone(),
        actor_name: metadata.actor_name.clone(),
        thread_id: metadata.thread_id.clone(),
        thread_title: metadata.thread_title.clone(),
        reply_target: metadata.reply_target.clone(),
    }
}

pub fn cron_registration_dto(registration: &CronRegistration) -> CronRegistrationDto {
    CronRegistrationDto {
        id: registration.id.clone(),
        schedule: registration.schedule.clone(),
        input: registration.input.clone(),
        session_id: registration.session_id.clone(),
        profile: registration.profile.clone(),
        skill: registration.skill.clone(),
        workflow: registration.workflow.clone(),
        created_at: registration.created_at,
        last_triggered_at: registration.last_triggered_at,
    }
}

pub fn session_detail_dto(session: &SessionRecord) -> SessionDetailDto {
    SessionDetailDto {
        id: session.id.clone(),
        title: session.title.clone(),
        created_at: session.created_at,
        updated_at: session.updated_at,
        provider_profile: session.provider_profile.clone(),
        provider_type: session.provider_type.clone(),
        model: session.model.clone(),
        last_run_id: session.last_run_id.clone(),
        channel_context: session_channel_dto(&session.channel_context),
        gateway: SessionGatewayDto {
            route: session.gateway.route.clone(),
            last_gateway_run_id: session.gateway.last_gateway_run_id.clone(),
            last_correlation_id: session.gateway.last_correlation_id.clone(),
        },
        memory_summary: session.memory.latest_summary.clone(),
        compressed_context: session.memory.compressed_context.clone(),
        references: session
            .references
            .iter()
            .map(|reference| mosaic_control_protocol::SessionReferenceDto {
                session_id: reference.session_id.clone(),
                reason: reference.reason.clone(),
                created_at: reference.created_at,
            })
            .collect(),
        transcript: session
            .transcript
            .iter()
            .map(|message| TranscriptMessageDto {
                role: match message.role {
                    TranscriptRole::System => TranscriptRoleDto::System,
                    TranscriptRole::User => TranscriptRoleDto::User,
                    TranscriptRole::Assistant => TranscriptRoleDto::Assistant,
                    TranscriptRole::Tool => TranscriptRoleDto::Tool,
                },
                content: message.content.clone(),
                tool_call_id: message.tool_call_id.clone(),
                created_at: message.created_at,
            })
            .collect(),
    }
}

pub fn run_response(result: GatewayRunResult) -> RunResponse {
    RunResponse {
        gateway_run_id: result.gateway_run_id,
        correlation_id: result.correlation_id,
        session_route: result.session_route,
        output: result.output,
        trace: result.trace,
        session_summary: result.session_summary.as_ref().map(session_summary_dto),
    }
}

#[derive(Clone)]
struct GatewayHttpState {
    gateway: GatewayHandle,
}

type HttpResult<T> = std::result::Result<Json<T>, (StatusCode, Json<ErrorResponse>)>;

pub fn http_router(gateway: GatewayHandle) -> Router {
    Router::new()
        .route("/health", get(http_health))
        .route("/ready", get(http_ready))
        .route("/metrics", get(http_metrics))
        .route("/adapters", get(http_list_adapters))
        .route("/sessions", get(http_list_sessions))
        .route("/sessions/{id}", get(http_get_session))
        .route("/capabilities/jobs", get(http_list_capability_jobs))
        .route("/capabilities/exec", post(http_exec_capability))
        .route("/capabilities/webhook", post(http_webhook_capability))
        .route("/runs", post(http_submit_run))
        .route("/cron", get(http_list_cron).post(http_register_cron))
        .route("/cron/{id}/trigger", post(http_trigger_cron))
        .route("/audit/events", get(http_audit_events))
        .route("/incidents/{id}", get(http_incident_bundle))
        .route("/ingress/webchat", post(http_webchat_ingress))
        .route("/ingress/telegram", post(http_telegram_ingress))
        .route("/events", get(http_events))
        .route("/events/recent", get(http_recent_events))
        .with_state(GatewayHttpState { gateway })
}

pub async fn serve_http(gateway: GatewayHandle, addr: SocketAddr) -> Result<()> {
    serve_http_with_shutdown(gateway, addr, std::future::pending::<()>()).await
}

pub async fn serve_http_with_shutdown<F>(
    gateway: GatewayHandle,
    addr: SocketAddr,
    shutdown: F,
) -> Result<()>
where
    F: std::future::Future<Output = ()> + Send + 'static,
{
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, http_router(gateway))
        .with_graceful_shutdown(shutdown)
        .await?;
    Ok(())
}

async fn http_health(State(state): State<GatewayHttpState>) -> Json<HealthResponse> {
    Json(state.gateway.health())
}

async fn http_ready(State(state): State<GatewayHttpState>) -> HttpResult<ReadinessResponse> {
    let components = state.gateway.snapshot_components();
    if !components.observability.enable_readiness {
        return Err((
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ErrorResponse {
                error: "readiness endpoint is disabled by config".to_owned(),
            }),
        ));
    }
    Ok(Json(state.gateway.readiness()))
}

async fn http_metrics(
    State(state): State<GatewayHttpState>,
    headers: HeaderMap,
) -> HttpResult<MetricsResponse> {
    authorize_control_request(&state.gateway, &headers, "/metrics")?;
    let components = state.gateway.snapshot_components();
    if !components.observability.enable_metrics {
        return Err((
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ErrorResponse {
                error: "metrics endpoint is disabled by config".to_owned(),
            }),
        ));
    }
    Ok(Json(state.gateway.metrics()))
}

async fn http_list_adapters(
    State(state): State<GatewayHttpState>,
    headers: HeaderMap,
) -> HttpResult<Vec<AdapterStatusDto>> {
    authorize_control_request(&state.gateway, &headers, "/adapters")?;
    Ok(Json(state.gateway.list_adapter_statuses()))
}

async fn http_list_sessions(
    State(state): State<GatewayHttpState>,
    headers: HeaderMap,
) -> HttpResult<Vec<SessionSummaryDto>> {
    authorize_control_request(&state.gateway, &headers, "/sessions")?;
    let sessions = state
        .gateway
        .list_sessions()
        .map_err(http_internal_error)?
        .iter()
        .map(session_summary_dto)
        .collect();
    Ok(Json(sessions))
}

async fn http_list_capability_jobs(
    State(state): State<GatewayHttpState>,
    headers: HeaderMap,
) -> HttpResult<Vec<CapabilityJobDto>> {
    authorize_control_request(&state.gateway, &headers, "/capabilities/jobs")?;
    Ok(Json(state.gateway.list_capability_jobs()))
}

async fn http_exec_capability(
    State(state): State<GatewayHttpState>,
    headers: HeaderMap,
    Json(request): Json<ExecJobRequest>,
) -> HttpResult<CapabilityJobDto> {
    authorize_control_request(&state.gateway, &headers, "/capabilities/exec")?;
    let job = state
        .gateway
        .run_exec_job(request)
        .await
        .map_err(http_internal_error)?;
    Ok(Json(job))
}

async fn http_webhook_capability(
    State(state): State<GatewayHttpState>,
    headers: HeaderMap,
    Json(request): Json<WebhookJobRequest>,
) -> HttpResult<CapabilityJobDto> {
    authorize_control_request(&state.gateway, &headers, "/capabilities/webhook")?;
    let job = state
        .gateway
        .run_webhook_job(request)
        .await
        .map_err(http_internal_error)?;
    Ok(Json(job))
}

async fn http_list_cron(
    State(state): State<GatewayHttpState>,
    headers: HeaderMap,
) -> HttpResult<Vec<CronRegistrationDto>> {
    authorize_control_request(&state.gateway, &headers, "/cron")?;
    let registrations = state
        .gateway
        .list_cron_registrations()
        .map_err(http_internal_error)?
        .iter()
        .map(cron_registration_dto)
        .collect();
    Ok(Json(registrations))
}

async fn http_register_cron(
    State(state): State<GatewayHttpState>,
    headers: HeaderMap,
    Json(request): Json<CronRegistrationRequest>,
) -> HttpResult<CronRegistrationDto> {
    authorize_control_request(&state.gateway, &headers, "/cron")?;
    let registration = state
        .gateway
        .register_cron(request)
        .map_err(http_internal_error)?;
    Ok(Json(cron_registration_dto(&registration)))
}

async fn http_trigger_cron(
    AxumPath(id): AxumPath<String>,
    State(state): State<GatewayHttpState>,
    headers: HeaderMap,
) -> HttpResult<RunResponse> {
    authorize_control_request(&state.gateway, &headers, "/cron/trigger")?;
    let result = state
        .gateway
        .trigger_cron(&id)
        .await
        .map_err(http_internal_error)?;
    Ok(Json(run_response(result)))
}

async fn http_get_session(
    AxumPath(id): AxumPath<String>,
    State(state): State<GatewayHttpState>,
    headers: HeaderMap,
) -> HttpResult<SessionDetailDto> {
    authorize_control_request(&state.gateway, &headers, "/sessions/{id}")?;
    let session = state
        .gateway
        .load_session(&id)
        .map_err(http_internal_error)?;
    match session {
        Some(session) => Ok(Json(session_detail_dto(&session))),
        None => Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: format!("session not found: {id}"),
            }),
        )),
    }
}

async fn http_submit_run(
    State(state): State<GatewayHttpState>,
    headers: HeaderMap,
    Json(mut request): Json<RunSubmission>,
) -> HttpResult<RunResponse> {
    authorize_control_request(&state.gateway, &headers, "/runs")?;
    if request.ingress.is_none() {
        request.ingress = Some(IngressTrace {
            kind: "remote_operator".to_owned(),
            channel: Some("api".to_owned()),
            source: Some("mosaic-sdk".to_owned()),
            remote_addr: None,
            display_name: None,
            actor_id: None,
            thread_id: None,
            thread_title: None,
            reply_target: None,
            gateway_url: None,
        });
    }

    let submitted = state
        .gateway
        .submit_run(request)
        .map_err(http_internal_error)?;
    let result = submitted.wait().await.map_err(http_run_error)?;
    Ok(Json(run_response(result)))
}

async fn http_webchat_ingress(
    State(state): State<GatewayHttpState>,
    headers: HeaderMap,
    Json(message): Json<InboundMessage>,
) -> HttpResult<RunResponse> {
    let auth = state.gateway.snapshot_components().auth;
    authorize_shared_secret_request(
        &state.gateway,
        &headers,
        auth.webchat_shared_secret_env.as_deref(),
        "x-mosaic-shared-secret",
        "webchat ingress",
    )?;
    let request = RunSubmission {
        system: None,
        input: message.input,
        skill: None,
        workflow: None,
        session_id: Some(
            message
                .session_id
                .unwrap_or_else(|| format!("webchat-{}", Uuid::new_v4())),
        ),
        profile: message.profile,
        ingress: Some(message.ingress.unwrap_or(IngressTrace {
            kind: "webchat".to_owned(),
            channel: Some("webchat".to_owned()),
            source: Some("webchat-ingress".to_owned()),
            remote_addr: None,
            display_name: message.display_name,
            actor_id: message.actor_id,
            thread_id: message.thread_id,
            thread_title: message.thread_title,
            reply_target: message.reply_target,
            gateway_url: None,
        })),
    };

    let submitted = state
        .gateway
        .submit_run(request)
        .map_err(http_internal_error)?;
    let result = submitted.wait().await.map_err(http_run_error)?;
    Ok(Json(run_response(result)))
}

async fn http_telegram_ingress(
    State(state): State<GatewayHttpState>,
    headers: HeaderMap,
    Json(update): Json<TelegramUpdate>,
) -> HttpResult<RunResponse> {
    let auth = state.gateway.snapshot_components().auth;
    authorize_shared_secret_request(
        &state.gateway,
        &headers,
        auth.telegram_secret_token_env.as_deref(),
        "x-telegram-bot-api-secret-token",
        "telegram ingress",
    )?;
    let submitted = state
        .gateway
        .submit_telegram_update(update)
        .map_err(http_internal_error)?;
    let result = submitted.wait().await.map_err(http_run_error)?;
    Ok(Json(run_response(result)))
}

async fn http_audit_events(
    State(state): State<GatewayHttpState>,
    headers: HeaderMap,
    Query(query): Query<LimitQuery>,
) -> HttpResult<Vec<GatewayAuditEventDto>> {
    authorize_control_request(&state.gateway, &headers, "/audit/events")?;
    Ok(Json(state.gateway.audit_events(
        query.limit.unwrap_or(DEFAULT_AUDIT_QUERY_LIMIT),
    )))
}

async fn http_incident_bundle(
    AxumPath(id): AxumPath<String>,
    State(state): State<GatewayHttpState>,
    headers: HeaderMap,
) -> HttpResult<IncidentBundleDto> {
    authorize_control_request(&state.gateway, &headers, "/incidents/{id}")?;
    let (bundle, _) = state
        .gateway
        .incident_bundle(&id)
        .map_err(http_internal_error)?;
    Ok(Json(bundle))
}

async fn http_events(
    State(state): State<GatewayHttpState>,
    headers: HeaderMap,
) -> std::result::Result<
    Sse<impl futures::Stream<Item = std::result::Result<SseEvent, Infallible>>>,
    (StatusCode, Json<ErrorResponse>),
> {
    authorize_control_request(&state.gateway, &headers, "/events")?;
    let receiver = state.gateway.subscribe();
    let gateway = state.gateway.clone();
    let stream = stream::unfold(receiver, move |mut receiver| {
        let gateway = gateway.clone();
        async move {
            match receiver.recv().await {
                Ok(envelope) => {
                    let payload = serde_json::to_string(&envelope)
                        .unwrap_or_else(|_| "{\"error\":\"failed to encode event\"}".to_owned());
                    Some((Ok(SseEvent::default().data(payload)), receiver))
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(skipped)) => {
                    increment_metric(gateway.inner.as_ref(), |metrics| {
                        metrics.broadcast_lag_events_total += skipped as u64;
                    });
                    Some((
                        Ok(SseEvent::default()
                            .event("lagged")
                            .data(skipped.to_string())),
                        receiver,
                    ))
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => None,
            }
        }
    });

    Ok(Sse::new(stream).keep_alive(KeepAlive::default()))
}

async fn http_recent_events(
    State(state): State<GatewayHttpState>,
    headers: HeaderMap,
    Query(query): Query<LimitQuery>,
) -> HttpResult<ReplayWindowResponse> {
    authorize_control_request(&state.gateway, &headers, "/events/recent")?;
    Ok(Json(state.gateway.replay_window(
        query.limit.unwrap_or(DEFAULT_REPLAY_QUERY_LIMIT),
    )))
}

fn http_internal_error(error: anyhow::Error) -> (StatusCode, Json<ErrorResponse>) {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(ErrorResponse {
            error: public_error_message(&error),
        }),
    )
}

fn http_run_error(error: GatewayRunError) -> (StatusCode, Json<ErrorResponse>) {
    let message = public_error_message(&error.into_parts().0);
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(ErrorResponse { error: message }),
    )
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::sync::Mutex;
    use std::time::Duration;

    use futures::StreamExt;
    use mosaic_config::{
        LoadConfigOptions, MosaicConfig, ProviderProfileConfig, load_mosaic_config,
    };
    use mosaic_extension_core::load_extension_set;
    use mosaic_mcp_core::{McpServerManager, McpServerSpec};
    use mosaic_memory::{FileMemoryStore, MemoryPolicy};
    use mosaic_provider::MockProvider;
    use mosaic_scheduler_core::FileCronStore;
    use mosaic_session_core::{SessionStore, TranscriptRole};
    use tokio::sync::oneshot;

    use super::*;

    #[derive(Default)]
    struct MemorySessionStore {
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
        config.active_profile = "mock".to_owned();
        config.profiles.insert(
            "mock".to_owned(),
            ProviderProfileConfig {
                provider_type: "mock".to_owned(),
                model: "mock".to_owned(),
                base_url: None,
                api_key_env: None,
            },
        );
        let profiles =
            ProviderProfileRegistry::from_config(&config).expect("profile registry should build");
        let mut tools = ToolRegistry::new();
        tools.register(Arc::new(mosaic_tool_core::TimeNowTool::new()));

        GatewayHandle::new_local(
            Handle::current(),
            GatewayRuntimeComponents {
                profiles: Arc::new(profiles),
                provider_override: Some(Arc::new(MockProvider)),
                session_store: Arc::new(MemorySessionStore::default()),
                memory_store: Arc::new(FileMemoryStore::new(
                    std::env::temp_dir().join("mosaic-gateway-tests-memory"),
                )),
                memory_policy: MemoryPolicy::default(),
                tools: Arc::new(tools),
                skills: Arc::new(SkillRegistry::new()),
                workflows: Arc::new(WorkflowRegistry::new()),
                node_store: test_node_store(),
                mcp_manager: None,
                cron_store: test_cron_store(),
                workspace_root: std::env::temp_dir().join("mosaic-gateway-tests-workspace"),
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
        let cron_store: Arc<dyn CronStore> =
            Arc::new(FileCronStore::new(root.join(".mosaic/cron")));
        let extension_set = load_extension_set(&loaded.config, None, root, cron_store.clone())
            .expect("extension set should load");

        GatewayHandle::new_local_with_reload_source(
            Handle::current(),
            GatewayRuntimeComponents {
                profiles: Arc::new(profiles),
                provider_override: Some(Arc::new(MockProvider)),
                session_store: Arc::new(MemorySessionStore::default()),
                memory_store: Arc::new(FileMemoryStore::new(root.join(".mosaic/memory"))),
                memory_policy: MemoryPolicy::default(),
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
        let mut saw_session_updated = false;
        let mut saw_completed = false;

        while let Ok(envelope) = receiver.try_recv() {
            if envelope.gateway_run_id != run_id {
                continue;
            }

            match envelope.event {
                GatewayEvent::RunSubmitted { .. } => saw_submitted = true,
                GatewayEvent::Runtime(_) => saw_runtime = true,
                GatewayEvent::SessionUpdated { .. } => saw_session_updated = true,
                GatewayEvent::RunCompleted { .. } => saw_completed = true,
                GatewayEvent::RunFailed { .. } => {}
                GatewayEvent::CapabilityJobUpdated { .. } => {}
                GatewayEvent::CronUpdated { .. } => {}
                GatewayEvent::ExtensionsReloaded { .. } => {}
                GatewayEvent::ExtensionReloadFailed { .. } => {}
            }
        }

        assert!(saw_submitted);
        assert!(saw_runtime);
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
        config.active_profile = "mock".to_owned();
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

        let gateway = GatewayHandle::new_local(
            Handle::current(),
            GatewayRuntimeComponents {
                profiles: Arc::new(profiles),
                provider_override: Some(Arc::new(MockProvider)),
                session_store: Arc::new(MemorySessionStore::default()),
                memory_store: Arc::new(FileMemoryStore::new(
                    std::env::temp_dir().join("mosaic-gateway-tests-memory"),
                )),
                memory_policy: MemoryPolicy::default(),
                tools: Arc::new(tools),
                skills: Arc::new(SkillRegistry::new()),
                workflows: Arc::new(WorkflowRegistry::new()),
                node_store: test_node_store(),
                mcp_manager: Some(manager),
                cron_store: test_cron_store(),
                workspace_root: std::env::temp_dir().join("mosaic-gateway-tests-workspace"),
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
        assert_eq!(health.active_profile, "mock");
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
                thread_id: Some("room-7".to_owned()),
                thread_title: Some("Launch Room".to_owned()),
                reply_target: Some("webchat:guest-1".to_owned()),
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
            Some("webchat")
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
        assert!(frame.contains("RunSubmitted"));
        assert!(frame.contains("events-demo"));

        let _ = shutdown.send(());
    }
}
