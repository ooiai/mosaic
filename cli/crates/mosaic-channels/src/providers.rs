use std::collections::HashMap;
use std::sync::{Arc, OnceLock};

use async_trait::async_trait;
use chrono::Utc;
use serde_json::{Value, json};

use mosaic_core::error::{MosaicError, Result};

use crate::policy::{RetryPolicy, should_retry_http_status};
use crate::schema::{DEFAULT_TELEGRAM_TOKEN_ENV, mask_endpoint};
use crate::types::ChannelCapability;

#[derive(Debug, Clone)]
pub(crate) struct DeliveryAttemptResult {
    pub ok: bool,
    pub attempts: usize,
    pub http_status: Option<u16>,
    pub error: Option<String>,
    pub endpoint_masked: Option<String>,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct ChannelDispatchRequest<'a> {
    pub channel_id: &'a str,
    pub channel_name: &'a str,
    pub endpoint: Option<&'a str>,
    pub target: Option<&'a str>,
    pub text: &'a str,
    pub parse_mode: Option<&'a str>,
    pub bearer_token: Option<&'a str>,
}

#[async_trait]
trait ChannelProvider: Send + Sync {
    fn canonical_kind(&self) -> &'static str;

    fn aliases(&self) -> &'static [&'static str] {
        &[]
    }

    fn default_token_env(&self) -> Option<&'static str> {
        None
    }

    fn capability(&self) -> ChannelCapability;

    fn validate_channel(&self, endpoint: Option<&str>, target: Option<&str>) -> Result<()>;

    async fn send(
        &self,
        request: ChannelDispatchRequest<'_>,
        policy: &RetryPolicy,
    ) -> Result<DeliveryAttemptResult>;
}

#[derive(Default)]
struct ChannelProviderRegistry {
    providers: HashMap<String, Arc<dyn ChannelProvider>>,
    aliases: HashMap<String, String>,
}

impl ChannelProviderRegistry {
    fn with_defaults() -> Self {
        let mut registry = Self::default();
        registry.register(Arc::new(SlackWebhookProvider));
        registry.register(Arc::new(DiscordWebhookProvider));
        registry.register(Arc::new(GenericWebhookProvider));
        registry.register(Arc::new(TelegramBotProvider));
        registry.register(Arc::new(LocalProvider { kind: "mock" }));
        registry.register(Arc::new(TerminalProvider));
        registry
    }

    fn register(&mut self, provider: Arc<dyn ChannelProvider>) {
        let canonical = normalize_kind_token(provider.canonical_kind());
        self.aliases.insert(canonical.clone(), canonical.clone());
        for alias in provider.aliases() {
            self.aliases
                .insert(normalize_kind_token(alias), canonical.clone());
        }
        self.providers.insert(canonical, provider);
    }

    fn resolve_kind(&self, kind: &str) -> Option<String> {
        self.aliases.get(&normalize_kind_token(kind)).cloned()
    }

    fn validate_channel_for_kind(
        &self,
        kind: &str,
        endpoint: Option<&str>,
        target: Option<&str>,
    ) -> Result<()> {
        let provider = self.provider_for_kind(kind)?;
        provider.validate_channel(endpoint, target)
    }

    async fn dispatch(
        &self,
        kind: &str,
        request: ChannelDispatchRequest<'_>,
        policy: &RetryPolicy,
    ) -> Result<DeliveryAttemptResult> {
        let provider = self.provider_for_kind(kind)?;
        provider.send(request, policy).await
    }

    fn supported_kinds(&self) -> Vec<String> {
        let mut kinds = self.providers.keys().cloned().collect::<Vec<_>>();
        kinds.sort();
        kinds
    }

    fn supported_kinds_hint(&self) -> String {
        self.supported_kinds().join("|")
    }

