pub mod events;

use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

use anyhow::{Result, anyhow, bail};
use async_trait::async_trait;
use chrono::Utc;
use mosaic_inspect::{
    AttachmentRouteMode, CapabilityInvocationTrace, CompressionTrace, EffectiveProfileTrace,
    ExtensionTrace, ExtensionUsageTrace, IngressTrace, MemoryReadTrace, MemoryWriteTrace,
    ModelSelectionTrace, ProviderAttemptTrace, ProviderFailureTrace, RunFailureTrace, RunTrace,
    SkillTrace, ToolTrace, WorkflowStepTrace,
};
use mosaic_memory::{
    MemoryEntryKind, MemoryPolicy, MemoryStore, SessionMemoryRecord, compress_fragments,
    summarize_fragments,
};
use mosaic_node_protocol::{NodeRouter, NodeToolDispatchOutcome, NodeToolExecutionRequest};
use mosaic_provider::{
    LlmProvider, Message, ProviderAttempt, ProviderError, ProviderProfile, ProviderProfileRegistry,
    ProviderTransportMetadata, Role, SchedulingIntent, SchedulingRequest, ToolDefinition,
    tool_definition_from_metadata, tool_is_visible_to_model, validate_step_tools_support,
};
use mosaic_session_core::{SessionRecord, SessionStore, TranscriptRole, session_title_from_input};
use mosaic_skill_core::{SkillContext, SkillRegistry};
use mosaic_tool_core::{CapabilityAudit, ToolMetadata, ToolRegistry};
use mosaic_workflow::{
    Workflow, WorkflowObserver, WorkflowRegistry, WorkflowStep, WorkflowStepExecutor,
    WorkflowStepKind,
};
use tracing::{info, warn};
use uuid::Uuid;

use crate::events::{RunEvent, SharedRunEventSink};

mod attachments;
mod branches;
mod failure;
mod provider;
mod routing;
mod session;
#[cfg(test)]
mod tests;
mod tools;
mod types;
mod workflow;

pub use types::{AgentRuntime, RunError, RunRequest, RunResult, RuntimeContext};
use types::{
    NodeTraceContext, SharedCapabilityTraceCollector, SharedModelSelectionCollector,
    SharedSkillTraceCollector, SharedToolTraceCollector, ToolExecutionFailure,
    ToolExecutionOutcome,
};
use workflow::{push_capability_trace, push_skill_trace, push_tool_trace, update_skill_trace};

impl AgentRuntime {
    pub fn new(ctx: RuntimeContext) -> Self {
        Self { ctx }
    }

    fn emit(&self, event: RunEvent) {
        self.ctx.event_sink.emit(event);
    }

