use mosaic_config::{
    AttachmentKindConfig, AttachmentRouteModeConfig, AttachmentRoutingTargetConfig,
    ProviderAttachmentRoutingConfig,
};
use mosaic_inspect::{
    AttachmentKind, AttachmentRouteMode, AttachmentRouteTrace, ChannelAttachment,
};

use super::*;

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct AttachmentRequirements {
    pub(crate) requires_vision: bool,
    pub(crate) requires_documents: bool,
    pub(crate) requires_audio: bool,
    pub(crate) requires_video: bool,
}

#[derive(Debug, Clone)]
pub(crate) struct ResolvedAttachmentTarget {
    pub(crate) target: AttachmentRoutingTargetConfig,
    pub(crate) policy_scope: String,
    pub(crate) bot_identity: Option<String>,
}

pub(crate) struct ResolvedAttachmentRoute {
    pub(crate) trace: AttachmentRouteTrace,
    pub(crate) specialized_skill: Option<String>,
    pub(crate) requested_profile: Option<String>,
    pub(crate) requirements: AttachmentRequirements,
}

impl AgentRuntime {
    pub(crate) fn request_attachments(req: &RunRequest) -> Vec<ChannelAttachment> {
        req.ingress
            .as_ref()
            .map(|ingress| ingress.attachments.clone())
            .unwrap_or_default()
    }

