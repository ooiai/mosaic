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
use mosaic_channel_telegram::TelegramUpdate;
use mosaic_config::{
    AppConfig, AuditConfig, AuthConfig, DeploymentConfig, LoadConfigOptions, ObservabilityConfig,
    PolicyConfig, load_mosaic_config,
};
use mosaic_control_protocol::{
    AdapterStatusDto, CapabilityJobDto, ChannelDeliveryTrace, ChannelInboundMessage,
    ChannelOutboundMessage, CronRegistrationDto, CronRegistrationRequest, ErrorResponse,
    EventStreamEnvelope, ExecJobRequest, ExtensionPolicyDto, ExtensionStatusDto,
    GatewayAuditEventDto, GatewayEvent, HealthResponse, InboundMessage, IncidentBundleDto,
    MetricsResponse, NodeBindingDto, ReadinessResponse, ReplayWindowResponse, RunDetailDto,
    RunResponse, RunSubmission, RunSummaryDto, SessionChannelDto, SessionDetailDto,
    SessionGatewayDto, SessionRunDto, SessionSummaryDto, TranscriptMessageDto, TranscriptRoleDto,
    WebhookJobRequest,
};
use mosaic_extension_core::{
    ExtensionStatus, ExtensionValidationReport, load_extension_set, validate_extension_set,
};
use mosaic_inspect::{
    ExtensionTrace, GovernanceTrace, IngressTrace, RouteDecisionTrace, RouteMode,
    RunLifecycleStatus, RunTrace,
};
use mosaic_mcp_core::McpServerManager;
use mosaic_memory::{MemoryPolicy, MemoryStore};
use mosaic_node_protocol::{
    DEFAULT_AFFINITY_KEY, DEFAULT_STALE_AFTER_SECS, FileNodeStore, NodeAffinityRecord,
    NodeCapabilityDeclaration, NodeRegistration,
};
use mosaic_provider::{LlmProvider, ProviderProfileRegistry, public_error_message};
use mosaic_runtime::events::{RunEvent, RunEventSink, SharedRunEventSink};
use mosaic_runtime::{RunError, RunResult, RuntimeContext};
use mosaic_sandbox_core::{SandboxCleanupPolicy, SandboxManager, SandboxSettings};
use mosaic_scheduler_core::{CronRegistration, CronStore};
use mosaic_session_core::{
    SessionChannelMetadata, SessionRecord, SessionStore, SessionSummary, TranscriptRole,
    session_route_for_id, session_title_from_input,
};
use mosaic_skill_core::SkillRegistry;
use mosaic_tool_core::ToolRegistry;
use mosaic_workflow::WorkflowRegistry;
use serde::{Deserialize, Serialize};
use tokio::runtime::Handle;
use tokio::sync::{broadcast, watch};
use tracing::{info, warn};
use uuid::Uuid;

pub use mosaic_control_protocol::{
    GatewayEvent as ProtocolGatewayEvent, HealthResponse as GatewayHealthResponse,
};

const DEFAULT_AUDIT_QUERY_LIMIT: usize = 50;
const DEFAULT_REPLAY_QUERY_LIMIT: usize = 50;

fn build_sandbox_manager(
    workspace_root: &FsPath,
    config: &mosaic_config::MosaicConfig,
) -> Result<Arc<SandboxManager>> {
    let manager = Arc::new(SandboxManager::new(
        workspace_root,
        SandboxSettings {
            base_dir: PathBuf::from(&config.sandbox.base_dir),
            python_strategy: config.sandbox.python.strategy,
            node_strategy: config.sandbox.node.strategy,
            cleanup: SandboxCleanupPolicy {
                run_workdirs_after_hours: config.sandbox.cleanup.run_workdirs_after_hours,
                attachments_after_hours: config.sandbox.cleanup.attachments_after_hours,
            },
        },
    ));
    manager.ensure_layout()?;
    Ok(manager)
}

