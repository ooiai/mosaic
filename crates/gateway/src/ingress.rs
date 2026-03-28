use chrono::Utc;

use super::*;

enum ParsedChannelCommand {
    Run(RunSubmission),
    Control {
        submission: RunSubmission,
        response_text: String,
        route_decision: RouteDecisionTrace,
        profile_override: Option<String>,
    },
}

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
        control_command: None,
        original_text: None,
        gateway_url: None,
    }
}

pub(crate) fn channel_message_to_submission(message: ChannelInboundMessage) -> RunSubmission {
    let ingress = ingress_trace_from_channel_message(&message);
    RunSubmission {
        system: None,
        input: message.text,
        tool: None,
        skill: None,
        workflow: None,
        session_id: message.session_hint.clone(),
        profile: message.profile_hint.clone(),
        ingress: Some(ingress),
    }
}

fn parse_channel_command(message: ChannelInboundMessage) -> ParsedChannelCommand {
    let trimmed = message.text.trim();
    if !trimmed.starts_with("/mosaic") {
        return ParsedChannelCommand::Run(channel_message_to_submission(message));
    }

    let ingress = ingress_trace_with_command(&message, None);
    let tail = trimmed.trim_start_matches("/mosaic").trim();
    if tail.is_empty() {
        return control_help(
            message,
            ingress,
            "control command requested help".to_owned(),
            None,
        );
    }

    let mut words = tail.split_whitespace();
    let command = words.next().unwrap_or_default().to_ascii_lowercase();
    match command.as_str() {
        "tool" => {
            let Some(name) = words.next() else {
                return control_help(
                    message,
                    ingress,
                    "missing tool name for /mosaic tool".to_owned(),
                    Some("tool".to_owned()),
                );
            };
            let payload = remainder_after(trimmed, 3);
            ParsedChannelCommand::Run(RunSubmission {
                system: None,
                input: payload.to_owned(),
                tool: Some(name.to_owned()),
                skill: None,
                workflow: None,
                session_id: message.session_hint.clone(),
                profile: message.profile_hint.clone(),
                ingress: Some(ingress_trace_with_command(&message, Some("tool"))),
            })
        }
        "skill" => {
            let Some(name) = words.next() else {
                return control_help(
                    message,
                    ingress,
                    "missing skill name for /mosaic skill".to_owned(),
                    Some("skill".to_owned()),
                );
            };
            let payload = remainder_after(trimmed, 3);
            ParsedChannelCommand::Run(RunSubmission {
                system: None,
                input: payload.to_owned(),
                tool: None,
                skill: Some(name.to_owned()),
                workflow: None,
                session_id: message.session_hint.clone(),
                profile: message.profile_hint.clone(),
                ingress: Some(ingress_trace_with_command(&message, Some("skill"))),
            })
        }
        "workflow" => {
            let Some(name) = words.next() else {
                return control_help(
                    message,
                    ingress,
                    "missing workflow name for /mosaic workflow".to_owned(),
                    Some("workflow".to_owned()),
                );
            };
            let payload = remainder_after(trimmed, 3);
            ParsedChannelCommand::Run(RunSubmission {
                system: None,
                input: payload.to_owned(),
                tool: None,
                skill: None,
                workflow: Some(name.to_owned()),
                session_id: message.session_hint.clone(),
                profile: message.profile_hint.clone(),
                ingress: Some(ingress_trace_with_command(&message, Some("workflow"))),
            })
        }
        "profile" => {
            let Some(profile_name) = words.next() else {
                return control_help(
                    message,
                    ingress,
                    "missing profile name for /mosaic profile".to_owned(),
                    Some("profile".to_owned()),
                );
            };
            ParsedChannelCommand::Control {
                submission: RunSubmission {
                    system: None,
                    input: message.text.clone(),
                    tool: None,
                    skill: None,
                    workflow: None,
                    session_id: message.session_hint.clone(),
                    profile: Some(profile_name.to_owned()),
                    ingress: Some(ingress_trace_with_command(&message, Some("profile"))),
                },
                response_text: format!("session profile set to {}", profile_name),
                route_decision: RouteDecisionTrace {
                    route_mode: RouteMode::Control,
                    selected_capability_type: Some("profile".to_owned()),
                    selected_capability_name: Some(profile_name.to_owned()),
                    selected_tool: None,
                    selected_skill: None,
                    selected_workflow: None,
                    selection_reason: "explicit /mosaic profile command".to_owned(),
                    capability_source: Some("workspace_config".to_owned()),
                    profile_used: Some(profile_name.to_owned()),
                },
                profile_override: Some(profile_name.to_owned()),
            }
        }
        "help" => control_help(
            message,
            ingress,
            "explicit /mosaic help command".to_owned(),
            Some("help".to_owned()),
        ),
        other => ParsedChannelCommand::Control {
            submission: RunSubmission {
                system: None,
                input: message.text.clone(),
                tool: None,
                skill: None,
                workflow: None,
                session_id: message.session_hint.clone(),
                profile: message.profile_hint.clone(),
                ingress: Some(ingress_trace_with_command(&message, Some(other))),
            },
            response_text: format!(
                "unknown /mosaic command '{}'\n{}",
                other,
                command_help_text()
            ),
            route_decision: RouteDecisionTrace {
                route_mode: RouteMode::Control,
                selected_capability_type: Some("help".to_owned()),
                selected_capability_name: Some("help".to_owned()),
                selected_tool: None,
                selected_skill: None,
                selected_workflow: None,
                selection_reason: format!("unknown /mosaic command '{}'", other),
                capability_source: Some("gateway.control".to_owned()),
                profile_used: message.profile_hint.clone(),
            },
            profile_override: None,
        },
    }
}