    fn default_token_env_for_kind(&self, kind: &str) -> Result<Option<&'static str>> {
        let provider = self.provider_for_kind(kind)?;
        Ok(provider.default_token_env())
    }

    fn capabilities_for_kind(&self, kind: Option<&str>) -> Result<Vec<ChannelCapability>> {
        if let Some(kind) = kind {
            let provider = self.provider_for_kind(kind)?;
            return Ok(vec![provider.capability()]);
        }
        let mut capabilities = self
            .providers
            .values()
            .map(|provider| provider.capability())
            .collect::<Vec<_>>();
        capabilities.sort_by(|lhs, rhs| lhs.kind.cmp(&rhs.kind));
        Ok(capabilities)
    }

    fn provider_for_kind(&self, kind: &str) -> Result<Arc<dyn ChannelProvider>> {
        let resolved = self.resolve_kind(kind).ok_or_else(|| {
            MosaicError::ChannelUnsupported(format!(
                "unsupported channel kind '{}', expected {}",
                kind,
                self.supported_kinds_hint()
            ))
        })?;
        self.providers.get(&resolved).cloned().ok_or_else(|| {
            MosaicError::Validation(format!(
                "channel provider for kind '{}' is not registered",
                resolved
            ))
        })
    }
}

struct SlackWebhookProvider;

#[async_trait]
impl ChannelProvider for SlackWebhookProvider {
    fn canonical_kind(&self) -> &'static str {
        "slack_webhook"
    }

    fn aliases(&self) -> &'static [&'static str] {
        &["slack", "slack-webhook"]
    }

    fn capability(&self) -> ChannelCapability {
        ChannelCapability {
            kind: self.canonical_kind().to_string(),
            aliases: self.aliases().iter().map(ToString::to_string).collect(),
            supports_endpoint: true,
            supports_token_env: true,
            supports_test_probe: true,
            supports_bearer_token: true,
            supports_parse_mode: false,
            supports_message_template: true,
            supports_idempotency_key: true,
            supports_rate_limit_report: false,
        }
    }

    fn validate_channel(&self, endpoint: Option<&str>, target: Option<&str>) -> Result<()> {
        if target.is_some() {
            return Err(MosaicError::Validation(
                "slack_webhook channel does not support --chat-id".to_string(),
            ));
        }
        let endpoint = endpoint.ok_or_else(|| {
            MosaicError::Validation("slack_webhook channel requires --endpoint".to_string())
        })?;
        if endpoint.starts_with("mock-http://") {
            return Ok(());
        }
        let url = reqwest::Url::parse(endpoint).map_err(|err| {
            MosaicError::Validation(format!("invalid slack webhook endpoint URL: {err}"))
        })?;
        let host_ok = matches!(url.host_str(), Some("hooks.slack.com"));
        let path_ok = url.path().starts_with("/services/");
        if !host_ok || !path_ok {
            return Err(MosaicError::Validation(
                "slack webhook endpoint must match https://hooks.slack.com/services/..."
                    .to_string(),
            ));
        }
        Ok(())
    }

    async fn send(
        &self,
        request: ChannelDispatchRequest<'_>,
        policy: &RetryPolicy,
    ) -> Result<DeliveryAttemptResult> {
        let endpoint = request.endpoint.ok_or_else(|| {
            MosaicError::Validation("slack_webhook channel requires --endpoint".to_string())
        })?;
        send_with_retry(
            endpoint,
            json!({ "text": request.text }),
            request.bearer_token.map(str::to_string),
            policy,
        )
        .await
    }
}

struct GenericWebhookProvider;

