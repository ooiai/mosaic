use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

pub(crate) const TEXT_PREVIEW_LIMIT: usize = 120;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelAuthConfig {
    pub token_env: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct ChannelTemplateDefaults {
    pub parse_mode: Option<String>,
    pub title: Option<String>,
    #[serde(default)]
    pub blocks: Vec<String>,
    pub metadata: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelEntry {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub endpoint: Option<String>,
    pub target: Option<String>,
    pub auth: ChannelAuthConfig,
    #[serde(default)]
    pub template_defaults: Option<ChannelTemplateDefaults>,
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
    pub target_masked: Option<String>,
    pub has_template_defaults: bool,
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
    pub target: Option<String>,
    pub token_env: Option<String>,
    pub template_defaults: ChannelTemplateDefaults,
}

#[derive(Debug, Clone, Default)]
pub struct UpdateChannelInput {
    pub name: Option<String>,
    pub endpoint: Option<String>,
    pub target: Option<String>,
    pub token_env: Option<String>,
    pub clear_token_env: bool,
    pub template_defaults: Option<ChannelTemplateDefaults>,
    pub clear_template_defaults: bool,
}

#[derive(Debug, Clone, Default)]
pub struct ChannelSendOptions {
    pub parse_mode: Option<String>,
    pub title: Option<String>,
    pub blocks: Vec<String>,
    pub idempotency_key: Option<String>,
    pub metadata: Option<Value>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ChannelSendResult {
    pub channel_id: String,
    pub delivered_via: String,
    pub kind: String,
    pub attempts: usize,
    pub http_status: Option<u16>,
    pub endpoint_masked: Option<String>,
    pub target_masked: Option<String>,
    pub parse_mode: Option<String>,
    pub idempotency_key: Option<String>,
    pub deduplicated: bool,
    pub rate_limited_ms: Option<u64>,
    pub event_path: String,
    pub probe: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct ChannelStatus {
    pub total_channels: usize,
    pub healthy_channels: usize,
    pub channels_with_errors: usize,
    pub kinds: BTreeMap<String, usize>,
    pub last_send_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelLogEntry {
    pub ts: DateTime<Utc>,
    pub channel_id: String,
    pub kind: String,
    pub delivery_status: String,
    pub attempt: usize,
    pub http_status: Option<u16>,
    pub error: Option<String>,
    pub text_preview: String,
    pub parse_mode: Option<String>,
    pub idempotency_key: Option<String>,
    pub rate_limited_ms: Option<u64>,
    #[serde(default)]
    pub deduplicated: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelCapability {
    pub kind: String,
    pub aliases: Vec<String>,
    pub supports_endpoint: bool,
    pub supports_token_env: bool,
    pub supports_test_probe: bool,
    pub supports_bearer_token: bool,
    #[serde(default)]
    pub supports_parse_mode: bool,
    #[serde(default)]
    pub supports_message_template: bool,
    #[serde(default)]
    pub supports_idempotency_key: bool,
    #[serde(default)]
    pub supports_rate_limit_report: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelDirectoryEntry {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub endpoint_masked: Option<String>,
    pub target_masked: Option<String>,
    pub last_send_at: Option<DateTime<Utc>>,
    pub last_error: Option<String>,
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
pub struct ChannelImportSummary {
    pub total: usize,
    pub imported: usize,
    pub updated: usize,
    pub skipped: usize,
    pub replace: bool,
    pub strict: bool,
    pub dry_run: bool,
}

#[derive(Debug, Clone)]
pub struct RotateTokenEnvInput {
    pub channel_id: Option<String>,
    pub all: bool,
    pub kind: Option<String>,
    pub from_token_env: Option<String>,
    pub to_token_env: String,
    pub dry_run: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct ChannelTokenRotationItem {
    pub channel_id: String,
    pub name: String,
    pub kind: String,
    pub previous_token_env: Option<String>,
    pub next_token_env: Option<String>,
    pub status: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ChannelTokenRotationSummary {
    pub total: usize,
    pub updated: usize,
    pub skipped_already_set: usize,
    pub skipped_unsupported: usize,
    pub skipped_from_mismatch: usize,
    pub dry_run: bool,
    pub from_token_env: Option<String>,
    pub to_token_env: String,
    pub items: Vec<ChannelTokenRotationItem>,
}

pub(crate) fn truncate_text(text: &str, max_chars: usize) -> String {
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
