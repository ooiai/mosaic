use super::*;
use mosaic_config::AttachmentRoutingTargetConfig;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ResolvedTelegramBot {
    pub(crate) name: String,
    pub(crate) enabled: bool,
    pub(crate) route_key: String,
    pub(crate) webhook_path: String,
    pub(crate) bot_token_env: String,
    pub(crate) webhook_secret_token_env: Option<String>,
    pub(crate) default_profile: Option<String>,
    pub(crate) allowed_tools: Vec<String>,
    pub(crate) allowed_skills: Vec<String>,
    pub(crate) allowed_workflows: Vec<String>,
    pub(crate) attachments: Option<AttachmentRoutingTargetConfig>,
    pub(crate) legacy: bool,
}

impl ResolvedTelegramBot {
    pub(crate) fn adapter_name(&self) -> String {
        if self.legacy {
            "telegram".to_owned()
        } else {
            format!("telegram.{}", self.name)
        }
    }

    pub(crate) fn allows_tool(&self, name: &str) -> bool {
        self.allowed_tools.is_empty() || self.allowed_tools.iter().any(|value| value == name)
    }

    pub(crate) fn allows_skill(&self, name: &str) -> bool {
        self.allowed_skills.is_empty() || self.allowed_skills.iter().any(|value| value == name)
    }

    pub(crate) fn allows_workflow(&self, name: &str) -> bool {
        self.allowed_workflows.is_empty()
            || self.allowed_workflows.iter().any(|value| value == name)
    }

    pub(crate) fn context(&self) -> mosaic_channel_telegram::TelegramBotContext {
        mosaic_channel_telegram::TelegramBotContext {
            name: Some(self.name.clone()),
            route: Some(self.route_key.clone()),
            default_profile: self.default_profile.clone(),
            bot_token_env: Some(self.bot_token_env.clone()),
            bot_secret_env: self.webhook_secret_token_env.clone(),
        }
    }
}

pub(crate) fn resolved_telegram_bots(
    components: &GatewayRuntimeComponents,
) -> Vec<ResolvedTelegramBot> {
    if components.telegram.bots.is_empty() {
        return vec![ResolvedTelegramBot {
            name: "default".to_owned(),
            enabled: true,
            route_key: "default".to_owned(),
            webhook_path: "/ingress/telegram".to_owned(),
            bot_token_env: "MOSAIC_TELEGRAM_BOT_TOKEN".to_owned(),
            webhook_secret_token_env: components.auth.telegram_secret_token_env.clone(),
            default_profile: None,
            allowed_tools: Vec::new(),
            allowed_skills: Vec::new(),
            allowed_workflows: Vec::new(),
            attachments: None,
            legacy: true,
        }];
    }

    let mut bots = components
        .telegram
        .bots
        .iter()
        .map(|(name, bot)| ResolvedTelegramBot {
            name: name.clone(),
            enabled: bot.enabled,
            route_key: bot.route_key(name),
            webhook_path: bot.webhook_path(name),
            bot_token_env: bot.bot_token_env.clone(),
            webhook_secret_token_env: bot.webhook_secret_token_env.clone(),
            default_profile: bot.default_profile.clone(),
            allowed_tools: bot.allowed_tools.clone(),
            allowed_skills: bot.allowed_skills.clone(),
            allowed_workflows: bot.allowed_workflows.clone(),
            attachments: bot.attachments.clone(),
            legacy: false,
        })
        .collect::<Vec<_>>();
    bots.sort_by(|left, right| left.name.cmp(&right.name));
    bots
}

pub(crate) fn resolved_telegram_bot_by_name(
    components: &GatewayRuntimeComponents,
    name: Option<&str>,
) -> Option<ResolvedTelegramBot> {
    let enabled = resolved_telegram_bots(components)
        .into_iter()
        .filter(|bot| bot.enabled)
        .collect::<Vec<_>>();
    match name {
        Some(name) => enabled.into_iter().find(|bot| bot.name == name),
        None if enabled.len() == 1 => enabled.into_iter().next(),
        None => None,
    }
}

