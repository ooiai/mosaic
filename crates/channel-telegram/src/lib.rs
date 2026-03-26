use anyhow::Result;
use mosaic_inspect::IngressTrace;
use serde::{Deserialize, Serialize};

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
    pub message_thread_id: Option<i64>,
    pub chat: TelegramChat,
    pub from: Option<TelegramUser>,
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NormalizedTelegramMessage {
    pub session_id: String,
    pub input: String,
    pub display_name: Option<String>,
    pub actor_id: Option<String>,
    pub thread_id: Option<String>,
    pub thread_title: Option<String>,
    pub reply_target: String,
}

impl NormalizedTelegramMessage {
    pub fn ingress(&self) -> IngressTrace {
        IngressTrace {
            kind: "telegram_webhook".to_owned(),
            channel: Some("telegram".to_owned()),
            source: Some("telegram-webhook".to_owned()),
            remote_addr: None,
            display_name: self.display_name.clone(),
            actor_id: self.actor_id.clone(),
            thread_id: self.thread_id.clone(),
            thread_title: self.thread_title.clone(),
            reply_target: Some(self.reply_target.clone()),
            gateway_url: None,
        }
    }
}

pub fn normalize_update(update: TelegramUpdate) -> Result<NormalizedTelegramMessage> {
    let message = update
        .message
        .or(update.edited_message)
        .or(update.channel_post)
        .ok_or_else(|| {
            anyhow::anyhow!("telegram update does not contain a supported message payload")
        })?;

    let input = message
        .text
        .or(message.caption)
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            anyhow::anyhow!("telegram update does not contain text or caption content")
        })?;

    let thread_id = message.message_thread_id.map(|value| value.to_string());
    let session_id = match thread_id.as_deref() {
        Some(thread_id) => format!("telegram-{}-{}", message.chat.id, thread_id),
        None => format!("telegram-{}", message.chat.id),
    };
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
    let reply_target = match thread_id.as_deref() {
        Some(thread_id) => format!("telegram:chat:{}:thread:{}", message.chat.id, thread_id),
        None => format!("telegram:chat:{}", message.chat.id),
    };

    Ok(NormalizedTelegramMessage {
        session_id,
        input,
        display_name,
        actor_id,
        thread_id,
        thread_title,
        reply_target,
    })
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_private_message_updates() {
        let normalized = normalize_update(TelegramUpdate {
            update_id: 1,
            message: Some(TelegramMessage {
                message_id: 10,
                text: Some("hello telegram".to_owned()),
                caption: None,
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

        assert_eq!(normalized.session_id, "telegram-42");
        assert_eq!(normalized.actor_id.as_deref(), Some("7"));
        assert_eq!(normalized.display_name.as_deref(), Some("Guest"));
        assert_eq!(normalized.reply_target, "telegram:chat:42");
        assert_eq!(normalized.ingress().channel.as_deref(), Some("telegram"));
    }

    #[test]
    fn normalizes_topic_thread_updates() {
        let normalized = normalize_update(TelegramUpdate {
            update_id: 2,
            message: Some(TelegramMessage {
                message_id: 11,
                text: Some("thread hello".to_owned()),
                caption: None,
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

        assert_eq!(normalized.session_id, "telegram--100123-99");
        assert_eq!(normalized.thread_id.as_deref(), Some("99"));
        assert_eq!(normalized.thread_title.as_deref(), Some("Build Ops"));
        assert_eq!(normalized.reply_target, "telegram:chat:-100123:thread:99");
    }

    #[test]
    fn rejects_updates_without_textual_content() {
        let error = normalize_update(TelegramUpdate {
            update_id: 3,
            message: Some(TelegramMessage {
                message_id: 12,
                text: None,
                caption: None,
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

        assert!(error.to_string().contains("text or caption"));
    }
}
