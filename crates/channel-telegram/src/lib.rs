use std::time::Duration;

use anyhow::{Result, anyhow, bail};
use chrono::Utc;
use mosaic_control_protocol::{AttachmentKind, ChannelAttachment, ChannelInboundMessage};
use mosaic_inspect::{
    ChannelDeliveryResult, ChannelDeliveryStatus, ChannelDeliveryTrace, ChannelOutboundMessage,
};
use reqwest::StatusCode;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use uuid::Uuid;

const DEFAULT_TIMEOUT_SECS: u64 = 10;
const DEFAULT_MAX_RETRIES: usize = 2;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TelegramUpdate {
    pub update_id: i64,
    pub message: Option<TelegramMessage>,
    pub edited_message: Option<TelegramMessage>,
    pub channel_post: Option<TelegramMessage>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TelegramMessage {
    pub message_id: i64,
    pub text: Option<String>,
    pub caption: Option<String>,
    #[serde(default)]
    pub photo: Vec<TelegramPhotoSize>,
    pub document: Option<TelegramDocument>,
    pub message_thread_id: Option<i64>,
    pub chat: TelegramChat,
    pub from: Option<TelegramUser>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TelegramPhotoSize {
    pub file_id: String,
    pub width: i64,
    pub height: i64,
    pub file_size: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TelegramDocument {
    pub file_id: String,
    pub file_name: Option<String>,
    pub mime_type: Option<String>,
    pub file_size: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TelegramFile {
    pub file_id: String,
    pub file_unique_id: Option<String>,
    pub file_size: Option<u64>,
    pub file_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TelegramChat {
    pub id: i64,
    #[serde(rename = "type")]
    pub chat_type: String,
    pub title: Option<String>,
    pub username: Option<String>,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TelegramUser {
    pub id: i64,
    pub username: Option<String>,
    pub first_name: String,
    pub last_name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TelegramReplyTarget {
    chat_id: i64,
    thread_id: Option<i64>,
    reply_to_message_id: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct TelegramOutboundClient {
    client: reqwest::Client,
    base_url: String,
    bot_token: String,
    max_retries: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TelegramWebhookConfig {
    pub url: String,
    pub secret_token: Option<String>,
    #[serde(default)]
    pub allowed_updates: Vec<String>,
    #[serde(default)]
    pub drop_pending_updates: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TelegramWebhookInfo {
    pub url: String,
    pub has_custom_certificate: bool,
    pub pending_update_count: i64,
    pub last_error_date: Option<i64>,
    pub last_error_message: Option<String>,
    pub last_synchronization_error_date: Option<i64>,
    pub max_connections: Option<i64>,
    pub ip_address: Option<String>,
    #[serde(default)]
    pub allowed_updates: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct TelegramBotContext {
    pub name: Option<String>,
    pub route: Option<String>,
    pub default_profile: Option<String>,
    pub bot_token_env: Option<String>,
    pub bot_secret_env: Option<String>,
}

impl TelegramOutboundClient {
    pub fn from_env() -> Result<Option<Self>> {
        Self::from_env_names(None, None)
    }

    pub fn from_env_names(
        bot_token_env: Option<&str>,
        base_url_env: Option<&str>,
    ) -> Result<Option<Self>> {
        let bot_token = bot_token_env
            .and_then(|name| {
                std::env::var(name)
                    .ok()
                    .filter(|value| !value.trim().is_empty())
            })
            .or_else(|| {
                std::env::var("MOSAIC_TELEGRAM_BOT_TOKEN")
                    .ok()
                    .filter(|value| !value.trim().is_empty())
            })
            .or_else(|| {
                std::env::var("TELEGRAM_BOT_TOKEN")
                    .ok()
                    .filter(|value| !value.trim().is_empty())
            });
        let Some(bot_token) = bot_token else {
            return Ok(None);
        };
        let base_url = base_url_env
            .and_then(|name| {
                std::env::var(name)
                    .ok()
                    .filter(|value| !value.trim().is_empty())
            })
            .or_else(|| {
                std::env::var("MOSAIC_TELEGRAM_API_BASE_URL")
                    .ok()
                    .filter(|value| !value.trim().is_empty())
            })
            .or_else(|| {
                std::env::var("TELEGRAM_API_BASE_URL")
                    .ok()
                    .filter(|value| !value.trim().is_empty())
            })
            .unwrap_or_else(|| "https://api.telegram.org".to_owned());
        Self::new_with_settings(
            bot_token,
            base_url,
            Duration::from_secs(DEFAULT_TIMEOUT_SECS),
            DEFAULT_MAX_RETRIES,
        )
        .map(Some)
    }

    pub fn new(bot_token: impl Into<String>, base_url: impl Into<String>) -> Result<Self> {
        Self::new_with_settings(
            bot_token,
            base_url,
            Duration::from_secs(DEFAULT_TIMEOUT_SECS),
            DEFAULT_MAX_RETRIES,
        )
    }

    pub fn new_with_settings(
        bot_token: impl Into<String>,
        base_url: impl Into<String>,
        timeout: Duration,
        max_retries: usize,
    ) -> Result<Self> {
        let client = reqwest::Client::builder().timeout(timeout).build()?;
        Ok(Self {
            client,
            base_url: base_url.into().trim_end_matches('/').to_owned(),
            bot_token: bot_token.into(),
            max_retries,
        })
    }

    pub async fn send_message(&self, message: ChannelOutboundMessage) -> ChannelDeliveryTrace {
        let delivery_id = Uuid::new_v4().to_string();
        let mut retry_count = 0usize;

        loop {
            match self.send_once(&message).await {
                Ok(provider_message_id) => {
                    return ChannelDeliveryTrace {
                        message,
                        result: ChannelDeliveryResult {
                            delivery_id,
                            status: ChannelDeliveryStatus::Delivered,
                            provider_message_id: Some(provider_message_id),
                            retry_count,
                            retryable: false,
                            error_kind: None,
                            error: None,
                            delivered_at: Some(Utc::now()),
                        },
                    };
                }
                Err(error) => {
                    if error.retryable && retry_count < self.max_retries {
                        retry_count += 1;
                        tokio::time::sleep(Duration::from_millis(200)).await;
                        continue;
                    }

                    return ChannelDeliveryTrace {
                        message,
                        result: ChannelDeliveryResult {
                            delivery_id,
                            status: ChannelDeliveryStatus::Failed,
                            provider_message_id: None,
                            retry_count,
                            retryable: error.retryable,
                            error_kind: Some(error.kind),
                            error: Some(error.message),
                            delivered_at: None,
                        },
                    };
                }
            }
        }
    }

    pub async fn set_webhook(&self, config: TelegramWebhookConfig) -> Result<TelegramWebhookInfo> {
        let response = self
            .client
            .post(format!(
                "{}/bot{}/setWebhook",
                self.base_url, self.bot_token
            ))
            .json(&TelegramSetWebhookRequest {
                url: config.url,
                secret_token: config.secret_token,
                allowed_updates: if config.allowed_updates.is_empty() {
                    None
                } else {
                    Some(config.allowed_updates)
                },
                drop_pending_updates: config.drop_pending_updates,
            })
            .send()
            .await?;
        self.expect_api_result::<bool>(response).await?;
        self.get_webhook_info().await
    }

    pub async fn get_webhook_info(&self) -> Result<TelegramWebhookInfo> {
        let response = self
            .client
            .get(format!(
                "{}/bot{}/getWebhookInfo",
                self.base_url, self.bot_token
            ))
            .send()
            .await?;
        self.expect_api_result::<TelegramWebhookInfo>(response)
            .await
    }

    pub async fn delete_webhook(&self, drop_pending_updates: bool) -> Result<()> {
        let response = self
            .client
            .post(format!(
                "{}/bot{}/deleteWebhook",
                self.base_url, self.bot_token
            ))
            .json(&TelegramDeleteWebhookRequest {
                drop_pending_updates,
            })
            .send()
            .await?;
        self.expect_api_result::<bool>(response).await?;
        Ok(())
    }

    pub async fn get_file(&self, file_id: &str) -> Result<TelegramFile> {
        let response = self
            .client
            .get(format!("{}/bot{}/getFile", self.base_url, self.bot_token))
            .query(&[("file_id", file_id)])
            .send()
            .await?;
        self.expect_api_result::<TelegramFile>(response).await
    }

    pub async fn download_file_bytes(&self, file_path: &str) -> Result<Vec<u8>> {
        let response = self
            .client
            .get(format!(
                "{}/file/bot{}/{}",
                self.base_url,
                self.bot_token,
                file_path.trim_start_matches('/')
            ))
            .send()
            .await?;
        let status = response.status();
        if !status.is_success() {
            bail!(
                "telegram file download failed with status {}",
                status.as_u16()
            );
        }
        Ok(response.bytes().await?.to_vec())
    }

    pub async fn send_test_message(
        &self,
        chat_id: i64,
        text: impl Into<String>,
        thread_id: Option<i64>,
        reply_to_message_id: Option<i64>,
        bot: Option<&TelegramBotContext>,
    ) -> ChannelDeliveryTrace {
        self.send_message(ChannelOutboundMessage {
            channel: "telegram".to_owned(),
            adapter: "telegram_cli".to_owned(),
            bot_name: bot.and_then(|bot| bot.name.clone()),
            bot_route: bot.and_then(|bot| bot.route.clone()),
            bot_profile: bot.and_then(|bot| bot.default_profile.clone()),
            bot_token_env: bot.and_then(|bot| bot.bot_token_env.clone()),
            conversation_id: conversation_id_for_chat(chat_id, bot),
            reply_target: reply_target_for_target(chat_id, thread_id, reply_to_message_id, bot),
            text: text.into(),
            idempotency_key: Uuid::new_v4().to_string(),
            correlation_id: "telegram-cli-test".to_owned(),
            gateway_run_id: "telegram-cli-test".to_owned(),
            session_id: session_hint_for_chat(chat_id, thread_id, bot, true),
        })
        .await
    }

    async fn send_once(
        &self,
        message: &ChannelOutboundMessage,
    ) -> std::result::Result<String, DeliveryError> {
        let reply_target = parse_reply_target(&message.reply_target)?;
        let response = self
            .client
            .post(format!(
                "{}/bot{}/sendMessage",
                self.base_url, self.bot_token
            ))
            .json(&TelegramSendMessageRequest {
                chat_id: reply_target.chat_id,
                text: message.text.clone(),
                message_thread_id: reply_target.thread_id,
                reply_to_message_id: reply_target.reply_to_message_id,
            })
            .send()
            .await
            .map_err(classify_reqwest_error)?;
        let status = response.status();
        let body = response.text().await.map_err(classify_reqwest_error)?;

        let payload: TelegramApiResponse<TelegramApiMessage> = serde_json::from_str(&body)
            .map_err(|err| {
                DeliveryError::protocol(format!("telegram returned invalid JSON: {err}"))
            })?;

        if status.is_success() && payload.ok {
            let provider_message_id = payload
                .result
                .and_then(|message| message.message_id)
                .ok_or_else(|| {
                    DeliveryError::protocol(
                        "telegram success response did not include result.message_id".to_owned(),
                    )
                })?;
            return Ok(provider_message_id.to_string());
        }

        Err(classify_telegram_error(status, payload.description))
    }

    async fn expect_api_result<T: DeserializeOwned>(
        &self,
        response: reqwest::Response,
    ) -> Result<T> {
        let status = response.status();
        let body = response.text().await?;
        let payload: TelegramApiResponse<T> = serde_json::from_str(&body)
            .map_err(|err| anyhow!("telegram returned invalid JSON: {err}"))?;

        if status.is_success() && payload.ok {
            return payload
                .result
                .ok_or_else(|| anyhow!("telegram success response did not include result"));
        }

        let error = classify_telegram_error(status, payload.description);
        Err(anyhow!("telegram {} error: {}", error.kind, error.message))
    }
}

pub fn normalize_update(update: TelegramUpdate) -> Result<ChannelInboundMessage> {
    normalize_update_with_context(update, None)
}

pub fn normalize_update_with_context(
    update: TelegramUpdate,
    bot: Option<&TelegramBotContext>,
) -> Result<ChannelInboundMessage> {
    let TelegramUpdate {
        update_id,
        message,
        edited_message,
        channel_post,
    } = update;
    let message = message
        .or(edited_message)
        .or(channel_post)
        .ok_or_else(|| anyhow!("telegram update does not contain a supported message payload"))?;

    let input = message
        .text
        .clone()
        .or(message.caption.clone())
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
        .unwrap_or_default();
    let attachments = attachments_from_message(&message);
    if input.is_empty() && attachments.is_empty() {
        return Err(anyhow!(
            "telegram update does not contain text, caption, photo, or document content"
        ));
    }

    let thread_id = message.message_thread_id.map(|value| value.to_string());
    let session_hint =
        session_hint_for_chat(message.chat.id, message.message_thread_id, bot, false);
    let display_name = message
        .from
        .as_ref()
        .map(display_name_for_user)
        .or_else(|| display_name_for_chat(&message.chat));
    let actor_id = message
        .from
        .as_ref()
        .map(|user| user.id.to_string())
        .or_else(|| Some(message.chat.id.to_string()));
    let thread_title = thread_id.as_ref().map(|_| {
        message
            .chat
            .title
            .clone()
            .unwrap_or_else(|| "telegram-thread".to_owned())
    });

    Ok(ChannelInboundMessage {
        channel: "telegram".to_owned(),
        adapter: "telegram_webhook".to_owned(),
        bot_name: bot.and_then(|bot| bot.name.clone()),
        bot_route: bot.and_then(|bot| bot.route.clone()),
        bot_profile: bot.and_then(|bot| bot.default_profile.clone()),
        bot_token_env: bot.and_then(|bot| bot.bot_token_env.clone()),
        actor_id,
        display_name,
        conversation_id: conversation_id_for_chat(message.chat.id, bot),
        thread_id: thread_id.clone(),
        thread_title,
        reply_target: reply_target_for_message(
            message.chat.id,
            message.message_thread_id,
            message.message_id,
            bot,
        ),
        message_id: message.message_id.to_string(),
        text: input,
        attachments,
        profile_hint: bot.and_then(|bot| bot.default_profile.clone()),
        session_hint: Some(session_hint),
        received_at: Utc::now(),
        raw_event_id: update_id.to_string(),
    })
}

fn attachments_from_message(message: &TelegramMessage) -> Vec<ChannelAttachment> {
    let mut attachments = Vec::new();
    let caption = message
        .caption
        .clone()
        .filter(|value| !value.trim().is_empty());

    if let Some(photo) = message
        .photo
        .iter()
        .max_by_key(|size| size.file_size.unwrap_or((size.width * size.height) as u64))
    {
        attachments.push(ChannelAttachment {
            id: format!("telegram-photo-{}", photo.file_id),
            kind: AttachmentKind::Image,
            filename: Some(format!("telegram-photo-{}.jpg", message.message_id)),
            mime_type: Some("image/jpeg".to_owned()),
            size_bytes: photo.file_size,
            source_ref: Some(format!("telegram:file_id:{}", photo.file_id)),
            remote_url: None,
            local_cache_path: None,
            caption: caption.clone(),
        });
    }

    if let Some(document) = message.document.as_ref() {
        attachments.push(ChannelAttachment {
            id: format!("telegram-document-{}", document.file_id),
            kind: AttachmentKind::Document,
            filename: document.file_name.clone(),
            mime_type: document.mime_type.clone(),
            size_bytes: document.file_size,
            source_ref: Some(format!("telegram:file_id:{}", document.file_id)),
            remote_url: None,
            local_cache_path: None,
            caption,
        });
    }

    attachments
}

fn display_name_for_user(user: &TelegramUser) -> String {
    let mut parts = vec![user.first_name.clone()];
    if let Some(last_name) = user.last_name.as_deref() {
        if !last_name.trim().is_empty() {
            parts.push(last_name.trim().to_owned());
        }
    }

    let joined = parts.join(" ");
    if joined.trim().is_empty() {
        user.username.clone().unwrap_or_else(|| user.id.to_string())
    } else {
        joined
    }
}

fn display_name_for_chat(chat: &TelegramChat) -> Option<String> {
    chat.title
        .clone()
        .or_else(|| chat.username.clone())
        .or_else(|| chat.first_name.clone())
}

fn conversation_id_for_chat(chat_id: i64, bot: Option<&TelegramBotContext>) -> String {
    match bot.and_then(|bot| bot.name.as_deref()) {
        Some(bot_name) => format!("telegram:bot:{bot_name}:chat:{chat_id}"),
        None => format!("telegram:chat:{chat_id}"),
    }
}

fn session_hint_for_chat(
    chat_id: i64,
    thread_id: Option<i64>,
    bot: Option<&TelegramBotContext>,
    cli_prefix: bool,
) -> String {
    let prefix = if cli_prefix {
        "telegram-cli"
    } else {
        "telegram"
    };
    match (bot.and_then(|bot| bot.name.as_deref()), thread_id) {
        (Some(bot_name), Some(thread_id)) => format!("{prefix}-{bot_name}-{chat_id}-{thread_id}"),
        (Some(bot_name), None) => format!("{prefix}-{bot_name}-{chat_id}"),
        (None, Some(thread_id)) => format!("{prefix}-{chat_id}-{thread_id}"),
        (None, None) => format!("{prefix}-{chat_id}"),
    }
}

fn reply_target_for_message(
    chat_id: i64,
    thread_id: Option<i64>,
    message_id: i64,
    bot: Option<&TelegramBotContext>,
) -> String {
    match thread_id {
        Some(thread_id) => {
            format!(
                "{}:thread:{thread_id}:message:{message_id}",
                conversation_id_for_chat(chat_id, bot)
            )
        }
        None => format!(
            "{}:message:{message_id}",
            conversation_id_for_chat(chat_id, bot)
        ),
    }
}

fn reply_target_for_target(
    chat_id: i64,
    thread_id: Option<i64>,
    reply_to_message_id: Option<i64>,
    bot: Option<&TelegramBotContext>,
) -> String {
    match (thread_id, reply_to_message_id) {
        (Some(thread_id), Some(message_id)) => {
            format!(
                "{}:thread:{thread_id}:message:{message_id}",
                conversation_id_for_chat(chat_id, bot)
            )
        }
        (Some(thread_id), None) => {
            format!(
                "{}:thread:{thread_id}",
                conversation_id_for_chat(chat_id, bot)
            )
        }
        (None, Some(message_id)) => {
            format!(
                "{}:message:{message_id}",
                conversation_id_for_chat(chat_id, bot)
            )
        }
        (None, None) => conversation_id_for_chat(chat_id, bot),
    }
}

fn parse_reply_target(value: &str) -> std::result::Result<TelegramReplyTarget, DeliveryError> {
    let parts = value.split(':').collect::<Vec<_>>();
    if parts.len() < 3 || parts[0] != "telegram" {
        return Err(DeliveryError::protocol(format!(
            "unsupported telegram reply target: {value}"
        )));
    }

    let (chat_index, mut index) = match parts.get(1) {
        Some(&"chat") => (2usize, 3usize),
        Some(&"bot") if parts.get(3) == Some(&"chat") => (4usize, 5usize),
        _ => {
            return Err(DeliveryError::protocol(format!(
                "unsupported telegram reply target: {value}"
            )));
        }
    };

    let chat_id = parts[chat_index].parse::<i64>().map_err(|_| {
        DeliveryError::protocol(format!("invalid telegram chat id: {}", parts[chat_index]))
    })?;

    let mut thread_id = None;
    let mut reply_to_message_id = None;
    while index + 1 < parts.len() {
        match parts[index] {
            "thread" => {
                thread_id = Some(parts[index + 1].parse::<i64>().map_err(|_| {
                    DeliveryError::protocol(format!(
                        "invalid telegram thread id: {}",
                        parts[index + 1]
                    ))
                })?);
            }
            "message" => {
                reply_to_message_id = Some(parts[index + 1].parse::<i64>().map_err(|_| {
                    DeliveryError::protocol(format!(
                        "invalid telegram message id: {}",
                        parts[index + 1]
                    ))
                })?);
            }
            _ => {
                return Err(DeliveryError::protocol(format!(
                    "unsupported telegram reply target segment: {}",
                    parts[index]
                )));
            }
        }
        index += 2;
    }

    Ok(TelegramReplyTarget {
        chat_id,
        thread_id,
        reply_to_message_id,
    })
}

fn classify_reqwest_error(error: reqwest::Error) -> DeliveryError {
    if error.is_timeout() || error.is_connect() {
        DeliveryError::network(error.to_string(), true)
    } else {
        DeliveryError::network(error.to_string(), false)
    }
}

fn classify_telegram_error(status: StatusCode, description: Option<String>) -> DeliveryError {
    let message = description
        .unwrap_or_else(|| format!("telegram request failed with status {}", status.as_u16()));
    match status {
        StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => DeliveryError::auth(message),
        StatusCode::TOO_MANY_REQUESTS => DeliveryError::rate_limit(message),
        StatusCode::BAD_REQUEST | StatusCode::NOT_FOUND | StatusCode::CONFLICT => {
            DeliveryError::protocol(message)
        }
        status if status.is_server_error() => DeliveryError::network(message, true),
        _ => DeliveryError::network(message, false),
    }
}

#[derive(Debug, Clone)]
struct DeliveryError {
    kind: String,
    message: String,
    retryable: bool,
}

impl DeliveryError {
    fn auth(message: String) -> Self {
        Self {
            kind: "auth".to_owned(),
            message,
            retryable: false,
        }
    }

    fn network(message: String, retryable: bool) -> Self {
        Self {
            kind: "network".to_owned(),
            message,
            retryable,
        }
    }

    fn rate_limit(message: String) -> Self {
        Self {
            kind: "rate_limit".to_owned(),
            message,
            retryable: true,
        }
    }

    fn protocol(message: String) -> Self {
        Self {
            kind: "protocol".to_owned(),
            message,
            retryable: false,
        }
    }
}

#[derive(Debug, Serialize)]
struct TelegramSendMessageRequest {
    chat_id: i64,
    text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    message_thread_id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    reply_to_message_id: Option<i64>,
}

#[derive(Debug, Serialize)]
struct TelegramSetWebhookRequest {
    url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    secret_token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    allowed_updates: Option<Vec<String>>,
    #[serde(default)]
    drop_pending_updates: bool,
}

#[derive(Debug, Serialize)]
struct TelegramDeleteWebhookRequest {
    #[serde(default)]
    drop_pending_updates: bool,
}

#[derive(Debug, Deserialize)]
struct TelegramApiResponse<T> {
    ok: bool,
    result: Option<T>,
    description: Option<String>,
}

#[derive(Debug, Deserialize)]
struct TelegramApiMessage {
    message_id: Option<i64>,
}

#[cfg(test)]
mod tests {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    use axum::{
        Json, Router,
        routing::{any, post},
    };

    use super::*;

    #[test]
    fn normalizes_private_message_updates() {
        let normalized = normalize_update(TelegramUpdate {
            update_id: 1,
            message: Some(TelegramMessage {
                message_id: 10,
                text: Some("hello telegram".to_owned()),
                caption: None,
                photo: vec![],
                document: None,
                message_thread_id: None,
                chat: TelegramChat {
                    id: 42,
                    chat_type: "private".to_owned(),
                    title: None,
                    username: Some("guest42".to_owned()),
                    first_name: Some("Guest".to_owned()),
                    last_name: None,
                },
                from: Some(TelegramUser {
                    id: 7,
                    username: Some("guest42".to_owned()),
                    first_name: "Guest".to_owned(),
                    last_name: None,
                }),
            }),
            edited_message: None,
            channel_post: None,
        })
        .expect("telegram update should normalize");

        assert_eq!(normalized.channel, "telegram");
        assert_eq!(normalized.adapter, "telegram_webhook");
        assert_eq!(normalized.session_hint.as_deref(), Some("telegram-42"));
        assert_eq!(normalized.actor_id.as_deref(), Some("7"));
        assert_eq!(normalized.display_name.as_deref(), Some("Guest"));
        assert_eq!(normalized.conversation_id, "telegram:chat:42");
        assert_eq!(normalized.reply_target, "telegram:chat:42:message:10");
    }

    #[test]
    fn normalizes_topic_thread_updates() {
        let normalized = normalize_update(TelegramUpdate {
            update_id: 2,
            message: Some(TelegramMessage {
                message_id: 11,
                text: Some("thread hello".to_owned()),
                caption: None,
                photo: vec![],
                document: None,
                message_thread_id: Some(99),
                chat: TelegramChat {
                    id: -100123,
                    chat_type: "supergroup".to_owned(),
                    title: Some("Build Ops".to_owned()),
                    username: None,
                    first_name: None,
                    last_name: None,
                },
                from: Some(TelegramUser {
                    id: 8,
                    username: Some("operator8".to_owned()),
                    first_name: "Operator".to_owned(),
                    last_name: Some("Eight".to_owned()),
                }),
            }),
            edited_message: None,
            channel_post: None,
        })
        .expect("telegram thread update should normalize");

        assert_eq!(
            normalized.session_hint.as_deref(),
            Some("telegram--100123-99")
        );
        assert_eq!(normalized.thread_id.as_deref(), Some("99"));
        assert_eq!(normalized.thread_title.as_deref(), Some("Build Ops"));
        assert_eq!(
            normalized.reply_target,
            "telegram:chat:-100123:thread:99:message:11"
        );
    }

    #[test]
    fn rejects_updates_without_textual_content() {
        let error = normalize_update(TelegramUpdate {
            update_id: 3,
            message: Some(TelegramMessage {
                message_id: 12,
                text: None,
                caption: None,
                photo: vec![],
                document: None,
                message_thread_id: None,
                chat: TelegramChat {
                    id: 42,
                    chat_type: "private".to_owned(),
                    title: None,
                    username: None,
                    first_name: Some("Guest".to_owned()),
                    last_name: None,
                },
                from: None,
            }),
            edited_message: None,
            channel_post: None,
        })
        .expect_err("telegram updates without text should fail");

        assert!(
            error
                .to_string()
                .contains("text, caption, photo, or document")
        );
    }

    #[test]
    fn normalizes_photo_only_updates_into_image_attachments() {
        let normalized = normalize_update(TelegramUpdate {
            update_id: 4,
            message: Some(TelegramMessage {
                message_id: 13,
                text: None,
                caption: None,
                photo: vec![
                    TelegramPhotoSize {
                        file_id: "small-photo".to_owned(),
                        width: 64,
                        height: 64,
                        file_size: Some(128),
                    },
                    TelegramPhotoSize {
                        file_id: "large-photo".to_owned(),
                        width: 1024,
                        height: 1024,
                        file_size: Some(4096),
                    },
                ],
                document: None,
                message_thread_id: None,
                chat: TelegramChat {
                    id: 77,
                    chat_type: "private".to_owned(),
                    title: None,
                    username: Some("photo_user".to_owned()),
                    first_name: Some("Photo".to_owned()),
                    last_name: None,
                },
                from: Some(TelegramUser {
                    id: 70,
                    username: Some("photo_user".to_owned()),
                    first_name: "Photo".to_owned(),
                    last_name: None,
                }),
            }),
            edited_message: None,
            channel_post: None,
        })
        .expect("telegram photo update should normalize");

        assert!(normalized.text.is_empty());
        assert_eq!(normalized.attachments.len(), 1);
        assert_eq!(normalized.attachments[0].kind, AttachmentKind::Image);
        assert_eq!(
            normalized.attachments[0].source_ref.as_deref(),
            Some("telegram:file_id:large-photo")
        );
    }

    #[test]
    fn normalizes_document_updates_into_file_attachments() {
        let normalized = normalize_update(TelegramUpdate {
            update_id: 5,
            message: Some(TelegramMessage {
                message_id: 14,
                text: None,
                caption: Some("summarize this".to_owned()),
                photo: vec![],
                document: Some(TelegramDocument {
                    file_id: "doc-1".to_owned(),
                    file_name: Some("notes.txt".to_owned()),
                    mime_type: Some("text/plain".to_owned()),
                    file_size: Some(256),
                }),
                message_thread_id: None,
                chat: TelegramChat {
                    id: 88,
                    chat_type: "private".to_owned(),
                    title: None,
                    username: Some("doc_user".to_owned()),
                    first_name: Some("Doc".to_owned()),
                    last_name: None,
                },
                from: Some(TelegramUser {
                    id: 71,
                    username: Some("doc_user".to_owned()),
                    first_name: "Doc".to_owned(),
                    last_name: None,
                }),
            }),
            edited_message: None,
            channel_post: None,
        })
        .expect("telegram document update should normalize");

        assert_eq!(normalized.text, "summarize this");
        assert_eq!(normalized.attachments.len(), 1);
        assert_eq!(normalized.attachments[0].kind, AttachmentKind::Document);
        assert_eq!(
            normalized.attachments[0].filename.as_deref(),
            Some("notes.txt")
        );
    }

    #[tokio::test]
    async fn sends_outbound_reply_to_chat_thread_and_message_target() {
        let captured = Arc::new(tokio::sync::Mutex::new(Vec::<serde_json::Value>::new()));
        let app = Router::new().route(
            "/bottest-token/sendMessage",
            post({
                let captured = captured.clone();
                move |Json(payload): Json<serde_json::Value>| {
                    let captured = captured.clone();
                    async move {
                        captured.lock().await.push(payload);
                        Json(serde_json::json!({
                            "ok": true,
                            "result": { "message_id": 88 }
                        }))
                    }
                }
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("listener should bind");
        let addr = listener.local_addr().expect("addr should exist");
        tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });

        let client = TelegramOutboundClient::new("test-token", format!("http://{addr}")).unwrap();
        let delivery = client
            .send_message(ChannelOutboundMessage {
                channel: "telegram".to_owned(),
                adapter: "telegram_webhook".to_owned(),
                bot_name: None,
                bot_route: None,
                bot_profile: None,
                bot_token_env: None,
                conversation_id: "telegram:chat:-10042".to_owned(),
                reply_target: "telegram:chat:-10042:thread:7:message:11".to_owned(),
                text: "reply".to_owned(),
                idempotency_key: "idem-1".to_owned(),
                correlation_id: "corr-1".to_owned(),
                gateway_run_id: "gateway-run-1".to_owned(),
                session_id: "telegram--10042-7".to_owned(),
            })
            .await;

        assert_eq!(delivery.result.status, ChannelDeliveryStatus::Delivered);
        assert_eq!(delivery.result.provider_message_id.as_deref(), Some("88"));
        let captured = captured.lock().await;
        assert_eq!(captured.len(), 1);
        assert_eq!(captured[0]["chat_id"], -10042);
        assert_eq!(captured[0]["message_thread_id"], 7);
        assert_eq!(captured[0]["reply_to_message_id"], 11);
    }

    #[tokio::test]
    async fn retries_rate_limited_telegram_delivery_until_success() {
        let attempts = Arc::new(AtomicUsize::new(0));
        let app = Router::new().route(
            "/bottest-token/sendMessage",
            post({
                let attempts = attempts.clone();
                move || {
                    let attempts = attempts.clone();
                    async move {
                        let attempt = attempts.fetch_add(1, Ordering::SeqCst);
                        if attempt == 0 {
                            (
                                StatusCode::TOO_MANY_REQUESTS,
                                Json(serde_json::json!({
                                    "ok": false,
                                    "description": "Too Many Requests"
                                })),
                            )
                        } else {
                            (
                                StatusCode::OK,
                                Json(serde_json::json!({
                                    "ok": true,
                                    "result": { "message_id": 91 }
                                })),
                            )
                        }
                    }
                }
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("listener should bind");
        let addr = listener.local_addr().expect("addr should exist");
        tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });

        let client = TelegramOutboundClient::new_with_settings(
            "test-token",
            format!("http://{addr}"),
            Duration::from_secs(3),
            1,
        )
        .unwrap();
        let delivery = client
            .send_message(ChannelOutboundMessage {
                channel: "telegram".to_owned(),
                adapter: "telegram_webhook".to_owned(),
                bot_name: None,
                bot_route: None,
                bot_profile: None,
                bot_token_env: None,
                conversation_id: "telegram:chat:42".to_owned(),
                reply_target: "telegram:chat:42:message:10".to_owned(),
                text: "retry".to_owned(),
                idempotency_key: "idem-2".to_owned(),
                correlation_id: "corr-2".to_owned(),
                gateway_run_id: "gateway-run-2".to_owned(),
                session_id: "telegram-42".to_owned(),
            })
            .await;

        assert_eq!(delivery.result.status, ChannelDeliveryStatus::Delivered);
        assert_eq!(delivery.result.retry_count, 1);
        assert_eq!(attempts.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn manages_webhook_lifecycle_against_bot_api_surface() {
        let requests = Arc::new(tokio::sync::Mutex::new(
            Vec::<(String, serde_json::Value)>::new(),
        ));
        let app = Router::new()
            .route(
                "/bottest-token/setWebhook",
                post({
                    let requests = requests.clone();
                    move |Json(payload): Json<serde_json::Value>| {
                        let requests = requests.clone();
                        async move {
                            requests
                                .lock()
                                .await
                                .push(("setWebhook".to_owned(), payload));
                            Json(serde_json::json!({
                                "ok": true,
                                "result": true
                            }))
                        }
                    }
                }),
            )
            .route(
                "/bottest-token/getWebhookInfo",
                any({
                    let requests = requests.clone();
                    move || {
                        let requests = requests.clone();
                        async move {
                            requests
                                .lock()
                                .await
                                .push(("getWebhookInfo".to_owned(), serde_json::json!({})));
                            Json(serde_json::json!({
                                "ok": true,
                                "result": {
                                    "url": "https://public.example.com/ingress/telegram",
                                    "has_custom_certificate": false,
                                    "pending_update_count": 0,
                                    "allowed_updates": ["message"]
                                }
                            }))
                        }
                    }
                }),
            )
            .route(
                "/bottest-token/deleteWebhook",
                post({
                    let requests = requests.clone();
                    move |Json(payload): Json<serde_json::Value>| {
                        let requests = requests.clone();
                        async move {
                            requests
                                .lock()
                                .await
                                .push(("deleteWebhook".to_owned(), payload));
                            Json(serde_json::json!({
                                "ok": true,
                                "result": true
                            }))
                        }
                    }
                }),
            );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("listener should bind");
        let addr = listener.local_addr().expect("addr should exist");
        tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });

        let client = TelegramOutboundClient::new("test-token", format!("http://{addr}")).unwrap();
        let info = client
            .set_webhook(TelegramWebhookConfig {
                url: "https://public.example.com/ingress/telegram".to_owned(),
                secret_token: Some("secret-1".to_owned()),
                allowed_updates: vec!["message".to_owned()],
                drop_pending_updates: true,
            })
            .await
            .expect("webhook should set");
        assert_eq!(info.url, "https://public.example.com/ingress/telegram");
        assert_eq!(info.allowed_updates, vec!["message".to_owned()]);

        let info = client
            .get_webhook_info()
            .await
            .expect("webhook info should load");
        assert_eq!(info.pending_update_count, 0);

        client
            .delete_webhook(true)
            .await
            .expect("webhook should delete");

        let requests = requests.lock().await;
        assert_eq!(requests[0].0, "setWebhook");
        assert_eq!(requests[0].1["secret_token"], "secret-1");
        assert_eq!(requests[1].0, "getWebhookInfo");
        assert_eq!(requests[2].0, "getWebhookInfo");
        assert_eq!(requests[3].0, "deleteWebhook");
        assert_eq!(requests[3].1["drop_pending_updates"], true);
    }

    #[tokio::test]
    async fn test_send_can_target_chat_without_reply_to_message() {
        let captured = Arc::new(tokio::sync::Mutex::new(Vec::<serde_json::Value>::new()));
        let app = Router::new().route(
            "/bottest-token/sendMessage",
            post({
                let captured = captured.clone();
                move |Json(payload): Json<serde_json::Value>| {
                    let captured = captured.clone();
                    async move {
                        captured.lock().await.push(payload);
                        Json(serde_json::json!({
                            "ok": true,
                            "result": { "message_id": 101 }
                        }))
                    }
                }
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("listener should bind");
        let addr = listener.local_addr().expect("addr should exist");
        tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });

        let client = TelegramOutboundClient::new("test-token", format!("http://{addr}")).unwrap();
        let delivery = client
            .send_test_message(42, "hello from cli", Some(7), None, None)
            .await;

        assert_eq!(delivery.result.status, ChannelDeliveryStatus::Delivered);
        let captured = captured.lock().await;
        assert_eq!(captured[0]["chat_id"], 42);
        assert_eq!(captured[0]["message_thread_id"], 7);
        assert!(captured[0]["reply_to_message_id"].is_null());
    }

    #[test]
    fn rejects_invalid_reply_target_format() {
        let error = parse_reply_target("telegram:bad").expect_err("reply target should fail");
        assert_eq!(error.kind, "protocol");
    }

    #[test]
    fn accepts_reply_target_without_message_segment() {
        let target = parse_reply_target("telegram:chat:42:thread:7")
            .expect("reply target without message id should parse");
        assert_eq!(target.chat_id, 42);
        assert_eq!(target.thread_id, Some(7));
        assert_eq!(target.reply_to_message_id, None);
    }
}
