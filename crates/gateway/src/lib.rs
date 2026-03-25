use std::{
    collections::BTreeMap,
    convert::Infallible,
    net::SocketAddr,
    path::PathBuf,
    sync::{Arc, Mutex},
    time::Duration,
};

use anyhow::{Result, anyhow, bail};
use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    response::sse::{Event as SseEvent, KeepAlive, Sse},
    routing::{get, post},
};
use chrono::Utc;
use futures::stream;
use mosaic_control_protocol::{
    CapabilityJobDto, CronRegistrationDto, CronRegistrationRequest, ErrorResponse,
    EventStreamEnvelope, ExecJobRequest, GatewayEvent, HealthResponse, InboundMessage, RunResponse,
    RunSubmission, SessionDetailDto, SessionGatewayDto, SessionSummaryDto, TranscriptMessageDto,
    TranscriptRoleDto, WebhookJobRequest,
};
use mosaic_inspect::{IngressTrace, RunTrace};
use mosaic_mcp_core::McpServerManager;
use mosaic_memory::{MemoryPolicy, MemoryStore};
use mosaic_provider::{LlmProvider, ProviderProfileRegistry, public_error_message};
use mosaic_runtime::events::{RunEvent, RunEventSink, SharedRunEventSink};
use mosaic_runtime::{AgentRuntime, RunError, RunRequest, RunResult, RuntimeContext};
use mosaic_scheduler_core::{CronRegistration, CronStore};
use mosaic_session_core::{
    SessionRecord, SessionStore, SessionSummary, TranscriptRole, session_route_for_id,
};
use mosaic_skill_core::SkillRegistry;
use mosaic_tool_core::ToolRegistry;
use mosaic_workflow::WorkflowRegistry;
use serde::{Deserialize, Serialize};
use tokio::runtime::Handle;
use tokio::sync::broadcast;
use uuid::Uuid;

pub use mosaic_control_protocol::{
    GatewayEvent as ProtocolGatewayEvent, HealthResponse as GatewayHealthResponse,
};

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
    pub mcp_manager: Option<Arc<McpServerManager>>,
    pub cron_store: Arc<dyn CronStore>,
    pub runs_dir: PathBuf,
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
            event_sink,
        }
    }
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

#[derive(Clone)]
pub struct GatewayHandle {
    inner: Arc<GatewayState>,
}

struct GatewayState {
    runtime_handle: Handle,
    components: GatewayRuntimeComponents,
    events: broadcast::Sender<GatewayEventEnvelope>,
    capability_jobs: Arc<Mutex<BTreeMap<String, CapabilityJobDto>>>,
}

