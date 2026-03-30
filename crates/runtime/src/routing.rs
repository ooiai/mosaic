use anyhow::{Result, anyhow, bail};
use mosaic_inspect::{RouteDecisionTrace, RouteMode};
use mosaic_tool_core::{CapabilityExposure, ToolMetadata};

use super::*;

pub(crate) enum PlannedRoute {
    Assistant {
        reason: String,
    },
    Tool {
        name: String,
        reason: String,
        source: String,
    },
    Skill {
        name: String,
        reason: String,
        source: String,
    },
    Workflow {
        name: String,
        reason: String,
        source: String,
    },
}

impl PlannedRoute {
    pub(crate) fn mode(&self) -> RouteMode {
        match self {
            Self::Assistant { .. } => RouteMode::Assistant,
            Self::Tool { .. } => RouteMode::Tool,
            Self::Skill { .. } => RouteMode::Skill,
            Self::Workflow { .. } => RouteMode::Workflow,
        }
    }

    pub(crate) fn decision(&self, profile_used: Option<&str>) -> RouteDecisionTrace {
        let (
            selected_capability_type,
            selected_capability_name,
            selected_tool,
            selected_skill,
            selected_workflow,
            selection_reason,
            capability_source,
        ) = match self {
            Self::Assistant { reason } => (None, None, None, None, None, reason.clone(), None),
            Self::Tool {
                name,
                reason,
                source,
            } => (
                Some("tool".to_owned()),
                Some(name.clone()),
                Some(name.clone()),
                None,
                None,
                reason.clone(),
                Some(source.clone()),
            ),
            Self::Skill {
                name,
                reason,
                source,
            } => (
                Some("skill".to_owned()),
                Some(name.clone()),
                None,
                Some(name.clone()),
                None,
                reason.clone(),
                Some(source.clone()),
            ),
            Self::Workflow {
                name,
                reason,
                source,
            } => (
                Some("workflow".to_owned()),
                Some(name.clone()),
                None,
                None,
                Some(name.clone()),
                reason.clone(),
                Some(source.clone()),
            ),
        };

        RouteDecisionTrace {
            route_mode: self.mode(),
            selected_capability_type,
            selected_capability_name,
            selected_tool,
            selected_skill,
            selected_workflow,
            selection_reason,
            capability_source,
            profile_used: profile_used.map(ToOwned::to_owned),
            selected_category: None,
            catalog_scope: None,
        }
    }
}

impl AgentRuntime {
    pub(crate) fn plan_route(&self, req: &RunRequest) -> Result<PlannedRoute> {
        let explicit = [
            req.tool.as_ref().map(|_| "tool"),
            req.skill.as_ref().map(|_| "skill"),
            req.workflow.as_ref().map(|_| "workflow"),
        ]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();
        if explicit.len() > 1 {
            bail!(
                "cannot select multiple capability routes at once: {}",
                explicit.join(", ")
            );
        }

        let channel = Self::request_channel(req);

        if let Some(tool_name) = req.tool.as_ref() {
            let tool = self
                .ctx
                .tools
                .get(tool_name)
                .ok_or_else(|| anyhow!("tool not found: {}", tool_name))?;
            Self::ensure_explicit_access(tool_name, "tool", &tool.metadata().exposure, channel)?;
            return Ok(PlannedRoute::Tool {
                name: tool_name.clone(),
                reason: "explicit /mosaic tool command".to_owned(),
                source: tool.metadata().exposure.source.clone(),
            });
        }

        if let Some(skill_name) = req.skill.as_ref() {
            let skill = self
                .ctx
                .skills
                .get(skill_name)
                .ok_or_else(|| anyhow!("skill not found: {}", skill_name))?;
            Self::ensure_explicit_access(skill_name, "skill", &skill.metadata().exposure, channel)?;
            return Ok(PlannedRoute::Skill {
                name: skill_name.clone(),
                reason: "explicit /mosaic skill command".to_owned(),
                source: skill.metadata().exposure.source.clone(),
            });
        }

        if let Some(workflow_name) = req.workflow.as_ref() {
            let workflow = self
                .ctx
                .workflows
                .get_registered(workflow_name)
                .ok_or_else(|| anyhow!("workflow not found: {}", workflow_name))?;
            Self::ensure_explicit_access(
                workflow_name,
                "workflow",
                &workflow.metadata.exposure,
                channel,
            )?;
            return Ok(PlannedRoute::Workflow {
                name: workflow_name.clone(),
                reason: "explicit /mosaic workflow command".to_owned(),
                source: workflow.metadata.exposure.source.clone(),
            });
        }

        if let Some(workflow_route) = self.auto_route_workflow(&req.input, channel) {
            return Ok(workflow_route);
        }

        if let Some(skill_route) = self.auto_route_skill(&req.input, channel) {
            return Ok(skill_route);
        }

        Ok(PlannedRoute::Assistant {
            reason: "default assistant path: no conversational capability matched".to_owned(),
        })
    }

