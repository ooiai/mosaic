use chrono::{DateTime, Utc};
use serde::Deserialize;
use serde_json::{Value, json};

use mosaic_core::error::{MosaicError, Result};

use crate::providers;
use crate::types::{ChannelAuthConfig, ChannelEntry, ChannelListItem, ChannelsFile};

pub const CHANNELS_SCHEMA_VERSION: u32 = 2;
pub const DEFAULT_CHANNEL_TOKEN_ENV: &str = "CHANNEL_TOKEN";

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

pub(crate) fn parse_channels_value(value: Value) -> Result<(ChannelsFile, bool)> {
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

pub(crate) fn normalize_channels(channels: &mut [ChannelEntry]) -> Result<()> {
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

pub(crate) fn normalize_kind(kind: &str) -> Result<String> {
    providers::resolve_kind(kind).ok_or_else(|| {
        MosaicError::Validation(format!(
            "unsupported channel kind '{}', expected {}",
            kind.trim(),
            providers::supported_kinds_hint()
        ))
    })
}

pub(crate) fn validate_endpoint_for_kind(kind: &str, endpoint: Option<&str>) -> Result<()> {
    providers::validate_endpoint_for_kind(kind, endpoint)
}

pub(crate) fn mask_optional_endpoint(endpoint: Option<&str>) -> Option<String> {
    endpoint.map(mask_endpoint)
}

pub(crate) fn mask_endpoint(endpoint: &str) -> String {
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

#[cfg(test)]
mod tests {
    use serde_json::json;

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

    #[test]
    fn endpoint_validation_for_discord_webhook() {
        assert!(
            validate_endpoint_for_kind(
                "discord_webhook",
                Some("https://discord.com/api/webhooks/1/abc")
            )
            .is_ok()
        );
        assert!(
            validate_endpoint_for_kind(
                "discord",
                Some("https://canary.discord.com/api/webhooks/1/abc")
            )
            .is_ok()
        );
        assert!(
            validate_endpoint_for_kind("discord_webhook", Some("https://example.com/abc")).is_err()
        );
    }
}
