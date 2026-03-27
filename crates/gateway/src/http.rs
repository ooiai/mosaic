use super::*;

#[derive(Clone)]
struct GatewayHttpState {
    gateway: GatewayHandle,
}

type HttpResult<T> = std::result::Result<Json<T>, (StatusCode, Json<ErrorResponse>)>;

pub fn http_router(gateway: GatewayHandle) -> Router {
    Router::new()
        .route("/health", get(http_health))
        .route("/ready", get(http_ready))
        .route("/metrics", get(http_metrics))
        .route("/adapters", get(http_list_adapters))
        .route("/sessions", get(http_list_sessions))
        .route("/sessions/{id}", get(http_get_session))
        .route("/capabilities/jobs", get(http_list_capability_jobs))
        .route("/capabilities/exec", post(http_exec_capability))
        .route("/capabilities/webhook", post(http_webhook_capability))
        .route("/runs", get(http_list_runs).post(http_submit_run))
        .route("/runs/{id}", get(http_get_run))
        .route("/runs/{id}/cancel", post(http_cancel_run))
        .route("/runs/{id}/retry", post(http_retry_run))
        .route("/cron", get(http_list_cron).post(http_register_cron))
        .route("/cron/{id}/trigger", post(http_trigger_cron))
        .route("/audit/events", get(http_audit_events))
        .route("/incidents/{id}", get(http_incident_bundle))
        .route("/ingress/webchat", post(http_webchat_ingress))
        .route("/ingress/telegram", post(http_telegram_ingress))
        .route("/events", get(http_events))
        .route("/events/recent", get(http_recent_events))
        .with_state(GatewayHttpState { gateway })
}

pub async fn serve_http(gateway: GatewayHandle, addr: SocketAddr) -> Result<()> {
    serve_http_with_shutdown(gateway, addr, std::future::pending::<()>()).await
}

pub async fn serve_http_with_shutdown<F>(
    gateway: GatewayHandle,
    addr: SocketAddr,
    shutdown: F,
) -> Result<()>
where
    F: std::future::Future<Output = ()> + Send + 'static,
{
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, http_router(gateway))
        .with_graceful_shutdown(shutdown)
        .await?;
    Ok(())
}

async fn http_health(State(state): State<GatewayHttpState>) -> Json<HealthResponse> {
    Json(state.gateway.health())
}

async fn http_ready(State(state): State<GatewayHttpState>) -> HttpResult<ReadinessResponse> {
    let components = state.gateway.snapshot_components();
    if !components.observability.enable_readiness {
        return Err((
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ErrorResponse {
                error: "readiness endpoint is disabled by config".to_owned(),
            }),
        ));
    }
    Ok(Json(state.gateway.readiness()))
}

async fn http_metrics(
    State(state): State<GatewayHttpState>,
    headers: HeaderMap,
) -> HttpResult<MetricsResponse> {
    authorize_control_request(&state.gateway, &headers, "/metrics")?;
    let components = state.gateway.snapshot_components();
    if !components.observability.enable_metrics {
        return Err((
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ErrorResponse {
                error: "metrics endpoint is disabled by config".to_owned(),
            }),
        ));
    }
    Ok(Json(state.gateway.metrics()))
}

async fn http_list_adapters(
    State(state): State<GatewayHttpState>,
    headers: HeaderMap,
) -> HttpResult<Vec<AdapterStatusDto>> {
    authorize_control_request(&state.gateway, &headers, "/adapters")?;
    Ok(Json(state.gateway.list_adapter_statuses()))
}

async fn http_list_sessions(
    State(state): State<GatewayHttpState>,
    headers: HeaderMap,
) -> HttpResult<Vec<SessionSummaryDto>> {
    authorize_control_request(&state.gateway, &headers, "/sessions")?;
    let sessions = state
        .gateway
        .list_sessions()
        .map_err(http_internal_error)?
        .iter()
        .map(session_summary_dto)
        .collect();
    Ok(Json(sessions))
}

async fn http_list_runs(
    State(state): State<GatewayHttpState>,
    headers: HeaderMap,
) -> HttpResult<Vec<RunSummaryDto>> {
    authorize_control_request(&state.gateway, &headers, "/runs")?;
    Ok(Json(
        state.gateway.list_runs().map_err(http_internal_error)?,
    ))
}

async fn http_get_run(
    AxumPath(id): AxumPath<String>,
    State(state): State<GatewayHttpState>,
    headers: HeaderMap,
) -> HttpResult<RunDetailDto> {
    authorize_control_request(&state.gateway, &headers, "/runs/{id}")?;
    match state.gateway.load_run(&id).map_err(http_internal_error)? {
        Some(run) => Ok(Json(run)),
        None => Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: format!("run not found: {id}"),
            }),
        )),
    }
}

