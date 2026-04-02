use chrono::Utc;
use mosaic_control_protocol::OrchestrationOwner;

use super::command_catalog::{
    ChannelCommandCatalog, ChannelCommandCategory, ChannelCommandContext, build_command_catalog,
};
use super::*;

enum ParsedChannelCommand {
    Run(RunSubmission),
    Control {
        submission: RunSubmission,
        response_text: String,
        route_decision: RouteDecisionTrace,
        profile_override: Option<String>,
        binding_update: Option<ChannelConversationBinding>,
    },
}

pub(crate) fn ingress_trace_from_channel_message(message: &ChannelInboundMessage) -> IngressTrace {
    IngressTrace {
        kind: message.adapter.clone(),
        channel: Some(message.channel.clone()),
        adapter: Some(message.adapter.clone()),
        bot_name: message.bot_name.clone(),
        bot_route: message.bot_route.clone(),
        bot_profile: message.bot_profile.clone(),
        bot_token_env: message.bot_token_env.clone(),
        bot_secret_env: None,
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
        attachments: message.attachments.clone(),
        attachment_failures: Vec::new(),
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

fn parse_channel_command(
    gateway: &GatewayHandle,
    message: ChannelInboundMessage,
) -> ParsedChannelCommand {
    let trimmed = message.text.trim().to_owned();
    if !trimmed.starts_with("/mosaic") {
        return ParsedChannelCommand::Run(channel_message_to_submission(message));
    }

    let tail = trimmed.trim_start_matches("/mosaic").trim();
    if tail.is_empty() {
        return control_catalog_response(
            gateway,
            message,
            "catalog",
            None,
            "control command requested help".to_owned(),
            None,
        );
    }

    let mut words = tail.split_whitespace();
    let command = words.next().unwrap_or_default().to_ascii_lowercase();
    match command.as_str() {
        "tool" => {
            let Some(name) = words.next() else {
                return control_catalog_response(
                    gateway,
                    message,
                    "tool",
                    Some(ChannelCommandCategory::Tools),
                    "missing tool name for /mosaic tool".to_owned(),
                    Some("Usage: /mosaic tool <tool_name> <input>".to_owned()),
                );
            };
            let payload = remainder_after(&trimmed, 3);
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
                return control_catalog_response(
                    gateway,
                    message,
                    "skill",
                    Some(ChannelCommandCategory::Skills),
                    "missing skill name for /mosaic skill".to_owned(),
                    Some("Usage: /mosaic skill <skill_name> <input>".to_owned()),
                );
            };
            let payload = remainder_after(&trimmed, 3);
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
                return control_catalog_response(
                    gateway,
                    message,
                    "workflow",
                    Some(ChannelCommandCategory::Workflows),
                    "missing workflow name for /mosaic workflow".to_owned(),
                    Some("Usage: /mosaic workflow <workflow_name> <input>".to_owned()),
                );
            };
            let payload = remainder_after(&trimmed, 3);
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
        "profile" => handle_profile_command(gateway, message, words.next()),
        "session" => handle_session_command(gateway, message, words),
        "gateway" => handle_gateway_command(gateway, message, words),
        "help" => {
            let requested = words.next();
            let category = requested.and_then(ChannelCommandCategory::parse);
            if requested.is_some() && category.is_none() {
                return control_catalog_response(
                    gateway,
                    message,
                    "help",
                    None,
                    "explicit /mosaic help command".to_owned(),
                    Some(format!(
                        "unknown help category '{}'",
                        requested.unwrap_or_default()
                    )),
                );
            }
            control_catalog_response(
                gateway,
                message,
                "help",
                category,
                "explicit /mosaic help command".to_owned(),
                None,
            )
        }
        other => control_catalog_response(
            gateway,
            message,
            other,
            None,
            format!("unknown /mosaic command '{}'", other),
            Some(format!("unknown /mosaic command '{}'", other)),
        ),
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

fn control_catalog_response(
    gateway: &GatewayHandle,
    message: ChannelInboundMessage,
    control_command: &str,
    selected_category: Option<ChannelCommandCategory>,
    selection_reason: String,
    prefix: Option<String>,
) -> ParsedChannelCommand {
    let context = gateway.command_context_for(&message);
    let components = gateway.snapshot_components();
    let catalog = build_command_catalog(&components, &context, selected_category);
    let response_text = render_catalog_response(prefix, &catalog);
    let scope = catalog.scope.clone();
    control_response(
        message,
        Some(control_command),
        response_text,
        RouteDecisionTrace {
            route_mode: RouteMode::Control,
            route_kind: None,
            selected_capability_type: Some("help".to_owned()),
            selected_capability_name: Some("help".to_owned()),
            selected_tool: None,
            selected_skill: None,
            selected_workflow: None,
            selection_reason,
            capability_source: Some("gateway.control".to_owned()),
            capability_source_kind: None,
            source_name: None,
            source_path: None,
            source_version: None,
            execution_target: None,
            orchestration_owner: Some(OrchestrationOwner::Gateway),
            policy_source: Some("gateway.control".to_owned()),
            sandbox_scope: None,
            profile_used: Some(context.profile.clone()),
            selected_category: selected_category.map(|category| category.slug().to_owned()),
            catalog_scope: Some(scope),
        },
        None,
        None,
    )
}

fn handle_profile_command(
    gateway: &GatewayHandle,
    message: ChannelInboundMessage,
    profile_name: Option<&str>,
) -> ParsedChannelCommand {
    let Some(profile_name) = profile_name else {
        return control_catalog_response(
            gateway,
            message,
            "profile",
            Some(ChannelCommandCategory::Runtime),
            "missing profile name for /mosaic profile".to_owned(),
            Some("Usage: /mosaic profile <profile_name>".to_owned()),
        );
    };

    let components = gateway.snapshot_components();
    let Some(profile) = components.profiles.get(profile_name).cloned() else {
        return control_catalog_response(
            gateway,
            message,
            "profile",
            Some(ChannelCommandCategory::Runtime),
            format!("unknown provider profile '{}'", profile_name),
            Some(format!("unknown provider profile '{}'", profile_name)),
        );
    };
    if profile.api_key_env.is_some() && !profile.api_key_present() {
        return control_catalog_response(
            gateway,
            message,
            "profile",
            Some(ChannelCommandCategory::Runtime),
            format!("provider profile '{}' is not available", profile_name),
            Some(format!(
                "provider profile '{}' is missing its configured API key env",
                profile_name
            )),
        );
    }

    let scope = gateway.command_context_for(&message).scope_label();
    control_response(
        message,
        Some("profile"),
        format!(
            "conversation profile set to {} ({}/{})",
            profile.name, profile.provider_type, profile.model
        ),
        RouteDecisionTrace {
            route_mode: RouteMode::Control,
            route_kind: None,
            selected_capability_type: Some("profile".to_owned()),
            selected_capability_name: Some(profile.name.clone()),
            selected_tool: None,
            selected_skill: None,
            selected_workflow: None,
            selection_reason: "explicit /mosaic profile command".to_owned(),
            capability_source: Some("workspace_config".to_owned()),
            capability_source_kind: None,
            source_name: None,
            source_path: None,
            source_version: None,
            execution_target: None,
            orchestration_owner: Some(OrchestrationOwner::Gateway),
            policy_source: None,
            sandbox_scope: None,
            profile_used: Some(profile.name.clone()),
            selected_category: Some(ChannelCommandCategory::Runtime.slug().to_owned()),
            catalog_scope: Some(scope),
        },
        Some(profile.name.clone()),
        Some(ChannelConversationBinding {
            session_id: None,
            profile: Some(profile.name),
        }),
    )
}

fn handle_session_command<'a>(
    gateway: &GatewayHandle,
    message: ChannelInboundMessage,
    mut words: impl Iterator<Item = &'a str>,
) -> ParsedChannelCommand {
    match words.next().map(|word| word.to_ascii_lowercase()) {
        Some(command) if command == "new" => {
            let Some(session_name) = words.next() else {
                return control_catalog_response(
                    gateway,
                    message,
                    "session",
                    Some(ChannelCommandCategory::Session),
                    "missing session name for /mosaic session new".to_owned(),
                    Some("Usage: /mosaic session new <name>".to_owned()),
                );
            };

            let context = gateway.command_context_for(&message);
            let target_session = session_name.to_owned();
            control_response(
                ChannelInboundMessage {
                    session_hint: Some(target_session.clone()),
                    ..message.clone()
                },
                Some("session"),
                format!(
                    "conversation bound to new session {}\nprofile: {}",
                    target_session, context.profile
                ),
                RouteDecisionTrace {
                    route_mode: RouteMode::Control,
                    route_kind: None,
                    selected_capability_type: Some("session".to_owned()),
                    selected_capability_name: Some("session.new".to_owned()),
                    selected_tool: None,
                    selected_skill: None,
                    selected_workflow: None,
                    selection_reason: "explicit /mosaic session new command".to_owned(),
                    capability_source: Some("gateway.control".to_owned()),
                    capability_source_kind: None,
                    source_name: None,
                    source_path: None,
                    source_version: None,
                    execution_target: None,
                    orchestration_owner: Some(OrchestrationOwner::Gateway),
                    policy_source: Some("gateway.control".to_owned()),
                    sandbox_scope: None,
                    profile_used: Some(context.profile.clone()),
                    selected_category: Some(ChannelCommandCategory::Session.slug().to_owned()),
                    catalog_scope: Some(context.scope_label()),
                },
                Some(context.profile.clone()),
                Some(ChannelConversationBinding {
                    session_id: Some(target_session),
                    profile: Some(context.profile),
                }),
            )
        }
        Some(command) if command == "switch" => {
            let Some(session_name) = words.next() else {
                return control_catalog_response(
                    gateway,
                    message,
                    "session",
                    Some(ChannelCommandCategory::Session),
                    "missing session name for /mosaic session switch".to_owned(),
                    Some("Usage: /mosaic session switch <name>".to_owned()),
                );
            };

            match gateway.load_session(session_name) {
                Ok(Some(session)) => {
                    let scope = gateway.command_context_for(&message).scope_label();
                    control_response(
                        ChannelInboundMessage {
                            session_hint: Some(session.id.clone()),
                            profile_hint: Some(session.provider_profile.clone()),
                            ..message.clone()
                        },
                        Some("session"),
                        format!(
                            "conversation switched to session {}\nprofile: {}\nstatus: {}",
                            session.id,
                            session.provider_profile,
                            session.run.status.label()
                        ),
                        RouteDecisionTrace {
                            route_mode: RouteMode::Control,
                            route_kind: None,
                            selected_capability_type: Some("session".to_owned()),
                            selected_capability_name: Some("session.switch".to_owned()),
                            selected_tool: None,
                            selected_skill: None,
                            selected_workflow: None,
                            selection_reason: "explicit /mosaic session switch command".to_owned(),
                            capability_source: Some("gateway.control".to_owned()),
                            capability_source_kind: None,
                            source_name: None,
                            source_path: None,
                            source_version: None,
                            execution_target: None,
                            orchestration_owner: Some(OrchestrationOwner::Gateway),
                            policy_source: Some("gateway.control".to_owned()),
                            sandbox_scope: None,
                            profile_used: Some(session.provider_profile.clone()),
                            selected_category: Some(
                                ChannelCommandCategory::Session.slug().to_owned(),
                            ),
                            catalog_scope: Some(scope),
                        },
                        Some(session.provider_profile.clone()),
                        Some(ChannelConversationBinding {
                            session_id: Some(session.id),
                            profile: Some(session.provider_profile),
                        }),
                    )
                }
                Ok(None) => control_catalog_response(
                    gateway,
                    message,
                    "session",
                    Some(ChannelCommandCategory::Session),
                    format!("session '{}' does not exist", session_name),
                    Some(format!(
                        "session '{}' does not exist\nUse /mosaic session new {} to create it.",
                        session_name, session_name
                    )),
                ),
                Err(err) => control_catalog_response(
                    gateway,
                    message,
                    "session",
                    Some(ChannelCommandCategory::Session),
                    format!("failed to load session '{}': {}", session_name, err),
                    Some(format!(
                        "failed to load session '{}': {}",
                        session_name, err
                    )),
                ),
            }
        }
        Some(command) if command == "status" => session_status_response(gateway, message),
        Some(other) => control_catalog_response(
            gateway,
            message,
            "session",
            Some(ChannelCommandCategory::Session),
            format!("unknown /mosaic session command '{}'", other),
            Some(format!("unknown /mosaic session command '{}'", other)),
        ),
        None => control_catalog_response(
            gateway,
            message,
            "session",
            Some(ChannelCommandCategory::Session),
            "explicit /mosaic session help".to_owned(),
            None,
        ),
    }
}

fn session_status_response(
    gateway: &GatewayHandle,
    message: ChannelInboundMessage,
) -> ParsedChannelCommand {
    let context = gateway.command_context_for(&message);
    let response_text = match context.session_id.as_deref() {
        Some(session_id) => match gateway.load_session(session_id) {
            Ok(Some(session)) => format!(
                "Session status\nsession: {}\nroute: {}\nprofile: {}\nmodel: {}\nstatus: {}\nlast_run: {}",
                session.id,
                session.gateway.route,
                session.provider_profile,
                session.model,
                session.run.status.label(),
                session.last_run_id.unwrap_or_else(|| "<none>".to_owned())
            ),
            Ok(None) => format!(
                "Session status\nsession: {}\nroute: {}\nprofile: {}\nstatus: not created yet",
                session_id,
                session_route_for_id(session_id),
                context.profile
            ),
            Err(err) => format!(
                "Session status\nsession: {}\nprofile: {}\nerror: {}",
                session_id, context.profile, err
            ),
        },
        None => format!(
            "Session status\nsession: <none>\nprofile: {}\nstatus: no conversation binding",
            context.profile
        ),
    };

    control_response(
        message,
        Some("session"),
        response_text,
        RouteDecisionTrace {
            route_mode: RouteMode::Control,
            route_kind: None,
            selected_capability_type: Some("session".to_owned()),
            selected_capability_name: Some("session.status".to_owned()),
            selected_tool: None,
            selected_skill: None,
            selected_workflow: None,
            selection_reason: "explicit /mosaic session status command".to_owned(),
            capability_source: Some("gateway.control".to_owned()),
            capability_source_kind: None,
            source_name: None,
            source_path: None,
            source_version: None,
            execution_target: None,
            orchestration_owner: Some(OrchestrationOwner::Gateway),
            policy_source: Some("gateway.control".to_owned()),
            sandbox_scope: None,
            profile_used: Some(context.profile.clone()),
            selected_category: Some(ChannelCommandCategory::Session.slug().to_owned()),
            catalog_scope: Some(context.scope_label()),
        },
        None,
        None,
    )
}

fn handle_gateway_command<'a>(
    gateway: &GatewayHandle,
    message: ChannelInboundMessage,
    mut words: impl Iterator<Item = &'a str>,
) -> ParsedChannelCommand {
    match words.next().map(|word| word.to_ascii_lowercase()) {
        Some(command) if command == "status" => {
            let context = gateway.command_context_for(&message);
            let health = gateway.health();
            let readiness = gateway.readiness();
            let adapters = gateway
                .list_adapter_statuses()
                .into_iter()
                .map(|adapter| format!("{}={}", adapter.name, adapter.status))
                .collect::<Vec<_>>()
                .join(", ");
            control_response(
                message,
                Some("gateway"),
                format!(
                    "Gateway status\nhealth: {}\nreadiness: {}\ntransport: {}\nactive_profile: {}\nsessions: {}\nadapters: {}",
                    health.status,
                    readiness.status,
                    health.transport,
                    health.active_profile,
                    health.session_count,
                    adapters
                ),
                RouteDecisionTrace {
                    route_mode: RouteMode::Control,
                    route_kind: None,
                    selected_capability_type: Some("gateway".to_owned()),
                    selected_capability_name: Some("gateway.status".to_owned()),
                    selected_tool: None,
                    selected_skill: None,
                    selected_workflow: None,
                    selection_reason: "explicit /mosaic gateway status command".to_owned(),
                    capability_source: Some("gateway.control".to_owned()),
                    capability_source_kind: None,
                    source_name: None,
                    source_path: None,
                    source_version: None,
                    execution_target: None,
                    orchestration_owner: Some(OrchestrationOwner::Gateway),
                    policy_source: Some("gateway.control".to_owned()),
                    sandbox_scope: None,
                    profile_used: Some(context.profile.clone()),
                    selected_category: Some(ChannelCommandCategory::Gateway.slug().to_owned()),
                    catalog_scope: Some(context.scope_label()),
                },
                None,
                None,
            )
        }
        Some(command) if command == "help" => control_catalog_response(
            gateway,
            message,
            "gateway",
            Some(ChannelCommandCategory::Gateway),
            "explicit /mosaic gateway help command".to_owned(),
            None,
        ),
        Some(other) => control_catalog_response(
            gateway,
            message,
            "gateway",
            Some(ChannelCommandCategory::Gateway),
            format!("unknown /mosaic gateway command '{}'", other),
            Some(format!("unknown /mosaic gateway command '{}'", other)),
        ),
        None => control_catalog_response(
            gateway,
            message,
            "gateway",
            Some(ChannelCommandCategory::Gateway),
            "explicit /mosaic gateway help".to_owned(),
            None,
        ),
    }
}

fn control_response(
    message: ChannelInboundMessage,
    control_command: Option<&str>,
    response_text: String,
    route_decision: RouteDecisionTrace,
    profile_override: Option<String>,
    binding_update: Option<ChannelConversationBinding>,
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
            ingress: Some(ingress_trace_with_command(&message, control_command)),
        },
        response_text,
        route_decision,
        profile_override,
        binding_update,
    }
}