#[derive(Clone)]
pub struct GatewayRuntimeComponents {
    pub profiles: Arc<ProviderProfileRegistry>,
    pub provider_override: Option<Arc<dyn LlmProvider>>,
    pub session_store: Arc<dyn SessionStore>,
    pub memory_store: Arc<dyn MemoryStore>,
    pub memory_policy: MemoryPolicy,
    pub runtime_policy: mosaic_config::RuntimePolicyConfig,
    pub attachments: mosaic_config::AttachmentConfig,
    pub sandbox: Arc<SandboxManager>,
    pub telegram: mosaic_config::TelegramAdapterConfig,
    pub app_name: Option<String>,
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
            runtime_policy: self.runtime_policy.clone(),
            attachments: self.attachments.clone(),
            sandbox: self.sandbox.clone(),
            telegram: self.telegram.clone(),
            app_name: self.app_name.clone(),
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
    canceled_runs_total: u64,
    capability_jobs_total: u64,
    audit_events_total: u64,
    auth_denials_total: u64,
    broadcast_lag_events_total: u64,
}

#[derive(Debug, Clone, Default)]
struct ChannelConversationBinding {
    session_id: Option<String>,
    profile: Option<String>,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StoredRunRecord {
    gateway_run_id: String,
    correlation_id: String,
    run_id: String,
    session_id: Option<String>,
    session_route: String,
    #[serde(default)]
    status: RunLifecycleStatus,
    requested_profile: Option<String>,
    effective_profile: Option<String>,
    effective_provider_type: Option<String>,
    effective_model: Option<String>,
    tool: Option<String>,
    skill: Option<String>,
    workflow: Option<String>,
    retry_of: Option<String>,
    created_at: chrono::DateTime<Utc>,
    updated_at: chrono::DateTime<Utc>,
    finished_at: Option<chrono::DateTime<Utc>>,
    input_preview: String,
    output_preview: Option<String>,
    error: Option<String>,
    failure_kind: Option<String>,
    failure_origin: Option<String>,
    trace_path: Option<String>,
    ingress: Option<IngressTrace>,
    #[serde(default)]
    outbound_deliveries: Vec<ChannelDeliveryTrace>,
    submission: RunSubmission,
}

impl StoredRunRecord {
    fn new(
        meta: &GatewayRunMeta,
        request: &RunSubmission,
        resolved_profile: Option<String>,
        retry_of: Option<String>,
    ) -> Self {
        let now = Utc::now();
        Self {
            gateway_run_id: meta.gateway_run_id.clone(),
            correlation_id: meta.correlation_id.clone(),
            run_id: meta.run_id.clone(),
            session_id: meta.session_id.clone(),
            session_route: meta.session_route.clone(),
            status: RunLifecycleStatus::Queued,
            requested_profile: resolved_profile,
            effective_profile: None,
            effective_provider_type: None,
            effective_model: None,
            tool: request.tool.clone(),
            skill: request.skill.clone(),
            workflow: request.workflow.clone(),
            retry_of,
            created_at: now,
            updated_at: now,
            finished_at: None,
            input_preview: truncate_preview(&request.input, 160),
            output_preview: None,
            error: None,
            failure_kind: None,
            failure_origin: None,
            trace_path: None,
            ingress: request.ingress.clone(),
            outbound_deliveries: Vec::new(),
            submission: request.clone(),
        }
    }

    fn set_status(&mut self, status: RunLifecycleStatus) {
        self.status = status;
        self.updated_at = Utc::now();
        if status.is_terminal() {
            self.finished_at = Some(self.updated_at);
        }
    }

    fn set_error(
        &mut self,
        error: Option<String>,
        failure_kind: Option<String>,
        failure_origin: Option<String>,
    ) {
        self.error = error;
        self.failure_kind = failure_kind;
        self.failure_origin = failure_origin;
        self.updated_at = Utc::now();
    }

    fn update_from_trace(&mut self, trace: &RunTrace, trace_path: Option<&FsPath>) {
        self.run_id = trace.run_id.clone();
        self.status = trace.lifecycle_status();
        self.updated_at = Utc::now();
        self.finished_at = trace.finished_at;
        self.ingress = trace.ingress.clone();
        self.output_preview = trace
            .output
            .as_deref()
            .map(|output| truncate_preview(output, 160));
        self.error = trace.error.clone();
        self.failure_kind = trace.failure.as_ref().map(|failure| failure.kind.clone());
        self.failure_origin = trace
            .failure
            .as_ref()
            .map(|failure| failure.origin.label().to_owned());
        self.trace_path = trace_path.map(|path| path.display().to_string());
        self.outbound_deliveries = trace.outbound_deliveries.clone();
        if let Some(profile) = trace.effective_profile.as_ref() {
            self.effective_profile = Some(profile.profile.clone());
            self.effective_provider_type = Some(profile.provider_type.clone());
            self.effective_model = Some(profile.model.clone());
        }
    }

