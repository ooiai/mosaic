use std::path::{Path, PathBuf};

use chrono::Utc;
use serde_json::Value;

use mosaic_core::error::{MosaicError, Result};

use crate::policy::RetryPolicy;
use crate::providers;
use crate::schema::{
    CHANNELS_SCHEMA_VERSION, mask_optional_endpoint, normalize_channels, normalize_kind,
    parse_channels_value, validate_endpoint_for_kind,
};
use crate::types::{
    AddChannelInput, ChannelAuthConfig, ChannelEntry, ChannelEvent, ChannelListItem,
    ChannelLoginResult, ChannelSendResult, ChannelsFile, DoctorCheck, TEXT_PREVIEW_LIMIT,
    truncate_text,
};

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
        let send_kind = if probe { "test_probe" } else { "message" };
        let text_preview = truncate_text(text, TEXT_PREVIEW_LIMIT);
        let retry_policy = RetryPolicy::from_env();
        let delivery = providers::dispatch_send(
            &channel.kind,
            providers::ChannelDispatchRequest {
                channel_id: &channel.id,
                channel_name: &channel.name,
                endpoint: channel.endpoint.as_deref(),
                text,
                bearer_token: token.as_deref(),
            },
            &retry_policy,
        )
        .await?;

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
