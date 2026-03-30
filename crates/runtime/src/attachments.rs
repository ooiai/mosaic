use mosaic_config::{AttachmentRouteModeConfig, AttachmentRoutingTargetConfig};
use mosaic_inspect::{AttachmentRouteMode, AttachmentRouteTrace, ChannelAttachment};

use super::*;

pub(crate) struct ResolvedAttachmentRoute {
    pub(crate) trace: AttachmentRouteTrace,
    pub(crate) specialized_skill: Option<String>,
    pub(crate) requires_vision: bool,
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

        let resolved = match planned_route {
            crate::routing::PlannedRoute::Assistant { .. } => {
                let target = self.resolve_attachment_target(req, base_profile);
                match target.mode {
                    AttachmentRouteModeConfig::ProviderNative => ResolvedAttachmentRoute {
                        trace: AttachmentRouteTrace {
                            mode: AttachmentRouteMode::ProviderNative,
                            selection_reason: "attachment route resolved to provider_native"
                                .to_owned(),
                            provider_profile: None,
                            provider_model: None,
                            processor: None,
                            attachment_count: attachments.len(),
                            attachment_kinds,
                            attachment_filenames,
                            failure_summary,
                        },
                        specialized_skill: None,
                        requires_vision: true,
                    },
                    AttachmentRouteModeConfig::SpecializedProcessor => {
                        let processor = target.processor.clone().ok_or_else(|| {
                            anyhow!(
                                "attachment routing mode specialized_processor requires processor"
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
                                selection_reason:
                                    "attachment route resolved to specialized_processor".to_owned(),
                                provider_profile: None,
                                provider_model: None,
                                processor: Some(processor.clone()),
                                attachment_count: attachments.len(),
                                attachment_kinds,
                                attachment_filenames,
                                failure_summary,
                            },
                            specialized_skill: Some(processor),
                            requires_vision: false,
                        }
                    }
                    AttachmentRouteModeConfig::Disabled => {
                        bail!("attachment handling is disabled for this route")
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
                        provider_profile: None,
                        provider_model: None,
                        processor: Some(format!("tool:{name}")),
                        attachment_count: attachments.len(),
                        attachment_kinds,
                        attachment_filenames,
                        failure_summary,
                    },
                    specialized_skill: None,
                    requires_vision: false,
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
                        provider_profile: None,
                        provider_model: None,
                        processor: Some(format!("skill:{name}")),
                        attachment_count: attachments.len(),
                        attachment_kinds,
                        attachment_filenames,
                        failure_summary,
                    },
                    specialized_skill: None,
                    requires_vision: false,
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
                        provider_profile: None,
                        provider_model: None,
                        processor: Some(format!("workflow:{name}")),
                        attachment_count: attachments.len(),
                        attachment_kinds,
                        attachment_filenames,
                        failure_summary,
                    },
                    specialized_skill: None,
                    requires_vision: false,
                }
            }
        };

        Ok(Some(resolved))
    }

    pub(crate) fn resolve_attachment_target(
        &self,
        req: &RunRequest,
        base_profile: &ProviderProfile,
    ) -> AttachmentRoutingTargetConfig {
        if let Some(app_name) = self.ctx.app_name.as_deref() {
            if let Some(target) = self.ctx.attachments.routing.bot_overrides.get(app_name) {
                return target.clone();
            }
        }
        if let Some(channel) = req
            .ingress
            .as_ref()
            .and_then(|ingress| ingress.channel.as_deref())
        {
            if let Some(target) = self.ctx.attachments.routing.channel_overrides.get(channel) {
                return target.clone();
            }
        }
        if base_profile.attachment_routing.mode.is_some()
            || base_profile.attachment_routing.processor.is_some()
        {
            return AttachmentRoutingTargetConfig {
                mode: base_profile
                    .attachment_routing
                    .mode
                    .unwrap_or(AttachmentRouteModeConfig::ProviderNative),
                processor: base_profile.attachment_routing.processor.clone(),
            };
        }
        self.ctx.attachments.routing.default.clone()
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
}
