use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::Result;
use chrono::{DateTime, Utc};
use mosaic_tool_core::{CapabilityKind, PermissionScope, ToolRiskLevel, ToolSource};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RouteMode {
    Assistant,
    Tool,
    Skill,
    Workflow,
    Control,
}

impl RouteMode {
    pub fn label(self) -> &'static str {
        match self {
            Self::Assistant => "assistant",
            Self::Tool => "tool",
            Self::Skill => "skill",
            Self::Workflow => "workflow",
            Self::Control => "control",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RouteKind {
    Assistant,
    Tool,
    Skill,
    Workflow,
}

impl RouteKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Assistant => "assistant",
            Self::Tool => "tool",
            Self::Skill => "skill",
            Self::Workflow => "workflow",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CapabilitySourceKind {
    Builtin,
    WorkspaceConfig,
    Extension,
    Mcp,
    NativeSkill,
    ManifestSkill,
    MarkdownSkillPack,
}

impl CapabilitySourceKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Builtin => "builtin",
            Self::WorkspaceConfig => "workspace_config",
            Self::Extension => "extension",
            Self::Mcp => "mcp",
            Self::NativeSkill => "native_skill",
            Self::ManifestSkill => "manifest_skill",
            Self::MarkdownSkillPack => "markdown_skill_pack",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionTarget {
    #[default]
    Local,
    McpServer,
    Node,
    Provider,
    WorkflowEngine,
}

impl ExecutionTarget {
    pub fn label(self) -> &'static str {
        match self {
            Self::Local => "local",
            Self::McpServer => "mcp_server",
            Self::Node => "node",
            Self::Provider => "provider",
            Self::WorkflowEngine => "workflow_engine",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum OrchestrationOwner {
    #[default]
    Runtime,
    WorkflowEngine,
    Gateway,
}

impl OrchestrationOwner {
    pub fn label(self) -> &'static str {
        match self {
            Self::Runtime => "runtime",
            Self::WorkflowEngine => "workflow_engine",
            Self::Gateway => "gateway",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum FailureOrigin {
    Provider,
    #[default]
    Runtime,
    Tool,
    Mcp,
    Node,
    Skill,
    Workflow,
    Sandbox,
    Config,
    Gateway,
}

impl FailureOrigin {
    pub fn label(self) -> &'static str {
        match self {
            Self::Provider => "provider",
            Self::Runtime => "runtime",
            Self::Tool => "tool",
            Self::Mcp => "mcp",
            Self::Node => "node",
            Self::Skill => "skill",
            Self::Workflow => "workflow",
            Self::Sandbox => "sandbox",
            Self::Config => "config",
            Self::Gateway => "gateway",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RouteDecisionTrace {
    pub route_mode: RouteMode,
    #[serde(default)]
    pub route_kind: Option<RouteKind>,
    #[serde(default)]
    pub selected_capability_type: Option<String>,
    #[serde(default)]
    pub selected_capability_name: Option<String>,
    #[serde(default)]
    pub selected_tool: Option<String>,
    #[serde(default)]
    pub selected_skill: Option<String>,
    #[serde(default)]
    pub selected_workflow: Option<String>,
    pub selection_reason: String,
    #[serde(default)]
    pub capability_source: Option<String>,
    #[serde(default)]
    pub capability_source_kind: Option<CapabilitySourceKind>,
    #[serde(default)]
    pub source_name: Option<String>,
    #[serde(default)]
    pub source_path: Option<String>,
    #[serde(default)]
    pub source_version: Option<String>,
    #[serde(default)]
    pub execution_target: Option<ExecutionTarget>,
    #[serde(default)]
    pub orchestration_owner: Option<OrchestrationOwner>,
    #[serde(default)]
    pub policy_source: Option<String>,
    #[serde(default)]
    pub sandbox_scope: Option<String>,
    #[serde(default)]
    pub profile_used: Option<String>,
    #[serde(default)]
    pub selected_category: Option<String>,
    #[serde(default)]
    pub catalog_scope: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolTrace {
    pub call_id: Option<String>,
    pub name: String,
    #[serde(default)]
    pub source: ToolSource,
    #[serde(default)]
    pub capability_source_kind: Option<CapabilitySourceKind>,
    #[serde(default)]
    pub source_name: Option<String>,
    #[serde(default)]
    pub source_path: Option<String>,
    #[serde(default)]
    pub source_version: Option<String>,
    pub input: serde_json::Value,
    pub output: Option<String>,
    #[serde(default)]
    pub node_attempted: bool,
    #[serde(default)]
    pub node_fallback_to_local: bool,
    #[serde(default)]
    pub node_failure_class: Option<String>,
    #[serde(default)]
    pub node_id: Option<String>,
    #[serde(default)]
    pub capability_route: Option<String>,
    #[serde(default)]
    pub disconnect_context: Option<String>,
    #[serde(default = "default_execution_target")]
    pub effective_execution_target: String,
    #[serde(default)]
    pub execution_target: ExecutionTarget,
    #[serde(default)]
    pub orchestration_owner: OrchestrationOwner,
    #[serde(default)]
    pub policy_source: Option<String>,
    #[serde(default)]
    pub sandbox_scope: Option<String>,
    #[serde(default)]
    pub sandbox: Option<SandboxEnvTrace>,
    pub started_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
}

impl ToolTrace {
    pub fn duration_ms(&self) -> Option<i64> {
        self.finished_at.map(|finished| {
            finished
                .signed_duration_since(self.started_at)
                .num_milliseconds()
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CapabilityInvocationTrace {
    pub job_id: String,
    pub call_id: Option<String>,
    pub tool_name: String,
    #[serde(default)]
    pub route_kind: Option<RouteKind>,
    #[serde(default)]
    pub capability_source_kind: Option<CapabilitySourceKind>,
    #[serde(default)]
    pub source_name: Option<String>,
    #[serde(default)]
    pub source_path: Option<String>,
    #[serde(default)]
    pub source_version: Option<String>,
    pub kind: CapabilityKind,
    #[serde(default)]
    pub permission_scopes: Vec<PermissionScope>,
    pub risk: ToolRiskLevel,
    pub status: String,
    pub summary: String,
    pub target: Option<String>,
    #[serde(default)]
    pub node_attempted: bool,
    #[serde(default)]
    pub node_fallback_to_local: bool,
    #[serde(default)]
    pub node_failure_class: Option<String>,
    #[serde(default)]
    pub node_id: Option<String>,
    #[serde(default)]
    pub capability_route: Option<String>,
    #[serde(default)]
    pub disconnect_context: Option<String>,
    #[serde(default = "default_execution_target")]
    pub effective_execution_target: String,
    #[serde(default)]
    pub execution_target: ExecutionTarget,
    #[serde(default)]
    pub orchestration_owner: OrchestrationOwner,
    #[serde(default)]
    pub policy_source: Option<String>,
    #[serde(default)]
    pub sandbox_scope: Option<String>,
    #[serde(default)]
    pub failure_origin: Option<FailureOrigin>,
    pub started_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
    pub error: Option<String>,
}

impl CapabilityInvocationTrace {
    pub fn duration_ms(&self) -> Option<i64> {
        self.finished_at.map(|finished| {
            finished
                .signed_duration_since(self.started_at)
                .num_milliseconds()
        })
    }
}

fn default_execution_target() -> String {
    "local".to_owned()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SideEffectSummary {
    pub total: usize,
    pub failed: usize,
    pub high_risk: usize,
    #[serde(default)]
    pub capability_kinds: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExtensionTrace {
    pub name: String,
    pub version: String,
    pub source: String,
    pub enabled: bool,
    pub active: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExtensionUsageTrace {
    pub name: String,
    pub version: String,
    pub component_kind: String,
    pub component_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SkillTrace {
    pub name: String,
    #[serde(default)]
    pub source_kind: Option<String>,
    #[serde(default)]
    pub capability_source_kind: Option<CapabilitySourceKind>,
    #[serde(default)]
    pub source_name: Option<String>,
    #[serde(default)]
    pub source_path: Option<String>,
    #[serde(default)]
    pub skill_version: Option<String>,
    #[serde(default)]
    pub source_version: Option<String>,
    #[serde(default)]
    pub runtime_requirements: Vec<String>,
    #[serde(default)]
    pub accepts_attachments: bool,
    #[serde(default)]
    pub execution_target: ExecutionTarget,
    #[serde(default)]
    pub orchestration_owner: OrchestrationOwner,
    #[serde(default)]
    pub policy_source: Option<String>,
    #[serde(default)]
    pub sandbox_scope: Option<String>,
    #[serde(default)]
    pub sandbox: Option<SandboxEnvTrace>,
    #[serde(default)]
    pub markdown_pack: Option<MarkdownSkillPackTrace>,
    pub input: serde_json::Value,
    pub output: Option<String>,
    pub started_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MarkdownSkillPackTrace {
    pub pack_name: String,
    pub pack_path: String,
    pub skill_md: String,
    #[serde(default)]
    pub template: Option<String>,
    #[serde(default)]
    pub references: Vec<String>,
    #[serde(default)]
    pub script: Option<String>,
    #[serde(default)]
    pub script_runtime: Option<String>,
    #[serde(default)]
    pub attachment_count: usize,
    #[serde(default)]
    pub attachment_summary: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CapabilityExplanationTrace {
    pub scope: String,
    pub name: String,
    #[serde(default)]
    pub route_kind: Option<String>,
    #[serde(default)]
    pub capability_source_kind: Option<String>,
    #[serde(default)]
    pub execution_target: Option<String>,
    #[serde(default)]
    pub orchestration_owner: Option<String>,
    pub status: String,
    pub summary: String,
    #[serde(default)]
    pub decision_basis: Option<String>,
    #[serde(default)]
    pub failure_origin: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SandboxEnvTrace {
    pub env_id: String,
    pub env_kind: String,
    pub env_scope: String,
    pub env_name: String,
    pub env_path: String,
    #[serde(default)]
    pub workdir: Option<String>,
    #[serde(default)]
    pub dependency_spec: Vec<String>,
    #[serde(default)]
    pub strategy: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub error: Option<String>,
    #[serde(default)]
    pub selection_reason: Option<String>,
    #[serde(default)]
    pub prepared: Option<bool>,
    #[serde(default)]
    pub reused: Option<bool>,
    #[serde(default)]
    pub failure_stage: Option<String>,
    #[serde(default)]
    pub last_transition: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SandboxRunTrace {
    pub root_dir: String,
    pub workdir: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct IngressTrace {
    pub kind: String,
    pub channel: Option<String>,
    #[serde(default)]
    pub adapter: Option<String>,
    #[serde(default)]
    pub bot_name: Option<String>,
    #[serde(default)]
    pub bot_route: Option<String>,
    #[serde(default)]
    pub bot_profile: Option<String>,
    #[serde(default)]
    pub bot_token_env: Option<String>,
    #[serde(default)]
    pub bot_secret_env: Option<String>,
    pub source: Option<String>,
    pub remote_addr: Option<String>,
    pub display_name: Option<String>,
    pub actor_id: Option<String>,
    #[serde(default)]
    pub conversation_id: Option<String>,
    pub thread_id: Option<String>,
    pub thread_title: Option<String>,
    pub reply_target: Option<String>,
    #[serde(default)]
    pub message_id: Option<String>,
    #[serde(default)]
    pub received_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub raw_event_id: Option<String>,
    #[serde(default)]
    pub session_hint: Option<String>,
    #[serde(default)]
    pub profile_hint: Option<String>,
    #[serde(default)]
    pub control_command: Option<String>,
    #[serde(default)]
    pub original_text: Option<String>,
    #[serde(default)]
    pub attachments: Vec<ChannelAttachment>,
    #[serde(default)]
    pub attachment_failures: Vec<AttachmentFailureTrace>,
    pub gateway_url: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum AttachmentKind {
    Image,
    Document,
    Audio,
    Video,
    #[default]
    Other,
}

impl AttachmentKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Image => "image",
            Self::Document => "document",
            Self::Audio => "audio",
            Self::Video => "video",
            Self::Other => "other",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct ChannelAttachment {
    pub id: String,
    #[serde(default)]
    pub kind: AttachmentKind,
    #[serde(default)]
    pub filename: Option<String>,
    #[serde(default)]
    pub mime_type: Option<String>,
    #[serde(default)]
    pub size_bytes: Option<u64>,
    #[serde(default)]
    pub source_ref: Option<String>,
    #[serde(default)]
    pub remote_url: Option<String>,
    #[serde(default)]
    pub local_cache_path: Option<String>,
    #[serde(default)]
    pub caption: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AttachmentFailureTrace {
    pub attachment_id: String,
    pub stage: String,
    pub kind: String,
    pub message: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum AttachmentRouteMode {
    #[default]
    ProviderNative,
    SpecializedProcessor,
    Disabled,
}

impl AttachmentRouteMode {
    pub fn label(self) -> &'static str {
        match self {
            Self::ProviderNative => "provider_native",
            Self::SpecializedProcessor => "specialized_processor",
            Self::Disabled => "disabled",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AttachmentRouteTrace {
    #[serde(default)]
    pub mode: AttachmentRouteMode,
    pub selection_reason: String,
    #[serde(default)]
    pub bot_identity: Option<String>,
    #[serde(default)]
    pub policy_scope: Option<String>,
    #[serde(default)]
    pub selected_profile: Option<String>,
    #[serde(default)]
    pub provider_profile: Option<String>,
    #[serde(default)]
    pub provider_model: Option<String>,
    #[serde(default)]
    pub processor: Option<String>,
    #[serde(default)]
    pub allowed_attachment_kinds: Vec<String>,
    #[serde(default)]
    pub max_attachment_size_mb: Option<u64>,
    #[serde(default)]
    pub attachment_count: usize,
    #[serde(default)]
    pub attachment_kinds: Vec<String>,
    #[serde(default)]
    pub attachment_filenames: Vec<String>,
    #[serde(default)]
    pub failure_summary: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChannelQuickReplyButton {
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChannelReplyMarkup {
    #[serde(default)]
    pub rows: Vec<Vec<ChannelQuickReplyButton>>,
    #[serde(default)]
    pub input_placeholder: Option<String>,
    #[serde(default)]
    pub persistent: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChannelOutboundMessage {
    pub channel: String,
    pub adapter: String,
    #[serde(default)]
    pub bot_name: Option<String>,
    #[serde(default)]
    pub bot_route: Option<String>,
    #[serde(default)]
    pub bot_profile: Option<String>,
    #[serde(default)]
    pub bot_token_env: Option<String>,
    pub conversation_id: String,
    pub reply_target: String,
    pub text: String,
    pub idempotency_key: String,
    pub correlation_id: String,
    pub gateway_run_id: String,
    pub session_id: String,
    #[serde(default)]
    pub reply_markup: Option<ChannelReplyMarkup>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ChannelDeliveryStatus {
    Delivered,
    Failed,
}

impl ChannelDeliveryStatus {
    pub fn label(self) -> &'static str {
        match self {
            Self::Delivered => "delivered",
            Self::Failed => "failed",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChannelDeliveryResult {
    pub delivery_id: String,
    pub status: ChannelDeliveryStatus,
    pub provider_message_id: Option<String>,
    #[serde(default)]
    pub retry_count: usize,
    pub retryable: bool,
    pub error_kind: Option<String>,
    pub error: Option<String>,
    pub delivered_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChannelDeliveryTrace {
    pub message: ChannelOutboundMessage,
    pub result: ChannelDeliveryResult,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EffectiveProfileTrace {
    pub profile: String,
    pub provider_type: String,
    pub model: String,
    pub base_url: Option<String>,
    pub api_key_env: Option<String>,
    #[serde(default)]
    pub api_key_present: bool,
    #[serde(default)]
    pub timeout_ms: u64,
    #[serde(default)]
    pub max_retries: u8,
    #[serde(default)]
    pub retry_backoff_ms: u64,
    #[serde(default)]
    pub api_version: Option<String>,
    #[serde(default)]
    pub version_header: Option<String>,
    #[serde(default)]
    pub custom_header_keys: Vec<String>,
    #[serde(default)]
    pub supports_tools: bool,
    #[serde(default)]
    pub supports_tool_call_shadow_messages: bool,
    #[serde(default)]
    pub supports_vision: bool,
    #[serde(default)]
    pub supports_documents: bool,
    #[serde(default)]
    pub supports_audio: bool,
    #[serde(default)]
    pub supports_video: bool,
    #[serde(default)]
    pub preferred_attachment_mode: AttachmentRouteMode,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RuntimePolicyTrace {
    pub max_provider_round_trips: usize,
    pub max_workflow_provider_round_trips: usize,
    pub continue_after_tool_error: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProviderAttemptTrace {
    pub attempt: u8,
    pub max_attempts: u8,
    pub status: String,
    pub error_kind: Option<String>,
    pub status_code: Option<u16>,
    pub retryable: bool,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProviderFailureTrace {
    pub kind: String,
    pub status_code: Option<u16>,
    pub retryable: bool,
    pub message: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum RunLifecycleStatus {
    #[default]
    Unknown,
    Queued,
    Running,
    Streaming,
    CancelRequested,
    Success,
    Failed,
    Canceled,
}

impl RunLifecycleStatus {
    pub fn label(self) -> &'static str {
        match self {
            Self::Unknown => "unknown",
            Self::Queued => "queued",
            Self::Running => "running",
            Self::Streaming => "streaming",
            Self::CancelRequested => "cancel_requested",
            Self::Success => "success",
            Self::Failed => "failed",
            Self::Canceled => "canceled",
        }
    }

    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Success | Self::Failed | Self::Canceled)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RunFailureTrace {
    pub kind: String,
    pub stage: String,
    #[serde(default)]
    pub origin: FailureOrigin,
    pub retryable: bool,
    pub message: String,
}

impl SkillTrace {
    pub fn duration_ms(&self) -> Option<i64> {
        self.finished_at.map(|finished| {
            finished
                .signed_duration_since(self.started_at)
                .num_milliseconds()
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WorkflowStepTrace {
    pub name: String,
    pub kind: String,
    #[serde(default)]
    pub execution_target: Option<ExecutionTarget>,
    #[serde(default)]
    pub orchestration_owner: Option<OrchestrationOwner>,
    pub input: String,
    pub output: Option<String>,
    pub started_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
    pub error: Option<String>,
}

impl WorkflowStepTrace {
    pub fn duration_ms(&self) -> Option<i64> {
        self.finished_at.map(|finished| {
            finished
                .signed_duration_since(self.started_at)
                .num_milliseconds()
        })
    }

    pub fn status(&self) -> &'static str {
        if self.error.is_some() {
            "failed"
        } else if self.finished_at.is_some() {
            "success"
        } else {
            "running"
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MemoryReadTrace {
    pub session_id: String,
    pub source: String,
    pub preview: String,
    #[serde(default)]
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MemoryWriteTrace {
    pub session_id: String,
    pub kind: String,
    pub preview: String,
    #[serde(default)]
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CompressionTrace {
    pub original_message_count: usize,
    pub kept_recent_count: usize,
    pub summary_preview: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ModelSelectionTrace {
    pub scope: String,
    pub requested_profile: Option<String>,
    pub selected_profile: String,
    pub selected_model: String,
    pub reason: String,
    #[serde(default)]
    pub context_window_chars: usize,
    #[serde(default)]
    pub budget_tier: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GovernanceTrace {
    pub deployment_profile: String,
    pub workspace_name: String,
    pub auth_mode: String,
    pub audit_retention_days: u32,
    pub event_replay_window: usize,
    pub redact_inputs: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RunSummary {
    pub status: String,
    pub failure_kind: Option<String>,
    pub failure_origin: Option<String>,
    pub tool_calls: usize,
    pub capability_invocations: usize,
    pub skill_calls: usize,
    pub workflow_steps: usize,
    pub provider_attempts: usize,
    pub model_selections: usize,
    pub memory_reads: usize,
    pub memory_writes: usize,
    pub active_extensions: usize,
    pub used_extensions: usize,
    pub outbound_deliveries: usize,
    pub failed_outbound_deliveries: usize,
    pub attachments: usize,
    pub attachment_failures: usize,
    pub output_chunks: usize,
    pub integrity_warnings: usize,
    pub has_compression: bool,
    pub duration_ms: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RunTrace {
    pub run_id: String,
    pub gateway_run_id: Option<String>,
    pub correlation_id: Option<String>,
    pub session_id: Option<String>,
    pub session_route: Option<String>,
    pub ingress: Option<IngressTrace>,
    pub route_decision: Option<RouteDecisionTrace>,
    #[serde(default)]
    pub attachment_route: Option<AttachmentRouteTrace>,
    #[serde(default)]
    pub outbound_deliveries: Vec<ChannelDeliveryTrace>,
    pub workflow_name: Option<String>,
    #[serde(default)]
    pub lifecycle_status: RunLifecycleStatus,
    #[serde(default)]
    pub failure: Option<RunFailureTrace>,
    pub started_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
    pub input: String,
    pub output: Option<String>,
    #[serde(default)]
    pub output_chunks: usize,
    pub effective_profile: Option<EffectiveProfileTrace>,
    pub provider_failure: Option<ProviderFailureTrace>,
    #[serde(default)]
    pub provider_attempts: Vec<ProviderAttemptTrace>,
    pub governance: Option<GovernanceTrace>,
    pub runtime_policy: Option<RuntimePolicyTrace>,
    #[serde(default)]
    pub sandbox_run: Option<SandboxRunTrace>,
    #[serde(default)]
    pub model_selections: Vec<ModelSelectionTrace>,
    #[serde(default)]
    pub memory_reads: Vec<MemoryReadTrace>,
    #[serde(default)]
    pub memory_writes: Vec<MemoryWriteTrace>,
    pub compression: Option<CompressionTrace>,
    #[serde(default)]
    pub tool_calls: Vec<ToolTrace>,
    #[serde(default)]
    pub capability_invocations: Vec<CapabilityInvocationTrace>,
    pub side_effect_summary: Option<SideEffectSummary>,
    #[serde(default)]
    pub active_extensions: Vec<ExtensionTrace>,
    #[serde(default)]
    pub used_extensions: Vec<ExtensionUsageTrace>,
    #[serde(default)]
    pub skill_calls: Vec<SkillTrace>,
    #[serde(default)]
    pub step_traces: Vec<WorkflowStepTrace>,
    #[serde(default)]
    pub integrity_warnings: Vec<String>,
    pub error: Option<String>,
}

impl RunTrace {
    pub fn new(input: String) -> Self {
        Self::new_with_id(Uuid::new_v4().to_string(), input)
    }

    pub fn new_with_id(run_id: impl Into<String>, input: String) -> Self {
        Self {
            run_id: run_id.into(),
            gateway_run_id: None,
            correlation_id: None,
            session_id: None,
            session_route: None,
            ingress: None,
            route_decision: None,
            attachment_route: None,
            outbound_deliveries: vec![],
            workflow_name: None,
            lifecycle_status: RunLifecycleStatus::Queued,
            failure: None,
            started_at: Utc::now(),
            finished_at: None,
            input,
            output: None,
            output_chunks: 0,
            effective_profile: None,
            provider_failure: None,
            provider_attempts: vec![],
            governance: None,
            runtime_policy: None,
            sandbox_run: None,
            model_selections: vec![],
            memory_reads: vec![],
            memory_writes: vec![],
            compression: None,
            tool_calls: vec![],
            capability_invocations: vec![],
            side_effect_summary: None,
            active_extensions: vec![],
            used_extensions: vec![],
            skill_calls: vec![],
            step_traces: vec![],
            integrity_warnings: vec![],
            error: None,
        }
    }

    pub fn bind_session(&mut self, session_id: impl Into<String>) {
        self.session_id = Some(session_id.into());
    }

    pub fn bind_gateway_context(
        &mut self,
        gateway_run_id: impl Into<String>,
        correlation_id: impl Into<String>,
        session_route: impl Into<String>,
    ) {
        self.gateway_run_id = Some(gateway_run_id.into());
        self.correlation_id = Some(correlation_id.into());
        self.session_route = Some(session_route.into());
    }

    pub fn bind_ingress(&mut self, ingress: IngressTrace) {
        self.ingress = Some(ingress);
    }

    pub fn bind_route_decision(&mut self, route_decision: RouteDecisionTrace) {
        self.route_decision = Some(route_decision);
    }

    pub fn bind_attachment_route(&mut self, attachment_route: AttachmentRouteTrace) {
        self.attachment_route = Some(attachment_route);
    }

    pub fn add_outbound_delivery(&mut self, delivery: ChannelDeliveryTrace) {
        self.outbound_deliveries.push(delivery);
    }

    pub fn bind_extensions(&mut self, extensions: Vec<ExtensionTrace>) {
        self.active_extensions = extensions;
    }

    pub fn record_extension_usage(&mut self, usage: ExtensionUsageTrace) {
        let duplicate = self.used_extensions.iter().any(|existing| {
            existing.name == usage.name
                && existing.version == usage.version
                && existing.component_kind == usage.component_kind
                && existing.component_name == usage.component_name
        });

        if !duplicate {
            self.used_extensions.push(usage);
        }
    }

    pub fn bind_workflow(&mut self, workflow_name: impl Into<String>) {
        self.workflow_name = Some(workflow_name.into());
    }

    pub fn bind_effective_profile(&mut self, profile: EffectiveProfileTrace) {
        self.effective_profile = Some(profile);
    }

    pub fn bind_provider_failure(&mut self, failure: ProviderFailureTrace) {
        self.provider_failure = Some(failure);
    }

    pub fn bind_failure(&mut self, failure: RunFailureTrace) {
        self.failure = Some(failure);
    }

    pub fn add_provider_attempt(&mut self, attempt: ProviderAttemptTrace) {
        self.provider_attempts.push(attempt);
    }

    pub fn bind_governance(&mut self, governance: GovernanceTrace) {
        self.governance = Some(governance);
    }

    pub fn bind_runtime_policy(&mut self, runtime_policy: RuntimePolicyTrace) {
        self.runtime_policy = Some(runtime_policy);
    }

    pub fn bind_sandbox_run(&mut self, sandbox_run: SandboxRunTrace) {
        self.sandbox_run = Some(sandbox_run);
    }

    pub fn add_model_selection(&mut self, trace: ModelSelectionTrace) {
        self.model_selections.push(trace);
    }

    pub fn add_memory_read(&mut self, trace: MemoryReadTrace) {
        self.memory_reads.push(trace);
    }

    pub fn add_memory_write(&mut self, trace: MemoryWriteTrace) {
        self.memory_writes.push(trace);
    }

    pub fn bind_compression(&mut self, trace: CompressionTrace) {
        self.compression = Some(trace);
    }

    pub fn add_capability_invocation(&mut self, trace: CapabilityInvocationTrace) {
        self.capability_invocations.push(trace);
        self.side_effect_summary = Some(self.compute_side_effect_summary());
    }

    pub fn mark_running(&mut self) {
        self.lifecycle_status = RunLifecycleStatus::Running;
    }

    pub fn mark_streaming(&mut self) {
        if !matches!(
            self.lifecycle_status,
            RunLifecycleStatus::Canceled | RunLifecycleStatus::Failed | RunLifecycleStatus::Success
        ) {
            self.lifecycle_status = RunLifecycleStatus::Streaming;
        }
    }

    pub fn mark_cancel_requested(&mut self) {
        if !self.lifecycle_status.is_terminal() {
            self.lifecycle_status = RunLifecycleStatus::CancelRequested;
        }
    }

    pub fn record_output_chunk(&mut self) {
        self.output_chunks += 1;
        self.mark_streaming();
    }

    fn compute_side_effect_summary(&self) -> SideEffectSummary {
        let mut capability_kinds = self
            .capability_invocations
            .iter()
            .map(|trace| trace.kind.label().to_owned())
            .collect::<Vec<_>>();
        capability_kinds.sort();
        capability_kinds.dedup();

        SideEffectSummary {
            total: self.capability_invocations.len(),
            failed: self
                .capability_invocations
                .iter()
                .filter(|trace| trace.status != "success")
                .count(),
            high_risk: self
                .capability_invocations
                .iter()
                .filter(|trace| trace.risk == ToolRiskLevel::High)
                .count(),
            capability_kinds,
        }
    }

    pub fn finish_ok(&mut self, output: String) {
        self.finished_at = Some(Utc::now());
        self.output = Some(output);
        self.error = None;
        self.failure = None;
        self.lifecycle_status = RunLifecycleStatus::Success;
        self.integrity_warnings = self.validate_integrity();
    }

    pub fn finish_err(&mut self, error: String) {
        self.finished_at = Some(Utc::now());
        self.error = Some(error);
        if self.lifecycle_status != RunLifecycleStatus::Canceled {
            self.lifecycle_status = RunLifecycleStatus::Failed;
        }
        self.integrity_warnings = self.validate_integrity();
    }

    pub fn finish_canceled(&mut self, reason: String) {
        self.finished_at = Some(Utc::now());
        self.error = Some(reason.clone());
        self.failure = Some(RunFailureTrace {
            kind: "canceled".to_owned(),
            stage: "gateway".to_owned(),
            origin: FailureOrigin::Gateway,
            retryable: true,
            message: reason,
        });
        self.lifecycle_status = RunLifecycleStatus::Canceled;
        self.integrity_warnings = self.validate_integrity();
    }

    pub fn duration_ms(&self) -> Option<i64> {
        self.finished_at.map(|finished| {
            finished
                .signed_duration_since(self.started_at)
                .num_milliseconds()
        })
    }

    pub fn lifecycle_status(&self) -> RunLifecycleStatus {
        if self.lifecycle_status != RunLifecycleStatus::Unknown {
            return self.lifecycle_status;
        }

        if self.error.is_some() {
            RunLifecycleStatus::Failed
        } else if self.finished_at.is_some() {
            RunLifecycleStatus::Success
        } else {
            RunLifecycleStatus::Running
        }
    }

    pub fn status(&self) -> &'static str {
        self.lifecycle_status().label()
    }

    pub fn validate_integrity(&self) -> Vec<String> {
        let mut warnings = Vec::new();
        let status = self.lifecycle_status();
        if status.is_terminal() && self.finished_at.is_none() {
            warnings.push("terminal run is missing finished_at".to_owned());
        }
        if matches!(status, RunLifecycleStatus::Success) && self.output.is_none() {
            warnings.push("successful run is missing output".to_owned());
        }
        if matches!(
            status,
            RunLifecycleStatus::Failed | RunLifecycleStatus::Canceled
        ) && self.error.is_none()
        {
            warnings.push("failed or canceled run is missing error text".to_owned());
        }
        if self.finished_at.is_some()
            && matches!(
                status,
                RunLifecycleStatus::Queued
                    | RunLifecycleStatus::Running
                    | RunLifecycleStatus::Streaming
                    | RunLifecycleStatus::CancelRequested
            )
        {
            warnings.push("finished run still reports a non-terminal lifecycle state".to_owned());
        }
        if self.provider_failure.is_some()
            && self
                .failure
                .as_ref()
                .is_some_and(|failure| failure.kind != "provider")
        {
            warnings.push(
                "provider_failure is present but top-level failure kind is not provider".to_owned(),
            );
        }
        warnings
    }

    pub fn summary(&self) -> RunSummary {
        RunSummary {
            status: self.status().to_owned(),
            failure_kind: self.failure.as_ref().map(|failure| failure.kind.clone()),
            failure_origin: self
                .failure
                .as_ref()
                .map(|failure| failure.origin.label().to_owned()),
            tool_calls: self.tool_calls.len(),
            capability_invocations: self.capability_invocations.len(),
            skill_calls: self.skill_calls.len(),
            workflow_steps: self.step_traces.len(),
            provider_attempts: self.provider_attempts.len(),
            model_selections: self.model_selections.len(),
            memory_reads: self.memory_reads.len(),
            memory_writes: self.memory_writes.len(),
            active_extensions: self.active_extensions.len(),
            used_extensions: self.used_extensions.len(),
            outbound_deliveries: self.outbound_deliveries.len(),
            failed_outbound_deliveries: self
                .outbound_deliveries
                .iter()
                .filter(|delivery| delivery.result.status == ChannelDeliveryStatus::Failed)
                .count(),
            attachments: self
                .ingress
                .as_ref()
                .map(|ingress| ingress.attachments.len())
                .unwrap_or_default(),
            attachment_failures: self
                .ingress
                .as_ref()
                .map(|ingress| ingress.attachment_failures.len())
                .unwrap_or_default(),
            output_chunks: self.output_chunks,
            integrity_warnings: self.integrity_warnings.len(),
            has_compression: self.compression.is_some(),
            duration_ms: self.duration_ms(),
        }
    }

    pub fn capability_explanations(&self) -> Vec<CapabilityExplanationTrace> {
        let mut explanations = Vec::new();

        for invocation in &self.capability_invocations {
            let mut decision = Vec::new();
            if let Some(source_name) = invocation.source_name.as_deref() {
                decision.push(format!("source={source_name}"));
            }
            if let Some(source_path) = invocation.source_path.as_deref() {
                decision.push(format!("source_path={source_path}"));
            }
            if let Some(policy_source) = invocation.policy_source.as_deref() {
                decision.push(format!("policy={policy_source}"));
            }
            if let Some(sandbox_scope) = invocation.sandbox_scope.as_deref() {
                decision.push(format!("sandbox_scope={sandbox_scope}"));
            }
            if let Some(route) = invocation.capability_route.as_deref() {
                decision.push(format!("node_route={route}"));
            }
            if invocation.node_fallback_to_local {
                decision.push("fallback_to_local=true".to_owned());
            }
            if let Some(node_failure_class) = invocation.node_failure_class.as_deref() {
                decision.push(format!("node_failure_class={node_failure_class}"));
            }
            if let Some(node_id) = invocation.node_id.as_deref() {
                decision.push(format!("node_id={node_id}"));
            }
            if let Some(target) = invocation.target.as_deref() {
                decision.push(format!("target={target}"));
            }

            explanations.push(CapabilityExplanationTrace {
                scope: "capability".to_owned(),
                name: invocation.tool_name.clone(),
                route_kind: invocation.route_kind.map(|kind| kind.label().to_owned()),
                capability_source_kind: invocation
                    .capability_source_kind
                    .map(|kind| kind.label().to_owned()),
                execution_target: Some(invocation.execution_target.label().to_owned()),
                orchestration_owner: Some(invocation.orchestration_owner.label().to_owned()),
                status: invocation.status.clone(),
                summary: invocation.summary.clone(),
                decision_basis: (!decision.is_empty()).then(|| decision.join(" | ")),
                failure_origin: invocation
                    .failure_origin
                    .map(|origin| origin.label().to_owned()),
            });
        }

        for skill in &self.skill_calls {
            let mut decision = Vec::new();
            if let Some(source_kind) = skill.source_kind.as_deref() {
                decision.push(format!("source_kind={source_kind}"));
            }
            if let Some(source_name) = skill.source_name.as_deref() {
                decision.push(format!("source={source_name}"));
            }
            if let Some(policy_source) = skill.policy_source.as_deref() {
                decision.push(format!("policy={policy_source}"));
            }
            if let Some(sandbox_scope) = skill.sandbox_scope.as_deref() {
                decision.push(format!("sandbox_scope={sandbox_scope}"));
            }
            if let Some(sandbox) = skill.sandbox.as_ref() {
                decision.push(format!("sandbox_env={}", sandbox.env_id));
            }
            if let Some(pack) = skill.markdown_pack.as_ref() {
                if let Some(template) = pack.template.as_deref() {
                    decision.push(format!("template={template}"));
                }
                if !pack.references.is_empty() {
                    decision.push(format!("references={}", pack.references.join(",")));
                }
                if let Some(script) = pack.script.as_deref() {
                    decision.push(format!("script={script}"));
                }
                if let Some(runtime) = pack.script_runtime.as_deref() {
                    decision.push(format!("script_runtime={runtime}"));
                }
                if pack.attachment_count > 0 {
                    decision.push(format!("attachments={}", pack.attachment_count));
                }
            }

            explanations.push(CapabilityExplanationTrace {
                scope: "skill".to_owned(),
                name: skill.name.clone(),
                route_kind: Some("skill".to_owned()),
                capability_source_kind: skill
                    .capability_source_kind
                    .map(|kind| kind.label().to_owned()),
                execution_target: Some(skill.execution_target.label().to_owned()),
                orchestration_owner: Some(skill.orchestration_owner.label().to_owned()),
                status: if skill.output.is_some() {
                    "success".to_owned()
                } else if skill.finished_at.is_some() {
                    "failed".to_owned()
                } else {
                    "running".to_owned()
                },
                summary: skill
                    .output
                    .as_deref()
                    .map(|output| truncate_preview(output, 180))
                    .unwrap_or_else(|| format!("skill {}", skill.name)),
                decision_basis: (!decision.is_empty()).then(|| decision.join(" | ")),
                failure_origin: None,
            });
        }

        for step in &self.step_traces {
            let mut decision = vec![format!("kind={}", step.kind)];
            if let Some(owner) = step.orchestration_owner {
                decision.push(format!("owner={}", owner.label()));
            }
            if let Some(target) = step.execution_target {
                decision.push(format!("target={}", target.label()));
            }

            explanations.push(CapabilityExplanationTrace {
                scope: "workflow_step".to_owned(),
                name: step.name.clone(),
                route_kind: Some("workflow".to_owned()),
                capability_source_kind: None,
                execution_target: step
                    .execution_target
                    .map(|target| target.label().to_owned()),
                orchestration_owner: step
                    .orchestration_owner
                    .map(|owner| owner.label().to_owned()),
                status: step.status().to_owned(),
                summary: step
                    .output
                    .as_deref()
                    .map(|output| truncate_preview(output, 180))
                    .unwrap_or_else(|| truncate_preview(&step.input, 180)),
                decision_basis: Some(decision.join(" | ")),
                failure_origin: step.error.as_ref().map(|_| "workflow".to_owned()),
            });
        }

        explanations
    }

    pub fn save_to_default_dir(&self) -> Result<PathBuf> {
        self.save_to_dir(PathBuf::from(".mosaic/runs"))
    }

    pub fn save_to_dir(&self, dir: impl AsRef<Path>) -> Result<PathBuf> {
        let dir = dir.as_ref();
        fs::create_dir_all(dir)?;
        let path = dir.join(format!("{}.json", self.run_id));
        fs::write(&path, serde_json::to_vec_pretty(self)?)?;
        Ok(path)
    }
}

fn truncate_preview(value: &str, limit: usize) -> String {
    let sanitized = value.replace('\n', " ");
    let char_count = sanitized.chars().count();
    if char_count <= limit {
        return sanitized;
    }

    let truncated = sanitized.chars().take(limit).collect::<String>();
    format!("{truncated}...")
}

#[cfg(test)]
mod tests {
    use std::{
        process,
        sync::atomic::{AtomicU64, Ordering},
        time::{SystemTime, UNIX_EPOCH},
    };

    use chrono::Duration;

    use super::*;

    static COUNTER: AtomicU64 = AtomicU64::new(0);

    fn temp_dir(label: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock should be after epoch")
            .as_nanos();
        let count = COUNTER.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!(
            "mosaic-inspect-{label}-{}-{nanos}-{count}",
            process::id()
        ))
    }

    #[test]
    fn saves_trace_to_a_custom_directory() {
        let dir = temp_dir("trace");
        let mut trace = RunTrace::new("hello".to_owned());
        trace.bind_ingress(IngressTrace {
            kind: "remote_operator".to_owned(),
            channel: Some("cli".to_owned()),
            adapter: Some("cli_remote".to_owned()),
            bot_name: None,
            bot_route: None,
            bot_profile: None,
            bot_token_env: None,
            bot_secret_env: None,
            source: Some("mosaic-cli".to_owned()),
            remote_addr: None,
            display_name: None,
            actor_id: None,
            conversation_id: Some("cli:operator".to_owned()),
            thread_id: None,
            thread_title: None,
            reply_target: None,
            message_id: None,
            received_at: None,
            raw_event_id: None,
            session_hint: None,
            profile_hint: None,
            control_command: None,
            original_text: None,
            attachments: Vec::new(),
            attachment_failures: Vec::new(),
            gateway_url: Some("http://127.0.0.1:8080".to_owned()),
        });
        trace.add_memory_read(MemoryReadTrace {
            session_id: "demo".to_owned(),
            source: "session_summary".to_owned(),
            preview: "Stored summary".to_owned(),
            tags: vec![],
        });
        trace.add_memory_write(MemoryWriteTrace {
            session_id: "demo".to_owned(),
            kind: "summary".to_owned(),
            preview: "New summary".to_owned(),
            tags: vec!["session".to_owned()],
        });
        trace.bind_compression(CompressionTrace {
            original_message_count: 12,
            kept_recent_count: 6,
            summary_preview: "Compressed older turns".to_owned(),
        });
        trace.tool_calls.push(ToolTrace {
            call_id: Some("call-1".to_owned()),
            name: "echo".to_owned(),
            source: ToolSource::Builtin,
            capability_source_kind: Some(CapabilitySourceKind::Builtin),
            source_name: Some("builtin.core".to_owned()),
            source_path: None,
            source_version: None,
            input: serde_json::json!({ "text": "hello" }),
            output: Some("hello".to_owned()),
            node_attempted: false,
            node_fallback_to_local: false,
            node_failure_class: None,
            node_id: None,
            capability_route: None,
            disconnect_context: None,
            effective_execution_target: "local".to_owned(),
            execution_target: ExecutionTarget::Local,
            orchestration_owner: OrchestrationOwner::Runtime,
            policy_source: None,
            sandbox_scope: None,
            sandbox: None,
            started_at: Utc::now(),
            finished_at: Some(Utc::now()),
        });
        trace.finish_ok("world".to_owned());

        let path = trace.save_to_dir(&dir).expect("trace should save");
        let content = fs::read_to_string(&path).expect("saved trace should be readable");
        let loaded: RunTrace = serde_json::from_str(&content).expect("trace should deserialize");

        assert_eq!(loaded.input, "hello");
        assert_eq!(loaded.output.as_deref(), Some("world"));
        assert_eq!(loaded.tool_calls[0].call_id.as_deref(), Some("call-1"));
        assert_eq!(
            loaded
                .ingress
                .as_ref()
                .and_then(|ingress| ingress.gateway_url.as_deref()),
            Some("http://127.0.0.1:8080")
        );
        assert_eq!(loaded.memory_reads.len(), 1);
        assert!(loaded.compression.is_some());

        fs::remove_file(path).ok();
        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn trace_summary_reports_status_counts_and_duration() {
        let started_at = Utc::now();
        let finished_at = started_at + Duration::milliseconds(18);

        let trace = RunTrace {
            run_id: "run-1".to_owned(),
            gateway_run_id: Some("gateway-run-1".to_owned()),
            correlation_id: Some("corr-1".to_owned()),
            session_id: Some("session-1".to_owned()),
            session_route: Some("gateway.local/session-1".to_owned()),
            ingress: None,
            route_decision: None,
            attachment_route: None,
            sandbox_run: None,
            outbound_deliveries: vec![],
            workflow_name: Some("research_brief".to_owned()),
            started_at,
            finished_at: Some(finished_at),
            input: "hello".to_owned(),
            output: Some("world".to_owned()),
            effective_profile: Some(EffectiveProfileTrace {
                profile: "gpt-5.4-mini".to_owned(),
                provider_type: "openai".to_owned(),
                model: "gpt-5.4-mini".to_owned(),
                base_url: Some("https://api.openai.com/v1".to_owned()),
                api_key_env: Some("OPENAI_API_KEY".to_owned()),
                api_key_present: true,
                timeout_ms: 45_000,
                max_retries: 2,
                retry_backoff_ms: 150,
                api_version: None,
                version_header: None,
                custom_header_keys: Vec::new(),
                supports_tools: true,
                supports_tool_call_shadow_messages: false,
                supports_vision: true,
                supports_documents: true,
                supports_audio: false,
                supports_video: false,
                preferred_attachment_mode: AttachmentRouteMode::ProviderNative,
            }),
            runtime_policy: Some(RuntimePolicyTrace {
                max_provider_round_trips: 8,
                max_workflow_provider_round_trips: 8,
                continue_after_tool_error: false,
            }),
            lifecycle_status: RunLifecycleStatus::Success,
            failure: None,
            output_chunks: 1,
            integrity_warnings: Vec::new(),
            provider_failure: None,
            provider_attempts: vec![],
            governance: Some(GovernanceTrace {
                deployment_profile: "local".to_owned(),
                workspace_name: "test-workspace".to_owned(),
                auth_mode: "open".to_owned(),
                audit_retention_days: 14,
                event_replay_window: 256,
                redact_inputs: true,
            }),
            model_selections: vec![ModelSelectionTrace {
                scope: "run".to_owned(),
                requested_profile: None,
                selected_profile: "gpt-5.4-mini".to_owned(),
                selected_model: "gpt-5.4-mini".to_owned(),
                reason: "active_profile".to_owned(),
                context_window_chars: 128000,
                budget_tier: "small".to_owned(),
            }],
            memory_reads: vec![],
            memory_writes: vec![],
            compression: None,
            tool_calls: vec![ToolTrace {
                call_id: Some("call-1".to_owned()),
                name: "echo".to_owned(),
                source: ToolSource::Builtin,
                capability_source_kind: Some(CapabilitySourceKind::Builtin),
                source_name: Some("builtin.core".to_owned()),
                source_path: None,
                source_version: None,
                input: serde_json::json!({ "text": "hello" }),
                output: Some("hello".to_owned()),
                node_attempted: false,
                node_fallback_to_local: false,
                node_failure_class: None,
                node_id: None,
                capability_route: None,
                disconnect_context: None,
                effective_execution_target: "local".to_owned(),
                execution_target: ExecutionTarget::Local,
                orchestration_owner: OrchestrationOwner::Runtime,
                policy_source: None,
                sandbox_scope: None,
                sandbox: None,
                started_at,
                finished_at: Some(started_at + Duration::milliseconds(3)),
            }],
            capability_invocations: vec![],
            side_effect_summary: None,
            active_extensions: vec![],
            used_extensions: vec![],
            skill_calls: vec![],
            step_traces: vec![WorkflowStepTrace {
                name: "draft".to_owned(),
                kind: "prompt".to_owned(),
                execution_target: Some(ExecutionTarget::Provider),
                orchestration_owner: Some(OrchestrationOwner::WorkflowEngine),
                input: "hello".to_owned(),
                output: Some("world".to_owned()),
                started_at,
                finished_at: Some(started_at + Duration::milliseconds(9)),
                error: None,
            }],
            error: None,
        };

        let summary = trace.summary();

        assert_eq!(trace.status(), "success");
        assert_eq!(trace.duration_ms(), Some(18));
        assert_eq!(summary.status, "success");
        assert_eq!(summary.tool_calls, 1);
        assert_eq!(summary.capability_invocations, 0);
        assert_eq!(summary.skill_calls, 0);
        assert_eq!(summary.workflow_steps, 1);
        assert_eq!(summary.provider_attempts, 0);
        assert_eq!(summary.model_selections, 1);
        assert_eq!(summary.memory_reads, 0);
        assert_eq!(summary.memory_writes, 0);
        assert_eq!(summary.active_extensions, 0);
        assert_eq!(summary.used_extensions, 0);
        assert_eq!(summary.outbound_deliveries, 0);
        assert_eq!(summary.failed_outbound_deliveries, 0);
        assert!(!summary.has_compression);
        assert_eq!(summary.output_chunks, 1);
        assert_eq!(summary.integrity_warnings, 0);
        assert_eq!(summary.duration_ms, Some(18));
        assert_eq!(trace.tool_calls[0].duration_ms(), Some(3));
        assert_eq!(trace.step_traces[0].duration_ms(), Some(9));
        assert_eq!(trace.step_traces[0].status(), "success");
        assert_eq!(trace.model_selections.len(), 1);
    }

    #[test]
    fn trace_status_reports_failure_when_error_exists() {
        let mut trace = RunTrace::new("hello".to_owned());
        trace.finish_err("boom".to_owned());

        assert_eq!(trace.status(), "failed");
        assert_eq!(trace.summary().status, "failed");
        assert!(trace.duration_ms().is_some());
    }

    #[test]
    fn bind_ingress_updates_trace_metadata() {
        let mut trace = RunTrace::new("hello".to_owned());
        trace.bind_ingress(IngressTrace {
            kind: "webchat".to_owned(),
            channel: Some("webchat".to_owned()),
            adapter: Some("webchat_http".to_owned()),
            bot_name: None,
            bot_route: None,
            bot_profile: None,
            bot_token_env: None,
            bot_secret_env: None,
            source: Some("browser".to_owned()),
            remote_addr: Some("127.0.0.1".to_owned()),
            display_name: Some("guest".to_owned()),
            actor_id: Some("guest-1".to_owned()),
            conversation_id: Some("webchat:lobby".to_owned()),
            thread_id: Some("room-7".to_owned()),
            thread_title: Some("Launch Room".to_owned()),
            reply_target: Some("webchat:guest-1".to_owned()),
            message_id: Some("message-1".to_owned()),
            received_at: Some(Utc::now()),
            raw_event_id: Some("event-1".to_owned()),
            session_hint: Some("webchat-demo".to_owned()),
            profile_hint: Some("gpt-5.4-mini".to_owned()),
            control_command: None,
            original_text: None,
            attachments: Vec::new(),
            attachment_failures: Vec::new(),
            gateway_url: None,
        });

        assert_eq!(
            trace.ingress.as_ref().map(|ingress| ingress.kind.as_str()),
            Some("webchat")
        );
        assert_eq!(
            trace
                .ingress
                .as_ref()
                .and_then(|ingress| ingress.display_name.as_deref()),
            Some("guest")
        );
        assert_eq!(
            trace
                .ingress
                .as_ref()
                .and_then(|ingress| ingress.conversation_id.as_deref()),
            Some("webchat:lobby")
        );
    }

    #[test]
    fn legacy_effective_profile_fields_default_when_missing() {
        let raw = serde_json::json!({
            "run_id": "legacy-run",
            "started_at": Utc::now(),
            "input": "hello",
            "tool_calls": [],
            "capability_invocations": [],
            "active_extensions": [],
            "used_extensions": [],
            "skill_calls": [],
            "step_traces": [],
            "provider_attempts": [],
            "model_selections": [],
            "memory_reads": [],
            "memory_writes": [],
            "effective_profile": {
                "profile": "demo-provider",
                "provider_type": "openai-compatible",
                "model": "demo-model",
                "base_url": "https://gateway.example/v1",
                "api_key_env": "COMPAT_API_KEY"
            }
        });

        let trace: RunTrace = serde_json::from_value(raw).expect("legacy trace should deserialize");
        let effective = trace
            .effective_profile
            .expect("effective profile should exist");
        assert_eq!(effective.timeout_ms, 0);
        assert_eq!(effective.max_retries, 0);
        assert!(!effective.supports_tools);
    }
}