async fn http_cancel_run(
    AxumPath(id): AxumPath<String>,
    State(state): State<GatewayHttpState>,
    headers: HeaderMap,
) -> HttpResult<RunDetailDto> {
    authorize_control_request(&state.gateway, &headers, "/runs/{id}/cancel")?;
    Ok(Json(
        state.gateway.cancel_run(&id).map_err(http_internal_error)?,
    ))
}

async fn http_retry_run(
    AxumPath(id): AxumPath<String>,
    State(state): State<GatewayHttpState>,
    headers: HeaderMap,
) -> HttpResult<RunResponse> {
    authorize_control_request(&state.gateway, &headers, "/runs/{id}/retry")?;
    let submitted = state.gateway.retry_run(&id).map_err(http_internal_error)?;
    let result = submitted.wait().await.map_err(http_run_error)?;
    Ok(Json(run_response(result)))
}

async fn http_list_capability_jobs(
    State(state): State<GatewayHttpState>,
    headers: HeaderMap,
) -> HttpResult<Vec<CapabilityJobDto>> {
    authorize_control_request(&state.gateway, &headers, "/capabilities/jobs")?;
    Ok(Json(state.gateway.list_capability_jobs()))
}

async fn http_exec_capability(
    State(state): State<GatewayHttpState>,
    headers: HeaderMap,
    Json(request): Json<ExecJobRequest>,
) -> HttpResult<CapabilityJobDto> {
    authorize_control_request(&state.gateway, &headers, "/capabilities/exec")?;
    let job = state
        .gateway
        .run_exec_job(request)
        .await
        .map_err(http_internal_error)?;
    Ok(Json(job))
}

async fn http_webhook_capability(
    State(state): State<GatewayHttpState>,
    headers: HeaderMap,
    Json(request): Json<WebhookJobRequest>,
) -> HttpResult<CapabilityJobDto> {
    authorize_control_request(&state.gateway, &headers, "/capabilities/webhook")?;
    let job = state
        .gateway
        .run_webhook_job(request)
        .await
        .map_err(http_internal_error)?;
    Ok(Json(job))
}

async fn http_list_cron(
    State(state): State<GatewayHttpState>,
    headers: HeaderMap,
) -> HttpResult<Vec<CronRegistrationDto>> {
    authorize_control_request(&state.gateway, &headers, "/cron")?;
    let registrations = state
        .gateway
        .list_cron_registrations()
        .map_err(http_internal_error)?
        .iter()
        .map(cron_registration_dto)
        .collect();
    Ok(Json(registrations))
}

async fn http_register_cron(
    State(state): State<GatewayHttpState>,
    headers: HeaderMap,
    Json(request): Json<CronRegistrationRequest>,
) -> HttpResult<CronRegistrationDto> {
    authorize_control_request(&state.gateway, &headers, "/cron")?;
    let registration = state
        .gateway
        .register_cron(request)
        .map_err(http_internal_error)?;
    Ok(Json(cron_registration_dto(&registration)))
}

async fn http_trigger_cron(
    AxumPath(id): AxumPath<String>,
    State(state): State<GatewayHttpState>,
    headers: HeaderMap,
) -> HttpResult<RunResponse> {
    authorize_control_request(&state.gateway, &headers, "/cron/trigger")?;
    let result = state
        .gateway
        .trigger_cron(&id)
        .await
        .map_err(http_internal_error)?;
    Ok(Json(run_response(result)))
}

async fn http_get_session(
    AxumPath(id): AxumPath<String>,
    State(state): State<GatewayHttpState>,
    headers: HeaderMap,
) -> HttpResult<SessionDetailDto> {
    authorize_control_request(&state.gateway, &headers, "/sessions/{id}")?;
    let session = state
        .gateway
        .load_session(&id)
        .map_err(http_internal_error)?;
    match session {
        Some(session) => Ok(Json(session_detail_dto(&session))),
        None => Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: format!("session not found: {id}"),
            }),
        )),
    }
}

async fn http_submit_run(
    State(state): State<GatewayHttpState>,
    headers: HeaderMap,
    Json(mut request): Json<RunSubmission>,
) -> HttpResult<RunResponse> {
    authorize_control_request(&state.gateway, &headers, "/runs")?;
    if request.ingress.is_none() {
        request.ingress = Some(IngressTrace {
            kind: "remote_operator".to_owned(),
            channel: Some("api".to_owned()),
            source: Some("mosaic-sdk".to_owned()),
            remote_addr: None,
            display_name: None,
            actor_id: None,
            thread_id: None,
            thread_title: None,
            reply_target: None,
            gateway_url: None,
        });
    }

    let submitted = state
        .gateway
        .submit_run(request)
        .map_err(http_internal_error)?;
    let result = submitted.wait().await.map_err(http_run_error)?;
    Ok(Json(run_response(result)))
}