fn ingress_trace_with_command(
    message: &ChannelInboundMessage,
    control_command: Option<&str>,
) -> IngressTrace {
    let mut ingress = ingress_trace_from_channel_message(message);
    ingress.control_command = control_command.map(ToOwned::to_owned);
    ingress.original_text = Some(message.text.clone());
    ingress
}

fn control_help(
    message: ChannelInboundMessage,
    _ingress: IngressTrace,
    selection_reason: String,
    command: Option<String>,
) -> ParsedChannelCommand {
    ParsedChannelCommand::Control {
        submission: RunSubmission {
            system: None,
            input: message.text.clone(),
            tool: None,
            skill: None,
            workflow: None,
            session_id: message.session_hint.clone(),
            profile: message.profile_hint.clone(),
            ingress: Some(ingress_trace_with_command(&message, command.as_deref())),
        },
        response_text: command_help_text(),
        route_decision: RouteDecisionTrace {
            route_mode: RouteMode::Control,
            selected_capability_type: Some("help".to_owned()),
            selected_capability_name: Some("help".to_owned()),
            selected_tool: None,
            selected_skill: None,
            selected_workflow: None,
            selection_reason,
            capability_source: Some("gateway.control".to_owned()),
            profile_used: message.profile_hint.clone(),
        },
        profile_override: None,
    }
}

fn command_help_text() -> String {
    [
        "/mosaic tool <tool_name> <input>",
        "/mosaic skill <skill_name> <input>",
        "/mosaic workflow <workflow_name> <input>",
        "/mosaic profile <profile_name>",
        "/mosaic help",
    ]
    .join("\n")
}

fn remainder_after(input: &str, word_count: usize) -> &str {
    let mut seen = 0usize;
    let mut in_token = false;
    for (idx, ch) in input.char_indices() {
        if ch.is_whitespace() {
            if in_token {
                seen += 1;
                in_token = false;
                if seen == word_count {
                    return input[idx..].trim();
                }
            }
        } else if !in_token {
            in_token = true;
        }
    }

    if in_token {
        seen += 1;
    }

    if seen >= word_count { "" } else { "" }
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
        match parse_channel_command(message) {
            ParsedChannelCommand::Run(submission) => self.submit_run(submission),
            ParsedChannelCommand::Control {
                submission,
                response_text,
                route_decision,
                profile_override,
            } => self.submit_control_response(
                submission,
                response_text,
                route_decision,
                profile_override,
            ),
        }
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

    #[test]
    fn parses_explicit_tool_command_into_run_submission() {
        let parsed = parse_channel_command(ChannelInboundMessage {
            channel: "telegram".to_owned(),
            adapter: "telegram_bot".to_owned(),
            actor_id: Some("17".to_owned()),
            display_name: Some("Operator".to_owned()),
            conversation_id: "telegram:chat:1".to_owned(),
            thread_id: None,
            thread_title: None,
            reply_target: "telegram:chat:1".to_owned(),
            message_id: "99".to_owned(),
            text: "/mosaic tool read_file README.md".to_owned(),
            profile_hint: None,
            session_hint: Some("demo".to_owned()),
            received_at: Utc::now(),
            raw_event_id: "raw-1".to_owned(),
        });

        match parsed {
            ParsedChannelCommand::Run(submission) => {
                assert_eq!(submission.tool.as_deref(), Some("read_file"));
                assert_eq!(submission.input, "README.md");
                assert_eq!(
                    submission
                        .ingress
                        .as_ref()
                        .and_then(|ingress| ingress.control_command.as_deref()),
                    Some("tool")
                );
            }
            ParsedChannelCommand::Control { .. } => {
                panic!("explicit tool command should submit a run")
            }
        }
    }
}
