use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Result, anyhow};
use chrono::{DateTime, Utc};
use mosaic_inspect::RunTrace;
use mosaic_provider::{LlmProvider, ProviderProfileRegistry};
use mosaic_runtime::events::{RunEvent, RunEventSink, SharedRunEventSink};
use mosaic_runtime::{AgentRuntime, RunError, RunRequest, RunResult, RuntimeContext};
use mosaic_session_core::{SessionRecord, SessionStore, SessionSummary, session_route_for_id};
use mosaic_skill_core::SkillRegistry;
use mosaic_tool_core::ToolRegistry;
use mosaic_workflow::WorkflowRegistry;
use serde::{Deserialize, Serialize};
use tokio::runtime::Handle;
use tokio::sync::broadcast;
use uuid::Uuid;

#[derive(Clone)]
pub struct GatewayRuntimeComponents {
    pub profiles: Arc<ProviderProfileRegistry>,
    pub provider_override: Option<Arc<dyn LlmProvider>>,
    pub session_store: Arc<dyn SessionStore>,
    pub tools: Arc<ToolRegistry>,
    pub skills: Arc<SkillRegistry>,
    pub workflows: Arc<WorkflowRegistry>,
    pub runs_dir: PathBuf,
}

impl GatewayRuntimeComponents {
    pub fn runtime_context(&self, event_sink: SharedRunEventSink) -> RuntimeContext {
        RuntimeContext {
            profiles: self.profiles.clone(),
            provider_override: self.provider_override.clone(),
            session_store: self.session_store.clone(),
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GatewayRunRequest {
    pub system: Option<String>,
    pub input: String,
    pub skill: Option<String>,
    pub workflow: Option<String>,
    pub session_id: Option<String>,
    pub profile: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum GatewayEvent {
    RunSubmitted { input: String, profile: String },
    Runtime(RunEvent),
    SessionUpdated { summary: SessionSummary },
    RunCompleted { output_preview: String },
    RunFailed { error: String },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GatewayEventEnvelope {
    pub gateway_run_id: String,
    pub correlation_id: String,
    pub session_id: Option<String>,
    pub session_route: String,
    pub emitted_at: DateTime<Utc>,
    pub event: GatewayEvent,
}

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
}

impl GatewayHandle {
    pub fn new_local(runtime_handle: Handle, components: GatewayRuntimeComponents) -> Self {
        let (events, _) = broadcast::channel(256);
        Self {
            inner: Arc::new(GatewayState {
                runtime_handle,
                components,
                events,
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
        };

        self.emit(meta.envelope(GatewayEvent::RunSubmitted {
            input: request.input.clone(),
            profile: resolved_profile,
        }));

        let state = self.inner.clone();
        let join = self.inner.runtime_handle.spawn(async move {
            let event_sink: SharedRunEventSink = Arc::new(GatewayRunEventSink {
                sender: state.events.clone(),
                meta: meta.clone(),
            });
            let runtime = AgentRuntime::new(state.components.runtime_context(event_sink));
            let run_request = RunRequest {
                system: request.system,
                input: request.input,
                skill: request.skill,
                workflow: request.workflow,
                session_id: meta.session_id.clone(),
                profile: request.profile,
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
}

impl RunEventSink for GatewayRunEventSink {
    fn emit(&self, event: RunEvent) {
        let _ = self
            .sender
            .send(self.meta.envelope(GatewayEvent::Runtime(event)));
    }
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
            .send(meta.envelope(GatewayEvent::SessionUpdated { summary }));
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
        error: source.to_string(),
    }));
    if let Some(summary) = session_summary {
        let _ = state
            .events
            .send(meta.envelope(GatewayEvent::SessionUpdated { summary }));
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

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::sync::Mutex;

    use mosaic_config::{MosaicConfig, ProviderProfileConfig};
    use mosaic_provider::MockProvider;
    use mosaic_session_core::{SessionStore, TranscriptRole};

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
                tools: Arc::new(tools),
                skills: Arc::new(SkillRegistry::new()),
                workflows: Arc::new(WorkflowRegistry::new()),
                runs_dir: std::env::temp_dir(),
            },
        )
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
            }
        }

        assert!(saw_submitted);
        assert!(saw_runtime);
        assert!(saw_session_updated);
        assert!(saw_completed);
    }
}