pub(crate) fn telegram_outbound_client_for_bot(
    bot: &ResolvedTelegramBot,
) -> Result<Option<mosaic_channel_telegram::TelegramOutboundClient>> {
    telegram_outbound_client_for_bot_with_settings(bot, Duration::from_secs(15), 2)
}

pub(crate) fn telegram_outbound_client_for_bot_with_settings(
    bot: &ResolvedTelegramBot,
    timeout: Duration,
    max_retries: usize,
) -> Result<Option<mosaic_channel_telegram::TelegramOutboundClient>> {
    let bot_token = if bot.legacy {
        env::var("MOSAIC_TELEGRAM_BOT_TOKEN")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .or_else(|| {
                env::var("TELEGRAM_BOT_TOKEN")
                    .ok()
                    .filter(|value| !value.trim().is_empty())
            })
    } else {
        env::var(&bot.bot_token_env)
            .ok()
            .filter(|value| !value.trim().is_empty())
    };
    let Some(bot_token) = bot_token else {
        return Ok(None);
    };

    let base_url = env::var("MOSAIC_TELEGRAM_API_BASE_URL")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .or_else(|| {
            env::var("TELEGRAM_API_BASE_URL")
                .ok()
                .filter(|value| !value.trim().is_empty())
        })
        .unwrap_or_else(|| "https://api.telegram.org".to_owned());

    mosaic_channel_telegram::TelegramOutboundClient::new_with_settings(
        bot_token,
        base_url,
        timeout,
        max_retries,
    )
    .map(Some)
}

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
        bot_name: ingress.and_then(|ingress| ingress.bot_name.clone()),
        bot_route: ingress.and_then(|ingress| ingress.bot_route.clone()),
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
        bot_name: None,
        bot_route: None,
        bot_profile: None,
        bot_token_env: None,
        capabilities: vec![
            "text_in".to_owned(),
            "session_hint".to_owned(),
            "thread_context".to_owned(),
        ],
        outbound_ready: true,
        status: status.to_owned(),
        detail: detail.to_owned(),
    }
}

pub(crate) fn telegram_adapter_statuses(
    components: &GatewayRuntimeComponents,
) -> Vec<AdapterStatusDto> {
    resolved_telegram_bots(components)
        .into_iter()
        .map(|bot| {
            let outbound_ready = if bot.legacy {
                env::var("MOSAIC_TELEGRAM_BOT_TOKEN").is_ok() || env::var("TELEGRAM_BOT_TOKEN").is_ok()
            } else {
                env::var(&bot.bot_token_env).is_ok()
            };
            let shared_secret = secret_status(bot.webhook_secret_token_env.as_deref());
            let (status, detail) = match (bot.enabled, outbound_ready, shared_secret) {
                (false, _, _) => ("disabled", "Telegram bot instance is disabled in workspace config."),
                (_, _, SecretStatus::MissingEnv(_)) => (
                    "error",
                    "Telegram webhook secret is configured but the environment variable is missing.",
                ),
                (true, true, SecretStatus::Ready(_)) => (
                    "ok",
                    "Telegram webhook ingress and replies are ready with secret-token verification.",
                ),
                (true, true, SecretStatus::Disabled) => {
                    ("ok", "Telegram webhook ingress and replies are ready.")
                }
                (true, false, SecretStatus::Ready(_)) => (
                    "warning",
                    "Telegram ingress is protected, but outbound replies still need the configured bot token env.",
                ),
                (true, false, SecretStatus::Disabled) => (
                    "warning",
                    "Telegram ingress is ready, but outbound replies still need the configured bot token env.",
                ),
            };
            let detail = format!(
                "{} scope: tools={} skills={} workflows={} attachments={}",
                detail,
                if bot.allowed_tools.is_empty() {
                    "<all>".to_owned()
                } else {
                    bot.allowed_tools.join(", ")
                },
                if bot.allowed_skills.is_empty() {
                    "<all>".to_owned()
                } else {
                    bot.allowed_skills.join(", ")
                },
                if bot.allowed_workflows.is_empty() {
                    "<all>".to_owned()
                } else {
                    bot.allowed_workflows.join(", ")
                },
                bot.attachments
                    .as_ref()
                    .map(attachment_policy_summary)
                    .unwrap_or_else(|| "workspace default".to_owned()),
            );
            AdapterStatusDto {
                name: bot.adapter_name(),
                channel: "telegram".to_owned(),
                transport: "http-webhook".to_owned(),
                ingress_path: bot.webhook_path.clone(),
                bot_name: Some(bot.name.clone()),
                bot_route: Some(bot.route_key.clone()),
                bot_profile: bot.default_profile.clone(),
                bot_token_env: Some(bot.bot_token_env.clone()),
                capabilities: vec![
                    "text_in".to_owned(),
                    "text_out".to_owned(),
                    "reply_target".to_owned(),
                    "thread_context".to_owned(),
                    "delivery_audit".to_owned(),
                ],
                outbound_ready,
                status: status.to_owned(),
                detail,
            }
        })
        .collect()
}