#[async_trait]
impl ChannelProvider for GenericWebhookProvider {
    fn canonical_kind(&self) -> &'static str {
        "webhook"
    }

    fn capability(&self) -> ChannelCapability {
        ChannelCapability {
            kind: self.canonical_kind().to_string(),
            aliases: Vec::new(),
            supports_endpoint: true,
            supports_token_env: true,
            supports_test_probe: true,
            supports_bearer_token: true,
            supports_parse_mode: false,
            supports_message_template: true,
            supports_idempotency_key: true,
            supports_rate_limit_report: false,
        }
    }

    fn validate_channel(&self, endpoint: Option<&str>, target: Option<&str>) -> Result<()> {
        if target.is_some() {
            return Err(MosaicError::Validation(
                "webhook channel does not support --chat-id".to_string(),
            ));
        }
        let endpoint = endpoint.ok_or_else(|| {
            MosaicError::Validation("webhook channel requires --endpoint".to_string())
        })?;
        if endpoint.starts_with("mock-http://") {
            return Ok(());
        }
        let url = reqwest::Url::parse(endpoint).map_err(|err| {
            MosaicError::Validation(format!("invalid webhook endpoint URL: {err}"))
        })?;
        match url.scheme() {
            "http" | "https" => Ok(()),
            scheme => Err(MosaicError::Validation(format!(
                "unsupported webhook endpoint scheme '{scheme}', expected http/https"
            ))),
        }
    }

    async fn send(
        &self,
        request: ChannelDispatchRequest<'_>,
        policy: &RetryPolicy,
    ) -> Result<DeliveryAttemptResult> {
        let endpoint = request.endpoint.ok_or_else(|| {
            MosaicError::Validation("webhook channel requires --endpoint".to_string())
        })?;
        let payload = json!({
            "channel_id": request.channel_id,
            "channel_name": request.channel_name,
            "text": request.text,
            "ts": Utc::now(),
        });
        send_with_retry(
            endpoint,
            payload,
            request.bearer_token.map(str::to_string),
            policy,
        )
        .await
    }
}

struct DiscordWebhookProvider;

#[async_trait]
impl ChannelProvider for DiscordWebhookProvider {
    fn canonical_kind(&self) -> &'static str {
        "discord_webhook"
    }

    fn aliases(&self) -> &'static [&'static str] {
        &["discord", "discord-webhook"]
    }

    fn capability(&self) -> ChannelCapability {
        ChannelCapability {
            kind: self.canonical_kind().to_string(),
            aliases: self.aliases().iter().map(ToString::to_string).collect(),
            supports_endpoint: true,
            supports_token_env: true,
            supports_test_probe: true,
            supports_bearer_token: true,
            supports_parse_mode: false,
            supports_message_template: true,
            supports_idempotency_key: true,
            supports_rate_limit_report: false,
        }
    }

    fn validate_channel(&self, endpoint: Option<&str>, target: Option<&str>) -> Result<()> {
        if target.is_some() {
            return Err(MosaicError::Validation(
                "discord_webhook channel does not support --chat-id".to_string(),
            ));
        }
        let endpoint = endpoint.ok_or_else(|| {
            MosaicError::Validation("discord_webhook channel requires --endpoint".to_string())
        })?;
        if endpoint.starts_with("mock-http://") {
            return Ok(());
        }
        let url = reqwest::Url::parse(endpoint).map_err(|err| {
            MosaicError::Validation(format!("invalid discord webhook endpoint URL: {err}"))
        })?;
        let host_ok = matches!(
            url.host_str(),
            Some("discord.com")
                | Some("canary.discord.com")
                | Some("ptb.discord.com")
                | Some("discordapp.com")
        );
        let path = url.path();
        let path_ok = path.starts_with("/api/webhooks/") || path.contains("/webhooks/");
        if !host_ok || !path_ok {
            return Err(MosaicError::Validation(
                "discord webhook endpoint must match https://discord.com/api/webhooks/..."
                    .to_string(),
            ));
        }
        Ok(())
    }

    async fn send(
        &self,
        request: ChannelDispatchRequest<'_>,
        policy: &RetryPolicy,
    ) -> Result<DeliveryAttemptResult> {
        let endpoint = request.endpoint.ok_or_else(|| {
            MosaicError::Validation("discord_webhook channel requires --endpoint".to_string())
        })?;
        send_with_retry(
            endpoint,
            json!({ "content": request.text }),
            request.bearer_token.map(str::to_string),
            policy,
        )
        .await
    }
}

struct TelegramBotProvider;

