use std::{
    fs,
    path::PathBuf,
    process,
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use chrono::Utc;
use mosaic_inspect::{
    AttachmentFailureTrace, AttachmentKind, AttachmentRouteMode, AttachmentRouteTrace,
    ChannelAttachment, ChannelDeliveryResult, ChannelDeliveryStatus, ChannelDeliveryTrace,
    ChannelOutboundMessage, IngressTrace, RouteDecisionTrace, RouteMode, RunTrace, SkillTrace,
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
        bot_name: None,
        bot_route: None,
        bot_profile: None,
        bot_token_env: None,
        bot_secret_env: None,
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
        attachments: Vec::new(),
        attachment_failures: Vec::new(),
        gateway_url: Some("http://127.0.0.1:8080".to_owned()),
    });
    trace.add_outbound_delivery(ChannelDeliveryTrace {
        message: ChannelOutboundMessage {
            channel: "telegram".to_owned(),
            adapter: "telegram_webhook".to_owned(),
            bot_name: None,
            bot_route: None,
            bot_profile: None,
            bot_token_env: None,
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
        bot_name: None,
        bot_route: None,
        bot_profile: None,
        bot_token_env: None,
        bot_secret_env: None,
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
        attachments: Vec::new(),
        attachment_failures: Vec::new(),
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
        selected_category: None,
        catalog_scope: None,
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

#[test]
fn persists_and_recovers_attachment_route_metadata() {
    let dir = temp_dir("attachment-route");
    let mut trace = RunTrace::new("look at this attachment".to_owned());
    trace.bind_session("demo");
    trace.bind_ingress(IngressTrace {
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
        conversation_id: Some("telegram:chat:42".to_owned()),
        thread_id: None,
        thread_title: None,
        reply_target: Some("telegram:chat:42:message:88".to_owned()),
        message_id: Some("88".to_owned()),
        received_at: None,
        raw_event_id: Some("event-attachment".to_owned()),
        session_hint: Some("demo".to_owned()),
        profile_hint: Some("mock".to_owned()),
        control_command: None,
        original_text: Some("look at this attachment".to_owned()),
        attachments: vec![ChannelAttachment {
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
        attachment_failures: vec![AttachmentFailureTrace {
            attachment_id: "img-2".to_owned(),
            stage: "policy".to_owned(),
            kind: "mime_not_allowed".to_owned(),
            message: "attachment mime_type 'application/zip' is not allowed".to_owned(),
        }],
        gateway_url: None,
    });
    trace.bind_attachment_route(AttachmentRouteTrace {
        mode: AttachmentRouteMode::SpecializedProcessor,
        selection_reason: "attachment route resolved to specialized_processor".to_owned(),
        bot_identity: Some("primary".to_owned()),
        policy_scope: Some("bot:primary".to_owned()),
        selected_profile: Some("vision-docs".to_owned()),
        provider_profile: None,
        provider_model: None,
        processor: Some("attachment_echo".to_owned()),
        allowed_attachment_kinds: vec!["image".to_owned(), "document".to_owned()],
        max_attachment_size_mb: Some(10),
        attachment_count: 1,
        attachment_kinds: vec!["image".to_owned()],
        attachment_filenames: vec!["photo.jpg".to_owned()],
        failure_summary: vec![
            "policy:mime_not_allowed:attachment mime_type 'application/zip' is not allowed"
                .to_owned(),
        ],
    });
    trace.finish_ok("attachment count: 1".to_owned());

    let path = trace.save_to_dir(&dir).expect("trace should save");
    let bytes = fs::read(path).expect("saved trace should be readable");
    let loaded: RunTrace = serde_json::from_slice(&bytes).expect("trace should deserialize");

    assert_eq!(
        loaded.attachment_route.as_ref().map(|route| route.mode),
        Some(AttachmentRouteMode::SpecializedProcessor)
    );
    assert_eq!(
        loaded
            .attachment_route
            .as_ref()
            .and_then(|route| route.processor.as_deref()),
        Some("attachment_echo")
    );
    assert_eq!(
        loaded
            .ingress
            .as_ref()
            .map(|ingress| ingress.attachments.len()),
        Some(1)
    );
    assert_eq!(
        loaded
            .ingress
            .as_ref()
            .map(|ingress| ingress.attachment_failures.len()),
        Some(1)
    );

    fs::remove_dir_all(dir).ok();
}

#[test]
fn persists_and_recovers_skill_trace_source_metadata() {
    let dir = temp_dir("skill-trace");
    let mut trace = RunTrace::new("render operator note".to_owned());
    trace.skill_calls.push(SkillTrace {
        name: "operator_note".to_owned(),
        source_kind: Some("markdown_pack".to_owned()),
        source_path: Some("/tmp/operator-note".to_owned()),
        skill_version: Some("0.1.0".to_owned()),
        runtime_requirements: vec!["python".to_owned()],
        sandbox: None,
        input: serde_json::json!({ "text": "disk usage high" }),
        output: Some("Operator note:\ndisk usage high".to_owned()),
        started_at: Utc::now(),
        finished_at: Some(Utc::now()),
    });
    trace.finish_ok("Operator note:\ndisk usage high".to_owned());

    let path = trace.save_to_dir(&dir).expect("trace should save");
    let bytes = fs::read(path).expect("saved trace should be readable");
    let loaded: RunTrace = serde_json::from_slice(&bytes).expect("trace should deserialize");

    assert_eq!(loaded.skill_calls.len(), 1);
    assert_eq!(
        loaded.skill_calls[0].source_kind.as_deref(),
        Some("markdown_pack")
    );
    assert_eq!(
        loaded.skill_calls[0].skill_version.as_deref(),
        Some("0.1.0")
    );
    assert_eq!(
        loaded.skill_calls[0].runtime_requirements,
        vec!["python".to_owned()]
    );

    fs::remove_dir_all(dir).ok();
}
