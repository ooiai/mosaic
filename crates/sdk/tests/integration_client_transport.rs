use std::convert::Infallible;

use axum::{
    Json, Router,
    response::sse::{Event, KeepAlive, Sse},
    routing::{get, post},
};
use chrono::Utc;
use futures::stream;
use mosaic_control_protocol::{
    CapabilityInventorySummaryDto, EventStreamEnvelope, GatewayEvent, HealthResponse,
    ReloadBoundaryDto, RunResponse, RunSubmission,
};
use mosaic_inspect::RunTrace;
use mosaic_sdk::GatewayClient;
use tokio::net::TcpListener;

#[tokio::test]
async fn gateway_client_consumes_health_and_event_stream_from_real_http_transport() {
    let app = Router::new()
        .route(
            "/health",
            get(|| async {
                Json(HealthResponse {
                    status: "ok".to_owned(),
                    active_profile: "demo-provider".to_owned(),
                    session_count: 0,
                    transport: "http".to_owned(),
                    deployment_profile: "test".to_owned(),
                    auth_mode: "disabled".to_owned(),
                    event_replay_window: 32,
                    capability_inventory: CapabilityInventorySummaryDto::default(),
                    reload_boundaries: ReloadBoundaryDto::default(),
                })
            }),
        )
        .route(
            "/runs",
            post(|Json(submission): Json<RunSubmission>| async move {
                Json(RunResponse {
                    gateway_run_id: "gw-1".to_owned(),
                    correlation_id: "corr-1".to_owned(),
                    session_route: submission
                        .session_id
                        .as_deref()
                        .map(|id| format!("gateway.local/{id}"))
                        .unwrap_or_else(|| "gateway.local/ephemeral".to_owned()),
                    output: "assistant response".to_owned(),
                    trace: RunTrace::new(submission.input),
                    session_summary: None,
                })
            }),
        )
        .route(
            "/events",
            get(|| async move {
                let envelope = EventStreamEnvelope {
                    gateway_run_id: "gw-1".to_owned(),
                    correlation_id: "corr-1".to_owned(),
                    session_id: Some("demo".to_owned()),
                    session_route: "gateway.local/demo".to_owned(),
                    emitted_at: Utc::now(),
                    event: GatewayEvent::RunCompleted {
                        output_preview: "assistant response".to_owned(),
                    },
                };
                Sse::new(stream::iter(vec![Ok::<_, Infallible>(
                    Event::default().data(serde_json::to_string(&envelope).expect("event json")),
                )]))
                .keep_alive(KeepAlive::default())
            }),
        );

    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("listener should bind");
    let addr = listener.local_addr().expect("listener addr should exist");
    let server = tokio::spawn(async move {
        axum::serve(listener, app).await.expect("server should run");
    });

    let client = GatewayClient::new(format!("http://{}", addr));
    let health = client.health().await.expect("health should succeed");
    assert_eq!(health.status, "ok");

    let run = client
        .submit_run(RunSubmission {
            system: None,
            input: "hello".to_owned(),
            tool: None,
            skill: None,
            workflow: None,
            session_id: Some("demo".to_owned()),
            profile: Some("demo-provider".to_owned()),
            ingress: None,
        })
        .await
        .expect("run submission should succeed");
    assert_eq!(run.gateway_run_id, "gw-1");

    let mut events = client
        .subscribe_events()
        .await
        .expect("event stream should connect");
    let envelope = events
        .next_event()
        .await
        .expect("next event should decode")
        .expect("event should exist");
    assert_eq!(envelope.gateway_run_id, "gw-1");

    server.abort();
}