#[async_trait]
impl ChannelProvider for TelegramBotProvider {
    fn canonical_kind(&self) -> &'static str {
        "telegram_bot"
    }

    fn aliases(&self) -> &'static [&'static str] {
        &["telegram", "telegram-bot"]
    }

    fn default_token_env(&self) -> Option<&'static str> {
        Some(DEFAULT_TELEGRAM_TOKEN_ENV)
    }

    fn capability(&self) -> ChannelCapability {
        ChannelCapability {
            kind: self.canonical_kind().to_string(),
            aliases: self.aliases().iter().map(ToString::to_string).collect(),
            supports_endpoint: true,
            supports_token_env: true,
            supports_test_probe: true,
            supports_bearer_token: true,
            supports_parse_mode: true,
            supports_message_template: true,
            supports_idempotency_key: true,
            supports_rate_limit_report: true,
        }
    }

    fn validate_channel(&self, endpoint: Option<&str>, target: Option<&str>) -> Result<()> {
        let target = target.ok_or_else(|| {
            MosaicError::Validation("telegram_bot channel requires --chat-id".to_string())
        })?;
        if target.trim().is_empty() {
            return Err(MosaicError::Validation(
                "telegram_bot channel requires non-empty --chat-id".to_string(),
            ));
        }
        let Some(endpoint) = endpoint else {
            return Ok(());
        };
        if endpoint.starts_with("mock-http://") {
            return Ok(());
        }
        let url = reqwest::Url::parse(endpoint).map_err(|err| {
            MosaicError::Validation(format!("invalid telegram endpoint URL: {err}"))
        })?;
        match url.scheme() {
            "http" | "https" => Ok(()),
            scheme => Err(MosaicError::Validation(format!(
                "unsupported telegram endpoint scheme '{scheme}', expected http/https"
            ))),
        }
    }

    async fn send(
        &self,
        request: ChannelDispatchRequest<'_>,
        policy: &RetryPolicy,
    ) -> Result<DeliveryAttemptResult> {
        let chat_id = request.target.ok_or_else(|| {
            MosaicError::Validation("telegram_bot channel requires configured chat id".to_string())
        })?;
        let token = request.bearer_token.ok_or_else(|| {
            MosaicError::Auth(format!(
                "telegram_bot channel requires bot token via --token-env or {}",
                DEFAULT_TELEGRAM_TOKEN_ENV
            ))
        })?;
        let endpoint = request.endpoint.unwrap_or("https://api.telegram.org");
        send_telegram_with_retry(
            endpoint,
            chat_id,
            request.text,
            request.parse_mode,
            token,
            policy,
        )
        .await
    }
}

struct TerminalProvider;

#[async_trait]
impl ChannelProvider for TerminalProvider {
    fn canonical_kind(&self) -> &'static str {
        "terminal"
    }

    fn aliases(&self) -> &'static [&'static str] {
        &["local", "stdout"]
    }

    fn capability(&self) -> ChannelCapability {
        ChannelCapability {
            kind: self.canonical_kind().to_string(),
            aliases: self.aliases().iter().map(ToString::to_string).collect(),
            supports_endpoint: false,
            supports_token_env: false,
            supports_test_probe: true,
            supports_bearer_token: false,
            supports_parse_mode: false,
            supports_message_template: true,
            supports_idempotency_key: true,
            supports_rate_limit_report: false,
        }
    }

    fn validate_channel(&self, endpoint: Option<&str>, target: Option<&str>) -> Result<()> {
        let _ = endpoint;
        if target.is_some() {
            return Err(MosaicError::Validation(
                "terminal channel does not support --chat-id".to_string(),
            ));
        }
        Ok(())
    }

    async fn send(
        &self,
        _request: ChannelDispatchRequest<'_>,
        _policy: &RetryPolicy,
    ) -> Result<DeliveryAttemptResult> {
        Ok(local_delivery_success())
    }
}

struct LocalProvider {
    kind: &'static str,
}

#[async_trait]
impl ChannelProvider for LocalProvider {
    fn canonical_kind(&self) -> &'static str {
        self.kind
    }

    fn capability(&self) -> ChannelCapability {
        ChannelCapability {
            kind: self.canonical_kind().to_string(),
            aliases: Vec::new(),
            supports_endpoint: false,
            supports_token_env: true,
            supports_test_probe: true,
            supports_bearer_token: true,
            supports_parse_mode: false,
            supports_message_template: true,
            supports_idempotency_key: true,
            supports_rate_limit_report: false,
        }
    }

    fn validate_channel(&self, _endpoint: Option<&str>, _target: Option<&str>) -> Result<()> {
        Ok(())
    }

    async fn send(
        &self,
        _request: ChannelDispatchRequest<'_>,
        _policy: &RetryPolicy,
    ) -> Result<DeliveryAttemptResult> {
        Ok(local_delivery_success())
    }
}