    pub(crate) fn resolve_attachment_route(
        &self,
        req: &RunRequest,
        planned_route: &crate::routing::PlannedRoute,
        base_profile: &ProviderProfile,
    ) -> Result<Option<ResolvedAttachmentRoute>> {
        let attachments = Self::request_attachments(req);
        if attachments.is_empty() {
            return Ok(None);
        }

        let failure_summary = req
            .ingress
            .as_ref()
            .map(|ingress| {
                ingress
                    .attachment_failures
                    .iter()
                    .map(|failure| {
                        format!("{}:{}:{}", failure.stage, failure.kind, failure.message)
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let attachment_kinds = attachments
            .iter()
            .map(|attachment| attachment.kind.label().to_owned())
            .collect::<Vec<_>>();
        let attachment_filenames = attachments
            .iter()
            .filter_map(|attachment| attachment.filename.clone())
            .collect::<Vec<_>>();
        let bot_identity = req
            .ingress
            .as_ref()
            .and_then(|ingress| ingress.bot_name.clone());

        let resolved = match planned_route {
            crate::routing::PlannedRoute::Assistant { .. } => {
                let resolved_target = self.resolve_attachment_target(req, base_profile);
                self.validate_attachment_target(
                    &resolved_target.target,
                    &attachments,
                    &resolved_target.policy_scope,
                )?;
                let requirements = Self::attachment_requirements(&attachments);
                let allowed_attachment_kinds =
                    Self::attachment_kind_config_labels(&resolved_target.target);
                match resolved_target.target.mode {
                    AttachmentRouteModeConfig::ProviderNative => ResolvedAttachmentRoute {
                        trace: AttachmentRouteTrace {
                            mode: AttachmentRouteMode::ProviderNative,
                            selection_reason: format!(
                                "attachment route resolved to provider_native via {} policy",
                                resolved_target.policy_scope
                            ),
                            bot_identity: resolved_target.bot_identity.clone(),
                            policy_scope: Some(resolved_target.policy_scope.clone()),
                            selected_profile: resolved_target.target.multimodal_profile.clone(),
                            provider_profile: None,
                            provider_model: None,
                            processor: None,
                            allowed_attachment_kinds,
                            max_attachment_size_mb: resolved_target.target.max_attachment_size_mb,
                            attachment_count: attachments.len(),
                            attachment_kinds,
                            attachment_filenames,
                            failure_summary,
                        },
                        specialized_skill: None,
                        requested_profile: resolved_target.target.multimodal_profile.clone(),
                        requirements,
                    },
                    AttachmentRouteModeConfig::SpecializedProcessor => {
                        let processor = resolved_target.target.processor.clone().ok_or_else(|| {
                            anyhow!(
                                "attachment routing mode specialized_processor requires processor in {}",
                                resolved_target.policy_scope
                            )
                        })?;
                        let skill = self.ctx.skills.get(&processor).ok_or_else(|| {
                            anyhow!("attachment processor skill not found: {processor}")
                        })?;
                        if !skill.metadata().exposure.accepts_attachments {
                            bail!(
                                "attachment processor skill '{}' does not accept attachments",
                                processor
                            );
                        }
                        ResolvedAttachmentRoute {
                            trace: AttachmentRouteTrace {
                                mode: AttachmentRouteMode::SpecializedProcessor,
                                selection_reason: format!(
                                    "attachment route resolved to specialized_processor via {} policy",
                                    resolved_target.policy_scope
                                ),
                                bot_identity: resolved_target.bot_identity.clone(),
                                policy_scope: Some(resolved_target.policy_scope.clone()),
                                selected_profile: resolved_target
                                    .target
                                    .specialized_processor_profile
                                    .clone(),
                                provider_profile: None,
                                provider_model: None,
                                processor: Some(processor.clone()),
                                allowed_attachment_kinds,
                                max_attachment_size_mb: resolved_target
                                    .target
                                    .max_attachment_size_mb,
                                attachment_count: attachments.len(),
                                attachment_kinds,
                                attachment_filenames,
                                failure_summary,
                            },
                            specialized_skill: Some(processor),
                            requested_profile: resolved_target
                                .target
                                .specialized_processor_profile
                                .clone(),
                            requirements: AttachmentRequirements::default(),
                        }
                    }
                    AttachmentRouteModeConfig::Disabled => {
                        bail!(
                            "attachment handling is disabled by {} policy",
                            resolved_target.policy_scope
                        )
                    }
                }
            }
            crate::routing::PlannedRoute::Tool { name, .. } => {
                let tool = self
                    .ctx
                    .tools
                    .get(name)
                    .ok_or_else(|| anyhow!("tool not found: {name}"))?;
                if !tool.metadata().exposure.accepts_attachments {
                    bail!("tool '{}' does not accept attachments", name);
                }
                ResolvedAttachmentRoute {
                    trace: AttachmentRouteTrace {
                        mode: AttachmentRouteMode::SpecializedProcessor,
                        selection_reason: "attachments forwarded to explicit tool metadata"
                            .to_owned(),
                        bot_identity: bot_identity.clone(),
                        policy_scope: Some("capability:tool".to_owned()),
                        selected_profile: None,
                        provider_profile: None,
                        provider_model: None,
                        processor: Some(format!("tool:{name}")),
                        allowed_attachment_kinds: Vec::new(),
                        max_attachment_size_mb: None,
                        attachment_count: attachments.len(),
                        attachment_kinds,
                        attachment_filenames,
                        failure_summary,
                    },
                    specialized_skill: None,
                    requested_profile: None,
                    requirements: AttachmentRequirements::default(),
                }
            }
            crate::routing::PlannedRoute::Skill { name, .. } => {
                let skill = self
                    .ctx
                    .skills
                    .get(name)
                    .ok_or_else(|| anyhow!("skill not found: {name}"))?;
                if !skill.metadata().exposure.accepts_attachments {
                    bail!("skill '{}' does not accept attachments", name);
                }
                ResolvedAttachmentRoute {
                    trace: AttachmentRouteTrace {
                        mode: AttachmentRouteMode::SpecializedProcessor,
                        selection_reason: "attachments forwarded to explicit skill metadata"
                            .to_owned(),
                        bot_identity: bot_identity.clone(),
                        policy_scope: Some("capability:skill".to_owned()),
                        selected_profile: None,
                        provider_profile: None,
                        provider_model: None,
                        processor: Some(format!("skill:{name}")),
                        allowed_attachment_kinds: Vec::new(),
                        max_attachment_size_mb: None,
                        attachment_count: attachments.len(),
                        attachment_kinds,
                        attachment_filenames,
                        failure_summary,
                    },
                    specialized_skill: None,
                    requested_profile: None,
                    requirements: AttachmentRequirements::default(),
                }
            }
            crate::routing::PlannedRoute::Workflow { name, .. } => {
                let workflow = self
                    .ctx
                    .workflows
                    .metadata(name)
                    .ok_or_else(|| anyhow!("workflow not found: {name}"))?;
                if !workflow.exposure.accepts_attachments {
                    bail!("workflow '{}' does not accept attachments", name);
                }
                ResolvedAttachmentRoute {
                    trace: AttachmentRouteTrace {
                        mode: AttachmentRouteMode::SpecializedProcessor,
                        selection_reason: "attachments forwarded to explicit workflow metadata"
                            .to_owned(),
                        bot_identity,
                        policy_scope: Some("capability:workflow".to_owned()),
                        selected_profile: None,
                        provider_profile: None,
                        provider_model: None,
                        processor: Some(format!("workflow:{name}")),
                        allowed_attachment_kinds: Vec::new(),
                        max_attachment_size_mb: None,
                        attachment_count: attachments.len(),
                        attachment_kinds,
                        attachment_filenames,
                        failure_summary,
                    },
                    specialized_skill: None,
                    requested_profile: None,
                    requirements: AttachmentRequirements::default(),
                }
            }
        };

        Ok(Some(resolved))
    }

    pub(crate) fn resolve_attachment_target(
        &self,
        req: &RunRequest,
        base_profile: &ProviderProfile,
    ) -> ResolvedAttachmentTarget {
        let ingress = req.ingress.as_ref();
        let bot_identity = ingress.and_then(|ingress| ingress.bot_name.clone());

        if let Some(bot_name) = ingress.and_then(|ingress| ingress.bot_name.as_deref()) {
            if let Some(target) = self
                .ctx
                .telegram
                .bots
                .get(bot_name)
                .and_then(|bot| bot.attachments.clone())
            {
                return ResolvedAttachmentTarget {
                    target,
                    policy_scope: format!("bot:{bot_name}"),
                    bot_identity: Some(bot_name.to_owned()),
                };
            }
            if let Some(target) = self.ctx.attachments.routing.bot_overrides.get(bot_name) {
                return ResolvedAttachmentTarget {
                    target: target.clone(),
                    policy_scope: format!("bot:{bot_name}:legacy"),
                    bot_identity: Some(bot_name.to_owned()),
                };
            }
        }
        if let Some(bot_route) = ingress.and_then(|ingress| ingress.bot_route.as_deref()) {
            if let Some(target) = self.ctx.attachments.routing.bot_overrides.get(bot_route) {
                return ResolvedAttachmentTarget {
                    target: target.clone(),
                    policy_scope: format!("bot_route:{bot_route}:legacy"),
                    bot_identity,
                };
            }
        }
        if let Some(channel) = ingress.and_then(|ingress| ingress.channel.as_deref()) {
            if let Some(target) = self.ctx.attachments.routing.channel_overrides.get(channel) {
                return ResolvedAttachmentTarget {
                    target: target.clone(),
                    policy_scope: format!("channel:{channel}"),
                    bot_identity,
                };
            }
        }
        if let Some(app_name) = self.ctx.app_name.as_deref() {
            if let Some(target) = self.ctx.attachments.routing.bot_overrides.get(app_name) {
                return ResolvedAttachmentTarget {
                    target: target.clone(),
                    policy_scope: format!("app:{app_name}:legacy"),
                    bot_identity,
                };
            }
        }
        if let Some(target) = Self::provider_attachment_target(base_profile) {
            return ResolvedAttachmentTarget {
                target,
                policy_scope: format!("profile:{}", base_profile.name),
                bot_identity,
            };
        }
        ResolvedAttachmentTarget {
            target: self.ctx.attachments.routing.default.clone(),
            policy_scope: "workspace:default".to_owned(),
            bot_identity,
        }
    }

    pub(crate) fn with_attachment_metadata(
        mut input: serde_json::Value,
        attachments: &[ChannelAttachment],
    ) -> serde_json::Value {
        if attachments.is_empty() {
            return input;
        }

        match input {
            serde_json::Value::Object(ref mut object) => {
                object.insert(
                    "attachments".to_owned(),
                    serde_json::to_value(attachments).unwrap_or_else(|_| serde_json::json!([])),
                );
                input
            }
            other => serde_json::json!({
                "input": other,
                "attachments": attachments,
            }),
        }
    }

    pub(crate) fn with_attachments_on_latest_user_message(
        messages: &mut [mosaic_provider::Message],
        attachments: &[ChannelAttachment],
    ) {
        if attachments.is_empty() {
            return;
        }

        if let Some(message) = messages
            .iter_mut()
            .rev()
            .find(|message| matches!(message.role, mosaic_provider::Role::User))
        {
            message.attachments = attachments.to_vec();
        }
    }

    pub(crate) fn attachment_requirements(
        attachments: &[ChannelAttachment],
    ) -> AttachmentRequirements {
        let mut requirements = AttachmentRequirements::default();
        for attachment in attachments {
            match attachment.kind {
                AttachmentKind::Image => requirements.requires_vision = true,
                AttachmentKind::Document => requirements.requires_documents = true,
                AttachmentKind::Audio => requirements.requires_audio = true,
                AttachmentKind::Video => requirements.requires_video = true,
                AttachmentKind::Other => {}
            }
        }
        requirements
    }

    fn provider_attachment_target(
        profile: &ProviderProfile,
    ) -> Option<AttachmentRoutingTargetConfig> {
        let routing = &profile.attachment_routing;
        let has_policy = routing.mode.is_some()
            || routing.processor.is_some()
            || routing.multimodal_profile.is_some()
            || routing.specialized_processor_profile.is_some()
            || !routing.allowed_attachment_kinds.is_empty()
            || routing.max_attachment_size_mb.is_some();
        if !has_policy {
            return None;
        }

        Some(Self::attachment_target_from_provider_routing(routing))
    }

    fn attachment_target_from_provider_routing(
        routing: &ProviderAttachmentRoutingConfig,
    ) -> AttachmentRoutingTargetConfig {
        AttachmentRoutingTargetConfig {
            mode: routing
                .mode
                .unwrap_or(AttachmentRouteModeConfig::ProviderNative),
            processor: routing.processor.clone(),
            multimodal_profile: routing.multimodal_profile.clone(),
            specialized_processor_profile: routing.specialized_processor_profile.clone(),
            allowed_attachment_kinds: routing.allowed_attachment_kinds.clone(),
            max_attachment_size_mb: routing.max_attachment_size_mb,
        }
    }

    fn attachment_kind_config_labels(target: &AttachmentRoutingTargetConfig) -> Vec<String> {
        target
            .allowed_attachment_kinds
            .iter()
            .map(|kind| kind.label().to_owned())
            .collect()
    }

    fn validate_attachment_target(
        &self,
        target: &AttachmentRoutingTargetConfig,
        attachments: &[ChannelAttachment],
        policy_scope: &str,
    ) -> Result<()> {
        if !target.allowed_attachment_kinds.is_empty() {
            let disallowed = attachments
                .iter()
                .filter(|attachment| {
                    !target
                        .allowed_attachment_kinds
                        .iter()
                        .copied()
                        .any(|kind| Self::attachment_kind_allowed(kind, attachment.kind))
                })
                .map(|attachment| attachment.kind.label().to_owned())
                .collect::<Vec<_>>();
            if !disallowed.is_empty() {
                bail!(
                    "attachment policy {} does not allow attachment kinds: {}",
                    policy_scope,
                    disallowed.join(", ")
                );
            }
        }

        if let Some(max_size_mb) = target.max_attachment_size_mb {
            let max_size_bytes = max_size_mb.saturating_mul(1024 * 1024);
            if let Some(attachment) = attachments.iter().find(|attachment| {
                attachment
                    .size_bytes
                    .is_some_and(|size_bytes| size_bytes > max_size_bytes)
            }) {
                bail!(
                    "attachment '{}' exceeds {} MB limit in {} policy",
                    attachment
                        .filename
                        .clone()
                        .unwrap_or_else(|| attachment.id.clone()),
                    max_size_mb,
                    policy_scope
                );
            }
        }

        Ok(())
    }

    fn attachment_kind_allowed(expected: AttachmentKindConfig, actual: AttachmentKind) -> bool {
        matches!(
            (expected, actual),
            (AttachmentKindConfig::Image, AttachmentKind::Image)
                | (AttachmentKindConfig::Document, AttachmentKind::Document)
                | (AttachmentKindConfig::Audio, AttachmentKind::Audio)
                | (AttachmentKindConfig::Video, AttachmentKind::Video)
                | (AttachmentKindConfig::Other, AttachmentKind::Other)
        )
    }
}
