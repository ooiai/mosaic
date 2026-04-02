use std::{
    collections::BTreeMap,
    fs, process,
    sync::{Arc, Mutex},
    time::Duration,
};

use anyhow::{Result, anyhow};
use async_trait::async_trait;
use chrono::Utc;
use mosaic_config::{AttachmentRouteModeConfig, MosaicConfig, ProviderProfileConfig};
use mosaic_inspect::{AttachmentKind, IngressTrace, OrchestrationOwner};
use mosaic_memory::{MemoryPolicy, MemorySearchHit, MemoryStore, SessionMemoryRecord};
use mosaic_node_protocol::{
    FileNodeStore, NodeCapabilityDeclaration, NodeCommandResultEnvelope, NodeRegistration,
};
use mosaic_provider::{
    CompletionResponse, LlmProvider, Message, MockProvider, ProviderCompletion, ProviderError,
    ProviderProfileRegistry, ProviderTransportMetadata, Role, ToolDefinition,
};
use mosaic_sandbox_core::{SandboxManager, SandboxSettings};
use mosaic_session_core::{
    SessionRecord, SessionStore, SessionSummary, TranscriptMessage, TranscriptRole,
};
use mosaic_skill_core::{MarkdownSkillPack, SkillOutput, SkillRegistry, SummarizeSkill};
use mosaic_tool_core::{
    CapabilityKind, CapabilityMetadata, PermissionScope, ReadFileTool, TimeNowTool, Tool,
    ToolMetadata, ToolRegistry, ToolResult, ToolRiskLevel, ToolSource,
};
use mosaic_workflow::{Workflow, WorkflowRegistry, WorkflowStep, WorkflowStepKind};

use super::{AgentRuntime, RunRequest, RuntimeContext};
use crate::events::{NoopEventSink, RunEvent, RunEventSink};

fn temp_dir(label: &str) -> std::path::PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock should be after epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("mosaic-runtime-{label}-{}-{nanos}", process::id()))
}

fn test_sandbox_manager() -> SandboxManager {
    let root = temp_dir("sandbox");
    let manager = SandboxManager::new(&root, SandboxSettings::default());
    manager
        .ensure_layout()
        .expect("sandbox layout should be created");
    manager
}

fn mock_profile_config() -> ProviderProfileConfig {
    ProviderProfileConfig {
        provider_type: "mock".to_owned(),
        model: "mock".to_owned(),
        base_url: None,
        api_key_env: None,
        transport: Default::default(),
        vendor: Default::default(),
        attachments: Default::default(),
    }
}

fn config_with_mock_active_profile() -> MosaicConfig {
    let mut config = MosaicConfig::default();
    config.active_profile = "mock".to_owned();
    config
        .profiles
        .insert("mock".to_owned(), mock_profile_config());
    config
}

#[derive(Default)]
struct VecEventSink {
    events: Mutex<Vec<RunEvent>>,
}

impl VecEventSink {
    fn snapshot(&self) -> Vec<RunEvent> {
        self.events
            .lock()
            .expect("event lock should not be poisoned")
            .clone()
    }
}

impl RunEventSink for VecEventSink {
    fn emit(&self, event: RunEvent) {
        self.events
            .lock()
            .expect("event lock should not be poisoned")
            .push(event);
    }
}

#[derive(Default)]
struct MemorySessionStore {
    sessions: Mutex<BTreeMap<String, SessionRecord>>,
}

impl MemorySessionStore {
    fn get(&self, id: &str) -> Option<SessionRecord> {
        self.sessions
            .lock()
            .expect("session lock should not be poisoned")
            .get(id)
            .cloned()
    }
}