fn render_catalog_response(prefix: Option<String>, catalog: &ChannelCommandCatalog) -> String {
    match prefix {
        Some(prefix) => format!("{}\n\n{}", prefix, catalog.render()),
        None => catalog.render(),
    }
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
        bot_name: None,
        bot_route: None,
        bot_profile: None,
        bot_token_env: None,
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
        attachments: Vec::new(),
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
    fn conversation_binding_key(&self, message: &ChannelInboundMessage) -> String {
        format!(
            "{}::{}",
            message.channel.to_ascii_lowercase(),
            message.conversation_id
        )
    }

    fn conversation_binding_for(
        &self,
        message: &ChannelInboundMessage,
    ) -> Option<ChannelConversationBinding> {
        self.inner
            .conversation_bindings
            .lock()
            .expect("conversation binding lock should not be poisoned")
            .get(&self.conversation_binding_key(message))
            .cloned()
    }

    fn persist_conversation_binding(
        &self,
        message: &ChannelInboundMessage,
        update: ChannelConversationBinding,
    ) {
        let key = self.conversation_binding_key(message);
        let mut bindings = self
            .inner
            .conversation_bindings
            .lock()
            .expect("conversation binding lock should not be poisoned");
        let entry = bindings.entry(key).or_default();
        if update.session_id.is_some() {
            entry.session_id = update.session_id;
        }
        if update.profile.is_some() {
            entry.profile = update.profile;
        }
    }

    fn apply_conversation_binding(
        &self,
        mut message: ChannelInboundMessage,
    ) -> ChannelInboundMessage {
        if let Some(binding) = self.conversation_binding_for(&message) {
            if let Some(session_id) = binding.session_id {
                message.session_hint = Some(session_id);
            }
            if let Some(profile) = binding.profile {
                message.profile_hint = Some(profile);
            }
        }
        message
    }

    fn command_context_for(&self, message: &ChannelInboundMessage) -> ChannelCommandContext {
        let components = self.snapshot_components();
        let profile = components
            .profiles
            .resolve(message.profile_hint.as_deref())
            .map(|profile| profile.name)
            .unwrap_or_else(|_| components.profiles.active_profile_name().to_owned());
        ChannelCommandContext {
            channel: message.channel.clone(),
            bot_name: message.bot_name.clone(),
            session_id: message.session_hint.clone(),
            profile,
        }
    }

    pub fn submit_channel_message(
        &self,
        message: ChannelInboundMessage,
    ) -> Result<GatewaySubmittedRun> {
        let message = self.apply_conversation_binding(message);
        match parse_channel_command(self, message) {
            ParsedChannelCommand::Run(submission) => self.submit_run(submission),
            ParsedChannelCommand::Control {
                submission,
                response_text,
                route_decision,
                profile_override,
                binding_update,
            } => {
                let submitted = self.submit_control_response(
                    submission.clone(),
                    response_text,
                    route_decision,
                    profile_override,
                )?;
                if let (Some(update), Some(ingress)) = (binding_update, submission.ingress.as_ref())
                {
                    let message = ChannelInboundMessage {
                        channel: ingress
                            .channel
                            .clone()
                            .unwrap_or_else(|| "unknown".to_owned()),
                        adapter: ingress
                            .adapter
                            .clone()
                            .unwrap_or_else(|| ingress.kind.clone()),
                        bot_name: ingress.bot_name.clone(),
                        bot_route: ingress.bot_route.clone(),
                        bot_profile: ingress.bot_profile.clone(),
                        bot_token_env: ingress.bot_token_env.clone(),
                        actor_id: ingress.actor_id.clone(),
                        display_name: ingress.display_name.clone(),
                        conversation_id: ingress
                            .conversation_id
                            .clone()
                            .unwrap_or_else(|| "unknown".to_owned()),
                        thread_id: ingress.thread_id.clone(),
                        thread_title: ingress.thread_title.clone(),
                        reply_target: ingress
                            .reply_target
                            .clone()
                            .unwrap_or_else(|| "unknown".to_owned()),
                        message_id: ingress
                            .message_id
                            .clone()
                            .unwrap_or_else(|| "unknown".to_owned()),
                        text: submission.input.clone(),
                        attachments: ingress.attachments.clone(),
                        profile_hint: submission.profile.clone(),
                        session_hint: submission.session_id.clone(),
                        received_at: ingress.received_at.unwrap_or_else(Utc::now),
                        raw_event_id: ingress
                            .raw_event_id
                            .clone()
                            .unwrap_or_else(|| "unknown".to_owned()),
                    };
                    self.persist_conversation_binding(&message, update);
                }
                Ok(submitted)
            }
        }
    }

    pub(crate) fn submit_telegram_update(
        &self,
        bot: &ResolvedTelegramBot,
        update: TelegramUpdate,
    ) -> Result<GatewaySubmittedRun> {
        self.submit_channel_message(mosaic_channel_telegram::normalize_update_with_context(
            update,
            Some(&bot.context()),
        )?)
    }

    pub fn submit_webchat_message(&self, message: InboundMessage) -> Result<GatewaySubmittedRun> {
        self.submit_channel_message(normalize_webchat_message(message))
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use mosaic_config::{MosaicConfig, ProviderProfileConfig};
    use mosaic_memory::{FileMemoryStore, MemoryPolicy};
    use mosaic_provider::MockProvider;
    use mosaic_scheduler_core::FileCronStore;

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
        let mut config = MosaicConfig::default();
        config.active_profile = "demo-provider".to_owned();
        config.profiles.insert(
            "demo-provider".to_owned(),
            ProviderProfileConfig {
                provider_type: "mock".to_owned(),
                model: "mock".to_owned(),
                base_url: None,
                api_key_env: None,
                transport: Default::default(),
                attachments: Default::default(),
                vendor: Default::default(),
            },
        );
        let profiles =
            ProviderProfileRegistry::from_config(&config).expect("profile registry should build");
        let runtime = tokio::runtime::Runtime::new().expect("test runtime should build");
        let workspace_root = std::env::temp_dir().join("mosaic-ingress-tests-workspace");
        let sandbox = crate::build_sandbox_manager(&workspace_root, &config)
            .expect("sandbox manager should build");
        let gateway = GatewayHandle::new_local(
            runtime.handle().clone(),
            GatewayRuntimeComponents {
                config_snapshot: config.clone(),
                profiles: Arc::new(profiles),
                provider_override: Some(Arc::new(MockProvider)),
                session_store: Arc::new(crate::tests::MemorySessionStore::default()),
                memory_store: Arc::new(FileMemoryStore::new(
                    std::env::temp_dir().join("mosaic-ingress-tests-memory"),
                )),
                memory_policy: MemoryPolicy::default(),
                runtime_policy: config.runtime.clone(),
                attachments: config.attachments.clone(),
                sandbox,
                telegram: config.telegram.clone(),
                app_name: None,
                tools: Arc::new(ToolRegistry::new()),
                skills: Arc::new(SkillRegistry::new()),
                workflows: Arc::new(WorkflowRegistry::new()),
                node_store: Arc::new(FileNodeStore::new(
                    std::env::temp_dir().join("mosaic-ingress-tests-nodes"),
                )),
                mcp_manager: None,
                cron_store: Arc::new(FileCronStore::new(
                    std::env::temp_dir().join("mosaic-ingress-tests-cron"),
                )),
                workspace_root,
                runs_dir: std::env::temp_dir().join("mosaic-ingress-tests-runs"),
                audit_root: std::env::temp_dir().join("mosaic-ingress-tests-audit"),
                extensions: Vec::new(),
                policies: PolicyConfig::default(),
                deployment: config.deployment.clone(),
                auth: config.auth.clone(),
                audit: config.audit.clone(),
                observability: config.observability.clone(),
            },
        );

        let parsed = parse_channel_command(
            &gateway,
            ChannelInboundMessage {
                channel: "telegram".to_owned(),
                adapter: "telegram_bot".to_owned(),
                bot_name: None,
                bot_route: None,
                bot_profile: None,
                bot_token_env: None,
                actor_id: Some("17".to_owned()),
                display_name: Some("Operator".to_owned()),
                conversation_id: "telegram:chat:1".to_owned(),
                thread_id: None,
                thread_title: None,
                reply_target: "telegram:chat:1".to_owned(),
                message_id: "99".to_owned(),
                text: "/mosaic tool read_file README.md".to_owned(),
                attachments: Vec::new(),
                profile_hint: None,
                session_hint: Some("demo".to_owned()),
                received_at: Utc::now(),
                raw_event_id: "raw-1".to_owned(),
            },
        );

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
