use super::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NormalizedWebchatMessage {
    pub session_id: String,
    pub input: String,
    pub profile: Option<String>,
    pub ingress: IngressTrace,
}

impl NormalizedWebchatMessage {
    fn into_submission(self) -> RunSubmission {
        RunSubmission {
            system: None,
            input: self.input,
            skill: None,
            workflow: None,
            session_id: Some(self.session_id),
            profile: self.profile,
            ingress: Some(self.ingress),
        }
    }
}

pub(crate) fn normalize_webchat_message(message: InboundMessage) -> NormalizedWebchatMessage {
    let session_id = message
        .session_id
        .unwrap_or_else(|| format!("webchat-{}", Uuid::new_v4()));
    let ingress = message.ingress.unwrap_or(IngressTrace {
        kind: "webchat".to_owned(),
        channel: Some("webchat".to_owned()),
        source: Some("webchat-ingress".to_owned()),
        remote_addr: None,
        display_name: message.display_name,
        actor_id: message.actor_id,
        thread_id: message.thread_id,
        thread_title: message.thread_title,
        reply_target: message.reply_target,
        gateway_url: None,
    });

    NormalizedWebchatMessage {
        session_id,
        input: message.input,
        profile: message.profile,
        ingress,
    }
}

impl GatewayHandle {
    pub fn submit_telegram_update(&self, update: TelegramUpdate) -> Result<GatewaySubmittedRun> {
        let normalized = normalize_telegram_update(update)?;
        let ingress = normalized.ingress();
        self.submit_run(RunSubmission {
            system: None,
            input: normalized.input,
            skill: None,
            workflow: None,
            session_id: Some(normalized.session_id),
            profile: None,
            ingress: Some(ingress),
        })
    }

    pub fn submit_webchat_message(&self, message: InboundMessage) -> Result<GatewaySubmittedRun> {
        self.submit_run(normalize_webchat_message(message).into_submission())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_webchat_messages_into_interaction_entry_shape() {
        let normalized = normalize_webchat_message(InboundMessage {
            session_id: Some("webchat-demo".to_owned()),
            input: "hello webchat".to_owned(),
            profile: Some("demo-provider".to_owned()),
            ingress: None,
            display_name: Some("Web Guest".to_owned()),
            actor_id: Some("actor-1".to_owned()),
            thread_id: Some("thread-1".to_owned()),
            thread_title: Some("Lobby".to_owned()),
            reply_target: Some("webchat:thread:thread-1".to_owned()),
        });

        assert_eq!(normalized.session_id, "webchat-demo");
        assert_eq!(normalized.profile.as_deref(), Some("demo-provider"));
        assert_eq!(normalized.ingress.channel.as_deref(), Some("webchat"));
        assert_eq!(
            normalized.ingress.reply_target.as_deref(),
            Some("webchat:thread:thread-1")
        );
    }
}
