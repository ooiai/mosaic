use super::*;

pub(crate) fn configured_secret_env_name(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}

pub(crate) fn secret_status(env_name: Option<&str>) -> SecretStatus {
    match configured_secret_env_name(env_name) {
        Some(name) => match env::var(name) {
            Ok(value) => SecretStatus::Ready(value),
            Err(_) => SecretStatus::MissingEnv(name.to_owned()),
        },
        None => SecretStatus::Disabled,
    }
}

pub(crate) fn operator_auth_mode(auth: &AuthConfig) -> String {
    match secret_status(auth.operator_token_env.as_deref()) {
        SecretStatus::Disabled => "disabled".to_owned(),
        SecretStatus::Ready(_) | SecretStatus::MissingEnv(_) => "required".to_owned(),
    }
}

pub(crate) fn auth_state_ready(deployment: &DeploymentConfig, auth: &AuthConfig) -> bool {
    match secret_status(auth.operator_token_env.as_deref()) {
        SecretStatus::Disabled => deployment.profile != "production",
        SecretStatus::Ready(_) => true,
        SecretStatus::MissingEnv(_) => false,
    }
}

pub(crate) fn header_token<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    headers.get(name).and_then(|value| value.to_str().ok())
}

pub(crate) fn bearer_token(headers: &HeaderMap) -> Option<&str> {
    header_token(headers, "authorization")?.strip_prefix("Bearer ")
}

pub(crate) fn ingress_actor(ingress: Option<&IngressTrace>) -> Option<String> {
    ingress.and_then(|ingress| {
        ingress
            .display_name
            .clone()
            .or_else(|| ingress.actor_id.clone())
            .or_else(|| ingress.source.clone())
    })
}

pub(crate) fn redact_audit_input(input: &str, redact: bool) -> String {
    if redact {
        "<redacted>".to_owned()
    } else {
        truncate_preview(input, 160)
    }
}

pub(crate) fn record_audit_event(
    state: &GatewayState,
    kind: &str,
    outcome: &str,
    summary: String,
    session_id: Option<String>,
    gateway_run_id: Option<String>,
    correlation_id: Option<String>,
    ingress: Option<&IngressTrace>,
    target: Option<String>,
    redacted: bool,
) {
    let event = GatewayAuditEventDto {
        id: Uuid::new_v4().to_string(),
        kind: kind.to_owned(),
        outcome: outcome.to_owned(),
        summary,
        actor: ingress_actor(ingress),
        session_id,
        gateway_run_id,
        correlation_id,
        channel: ingress.and_then(|ingress| ingress.channel.clone()),
        target,
        emitted_at: Utc::now(),
        redacted,
    };
    state.audit_log.append(event);
    increment_metric(state, |metrics| metrics.audit_events_total += 1);
}

pub(crate) fn maybe_record_capability_job_audit(
    state: &GatewayState,
    job: &CapabilityJobDto,
    ingress: Option<&IngressTrace>,
) {
    if matches!(job.status.as_str(), "queued" | "success" | "failed") {
        record_audit_event(
            state,
            "capability.job",
            &job.status,
            job.summary.clone().unwrap_or_else(|| job.name.clone()),
            job.session_id.clone(),
            job.gateway_run_id.clone(),
            job.correlation_id.clone(),
            ingress,
            job.target.clone(),
            false,
        );
    }
}

pub(crate) fn record_auth_denial(gateway: &GatewayHandle, surface: &str) {
    increment_metric(gateway.inner.as_ref(), |metrics| {
        metrics.auth_denials_total += 1
    });
    gateway.record_audit_event(
        "auth.denied",
        "denied",
        format!("authorization denied for {surface}"),
        None,
        None,
        None,
        None,
        Some(surface.to_owned()),
        false,
    );
}

pub(crate) fn authorize_control_request(
    gateway: &GatewayHandle,
    headers: &HeaderMap,
    surface: &str,
) -> Result<(), (StatusCode, Json<ErrorResponse>)> {
    let components = gateway.snapshot_components();
    match secret_status(components.auth.operator_token_env.as_deref()) {
        SecretStatus::Disabled => Ok(()),
        SecretStatus::Ready(secret) => {
            if bearer_token(headers) == Some(secret.as_str()) {
                Ok(())
            } else {
                record_auth_denial(gateway, surface);
                Err((
                    StatusCode::UNAUTHORIZED,
                    Json(ErrorResponse {
                        error: "operator authorization required".to_owned(),
                    }),
                ))
            }
        }
        SecretStatus::MissingEnv(name) => Err((
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ErrorResponse {
                error: format!("configured operator token env is missing: {name}"),
            }),
        )),
    }
}

