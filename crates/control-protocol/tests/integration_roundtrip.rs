use chrono::Utc;
use mosaic_control_protocol::{EventStreamEnvelope, GatewayEvent, RunSubmission};

#[test]
fn serializes_run_submission_and_event_envelope_across_public_boundary() {
    let submission = RunSubmission {
        input: "hello".to_owned(),
        system: Some("system".to_owned()),
        session_id: Some("demo".to_owned()),
        profile: Some("demo-provider".to_owned()),
        skill: None,
        workflow: None,
        ingress: None,
    };
    let envelope = EventStreamEnvelope {
        gateway_run_id: "gw-1".to_owned(),
        correlation_id: "corr-1".to_owned(),
        session_id: Some("demo".to_owned()),
        session_route: "gateway.local/demo".to_owned(),
        emitted_at: Utc::now(),
        event: GatewayEvent::RunSubmitted {
            input: submission.input.clone(),
            profile: "demo-provider".to_owned(),
            ingress: None,
        },
    };

    let submission_json =
        serde_json::to_string(&submission).expect("submission should serialize to json");
    let envelope_json =
        serde_json::to_string(&envelope).expect("envelope should serialize to json");

    let submission_back: RunSubmission =
        serde_json::from_str(&submission_json).expect("submission should deserialize");
    let envelope_back: EventStreamEnvelope =
        serde_json::from_str(&envelope_json).expect("envelope should deserialize");

    assert_eq!(submission_back.session_id.as_deref(), Some("demo"));
    assert_eq!(submission_back.profile.as_deref(), Some("demo-provider"));
    assert_eq!(envelope_back.gateway_run_id, "gw-1");
    assert_eq!(envelope_back.session_route, "gateway.local/demo");
}