fn normalize_kind_token(kind: &str) -> String {
    kind.trim().to_lowercase().replace('-', "_")
}

fn default_registry() -> &'static ChannelProviderRegistry {
    static REGISTRY: OnceLock<ChannelProviderRegistry> = OnceLock::new();
    REGISTRY.get_or_init(ChannelProviderRegistry::with_defaults)
}

pub(crate) fn resolve_kind(kind: &str) -> Option<String> {
    default_registry().resolve_kind(kind)
}

pub(crate) fn supported_kinds_hint() -> String {
    default_registry().supported_kinds_hint()
}

pub(crate) fn validate_channel_for_kind(
    kind: &str,
    endpoint: Option<&str>,
    target: Option<&str>,
) -> Result<()> {
    default_registry().validate_channel_for_kind(kind, endpoint, target)
}

pub(crate) fn capabilities_for_kind(kind: Option<&str>) -> Result<Vec<ChannelCapability>> {
    default_registry().capabilities_for_kind(kind)
}

pub(crate) fn default_token_env_for_kind(kind: &str) -> Option<&'static str> {
    default_registry()
        .default_token_env_for_kind(kind)
        .unwrap_or_default()
}

pub(crate) async fn dispatch_send(
    kind: &str,
    request: ChannelDispatchRequest<'_>,
    policy: &RetryPolicy,
) -> Result<DeliveryAttemptResult> {
    default_registry().dispatch(kind, request, policy).await
}

async fn send_with_retry(
    endpoint: &str,
    payload: Value,
    bearer_token: Option<String>,
    policy: &RetryPolicy,
) -> Result<DeliveryAttemptResult> {
    if endpoint.starts_with("mock-http://") {
        return simulate_mock_http(endpoint, policy).await;
    }

    let client = reqwest::Client::builder()
        .timeout(policy.timeout)
        .build()
        .map_err(|err| MosaicError::Network(format!("failed to build HTTP client: {err}")))?;

    let mut attempts = 0usize;
    let mut last_error: Option<String> = None;
    let mut last_status: Option<u16> = None;

    for attempt_idx in 0..policy.max_attempts() {
        attempts += 1;
        if let Some(delay) = policy.backoff_before_attempt(attempt_idx) {
            tokio::time::sleep(delay).await;
        }

        let mut request = client.post(endpoint).json(&payload);
        if let Some(token) = &bearer_token {
            request = request.bearer_auth(token);
        }

        match request.send().await {
            Ok(response) => {
                let status = response.status().as_u16();
                last_status = Some(status);
                if response.status().is_success() {
                    return Ok(DeliveryAttemptResult {
                        ok: true,
                        attempts,
                        http_status: Some(status),
                        error: None,
                        endpoint_masked: Some(mask_endpoint(endpoint)),
                    });
                }

                if response.status().is_client_error() {
                    return Ok(DeliveryAttemptResult {
                        ok: false,
                        attempts,
                        http_status: Some(status),
                        error: Some(format!("webhook returned client error status {status}")),
                        endpoint_masked: Some(mask_endpoint(endpoint)),
                    });
                }

                last_error = Some(format!("webhook returned server error status {status}"));
                if !should_retry_http_status(status) || attempt_idx + 1 >= policy.max_attempts() {
                    break;
                }
            }
            Err(err) => {
                last_error = Some(if err.is_timeout() {
                    "webhook request timed out".to_string()
                } else {
                    format!("webhook request failed: {err}")
                });
                if attempt_idx + 1 >= policy.max_attempts() {
                    break;
                }
            }
        }
    }

    Ok(DeliveryAttemptResult {
        ok: false,
        attempts,
        http_status: last_status,
        error: Some(
            last_error.unwrap_or_else(|| "webhook request failed after retries".to_string()),
        ),
        endpoint_masked: Some(mask_endpoint(endpoint)),
    })
}

