use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use mosaic_inspect::{RunLifecycleStatus, RunTrace};
use mosaic_runtime::events::RunEvent;
use serde::{Deserialize, Serialize};

pub use mosaic_inspect::{
    AttachmentFailureTrace, AttachmentKind, AttachmentRouteMode, AttachmentRouteTrace,
    ChannelAttachment, ChannelDeliveryResult, ChannelDeliveryStatus, ChannelDeliveryTrace,
    ChannelOutboundMessage, IngressTrace, RouteDecisionTrace, RouteMode,
};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HealthResponse {
    pub status: String,
    pub active_profile: String,
    pub session_count: usize,
    pub transport: String,
    pub deployment_profile: String,
    pub auth_mode: String,
    pub event_replay_window: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReadinessResponse {
    pub status: String,
    pub transport: String,
    pub deployment_profile: String,
    pub auth_mode: String,
    pub session_store_ready: bool,
    pub audit_ready: bool,
    pub extension_count: usize,
    pub session_count: usize,
    pub replay_events_buffered: usize,
    pub event_replay_window: usize,
    pub slow_consumer_lag_threshold: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MetricsResponse {
    pub transport: String,
    pub deployment_profile: String,
    pub auth_mode: String,
    pub session_count: usize,
    pub capability_job_count: usize,
    pub queued_run_count: usize,
    pub running_run_count: usize,
    pub completed_runs_total: u64,
    pub failed_runs_total: u64,
    pub canceled_runs_total: u64,
    pub capability_jobs_total: u64,
    pub audit_events_total: u64,
    pub auth_denials_total: u64,
    pub broadcast_lag_events_total: u64,
    pub replay_events_buffered: usize,
    pub event_replay_window: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GatewayAuditEventDto {
    pub id: String,
    pub kind: String,
    pub outcome: String,
    pub summary: String,
    pub actor: Option<String>,
    pub session_id: Option<String>,
    pub gateway_run_id: Option<String>,
    pub correlation_id: Option<String>,
    pub channel: Option<String>,
    pub bot_name: Option<String>,
    pub bot_route: Option<String>,
    pub target: Option<String>,
    pub emitted_at: DateTime<Utc>,
    pub redacted: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ReplayWindowResponse {
    pub capacity: usize,
    pub dropped_events_total: u64,
    pub events: Vec<EventStreamEnvelope>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct IncidentBundleDto {
    pub identifier: String,
    pub generated_at: DateTime<Utc>,
    pub deployment_profile: String,
    pub auth_mode: String,
    pub redaction_policy: String,
    pub trace: RunTrace,
    pub run: Option<RunDetailDto>,
    pub audit_events: Vec<GatewayAuditEventDto>,
    pub metrics: MetricsResponse,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AdapterStatusDto {
    pub name: String,
    pub channel: String,
    pub transport: String,
    pub ingress_path: String,
    #[serde(default)]
    pub bot_name: Option<String>,
    #[serde(default)]
    pub bot_route: Option<String>,
    #[serde(default)]
    pub bot_profile: Option<String>,
    #[serde(default)]
    pub bot_token_env: Option<String>,
    #[serde(default)]
    pub capabilities: Vec<String>,
    pub outbound_ready: bool,
    pub status: String,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChannelInboundMessage {
    pub channel: String,
    pub adapter: String,
    pub bot_name: Option<String>,
    pub bot_route: Option<String>,
    pub bot_profile: Option<String>,
    pub bot_token_env: Option<String>,
    pub actor_id: Option<String>,
    pub display_name: Option<String>,
    pub conversation_id: String,
    pub thread_id: Option<String>,
    pub thread_title: Option<String>,
    pub reply_target: String,
    pub message_id: String,
    pub text: String,
    #[serde(default)]
    pub attachments: Vec<ChannelAttachment>,
    pub profile_hint: Option<String>,
    pub session_hint: Option<String>,
    pub received_at: DateTime<Utc>,
    pub raw_event_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RunSubmission {
    pub system: Option<String>,
    pub input: String,
    pub tool: Option<String>,
    pub skill: Option<String>,
    pub workflow: Option<String>,
    pub session_id: Option<String>,
    pub profile: Option<String>,
    pub ingress: Option<IngressTrace>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RunResponse {
    pub gateway_run_id: String,
    pub correlation_id: String,
    pub session_route: String,
    pub output: String,
    pub trace: RunTrace,
    pub session_summary: Option<SessionSummaryDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RunSummaryDto {
    pub gateway_run_id: String,
    pub correlation_id: String,
    pub run_id: String,
    pub session_id: Option<String>,
    pub session_route: String,
    #[serde(default)]
    pub status: RunLifecycleStatus,
    pub requested_profile: Option<String>,
    pub effective_profile: Option<String>,
    pub effective_provider_type: Option<String>,
    pub effective_model: Option<String>,
    pub tool: Option<String>,
    pub skill: Option<String>,
    pub workflow: Option<String>,
    pub retry_of: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
    pub input_preview: String,
    pub output_preview: Option<String>,
    pub error: Option<String>,
    pub failure_kind: Option<String>,
    pub trace_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RunDetailDto {
    pub summary: RunSummaryDto,
    pub ingress: Option<IngressTrace>,
    #[serde(default)]
    pub outbound_deliveries: Vec<ChannelDeliveryTrace>,
    pub submission: RunSubmission,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CapabilityJobDto {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub risk: String,
    #[serde(default)]
    pub permission_scopes: Vec<String>,
    pub status: String,
    pub summary: Option<String>,
    pub target: Option<String>,
    pub session_id: Option<String>,
    pub gateway_run_id: Option<String>,
    pub correlation_id: Option<String>,
    pub started_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExecJobRequest {
    pub session_id: Option<String>,
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    pub cwd: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WebhookJobRequest {
    pub session_id: Option<String>,
    pub url: String,
    pub method: Option<String>,
    pub body: Option<String>,
    #[serde(default)]
    pub headers: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CronRegistrationDto {
    pub id: String,
    pub schedule: String,
    pub input: String,
    pub session_id: Option<String>,
    pub profile: Option<String>,
    pub skill: Option<String>,
    pub workflow: Option<String>,
    pub created_at: DateTime<Utc>,
    pub last_triggered_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CronRegistrationRequest {
    pub id: String,
    pub schedule: String,
    pub input: String,
    pub session_id: Option<String>,
    pub profile: Option<String>,
    pub skill: Option<String>,
    pub workflow: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExtensionPolicyDto {
    pub allow_exec: bool,
    pub allow_webhook: bool,
    pub allow_cron: bool,
    pub allow_mcp: bool,
    pub hot_reload_enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExtensionStatusDto {
    pub name: String,
    pub version: String,
    pub source: String,
    pub enabled: bool,
    pub active: bool,
    #[serde(default)]
    pub tools: Vec<String>,
    #[serde(default)]
    pub skills: Vec<String>,
    #[serde(default)]
    pub workflows: Vec<String>,
    #[serde(default)]
    pub mcp_servers: Vec<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InboundMessage {
    pub session_id: Option<String>,
    #[serde(alias = "text")]
    pub input: String,
    pub profile: Option<String>,
    pub display_name: Option<String>,
    pub actor_id: Option<String>,
    pub conversation_id: Option<String>,
    pub thread_id: Option<String>,
    pub thread_title: Option<String>,
    pub reply_target: Option<String>,
    pub message_id: Option<String>,
    pub received_at: Option<DateTime<Utc>>,
    pub raw_event_id: Option<String>,
    pub ingress: Option<IngressTrace>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum GatewayEvent {
    InboundReceived {
        ingress: IngressTrace,
        text_preview: String,
    },
    RunSubmitted {
        input: String,
        profile: String,
        ingress: Option<IngressTrace>,
    },
    Runtime(RunEvent),
    RunUpdated {
        run: RunSummaryDto,
    },
    CapabilityJobUpdated {
        job: CapabilityJobDto,
    },
    CronUpdated {
        registration: CronRegistrationDto,
    },
    ExtensionsReloaded {
        extensions: Vec<ExtensionStatusDto>,
        policies: ExtensionPolicyDto,
    },
    ExtensionReloadFailed {
        error: String,
    },
    SessionUpdated {
        summary: SessionSummaryDto,
    },
    RunCompleted {
        output_preview: String,
    },
    OutboundDelivered {
        delivery: ChannelDeliveryTrace,
    },
    OutboundFailed {
        delivery: ChannelDeliveryTrace,
    },
    RunFailed {
        error: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EventStreamEnvelope {
    pub gateway_run_id: String,
    pub correlation_id: String,
    pub session_id: Option<String>,
    pub session_route: String,
    pub emitted_at: DateTime<Utc>,
    pub event: GatewayEvent,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SessionSummaryDto {
    pub id: String,
    pub title: String,
    pub updated_at: DateTime<Utc>,
    pub provider_profile: String,
    pub provider_type: String,
    pub model: String,
    pub session_route: String,
    #[serde(default)]
    pub channel_context: SessionChannelDto,
    #[serde(default)]
    pub run: SessionRunDto,
    pub last_gateway_run_id: Option<String>,
    pub last_correlation_id: Option<String>,
    pub message_count: usize,
    pub last_message_preview: Option<String>,
    pub memory_summary_preview: Option<String>,
    pub reference_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SessionDetailDto {
    pub id: String,
    pub title: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub provider_profile: String,
    pub provider_type: String,
    pub model: String,
    pub last_run_id: Option<String>,
    #[serde(default)]
    pub channel_context: SessionChannelDto,
    #[serde(default)]
    pub run: SessionRunDto,
    pub gateway: SessionGatewayDto,
    pub memory_summary: Option<String>,
    pub compressed_context: Option<String>,
    pub references: Vec<SessionReferenceDto>,
    pub transcript: Vec<TranscriptMessageDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SessionGatewayDto {
    pub route: String,
    pub last_gateway_run_id: Option<String>,
    pub last_correlation_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct SessionRunDto {
    pub current_run_id: Option<String>,
    pub current_gateway_run_id: Option<String>,
    pub current_correlation_id: Option<String>,
    #[serde(default)]
    pub status: RunLifecycleStatus,
    pub last_error: Option<String>,
    pub last_failure_kind: Option<String>,
    pub updated_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct SessionChannelDto {
    pub ingress_kind: Option<String>,
    pub channel: Option<String>,
    pub adapter: Option<String>,
    pub bot_name: Option<String>,
    pub bot_route: Option<String>,
    pub bot_profile: Option<String>,
    pub bot_token_env: Option<String>,
    pub source: Option<String>,
    pub actor_id: Option<String>,
    pub actor_name: Option<String>,
    pub conversation_id: Option<String>,
    pub thread_id: Option<String>,
    pub thread_title: Option<String>,
    pub reply_target: Option<String>,
    pub last_message_id: Option<String>,
    pub last_delivery_id: Option<String>,
    pub last_delivery_status: Option<String>,
    pub last_delivery_error: Option<String>,
    pub last_delivery_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SessionReferenceDto {
    pub session_id: String,
    pub reason: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TranscriptMessageDto {
    pub role: TranscriptRoleDto,
    pub content: String,
    pub tool_call_id: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TranscriptRoleDto {
    System,
    User,
    Assistant,
    Tool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ErrorResponse {
    pub error: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_submission_roundtrips_through_json() {
        let submission = RunSubmission {
            system: Some("You are helpful.".to_owned()),
            input: "hello".to_owned(),
            tool: None,
            skill: None,
            workflow: None,
            session_id: Some("demo".to_owned()),
            profile: Some("demo-provider".to_owned()),
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
                remote_addr: None,
                display_name: None,
                actor_id: None,
                conversation_id: None,
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
                gateway_url: Some("http://127.0.0.1:8080".to_owned()),
                attachments: vec![],
                attachment_failures: vec![],
            }),
        };

        let encoded = serde_json::to_vec(&submission).expect("submission should serialize");
        let decoded: RunSubmission =
            serde_json::from_slice(&encoded).expect("submission should deserialize");

        assert_eq!(decoded, submission);
    }

    #[test]
    fn capability_job_roundtrips_through_json() {
        let job = CapabilityJobDto {
            id: "job-1".to_owned(),
            name: "exec_command".to_owned(),
            kind: "exec".to_owned(),
            risk: "high".to_owned(),
            permission_scopes: vec!["local_exec".to_owned()],
            status: "success".to_owned(),
            summary: Some("exec pwd finished with code 0".to_owned()),
            target: Some("pwd".to_owned()),
            session_id: Some("demo".to_owned()),
            gateway_run_id: Some("gateway-run-1".to_owned()),
            correlation_id: Some("corr-1".to_owned()),
            started_at: Utc::now(),
            finished_at: Some(Utc::now()),
            error: None,
        };

        let encoded = serde_json::to_vec(&job).expect("job should serialize");
        let decoded: CapabilityJobDto =
            serde_json::from_slice(&encoded).expect("job should deserialize");

        assert_eq!(decoded, job);
    }

    #[test]
    fn event_envelope_roundtrips_through_json() {
        let envelope = EventStreamEnvelope {
            gateway_run_id: "run-1".to_owned(),
            correlation_id: "corr-1".to_owned(),
            session_id: Some("demo".to_owned()),
            session_route: "gateway.local/demo".to_owned(),
            emitted_at: Utc::now(),
            event: GatewayEvent::RunSubmitted {
                input: "hello".to_owned(),
                profile: "demo-provider".to_owned(),
                ingress: Some(IngressTrace {
                    kind: "webchat".to_owned(),
                    channel: Some("webchat".to_owned()),
                    adapter: Some("webchat_http".to_owned()),
                    bot_name: None,
                    bot_route: None,
                    bot_profile: None,
                    bot_token_env: None,
                    bot_secret_env: None,
                    source: Some("browser".to_owned()),
                    remote_addr: None,
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
                    profile_hint: None,
                    control_command: None,
                    original_text: None,
                    gateway_url: None,
                    attachments: vec![],
                    attachment_failures: vec![],
                }),
            },
        };

        let encoded = serde_json::to_vec(&envelope).expect("envelope should serialize");
        let decoded: EventStreamEnvelope =
            serde_json::from_slice(&encoded).expect("envelope should deserialize");

        assert_eq!(decoded, envelope);
    }
}
