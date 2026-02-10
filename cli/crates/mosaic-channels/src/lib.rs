use std::path::{Path, PathBuf};
use std::time::Duration;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use mosaic_core::error::{MosaicError, Result};

pub const CHANNELS_SCHEMA_VERSION: u32 = 2;
pub const DEFAULT_CHANNEL_TOKEN_ENV: &str = "CHANNEL_TOKEN";
const SEND_RETRY_BACKOFF_MS: [u64; 3] = [200, 500, 1000];
const DEFAULT_HTTP_TIMEOUT_MS: u64 = 15_000;
const TEXT_PREVIEW_LIMIT: usize = 120;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelAuthConfig {
    pub token_env: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelEntry {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub endpoint: Option<String>,
    pub auth: ChannelAuthConfig,
    pub created_at: DateTime<Utc>,
    pub last_login_at: Option<DateTime<Utc>>,
    pub last_send_at: Option<DateTime<Utc>>,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelsFile {
    pub version: u32,
    pub channels: Vec<ChannelEntry>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ChannelListItem {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub endpoint_masked: Option<String>,
    pub created_at: DateTime<Utc>,
    pub last_login_at: Option<DateTime<Utc>>,
    pub last_send_at: Option<DateTime<Utc>>,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone)]
pub struct AddChannelInput {
    pub name: String,
    pub kind: String,
    pub endpoint: Option<String>,
    pub token_env: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ChannelSendResult {
    pub channel_id: String,
    pub delivered_via: String,
    pub kind: String,
    pub attempts: usize,
    pub http_status: Option<u16>,
    pub endpoint_masked: Option<String>,
    pub event_path: String,
    pub probe: bool,
}

#[derive(Debug, Clone)]
pub struct ChannelLoginResult {
    pub token_env: String,
    pub token_present: bool,
    pub channel: ChannelEntry,
}

#[derive(Debug, Clone)]
pub struct DoctorCheck {
    pub name: String,
    pub ok: bool,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize)]
struct ChannelEvent {
    ts: DateTime<Utc>,
    channel_id: String,
    kind: String,
    delivery_status: String,
    attempt: usize,
    http_status: Option<u16>,
    error: Option<String>,
    text_preview: String,
}

#[derive(Debug, Clone)]
pub struct ChannelRepository {
    channels_path: PathBuf,
    events_dir: PathBuf,
}

impl ChannelRepository {
    pub fn new(channels_path: PathBuf, events_dir: PathBuf) -> Self {
        Self {
            channels_path,
            events_dir,
        }
    }

    pub fn list(&self) -> Result<Vec<ChannelListItem>> {
        let file = self.load_channels_file()?;
        let items = file
            .channels
            .into_iter()
            .map(|channel| ChannelListItem {
                id: channel.id,
                name: channel.name,
                kind: channel.kind,
                endpoint_masked: mask_optional_endpoint(channel.endpoint.as_deref()),
                created_at: channel.created_at,
                last_login_at: channel.last_login_at,
                last_send_at: channel.last_send_at,
                last_error: channel.last_error,
            })
            .collect::<Vec<_>>();
        Ok(items)
    }

    pub fn add(&self, input: AddChannelInput) -> Result<ChannelEntry> {
        let mut file = self.load_channels_file()?;
        let name = input.name.trim();
        if name.is_empty() {
            return Err(MosaicError::Validation(
                "channel name cannot be empty".to_string(),
            ));
        }
        let kind = normalize_kind(&input.kind)?;
        validate_endpoint_for_kind(&kind, input.endpoint.as_deref())?;
        if file
            .channels
            .iter()
            .any(|channel| channel.name.eq_ignore_ascii_case(name))
        {
            return Err(MosaicError::Validation(format!(
                "channel name '{}' already exists",
                name
            )));
        }
        let now = Utc::now();
        let channel = ChannelEntry {
            id: format!("ch_{}", uuid::Uuid::new_v4()),
            name: name.to_string(),
            kind,
            endpoint: input.endpoint,
            auth: ChannelAuthConfig {
                token_env: input.token_env,
            },
            created_at: now,
            last_login_at: None,
            last_send_at: None,
            last_error: None,
        };
        file.channels.push(channel.clone());
        self.save_channels_file(&file)?;
        Ok(channel)
    }

    pub fn login(&self, channel_id: &str, token_env: &str) -> Result<ChannelLoginResult> {
        if token_env.trim().is_empty() {
            return Err(MosaicError::Validation(
                "token env cannot be empty".to_string(),
            ));
        }
        let mut file = self.load_channels_file()?;
        let channel = file
            .channels
            .iter_mut()
            .find(|entry| entry.id == channel_id)
            .ok_or_else(|| MosaicError::Config(format!("channel '{channel_id}' not found")))?;
        channel.last_login_at = Some(Utc::now());
        channel.auth.token_env = Some(token_env.to_string());
        let token_present = std::env::var(token_env).is_ok();
        let channel_cloned = channel.clone();
        self.save_channels_file(&file)?;
        Ok(ChannelLoginResult {
            token_env: token_env.to_string(),
            token_present,
            channel: channel_cloned,
        })
    }

    pub async fn send(
        &self,
        channel_id: &str,
        text: &str,
        token_env_override: Option<String>,
        probe: bool,
    ) -> Result<ChannelSendResult> {
        if text.trim().is_empty() {
            return Err(MosaicError::Validation(
                "send text cannot be empty".to_string(),
            ));
        }

        let mut file = self.load_channels_file()?;
        let idx = file
            .channels
            .iter()
            .position(|entry| entry.id == channel_id)
            .ok_or_else(|| MosaicError::Config(format!("channel '{channel_id}' not found")))?;
        let channel = file.channels[idx].clone();

        let token_env = token_env_override.or_else(|| channel.auth.token_env.clone());
        let token = resolve_token_value(token_env.as_deref())?;
        let kind = channel.kind.clone();
        let send_kind = if probe { "test_probe" } else { "message" };
        let text_preview = truncate_text(text, TEXT_PREVIEW_LIMIT);

        let delivery = match kind.as_str() {
            "mock" | "local" | "stdout" => DeliveryAttemptResult {
                ok: true,
                attempts: 1,
                http_status: Some(200),
                error: None,
                endpoint_masked: None,
            },
            "slack_webhook" => {
                let endpoint = channel.endpoint.clone().ok_or_else(|| {
                    MosaicError::Validation(format!(
                        "channel '{}' kind=slack_webhook requires endpoint",
                        channel.id
                    ))
                })?;
                send_with_retry(
                    &endpoint,
                    json!({ "text": text }),
                    token,
                    read_http_timeout(),
                )
                .await?
            }
            "webhook" => {
                let endpoint = channel.endpoint.clone().ok_or_else(|| {
                    MosaicError::Validation(format!(
                        "channel '{}' kind=webhook requires endpoint",
                        channel.id
                    ))
                })?;
                send_with_retry(
                    &endpoint,
                    json!({
                        "channel_id": channel.id,
                        "channel_name": channel.name,
                        "text": text,
                        "ts": Utc::now(),
                    }),
                    token,
                    read_http_timeout(),
                )
                .await?
            }
            other => {
                return Err(MosaicError::Validation(format!(
                    "unsupported channel kind '{other}', expected slack_webhook|webhook|mock|local|stdout"
                )));
            }
        };

        let event = ChannelEvent {
            ts: Utc::now(),
            channel_id: channel.id.clone(),
            kind: send_kind.to_string(),
            delivery_status: if delivery.ok {
                "success".to_string()
            } else {
                "failed".to_string()
            },
            attempt: delivery.attempts,
            http_status: delivery.http_status,
            error: delivery.error.clone(),
            text_preview,
        };
        let event_path = self.append_event(&channel.id, &event)?;

        if delivery.ok {
            if !probe {
                file.channels[idx].last_send_at = Some(Utc::now());
            }
            file.channels[idx].last_error = None;
        } else {
            file.channels[idx].last_error = delivery.error.clone();
        }
        self.save_channels_file(&file)?;

        if delivery.ok {
            return Ok(ChannelSendResult {
                channel_id: channel.id,
                delivered_via: channel.kind,
                kind: send_kind.to_string(),
                attempts: delivery.attempts,
                http_status: delivery.http_status,
                endpoint_masked: delivery.endpoint_masked,
                event_path: event_path.display().to_string(),
                probe,
            });
        }

        Err(MosaicError::Network(
            delivery
                .error
                .unwrap_or_else(|| "channel delivery failed".to_string()),
        ))
    }

    pub fn doctor_checks(&self) -> Result<Vec<DoctorCheck>> {
        let file = self.load_channels_file()?;
        let mut checks = vec![DoctorCheck {
            name: "channels_file".to_string(),
            ok: true,
            detail: format!(
                "loaded {} channels (schema v{})",
                file.channels.len(),
                file.version
            ),
        }];

        for channel in file.channels {
            let endpoint_valid =
                validate_endpoint_for_kind(&channel.kind, channel.endpoint.as_deref()).is_ok();
            checks.push(DoctorCheck {
                name: format!("channel_{}_endpoint", channel.id),
                ok: endpoint_valid,
                detail: if endpoint_valid {
                    format!(
                        "{} endpoint looks valid ({})",
                        channel.kind,
                        mask_optional_endpoint(channel.endpoint.as_deref())
                            .unwrap_or_else(|| "-".to_string())
                    )
                } else {
                    format!("{} endpoint invalid", channel.kind)
                },
            });
            if let Some(token_env) = &channel.auth.token_env {
                checks.push(DoctorCheck {
                    name: format!("channel_{}_token_env", channel.id),
                    ok: std::env::var(token_env).is_ok(),
                    detail: format!(
                        "{} {}",
                        token_env,
                        if std::env::var(token_env).is_ok() {
                            "is set"
                        } else {
                            "is missing"
                        }
                    ),
                });
            }
        }

        Ok(checks)
    }

    fn load_channels_file(&self) -> Result<ChannelsFile> {
        if !self.channels_path.exists() {
            return Ok(ChannelsFile {
                version: CHANNELS_SCHEMA_VERSION,
                channels: vec![],
            });
        }

        let raw = std::fs::read_to_string(&self.channels_path)?;
        let value: Value = serde_json::from_str(&raw).map_err(|err| {
            MosaicError::Validation(format!(
                "invalid channels JSON {}: {err}",
                self.channels_path.display()
            ))
        })?;
        let (mut file, migrated) = parse_channels_value(value)?;
        normalize_channels(&mut file.channels)?;
        if migrated || file.version != CHANNELS_SCHEMA_VERSION {
            file.version = CHANNELS_SCHEMA_VERSION;
            self.save_channels_file(&file)?;
        }
        Ok(file)
    }

    fn save_channels_file(&self, file: &ChannelsFile) -> Result<()> {
        if let Some(parent) = self.channels_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let rendered = serde_json::to_string_pretty(file).map_err(|err| {
            MosaicError::Validation(format!("failed to encode channels JSON: {err}"))
        })?;
        std::fs::write(&self.channels_path, rendered)?;
        Ok(())
    }

    fn append_event(&self, channel_id: &str, event: &ChannelEvent) -> Result<PathBuf> {
        std::fs::create_dir_all(&self.events_dir)?;
        let path = self.events_dir.join(format!("{channel_id}.jsonl"));
        let encoded = serde_json::to_string(event).map_err(|err| {
            MosaicError::Validation(format!("failed to encode channel event: {err}"))
        })?;
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)?;
        use std::io::Write as _;
        file.write_all(encoded.as_bytes())?;
        file.write_all(b"\n")?;
        Ok(path)
    }
}

#[derive(Debug, Clone, Deserialize)]
struct LegacyChannelEntry {
    id: String,
    name: String,
    kind: String,
    endpoint: Option<String>,
    created_at: DateTime<Utc>,
    last_login_at: Option<DateTime<Utc>>,
    #[serde(default)]
    last_login_token_env: Option<String>,
}

fn parse_channels_value(value: Value) -> Result<(ChannelsFile, bool)> {
    if value.is_array() {
        let legacy = serde_json::from_value::<Vec<LegacyChannelEntry>>(value).map_err(|err| {
            MosaicError::Validation(format!("invalid legacy channels array format: {err}"))
        })?;
        let channels = legacy
            .into_iter()
            .map(|entry| ChannelEntry {
                id: entry.id,
                name: entry.name,
                kind: normalize_kind(&entry.kind).unwrap_or_else(|_| "mock".to_string()),
                endpoint: entry.endpoint,
                auth: ChannelAuthConfig {
                    token_env: entry.last_login_token_env,
                },
                created_at: entry.created_at,
                last_login_at: entry.last_login_at,
                last_send_at: None,
                last_error: None,
            })
            .collect::<Vec<_>>();
        return Ok((
            ChannelsFile {
                version: CHANNELS_SCHEMA_VERSION,
                channels,
            },
            true,
        ));
    }

    let object = value.as_object().ok_or_else(|| {
        MosaicError::Validation("channels file must be an object or array".to_string())
    })?;
    let version = object.get("version").and_then(Value::as_u64).unwrap_or(1) as u32;
    let channels_value = object
        .get("channels")
        .cloned()
        .unwrap_or_else(|| Value::Array(vec![]));
    if version >= CHANNELS_SCHEMA_VERSION {
        let parsed = serde_json::from_value::<ChannelsFile>(json!({
            "version": CHANNELS_SCHEMA_VERSION,
            "channels": channels_value,
        }))
        .map_err(|err| MosaicError::Validation(format!("invalid channels v2 format: {err}")))?;
        return Ok((parsed, version != CHANNELS_SCHEMA_VERSION));
    }

    let legacy =
        serde_json::from_value::<Vec<LegacyChannelEntry>>(channels_value).map_err(|err| {
            MosaicError::Validation(format!("invalid channels legacy object format: {err}"))
        })?;
    let channels = legacy
        .into_iter()
        .map(|entry| ChannelEntry {
            id: entry.id,
            name: entry.name,
            kind: normalize_kind(&entry.kind).unwrap_or_else(|_| "mock".to_string()),
            endpoint: entry.endpoint,
            auth: ChannelAuthConfig {
                token_env: entry.last_login_token_env,
            },
            created_at: entry.created_at,
            last_login_at: entry.last_login_at,
            last_send_at: None,
            last_error: None,
        })
        .collect::<Vec<_>>();
    Ok((
        ChannelsFile {
            version: CHANNELS_SCHEMA_VERSION,
            channels,
        },
        true,
    ))
}

fn normalize_channels(channels: &mut [ChannelEntry]) -> Result<()> {
    for channel in channels {
        channel.kind = normalize_kind(&channel.kind)?;
        if channel
            .auth
            .token_env
            .as_ref()
            .is_some_and(|value| value.trim().is_empty())
        {
            channel.auth.token_env = None;
        }
        validate_endpoint_for_kind(&channel.kind, channel.endpoint.as_deref())?;
    }
    Ok(())
}

fn normalize_kind(kind: &str) -> Result<String> {
    let normalized = kind.trim().to_lowercase();
    let normalized = match normalized.as_str() {
        "slack" | "slack-webhook" | "slack_webhook" => "slack_webhook".to_string(),
        "webhook" => "webhook".to_string(),
        "mock" | "local" | "stdout" => normalized,
        other => {
            return Err(MosaicError::Validation(format!(
                "unsupported channel kind '{other}', expected slack_webhook|webhook|mock|local|stdout"
            )));
        }
    };
    Ok(normalized)
}

fn validate_endpoint_for_kind(kind: &str, endpoint: Option<&str>) -> Result<()> {
    match kind {
        "slack_webhook" => {
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
        }
        "webhook" => {
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
                "http" | "https" => {}
                scheme => {
                    return Err(MosaicError::Validation(format!(
                        "unsupported webhook endpoint scheme '{scheme}', expected http/https"
                    )));
                }
            }
        }
        "mock" | "local" | "stdout" => {}
        other => {
            return Err(MosaicError::Validation(format!(
                "unsupported channel kind '{other}'"
            )));
        }
    }
    Ok(())
}