async fn send_telegram_with_retry(
    endpoint: &str,
    chat_id: &str,
    text: &str,
    parse_mode: Option<&str>,
    token: &str,
    policy: &RetryPolicy,
) -> Result<DeliveryAttemptResult> {
    if endpoint.starts_with("mock-http://") {
        return simulate_mock_http(endpoint, policy).await;
    }

    let base_url = reqwest::Url::parse(endpoint)
        .map_err(|err| MosaicError::Validation(format!("invalid telegram endpoint URL: {err}")))?;
    let scheme = base_url.scheme();
    if scheme != "http" && scheme != "https" {
        return Err(MosaicError::Validation(format!(
            "unsupported telegram endpoint scheme '{scheme}', expected http/https"
        )));
    }
    let base_endpoint = endpoint.trim_end_matches('/');
    let url = format!("{base_endpoint}/bot{token}/sendMessage");
    let mut payload = json!({
        "chat_id": chat_id,
        "text": text,
    });
    if let Some(parse_mode) = parse_mode {
        payload["parse_mode"] = Value::String(parse_mode.to_string());
    }

    let client = reqwest::Client::builder()
        .timeout(policy.timeout)
        .build()
        .map_err(|err| MosaicError::Network(format!("failed to build HTTP client: {err}")))?;

    let mut attempts = 0usize;
    let mut last_error: Option<String> = None;
    let mut last_status: Option<u16> = None;

    for attempt_idx in 0..policy.max_attempts() {
        attempts += 1;
        if let Some(delay) = policy.backoff_before_attempt(attempt_idx) {
            tokio::time::sleep(delay).await;
        }

        match client.post(&url).json(&payload).send().await {
            Ok(response) => {
                let status = response.status().as_u16();
                last_status = Some(status);
                if response.status().is_success() {
                    let body = response.json::<Value>().await.map_err(|err| {
                        MosaicError::Network(format!("failed to parse telegram response: {err}"))
                    })?;
                    return match parse_telegram_success_response(&body) {
                        Ok(()) => Ok(DeliveryAttemptResult {
                            ok: true,
                            attempts,
                            http_status: Some(status),
                            error: None,
                            endpoint_masked: Some(mask_endpoint(endpoint)),
                        }),
                        Err(message) => Ok(DeliveryAttemptResult {
                            ok: false,
                            attempts,
                            http_status: Some(status),
                            error: Some(message),
                            endpoint_masked: Some(mask_endpoint(endpoint)),
                        }),
                    };
                }

                if response.status().is_client_error() {
                    return Ok(DeliveryAttemptResult {
                        ok: false,
                        attempts,
                        http_status: Some(status),
                        error: Some(format!(
                            "telegram API returned client error status {status}"
                        )),
                        endpoint_masked: Some(mask_endpoint(endpoint)),
                    });
                }

                last_error = Some(format!(
                    "telegram API returned server error status {status}"
                ));
                if !should_retry_http_status(status) || attempt_idx + 1 >= policy.max_attempts() {
                    break;
                }
            }
            Err(err) => {
                last_error = Some(if err.is_timeout() {
                    "telegram request timed out".to_string()
                } else {
                    format!("telegram request failed: {err}")
                });
                if attempt_idx + 1 >= policy.max_attempts() {
                    break;
                }
            }
        }
    }

    Ok(DeliveryAttemptResult {
        ok: false,
        attempts,
        http_status: last_status,
        error: Some(
            last_error.unwrap_or_else(|| "telegram request failed after retries".to_string()),
        ),
        endpoint_masked: Some(mask_endpoint(endpoint)),
    })
}

fn parse_telegram_success_response(body: &Value) -> std::result::Result<(), String> {
    let object = body
        .as_object()
        .ok_or_else(|| "telegram API response is not a JSON object".to_string())?;
    let ok = object
        .get("ok")
        .and_then(Value::as_bool)
        .ok_or_else(|| "telegram API response missing boolean 'ok'".to_string())?;
    if ok {
        Ok(())
    } else if let Some(description) = object.get("description").and_then(Value::as_str) {
        Err(format!("telegram API error: {description}"))
    } else {
        Err("telegram API returned ok=false".to_string())
    }
}