    pub async fn run(&self, req: RunRequest) -> std::result::Result<RunResult, RunError> {
        let mut trace = RunTrace::new_with_id(
            req.run_id
                .clone()
                .unwrap_or_else(|| Uuid::new_v4().to_string()),
            req.input.clone(),
        );
        info!(
            run_id = %trace.run_id,
            session_id = ?req.session_id,
            tool = ?req.tool,
            skill = ?req.skill,
            workflow = ?req.workflow,
            "runtime run started"
        );

        let requested_profile = req.profile.clone();
        let base_profile = match self.ctx.profiles.resolve(req.profile.as_deref()) {
            Ok(profile) => profile,
            Err(err) => return self.fail_run(trace, err),
        };
        if let Some(ingress) = req.ingress.clone() {
            trace.bind_ingress(ingress);
        }
        trace.bind_extensions(self.ctx.active_extensions.clone());

        let mut session = match self.load_session(&req, &base_profile, &mut trace) {
            Ok(session) => session,
            Err(err) => return self.fail_run(trace, err),
        };
        trace.mark_running();
        self.emit(RunEvent::RunStarted {
            run_id: trace.run_id.clone(),
            input: trace.input.clone(),
        });

        let mut planned_route = match self.plan_route(&req) {
            Ok(route) => route,
            Err(err) => return self.fail_run(trace, err),
        };
        let mut attachment_route =
            match self.resolve_attachment_route(&req, &planned_route, &base_profile) {
                Ok(route) => route,
                Err(err) => return self.fail_run(trace, err),
            };
        if let Some(route) = attachment_route.as_ref() {
            if route.trace.mode == AttachmentRouteMode::SpecializedProcessor {
                if let Some(skill_name) = route.specialized_skill.clone() {
                    planned_route = crate::routing::PlannedRoute::Skill {
                        name: skill_name,
                        reason: "attachment specialized processor".to_owned(),
                        source: "attachment.routing".to_owned(),
                    };
                }
            }
        }

        let (profile, selection_scope, selection_reason) = match &planned_route {
            crate::routing::PlannedRoute::Tool { .. }
            | crate::routing::PlannedRoute::Skill { .. } => (
                base_profile,
                format!("run:{}", planned_route.mode().label()),
                if requested_profile.is_some() {
                    "requested_profile"
                } else {
                    "active_profile"
                }
                .to_owned(),
            ),
            crate::routing::PlannedRoute::Workflow { .. }
            | crate::routing::PlannedRoute::Assistant { .. } => {
                let scheduling_intent =
                    if matches!(planned_route, crate::routing::PlannedRoute::Workflow { .. }) {
                        SchedulingIntent::WorkflowStep
                    } else {
                        SchedulingIntent::InteractiveRun
                    };
                let scheduled = match self.ctx.profiles.schedule(SchedulingRequest {
                    requested_profile: requested_profile.clone(),
                    channel: req
                        .ingress
                        .as_ref()
                        .and_then(|ingress| ingress.channel.clone()),
                    intent: scheduling_intent,
                    estimated_context_chars: req.input.chars().count()
                        + session
                            .as_ref()
                            .map(Self::session_context_chars)
                            .unwrap_or_default(),
                    requires_tools: matches!(
                        planned_route,
                        crate::routing::PlannedRoute::Assistant { .. }
                    ),
                    requires_vision: attachment_route
                        .as_ref()
                        .map(|route| route.requires_vision)
                        .unwrap_or(false),
                }) {
                    Ok(profile) => profile,
                    Err(err) => return self.fail_run(trace, err),
                };
                (
                    scheduled.profile,
                    format!("run:{}", planned_route.mode().label()),
                    scheduled.reason,
                )
            }
        };

        trace.add_model_selection(Self::model_selection_trace(
            selection_scope,
            requested_profile,
            &profile,
            selection_reason,
        ));
        if let Some(route) = attachment_route.as_mut() {
            if route.trace.mode == AttachmentRouteMode::ProviderNative {
                route.trace.provider_profile = Some(profile.name.clone());
                route.trace.provider_model = Some(profile.model.clone());
            }
            trace.bind_attachment_route(route.trace.clone());
        }
        trace.bind_effective_profile(Self::effective_profile_trace(
            &profile,
            &Self::provider_metadata_from_profile(&profile),
        ));
        trace.bind_runtime_policy(self.runtime_policy_trace());
        trace.bind_route_decision(planned_route.decision(Some(profile.name.as_str())));

        if let Some(session_ref) = session.as_mut() {
            if let Err(err) = self.rebind_session_profile(session_ref, &profile) {
                return self.fail_run(trace, err);
            }
        }

        match planned_route {
            crate::routing::PlannedRoute::Workflow { name, .. } => {
                self.run_workflow(req, name, profile, trace, session).await
            }
            crate::routing::PlannedRoute::Skill { name, .. } => {
                self.run_skill(req, name, trace, session).await
            }
            crate::routing::PlannedRoute::Tool { name, .. } => {
                self.run_tool(req, name, trace, session).await
            }
            crate::routing::PlannedRoute::Assistant { .. } => {
                self.run_plain_assistant(req, profile, trace, session).await
            }
        }
    }