pub(crate) fn authorize_shared_secret_request(
    gateway: &GatewayHandle,
    headers: &HeaderMap,
    env_name: Option<&str>,
    header_name: &str,
    surface: &str,
) -> Result<(), (StatusCode, Json<ErrorResponse>)> {
    match secret_status(env_name) {
        SecretStatus::Disabled => Ok(()),
        SecretStatus::Ready(secret) => {
            if header_token(headers, header_name) == Some(secret.as_str()) {
                Ok(())
            } else {
                record_auth_denial(gateway, surface);
                Err((
                    StatusCode::UNAUTHORIZED,
                    Json(ErrorResponse {
                        error: format!("{surface} shared secret required"),
                    }),
                ))
            }
        }
        SecretStatus::MissingEnv(name) => Err((
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ErrorResponse {
                error: format!("configured shared secret env is missing: {name}"),
            }),
        )),
    }
}

pub(crate) fn matches_identifier(left: Option<&str>, right: Option<&str>) -> bool {
    matches!((left, right), (Some(left), Some(right)) if left == right)
}

pub(crate) fn load_incident_trace(runs_dir: &FsPath, identifier: &str) -> Result<RunTrace> {
    for entry in fs::read_dir(runs_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
            continue;
        }
        let content = fs::read_to_string(&path)?;
        let trace: RunTrace = serde_json::from_str(&content)?;
        if trace.run_id == identifier
            || trace.gateway_run_id.as_deref() == Some(identifier)
            || trace.correlation_id.as_deref() == Some(identifier)
        {
            return Ok(trace);
        }
    }

    bail!("incident trace not found: {}", identifier)
}

pub(crate) fn webchat_adapter_status(auth: &AuthConfig) -> AdapterStatusDto {
    let shared_secret = secret_status(auth.webchat_shared_secret_env.as_deref());
    let (status, detail) = match shared_secret {
        SecretStatus::Disabled => (
            "ok",
            "WebChat ingress is ready without an additional shared secret.",
        ),
        SecretStatus::Ready(_) => (
            "ok",
            "WebChat ingress is ready and protected by X-Mosaic-Shared-Secret.",
        ),
        SecretStatus::MissingEnv(_) => (
            "error",
            "WebChat ingress secret is configured but the environment variable is missing.",
        ),
    };
    AdapterStatusDto {
        name: "webchat".to_owned(),
        channel: "webchat".to_owned(),
        transport: "http".to_owned(),
        ingress_path: "/ingress/webchat".to_owned(),
        outbound_ready: true,
        status: status.to_owned(),
        detail: detail.to_owned(),
    }
}

pub(crate) fn telegram_adapter_status(auth: &AuthConfig) -> AdapterStatusDto {
    let outbound_ready =
        env::var("MOSAIC_TELEGRAM_BOT_TOKEN").is_ok() || env::var("TELEGRAM_BOT_TOKEN").is_ok();
    let shared_secret = secret_status(auth.telegram_secret_token_env.as_deref());
    let (status, detail) = match (outbound_ready, shared_secret) {
        (_, SecretStatus::MissingEnv(_)) => (
            "error",
            "Telegram webhook secret is configured but the environment variable is missing.",
        ),
        (true, SecretStatus::Ready(_)) => (
            "ok",
            "Telegram webhook ingress and replies are ready with secret-token verification.",
        ),
        (true, SecretStatus::Disabled) => ("ok", "Telegram webhook ingress and replies are ready."),
        (false, SecretStatus::Ready(_)) => (
            "warning",
            "Telegram ingress is protected, but outbound replies still need TELEGRAM_BOT_TOKEN or MOSAIC_TELEGRAM_BOT_TOKEN.",
        ),
        (false, SecretStatus::Disabled) => (
            "warning",
            "Telegram ingress is ready, but outbound replies need TELEGRAM_BOT_TOKEN or MOSAIC_TELEGRAM_BOT_TOKEN.",
        ),
    };
    AdapterStatusDto {
        name: "telegram".to_owned(),
        channel: "telegram".to_owned(),
        transport: "http-webhook".to_owned(),
        ingress_path: "/ingress/telegram".to_owned(),
        outbound_ready,
        status: status.to_owned(),
        detail: detail.to_owned(),
    }
}

pub(crate) fn ingress_route(
    session_id: Option<&str>,
    ingress: Option<&IngressTrace>,
) -> Option<String> {
    let ingress = ingress?;
    let channel = ingress.channel.as_deref().unwrap_or(ingress.kind.as_str());
    let target = ingress
        .reply_target
        .as_deref()
        .or(ingress.actor_id.as_deref())
        .or(session_id)?;
    let mut route = format!(
        "gateway.channel/{}/{}",
        route_segment(channel),
        route_segment(target)
    );
    if let Some(thread_id) = ingress.thread_id.as_deref() {
        route.push_str("/thread/");
        route.push_str(&route_segment(thread_id));
    }
    Some(route)
}

pub(crate) fn route_segment(value: &str) -> String {
    let mut normalized = String::with_capacity(value.len());
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.') {
            normalized.push(ch);
        } else {
            normalized.push('-');
        }
    }
    normalized.trim_matches('-').to_owned()
}

pub(crate) fn truncate_preview(value: &str, limit: usize) -> String {
    if value.chars().count() <= limit {
        return value.to_owned();
    }

    let truncated: String = value.chars().take(limit).collect();
    format!("{truncated}...")
}
