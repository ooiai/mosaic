use std::collections::{BTreeMap, HashSet};
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

use chrono::{DateTime, Duration, Utc};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use mosaic_core::error::{MosaicError, Result};

use crate::policy::RetryPolicy;
use crate::providers;
use crate::schema::{
    CHANNELS_SCHEMA_VERSION, DEFAULT_CHANNEL_TOKEN_ENV, mask_optional_endpoint,
    mask_optional_target, normalize_channels, normalize_kind, parse_channels_value,
    validate_channel_for_kind,
};
use crate::types::{
    AddChannelInput, ChannelAuthConfig, ChannelCapability, ChannelDirectoryEntry, ChannelEntry,
    ChannelListItem, ChannelLogEntry, ChannelLoginResult, ChannelSendOptions, ChannelSendResult,
    ChannelStatus, ChannelsFile, DoctorCheck, TEXT_PREVIEW_LIMIT, truncate_text,
};

const CACHE_TTL_SECONDS: i64 = 300;
const DEFAULT_TELEGRAM_MIN_INTERVAL_MS: u64 = 800;
const DEFAULT_IDEMPOTENCY_WINDOW_SECONDS: i64 = 86_400;

#[derive(Debug, Clone)]
pub struct ChannelRepository {
    channels_path: PathBuf,
    events_dir: PathBuf,
    cache_dir: PathBuf,
    rate_dir: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CacheEnvelope {
    cached_at: DateTime<Utc>,
    value: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ChannelRateState {
    last_sent_at: DateTime<Utc>,
}

impl ChannelRepository {
    pub fn new(channels_path: PathBuf, events_dir: PathBuf) -> Self {
        let cache_parent = events_dir
            .parent()
            .map(ToOwned::to_owned)
            .unwrap_or_else(|| events_dir.clone());
        let cache_dir = cache_parent.join("channel-cache");
        let rate_dir = cache_parent.join("channel-rate");
        Self {
            channels_path,
            events_dir,
            cache_dir,
            rate_dir,
        }
    }

    pub fn list(&self) -> Result<Vec<ChannelListItem>> {
        let file = self.load_channels_file()?;
        let items = file
            .channels
            .into_iter()
            .map(|channel| {
                let target_masked = mask_optional_target(
                    &channel.kind,
                    channel.target.as_deref(),
                    channel.endpoint.as_deref(),
                );
                ChannelListItem {
                    id: channel.id,
                    name: channel.name,
                    kind: channel.kind,
                    endpoint_masked: mask_optional_endpoint(channel.endpoint.as_deref()),
                    target_masked,
                    created_at: channel.created_at,
                    last_login_at: channel.last_login_at,
                    last_send_at: channel.last_send_at,
                    last_error: channel.last_error,
                }
            })
            .collect::<Vec<_>>();
        Ok(items)
    }

    pub fn status(&self) -> Result<ChannelStatus> {
        let channels = self.list()?;
        let mut kinds = BTreeMap::new();
        for channel in &channels {
            *kinds.entry(channel.kind.clone()).or_insert(0usize) += 1;
        }
        Ok(ChannelStatus {
            total_channels: channels.len(),
            healthy_channels: channels.iter().filter(|c| c.last_error.is_none()).count(),
            channels_with_errors: channels.iter().filter(|c| c.last_error.is_some()).count(),
            kinds,
            last_send_at: channels.iter().filter_map(|c| c.last_send_at).max(),
        })
    }

    pub fn add(&self, input: AddChannelInput) -> Result<ChannelEntry> {
        let AddChannelInput {
            name,
            kind: raw_kind,
            endpoint,
            target,
            token_env,
        } = input;
        let mut file = self.load_channels_file()?;
        let name = name.trim();
        if name.is_empty() {
            return Err(MosaicError::Validation(
                "channel name cannot be empty".to_string(),
            ));
        }
        let kind = normalize_kind(&raw_kind)?;
        let endpoint = normalize_optional(endpoint);
        let target = normalize_optional(target);
        validate_channel_for_kind(&kind, endpoint.as_deref(), target.as_deref())?;
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
        let token_env = normalize_optional(token_env)
            .or_else(|| providers::default_token_env_for_kind(&kind).map(str::to_string));
        let channel = ChannelEntry {
            id: format!("ch_{}", uuid::Uuid::new_v4()),
            name: name.to_string(),
            kind,
            endpoint,
            target,
            auth: ChannelAuthConfig { token_env },
            created_at: now,
            last_login_at: None,
            last_send_at: None,
            last_error: None,
        };
        file.channels.push(channel.clone());
        self.save_channels_file(&file)?;
        Ok(channel)
    }

    pub fn login(&self, channel_id: &str, token_env: Option<&str>) -> Result<ChannelLoginResult> {
        let mut file = self.load_channels_file()?;
        let channel = file
            .channels
            .iter_mut()
            .find(|entry| entry.id == channel_id)
            .ok_or_else(|| MosaicError::Config(format!("channel '{channel_id}' not found")))?;
        let token_env = token_env
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned)
            .or_else(|| channel.auth.token_env.clone())
            .or_else(|| providers::default_token_env_for_kind(&channel.kind).map(str::to_string))
            .unwrap_or_else(|| DEFAULT_CHANNEL_TOKEN_ENV.to_string());

        channel.last_login_at = Some(Utc::now());
        channel.auth.token_env = Some(token_env.clone());
        let token_present = std::env::var(&token_env).is_ok();
        let channel_cloned = channel.clone();
        self.save_channels_file(&file)?;

        Ok(ChannelLoginResult {
            token_env,
            token_present,
            channel: channel_cloned,
        })
    }

    pub fn logout(&self, channel_id: &str) -> Result<ChannelEntry> {
        let mut file = self.load_channels_file()?;
        let channel = file
            .channels
            .iter_mut()
            .find(|entry| entry.id == channel_id)
            .ok_or_else(|| MosaicError::Config(format!("channel '{channel_id}' not found")))?;
        channel.auth.token_env = None;
        channel.last_login_at = None;
        let snapshot = channel.clone();
        self.save_channels_file(&file)?;
        Ok(snapshot)
    }

    pub fn remove(&self, channel_id: &str) -> Result<ChannelEntry> {
        let mut file = self.load_channels_file()?;
        let idx = file
            .channels
            .iter()
            .position(|entry| entry.id == channel_id)
            .ok_or_else(|| MosaicError::Config(format!("channel '{channel_id}' not found")))?;
        let removed = file.channels.remove(idx);
        self.save_channels_file(&file)?;

        let event_path = self.events_dir.join(format!("{channel_id}.jsonl"));
        if event_path.exists() {
            let _ = std::fs::remove_file(event_path);
        }

        Ok(removed)
    }

    pub async fn send(
        &self,
        channel_id: &str,
        text: &str,
        token_env_override: Option<String>,
        probe: bool,
    ) -> Result<ChannelSendResult> {
        self.send_with_options(
            channel_id,
            text,
            token_env_override,
            probe,
            ChannelSendOptions::default(),
        )
        .await
    }

    pub async fn send_with_options(
        &self,
        channel_id: &str,
        text: &str,
        token_env_override: Option<String>,
        probe: bool,
        options: ChannelSendOptions,
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

        let token_env = token_env_override
            .or_else(|| channel.auth.token_env.clone())
            .or_else(|| providers::default_token_env_for_kind(&channel.kind).map(str::to_string));
        let token = resolve_token_value(token_env.as_deref())?;
        let send_kind = if probe { "test_probe" } else { "message" };
        let parse_mode = normalize_parse_mode(options.parse_mode, &channel.kind)?;
        let idempotency_key = normalize_optional(options.idempotency_key);
        let rendered_text = render_message_template(
            text,
            options.title.as_deref(),
            &options.blocks,
            options.metadata.as_ref(),
        );
        let text_preview = truncate_text(&rendered_text, TEXT_PREVIEW_LIMIT);
        if !probe
            && let Some(key) = idempotency_key.as_deref()
            && let Some(previous_http_status) =
                self.find_recent_successful_idempotent(&channel.id, key)?
        {
            let event = ChannelLogEntry {
                ts: Utc::now(),
                channel_id: channel.id.clone(),
                kind: send_kind.to_string(),
                delivery_status: "deduplicated".to_string(),
                attempt: 0,
                http_status: previous_http_status,
                error: None,
                text_preview,
                parse_mode: parse_mode.clone(),
                idempotency_key: Some(key.to_string()),
                rate_limited_ms: Some(0),
                deduplicated: true,
            };
            let event_path = self.append_event(&channel.id, &event)?;
            file.channels[idx].last_send_at = Some(Utc::now());
            file.channels[idx].last_error = None;
            self.save_channels_file(&file)?;

            return Ok(ChannelSendResult {
                channel_id: channel.id,
                delivered_via: channel.kind.clone(),
                kind: send_kind.to_string(),
                attempts: 0,
                http_status: previous_http_status,
                endpoint_masked: mask_optional_endpoint(channel.endpoint.as_deref()),
                target_masked: mask_optional_target(
                    &channel.kind,
                    channel.target.as_deref(),
                    channel.endpoint.as_deref(),
                ),
                parse_mode,
                idempotency_key: Some(key.to_string()),
                deduplicated: true,
                rate_limited_ms: Some(0),
                event_path: event_path.display().to_string(),
                probe,
            });
        }

        let rate_limited_ms = self.apply_telegram_rate_limit(&channel, probe).await?;
        let retry_policy = RetryPolicy::from_env();
        let delivery = providers::dispatch_send(
            &channel.kind,
            providers::ChannelDispatchRequest {
                channel_id: &channel.id,
                channel_name: &channel.name,
                endpoint: channel.endpoint.as_deref(),
                target: channel.target.as_deref(),
                text: &rendered_text,
                parse_mode: parse_mode.as_deref(),
                bearer_token: token.as_deref(),
            },
            &retry_policy,
        )
        .await?;

        let event = ChannelLogEntry {
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
            parse_mode: parse_mode.clone(),
            idempotency_key: idempotency_key.clone(),
            rate_limited_ms,
            deduplicated: false,
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
            let target_masked = mask_optional_target(
                &channel.kind,
                channel.target.as_deref(),
                channel.endpoint.as_deref(),
            );
            return Ok(ChannelSendResult {
                channel_id: channel.id,
                delivered_via: channel.kind,
                kind: send_kind.to_string(),
                attempts: delivery.attempts,
                http_status: delivery.http_status,
                endpoint_masked: delivery.endpoint_masked,
                target_masked,
                parse_mode,
                idempotency_key,
                deduplicated: false,
                rate_limited_ms,
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

    pub fn logs(&self, channel_filter: Option<&str>, tail: usize) -> Result<Vec<ChannelLogEntry>> {
        if !self.events_dir.exists() {
            return Ok(Vec::new());
        }

        let filter = channel_filter
            .map(str::trim)
            .filter(|value| !value.is_empty() && *value != "all")
            .map(ToOwned::to_owned);

        let allowed: Option<HashSet<String>> = if let Some(value) = filter {
            Some(HashSet::from([value]))
        } else {
            None
        };

        let mut events = Vec::new();
        for entry in std::fs::read_dir(&self.events_dir)? {
            let path = entry?.path();
            if path.extension().and_then(|value| value.to_str()) != Some("jsonl") {
                continue;
            }
            let raw = std::fs::read_to_string(&path)?;
            for line in raw.lines() {
                if line.trim().is_empty() {
                    continue;
                }
                let event = serde_json::from_str::<ChannelLogEntry>(line).map_err(|err| {
                    MosaicError::Validation(format!(
                        "invalid channel event {}: {err}",
                        path.display()
                    ))
                })?;
                if let Some(allowed) = &allowed
                    && !allowed.contains(&event.channel_id)
                {
                    continue;
                }
                events.push(event);
            }
        }

        events.sort_by(|lhs, rhs| lhs.ts.cmp(&rhs.ts));
        if events.len() > tail {
            let keep_from = events.len() - tail;
            events = events.split_off(keep_from);
        }
        Ok(events)
    }

    pub fn capabilities(
        &self,
        kind: Option<&str>,
        target: Option<&str>,
    ) -> Result<Vec<ChannelCapability>> {
        if kind.is_some() && target.is_some() {
            return Err(MosaicError::Validation(
                "use either --kind or --target for capabilities, not both".to_string(),
            ));
        }

        let resolved_kind = if let Some(target) = target {
            let file = self.load_channels_file()?;
            let channel = file
                .channels
                .into_iter()
                .find(|entry| entry.id == target)
                .ok_or_else(|| MosaicError::Config(format!("channel '{target}' not found")))?;
            Some(channel.kind)
        } else {
            kind.map(normalize_kind).transpose()?
        };

        let cache_key = format!(
            "capabilities:{}",
            resolved_kind
                .as_deref()
                .unwrap_or("all")
                .trim()
                .to_lowercase()
        );
        if let Some(cached) = self.read_cache::<Vec<ChannelCapability>>(&cache_key)? {
            return Ok(cached);
        }

        let capabilities = providers::capabilities_for_kind(resolved_kind.as_deref())?;
        self.write_cache(&cache_key, &capabilities)?;
        Ok(capabilities)
    }

    pub fn resolve(&self, kind: &str, query: &str) -> Result<Vec<ChannelDirectoryEntry>> {
        let kind = normalize_kind(kind)?;
        let query = query.trim().to_lowercase();
        let cache_key = format!("resolve:{kind}:{query}");
        if let Some(cached) = self.read_cache::<Vec<ChannelDirectoryEntry>>(&cache_key)? {
            return Ok(cached);
        }

        let file = self.load_channels_file()?;
        let terms = query
            .split_whitespace()
            .filter(|item| !item.is_empty())
            .collect::<Vec<_>>();

        let mut items = file
            .channels
            .into_iter()
            .filter(|entry| entry.kind == kind)
            .filter(|entry| {
                if terms.is_empty() {
                    return true;
                }
                let haystack = format!(
                    "{} {} {} {}",
                    entry.id,
                    entry.name.to_lowercase(),
                    entry
                        .endpoint
                        .as_deref()
                        .map(str::to_lowercase)
                        .unwrap_or_default(),
                    entry
                        .target
                        .as_deref()
                        .map(str::to_lowercase)
                        .unwrap_or_default()
                );
                terms.iter().all(|term| haystack.contains(term))
            })
            .map(|entry| {
                let target_masked = mask_optional_target(
                    &entry.kind,
                    entry.target.as_deref(),
                    entry.endpoint.as_deref(),
                );
                ChannelDirectoryEntry {
                    id: entry.id,
                    name: entry.name,
                    kind: entry.kind,
                    endpoint_masked: mask_optional_endpoint(entry.endpoint.as_deref()),
                    target_masked,
                    last_send_at: entry.last_send_at,
                    last_error: entry.last_error,
                }
            })
            .collect::<Vec<_>>();
        items.sort_by(|lhs, rhs| lhs.name.cmp(&rhs.name));

        self.write_cache(&cache_key, &items)?;
        Ok(items)
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
            let endpoint_valid = validate_channel_for_kind(
                &channel.kind,
                channel.endpoint.as_deref(),
                channel.target.as_deref(),
            )
            .is_ok();
            checks.push(DoctorCheck {
                name: format!("channel_{}_target", channel.id),
                ok: endpoint_valid,
                detail: if endpoint_valid {
                    format!(
                        "{} target looks valid ({})",
                        channel.kind,
                        mask_optional_target(
                            &channel.kind,
                            channel.target.as_deref(),
                            channel.endpoint.as_deref(),
                        )
                        .unwrap_or_else(|| "-".to_string())
                    )
                } else {
                    format!("{} target invalid", channel.kind)
                },
            });

            let token_env = channel.auth.token_env.clone().or_else(|| {
                providers::default_token_env_for_kind(&channel.kind).map(str::to_string)
            });
            if let Some(token_env) = token_env {
                let token_present = std::env::var(&token_env).is_ok();
                checks.push(DoctorCheck {
                    name: format!("channel_{}_token_env", channel.id),
                    ok: token_present,
                    detail: format!(
                        "{} {}",
                        token_env,
                        if token_present {
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

    fn append_event(&self, channel_id: &str, event: &ChannelLogEntry) -> Result<PathBuf> {
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

    fn find_recent_successful_idempotent(
        &self,
        channel_id: &str,
        idempotency_key: &str,
    ) -> Result<Option<Option<u16>>> {
        let path = self.events_dir.join(format!("{channel_id}.jsonl"));
        if !path.exists() {
            return Ok(None);
        }
        let window_seconds = std::env::var("MOSAIC_CHANNELS_IDEMPOTENCY_WINDOW_SECONDS")
            .ok()
            .and_then(|value| value.parse::<i64>().ok())
            .unwrap_or(DEFAULT_IDEMPOTENCY_WINDOW_SECONDS)
            .max(1);
        let now = Utc::now();
        let raw = std::fs::read_to_string(path)?;
        let mut latest_status: Option<Option<u16>> = None;
        for line in raw.lines() {
            if line.trim().is_empty() {
                continue;
            }
            let event = serde_json::from_str::<ChannelLogEntry>(line).map_err(|err| {
                MosaicError::Validation(format!("invalid channel event entry: {err}"))
            })?;
            if event.kind != "message" {
                continue;
            }
            if event.idempotency_key.as_deref() != Some(idempotency_key) {
                continue;
            }
            if event.delivery_status != "success" && event.delivery_status != "deduplicated" {
                continue;
            }
            if now - event.ts > Duration::seconds(window_seconds) {
                continue;
            }
            latest_status = Some(event.http_status);
        }
        Ok(latest_status)
    }

    async fn apply_telegram_rate_limit(
        &self,
        channel: &ChannelEntry,
        probe: bool,
    ) -> Result<Option<u64>> {
        if probe || channel.kind != "telegram_bot" {
            return Ok(None);
        }

        let interval_ms = std::env::var("MOSAIC_CHANNELS_TELEGRAM_MIN_INTERVAL_MS")
            .ok()
            .and_then(|value| value.parse::<u64>().ok())
            .unwrap_or(DEFAULT_TELEGRAM_MIN_INTERVAL_MS);
        if interval_ms == 0 {
            return Ok(Some(0));
        }

        std::fs::create_dir_all(&self.rate_dir)?;
        let path = self.rate_dir.join(format!("{}.json", channel.id));
        let mut waited_ms = 0u64;
        if path.exists() {
            let raw = std::fs::read_to_string(&path)?;
            let state = serde_json::from_str::<ChannelRateState>(&raw).map_err(|err| {
                MosaicError::Validation(format!("invalid channel rate state: {err}"))
            })?;
            let elapsed = (Utc::now() - state.last_sent_at).num_milliseconds().max(0) as u64;
            if elapsed < interval_ms {
                waited_ms = interval_ms - elapsed;
                tokio::time::sleep(std::time::Duration::from_millis(waited_ms)).await;
            }
        }

        let next = ChannelRateState {
            last_sent_at: Utc::now(),
        };
        let rendered = serde_json::to_string(&next)
            .map_err(|err| MosaicError::Validation(format!("invalid channel rate JSON: {err}")))?;
        std::fs::write(path, rendered)?;
        Ok(Some(waited_ms))
    }

    fn read_cache<T: DeserializeOwned>(&self, key: &str) -> Result<Option<T>> {
        let path = self.cache_path(key);
        if !path.exists() {
            return Ok(None);
        }
        let raw = std::fs::read_to_string(path)?;
        let envelope = serde_json::from_str::<CacheEnvelope>(&raw)
            .map_err(|err| MosaicError::Validation(format!("invalid channel cache JSON: {err}")))?;
        if Utc::now() - envelope.cached_at > Duration::seconds(CACHE_TTL_SECONDS) {
            return Ok(None);
        }
        let value = serde_json::from_value::<T>(envelope.value).map_err(|err| {
            MosaicError::Validation(format!("invalid cached channel payload: {err}"))
        })?;
        Ok(Some(value))
    }

    fn write_cache<T: Serialize>(&self, key: &str, value: &T) -> Result<()> {
        std::fs::create_dir_all(&self.cache_dir)?;
        let path = self.cache_path(key);
        let envelope = CacheEnvelope {
            cached_at: Utc::now(),
            value: serde_json::to_value(value).map_err(|err| {
                MosaicError::Validation(format!("failed to encode channel cache payload: {err}"))
            })?,
        };
        let encoded = serde_json::to_string_pretty(&envelope).map_err(|err| {
            MosaicError::Validation(format!("failed to encode channel cache JSON: {err}"))
        })?;
        std::fs::write(path, encoded)?;
        Ok(())
    }

    fn cache_path(&self, key: &str) -> PathBuf {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        key.hash(&mut hasher);
        self.cache_dir.join(format!("{:x}.json", hasher.finish()))
    }
}

fn normalize_optional(value: Option<String>) -> Option<String> {
    value.and_then(|raw| {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}

fn normalize_parse_mode(parse_mode: Option<String>, kind: &str) -> Result<Option<String>> {
    let Some(parse_mode) = normalize_optional(parse_mode) else {
        return Ok(None);
    };
    if kind != "telegram_bot" {
        return Err(MosaicError::Validation(
            "--parse-mode is only supported for telegram_bot channels".to_string(),
        ));
    }
    let normalized = match parse_mode.to_lowercase().as_str() {
        "markdown" => "Markdown",
        "markdownv2" | "markdown_v2" | "mdv2" => "MarkdownV2",
        "html" => "HTML",
        _ => {
            return Err(MosaicError::Validation(format!(
                "unsupported parse mode '{}', expected markdown|markdown_v2|html",
                parse_mode
            )));
        }
    };
    Ok(Some(normalized.to_string()))
}

fn render_message_template(
    text: &str,
    title: Option<&str>,
    blocks: &[String],
    metadata: Option<&Value>,
) -> String {
    let mut segments = Vec::new();
    if let Some(title) = title.map(str::trim).filter(|value| !value.is_empty()) {
        segments.push(title.to_string());
    }
    for block in blocks {
        let block = block.trim();
        if !block.is_empty() {
            segments.push(block.to_string());
        }
    }
    if let Some(metadata) = metadata
        && let Some(object) = metadata.as_object()
        && !object.is_empty()
    {
        let mut keys = object.keys().cloned().collect::<Vec<_>>();
        keys.sort();
        let lines = keys
            .into_iter()
            .map(|key| {
                let value = object
                    .get(&key)
                    .map(format_metadata_value)
                    .unwrap_or_else(|| "null".to_string());
                format!("{key}: {value}")
            })
            .collect::<Vec<_>>()
            .join("\n");
        if !lines.is_empty() {
            segments.push(lines);
        }
    }
    segments.push(text.trim().to_string());
    segments.join("\n\n")
}

fn format_metadata_value(value: &Value) -> String {
    match value {
        Value::Null => "null".to_string(),
        Value::Bool(v) => v.to_string(),
        Value::Number(v) => v.to_string(),
        Value::String(v) => v.clone(),
        Value::Array(_) | Value::Object(_) => value.to_string(),
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
                target: None,
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

    #[test]
    fn capabilities_and_resolve_work() {
        let temp = tempdir().expect("tempdir");
        let repo = ChannelRepository::new(
            channels_file_path(temp.path()),
            channels_events_dir(temp.path()),
        );
        let _ = repo
            .add(AddChannelInput {
                name: "alerts".to_string(),
                kind: "slack".to_string(),
                endpoint: Some("mock-http://200".to_string()),
                target: None,
                token_env: None,
            })
            .expect("add");

        let capabilities = repo
            .capabilities(Some("slack"), None)
            .expect("capabilities");
        assert_eq!(capabilities.len(), 1);
        assert_eq!(capabilities[0].kind, "slack_webhook");

        let resolved = repo.resolve("slack_webhook", "alert").expect("resolve");
        assert_eq!(resolved.len(), 1);
        assert_eq!(resolved[0].name, "alerts");
    }
}
