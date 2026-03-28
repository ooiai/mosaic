use std::{
    fs,
    path::PathBuf,
    process,
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use chrono::Utc;
use mosaic_inspect::{
    ChannelDeliveryResult, ChannelDeliveryStatus, ChannelDeliveryTrace, ChannelOutboundMessage,
    IngressTrace, RouteDecisionTrace, RouteMode, RunTrace,
};

static COUNTER: AtomicU64 = AtomicU64::new(0);

fn temp_dir(label: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock should be after epoch")
        .as_nanos();
    let count = COUNTER.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "mosaic-inspect-integration-{label}-{}-{nanos}-{count}",
        process::id()
    ))
}

#[test]
fn persists_and_recovers_trace_with_ingress_metadata() {
    let dir = temp_dir("trace");
    let mut trace = RunTrace::new("inspect me".to_owned());
    trace.bind_session("demo");
    trace.bind_ingress(IngressTrace {
        kind: "webchat".to_owned(),
        channel: Some("webchat".to_owned()),
        adapter: Some("webchat_http".to_owned()),
        source: Some("integration-test".to_owned()),
        remote_addr: Some("127.0.0.1".to_owned()),
        display_name: Some("Operator".to_owned()),
        actor_id: Some("42".to_owned()),
        conversation_id: Some("webchat:demo".to_owned()),
        thread_id: None,
        thread_title: None,
        reply_target: Some("webchat:demo".to_owned()),
        message_id: Some("message-1".to_owned()),
        received_at: None,
        raw_event_id: Some("event-1".to_owned()),
        session_hint: Some("demo".to_owned()),
        profile_hint: None,
        control_command: None,
        original_text: None,
        gateway_url: Some("http://127.0.0.1:8080".to_owned()),
    });
    trace.add_outbound_delivery(ChannelDeliveryTrace {
        message: ChannelOutboundMessage {
            channel: "telegram".to_owned(),
            adapter: "telegram_webhook".to_owned(),
            conversation_id: "telegram:chat:42".to_owned(),
            reply_target: "telegram:chat:42:message:10".to_owned(),
            text: "done".to_owned(),
            idempotency_key: "idem-1".to_owned(),
            correlation_id: "corr-1".to_owned(),
            gateway_run_id: "gw-1".to_owned(),
            session_id: "demo".to_owned(),
        },
        result: ChannelDeliveryResult {
            delivery_id: "delivery-1".to_owned(),
            status: ChannelDeliveryStatus::Delivered,
            provider_message_id: Some("88".to_owned()),
            retry_count: 1,
            retryable: false,
            error_kind: None,
            error: None,
            delivered_at: Some(Utc::now()),
        },
    });
    trace.finish_ok("done".to_owned());

    let path = trace.save_to_dir(&dir).expect("trace should save");
    let bytes = fs::read(path).expect("saved trace should be readable");
    let loaded: RunTrace = serde_json::from_slice(&bytes).expect("trace should deserialize");

    assert_eq!(loaded.status(), "success");
    assert_eq!(loaded.session_id.as_deref(), Some("demo"));
    assert_eq!(
        loaded
            .ingress
            .as_ref()
            .and_then(|ingress| ingress.channel.as_deref()),
        Some("webchat")
    );
    assert_eq!(loaded.outbound_deliveries.len(), 1);
    assert_eq!(
        loaded.outbound_deliveries[0]
            .result
            .provider_message_id
            .as_deref(),
        Some("88")
    );

    fs::remove_dir_all(dir).ok();
}

#[test]
fn persists_and_recovers_route_decision_metadata() {
    let dir = temp_dir("route-decision");
    let mut trace = RunTrace::new("summarize this for handoff".to_owned());
    trace.bind_session("demo");
    trace.bind_ingress(IngressTrace {
        kind: "telegram_bot".to_owned(),
        channel: Some("telegram".to_owned()),
        adapter: Some("telegram_bot".to_owned()),
        source: Some("telegram_bot".to_owned()),
        remote_addr: None,
        display_name: Some("Operator".to_owned()),
        actor_id: Some("17".to_owned()),
        conversation_id: Some("telegram:chat:42".to_owned()),
        thread_id: Some("7".to_owned()),
        thread_title: Some("Ops".to_owned()),
        reply_target: Some("telegram:chat:42:message:10".to_owned()),
        message_id: Some("10".to_owned()),
        received_at: None,
        raw_event_id: Some("event-2".to_owned()),
        session_hint: Some("demo".to_owned()),
        profile_hint: Some("demo-provider".to_owned()),
        control_command: Some("skill".to_owned()),
        original_text: Some("/mosaic skill summarize summarize this for handoff".to_owned()),
        gateway_url: None,
    });
    trace.bind_route_decision(RouteDecisionTrace {
        route_mode: RouteMode::Skill,
        selected_capability_type: Some("skill".to_owned()),
        selected_capability_name: Some("summarize".to_owned()),
        selected_tool: None,
        selected_skill: Some("summarize".to_owned()),
        selected_workflow: None,
        selection_reason: "explicit /mosaic skill command".to_owned(),
        capability_source: Some("builtin.core".to_owned()),
        profile_used: Some("demo-provider".to_owned()),
    });
    trace.finish_ok("summary: summarize this for handoff".to_owned());

    let path = trace.save_to_dir(&dir).expect("trace should save");
    let bytes = fs::read(path).expect("saved trace should be readable");
    let loaded: RunTrace = serde_json::from_slice(&bytes).expect("trace should deserialize");

    assert_eq!(
        loaded.route_decision.as_ref().map(|route| route.route_mode),
        Some(RouteMode::Skill)
    );
    assert_eq!(
        loaded
            .route_decision
            .as_ref()
            .and_then(|route| route.selected_skill.as_deref()),
        Some("summarize")
    );
    assert_eq!(
        loaded
            .route_decision
            .as_ref()
            .and_then(|route| route.capability_source.as_deref()),
        Some("builtin.core")
    );
    assert_eq!(
        loaded
            .route_decision
            .as_ref()
            .and_then(|route| route.profile_used.as_deref()),
        Some("demo-provider")
    );
    assert_eq!(
        loaded
            .ingress
            .as_ref()
            .and_then(|ingress| ingress.control_command.as_deref()),
        Some("skill")
    );
    assert_eq!(
        loaded
            .ingress
            .as_ref()
            .and_then(|ingress| ingress.original_text.as_deref()),
        Some("/mosaic skill summarize summarize this for handoff")
    );

    fs::remove_dir_all(dir).ok();
}