pub(crate) fn resolve_telegram_ingress_bot(
    gateway: &GatewayHandle,
    requested_route: Option<&str>,
    headers: &HeaderMap,
) -> Result<ResolvedTelegramBot, (StatusCode, Json<ErrorResponse>)> {
    let components = gateway.snapshot_components();
    let enabled_bots = resolved_telegram_bots(&components)
        .into_iter()
        .filter(|bot| bot.enabled)
        .collect::<Vec<_>>();
    let secret_header = header_token(headers, "x-telegram-bot-api-secret-token");

    let selected = if let Some(requested_route) = requested_route.map(str::trim) {
        let Some(bot) = enabled_bots.iter().find(|bot| {
            bot.route_key.eq_ignore_ascii_case(requested_route)
                || bot.name.eq_ignore_ascii_case(requested_route)
        }) else {
            return Err((
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: format!("telegram bot route not found: {requested_route}"),
                }),
            ));
        };
        bot.clone()
    } else if let Some(secret) = secret_header {
        let matches = enabled_bots
            .iter()
            .filter(|bot| {
                matches!(
                    secret_status(bot.webhook_secret_token_env.as_deref()),
                    SecretStatus::Ready(expected) if expected == secret
                )
            })
            .cloned()
            .collect::<Vec<_>>();
        match matches.as_slice() {
            [bot] => bot.clone(),
            [] => {
                record_auth_denial(gateway, "telegram ingress");
                return Err((
                    StatusCode::UNAUTHORIZED,
                    Json(ErrorResponse {
                        error: "telegram ingress shared secret required".to_owned(),
                    }),
                ));
            }
            _ => {
                return Err((
                    StatusCode::BAD_REQUEST,
                    Json(ErrorResponse {
                        error: "telegram ingress bot resolution is ambiguous".to_owned(),
                    }),
                ));
            }
        }
    } else if enabled_bots.len() == 1 {
        enabled_bots[0].clone()
    } else {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "telegram bot could not be resolved; use /ingress/telegram/<bot> or provide a matching secret token".to_owned(),
            }),
        ));
    };

    authorize_shared_secret_request(
        gateway,
        headers,
        selected.webhook_secret_token_env.as_deref(),
        "x-telegram-bot-api-secret-token",
        "telegram ingress",
    )?;

    Ok(selected)
}

fn attachment_policy_summary(target: &AttachmentRoutingTargetConfig) -> String {
    format!(
        "mode={} processor={} multimodal_profile={} specialized_processor_profile={}",
        target.mode.label(),
        target.processor.as_deref().unwrap_or("<none>"),
        target.multimodal_profile.as_deref().unwrap_or("<none>"),
        target
            .specialized_processor_profile
            .as_deref()
            .unwrap_or("<none>"),
    )
}

pub(crate) fn ingress_route(
    session_id: Option<&str>,
    ingress: Option<&IngressTrace>,
) -> Option<String> {
    let ingress = ingress?;
    let channel = ingress.channel.as_deref().unwrap_or(ingress.kind.as_str());
    let target = ingress
        .conversation_id
        .as_deref()
        .or(ingress.reply_target.as_deref())
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