impl SessionStore for MemorySessionStore {
    fn load(&self, id: &str) -> Result<Option<SessionRecord>> {
        Ok(self.get(id))
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

#[derive(Default)]
struct MemoryMemoryStore {
    sessions: Mutex<BTreeMap<String, SessionMemoryRecord>>,
}

impl MemoryMemoryStore {
    fn get(&self, id: &str) -> Option<SessionMemoryRecord> {
        self.sessions
            .lock()
            .expect("memory lock should not be poisoned")
            .get(id)
            .cloned()
    }
}

impl MemoryStore for MemoryMemoryStore {
    fn load_session(&self, session_id: &str) -> Result<Option<SessionMemoryRecord>> {
        Ok(self.get(session_id))
    }

    fn save_session(&self, record: &SessionMemoryRecord) -> Result<()> {
        self.sessions
            .lock()
            .expect("memory lock should not be poisoned")
            .insert(record.session_id.clone(), record.clone());
        Ok(())
    }

    fn list_sessions(&self) -> Result<Vec<SessionMemoryRecord>> {
        Ok(self
            .sessions
            .lock()
            .expect("memory lock should not be poisoned")
            .values()
            .cloned()
            .collect())
    }

    fn search(&self, query: &str, _tag: Option<&str>) -> Result<Vec<MemorySearchHit>> {
        let query = query.to_ascii_lowercase();
        let mut hits = Vec::new();
        for record in self
            .sessions
            .lock()
            .expect("memory lock should not be poisoned")
            .values()
        {
            if let Some(summary) = record.summary.as_deref() {
                if query.is_empty() || summary.to_ascii_lowercase().contains(&query) {
                    hits.push(MemorySearchHit {
                        session_id: record.session_id.clone(),
                        kind: "summary".to_owned(),
                        preview: summary.to_owned(),
                        tags: record.tags.clone(),
                        updated_at: record.updated_at,
                    });
                }
            }
        }
        Ok(hits)
    }
}

struct EmptyProvider;

#[derive(Default)]
struct RecordingProvider {
    messages: Mutex<Vec<Message>>,
}

impl RecordingProvider {
    fn latest_messages(&self) -> Vec<Message> {
        self.messages
            .lock()
            .expect("provider messages lock should not be poisoned")
            .clone()
    }
}

#[async_trait]
impl LlmProvider for EmptyProvider {
    fn metadata(&self) -> ProviderTransportMetadata {
        ProviderTransportMetadata {
            provider_type: "mock".to_owned(),
            base_url: None,
            timeout_ms: 0,
            max_retries: 0,
            retry_backoff_ms: 0,
            api_version: None,
            version_header: None,
            custom_header_keys: Vec::new(),
            supports_tool_call_shadow_messages: false,
            supports_vision: false,
            supports_documents: false,
            supports_audio: false,
            supports_video: false,
            preferred_attachment_mode: AttachmentRouteModeConfig::SpecializedProcessor,
        }
    }

    async fn complete(
        &self,
        _messages: &[Message],
        _tools: Option<&[ToolDefinition]>,
    ) -> std::result::Result<ProviderCompletion, ProviderError> {
        Ok(ProviderCompletion {
            response: CompletionResponse {
                message: None,
                tool_calls: vec![],
                finish_reason: Some("stop".to_owned()),
            },
            attempts: vec![mosaic_provider::ProviderAttempt {
                attempt: 1,
                max_attempts: 1,
                status: "success".to_owned(),
                error_kind: None,
                status_code: None,
                retryable: false,
                message: None,
            }],
        })
    }
}

#[async_trait]
impl LlmProvider for RecordingProvider {
    fn metadata(&self) -> ProviderTransportMetadata {
        ProviderTransportMetadata {
            provider_type: "mock".to_owned(),
            base_url: None,
            timeout_ms: 0,
            max_retries: 0,
            retry_backoff_ms: 0,
            api_version: None,
            version_header: None,
            custom_header_keys: Vec::new(),
            supports_tool_call_shadow_messages: false,
            supports_vision: true,
            supports_documents: true,
            supports_audio: false,
            supports_video: false,
            preferred_attachment_mode: AttachmentRouteModeConfig::ProviderNative,
        }
    }

    async fn complete(
        &self,
        messages: &[Message],
        _tools: Option<&[ToolDefinition]>,
    ) -> std::result::Result<ProviderCompletion, ProviderError> {
        *self
            .messages
            .lock()
            .expect("provider messages lock should not be poisoned") = messages.to_vec();
        Ok(ProviderCompletion {
            response: CompletionResponse {
                message: Some(Message {
                    role: Role::Assistant,
                    content: "vision response".to_owned(),
                    tool_call_id: None,
                    attachments: Vec::new(),
                }),
                tool_calls: vec![],
                finish_reason: Some("stop".to_owned()),
            },
            attempts: vec![mosaic_provider::ProviderAttempt {
                attempt: 1,
                max_attempts: 1,
                status: "success".to_owned(),
                error_kind: None,
                status_code: None,
                retryable: false,
                message: None,
            }],
        })
    }
}

struct FailingSkill;

struct AttachmentEchoSkill;

#[async_trait]
impl mosaic_skill_core::Skill for FailingSkill {
    fn name(&self) -> &str {
        "explode"
    }

    async fn execute(
        &self,
        _input: serde_json::Value,
        _ctx: &mosaic_skill_core::SkillContext,
    ) -> Result<mosaic_skill_core::SkillOutput> {
        Err(anyhow!("skill exploded"))
    }
}

#[async_trait]
impl mosaic_skill_core::Skill for AttachmentEchoSkill {
    fn name(&self) -> &str {
        "attachment_echo"
    }

    async fn execute(
        &self,
        input: serde_json::Value,
        _ctx: &mosaic_skill_core::SkillContext,
    ) -> Result<SkillOutput> {
        let attachment_count = input
            .get("attachments")
            .and_then(serde_json::Value::as_array)
            .map(|attachments| attachments.len())
            .unwrap_or(0);
        Ok(SkillOutput {
            content: format!("attachment count: {attachment_count}"),
            structured: Some(input),
        })
    }
}

struct FakeMcpReadFileTool {
    meta: ToolMetadata,
}

impl FakeMcpReadFileTool {
    fn new() -> Self {
        Self {
            meta: ToolMetadata::mcp(
                "filesystem",
                "read_file",
                "Read a UTF-8 text file from disk via MCP",
                serde_json::json!({
                    "type": "object",
                    "properties": {
                        "path": { "type": "string" }
                    },
                    "required": ["path"]
                }),
            ),
        }
    }
}

#[async_trait]
impl Tool for FakeMcpReadFileTool {
    fn metadata(&self) -> &ToolMetadata {
        &self.meta
    }

    async fn call(
        &self,
        input: serde_json::Value,
        _ctx: &mosaic_tool_core::ToolContext,
    ) -> Result<ToolResult> {
        let path = input
            .get("path")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("README.md");

        Ok(ToolResult {
            content: format!("remote mcp contents from {path}"),
            structured: Some(serde_json::json!({
                "path": path,
                "origin": "mcp",
            })),
            is_error: false,
            audit: None,
        })
    }
}

struct RequiredNodeTool {
    meta: ToolMetadata,
}

impl RequiredNodeTool {
    fn new() -> Self {
        Self {
            meta: ToolMetadata::builtin(
                "require_node_echo",
                "Only runs through a node route",
                serde_json::json!({
                    "type": "object",
                    "properties": {
                        "text": { "type": "string" }
                    }
                }),
            )
            .with_capability(CapabilityMetadata::exec().with_node_route(
                "exec_command",
                true,
                true,
            )),
        }
    }
}

#[async_trait]
impl Tool for RequiredNodeTool {
    fn metadata(&self) -> &ToolMetadata {
        &self.meta
    }

    async fn call(
        &self,
        input: serde_json::Value,
        _ctx: &mosaic_tool_core::ToolContext,
    ) -> Result<ToolResult> {
        Ok(ToolResult::ok(format!(
            "local fallback should not happen: {}",
            input
                .get("text")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("missing")
        )))
    }
}

fn runtime_with_provider(
    provider: Arc<dyn LlmProvider>,
    session_store: Arc<dyn SessionStore>,
    event_sink: Arc<dyn RunEventSink>,
) -> AgentRuntime {
    runtime_with_provider_and_workflows(
        provider,
        session_store,
        event_sink,
        WorkflowRegistry::new(),
    )
}

fn runtime_with_provider_and_workflows(
    provider: Arc<dyn LlmProvider>,
    session_store: Arc<dyn SessionStore>,
    event_sink: Arc<dyn RunEventSink>,
    workflows: WorkflowRegistry,
) -> AgentRuntime {
    let config = config_with_mock_active_profile();

    let profiles =
        ProviderProfileRegistry::from_config(&config).expect("profile registry should build");
    let mut tools = ToolRegistry::new();
    tools.register(Arc::new(TimeNowTool::new()));

    AgentRuntime::new(RuntimeContext {
        profiles: Arc::new(profiles),
        provider_override: Some(provider),
        session_store,
        memory_store: Arc::new(MemoryMemoryStore::default()),
        memory_policy: MemoryPolicy::default(),
        runtime_policy: MosaicConfig::default().runtime,
        attachments: MosaicConfig::default().attachments,
        sandbox: Arc::new(test_sandbox_manager()),
        telegram: Default::default(),
        app_name: None,
        tools: Arc::new(tools),
        skills: Arc::new(SkillRegistry::new()),
        workflows: Arc::new(workflows),
        node_router: None,
        active_extensions: Vec::new(),
        event_sink,
    })
}

fn event_names(events: &[RunEvent]) -> Vec<&'static str> {
    events
        .iter()
        .map(|event| match event {
            RunEvent::RunStarted { .. } => "RunStarted",
            RunEvent::WorkflowStarted { .. } => "WorkflowStarted",
            RunEvent::WorkflowStepStarted { .. } => "WorkflowStepStarted",
            RunEvent::WorkflowStepFinished { .. } => "WorkflowStepFinished",
            RunEvent::WorkflowStepFailed { .. } => "WorkflowStepFailed",
            RunEvent::WorkflowFinished { .. } => "WorkflowFinished",
            RunEvent::SkillStarted { .. } => "SkillStarted",
            RunEvent::SkillFinished { .. } => "SkillFinished",
            RunEvent::SkillFailed { .. } => "SkillFailed",
            RunEvent::ProviderRequest { .. } => "ProviderRequest",
            RunEvent::ProviderRetry { .. } => "ProviderRetry",
            RunEvent::ProviderFailed { .. } => "ProviderFailed",
            RunEvent::ToolCalling { .. } => "ToolCalling",
            RunEvent::ToolFinished { .. } => "ToolFinished",
            RunEvent::ToolFailed { .. } => "ToolFailed",
            RunEvent::CapabilityJobQueued { .. } => "CapabilityJobQueued",
            RunEvent::CapabilityJobStarted { .. } => "CapabilityJobStarted",
            RunEvent::CapabilityJobRetried { .. } => "CapabilityJobRetried",
            RunEvent::CapabilityJobFinished { .. } => "CapabilityJobFinished",
            RunEvent::CapabilityJobFailed { .. } => "CapabilityJobFailed",
            RunEvent::PermissionCheckFailed { .. } => "PermissionCheckFailed",
            RunEvent::OutputDelta { .. } => "OutputDelta",
            RunEvent::FinalAnswerReady { .. } => "FinalAnswerReady",
            RunEvent::RunFinished { .. } => "RunFinished",
            RunEvent::RunFailed { .. } => "RunFailed",
            RunEvent::RunCanceled { .. } => "RunCanceled",
        })
        .collect()
}

fn research_workflow() -> Workflow {
    Workflow {
        name: "research_brief".to_owned(),
        description: Some("Draft and summarize a short brief".to_owned()),
        visibility: mosaic_tool_core::CapabilityExposure::default(),
        steps: vec![
            WorkflowStep {
                name: "draft".to_owned(),
                kind: WorkflowStepKind::Prompt {
                    prompt: "Draft notes for: {{input}}".to_owned(),
                    system: Some("You are a concise researcher.".to_owned()),
                    tools: Vec::new(),
                    profile: None,
                },
            },
            WorkflowStep {
                name: "summarize".to_owned(),
                kind: WorkflowStepKind::Skill {
                    skill: "summarize".to_owned(),
                    input: "{{steps.draft.output}}".to_owned(),
                },
            },
        ],
    }
}

#[tokio::test]
async fn provider_only_run_returns_mock_output() {
    let sink = Arc::new(VecEventSink::default());
    let runtime = runtime_with_provider(
        Arc::new(MockProvider),
        Arc::new(MemorySessionStore::default()),
        sink.clone(),
    );

    let result = runtime
        .run(RunRequest {
            run_id: None,
            system: Some("You are helpful.".to_owned()),
            input: "Explain Mosaic.".to_owned(),
            tool: None,
            skill: None,
            workflow: None,
            session_id: None,
            profile: None,
            ingress: None,
        })
        .await
        .expect("runtime should succeed");

    assert_eq!(result.output, "mock response: Explain Mosaic.");
    assert_eq!(
        result
            .trace
            .effective_profile
            .as_ref()
            .map(|profile| profile.profile.as_str()),
        Some("mock")
    );

    assert_eq!(
        event_names(&sink.snapshot()),
        vec![
            "RunStarted",
            "ProviderRequest",
            "OutputDelta",
            "FinalAnswerReady",
            "RunFinished"
        ]
    );
}

#[tokio::test]
async fn run_records_ingress_metadata_in_trace() {
    let runtime = runtime_with_provider(
        Arc::new(MockProvider),
        Arc::new(MemorySessionStore::default()),
        Arc::new(NoopEventSink),
    );

    let result = runtime
        .run(RunRequest {
            run_id: None,
            system: Some("You are helpful.".to_owned()),
            input: "hello ingress".to_owned(),
            tool: None,
            skill: None,
            workflow: None,
            session_id: Some("ingress-demo".to_owned()),
            profile: None,
            ingress: Some(IngressTrace {
                kind: "remote_operator".to_owned(),
                channel: Some("cli".to_owned()),
                adapter: Some("cli_remote".to_owned()),
                bot_name: None,
                bot_route: None,
                bot_profile: None,
                bot_token_env: None,
                bot_secret_env: None,
                source: Some("mosaic-cli".to_owned()),
                remote_addr: Some("127.0.0.1".to_owned()),
                display_name: Some("operator".to_owned()),
                actor_id: Some("operator-1".to_owned()),
                conversation_id: Some("cli:operator-1".to_owned()),
                thread_id: Some("incident-7".to_owned()),
                thread_title: Some("Incident 7".to_owned()),
                reply_target: Some("cli:operator-1".to_owned()),
                message_id: Some("message-1".to_owned()),
                received_at: Some(Utc::now()),
                raw_event_id: Some("event-1".to_owned()),
                session_hint: Some("ingress-demo".to_owned()),
                profile_hint: None,
                control_command: None,
                original_text: None,
                attachments: Vec::new(),
                attachment_failures: Vec::new(),
                gateway_url: Some("http://127.0.0.1:8080".to_owned()),
            }),
        })
        .await
        .expect("runtime should succeed");

    assert_eq!(result.trace.session_id.as_deref(), Some("ingress-demo"));
    assert_eq!(
        result
            .trace
            .ingress
            .as_ref()
            .map(|ingress| ingress.kind.as_str()),
        Some("remote_operator")
    );
    assert_eq!(
        result
            .trace
            .ingress
            .as_ref()
            .and_then(|ingress| ingress.gateway_url.as_deref()),
        Some("http://127.0.0.1:8080")
    );
    assert_eq!(
        result
            .trace
            .ingress
            .as_ref()
            .and_then(|ingress| ingress.actor_id.as_deref()),
        Some("operator-1")
    );
    assert_eq!(
        result
            .trace
            .ingress
            .as_ref()
            .and_then(|ingress| ingress.thread_id.as_deref()),
        Some("incident-7")
    );
}

#[tokio::test]
async fn conversational_skill_auto_route_records_route_decision() {
    let config = config_with_mock_active_profile();
    let profiles =
        ProviderProfileRegistry::from_config(&config).expect("profile registry should build");
    let mut skills = SkillRegistry::new();
    skills.register_native(Arc::new(SummarizeSkill));

    let runtime = AgentRuntime::new(RuntimeContext {
        profiles: Arc::new(profiles),
        provider_override: Some(Arc::new(MockProvider)),
        session_store: Arc::new(MemorySessionStore::default()),
        memory_store: Arc::new(MemoryMemoryStore::default()),
        memory_policy: MemoryPolicy::default(),
        runtime_policy: MosaicConfig::default().runtime,
        attachments: MosaicConfig::default().attachments,
        sandbox: Arc::new(test_sandbox_manager()),
        telegram: Default::default(),
        app_name: None,
        tools: Arc::new(ToolRegistry::new()),
        skills: Arc::new(skills),
        workflows: Arc::new(WorkflowRegistry::new()),
        node_router: None,
        active_extensions: Vec::new(),
        event_sink: Arc::new(NoopEventSink),
    });

    let result = runtime
        .run(RunRequest {
            run_id: None,
            system: None,
            input: "please summarize this transcript".to_owned(),
            tool: None,
            skill: None,
            workflow: None,
            session_id: Some("auto-skill".to_owned()),
            profile: None,
            ingress: None,
        })
        .await
        .expect("auto-routed skill run should succeed");

    assert_eq!(
        result
            .trace
            .route_decision
            .as_ref()
            .and_then(|route| route.selected_skill.as_deref()),
        Some("summarize")
    );
    assert_eq!(
        result
            .trace
            .route_decision
            .as_ref()
            .map(|route| route.route_mode.label()),
        Some("skill")
    );
    assert_eq!(
        result
            .trace
            .route_decision
            .as_ref()
            .and_then(|route| route.route_kind)
            .map(|kind| kind.label()),
        Some("skill")
    );
    assert_eq!(
        result
            .trace
            .route_decision
            .as_ref()
            .and_then(|route| route.capability_source_kind)
            .map(|kind| kind.label()),
        Some("native_skill")
    );
    assert_eq!(
        result
            .trace
            .route_decision
            .as_ref()
            .and_then(|route| route.execution_target)
            .map(|target| target.label()),
        Some("local")
    );
    assert!(result.output.starts_with("summary:"));
}

#[tokio::test]
async fn default_messages_remain_on_assistant_route() {
    let runtime = runtime_with_provider(
        Arc::new(MockProvider),
        Arc::new(MemorySessionStore::default()),
        Arc::new(NoopEventSink),
    );

    let result = runtime
        .run(RunRequest {
            run_id: None,
            system: None,
            input: "say hello to the operator".to_owned(),
            tool: None,
            skill: None,
            workflow: None,
            session_id: None,
            profile: None,
            ingress: None,
        })
        .await
        .expect("assistant run should succeed");

    assert_eq!(
        result
            .trace
            .route_decision
            .as_ref()
            .map(|route| route.route_mode.label()),
        Some("assistant")
    );
    assert_eq!(
        result
            .trace
            .route_decision
            .as_ref()
            .and_then(|route| route.route_kind)
            .map(|kind| kind.label()),
        Some("assistant")
    );
    assert_eq!(
        result
            .trace
            .route_decision
            .as_ref()
            .and_then(|route| route.execution_target)
            .map(|target| target.label()),
        Some("provider")
    );
    assert_eq!(
        result
            .trace
            .route_decision
            .as_ref()
            .map(|route| route.selection_reason.as_str()),
        Some("default assistant path: no conversational capability matched")
    );
}

#[tokio::test]
async fn assistant_runs_with_attachments_use_provider_native_multimodal_route() {
    let provider = Arc::new(RecordingProvider::default());
    let runtime = runtime_with_provider(
        provider.clone(),
        Arc::new(MemorySessionStore::default()),
        Arc::new(NoopEventSink),
    );

    let result = runtime
        .run(RunRequest {
            run_id: None,
            system: None,
            input: "describe this image".to_owned(),
            tool: None,
            skill: None,
            workflow: None,
            session_id: None,
            profile: None,
            ingress: Some(IngressTrace {
                kind: "telegram".to_owned(),
                channel: Some("telegram".to_owned()),
                adapter: Some("telegram_webhook".to_owned()),
                bot_name: None,
                bot_route: None,
                bot_profile: None,
                bot_token_env: None,
                bot_secret_env: None,
                source: Some("telegram".to_owned()),
                remote_addr: None,
                display_name: Some("Operator".to_owned()),
                actor_id: Some("17".to_owned()),
                conversation_id: Some("telegram:chat:1".to_owned()),
                thread_id: None,
                thread_title: None,
                reply_target: Some("telegram:chat:1:message:10".to_owned()),
                message_id: Some("10".to_owned()),
                received_at: Some(Utc::now()),
                raw_event_id: Some("event-attach-1".to_owned()),
                session_hint: None,
                profile_hint: None,
                control_command: None,
                original_text: None,
                attachments: vec![mosaic_inspect::ChannelAttachment {
                    id: "img-1".to_owned(),
                    kind: AttachmentKind::Image,
                    filename: Some("photo.jpg".to_owned()),
                    mime_type: Some("image/jpeg".to_owned()),
                    size_bytes: Some(2048),
                    source_ref: Some("telegram:file_id:img-1".to_owned()),
                    remote_url: Some("telegram:file_path:files/photo.jpg".to_owned()),
                    local_cache_path: Some("/tmp/photo.jpg".to_owned()),
                    caption: Some("operator photo".to_owned()),
                }],
                attachment_failures: vec![],
                gateway_url: None,
            }),
        })
        .await
        .expect("multimodal assistant run should succeed");

    assert_eq!(
        result
            .trace
            .attachment_route
            .as_ref()
            .map(|route| route.mode.label()),
        Some("provider_native")
    );
    assert_eq!(
        result
            .trace
            .attachment_route
            .as_ref()
            .and_then(|route| route.provider_profile.as_deref()),
        Some("mock")
    );
    assert_eq!(
        result
            .trace
            .effective_profile
            .as_ref()
            .map(|profile| profile.supports_vision),
        Some(true)
    );

    let messages = provider.latest_messages();
    let user_message = messages
        .iter()
        .find(|message| matches!(message.role, Role::User))
        .expect("user message should be present");
    assert_eq!(user_message.attachments.len(), 1);
    assert_eq!(user_message.attachments[0].id, "img-1");
}

#[tokio::test]
async fn attachments_can_route_to_specialized_processor_skills() {
    let mut config = config_with_mock_active_profile();
    config.attachments.routing.default.mode = AttachmentRouteModeConfig::SpecializedProcessor;
    config.attachments.routing.default.processor = Some("attachment_echo".to_owned());

    let profiles =
        ProviderProfileRegistry::from_config(&config).expect("profile registry should build");
    let mut skills = SkillRegistry::new();
    skills.register_native_with_metadata(
        Arc::new(AttachmentEchoSkill),
        mosaic_skill_core::SkillMetadata::native("attachment_echo").with_exposure(
            mosaic_tool_core::CapabilityExposure::default().with_accepts_attachments(true),
        ),
    );

    let runtime = AgentRuntime::new(RuntimeContext {
        profiles: Arc::new(profiles),
        provider_override: Some(Arc::new(EmptyProvider)),
        session_store: Arc::new(MemorySessionStore::default()),
        memory_store: Arc::new(MemoryMemoryStore::default()),
        memory_policy: MemoryPolicy::default(),
        runtime_policy: config.runtime.clone(),
        attachments: config.attachments.clone(),
        sandbox: Arc::new(test_sandbox_manager()),
        telegram: Default::default(),
        app_name: None,
        tools: Arc::new(ToolRegistry::new()),
        skills: Arc::new(skills),
        workflows: Arc::new(WorkflowRegistry::new()),
        node_router: None,
        active_extensions: Vec::new(),
        event_sink: Arc::new(NoopEventSink),
    });

    let result = runtime
        .run(RunRequest {
            run_id: None,
            system: None,
            input: String::new(),
            tool: None,
            skill: None,
            workflow: None,
            session_id: None,
            profile: None,
            ingress: Some(IngressTrace {
                kind: "telegram".to_owned(),
                channel: Some("telegram".to_owned()),
                adapter: Some("telegram_webhook".to_owned()),
                bot_name: None,
                bot_route: None,
                bot_profile: None,
                bot_token_env: None,
                bot_secret_env: None,
                source: Some("telegram".to_owned()),
                remote_addr: None,
                display_name: Some("Operator".to_owned()),
                actor_id: Some("17".to_owned()),
                conversation_id: Some("telegram:chat:1".to_owned()),
                thread_id: None,
                thread_title: None,
                reply_target: Some("telegram:chat:1:message:11".to_owned()),
                message_id: Some("11".to_owned()),
                received_at: Some(Utc::now()),
                raw_event_id: Some("event-attach-2".to_owned()),
                session_hint: None,
                profile_hint: None,
                control_command: None,
                original_text: None,
                attachments: vec![mosaic_inspect::ChannelAttachment {
                    id: "doc-1".to_owned(),
                    kind: AttachmentKind::Document,
                    filename: Some("notes.txt".to_owned()),
                    mime_type: Some("text/plain".to_owned()),
                    size_bytes: Some(128),
                    source_ref: Some("telegram:file_id:doc-1".to_owned()),
                    remote_url: None,
                    local_cache_path: None,
                    caption: None,
                }],
                attachment_failures: vec![],
                gateway_url: None,
            }),
        })
        .await
        .expect("specialized processor route should succeed");

    assert_eq!(result.output, "attachment count: 1");
    assert_eq!(
        result
            .trace
            .route_decision
            .as_ref()
            .and_then(|route| route.selected_skill.as_deref()),
        Some("attachment_echo")
    );
    assert_eq!(
        result
            .trace
            .attachment_route
            .as_ref()
            .map(|route| route.mode.label()),
        Some("specialized_processor")
    );
    assert_eq!(
        result
            .trace
            .attachment_route
            .as_ref()
            .and_then(|route| route.processor.as_deref()),
        Some("attachment_echo")
    );
}

#[tokio::test]
async fn bot_attachment_policy_overrides_workspace_defaults_and_records_scope() {
    let provider = Arc::new(RecordingProvider::default());
    let mut config = config_with_mock_active_profile();
    config.profiles.insert(
        "gpt-5.4".to_owned(),
        ProviderProfileConfig {
            provider_type: "openai".to_owned(),
            model: "gpt-5.4".to_owned(),
            base_url: Some("https://api.openai.com/v1".to_owned()),
            api_key_env: Some("OPENAI_API_KEY".to_owned()),
            transport: Default::default(),
            vendor: Default::default(),
            attachments: Default::default(),
        },
    );
    config.attachments.routing.default.mode = AttachmentRouteModeConfig::SpecializedProcessor;
    config.attachments.routing.default.processor = Some("attachment_echo".to_owned());
    config.telegram.bots.insert(
        "primary".to_owned(),
        mosaic_config::TelegramBotConfig {
            bot_token_env: "MOSAIC_TELEGRAM_BOT_TOKEN".to_owned(),
            attachments: Some(mosaic_config::AttachmentRoutingTargetConfig {
                mode: AttachmentRouteModeConfig::ProviderNative,
                processor: None,
                multimodal_profile: Some("gpt-5.4".to_owned()),
                specialized_processor_profile: None,
                allowed_attachment_kinds: vec![mosaic_config::AttachmentKindConfig::Image],
                max_attachment_size_mb: Some(5),
            }),
            ..mosaic_config::TelegramBotConfig::default()
        },
    );

    let profiles =
        ProviderProfileRegistry::from_config(&config).expect("profile registry should build");
    let runtime = AgentRuntime::new(RuntimeContext {
        profiles: Arc::new(profiles),
        provider_override: Some(provider.clone()),
        session_store: Arc::new(MemorySessionStore::default()),
        memory_store: Arc::new(MemoryMemoryStore::default()),
        memory_policy: MemoryPolicy::default(),
        runtime_policy: config.runtime.clone(),
        attachments: config.attachments.clone(),
        sandbox: Arc::new(test_sandbox_manager()),
        telegram: config.telegram.clone(),
        app_name: None,
        tools: Arc::new(ToolRegistry::new()),
        skills: Arc::new(SkillRegistry::new()),
        workflows: Arc::new(WorkflowRegistry::new()),
        node_router: None,
        active_extensions: Vec::new(),
        event_sink: Arc::new(NoopEventSink),
    });

    let result = runtime
        .run(RunRequest {
            run_id: None,
            system: None,
            input: "describe this image".to_owned(),
            tool: None,
            skill: None,
            workflow: None,
            session_id: None,
            profile: None,
            ingress: Some(IngressTrace {
                kind: "telegram".to_owned(),
                channel: Some("telegram".to_owned()),
                adapter: Some("telegram_webhook".to_owned()),
                bot_name: Some("primary".to_owned()),
                bot_route: Some("primary".to_owned()),
                bot_profile: Some("gpt-5.4".to_owned()),
                bot_token_env: Some("MOSAIC_TELEGRAM_BOT_TOKEN".to_owned()),
                bot_secret_env: None,
                source: Some("telegram".to_owned()),
                remote_addr: None,
                display_name: Some("Operator".to_owned()),
                actor_id: Some("17".to_owned()),
                conversation_id: Some("telegram:chat:1".to_owned()),
                thread_id: None,
                thread_title: None,
                reply_target: Some("telegram:chat:1:message:12".to_owned()),
                message_id: Some("12".to_owned()),
                received_at: Some(Utc::now()),
                raw_event_id: Some("event-attach-bot-1".to_owned()),
                session_hint: None,
                profile_hint: None,
                control_command: None,
                original_text: None,
                attachments: vec![mosaic_inspect::ChannelAttachment {
                    id: "img-1".to_owned(),
                    kind: AttachmentKind::Image,
                    filename: Some("photo.jpg".to_owned()),
                    mime_type: Some("image/jpeg".to_owned()),
                    size_bytes: Some(1024),
                    source_ref: Some("telegram:file_id:img-1".to_owned()),
                    remote_url: Some("telegram:file_path:files/photo.jpg".to_owned()),
                    local_cache_path: Some("/tmp/photo.jpg".to_owned()),
                    caption: Some("operator photo".to_owned()),
                }],
                attachment_failures: vec![],
                gateway_url: None,
            }),
        })
        .await
        .expect("bot policy multimodal run should succeed");

    assert_eq!(
        result
            .trace
            .attachment_route
            .as_ref()
            .and_then(|route| route.policy_scope.as_deref()),
        Some("bot:primary")
    );
    assert_eq!(
        result
            .trace
            .attachment_route
            .as_ref()
            .and_then(|route| route.selected_profile.as_deref()),
        Some("gpt-5.4")
    );
    assert_eq!(
        result
            .trace
            .attachment_route
            .as_ref()
            .and_then(|route| route.bot_identity.as_deref()),
        Some("primary")
    );
    assert_eq!(
        result
            .trace
            .effective_profile
            .as_ref()
            .map(|profile| profile.profile.as_str()),
        Some("gpt-5.4")
    );
}

#[tokio::test]
async fn explicit_tools_reject_attachments_when_metadata_disallows_them() {
    let runtime = runtime_with_provider(
        Arc::new(EmptyProvider),
        Arc::new(MemorySessionStore::default()),
        Arc::new(NoopEventSink),
    );

    let error = runtime
        .run(RunRequest {
            run_id: None,
            system: None,
            input: String::new(),
            tool: Some("time_now".to_owned()),
            skill: None,
            workflow: None,
            session_id: None,
            profile: None,
            ingress: Some(IngressTrace {
                kind: "telegram".to_owned(),
                channel: Some("telegram".to_owned()),
                adapter: Some("telegram_webhook".to_owned()),
                bot_name: None,
                bot_route: None,
                bot_profile: None,
                bot_token_env: None,
                bot_secret_env: None,
                source: Some("telegram".to_owned()),
                remote_addr: None,
                display_name: Some("Operator".to_owned()),
                actor_id: Some("17".to_owned()),
                conversation_id: Some("telegram:chat:1".to_owned()),
                thread_id: None,
                thread_title: None,
                reply_target: Some("telegram:chat:1:message:12".to_owned()),
                message_id: Some("12".to_owned()),
                received_at: Some(Utc::now()),
                raw_event_id: Some("event-attach-3".to_owned()),
                session_hint: None,
                profile_hint: None,
                control_command: None,
                original_text: None,
                attachments: vec![mosaic_inspect::ChannelAttachment {
                    id: "img-2".to_owned(),
                    kind: AttachmentKind::Image,
                    filename: Some("photo.jpg".to_owned()),
                    mime_type: Some("image/jpeg".to_owned()),
                    size_bytes: Some(256),
                    source_ref: Some("telegram:file_id:img-2".to_owned()),
                    remote_url: None,
                    local_cache_path: None,
                    caption: None,
                }],
                attachment_failures: vec![],
                gateway_url: None,
            }),
        })
        .await
        .expect_err("tool route should reject attachments");

    assert!(
        error
            .to_string()
            .contains("tool 'time_now' does not accept attachments")
    );
}

#[tokio::test]
async fn session_runs_roundtrip_transcript_messages() {
    let store = Arc::new(MemorySessionStore::default());
    let runtime = runtime_with_provider(
        Arc::new(MockProvider),
        store.clone(),
        Arc::new(NoopEventSink),
    );

    runtime
        .run(RunRequest {
            run_id: None,
            system: Some("You are helpful.".to_owned()),
            input: "hello".to_owned(),
            tool: None,
            skill: None,
            workflow: None,
            session_id: Some("demo".to_owned()),
            profile: None,
            ingress: None,
        })
        .await
        .expect("first run should succeed");

    runtime
        .run(RunRequest {
            run_id: None,
            system: Some("You are helpful.".to_owned()),
            input: "second turn".to_owned(),
            tool: None,
            skill: None,
            workflow: None,
            session_id: Some("demo".to_owned()),
            profile: None,
            ingress: None,
        })
        .await
        .expect("second run should succeed");

    let session = store.get("demo").expect("session should exist");
    let transcript_roles = session
        .transcript
        .iter()
        .map(|message| &message.role)
        .collect::<Vec<_>>();

    assert_eq!(session.provider_profile, "mock");
    assert!(session.last_run_id.is_some());
    assert_eq!(
        transcript_roles,
        vec![
            &TranscriptRole::System,
            &TranscriptRole::User,
            &TranscriptRole::Assistant,
            &TranscriptRole::User,
            &TranscriptRole::Assistant,
        ]
    );
}

#[tokio::test]
async fn runtime_session_persistence_leaves_gateway_lifecycle_fields_unset() {
    let store = Arc::new(MemorySessionStore::default());
    let runtime = runtime_with_provider(
        Arc::new(MockProvider),
        store.clone(),
        Arc::new(NoopEventSink),
    );

    let result = runtime
        .run(RunRequest {
            run_id: None,
            system: Some("You are helpful.".to_owned()),
            input: "runtime owns transcript and memory facts".to_owned(),
            tool: None,
            skill: None,
            workflow: None,
            session_id: Some("writer-demo".to_owned()),
            profile: None,
            ingress: None,
        })
        .await
        .expect("runtime session run should succeed");

    let session = store.get("writer-demo").expect("session should exist");

    assert_eq!(
        session.last_run_id.as_deref(),
        Some(result.trace.run_id.as_str())
    );
    assert!(
        session
            .transcript
            .iter()
            .any(|message| message.role == TranscriptRole::Assistant)
    );
    assert!(session.gateway.last_gateway_run_id.is_none());
    assert!(session.gateway.last_correlation_id.is_none());
    assert_eq!(
        session.run.status,
        mosaic_inspect::RunLifecycleStatus::Unknown
    );
    assert!(session.run.current_gateway_run_id.is_none());
    assert!(session.run.current_correlation_id.is_none());
}

#[tokio::test]
async fn tool_loop_executes_time_now_and_records_tool_trace() {
    let sink = Arc::new(VecEventSink::default());
    let runtime = runtime_with_provider(
        Arc::new(MockProvider),
        Arc::new(MemorySessionStore::default()),
        sink.clone(),
    );

    let result = runtime
        .run(RunRequest {
            run_id: None,
            system: Some("Use tools when needed.".to_owned()),
            input: "What time is it now?".to_owned(),
            tool: None,
            skill: None,
            workflow: None,
            session_id: Some("time-demo".to_owned()),
            profile: None,
            ingress: None,
        })
        .await
        .expect("tool loop should succeed");

    assert!(result.output.starts_with("The current time is: "));
    assert_eq!(result.trace.tool_calls.len(), 1);
    assert_eq!(result.trace.session_id.as_deref(), Some("time-demo"));
    assert_eq!(result.trace.tool_calls[0].source, ToolSource::Builtin);
    assert_eq!(result.trace.capability_invocations.len(), 1);
    assert_eq!(result.trace.capability_invocations[0].tool_name, "time_now");
    assert_eq!(
        event_names(&sink.snapshot()),
        vec![
            "RunStarted",
            "ProviderRequest",
            "ToolCalling",
            "CapabilityJobQueued",
            "CapabilityJobStarted",
            "CapabilityJobFinished",
            "ToolFinished",
            "ProviderRequest",
            "OutputDelta",
            "FinalAnswerReady",
            "RunFinished",
        ]
    );
}

#[tokio::test]
async fn tool_loop_records_mcp_tool_source_for_remote_tools() {
    let config = config_with_mock_active_profile();
    let profiles =
        ProviderProfileRegistry::from_config(&config).expect("profile registry should build");
    let mut tools = ToolRegistry::new();
    tools.register(Arc::new(FakeMcpReadFileTool::new()));
    let runtime = AgentRuntime::new(RuntimeContext {
        profiles: Arc::new(profiles),
        provider_override: Some(Arc::new(MockProvider)),
        session_store: Arc::new(MemorySessionStore::default()),
        memory_store: Arc::new(MemoryMemoryStore::default()),
        memory_policy: MemoryPolicy::default(),
        runtime_policy: MosaicConfig::default().runtime,
        attachments: MosaicConfig::default().attachments,
        sandbox: Arc::new(test_sandbox_manager()),
        telegram: Default::default(),
        app_name: None,
        tools: Arc::new(tools),
        skills: Arc::new(SkillRegistry::new()),
        workflows: Arc::new(WorkflowRegistry::new()),
        node_router: None,
        active_extensions: Vec::new(),
        event_sink: Arc::new(NoopEventSink),
    });

    let result = runtime
        .run(RunRequest {
            run_id: None,
            system: Some("Use tools when needed.".to_owned()),
            input: "Read a file for me.".to_owned(),
            tool: None,
            skill: None,
            workflow: None,
            session_id: None,
            profile: None,
            ingress: None,
        })
        .await
        .expect("remote MCP tool loop should succeed");

    assert_eq!(result.trace.tool_calls.len(), 1);
    assert_eq!(result.trace.tool_calls[0].name, "mcp.filesystem.read_file");
    assert_eq!(
        result.trace.tool_calls[0].source,
        ToolSource::Mcp {
            server: "filesystem".to_owned(),
            remote_tool: "read_file".to_owned(),
        }
    );
    assert_eq!(
        result.trace.tool_calls[0]
            .capability_source_kind
            .map(|kind| kind.label()),
        Some("mcp")
    );
    assert_eq!(
        result.trace.tool_calls[0].execution_target.label(),
        "mcp_server"
    );
    assert!(
        result
            .output
            .starts_with("I read the file successfully. Preview:\n")
    );
}

#[tokio::test]
async fn tool_loop_routes_read_file_via_node_when_affinity_is_present() {
    let config = config_with_mock_active_profile();
    let profiles =
        ProviderProfileRegistry::from_config(&config).expect("profile registry should build");
    let workspace_root = std::env::temp_dir().join(format!(
        "mosaic-runtime-node-tests-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&workspace_root).expect("workspace root should be created");
    std::fs::write(workspace_root.join("README.md"), "node-routed contents")
        .expect("README should be written");

    let mut tools = ToolRegistry::new();
    tools.register(Arc::new(ReadFileTool::new_with_allowed_roots(vec![
        workspace_root.clone(),
    ])));
    let node_store = Arc::new(FileNodeStore::new(workspace_root.join(".mosaic/nodes")));
    node_store
        .register_node(&NodeRegistration::new(
            "node-a",
            "Headless Node",
            "file-bus",
            "headless",
            vec![NodeCapabilityDeclaration {
                name: "read_file".to_owned(),
                kind: CapabilityKind::File,
                permission_scopes: vec![PermissionScope::LocalRead],
                risk: ToolRiskLevel::Low,
            }],
        ))
        .expect("node registration should persist");
    node_store
        .attach_session("node-demo", "node-a")
        .expect("node affinity should persist");

    let worker_store = node_store.clone();
    let worker_root = workspace_root.clone();
    let worker = tokio::spawn(async move {
        let tool = ReadFileTool::new_with_allowed_roots(vec![worker_root]);
        loop {
            let pending = worker_store
                .pending_commands("node-a")
                .expect("pending commands should load");
            if let Some(dispatch) = pending.into_iter().next() {
                let result = tool
                    .call(
                        serde_json::json!({
                            "path": workspace_root.join("README.md").display().to_string(),
                        }),
                        &mosaic_tool_core::ToolContext::default(),
                    )
                    .await
                    .expect("node read_file should succeed");
                worker_store
                    .complete_command(&NodeCommandResultEnvelope::success(&dispatch, result))
                    .expect("node result should persist");
                break;
            }

            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    });

    let runtime = AgentRuntime::new(RuntimeContext {
        profiles: Arc::new(profiles),
        provider_override: Some(Arc::new(MockProvider)),
        session_store: Arc::new(MemorySessionStore::default()),
        memory_store: Arc::new(MemoryMemoryStore::default()),
        memory_policy: MemoryPolicy::default(),
        runtime_policy: MosaicConfig::default().runtime,
        attachments: MosaicConfig::default().attachments,
        sandbox: Arc::new(test_sandbox_manager()),
        telegram: Default::default(),
        app_name: None,
        tools: Arc::new(tools),
        skills: Arc::new(SkillRegistry::new()),
        workflows: Arc::new(WorkflowRegistry::new()),
        node_router: Some(node_store.clone()),
        active_extensions: Vec::new(),
        event_sink: Arc::new(NoopEventSink),
    });

    let result = runtime
        .run(RunRequest {
            run_id: None,
            system: Some("Use tools when needed.".to_owned()),
            input: "Read a file for me.".to_owned(),
            tool: None,
            skill: None,
            workflow: None,
            session_id: Some("node-demo".to_owned()),
            profile: None,
            ingress: None,
        })
        .await
        .expect("node-routed tool loop should succeed");

    worker.await.expect("node worker should join");

    assert_eq!(result.trace.tool_calls.len(), 1);
    assert_eq!(
        result.trace.tool_calls[0].node_id.as_deref(),
        Some("node-a")
    );
    assert_eq!(
        result.trace.tool_calls[0].capability_route.as_deref(),
        Some("session_affinity")
    );
    assert_eq!(result.trace.capability_invocations.len(), 1);
    assert_eq!(
        result.trace.capability_invocations[0].node_id.as_deref(),
        Some("node-a")
    );
    assert_eq!(
        result.trace.capability_invocations[0]
            .capability_route
            .as_deref(),
        Some("session_affinity")
    );
    assert!(result.output.contains("node-routed contents"));
}

#[tokio::test]
async fn node_preferred_read_file_falls_back_to_local_when_matching_node_is_offline() {
    let config = config_with_mock_active_profile();
    let profiles =
        ProviderProfileRegistry::from_config(&config).expect("profile registry should build");
    let workspace_root = std::env::temp_dir().join(format!(
        "mosaic-runtime-node-fallback-tests-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&workspace_root).expect("workspace root should be created");
    let path = workspace_root.join("README.md");
    std::fs::write(&path, "local fallback contents").expect("README should be written");

    let mut tools = ToolRegistry::new();
    tools.register(Arc::new(ReadFileTool::new_with_allowed_roots(vec![
        workspace_root.clone(),
    ])));
    let node_store = Arc::new(FileNodeStore::new(workspace_root.join(".mosaic/nodes")));
    let mut registration = NodeRegistration::new(
        "node-offline",
        "Offline Node",
        "file-bus",
        "headless",
        vec![NodeCapabilityDeclaration {
            name: "read_file".to_owned(),
            kind: CapabilityKind::File,
            permission_scopes: vec![PermissionScope::LocalRead],
            risk: ToolRiskLevel::Low,
        }],
    );
    registration.disconnect("operator_shutdown");
    node_store
        .register_node(&registration)
        .expect("node registration should persist");

    let runtime = AgentRuntime::new(RuntimeContext {
        profiles: Arc::new(profiles),
        provider_override: Some(Arc::new(MockProvider)),
        session_store: Arc::new(MemorySessionStore::default()),
        memory_store: Arc::new(MemoryMemoryStore::default()),
        memory_policy: MemoryPolicy::default(),
        runtime_policy: MosaicConfig::default().runtime,
        attachments: MosaicConfig::default().attachments,
        sandbox: Arc::new(test_sandbox_manager()),
        telegram: Default::default(),
        app_name: None,
        tools: Arc::new(tools),
        skills: Arc::new(SkillRegistry::new()),
        workflows: Arc::new(WorkflowRegistry::new()),
        node_router: Some(node_store),
        active_extensions: Vec::new(),
        event_sink: Arc::new(NoopEventSink),
    });

    let outcome = runtime
        .invoke_tool_with_guardrails(
            Some("fallback-demo"),
            "read_file".to_owned(),
            "call-1".to_owned(),
            serde_json::json!({
                "path": path.display().to_string(),
            }),
            None,
            OrchestrationOwner::Runtime,
        )
        .await
        .expect("read_file should fall back locally");

    assert_eq!(outcome.output, "local fallback contents");
    assert_eq!(outcome.tool_trace.effective_execution_target, "local");
    assert!(outcome.tool_trace.node_attempted);
    assert!(outcome.tool_trace.node_fallback_to_local);
    assert_eq!(
        outcome.tool_trace.node_failure_class.as_deref(),
        Some("node_unavailable")
    );
    assert_eq!(outcome.tool_trace.node_id.as_deref(), Some("node-offline"));
    assert_eq!(
        outcome.tool_trace.capability_route.as_deref(),
        Some("capability_match")
    );
    assert_eq!(outcome.capability_trace.effective_execution_target, "local");
    assert!(outcome.capability_trace.node_fallback_to_local);
    assert_eq!(
        outcome.capability_trace.node_failure_class.as_deref(),
        Some("node_unavailable")
    );
    assert!(
        outcome
            .capability_trace
            .summary
            .contains("fallback_to_local=true")
    );
}

#[tokio::test]
async fn require_node_tool_does_not_fall_back_when_no_node_is_available() {
    let config = config_with_mock_active_profile();
    let profiles =
        ProviderProfileRegistry::from_config(&config).expect("profile registry should build");
    let workspace_root = std::env::temp_dir().join(format!(
        "mosaic-runtime-node-required-tests-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&workspace_root).expect("workspace root should be created");

    let mut tools = ToolRegistry::new();
    tools.register(Arc::new(RequiredNodeTool::new()));
    let node_store = Arc::new(FileNodeStore::new(workspace_root.join(".mosaic/nodes")));

    let runtime = AgentRuntime::new(RuntimeContext {
        profiles: Arc::new(profiles),
        provider_override: Some(Arc::new(MockProvider)),
        session_store: Arc::new(MemorySessionStore::default()),
        memory_store: Arc::new(MemoryMemoryStore::default()),
        memory_policy: MemoryPolicy::default(),
        runtime_policy: MosaicConfig::default().runtime,
        attachments: MosaicConfig::default().attachments,
        sandbox: Arc::new(test_sandbox_manager()),
        telegram: Default::default(),
        app_name: None,
        tools: Arc::new(tools),
        skills: Arc::new(SkillRegistry::new()),
        workflows: Arc::new(WorkflowRegistry::new()),
        node_router: Some(node_store),
        active_extensions: Vec::new(),
        event_sink: Arc::new(NoopEventSink),
    });

    let failure = runtime
        .invoke_tool_with_guardrails(
            Some("required-demo"),
            "require_node_echo".to_owned(),
            "call-required".to_owned(),
            serde_json::json!({ "text": "hello" }),
            None,
            OrchestrationOwner::Runtime,
        )
        .await
        .expect_err("require_node tool should fail without a node");

    let tool_trace = failure.tool_trace.expect("tool trace should exist");
    assert_eq!(tool_trace.effective_execution_target, "node");
    assert!(tool_trace.node_attempted);
    assert!(!tool_trace.node_fallback_to_local);
    assert_eq!(
        tool_trace.node_failure_class.as_deref(),
        Some("no_eligible_node")
    );
    let capability_trace = failure
        .capability_trace
        .expect("capability trace should exist");
    assert_eq!(
        capability_trace.failure_origin.map(|origin| origin.label()),
        Some("node")
    );
}

#[tokio::test]
async fn permission_denied_affinity_does_not_fall_back_to_local_execution() {
    let config = config_with_mock_active_profile();
    let profiles =
        ProviderProfileRegistry::from_config(&config).expect("profile registry should build");
    let workspace_root = std::env::temp_dir().join(format!(
        "mosaic-runtime-node-permission-tests-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&workspace_root).expect("workspace root should be created");
    let path = workspace_root.join("README.md");
    std::fs::write(&path, "permission denied contents").expect("README should be written");

    let mut tools = ToolRegistry::new();
    tools.register(Arc::new(ReadFileTool::new_with_allowed_roots(vec![
        workspace_root.clone(),
    ])));
    let node_store = Arc::new(FileNodeStore::new(workspace_root.join(".mosaic/nodes")));
    node_store
        .register_node(&NodeRegistration::new(
            "node-exec-only",
            "Exec Only",
            "file-bus",
            "headless",
            vec![NodeCapabilityDeclaration {
                name: "exec_command".to_owned(),
                kind: CapabilityKind::Exec,
                permission_scopes: vec![PermissionScope::LocalExec],
                risk: ToolRiskLevel::High,
            }],
        ))
        .expect("node registration should persist");
    node_store
        .attach_session("permission-demo", "node-exec-only")
        .expect("session affinity should persist");

    let runtime = AgentRuntime::new(RuntimeContext {
        profiles: Arc::new(profiles),
        provider_override: Some(Arc::new(MockProvider)),
        session_store: Arc::new(MemorySessionStore::default()),
        memory_store: Arc::new(MemoryMemoryStore::default()),
        memory_policy: MemoryPolicy::default(),
        runtime_policy: MosaicConfig::default().runtime,
        attachments: MosaicConfig::default().attachments,
        sandbox: Arc::new(test_sandbox_manager()),
        telegram: Default::default(),
        app_name: None,
        tools: Arc::new(tools),
        skills: Arc::new(SkillRegistry::new()),
        workflows: Arc::new(WorkflowRegistry::new()),
        node_router: Some(node_store),
        active_extensions: Vec::new(),
        event_sink: Arc::new(NoopEventSink),
    });

    let failure = runtime
        .invoke_tool_with_guardrails(
            Some("permission-demo"),
            "read_file".to_owned(),
            "call-permission".to_owned(),
            serde_json::json!({
                "path": path.display().to_string(),
            }),
            None,
            OrchestrationOwner::Runtime,
        )
        .await
        .expect_err("permission denied should not fall back to local execution");

    let tool_trace = failure.tool_trace.expect("tool trace should exist");
    assert_eq!(tool_trace.effective_execution_target, "node");
    assert!(tool_trace.node_attempted);
    assert!(!tool_trace.node_fallback_to_local);
    assert_eq!(
        tool_trace.node_failure_class.as_deref(),
        Some("node_permission_denied")
    );
    let capability_trace = failure
        .capability_trace
        .expect("capability trace should exist");
    assert_eq!(
        capability_trace.failure_origin.map(|origin| origin.label()),
        Some("node")
    );
}

#[tokio::test]
async fn workflow_runs_record_step_trace_and_skill_invocation() {
    let sink = Arc::new(VecEventSink::default());
    let store = Arc::new(MemorySessionStore::default());
    let mut workflows = WorkflowRegistry::new();
    workflows.register(research_workflow());
    let config = config_with_mock_active_profile();
    let profiles =
        ProviderProfileRegistry::from_config(&config).expect("profile registry should build");
    let mut skills = SkillRegistry::new();
    skills.register(Arc::new(SummarizeSkill));
    let mut tools = ToolRegistry::new();
    tools.register(Arc::new(TimeNowTool::new()));

    let runtime = AgentRuntime::new(RuntimeContext {
        profiles: Arc::new(profiles),
        provider_override: Some(Arc::new(MockProvider)),
        session_store: store.clone(),
        memory_store: Arc::new(MemoryMemoryStore::default()),
        memory_policy: MemoryPolicy::default(),
        runtime_policy: MosaicConfig::default().runtime,
        attachments: MosaicConfig::default().attachments,
        sandbox: Arc::new(test_sandbox_manager()),
        telegram: Default::default(),
        app_name: None,
        tools: Arc::new(tools),
        skills: Arc::new(skills),
        workflows: Arc::new(workflows),
        node_router: None,
        active_extensions: Vec::new(),
        event_sink: sink.clone(),
    });

    let result = runtime
        .run(RunRequest {
            run_id: None,
            system: None,
            input: "Rust async enables efficient concurrency.".to_owned(),
            tool: None,
            skill: None,
            workflow: Some("research_brief".to_owned()),
            session_id: Some("workflow-demo".to_owned()),
            profile: None,
            ingress: None,
        })
        .await
        .expect("workflow run should succeed");

    assert_eq!(
        result.trace.workflow_name.as_deref(),
        Some("research_brief")
    );
    assert_eq!(result.trace.step_traces.len(), 2);
    assert_eq!(result.trace.step_traces[0].name, "draft");
    assert_eq!(result.trace.step_traces[0].status(), "success");
    assert_eq!(result.trace.step_traces[1].name, "summarize");
    assert_eq!(result.trace.step_traces[1].status(), "success");
    assert_eq!(result.trace.skill_calls.len(), 1);
    assert!(
        result
            .output
            .starts_with("summary: mock response: Draft notes for:")
    );

    let session = store
        .get("workflow-demo")
        .expect("workflow session should exist");
    assert!(
        session
            .transcript
            .iter()
            .any(|message| message.role == TranscriptRole::Assistant)
    );

    assert_eq!(
        event_names(&sink.snapshot()),
        vec![
            "RunStarted",
            "WorkflowStarted",
            "WorkflowStepStarted",
            "ProviderRequest",
            "WorkflowStepFinished",
            "WorkflowStepStarted",
            "SkillStarted",
            "SkillFinished",
            "WorkflowStepFinished",
            "WorkflowFinished",
            "OutputDelta",
            "OutputDelta",
            "FinalAnswerReady",
            "RunFinished",
        ]
    );
}

#[tokio::test]
async fn markdown_skill_pack_runs_and_records_source_metadata() {
    let dir = temp_dir("markdown-skill");
    fs::create_dir_all(&dir).expect("temp dir should exist");
    fs::write(
        dir.join("SKILL.md"),
        r#"---
name: operator_note
description: Render an operator note
version: 0.1.0
runtime_requirements:
  - python
---
Operator note:
{{input}}
"#,
    )
    .expect("skill pack should be written");

    let pack = MarkdownSkillPack::load_from_dir(&dir).expect("markdown skill should load");
    let mut skills = SkillRegistry::new();
    skills.register_markdown_pack(pack);

    let runtime = AgentRuntime::new(RuntimeContext {
        profiles: Arc::new(
            ProviderProfileRegistry::from_config(&MosaicConfig::default())
                .expect("profile registry should build"),
        ),
        provider_override: Some(Arc::new(MockProvider)),
        session_store: Arc::new(MemorySessionStore::default()),
        memory_store: Arc::new(MemoryMemoryStore::default()),
        memory_policy: MemoryPolicy::default(),
        runtime_policy: MosaicConfig::default().runtime,
        attachments: MosaicConfig::default().attachments,
        sandbox: Arc::new(test_sandbox_manager()),
        telegram: Default::default(),
        app_name: None,
        tools: Arc::new(ToolRegistry::new()),
        skills: Arc::new(skills),
        workflows: Arc::new(WorkflowRegistry::new()),
        node_router: None,
        active_extensions: Vec::new(),
        event_sink: Arc::new(NoopEventSink),
    });

    let result = runtime
        .run(RunRequest {
            run_id: None,
            system: None,
            input: "Disk usage high on host-7".to_owned(),
            tool: None,
            skill: Some("operator_note".to_owned()),
            workflow: None,
            session_id: Some("markdown-skill-demo".to_owned()),
            profile: None,
            ingress: None,
        })
        .await
        .expect("markdown skill run should succeed");

    assert_eq!(result.output, "Operator note:\nDisk usage high on host-7");
    assert_eq!(result.trace.skill_calls.len(), 1);
    assert_eq!(
        result.trace.skill_calls[0].source_kind.as_deref(),
        Some("markdown_pack")
    );
    assert_eq!(
        result.trace.skill_calls[0].skill_version.as_deref(),
        Some("0.1.0")
    );
    assert_eq!(
        result.trace.skill_calls[0].runtime_requirements,
        vec!["python".to_owned()]
    );

    fs::remove_dir_all(dir).ok();
}

#[tokio::test]
async fn markdown_skill_pack_trace_records_templates_references_scripts_and_event_summary() {
    let dir = temp_dir("markdown-rich-skill");
    fs::create_dir_all(dir.join("templates")).expect("templates dir should exist");
    fs::create_dir_all(dir.join("references")).expect("references dir should exist");
    fs::create_dir_all(dir.join("scripts")).expect("scripts dir should exist");
    fs::write(
        dir.join("SKILL.md"),
        r#"---
name: operator_note
description: Render an operator note
version: 0.2.0
template: note.md
references:
  - escalation.md
script: annotate.py
script_runtime: python
accepts_attachments: true
runtime_requirements:
  - python
---
Reference:
{{references.escalation}}
"#,
    )
    .expect("skill pack should be written");
    fs::write(
        dir.join("templates").join("note.md"),
        "Operator note:\n{{input}}\nAttachments: {{attachments.summary}}\n",
    )
    .expect("template should be written");
    fs::write(
        dir.join("references").join("escalation.md"),
        "Escalate to the platform team.",
    )
    .expect("reference should be written");
    fs::write(
        dir.join("scripts").join("annotate.py"),
        r#"import json, sys
payload = json.load(sys.stdin)
print(json.dumps({
  "content": payload["rendered_prompt"] + "\nscript=ok",
  "output_mode": "json"
}))
"#,
    )
    .expect("script should be written");

    let pack = MarkdownSkillPack::load_from_dir(&dir).expect("markdown skill should load");
    let mut skills = SkillRegistry::new();
    skills.register_markdown_pack(pack);
    let sink = Arc::new(VecEventSink::default());

    let runtime = AgentRuntime::new(RuntimeContext {
        profiles: Arc::new(
            ProviderProfileRegistry::from_config(&MosaicConfig::default())
                .expect("profile registry should build"),
        ),
        provider_override: Some(Arc::new(MockProvider)),
        session_store: Arc::new(MemorySessionStore::default()),
        memory_store: Arc::new(MemoryMemoryStore::default()),
        memory_policy: MemoryPolicy::default(),
        runtime_policy: MosaicConfig::default().runtime,
        attachments: MosaicConfig::default().attachments,
        sandbox: Arc::new(test_sandbox_manager()),
        telegram: Default::default(),
        app_name: None,
        tools: Arc::new(ToolRegistry::new()),
        skills: Arc::new(skills),
        workflows: Arc::new(WorkflowRegistry::new()),
        node_router: None,
        active_extensions: Vec::new(),
        event_sink: sink.clone(),
    });

    let result = runtime
        .run(RunRequest {
            run_id: None,
            system: None,
            input: "Disk usage high on host-7".to_owned(),
            tool: None,
            skill: Some("operator_note".to_owned()),
            workflow: None,
            session_id: Some("markdown-skill-details".to_owned()),
            profile: None,
            ingress: Some(mosaic_inspect::IngressTrace {
                kind: "telegram_bot".to_owned(),
                channel: Some("telegram".to_owned()),
                adapter: Some("telegram_webhook".to_owned()),
                bot_name: None,
                bot_route: None,
                bot_profile: None,
                bot_token_env: None,
                bot_secret_env: None,
                source: None,
                remote_addr: None,
                display_name: Some("Operator".to_owned()),
                actor_id: Some("42".to_owned()),
                conversation_id: Some("telegram:chat:42".to_owned()),
                thread_id: None,
                thread_title: None,
                reply_target: Some("telegram:chat:42:msg:1".to_owned()),
                message_id: Some("1".to_owned()),
                received_at: None,
                raw_event_id: Some("event-1".to_owned()),
                session_hint: None,
                profile_hint: None,
                control_command: Some("skill".to_owned()),
                original_text: Some(
                    "/mosaic skill operator_note Disk usage high on host-7".to_owned(),
                ),
                attachments: vec![mosaic_inspect::ChannelAttachment {
                    id: "attachment-1".to_owned(),
                    kind: mosaic_inspect::AttachmentKind::Image,
                    filename: Some("dashboard.png".to_owned()),
                    mime_type: Some("image/png".to_owned()),
                    size_bytes: Some(128),
                    source_ref: Some("file-1".to_owned()),
                    remote_url: None,
                    local_cache_path: None,
                    caption: None,
                }],
                attachment_failures: Vec::new(),
                gateway_url: None,
            }),
        })
        .await
        .expect("markdown skill run should succeed");

    let call = &result.trace.skill_calls[0];
    assert!(result.output.contains("script=ok"));
    assert!(call.accepts_attachments);
    assert_eq!(
        call.markdown_pack
            .as_ref()
            .and_then(|pack| pack.template.as_deref()),
        Some("note.md")
    );
    assert_eq!(
        call.markdown_pack
            .as_ref()
            .map(|pack| pack.references.clone())
            .unwrap_or_default(),
        vec!["escalation.md".to_owned()]
    );
    assert_eq!(
        call.markdown_pack
            .as_ref()
            .and_then(|pack| pack.script.as_deref()),
        Some("annotate.py")
    );
    assert_eq!(
        call.markdown_pack
            .as_ref()
            .and_then(|pack| pack.script_runtime.as_deref()),
        Some("python")
    );
    assert_eq!(
        call.markdown_pack
            .as_ref()
            .map(|pack| pack.attachment_count),
        Some(1)
    );
    assert!(call.sandbox.is_some());

    let events = sink.snapshot();
    assert!(events.iter().any(|event| matches!(
        event,
        RunEvent::SkillStarted { summary: Some(summary), .. }
            if summary.contains("template=note.md") && summary.contains("script=annotate.py")
    )));
    assert!(events.iter().any(|event| matches!(
        event,
        RunEvent::SkillFinished { summary: Some(summary), .. }
            if summary.contains("script=annotate.py (python)") && summary.contains("attachments=1")
    )));

    fs::remove_dir_all(dir).ok();
}

#[tokio::test]
async fn workflow_can_call_markdown_skill_pack() {
    let dir = temp_dir("markdown-workflow");
    fs::create_dir_all(&dir).expect("temp dir should exist");
    fs::write(
        dir.join("SKILL.md"),
        "---\nname: workflow_note\nversion: 0.1.0\n---\nWorkflow note:\n{{input}}\n",
    )
    .expect("skill pack should be written");

    let pack = MarkdownSkillPack::load_from_dir(&dir).expect("markdown skill should load");
    let mut skills = SkillRegistry::new();
    skills.register_markdown_pack(pack);

    let mut workflows = WorkflowRegistry::new();
    workflows.register(Workflow {
        name: "markdown_flow".to_owned(),
        description: Some("Call markdown skill".to_owned()),
        visibility: mosaic_tool_core::CapabilityExposure::default(),
        steps: vec![WorkflowStep {
            name: "note".to_owned(),
            kind: WorkflowStepKind::Skill {
                skill: "workflow_note".to_owned(),
                input: "{{input}}".to_owned(),
            },
        }],
    });

    let runtime = AgentRuntime::new(RuntimeContext {
        profiles: Arc::new(
            ProviderProfileRegistry::from_config(&MosaicConfig::default())
                .expect("profile registry should build"),
        ),
        provider_override: Some(Arc::new(MockProvider)),
        session_store: Arc::new(MemorySessionStore::default()),
        memory_store: Arc::new(MemoryMemoryStore::default()),
        memory_policy: MemoryPolicy::default(),
        runtime_policy: MosaicConfig::default().runtime,
        attachments: MosaicConfig::default().attachments,
        sandbox: Arc::new(test_sandbox_manager()),
        telegram: Default::default(),
        app_name: None,
        tools: Arc::new(ToolRegistry::new()),
        skills: Arc::new(skills),
        workflows: Arc::new(workflows),
        node_router: None,
        active_extensions: Vec::new(),
        event_sink: Arc::new(NoopEventSink),
    });

    let result = runtime
        .run(RunRequest {
            run_id: None,
            system: None,
            input: "Escalate the node restart.".to_owned(),
            tool: None,
            skill: None,
            workflow: Some("markdown_flow".to_owned()),
            session_id: Some("markdown-workflow-demo".to_owned()),
            profile: None,
            ingress: None,
        })
        .await
        .expect("workflow run should succeed");

    assert_eq!(result.output, "Workflow note:\nEscalate the node restart.");
    assert_eq!(result.trace.skill_calls.len(), 1);
    assert_eq!(
        result.trace.skill_calls[0].source_kind.as_deref(),
        Some("markdown_pack")
    );

    fs::remove_dir_all(dir).ok();
}

#[tokio::test]
async fn workflow_step_tool_capability_failures_surface_as_run_failures() {
    let sink = Arc::new(VecEventSink::default());
    let store = Arc::new(MemorySessionStore::default());
    let mut config = MosaicConfig::default();
    config.active_profile = "text-only".to_owned();
    config.profiles.clear();
    config.profiles.insert(
        "text-only".to_owned(),
        ProviderProfileConfig {
            provider_type: "plain".to_owned(),
            model: "plain-1".to_owned(),
            base_url: None,
            api_key_env: None,
            transport: Default::default(),
            vendor: Default::default(),
            attachments: Default::default(),
        },
    );
    let profiles =
        ProviderProfileRegistry::from_config(&config).expect("profile registry should build");
    let mut workflows = WorkflowRegistry::new();
    workflows.register(Workflow {
        name: "tool_step".to_owned(),
        description: None,
        visibility: mosaic_tool_core::CapabilityExposure::default(),
        steps: vec![WorkflowStep {
            name: "lookup_time".to_owned(),
            kind: WorkflowStepKind::Prompt {
                prompt: "What time is it?".to_owned(),
                system: None,
                tools: vec!["time_now".to_owned()],
                profile: None,
            },
        }],
    });
    let mut tools = ToolRegistry::new();
    tools.register(Arc::new(TimeNowTool::new()));

    let runtime = AgentRuntime::new(RuntimeContext {
        profiles: Arc::new(profiles),
        provider_override: Some(Arc::new(MockProvider)),
        session_store: store,
        memory_store: Arc::new(MemoryMemoryStore::default()),
        memory_policy: MemoryPolicy::default(),
        runtime_policy: MosaicConfig::default().runtime,
        attachments: MosaicConfig::default().attachments,
        sandbox: Arc::new(test_sandbox_manager()),
        telegram: Default::default(),
        app_name: None,
        tools: Arc::new(tools),
        skills: Arc::new(SkillRegistry::new()),
        workflows: Arc::new(workflows),
        node_router: None,
        active_extensions: Vec::new(),
        event_sink: sink.clone(),
    });

    let err = runtime
        .run(RunRequest {
            run_id: None,
            system: None,
            input: "Need the current time".to_owned(),
            tool: None,
            skill: None,
            workflow: Some("tool_step".to_owned()),
            session_id: None,
            profile: None,
            ingress: None,
        })
        .await
        .expect_err("tool-capability mismatch should fail");

    assert!(!err.to_string().is_empty());
    assert_eq!(
        err.trace()
            .failure
            .as_ref()
            .map(|failure| failure.origin.label()),
        Some("workflow")
    );
    assert_eq!(
        event_names(&sink.snapshot()),
        vec![
            "RunStarted",
            "WorkflowStarted",
            "WorkflowStepStarted",
            "WorkflowStepFailed",
            "RunFailed",
        ]
    );
}

#[tokio::test]
async fn empty_provider_response_returns_an_error() {
    let sink = Arc::new(VecEventSink::default());
    let runtime = runtime_with_provider(
        Arc::new(EmptyProvider),
        Arc::new(MemorySessionStore::default()),
        sink.clone(),
    );

    let err = runtime
        .run(RunRequest {
            run_id: None,
            system: None,
            input: "Explain Mosaic.".to_owned(),
            tool: None,
            skill: None,
            workflow: None,
            session_id: None,
            profile: None,
            ingress: None,
        })
        .await
        .expect_err("empty provider response should fail");

    assert!(
        err.to_string()
            .contains("runtime stopped without final assistant message")
    );
    assert_eq!(
        event_names(&sink.snapshot()),
        vec!["RunStarted", "ProviderRequest", "RunFailed"]
    );
}

#[tokio::test]
async fn skill_failures_emit_skill_failed_then_run_failed() {
    let sink = Arc::new(VecEventSink::default());
    let config = config_with_mock_active_profile();

    let profiles =
        ProviderProfileRegistry::from_config(&config).expect("profile registry should build");
    let mut skills = SkillRegistry::new();
    skills.register(Arc::new(FailingSkill));

    let runtime = AgentRuntime::new(RuntimeContext {
        profiles: Arc::new(profiles),
        provider_override: Some(Arc::new(MockProvider)),
        session_store: Arc::new(MemorySessionStore::default()),
        memory_store: Arc::new(MemoryMemoryStore::default()),
        memory_policy: MemoryPolicy::default(),
        runtime_policy: MosaicConfig::default().runtime,
        attachments: MosaicConfig::default().attachments,
        sandbox: Arc::new(test_sandbox_manager()),
        telegram: Default::default(),
        app_name: None,
        tools: Arc::new(ToolRegistry::new()),
        skills: Arc::new(skills),
        workflows: Arc::new(WorkflowRegistry::new()),
        node_router: None,
        active_extensions: Vec::new(),
        event_sink: sink.clone(),
    });

    let err = runtime
        .run(RunRequest {
            run_id: None,
            system: None,
            input: "boom".to_owned(),
            tool: None,
            skill: Some("explode".to_owned()),
            workflow: None,
            session_id: Some("skill-demo".to_owned()),
            profile: None,
            ingress: None,
        })
        .await
        .expect_err("failing skill should fail");

    assert!(err.to_string().contains("skill exploded"));
    assert_eq!(
        event_names(&sink.snapshot()),
        vec!["RunStarted", "SkillStarted", "SkillFailed", "RunFailed"]
    );
}

#[tokio::test]
async fn session_skill_runs_persist_assistant_output() {
    let config = config_with_mock_active_profile();
    let profiles =
        ProviderProfileRegistry::from_config(&config).expect("profile registry should build");
    let store = Arc::new(MemorySessionStore::default());
    let mut skills = SkillRegistry::new();
    skills.register(Arc::new(SummarizeSkill));

    let runtime = AgentRuntime::new(RuntimeContext {
        profiles: Arc::new(profiles),
        provider_override: Some(Arc::new(MockProvider)),
        session_store: store.clone(),
        memory_store: Arc::new(MemoryMemoryStore::default()),
        memory_policy: MemoryPolicy::default(),
        runtime_policy: MosaicConfig::default().runtime,
        attachments: MosaicConfig::default().attachments,
        sandbox: Arc::new(test_sandbox_manager()),
        telegram: Default::default(),
        app_name: None,
        tools: Arc::new(ToolRegistry::new()),
        skills: Arc::new(skills),
        workflows: Arc::new(WorkflowRegistry::new()),
        node_router: None,
        active_extensions: Vec::new(),
        event_sink: Arc::new(NoopEventSink),
    });

    let result = runtime
        .run(RunRequest {
            run_id: None,
            system: None,
            input: "Rust async enables concurrency.".to_owned(),
            tool: None,
            skill: Some("summarize".to_owned()),
            workflow: None,
            session_id: Some("summary-demo".to_owned()),
            profile: None,
            ingress: None,
        })
        .await
        .expect("skill run should succeed");

    let session = store.get("summary-demo").expect("session should exist");
    assert_eq!(result.trace.session_id.as_deref(), Some("summary-demo"));
    assert!(
        session
            .transcript
            .iter()
            .any(|message: &TranscriptMessage| message.content.contains("summary:"))
    );
}

#[tokio::test]
async fn session_runs_persist_memory_and_record_compression_trace() {
    let config = config_with_mock_active_profile();
    let profiles =
        ProviderProfileRegistry::from_config(&config).expect("profile registry should build");
    let store = Arc::new(MemorySessionStore::default());
    let memory_store = Arc::new(MemoryMemoryStore::default());

    let runtime = AgentRuntime::new(RuntimeContext {
        profiles: Arc::new(profiles),
        provider_override: Some(Arc::new(MockProvider)),
        session_store: store.clone(),
        memory_store: memory_store.clone(),
        memory_policy: MemoryPolicy {
            compression_message_threshold: 3,
            recent_message_window: 2,
            summary_char_budget: 160,
            note_char_budget: 120,
        },
        runtime_policy: MosaicConfig::default().runtime,
        attachments: MosaicConfig::default().attachments,
        sandbox: Arc::new(test_sandbox_manager()),
        telegram: Default::default(),
        app_name: None,
        tools: Arc::new(ToolRegistry::new()),
        skills: Arc::new(SkillRegistry::new()),
        workflows: Arc::new(WorkflowRegistry::new()),
        node_router: None,
        active_extensions: Vec::new(),
        event_sink: Arc::new(NoopEventSink),
    });

    runtime
        .run(RunRequest {
            run_id: None,
            system: Some("You are helpful.".to_owned()),
            input: "first turn".to_owned(),
            tool: None,
            skill: None,
            workflow: None,
            session_id: Some("memory-demo".to_owned()),
            profile: None,
            ingress: None,
        })
        .await
        .expect("first run should succeed");

    let result = runtime
        .run(RunRequest {
            run_id: None,
            system: Some("You are helpful.".to_owned()),
            input: "second turn should reuse compressed context".to_owned(),
            tool: None,
            skill: None,
            workflow: None,
            session_id: Some("memory-demo".to_owned()),
            profile: None,
            ingress: None,
        })
        .await
        .expect("second run should succeed");

    let session = store.get("memory-demo").expect("session should exist");
    let memory = memory_store
        .get("memory-demo")
        .expect("memory record should exist");

    assert!(session.memory.latest_summary.is_some());
    assert!(memory.summary.is_some());
    assert!(!result.trace.memory_writes.is_empty());
    assert!(result.trace.compression.is_some());
}

#[tokio::test]
async fn cross_session_reference_records_memory_reads_and_session_links() {
    let config = config_with_mock_active_profile();
    let profiles =
        ProviderProfileRegistry::from_config(&config).expect("profile registry should build");
    let store = Arc::new(MemorySessionStore::default());
    let memory_store = Arc::new(MemoryMemoryStore::default());
    memory_store
        .save_session(&{
            let mut record = SessionMemoryRecord::new("source-session");
            record.set_summary(Some("Source session summary".to_owned()));
            record
        })
        .expect("memory seed should save");

    let runtime = AgentRuntime::new(RuntimeContext {
        profiles: Arc::new(profiles),
        provider_override: Some(Arc::new(MockProvider)),
        session_store: store.clone(),
        memory_store: memory_store.clone(),
        memory_policy: MemoryPolicy::default(),
        runtime_policy: MosaicConfig::default().runtime,
        attachments: MosaicConfig::default().attachments,
        sandbox: Arc::new(test_sandbox_manager()),
        telegram: Default::default(),
        app_name: None,
        tools: Arc::new(ToolRegistry::new()),
        skills: Arc::new(SkillRegistry::new()),
        workflows: Arc::new(WorkflowRegistry::new()),
        node_router: None,
        active_extensions: Vec::new(),
        event_sink: Arc::new(NoopEventSink),
    });

    let result = runtime
        .run(RunRequest {
            run_id: None,
            system: Some("You are helpful.".to_owned()),
            input: "Please use [[session:source-session]] for context".to_owned(),
            tool: None,
            skill: None,
            workflow: None,
            session_id: Some("target-session".to_owned()),
            profile: None,
            ingress: None,
        })
        .await
        .expect("run should succeed");

    let session = store
        .get("target-session")
        .expect("target session should exist");
    assert!(result.trace.memory_reads.iter().any(|read| {
        read.session_id == "source-session" && read.source == "cross_session_reference"
    }));
    assert_eq!(session.references.len(), 1);
    assert_eq!(session.references[0].session_id, "source-session");
}