    fn summary_dto(&self) -> RunSummaryDto {
        RunSummaryDto {
            gateway_run_id: self.gateway_run_id.clone(),
            correlation_id: self.correlation_id.clone(),
            run_id: self.run_id.clone(),
            session_id: self.session_id.clone(),
            session_route: self.session_route.clone(),
            status: self.status,
            requested_profile: self.requested_profile.clone(),
            effective_profile: self.effective_profile.clone(),
            effective_provider_type: self.effective_provider_type.clone(),
            effective_model: self.effective_model.clone(),
            tool: self.tool.clone(),
            skill: self.skill.clone(),
            workflow: self.workflow.clone(),
            retry_of: self.retry_of.clone(),
            created_at: self.created_at,
            updated_at: self.updated_at,
            finished_at: self.finished_at,
            input_preview: self.input_preview.clone(),
            output_preview: self.output_preview.clone(),
            error: self.error.clone(),
            failure_kind: self.failure_kind.clone(),
            failure_origin: self.failure_origin.clone(),
            trace_path: self.trace_path.clone(),
        }
    }

    fn detail_dto(&self) -> RunDetailDto {
        RunDetailDto {
            summary: self.summary_dto(),
            ingress: self.ingress.clone(),
            outbound_deliveries: self.outbound_deliveries.clone(),
            submission: self.submission.clone(),
        }
    }
}

#[derive(Debug)]
struct GatewayRunStore {
    root: PathBuf,
}

impl GatewayRunStore {
    fn new(runs_dir: PathBuf) -> Self {
        Self {
            root: runs_dir.join("registry"),
        }
    }

    fn ensure_root(&self) -> Result<()> {
        fs::create_dir_all(&self.root)?;
        Ok(())
    }

    fn path_for(&self, gateway_run_id: &str) -> PathBuf {
        self.root.join(format!("{gateway_run_id}.json"))
    }

    fn save(&self, record: &StoredRunRecord) -> Result<()> {
        self.ensure_root()?;
        fs::write(
            self.path_for(&record.gateway_run_id),
            serde_json::to_vec_pretty(record)?,
        )?;
        Ok(())
    }

    fn load_gateway(&self, gateway_run_id: &str) -> Result<Option<StoredRunRecord>> {
        let path = self.path_for(gateway_run_id);
        if !path.exists() {
            return Ok(None);
        }
        Ok(Some(serde_json::from_str(&fs::read_to_string(path)?)?))
    }

    fn list(&self) -> Result<Vec<StoredRunRecord>> {
        self.ensure_root()?;
        let mut records = Vec::new();
        for entry in fs::read_dir(&self.root)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
                continue;
            }
            records.push(serde_json::from_str::<StoredRunRecord>(
                &fs::read_to_string(path)?,
            )?);
        }
        records.sort_by(|left, right| right.updated_at.cmp(&left.updated_at));
        Ok(records)
    }

    fn resolve(&self, identifier: &str) -> Result<Option<StoredRunRecord>> {
        if let Some(record) = self.load_gateway(identifier)? {
            return Ok(Some(record));
        }

        Ok(self
            .list()?
            .into_iter()
            .find(|record| record.correlation_id == identifier || record.run_id == identifier))
    }
}

