use chrono::{DateTime, Utc};
use mosaic_inspect::RunTrace;
use mosaic_runtime::events::RunEvent;
use serde::{Deserialize, Serialize};

pub use mosaic_inspect::IngressTrace;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HealthResponse {
    pub status: String,
    pub active_profile: String,
    pub session_count: usize,
    pub transport: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RunSubmission {
    pub system: Option<String>,
    pub input: String,
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
pub struct InboundMessage {
    pub session_id: Option<String>,
    pub input: String,
    pub profile: Option<String>,
    pub display_name: Option<String>,
    pub ingress: Option<IngressTrace>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum GatewayEvent {
    RunSubmitted {
        input: String,
        profile: String,
        ingress: Option<IngressTrace>,
    },
    Runtime(RunEvent),
    SessionUpdated {
        summary: SessionSummaryDto,
    },
    RunCompleted {
        output_preview: String,
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
    pub last_gateway_run_id: Option<String>,
    pub last_correlation_id: Option<String>,
    pub message_count: usize,
    pub last_message_preview: Option<String>,
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
    pub gateway: SessionGatewayDto,
    pub transcript: Vec<TranscriptMessageDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SessionGatewayDto {
    pub route: String,
    pub last_gateway_run_id: Option<String>,
    pub last_correlation_id: Option<String>,
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
            skill: None,
            workflow: None,
            session_id: Some("demo".to_owned()),
            profile: Some("mock".to_owned()),
            ingress: Some(IngressTrace {
                kind: "remote_operator".to_owned(),
                channel: Some("cli".to_owned()),
                source: Some("mosaic-cli".to_owned()),
                remote_addr: None,
                display_name: None,
                gateway_url: Some("http://127.0.0.1:8080".to_owned()),
            }),
        };

        let encoded = serde_json::to_vec(&submission).expect("submission should serialize");
        let decoded: RunSubmission =
            serde_json::from_slice(&encoded).expect("submission should deserialize");

        assert_eq!(decoded, submission);
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
                profile: "mock".to_owned(),
                ingress: Some(IngressTrace {
                    kind: "webchat".to_owned(),
                    channel: Some("webchat".to_owned()),
                    source: Some("browser".to_owned()),
                    remote_addr: None,
                    display_name: Some("guest".to_owned()),
                    gateway_url: None,
                }),
            },
        };

        let encoded = serde_json::to_vec(&envelope).expect("envelope should serialize");
        let decoded: EventStreamEnvelope =
            serde_json::from_slice(&encoded).expect("envelope should deserialize");

        assert_eq!(decoded, envelope);
    }
}
