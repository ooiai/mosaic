use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

pub(crate) const TEXT_PREVIEW_LIMIT: usize = 120;

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
pub(crate) struct ChannelEvent {
    pub ts: DateTime<Utc>,
    pub channel_id: String,
    pub kind: String,
    pub delivery_status: String,
    pub attempt: usize,
    pub http_status: Option<u16>,
    pub error: Option<String>,
    pub text_preview: String,
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