impl GatewayHandle {
    pub fn new_local(runtime_handle: Handle, components: GatewayRuntimeComponents) -> Self {
        let (events, _) = broadcast::channel(256);
        Self {
            inner: Arc::new(GatewayState {
                runtime_handle,
                components,
                events,
                capability_jobs: Arc::new(Mutex::new(BTreeMap::new())),
            }),
        }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<GatewayEventEnvelope> {
        self.inner.events.subscribe()
    }

    pub fn list_sessions(&self) -> Result<Vec<SessionSummary>> {
        self.inner.components.session_store.list()
    }

    pub fn load_session(&self, id: &str) -> Result<Option<SessionRecord>> {
        self.inner.components.session_store.load(id)
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

    pub fn list_cron_registrations(&self) -> Result<Vec<CronRegistration>> {
        self.inner.components.cron_store.list()
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
        self.inner.components.cron_store.save(&registration)?;
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

    pub fn active_profile_name(&self) -> &str {
        self.inner.components.profiles.active_profile_name()
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

    pub async fn trigger_cron(&self, id: &str) -> Result<GatewayRunResult> {
        let mut registration = self
            .inner
            .components
            .cron_store
            .load(id)?
            .ok_or_else(|| anyhow!("cron not found: {id}"))?;
        registration.mark_triggered();
        self.inner.components.cron_store.save(&registration)?;
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
        let session_route = self.resolve_session_route(request.session_id.as_deref())?;
        let resolved_profile = request.profile.clone().unwrap_or_else(|| {
            self.inner
                .components
                .profiles
                .active_profile_name()
                .to_owned()
        });
        let meta = GatewayRunMeta {
            gateway_run_id: gateway_run_id.clone(),
            correlation_id: correlation_id.clone(),
            session_id: request.session_id.clone(),
            session_route: session_route.clone(),
            ingress: request.ingress.clone(),
        };

        self.emit(meta.envelope(GatewayEvent::RunSubmitted {
            input: request.input.clone(),
            profile: resolved_profile,
            ingress: request.ingress.clone(),
        }));

        let state = self.inner.clone();
        let join = self.inner.runtime_handle.spawn(async move {
            let event_sink: SharedRunEventSink = Arc::new(GatewayRunEventSink {
                sender: state.events.clone(),
                meta: meta.clone(),
                jobs: state.capability_jobs.clone(),
            });
            let runtime = AgentRuntime::new(state.components.runtime_context(event_sink));
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
        let _ = self.inner.events.send(envelope);
    }

    fn resolve_session_route(&self, session_id: Option<&str>) -> Result<String> {
        match session_id {
            Some(id) => Ok(self
                .load_session(id)?
                .and_then(|session| {
                    (!session.gateway.route.is_empty()).then_some(session.gateway.route)
                })
                .unwrap_or_else(|| session_route_for_id(id))),
            None => Ok("gateway.local/ephemeral".to_owned()),
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
    sender: broadcast::Sender<GatewayEventEnvelope>,
    meta: GatewayRunMeta,
    jobs: Arc<Mutex<BTreeMap<String, CapabilityJobDto>>>,
}

impl RunEventSink for GatewayRunEventSink {
    fn emit(&self, event: RunEvent) {
        if let Some(job) = update_runtime_capability_job(&self.jobs, &self.meta, &event) {
            let _ = self.sender.send(
                self.meta
                    .envelope(GatewayEvent::CapabilityJobUpdated { job }),
            );
        }
        let _ = self
            .sender
            .send(self.meta.envelope(GatewayEvent::Runtime(event)));
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
        .components
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

    let _ = state.events.send(GatewayEventEnvelope {
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
    });

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
    let mut trace = result.trace;
    trace.bind_gateway_context(
        meta.gateway_run_id.clone(),
        meta.correlation_id.clone(),
        meta.session_route.clone(),
    );
    let session_summary = update_gateway_session_metadata(&state, &meta);
    let trace_path = trace
        .save_to_dir(&state.components.runs_dir)
        .map_err(|err| GatewayRunError {
            source: err,
            trace: trace.clone(),
            trace_path: PathBuf::new(),
            gateway_run_id: meta.gateway_run_id.clone(),
            correlation_id: meta.correlation_id.clone(),
            session_route: meta.session_route.clone(),
        })?;

    let output_preview = truncate_preview(&result.output, 120);
    let _ = state
        .events
        .send(meta.envelope(GatewayEvent::RunCompleted { output_preview }));
    if let Some(summary) = session_summary.clone() {
        let _ = state
            .events
            .send(meta.envelope(GatewayEvent::SessionUpdated {
                summary: session_summary_dto(&summary),
            }));
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
    let (source, mut trace) = err.into_parts();
    trace.bind_gateway_context(
        meta.gateway_run_id.clone(),
        meta.correlation_id.clone(),
        meta.session_route.clone(),
    );
    let session_summary = update_gateway_session_metadata(&state, &meta);
    let trace_path = trace
        .save_to_dir(&state.components.runs_dir)
        .map_err(|save_err| GatewayRunError {
            source: save_err,
            trace: trace.clone(),
            trace_path: PathBuf::new(),
            gateway_run_id: meta.gateway_run_id.clone(),
            correlation_id: meta.correlation_id.clone(),
            session_route: meta.session_route.clone(),
        })?;

    let _ = state.events.send(meta.envelope(GatewayEvent::RunFailed {
        error: public_error_message(&source),
    }));
    if let Some(summary) = session_summary {
        let _ = state
            .events
            .send(meta.envelope(GatewayEvent::SessionUpdated {
                summary: session_summary_dto(&summary),
            }));
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
    let mut session = state.components.session_store.load(session_id).ok()??;
    session.set_gateway_binding(
        meta.session_route.clone(),
        meta.gateway_run_id.clone(),
        meta.correlation_id.clone(),
    );
    state.components.session_store.save(&session).ok()?;
    Some(session.summary())
}

fn truncate_preview(value: &str, limit: usize) -> String {
    if value.chars().count() <= limit {
        return value.to_owned();
    }

    let truncated: String = value.chars().take(limit).collect();
    format!("{truncated}...")
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
        last_gateway_run_id: summary.last_gateway_run_id.clone(),
        last_correlation_id: summary.last_correlation_id.clone(),
        message_count: summary.message_count,
        last_message_preview: summary.last_message_preview.clone(),
        memory_summary_preview: summary.memory_summary_preview.clone(),
        reference_count: summary.reference_count,
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
        .route("/sessions", get(http_list_sessions))
        .route("/sessions/{id}", get(http_get_session))
        .route("/capabilities/jobs", get(http_list_capability_jobs))
        .route("/capabilities/exec", post(http_exec_capability))
        .route("/capabilities/webhook", post(http_webhook_capability))
        .route("/runs", post(http_submit_run))
        .route("/cron", get(http_list_cron).post(http_register_cron))
        .route("/cron/{id}/trigger", post(http_trigger_cron))
        .route("/ingress/webchat", post(http_webchat_ingress))
        .route("/events", get(http_events))
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
    Json(HealthResponse {
        status: "ok".to_owned(),
        active_profile: state.gateway.active_profile_name().to_owned(),
        session_count: state
            .gateway
            .list_sessions()
            .map(|sessions| sessions.len())
            .unwrap_or(0),
        transport: "http+sse".to_owned(),
    })
}

async fn http_list_sessions(
    State(state): State<GatewayHttpState>,
) -> HttpResult<Vec<SessionSummaryDto>> {
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
) -> HttpResult<Vec<CapabilityJobDto>> {
    Ok(Json(state.gateway.list_capability_jobs()))
}

async fn http_exec_capability(
    State(state): State<GatewayHttpState>,
    Json(request): Json<ExecJobRequest>,
) -> HttpResult<CapabilityJobDto> {
    let job = state
        .gateway
        .run_exec_job(request)
        .await
        .map_err(http_internal_error)?;
    Ok(Json(job))
}

async fn http_webhook_capability(
    State(state): State<GatewayHttpState>,
    Json(request): Json<WebhookJobRequest>,
) -> HttpResult<CapabilityJobDto> {
    let job = state
        .gateway
        .run_webhook_job(request)
        .await
        .map_err(http_internal_error)?;
    Ok(Json(job))
}

async fn http_list_cron(
    State(state): State<GatewayHttpState>,
) -> HttpResult<Vec<CronRegistrationDto>> {
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
    Json(request): Json<CronRegistrationRequest>,
) -> HttpResult<CronRegistrationDto> {
    let registration = state
        .gateway
        .register_cron(request)
        .map_err(http_internal_error)?;
    Ok(Json(cron_registration_dto(&registration)))
}

async fn http_trigger_cron(
    Path(id): Path<String>,
    State(state): State<GatewayHttpState>,
) -> HttpResult<RunResponse> {
    let result = state
        .gateway
        .trigger_cron(&id)
        .await
        .map_err(http_internal_error)?;
    Ok(Json(run_response(result)))
}

async fn http_get_session(
    Path(id): Path<String>,
    State(state): State<GatewayHttpState>,
) -> HttpResult<SessionDetailDto> {
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
    Json(mut request): Json<RunSubmission>,
) -> HttpResult<RunResponse> {
    if request.ingress.is_none() {
        request.ingress = Some(IngressTrace {
            kind: "remote_operator".to_owned(),
            channel: Some("api".to_owned()),
            source: Some("mosaic-sdk".to_owned()),
            remote_addr: None,
            display_name: None,
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
    Json(message): Json<InboundMessage>,
) -> HttpResult<RunResponse> {
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

async fn http_events(
    State(state): State<GatewayHttpState>,
) -> Sse<impl futures::Stream<Item = std::result::Result<SseEvent, Infallible>>> {
    let receiver = state.gateway.subscribe();
    let stream = stream::unfold(receiver, |mut receiver| async move {
        match receiver.recv().await {
            Ok(envelope) => {
                let payload = serde_json::to_string(&envelope)
                    .unwrap_or_else(|_| "{\"error\":\"failed to encode event\"}".to_owned());
                Some((Ok(SseEvent::default().data(payload)), receiver))
            }
            Err(tokio::sync::broadcast::error::RecvError::Lagged(skipped)) => Some((
                Ok(SseEvent::default()
                    .event("lagged")
                    .data(skipped.to_string())),
                receiver,
            )),
            Err(tokio::sync::broadcast::error::RecvError::Closed) => None,
        }
    });

    Sse::new(stream).keep_alive(KeepAlive::default())
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
    use mosaic_config::{MosaicConfig, ProviderProfileConfig};
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
                mcp_manager: None,
                cron_store: test_cron_store(),
                runs_dir: std::env::temp_dir(),
            },
        )
    }

    fn mcp_script_path() -> String {
        format!(
            "{}/../../scripts/mock_mcp_server.py",
            env!("CARGO_MANIFEST_DIR")
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
            }
        }

        assert!(saw_submitted);
        assert!(saw_runtime);
        assert!(saw_session_updated);
        assert!(saw_completed);
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
                mcp_manager: Some(manager),
                cron_store: test_cron_store(),
                runs_dir: std::env::temp_dir(),
            },
        );

        assert_eq!(
            gateway
                .inner
                .components
                .mcp_manager
                .as_ref()
                .map(|manager| manager.server_count()),
            Some(1)
        );
        assert_eq!(
            gateway
                .inner
                .components
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
        assert_eq!(detail.gateway.route, "gateway.local/http-demo");
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