    async fn execute_assistant_run(
        &self,
        system: Option<String>,
        input: String,
        attachments: &[mosaic_inspect::ChannelAttachment],
        reference_contexts: &[String],
        profile: &ProviderProfile,
        mut session: Option<&mut SessionRecord>,
        trace: &mut RunTrace,
    ) -> Result<String> {
        let provider = self.provider_for_profile(profile)?;
        let provider_metadata = provider.metadata();
        trace.bind_effective_profile(Self::effective_profile_trace(profile, &provider_metadata));
        let tool_defs = if profile.capabilities.supports_tools {
            self.collect_tool_definitions(
                None,
                trace
                    .ingress
                    .as_ref()
                    .and_then(|ingress| ingress.channel.as_deref()),
            )?
        } else {
            Vec::new()
        };
        let provider_tools = (!tool_defs.is_empty()).then_some(tool_defs.as_slice());

        let mut messages = if let Some(session_ref) = session.as_deref() {
            self.session_messages_for_provider(session_ref, reference_contexts, trace)?
        } else {
            let mut messages = Vec::new();
            if let Some(system) = system {
                messages.push(Message {
                    role: Role::System,
                    content: system,
                    tool_call_id: None,
                    attachments: Vec::new(),
                });
            }
            for reference_context in reference_contexts {
                messages.push(Message {
                    role: Role::System,
                    content: format!(
                        "Referenced session context:
{}",
                        reference_context
                    ),
                    tool_call_id: None,
                    attachments: Vec::new(),
                });
            }
            messages.push(Message {
                role: Role::User,
                content: input,
                tool_call_id: None,
                attachments: attachments.to_vec(),
            });
            messages
        };
        Self::with_attachments_on_latest_user_message(&mut messages, attachments);

        'provider_rounds: for _ in 0..self.ctx.runtime_policy.max_provider_round_trips {
            info!(
                run_id = %trace.run_id,
                provider_type = %provider_metadata.provider_type,
                profile = %profile.name,
                model = %profile.model,
                message_count = messages.len(),
                tool_count = tool_defs.len(),
                "provider request dispatched"
            );
            self.emit(RunEvent::ProviderRequest {
                provider_type: provider_metadata.provider_type.clone(),
                profile: profile.name.clone(),
                model: profile.model.clone(),
                tool_count: tool_defs.len(),
                message_count: messages.len(),
                max_attempts: provider_metadata.max_retries.saturating_add(1),
            });

            let completion = match provider.complete(&messages, provider_tools).await {
                Ok(completion) => completion,
                Err(error) => {
                    self.trace_provider_error(profile, trace, &error);
                    self.emit_provider_retry_events(profile, &error.attempts);
                    self.emit(RunEvent::ProviderFailed {
                        provider_type: profile.provider_type.clone(),
                        profile: profile.name.clone(),
                        model: profile.model.clone(),
                        kind: error.kind_label().to_owned(),
                        status_code: error.status_code,
                        retryable: error.retryable,
                        error: error.public_message().to_owned(),
                    });
                    return Err(anyhow::Error::new(error));
                }
            };
            self.trace_provider_attempts(trace, &completion.attempts);
            self.emit_provider_retry_events(profile, &completion.attempts);
            let response = completion.response;

            if !response.tool_calls.is_empty() {
                if let Some(shadow) = provider.tool_call_shadow_message(&response.tool_calls) {
                    messages.push(shadow);
                }
                for call in response.tool_calls {
                    let call_id = call.id.clone();
                    match self
                        .invoke_tool_with_guardrails(
                            session.as_ref().map(|record| record.id.as_str()),
                            call.name.clone(),
                            call_id.clone(),
                            call.arguments.clone(),
                        )
                        .await
                    {
                        Ok(outcome) => {
                            if let Some(tool) = self.ctx.tools.get(&call.name) {
                                if let Some(usage) = Self::tool_extension_usage(tool.metadata()) {
                                    trace.record_extension_usage(usage);
                                }
                            }
                            trace.tool_calls.push(outcome.tool_trace);
                            trace.add_capability_invocation(outcome.capability_trace);

                            if let Some(session_ref) = session.as_deref_mut() {
                                self.append_session_message(
                                    session_ref,
                                    TranscriptRole::Tool,
                                    outcome.output.clone(),
                                    Some(call_id.clone()),
                                )?;
                            }
                            messages.push(Message {
                                role: Role::Tool,
                                content: outcome.output,
                                tool_call_id: Some(call_id),
                                attachments: Vec::new(),
                            });
                        }
                        Err(failure) => {
                            if let Some(tool_trace) = failure.tool_trace {
                                trace.tool_calls.push(tool_trace);
                            }
                            if let Some(capability_trace) = failure.capability_trace {
                                trace.add_capability_invocation(capability_trace);
                            }
                            if self.ctx.runtime_policy.continue_after_tool_error {
                                messages.push(Message {
                                    role: Role::Tool,
                                    content: format!("tool_error: {}", failure.error),
                                    tool_call_id: Some(call_id),
                                    attachments: Vec::new(),
                                });
                                continue 'provider_rounds;
                            }
                            return Err(failure.error);
                        }
                    }
                }

                continue;
            }

            if let Some(message) = response.message {
                if let Some(session_ref) = session.as_deref_mut() {
                    self.append_session_message(
                        session_ref,
                        TranscriptRole::Assistant,
                        message.content.clone(),
                        None,
                    )?;
                }

                return Ok(message.content);
            }

            break;
        }