#[derive(Clone)]
struct ActiveRunHandle {
    cancel: watch::Sender<bool>,
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
    conversation_bindings: Mutex<BTreeMap<String, ChannelConversationBinding>>,
    capability_jobs: Arc<Mutex<BTreeMap<String, CapabilityJobDto>>>,
    run_store: Arc<GatewayRunStore>,
    active_runs: Mutex<BTreeMap<String, ActiveRunHandle>>,
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
        let runs_dir = components.runs_dir.clone();
        Self {
            inner: Arc::new(GatewayState {
                runtime_handle,
                components: Mutex::new(components),
                reload_source,
                events,
                conversation_bindings: Mutex::new(BTreeMap::new()),
                capability_jobs: Arc::new(Mutex::new(BTreeMap::new())),
                run_store: Arc::new(GatewayRunStore::new(runs_dir)),
                active_runs: Mutex::new(BTreeMap::new()),
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
        let runs = self.inner.run_store.list().unwrap_or_default();
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
            queued_run_count: runs
                .iter()
                .filter(|record| {
                    matches!(
                        record.status,
                        RunLifecycleStatus::Queued | RunLifecycleStatus::CancelRequested
                    )
                })
                .count(),
            running_run_count: runs
                .iter()
                .filter(|record| {
                    matches!(
                        record.status,
                        RunLifecycleStatus::Running | RunLifecycleStatus::Streaming
                    )
                })
                .count(),
            completed_runs_total: metrics.completed_runs_total,
            failed_runs_total: metrics.failed_runs_total,
            canceled_runs_total: metrics.canceled_runs_total,
            capability_jobs_total: metrics.capability_jobs_total,
            audit_events_total: metrics.audit_events_total,
            auth_denials_total: metrics.auth_denials_total,
            broadcast_lag_events_total: metrics.broadcast_lag_events_total,
            replay_events_buffered: replay_window.events.len(),
            event_replay_window: replay_window.capacity,
        }
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
        let sandbox = build_sandbox_manager(&source.workspace_root, &loaded.config)?;
        let updated = GatewayRuntimeComponents {
            profiles,
            provider_override: current.provider_override.clone(),
            session_store: current.session_store.clone(),
            memory_store: current.memory_store.clone(),
            memory_policy: current.memory_policy.clone(),
            runtime_policy: current.runtime_policy.clone(),
            attachments: loaded.config.attachments.clone(),
            sandbox,
            telegram: loaded.config.telegram.clone(),
            app_name: source
                .app_config
                .as_ref()
                .and_then(|app| app.app.as_ref())
                .and_then(|app| app.name.clone()),
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
        let components = self.snapshot_components();
        let mut adapters = vec![webchat_adapter_status(&components.auth)];
        adapters.extend(telegram_adapter_statuses(&components));
        adapters
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

    pub fn list_node_affinities(&self) -> Result<Vec<NodeAffinityRecord>> {
        self.snapshot_components().node_store.list_affinities()
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

    pub fn node_binding(&self, session_id: Option<&str>) -> Result<Option<NodeBindingDto>> {
        let components = self.snapshot_components();
        let Some(affinity) = components.node_store.affinity_for_session(session_id)? else {
            return Ok(None);
        };
        let registration = components.node_store.load_node(&affinity.node_id)?;
        let (health, last_heartbeat_at, last_disconnect_reason) = match registration {
            Some(node) => (
                node.health(Utc::now(), DEFAULT_STALE_AFTER_SECS)
                    .label()
                    .to_owned(),
                Some(node.last_heartbeat_at),
                node.last_disconnect_reason,
            ),
            None => ("missing".to_owned(), None, None),
        };
        Ok(Some(NodeBindingDto {
            node_id: affinity.node_id,
            affinity_scope: if affinity.session_id == DEFAULT_AFFINITY_KEY {
                "default".to_owned()
            } else {
                "session".to_owned()
            },
            health,
            last_heartbeat_at,
            last_disconnect_reason,
        }))
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

    pub fn detach_node(&self, session_id: Option<&str>) -> Result<bool> {
        let components = self.snapshot_components();
        match session_id {
            Some(session_id) => components.node_store.detach_session(session_id),
            None => components.node_store.detach_default(),
        }
    }

    pub fn prune_stale_nodes(&self) -> Result<Vec<NodeRegistration>> {
        self.snapshot_components().node_store.prune_stale_nodes()
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
            tool: None,
            skill: registration.skill.clone(),
            workflow: registration.workflow.clone(),
            session_id: registration.session_id.clone(),
            profile: registration.profile.clone(),
            ingress: Some(IngressTrace {
                kind: "cron".to_owned(),
                channel: Some("cron".to_owned()),
                adapter: Some("cron_schedule".to_owned()),
                bot_name: None,
                bot_route: None,
                bot_profile: None,
                bot_token_env: None,
                bot_secret_env: None,
                source: Some(registration.id.clone()),
                remote_addr: None,
                display_name: None,
                actor_id: None,
                conversation_id: Some(format!("cron:{}", registration.id)),
                thread_id: None,
                thread_title: None,
                reply_target: None,
                message_id: None,
                received_at: None,
                raw_event_id: None,
                session_hint: registration.session_id.clone(),
                profile_hint: registration.profile.clone(),
                control_command: None,
                original_text: None,
                attachments: Vec::new(),
                attachment_failures: Vec::new(),
                gateway_url: None,
            }),
        })?
        .wait()
        .await
        .map_err(|err| anyhow!(err.to_string()))
    }

    fn emit(&self, envelope: GatewayEventEnvelope) {
        broadcast_envelope(self.inner.as_ref(), envelope);
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
    run_id: String,
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

fn run_record_envelope(record: &StoredRunRecord) -> GatewayEventEnvelope {
    GatewayEventEnvelope {
        gateway_run_id: record.gateway_run_id.clone(),
        correlation_id: record.correlation_id.clone(),
        session_id: record.session_id.clone(),
        session_route: record.session_route.clone(),
        emitted_at: Utc::now(),
        event: GatewayEvent::RunUpdated {
            run: record.summary_dto(),
        },
    }
}

async fn wait_for_cancellation(mut receiver: watch::Receiver<bool>) {
    loop {
        if *receiver.borrow() {
            break;
        }
        if receiver.changed().await.is_err() {
            std::future::pending::<()>().await;
        }
    }
}

fn sync_session_run_state(
    state: &GatewayState,
    record: &StoredRunRecord,
    status: RunLifecycleStatus,
    run_id: Option<String>,
    last_error: Option<String>,
    last_failure_kind: Option<String>,
) {
    let Some(session_id) = record.session_id.as_deref() else {
        return;
    };
    let components = state.snapshot_components();
    let Ok(Some(mut session)) = components.session_store.load(session_id) else {
        return;
    };
    session.set_gateway_binding(
        record.session_route.clone(),
        record.gateway_run_id.clone(),
        record.correlation_id.clone(),
    );
    session.set_run_state(
        status,
        run_id,
        Some(record.gateway_run_id.clone()),
        Some(record.correlation_id.clone()),
        last_error,
        last_failure_kind,
    );
    let _ = components.session_store.save(&session);
}

fn update_run_record(
    state: &GatewayState,
    gateway_run_id: &str,
    update: impl FnOnce(&mut StoredRunRecord),
) -> Option<StoredRunRecord> {
    let mut record = state.run_store.load_gateway(gateway_run_id).ok()??;
    update(&mut record);
    state.run_store.save(&record).ok()?;
    Some(record)
}

struct GatewayRunEventSink {
    state: Arc<GatewayState>,
    meta: GatewayRunMeta,
}

impl RunEventSink for GatewayRunEventSink {
    fn emit(&self, event: RunEvent) {
        if let Some(job) = capability::update_runtime_capability_job(
            &self.state.capability_jobs,
            &self.meta,
            &event,
        ) {
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

        let updated_record = match &event {
            RunEvent::RunStarted { run_id, .. } => {
                update_run_record(self.state.as_ref(), &self.meta.gateway_run_id, |record| {
                    record.run_id = run_id.clone();
                    record.set_status(RunLifecycleStatus::Running);
                    record.set_error(None, None, None);
                })
            }
            RunEvent::OutputDelta { .. } | RunEvent::FinalAnswerReady { .. } => {
                update_run_record(self.state.as_ref(), &self.meta.gateway_run_id, |record| {
                    record.set_status(RunLifecycleStatus::Streaming)
                })
            }
            RunEvent::RunFinished {
                run_id,
                output_preview,
            } => update_run_record(self.state.as_ref(), &self.meta.gateway_run_id, |record| {
                record.run_id = run_id.clone();
                record.set_status(RunLifecycleStatus::Success);
                record.output_preview = Some(output_preview.clone());
                record.set_error(None, None, None);
            }),
            RunEvent::RunFailed {
                run_id,
                error,
                failure_kind,
                failure_origin,
            } => update_run_record(self.state.as_ref(), &self.meta.gateway_run_id, |record| {
                record.run_id = run_id.clone();
                record.set_status(RunLifecycleStatus::Failed);
                record.set_error(
                    Some(error.clone()),
                    failure_kind.clone(),
                    failure_origin.clone(),
                );
            }),
            RunEvent::RunCanceled { run_id, reason } => {
                update_run_record(self.state.as_ref(), &self.meta.gateway_run_id, |record| {
                    record.run_id = run_id.clone();
                    record.set_status(RunLifecycleStatus::Canceled);
                    record.set_error(
                        Some(reason.clone()),
                        Some("canceled".to_owned()),
                        Some("gateway".to_owned()),
                    );
                })
            }
            _ => None,
        };

        if let Some(record) = updated_record {
            sync_session_run_state(
                self.state.as_ref(),
                &record,
                record.status,
                Some(record.run_id.clone()),
                record.error.clone(),
                record.failure_kind.clone(),
            );
            broadcast_envelope(self.state.as_ref(), run_record_envelope(&record));
        }

        broadcast_envelope(
            self.state.as_ref(),
            self.meta.envelope(GatewayEvent::Runtime(event)),
        );
    }
}

async fn finalize_run(
    state: Arc<GatewayState>,
    meta: GatewayRunMeta,
    outcome: std::result::Result<RunResult, RunError>,
) -> Result<GatewayRunResult, GatewayRunError> {
    match outcome {
        Ok(result) => finalize_success(state, meta, result).await,
        Err(err) => finalize_failure(state, meta, err).await,
    }
}

async fn finalize_success(
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
    let delivery_traces = dispatch_outbound_replies(state.as_ref(), &meta, &result.output).await;
    for delivery in delivery_traces.iter().cloned() {
        trace.add_outbound_delivery(delivery);
    }
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
    if let Some(record) = update_run_record(state.as_ref(), &meta.gateway_run_id, |record| {
        record.update_from_trace(&trace, Some(&trace_path));
    }) {
        sync_session_run_state(
            state.as_ref(),
            &record,
            RunLifecycleStatus::Success,
            Some(trace.run_id.clone()),
            None,
            None,
        );
        broadcast_envelope(state.as_ref(), run_record_envelope(&record));
    }
    let session_summary = update_gateway_session_metadata(
        &state,
        &meta,
        Some(trace.run_id.clone()),
        RunLifecycleStatus::Success,
        None,
        None,
        delivery_traces.last(),
    );

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
    for delivery in &delivery_traces {
        record_channel_delivery_outcome(state.as_ref(), &meta, delivery);
    }
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

async fn finalize_failure(
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
    let failure_kind = trace.failure.as_ref().map(|failure| failure.kind.clone());
    if let Some(record) = update_run_record(state.as_ref(), &meta.gateway_run_id, |record| {
        record.update_from_trace(&trace, Some(&trace_path));
    }) {
        sync_session_run_state(
            state.as_ref(),
            &record,
            RunLifecycleStatus::Failed,
            Some(trace.run_id.clone()),
            trace.error.clone(),
            failure_kind.clone(),
        );
        broadcast_envelope(state.as_ref(), run_record_envelope(&record));
    }
    let session_summary = update_gateway_session_metadata(
        &state,
        &meta,
        Some(trace.run_id.clone()),
        RunLifecycleStatus::Failed,
        trace.error.clone(),
        failure_kind.clone(),
        None,
    );

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

async fn finalize_canceled(
    state: Arc<GatewayState>,
    meta: GatewayRunMeta,
    request: &RunSubmission,
) -> Result<GatewayRunResult, GatewayRunError> {
    let components = state.snapshot_components();
    let mut trace = RunTrace::new_with_id(meta.run_id.clone(), request.input.clone());
    if let Some(session_id) = meta.session_id.clone() {
        trace.bind_session(session_id);
    }
    if let Some(ingress) = meta.ingress.clone() {
        trace.bind_ingress(ingress);
    }
    trace.bind_extensions(components.extensions.iter().map(extension_trace).collect());
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
    let cancel_reason = "run canceled by operator".to_owned();
    trace.finish_canceled(cancel_reason.clone());
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
    if let Some(record) = update_run_record(state.as_ref(), &meta.gateway_run_id, |record| {
        record.update_from_trace(&trace, Some(&trace_path));
    }) {
        sync_session_run_state(
            state.as_ref(),
            &record,
            RunLifecycleStatus::Canceled,
            Some(trace.run_id.clone()),
            trace.error.clone(),
            Some("canceled".to_owned()),
        );
        broadcast_envelope(state.as_ref(), run_record_envelope(&record));
    }
    let session_summary = update_gateway_session_metadata(
        &state,
        &meta,
        Some(trace.run_id.clone()),
        RunLifecycleStatus::Canceled,
        trace.error.clone(),
        Some("canceled".to_owned()),
        None,
    );
    increment_metric(state.as_ref(), |metrics| metrics.canceled_runs_total += 1);
    record_audit_event(
        state.as_ref(),
        "run.canceled",
        "canceled",
        cancel_reason.clone(),
        meta.session_id.clone(),
        Some(meta.gateway_run_id.clone()),
        Some(meta.correlation_id.clone()),
        meta.ingress.as_ref(),
        Some(trace_path.display().to_string()),
        false,
    );
    broadcast_envelope(
        state.as_ref(),
        meta.envelope(GatewayEvent::Runtime(RunEvent::RunCanceled {
            run_id: trace.run_id.clone(),
            reason: cancel_reason.clone(),
        })),
    );
    if let Some(summary) = session_summary.clone() {
        broadcast_envelope(
            state.as_ref(),
            meta.envelope(GatewayEvent::SessionUpdated {
                summary: session_summary_dto(&summary),
            }),
        );
    }

    Err(GatewayRunError {
        source: anyhow!(cancel_reason),
        trace,
        trace_path,
        gateway_run_id: meta.gateway_run_id,
        correlation_id: meta.correlation_id,
        session_route: meta.session_route,
    })
}

fn build_outbound_message(meta: &GatewayRunMeta, output: &str) -> Option<ChannelOutboundMessage> {
    let ingress = meta.ingress.as_ref()?;
    let channel = ingress.channel.clone()?;
    let adapter = ingress
        .adapter
        .clone()
        .unwrap_or_else(|| ingress.kind.clone());
    let conversation_id = ingress.conversation_id.clone()?;
    let reply_target = ingress.reply_target.clone()?;
    let session_id = meta.session_id.clone()?;

    Some(ChannelOutboundMessage {
        channel,
        adapter,
        bot_name: ingress.bot_name.clone(),
        bot_route: ingress.bot_route.clone(),
        bot_profile: ingress.bot_profile.clone(),
        bot_token_env: ingress.bot_token_env.clone(),
        conversation_id,
        reply_target,
        text: output.to_owned(),
        idempotency_key: format!("{}:{}", meta.gateway_run_id, meta.correlation_id),
        correlation_id: meta.correlation_id.clone(),
        gateway_run_id: meta.gateway_run_id.clone(),
        session_id,
    })
}

async fn dispatch_outbound_replies(
    state: &GatewayState,
    meta: &GatewayRunMeta,
    output: &str,
) -> Vec<ChannelDeliveryTrace> {
    let Some(message) = build_outbound_message(meta, output) else {
        return Vec::new();
    };

    match message.channel.as_str() {
        "telegram" => vec![dispatch_telegram_reply(state, message).await],
        _ => Vec::new(),
    }
}

async fn dispatch_telegram_reply(
    state: &GatewayState,
    message: ChannelOutboundMessage,
) -> ChannelDeliveryTrace {
    let resolved_bot = match resolved_telegram_bot_by_name(
        &state.snapshot_components(),
        message.bot_name.as_deref(),
    ) {
        Some(bot) => bot,
        None => {
            return ChannelDeliveryTrace {
                message,
                result: mosaic_inspect::ChannelDeliveryResult {
                    delivery_id: Uuid::new_v4().to_string(),
                    status: mosaic_inspect::ChannelDeliveryStatus::Failed,
                    provider_message_id: None,
                    retry_count: 0,
                    retryable: false,
                    error_kind: Some("auth".to_owned()),
                    error: Some(
                        "telegram outbound reply could not resolve a configured bot instance"
                            .to_owned(),
                    ),
                    delivered_at: None,
                },
            };
        }
    };

    let Some(client) = telegram_outbound_client_for_bot(&resolved_bot).unwrap_or_else(|err| {
        warn!(error = %err, bot = %resolved_bot.name, "failed to initialize telegram outbound client");
        None
    }) else {
        return ChannelDeliveryTrace {
            message,
            result: mosaic_inspect::ChannelDeliveryResult {
                delivery_id: Uuid::new_v4().to_string(),
                status: mosaic_inspect::ChannelDeliveryStatus::Failed,
                provider_message_id: None,
                retry_count: 0,
                retryable: false,
                error_kind: Some("auth".to_owned()),
                error: Some(
                    format!(
                        "telegram outbound replies require configured env `{}` for bot `{}`",
                        resolved_bot.bot_token_env, resolved_bot.name
                    ),
                ),
                delivered_at: None,
            },
        };
    };

    client.send_message(message).await
}

fn record_channel_delivery_outcome(
    state: &GatewayState,
    meta: &GatewayRunMeta,
    delivery: &ChannelDeliveryTrace,
) {
    let (kind, outcome, summary, event) = match delivery.result.status {
        mosaic_inspect::ChannelDeliveryStatus::Delivered => (
            "channel.outbound_delivered",
            "success",
            format!(
                "{} -> {}",
                delivery.message.reply_target,
                delivery
                    .result
                    .provider_message_id
                    .clone()
                    .unwrap_or_else(|| "<unknown>".to_owned())
            ),
            GatewayEvent::OutboundDelivered {
                delivery: delivery.clone(),
            },
        ),
        mosaic_inspect::ChannelDeliveryStatus::Failed => (
            "channel.outbound_failed",
            "failed",
            format!(
                "{} | kind={} | error={}",
                delivery.message.reply_target,
                delivery
                    .result
                    .error_kind
                    .clone()
                    .unwrap_or_else(|| "unknown".to_owned()),
                delivery
                    .result
                    .error
                    .clone()
                    .unwrap_or_else(|| "<unknown>".to_owned())
            ),
            GatewayEvent::OutboundFailed {
                delivery: delivery.clone(),
            },
        ),
    };

    record_audit_event(
        state,
        kind,
        outcome,
        summary,
        Some(delivery.message.session_id.clone()),
        Some(meta.gateway_run_id.clone()),
        Some(meta.correlation_id.clone()),
        meta.ingress.as_ref(),
        Some(delivery.message.reply_target.clone()),
        false,
    );
    broadcast_envelope(state, meta.envelope(event));
}

fn update_gateway_session_metadata(
    state: &GatewayState,
    meta: &GatewayRunMeta,
    run_id: Option<String>,
    status: RunLifecycleStatus,
    last_error: Option<String>,
    last_failure_kind: Option<String>,
    last_delivery: Option<&ChannelDeliveryTrace>,
) -> Option<SessionSummary> {
    let session_id = meta.session_id.as_deref()?;
    let components = state.snapshot_components();
    let mut session = components.session_store.load(session_id).ok()??;
    session.set_gateway_binding(
        meta.session_route.clone(),
        meta.gateway_run_id.clone(),
        meta.correlation_id.clone(),
    );
    session.set_run_state(
        status,
        run_id,
        Some(meta.gateway_run_id.clone()),
        Some(meta.correlation_id.clone()),
        last_error,
        last_failure_kind,
    );
    if let Some(delivery) = last_delivery {
        session.bind_delivery_context(delivery);
    }
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

mod attachments;
mod audit;
mod auth;
mod capability;
mod command_catalog;
mod dto;
mod http;
mod ingress;
mod runs;
#[cfg(test)]
mod tests;

use auth::*;
use dto::*;
pub use dto::{cron_registration_dto, run_response, session_detail_dto, session_summary_dto};
pub use http::{http_router, serve_http, serve_http_with_shutdown};
