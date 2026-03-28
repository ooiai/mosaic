use chrono::Utc;

use super::*;

pub(crate) fn ingress_trace_from_channel_message(message: &ChannelInboundMessage) -> IngressTrace {
    IngressTrace {
        kind: message.adapter.clone(),
        channel: Some(message.channel.clone()),
        adapter: Some(message.adapter.clone()),
        source: Some(message.adapter.clone()),
        remote_addr: None,
        display_name: message.display_name.clone(),
        actor_id: message.actor_id.clone(),
        conversation_id: Some(message.conversation_id.clone()),
        thread_id: message.thread_id.clone(),
        thread_title: message.thread_title.clone(),
        reply_target: Some(message.reply_target.clone()),
        message_id: Some(message.message_id.clone()),
        received_at: Some(message.received_at),
        raw_event_id: Some(message.raw_event_id.clone()),
        session_hint: message.session_hint.clone(),
        profile_hint: message.profile_hint.clone(),
        gateway_url: None,
    }
}

pub(crate) fn channel_message_to_submission(message: ChannelInboundMessage) -> RunSubmission {
    let ingress = ingress_trace_from_channel_message(&message);
    RunSubmission {
        system: None,
        input: message.text,
        skill: None,
        workflow: None,
        session_id: message.session_hint.clone(),
        profile: message.profile_hint.clone(),
        ingress: Some(ingress),
    }
}

pub(crate) fn normalize_webchat_message(message: InboundMessage) -> ChannelInboundMessage {
    let ingress = message.ingress;
    let session_hint = message
        .session_id
        .or_else(|| {
            ingress
                .as_ref()
                .and_then(|ingress| ingress.session_hint.clone())
        })
        .unwrap_or_else(|| format!("webchat-{}", Uuid::new_v4()));
    let actor_id = message.actor_id.or_else(|| {
        ingress
            .as_ref()
            .and_then(|ingress| ingress.actor_id.clone())
    });
    let conversation_id = message.conversation_id.unwrap_or_else(|| {
        ingress
            .as_ref()
            .and_then(|ingress| ingress.conversation_id.clone())
            .or_else(|| {
                message
                    .thread_id
                    .as_deref()
                    .map(|thread_id| format!("webchat:thread:{thread_id}"))
            })
            .or_else(|| {
                actor_id
                    .as_deref()
                    .map(|actor_id| format!("webchat:actor:{actor_id}"))
            })
            .unwrap_or_else(|| format!("webchat:session:{session_hint}"))
    });
    let message_id = message
        .message_id
        .or_else(|| {
            ingress
                .as_ref()
                .and_then(|ingress| ingress.message_id.clone())
        })
        .unwrap_or_else(|| Uuid::new_v4().to_string());
    let raw_event_id = message
        .raw_event_id
        .or_else(|| {
            ingress
                .as_ref()
                .and_then(|ingress| ingress.raw_event_id.clone())
        })
        .unwrap_or_else(|| format!("webchat-event:{message_id}"));
    let received_at = message
        .received_at
        .or_else(|| ingress.as_ref().and_then(|ingress| ingress.received_at))
        .unwrap_or_else(Utc::now);
    let reply_target = message
        .reply_target
        .or_else(|| {
            ingress
                .as_ref()
                .and_then(|ingress| ingress.reply_target.clone())
        })
        .unwrap_or_else(|| conversation_id.clone());

    ChannelInboundMessage {
        channel: "webchat".to_owned(),
        adapter: "webchat_http".to_owned(),
        actor_id,
        display_name: message.display_name.or_else(|| {
            ingress
                .as_ref()
                .and_then(|ingress| ingress.display_name.clone())
        }),
        conversation_id,
        thread_id: message.thread_id.or_else(|| {
            ingress
                .as_ref()
                .and_then(|ingress| ingress.thread_id.clone())
        }),
        thread_title: message.thread_title.or_else(|| {
            ingress
                .as_ref()
                .and_then(|ingress| ingress.thread_title.clone())
        }),
        reply_target,
        message_id,
        text: message.input,
        profile_hint: message.profile.or_else(|| {
            ingress
                .as_ref()
                .and_then(|ingress| ingress.profile_hint.clone())
        }),
        session_hint: Some(session_hint),
        received_at,
        raw_event_id,
    }
}

impl GatewayHandle {
    pub fn submit_channel_message(
        &self,
        message: ChannelInboundMessage,
    ) -> Result<GatewaySubmittedRun> {
        self.submit_run(channel_message_to_submission(message))
    }

    pub fn submit_telegram_update(&self, update: TelegramUpdate) -> Result<GatewaySubmittedRun> {
        self.submit_channel_message(normalize_telegram_update(update)?)
    }

    pub fn submit_webchat_message(&self, message: InboundMessage) -> Result<GatewaySubmittedRun> {
        self.submit_channel_message(normalize_webchat_message(message))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_webchat_messages_into_channel_contract() {
        let normalized = normalize_webchat_message(InboundMessage {
            session_id: Some("webchat-demo".to_owned()),
            input: "hello webchat".to_owned(),
            profile: Some("demo-provider".to_owned()),
            display_name: Some("Web Guest".to_owned()),
            actor_id: Some("actor-1".to_owned()),
            conversation_id: None,
            thread_id: Some("thread-1".to_owned()),
            thread_title: Some("Lobby".to_owned()),
            reply_target: Some("webchat:thread:thread-1".to_owned()),
            message_id: None,
            received_at: None,
            raw_event_id: None,
            ingress: None,
        });

        assert_eq!(normalized.channel, "webchat");
        assert_eq!(normalized.adapter, "webchat_http");
        assert_eq!(normalized.session_hint.as_deref(), Some("webchat-demo"));
        assert_eq!(normalized.profile_hint.as_deref(), Some("demo-provider"));
        assert_eq!(normalized.conversation_id, "webchat:thread:thread-1");
        assert_eq!(normalized.reply_target, "webchat:thread:thread-1");
    }
}