        Err(anyhow!("runtime stopped without final assistant message"))
    }

    async fn execute_workflow_prompt_step(
        &self,
        session_id: Option<&str>,
        profile: &ProviderProfile,
        system: Option<String>,
        input: String,
        attachments: &[mosaic_inspect::ChannelAttachment],
        tool_defs: Vec<ToolDefinition>,
        tool_traces: &SharedToolTraceCollector,
        capability_traces: &SharedCapabilityTraceCollector,
    ) -> Result<String> {
        let provider = self.provider_for_profile(profile)?;
        let provider_metadata = provider.metadata();
        let provider_tools = (!tool_defs.is_empty()).then_some(tool_defs.as_slice());
        let mut messages = Vec::new();

        if let Some(system) = system {
            messages.push(Message {
                role: Role::System,
                content: system,
                tool_call_id: None,
                attachments: Vec::new(),
            });
        }

        messages.push(Message {
            role: Role::User,
            content: input,
            tool_call_id: None,
            attachments: attachments.to_vec(),
        });

        'workflow_rounds: for _ in 0..self.ctx.runtime_policy.max_workflow_provider_round_trips {
            info!(
                provider_type = %provider_metadata.provider_type,
                profile = %profile.name,
                model = %profile.model,
                message_count = messages.len(),
                tool_count = tool_defs.len(),
                workflow_session_id = ?session_id,
                "workflow provider request dispatched"
            );
            self.emit(RunEvent::ProviderRequest {
                provider_type: provider_metadata.provider_type.clone(),
                profile: profile.name.clone(),
                model: profile.model.clone(),
                tool_count: tool_defs.len(),
                message_count: messages.len(),
                max_attempts: provider_metadata.max_retries.saturating_add(1),
            });

            let completion = match provider.complete(&messages, provider_tools).await {
                Ok(completion) => completion,
                Err(error) => {
                    self.emit_provider_retry_events(profile, &error.attempts);
                    self.emit(RunEvent::ProviderFailed {
                        provider_type: profile.provider_type.clone(),
                        profile: profile.name.clone(),
                        model: profile.model.clone(),
                        kind: error.kind_label().to_owned(),
                        status_code: error.status_code,
                        retryable: error.retryable,
                        error: error.public_message().to_owned(),
                    });
                    return Err(anyhow::Error::new(error));
                }
            };
            self.emit_provider_retry_events(profile, &completion.attempts);
            let response = completion.response;

            if !response.tool_calls.is_empty() {
                if let Some(shadow) = provider.tool_call_shadow_message(&response.tool_calls) {
                    messages.push(shadow);
                }
                for call in response.tool_calls {
                    let call_id = call.id.clone();
                    match self
                        .invoke_tool_with_guardrails(
                            session_id,
                            call.name.clone(),
                            call_id.clone(),
                            call.arguments.clone(),
                        )
                        .await
                    {
                        Ok(outcome) => {
                            push_tool_trace(tool_traces, outcome.tool_trace);
                            push_capability_trace(capability_traces, outcome.capability_trace);
                            messages.push(Message {
                                role: Role::Tool,
                                content: outcome.output,
                                tool_call_id: Some(call_id),
                                attachments: Vec::new(),
                            });
                        }
                        Err(failure) => {
                            if let Some(tool_trace) = failure.tool_trace {
                                push_tool_trace(tool_traces, tool_trace);
                            }
                            if let Some(capability_trace) = failure.capability_trace {
                                push_capability_trace(capability_traces, capability_trace);
                            }
                            if self.ctx.runtime_policy.continue_after_tool_error {
                                messages.push(Message {
                                    role: Role::Tool,
                                    content: format!("tool_error: {}", failure.error),
                                    tool_call_id: Some(call_id),
                                    attachments: Vec::new(),
                                });
                                continue 'workflow_rounds;
                            }
                            return Err(failure.error);
                        }
                    }
                }

                continue;
            }

            if let Some(message) = response.message {
                return Ok(message.content);
            }

            break;
        }

        Err(anyhow!("runtime stopped without final assistant message"))
    }

    async fn execute_skill_for_trace(
        &self,
        skill_name: String,
        input: String,
        attachments: &[mosaic_inspect::ChannelAttachment],
        trace: &mut RunTrace,
    ) -> Result<String> {
        let skill = self
            .ctx
            .skills
            .get(&skill_name)
            .ok_or_else(|| anyhow!("skill not found: {}", skill_name))?;
        if !attachments.is_empty() && !skill.metadata().exposure.accepts_attachments {
            bail!("skill '{}' does not accept attachments", skill_name);
        }

        self.emit(RunEvent::SkillStarted {
            name: skill_name.clone(),
        });

        let skill_input =
            Self::with_attachment_metadata(serde_json::json!({ "text": input }), attachments);
        trace.skill_calls.push(SkillTrace {
            name: skill_name.clone(),
            input: skill_input.clone(),
            output: None,
            started_at: Utc::now(),
            finished_at: None,
        });

        let ctx = SkillContext {
            tools: self.ctx.tools.clone(),
        };

        match skill.execute(skill_input, &ctx).await {
            Ok(output) => {
                if let Some(last) = trace.skill_calls.last_mut() {
                    last.output = Some(output.content.clone());
                    last.finished_at = Some(Utc::now());
                }

                self.emit(RunEvent::SkillFinished { name: skill_name });
                Ok(output.content)
            }
            Err(err) => {
                if let Some(last) = trace.skill_calls.last_mut() {
                    last.finished_at = Some(Utc::now());
                }

                self.emit(RunEvent::SkillFailed {
                    name: skill_name,
                    error: err.to_string(),
                });
                Err(err)
            }
        }
    }

    async fn execute_workflow_skill_step(
        &self,
        skill_name: String,
        input: String,
        attachments: &[mosaic_inspect::ChannelAttachment],
        skill_traces: &SharedSkillTraceCollector,
    ) -> Result<String> {
        let skill = self
            .ctx
            .skills
            .get(&skill_name)
            .ok_or_else(|| anyhow!("skill not found: {}", skill_name))?;
        if !attachments.is_empty() && !skill.metadata().exposure.accepts_attachments {
            bail!("skill '{}' does not accept attachments", skill_name);
        }

        self.emit(RunEvent::SkillStarted {
            name: skill_name.clone(),
        });

        let skill_input =
            Self::with_attachment_metadata(serde_json::json!({ "text": input }), attachments);
        push_skill_trace(
            skill_traces,
            SkillTrace {
                name: skill_name.clone(),
                input: skill_input.clone(),
                output: None,
                started_at: Utc::now(),
                finished_at: None,
            },
        );

        let ctx = SkillContext {
            tools: self.ctx.tools.clone(),
        };

        match skill.execute(skill_input, &ctx).await {
            Ok(output) => {
                update_skill_trace(skill_traces, &skill_name, |trace| {
                    trace.output = Some(output.content.clone());
                    trace.finished_at = Some(Utc::now());
                });

                self.emit(RunEvent::SkillFinished { name: skill_name });
                Ok(output.content)
            }
            Err(err) => {
                update_skill_trace(skill_traces, &skill_name, |trace| {
                    trace.finished_at = Some(Utc::now());
                });

                self.emit(RunEvent::SkillFailed {
                    name: skill_name,
                    error: err.to_string(),
                });
                Err(err)
            }
        }
    }

    fn tool_extension_usage(metadata: &ToolMetadata) -> Option<ExtensionUsageTrace> {
        Some(ExtensionUsageTrace {
            name: metadata.extension.clone()?,
            version: metadata
                .version
                .clone()
                .unwrap_or_else(|| "unknown".to_owned()),
            component_kind: "tool".to_owned(),
            component_name: metadata.name.clone(),
        })
    }

    fn skill_extension_usage(
        metadata: &mosaic_skill_core::SkillMetadata,
    ) -> Option<ExtensionUsageTrace> {
        Some(ExtensionUsageTrace {
            name: metadata.extension.clone()?,
            version: metadata
                .version
                .clone()
                .unwrap_or_else(|| "unknown".to_owned()),
            component_kind: "skill".to_owned(),
            component_name: metadata.name.clone(),
        })
    }

    fn workflow_extension_usage(
        metadata: &mosaic_workflow::WorkflowMetadata,
    ) -> Option<ExtensionUsageTrace> {
        Some(ExtensionUsageTrace {
            name: metadata.extension.clone()?,
            version: metadata
                .version
                .clone()
                .unwrap_or_else(|| "unknown".to_owned()),
            component_kind: "workflow".to_owned(),
            component_name: metadata.name.clone(),
        })
    }
}