    pub(crate) fn parse_direct_tool_input(
        metadata: &ToolMetadata,
        input: &str,
    ) -> serde_json::Value {
        let trimmed = input.trim();
        if trimmed.is_empty() {
            return serde_json::json!({});
        }

        if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(trimmed) {
            return parsed;
        }

        if metadata.name == "exec_command" {
            let mut parts = trimmed.split_whitespace();
            let command = parts.next().unwrap_or_default();
            let args = parts.map(ToOwned::to_owned).collect::<Vec<_>>();
            return serde_json::json!({
                "command": command,
                "args": args,
            });
        }

        let required_properties = metadata
            .input_schema
            .get("required")
            .and_then(serde_json::Value::as_array)
            .map(|values| {
                values
                    .iter()
                    .filter_map(serde_json::Value::as_str)
                    .map(ToOwned::to_owned)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        if required_properties.len() == 1 {
            return serde_json::json!({ required_properties[0].clone(): trimmed });
        }

        let property_names = metadata
            .input_schema
            .get("properties")
            .and_then(serde_json::Value::as_object)
            .map(|properties| properties.keys().cloned().collect::<Vec<_>>())
            .unwrap_or_default();
        if property_names.len() == 1 {
            return serde_json::json!({ property_names[0].clone(): trimmed });
        }

        if property_names.iter().any(|name| name == "text") {
            return serde_json::json!({ "text": trimmed });
        }

        serde_json::json!({ "input": trimmed })
    }

    fn request_channel(req: &RunRequest) -> Option<&str> {
        req.ingress
            .as_ref()
            .and_then(|ingress| ingress.channel.as_deref())
    }

    fn ensure_explicit_access(
        name: &str,
        kind: &str,
        exposure: &CapabilityExposure,
        channel: Option<&str>,
    ) -> Result<()> {
        if !exposure.allows_explicit(channel) {
            let policy_hint = exposure
                .required_policy
                .as_deref()
                .map(|policy| format!(" required_policy={policy}"))
                .unwrap_or_default();
            let channels = if exposure.allowed_channels.is_empty() {
                "<all>".to_owned()
            } else {
                exposure.allowed_channels.join(", ")
            };
            bail!(
                "{} '{}' is not available in this session (visibility={}, invocation_mode={}, allowed_channels={}{} )",
                kind,
                name,
                exposure.visibility.label(),
                exposure.invocation_mode.label(),
                channels,
                policy_hint,
            );
        }
        Ok(())
    }

    fn auto_route_skill(&self, input: &str, channel: Option<&str>) -> Option<PlannedRoute> {
        self.ctx.skills.list().into_iter().find_map(|name| {
            let skill = self.ctx.skills.get(&name)?;
            if !skill.metadata().exposure.allows_conversational(channel) {
                return None;
            }
            Self::match_reason(input, &name, None).map(|reason| PlannedRoute::Skill {
                name,
                reason,
                source: skill.metadata().exposure.source.clone(),
            })
        })
    }

    fn auto_route_workflow(&self, input: &str, channel: Option<&str>) -> Option<PlannedRoute> {
        self.ctx.workflows.list().into_iter().find_map(|name| {
            let registered = self.ctx.workflows.get_registered(&name)?;
            if !registered.metadata.exposure.allows_conversational(channel) {
                return None;
            }
            Self::match_reason(
                input,
                &registered.workflow.name,
                registered.workflow.description.as_deref(),
            )
            .map(|reason| PlannedRoute::Workflow {
                name,
                reason,
                source: registered.metadata.exposure.source.clone(),
            })
        })
    }

    fn match_reason(input: &str, name: &str, description: Option<&str>) -> Option<String> {
        let normalized_input = normalize_phrase(input);
        let normalized_name = normalize_phrase(name);
        if normalized_name.len() > 2 && normalized_input.contains(&normalized_name) {
            return Some(format!(
                "automatic conversational route matched capability name '{}'",
                name
            ));
        }

        let mut matched_tokens = capability_tokens(name);
        if let Some(description) = description {
            matched_tokens.extend(capability_tokens(description));
        }
        matched_tokens.sort();
        matched_tokens.dedup();

        let input_tokens = capability_tokens(input);
        if let Some(token) = matched_tokens
            .into_iter()
            .find(|token| token.len() > 3 && input_tokens.iter().any(|value| value == token))
        {
            return Some(format!(
                "automatic conversational route matched keyword '{}' for '{}'",
                token, name
            ));
        }

        None
    }
}

fn normalize_phrase(value: &str) -> String {
    capability_tokens(value).join(" ")
}

fn capability_tokens(value: &str) -> Vec<String> {
    value
        .split(|ch: char| !ch.is_ascii_alphanumeric())
        .filter(|token| !token.is_empty())
        .map(|token| token.to_ascii_lowercase())
        .collect()
}