fn mask_optional_endpoint(endpoint: Option<&str>) -> Option<String> {
    endpoint.map(mask_endpoint)
}

fn mask_endpoint(endpoint: &str) -> String {
    if endpoint.starts_with("mock-http://") {
        return "mock-http://***".to_string();
    }
    if let Ok(url) = reqwest::Url::parse(endpoint) {
        let host = url.host_str().unwrap_or("-");
        let path = url.path();
        let tail_len = 4usize;
        let tail = if path.len() > tail_len {
            &path[path.len() - tail_len..]
        } else {
            path
        };
        return format!("{}://{}/***{}", url.scheme(), host, tail);
    }
    if endpoint.len() <= 8 {
        "***".to_string()
    } else {
        format!("***{}", &endpoint[endpoint.len() - 4..])
    }
}

fn resolve_token_value(token_env: Option<&str>) -> Result<Option<String>> {
    let Some(token_env) = token_env else {
        return Ok(None);
    };
    let token_env = token_env.trim();
    if token_env.is_empty() {
        return Ok(None);
    }
    let token = std::env::var(token_env).map_err(|_| {
        MosaicError::Auth(format!("environment variable {} is required", token_env))
    })?;
    Ok(Some(token))
}

fn read_http_timeout() -> Duration {
    let ms = std::env::var("MOSAIC_CHANNELS_HTTP_TIMEOUT_MS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(DEFAULT_HTTP_TIMEOUT_MS);
    Duration::from_millis(ms)
}

fn truncate_text(text: &str, max_chars: usize) -> String {
    let mut truncated = String::new();
    for (idx, ch) in text.chars().enumerate() {
        if idx >= max_chars {
            truncated.push_str("...");
            break;
        }
        truncated.push(ch);
    }
    truncated
}

#[derive(Debug, Clone)]
struct DeliveryAttemptResult {
    ok: bool,
    attempts: usize,
    http_status: Option<u16>,
    error: Option<String>,
    endpoint_masked: Option<String>,
}

async fn send_with_retry(
    endpoint: &str,
    payload: Value,
    bearer_token: Option<String>,
    timeout: Duration,
) -> Result<DeliveryAttemptResult> {
    if endpoint.starts_with("mock-http://") {
        return simulate_mock_http(endpoint).await;
    }

    let client = reqwest::Client::builder()
        .timeout(timeout)
        .build()
        .map_err(|err| MosaicError::Network(format!("failed to build HTTP client: {err}")))?;
    let mut attempts = 0usize;
    let mut last_error: Option<String> = None;
    let mut last_status: Option<u16> = None;

    for retry_idx in 0..=SEND_RETRY_BACKOFF_MS.len() {
        attempts += 1;
        if retry_idx > 0 {
            tokio::time::sleep(Duration::from_millis(SEND_RETRY_BACKOFF_MS[retry_idx - 1])).await;
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
            }
            Err(err) => {
                let message = if err.is_timeout() {
                    "webhook request timed out".to_string()
                } else {
                    format!("webhook request failed: {err}")
                };
                last_error = Some(message);
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

async fn simulate_mock_http(endpoint: &str) -> Result<DeliveryAttemptResult> {
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
        if idx > 0 && idx - 1 < SEND_RETRY_BACKOFF_MS.len() {
            tokio::time::sleep(Duration::from_millis(SEND_RETRY_BACKOFF_MS[idx - 1])).await;
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

pub fn format_channel_for_output(channel: &ChannelEntry) -> ChannelListItem {
    ChannelListItem {
        id: channel.id.clone(),
        name: channel.name.clone(),
        kind: channel.kind.clone(),
        endpoint_masked: mask_optional_endpoint(channel.endpoint.as_deref()),
        created_at: channel.created_at,
        last_login_at: channel.last_login_at,
        last_send_at: channel.last_send_at,
        last_error: channel.last_error.clone(),
    }
}

pub fn channels_file_path(data_dir: &Path) -> PathBuf {
    data_dir.join("channels.json")
}

pub fn channels_events_dir(data_dir: &Path) -> PathBuf {
    data_dir.join("channel-events")
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::*;

    #[test]
    fn migrate_legacy_channels_array_to_v2() {
        let legacy = json!([
            {
                "id": "ch_1",
                "name": "legacy",
                "kind": "webhook",
                "endpoint": "https://example.com/webhook",
                "created_at": "2026-02-10T00:00:00Z",
                "last_login_at": null,
                "last_login_token_env": "LEGACY_TOKEN"
            }
        ]);
        let (file, migrated) = parse_channels_value(legacy).expect("parse");
        assert!(migrated);
        assert_eq!(file.version, CHANNELS_SCHEMA_VERSION);
        assert_eq!(file.channels.len(), 1);
        assert_eq!(
            file.channels[0].auth.token_env.as_deref(),
            Some("LEGACY_TOKEN")
        );
        assert!(file.channels[0].last_send_at.is_none());
    }

    #[test]
    fn endpoint_validation_for_slack_webhook() {
        assert!(
            validate_endpoint_for_kind(
                "slack_webhook",
                Some("https://hooks.slack.com/services/T/B/X")
            )
            .is_ok()
        );
        assert!(validate_endpoint_for_kind("slack_webhook", Some("https://example.com")).is_err());
    }

    #[tokio::test]
    async fn mock_http_simulates_retry_and_success() {
        let result = send_with_retry(
            "mock-http://500,500,200",
            json!({"text":"hello"}),
            None,
            Duration::from_millis(10),
        )
        .await
        .expect("send");
        assert!(result.ok);
        assert_eq!(result.attempts, 3);
        assert_eq!(result.http_status, Some(200));
    }

    #[tokio::test]
    async fn repository_send_probe_does_not_set_last_send_at() {
        let temp = tempdir().expect("tempdir");
        let repo = ChannelRepository::new(
            channels_file_path(temp.path()),
            channels_events_dir(temp.path()),
        );
        let channel = repo
            .add(AddChannelInput {
                name: "slack".to_string(),
                kind: "slack_webhook".to_string(),
                endpoint: Some("mock-http://200".to_string()),
                token_env: None,
            })
            .expect("add");
        let result = repo
            .send(&channel.id, "probe", None, true)
            .await
            .expect("probe send");
        assert!(result.probe);
        let list = repo.list().expect("list");
        assert!(list[0].last_send_at.is_none());
    }
}
