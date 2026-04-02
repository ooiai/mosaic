use super::*;
use mosaic_config::{AttachmentConfig, RuntimePolicyConfig, TelegramAdapterConfig};
use mosaic_sandbox_core::SandboxManager;

pub(crate) type SharedToolTraceCollector = Arc<Mutex<Vec<ToolTrace>>>;
pub(crate) type SharedSkillTraceCollector = Arc<Mutex<Vec<SkillTrace>>>;
pub(crate) type SharedModelSelectionCollector = Arc<Mutex<Vec<ModelSelectionTrace>>>;
pub(crate) type SharedCapabilityTraceCollector = Arc<Mutex<Vec<CapabilityInvocationTrace>>>;

#[derive(Debug)]
pub(crate) struct ToolExecutionOutcome {
    pub(crate) output: String,
    pub(crate) tool_trace: ToolTrace,
    pub(crate) capability_trace: CapabilityInvocationTrace,
}

#[derive(Debug)]
pub(crate) struct ToolExecutionFailure {
    pub(crate) error: anyhow::Error,
    pub(crate) tool_trace: Option<ToolTrace>,
    pub(crate) capability_trace: Option<CapabilityInvocationTrace>,
}

#[derive(Debug, Clone)]
pub(crate) struct NodeTraceContext {
    pub(crate) node_id: Option<String>,
    pub(crate) capability_route: Option<String>,
    pub(crate) disconnect_context: Option<String>,
    pub(crate) node_attempted: bool,
    pub(crate) node_fallback_to_local: bool,
    pub(crate) node_failure_class: Option<String>,
    pub(crate) effective_execution_target: ExecutionTarget,
}

impl Default for NodeTraceContext {
    fn default() -> Self {
        Self {
            node_id: None,
            capability_route: None,
            disconnect_context: None,
            node_attempted: false,
            node_fallback_to_local: false,
            node_failure_class: None,
            effective_execution_target: ExecutionTarget::Local,
        }
    }
}

pub struct RuntimeContext {
    pub profiles: Arc<ProviderProfileRegistry>,
    pub provider_override: Option<Arc<dyn LlmProvider>>,
    pub session_store: Arc<dyn SessionStore>,
    pub memory_store: Arc<dyn MemoryStore>,
    pub memory_policy: MemoryPolicy,
    pub runtime_policy: RuntimePolicyConfig,
    pub attachments: AttachmentConfig,
    pub sandbox: Arc<SandboxManager>,
    pub telegram: TelegramAdapterConfig,
    pub app_name: Option<String>,
    pub tools: Arc<ToolRegistry>,
    pub skills: Arc<SkillRegistry>,
    pub workflows: Arc<WorkflowRegistry>,
    pub node_router: Option<Arc<dyn NodeRouter>>,
    pub active_extensions: Vec<ExtensionTrace>,
    pub event_sink: SharedRunEventSink,
}

pub struct RunRequest {
    pub run_id: Option<String>,
    pub system: Option<String>,
    pub input: String,
    pub tool: Option<String>,
    pub skill: Option<String>,
    pub workflow: Option<String>,
    pub session_id: Option<String>,
    pub profile: Option<String>,
    pub ingress: Option<IngressTrace>,
}

#[derive(Debug)]
pub struct RunResult {
    pub output: String,
    pub trace: RunTrace,
}

#[derive(Debug)]
pub struct RunError {
    source: anyhow::Error,
    trace: RunTrace,
}

impl RunError {
    pub(crate) fn new(source: anyhow::Error, trace: RunTrace) -> Self {
        Self { source, trace }
    }

    pub fn trace(&self) -> &RunTrace {
        &self.trace
    }

    pub fn into_parts(self) -> (anyhow::Error, RunTrace) {
        (self.source, self.trace)
    }
}

impl std::fmt::Display for RunError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.source)
    }
}

impl std::error::Error for RunError {}

pub struct AgentRuntime {
    pub(crate) ctx: RuntimeContext,
}