async fn http_webchat_ingress(
    State(state): State<GatewayHttpState>,
    headers: HeaderMap,
    Json(message): Json<InboundMessage>,
) -> HttpResult<RunResponse> {
    let auth = state.gateway.snapshot_components().auth;
    authorize_shared_secret_request(
        &state.gateway,
        &headers,
        auth.webchat_shared_secret_env.as_deref(),
        "x-mosaic-shared-secret",
        "webchat ingress",
    )?;
    let submitted = state
        .gateway
        .submit_webchat_message(message)
        .map_err(http_internal_error)?;
    let result = submitted.wait().await.map_err(http_run_error)?;
    Ok(Json(run_response(result)))
}

async fn http_telegram_ingress(
    State(state): State<GatewayHttpState>,
    headers: HeaderMap,
    Json(update): Json<TelegramUpdate>,
) -> HttpResult<RunResponse> {
    let auth = state.gateway.snapshot_components().auth;
    authorize_shared_secret_request(
        &state.gateway,
        &headers,
        auth.telegram_secret_token_env.as_deref(),
        "x-telegram-bot-api-secret-token",
        "telegram ingress",
    )?;
    let submitted = state
        .gateway
        .submit_telegram_update(update)
        .map_err(http_internal_error)?;
    let result = submitted.wait().await.map_err(http_run_error)?;
    Ok(Json(run_response(result)))
}

async fn http_audit_events(
    State(state): State<GatewayHttpState>,
    headers: HeaderMap,
    Query(query): Query<LimitQuery>,
) -> HttpResult<Vec<GatewayAuditEventDto>> {
    authorize_control_request(&state.gateway, &headers, "/audit/events")?;
    Ok(Json(state.gateway.audit_events(
        query.limit.unwrap_or(DEFAULT_AUDIT_QUERY_LIMIT),
    )))
}

async fn http_incident_bundle(
    AxumPath(id): AxumPath<String>,
    State(state): State<GatewayHttpState>,
    headers: HeaderMap,
) -> HttpResult<IncidentBundleDto> {
    authorize_control_request(&state.gateway, &headers, "/incidents/{id}")?;
    let (bundle, _) = state
        .gateway
        .incident_bundle(&id)
        .map_err(http_internal_error)?;
    Ok(Json(bundle))
}

async fn http_events(
    State(state): State<GatewayHttpState>,
    headers: HeaderMap,
) -> std::result::Result<
    Sse<impl futures::Stream<Item = std::result::Result<SseEvent, Infallible>>>,
    (StatusCode, Json<ErrorResponse>),
> {
    authorize_control_request(&state.gateway, &headers, "/events")?;
    let receiver = state.gateway.subscribe();
    let gateway = state.gateway.clone();
    let stream = stream::unfold(receiver, move |mut receiver| {
        let gateway = gateway.clone();
        async move {
            match receiver.recv().await {
                Ok(envelope) => {
                    let payload = serde_json::to_string(&envelope)
                        .unwrap_or_else(|_| "{\"error\":\"failed to encode event\"}".to_owned());
                    Some((Ok(SseEvent::default().data(payload)), receiver))
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(skipped)) => {
                    increment_metric(gateway.inner.as_ref(), |metrics| {
                        metrics.broadcast_lag_events_total += skipped as u64;
                    });
                    Some((
                        Ok(SseEvent::default()
                            .event("lagged")
                            .data(skipped.to_string())),
                        receiver,
                    ))
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => None,
            }
        }
    });

    Ok(Sse::new(stream).keep_alive(KeepAlive::default()))
}

async fn http_recent_events(
    State(state): State<GatewayHttpState>,
    headers: HeaderMap,
    Query(query): Query<LimitQuery>,
) -> HttpResult<ReplayWindowResponse> {
    authorize_control_request(&state.gateway, &headers, "/events/recent")?;
    Ok(Json(state.gateway.replay_window(
        query.limit.unwrap_or(DEFAULT_REPLAY_QUERY_LIMIT),
    )))
}

fn http_internal_error(error: anyhow::Error) -> (StatusCode, Json<ErrorResponse>) {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(ErrorResponse {
            error: public_error_message(&error),
        }),
    )
}

fn http_run_error(error: GatewayRunError) -> (StatusCode, Json<ErrorResponse>) {
    let message = public_error_message(&error.into_parts().0);
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(ErrorResponse { error: message }),
    )
}