async fn simulate_mock_http(endpoint: &str, policy: &RetryPolicy) -> Result<DeliveryAttemptResult> {
    let sequence = endpoint.trim_start_matches("mock-http://");
    if sequence.trim().is_empty() {
        return Ok(DeliveryAttemptResult {
            ok: true,
            attempts: 1,
            http_status: Some(200),
            error: None,
            endpoint_masked: Some(mask_endpoint(endpoint)),
        });
    }

    let steps = sequence
        .split(',')
        .map(|value| value.trim().to_lowercase())
        .collect::<Vec<_>>();

    let mut attempts = 0usize;
    let mut last_status: Option<u16> = None;
    let mut last_error: Option<String> = None;

    for (idx, step) in steps.iter().enumerate() {
        attempts += 1;
        if let Some(delay) = policy.backoff_before_attempt(idx) {
            tokio::time::sleep(delay).await;
        }

        if step == "timeout" {
            last_error = Some("webhook request timed out".to_string());
            continue;
        }

        let status = step.parse::<u16>().map_err(|_| {
            MosaicError::Validation(format!(
                "invalid mock-http response step '{}' in endpoint {}",
                step, endpoint
            ))
        })?;

        last_status = Some(status);
        if (200..300).contains(&status) {
            return Ok(DeliveryAttemptResult {
                ok: true,
                attempts,
                http_status: Some(status),
                error: None,
                endpoint_masked: Some(mask_endpoint(endpoint)),
            });
        }

        if (400..500).contains(&status) {
            return Ok(DeliveryAttemptResult {
                ok: false,
                attempts,
                http_status: Some(status),
                error: Some(format!("webhook returned client error status {status}")),
                endpoint_masked: Some(mask_endpoint(endpoint)),
            });
        }

        last_error = Some(format!("webhook returned server error status {status}"));
    }

    Ok(DeliveryAttemptResult {
        ok: false,
        attempts,
        http_status: last_status,
        error: Some(last_error.unwrap_or_else(|| "mock-http failed".to_string())),
        endpoint_masked: Some(mask_endpoint(endpoint)),
    })
}

fn local_delivery_success() -> DeliveryAttemptResult {
    DeliveryAttemptResult {
        ok: true,
        attempts: 1,
        http_status: Some(200),
        error: None,
        endpoint_masked: None,
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use serde_json::json;

    use super::*;

    #[test]
    fn registry_resolves_kind_aliases() {
        assert_eq!(resolve_kind("slack"), Some("slack_webhook".to_string()));
        assert_eq!(
            resolve_kind("slack-webhook"),
            Some("slack_webhook".to_string())
        );
        assert_eq!(resolve_kind("discord"), Some("discord_webhook".to_string()));
        assert_eq!(
            resolve_kind("discord-webhook"),
            Some("discord_webhook".to_string())
        );
        assert_eq!(resolve_kind("local"), Some("terminal".to_string()));
        assert_eq!(resolve_kind("stdout"), Some("terminal".to_string()));
        assert_eq!(resolve_kind("terminal"), Some("terminal".to_string()));
        assert_eq!(resolve_kind("telegram"), Some("telegram_bot".to_string()));
    }

    #[tokio::test]
    async fn mock_http_simulates_retry_and_success() {
        let policy = RetryPolicy {
            timeout: Duration::from_millis(10),
            backoff_ms: vec![1, 1, 1],
        };
        let result = dispatch_send(
            "webhook",
            ChannelDispatchRequest {
                channel_id: "ch_1",
                channel_name: "demo",
                endpoint: Some("mock-http://500,500,200"),
                target: None,
                text: "hello",
                parse_mode: None,
                bearer_token: None,
            },
            &policy,
        )
        .await
        .expect("send");
        assert!(result.ok);
        assert_eq!(result.attempts, 3);
        assert_eq!(result.http_status, Some(200));
    }

    #[test]
    fn parse_telegram_response_checks_ok_flag() {
        assert!(parse_telegram_success_response(&json!({"ok": true})).is_ok());
        assert!(parse_telegram_success_response(&json!({"ok": false})).is_err());
        assert!(parse_telegram_success_response(&json!({"description": "missing"})).is_err());
    }
}
