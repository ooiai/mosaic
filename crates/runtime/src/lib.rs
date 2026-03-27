pub mod events;

use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

use anyhow::{Result, anyhow, bail};
use async_trait::async_trait;
use chrono::Utc;
use mosaic_inspect::{
    CapabilityInvocationTrace, CompressionTrace, EffectiveProfileTrace, ExtensionTrace,
    ExtensionUsageTrace, IngressTrace, MemoryReadTrace, MemoryWriteTrace, ModelSelectionTrace,
    ProviderAttemptTrace, ProviderFailureTrace, RunFailureTrace, RunTrace, SkillTrace, ToolTrace,
    WorkflowStepTrace,
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
    Workflow, WorkflowObserver, WorkflowRegistry, WorkflowRunner, WorkflowStep,
    WorkflowStepExecutor, WorkflowStepKind,
};
use tracing::{info, warn};
use uuid::Uuid;

use crate::events::{RunEvent, SharedRunEventSink};

#[cfg(test)]
mod tests;
mod types;
mod workflow;

pub use types::{AgentRuntime, RunError, RunRequest, RunResult, RuntimeContext};
use types::{
    NodeTraceContext, SharedCapabilityTraceCollector, SharedModelSelectionCollector,
    SharedSkillTraceCollector, SharedToolTraceCollector, ToolExecutionFailure,
    ToolExecutionOutcome,
};
use workflow::{
    RuntimeWorkflowExecutor, RuntimeWorkflowObserver, drain_capability_trace_collector,
    drain_model_selection_collector, drain_skill_trace_collector, drain_tool_trace_collector,
    push_capability_trace, push_skill_trace, push_tool_trace, update_skill_trace,
};

impl AgentRuntime {
    pub fn new(ctx: RuntimeContext) -> Self {
        Self { ctx }
    }

    fn emit(&self, event: RunEvent) {
        self.ctx.event_sink.emit(event);
    }

    fn truncate_preview(value: &str, limit: usize) -> String {
        if value.chars().count() <= limit {
            return value.to_owned();
        }

        let truncated: String = value.chars().take(limit).collect();
        format!("{truncated}...")
    }

    fn failure_trace(trace: &RunTrace, error: &anyhow::Error) -> RunFailureTrace {
        if let Some(provider_error) = error.downcast_ref::<ProviderError>() {
            return RunFailureTrace {
                kind: "provider".to_owned(),
                stage: if trace.workflow_name.is_some() {
                    "workflow_provider"
                } else {
                    "provider"
                }
                .to_owned(),
                retryable: provider_error.retryable,
                message: provider_error.public_message().to_owned(),
            };
        }

        if let Some(provider_failure) = trace.provider_failure.as_ref() {
            return RunFailureTrace {
                kind: "provider".to_owned(),
                stage: if trace.workflow_name.is_some() {
                    "workflow_provider"
                } else {
                    "provider"
                }
                .to_owned(),
                retryable: provider_failure.retryable,
                message: provider_failure.message.clone(),
            };
        }

        if trace
            .capability_invocations
            .iter()
            .any(|invocation| invocation.status != "success")
            || trace.tool_calls.iter().any(|call| call.output.is_some())
        {
            return RunFailureTrace {
                kind: "tool".to_owned(),
                stage: if trace.workflow_name.is_some() {
                    "workflow_tool"
                } else {
                    "tool"
                }
                .to_owned(),
                retryable: true,
                message: error.to_string(),
            };
        }

        if trace.step_traces.iter().any(|step| step.error.is_some()) {
            return RunFailureTrace {
                kind: "workflow".to_owned(),
                stage: "workflow".to_owned(),
                retryable: true,
                message: error.to_string(),
            };
        }

        let message = error.to_string();
        let lower = message.to_ascii_lowercase();
        let (kind, stage, retryable) =
            if lower.contains("memory") || lower.contains("compressed context") {
                ("memory", "memory", true)
            } else if lower.contains("session") || lower.contains("transcript") {
                ("session", "session", true)
            } else if lower.contains("cannot select both skill and workflow")
                || lower.contains("workflow not found")
                || lower.contains("skill not found")
                || lower.contains("unknown provider profile")
            {
                ("validation", "validation", false)
            } else {
                ("runtime", "runtime", false)
            };

        RunFailureTrace {
            kind: kind.to_owned(),
            stage: stage.to_owned(),
            retryable,
            message,
        }
    }

    fn emit_output_deltas(&self, trace: &mut RunTrace, output: &str) {
        let mut chars = output.chars();
        let mut emitted = 0usize;
        loop {
            let chunk: String = chars.by_ref().take(80).collect();
            if chunk.is_empty() {
                break;
            }
            emitted += chunk.chars().count();
            trace.record_output_chunk();
            self.emit(RunEvent::OutputDelta {
                run_id: trace.run_id.clone(),
                chunk,
                accumulated_chars: emitted,
            });
        }
    }

    fn fail_run<T>(
        &self,
        mut trace: RunTrace,
        error: anyhow::Error,
    ) -> std::result::Result<T, RunError> {
        let message = error.to_string();
        let failure = Self::failure_trace(&trace, &error);

        warn!(
            run_id = %trace.run_id,
            session_id = ?trace.session_id,
            failure_kind = %failure.kind,
            failure_stage = %failure.stage,
            error = %message,
            "runtime run failed"
        );
        trace.bind_failure(failure.clone());
        trace.finish_err(message.clone());
        self.emit(RunEvent::RunFailed {
            run_id: trace.run_id.clone(),
            error: message,
            failure_kind: Some(failure.kind),
        });

        Err(RunError::new(error, trace))
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
            skill = ?req.skill,
            workflow = ?req.workflow,
            "runtime run started"
        );

        if req.skill.is_some() && req.workflow.is_some() {
            return self.fail_run(trace, anyhow!("cannot select both skill and workflow"));
        }

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

        let (profile, selection_scope, selection_reason) = if req.skill.is_some() {
            (
                base_profile,
                "run:skill".to_owned(),
                if requested_profile.is_some() {
                    "requested_profile"
                } else {
                    "active_profile"
                }
                .to_owned(),
            )
        } else {
            let scheduling_intent = if req.workflow.is_some() {
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
                requires_tools: req.skill.is_none(),
            }) {
                Ok(profile) => profile,
                Err(err) => return self.fail_run(trace, err),
            };
            (
                scheduled.profile,
                if req.workflow.is_some() {
                    "run:workflow"
                } else {
                    "run:assistant"
                }
                .to_owned(),
                scheduled.reason,
            )
        };

        trace.add_model_selection(Self::model_selection_trace(
            selection_scope,
            requested_profile,
            &profile,
            selection_reason,
        ));
        trace.bind_effective_profile(Self::effective_profile_trace(
            &profile,
            &Self::provider_metadata_from_profile(&profile),
        ));

        if let Some(session_ref) = session.as_mut() {
            if let Err(err) = self.rebind_session_profile(session_ref, &profile) {
                return self.fail_run(trace, err);
            }
        }

        if let Some(workflow_name) = req.workflow.clone() {
            return self
                .run_workflow(req, workflow_name, profile, trace, session)
                .await;
        }

        if let Some(skill_name) = req.skill.clone() {
            return self.run_skill(req, skill_name, trace, session).await;
        }

        self.run_plain_assistant(req, profile, trace, session).await
    }

    async fn run_plain_assistant(
        &self,
        req: RunRequest,
        profile: ProviderProfile,
        mut trace: RunTrace,
        mut session: Option<SessionRecord>,
    ) -> std::result::Result<RunResult, RunError> {
        let reference_contexts =
            match self.resolve_cross_session_contexts(session.as_mut(), &req.input, &mut trace) {
                Ok(contexts) => contexts,
                Err(err) => return self.fail_run(trace, err),
            };

        if let Some(session_ref) = session.as_mut() {
            if let Err(err) = self.append_session_message(
                session_ref,
                TranscriptRole::User,
                req.input.clone(),
                None,
            ) {
                return self.fail_run(trace, err);
            }
        }

        let output = match self
            .execute_assistant_run(
                req.system,
                req.input,
                &reference_contexts,
                &profile,
                session.as_mut(),
                &mut trace,
            )
            .await
        {
            Ok(output) => output,
            Err(err) => return self.fail_run(trace, err),
        };

        if let Some(session_ref) = session.as_mut() {
            if let Err(err) = self.persist_session_memory(session_ref, &mut trace) {
                return self.fail_run(trace, err);
            }
        }

        self.emit_output_deltas(&mut trace, &output);
        self.emit(RunEvent::FinalAnswerReady {
            run_id: trace.run_id.clone(),
        });
        self.emit(RunEvent::RunFinished {
            run_id: trace.run_id.clone(),
            output_preview: Self::truncate_preview(&output, 120),
        });
        trace.finish_ok(output.clone());

        Ok(RunResult { output, trace })
    }

    async fn run_skill(
        &self,
        req: RunRequest,
        skill_name: String,
        mut trace: RunTrace,
        mut session: Option<SessionRecord>,
    ) -> std::result::Result<RunResult, RunError> {
        let reference_contexts =
            match self.resolve_cross_session_contexts(session.as_mut(), &req.input, &mut trace) {
                Ok(contexts) => contexts,
                Err(err) => return self.fail_run(trace, err),
            };
        let skill_input =
            Self::augment_input_with_reference_context(&req.input, &reference_contexts);

        if let Some(session_ref) = session.as_mut() {
            if let Err(err) = self.append_session_message(
                session_ref,
                TranscriptRole::User,
                req.input.clone(),
                None,
            ) {
                return self.fail_run(trace, err);
            }
        }

        if let Some(skill) = self.ctx.skills.get(&skill_name) {
            if let Some(usage) = Self::skill_extension_usage(skill.metadata()) {
                trace.record_extension_usage(usage);
            }
        }

        let output = match self
            .execute_skill_for_trace(skill_name.clone(), skill_input, &mut trace)
            .await
        {
            Ok(output) => output,
            Err(err) => return self.fail_run(trace, err),
        };

        if let Some(session_ref) = session.as_mut() {
            if let Err(err) = self.append_session_message(
                session_ref,
                TranscriptRole::Assistant,
                output.clone(),
                None,
            ) {
                return self.fail_run(trace, err);
            }
            if let Err(err) = self.persist_session_memory(session_ref, &mut trace) {
                return self.fail_run(trace, err);
            }
        }

        self.emit_output_deltas(&mut trace, &output);
        self.emit(RunEvent::FinalAnswerReady {
            run_id: trace.run_id.clone(),
        });
        self.emit(RunEvent::RunFinished {
            run_id: trace.run_id.clone(),
            output_preview: Self::truncate_preview(&output, 120),
        });
        trace.finish_ok(output.clone());

        Ok(RunResult { output, trace })
    }

    async fn run_workflow(
        &self,
        req: RunRequest,
        workflow_name: String,
        profile: ProviderProfile,
        mut trace: RunTrace,
        mut session: Option<SessionRecord>,
    ) -> std::result::Result<RunResult, RunError> {
        let workflow = match self.ctx.workflows.get(&workflow_name) {
            Some(workflow) => workflow.clone(),
            None => return self.fail_run(trace, anyhow!("workflow not found: {}", workflow_name)),
        };
        if let Some(metadata) = self.ctx.workflows.metadata(&workflow_name) {
            if let Some(usage) = Self::workflow_extension_usage(metadata) {
                trace.record_extension_usage(usage);
            }
        }
        let reference_contexts =
            match self.resolve_cross_session_contexts(session.as_mut(), &req.input, &mut trace) {
                Ok(contexts) => contexts,
                Err(err) => return self.fail_run(trace, err),
            };

        if let Some(session_ref) = session.as_mut() {
            if let Err(err) = self.append_session_message(
                session_ref,
                TranscriptRole::User,
                req.input.clone(),
                None,
            ) {
                return self.fail_run(trace, err);
            }
        }

        let tool_traces = Arc::new(Mutex::new(Vec::new()));
        let skill_traces = Arc::new(Mutex::new(Vec::new()));
        let model_selections = Arc::new(Mutex::new(Vec::new()));
        let capability_traces = Arc::new(Mutex::new(Vec::new()));
        let runner = WorkflowRunner::new();
        let workflow_input =
            Self::augment_input_with_reference_context(&req.input, &reference_contexts);

        let workflow_result = {
            let executor = RuntimeWorkflowExecutor {
                runtime: self,
                default_profile: profile,
                session_id: req.session_id.clone(),
                ingress_channel: req
                    .ingress
                    .as_ref()
                    .and_then(|ingress| ingress.channel.clone()),
                tool_traces: tool_traces.clone(),
                skill_traces: skill_traces.clone(),
                model_selections: model_selections.clone(),
                capability_traces: capability_traces.clone(),
            };
            let mut observer = RuntimeWorkflowObserver {
                runtime: self,
                trace: &mut trace,
            };

            runner
                .run_with_observer(&workflow, workflow_input, &executor, &mut observer)
                .await
        };

        trace
            .tool_calls
            .extend(drain_tool_trace_collector(&tool_traces));
        trace
            .skill_calls
            .extend(drain_skill_trace_collector(&skill_traces));
        trace
            .model_selections
            .extend(drain_model_selection_collector(&model_selections));
        for capability_trace in drain_capability_trace_collector(&capability_traces) {
            trace.add_capability_invocation(capability_trace);
        }

        let output = match workflow_result {
            Ok(result) => result.output,
            Err(err) => return self.fail_run(trace, err),
        };

        if let Some(session_ref) = session.as_mut() {
            if let Err(err) = self.append_session_message(
                session_ref,
                TranscriptRole::Assistant,
                output.clone(),
                None,
            ) {
                return self.fail_run(trace, err);
            }
            if let Err(err) = self.persist_session_memory(session_ref, &mut trace) {
                return self.fail_run(trace, err);
            }
        }

        self.emit_output_deltas(&mut trace, &output);
        self.emit(RunEvent::FinalAnswerReady {
            run_id: trace.run_id.clone(),
        });
        self.emit(RunEvent::RunFinished {
            run_id: trace.run_id.clone(),
            output_preview: Self::truncate_preview(&output, 120),
        });
        trace.finish_ok(output.clone());

        Ok(RunResult { output, trace })
    }

    async fn execute_assistant_run(
        &self,
        system: Option<String>,
        input: String,
        reference_contexts: &[String],
        profile: &ProviderProfile,
        mut session: Option<&mut SessionRecord>,
        trace: &mut RunTrace,
    ) -> Result<String> {
        let provider = self.provider_for_profile(profile)?;
        let provider_metadata = provider.metadata();
        trace.bind_effective_profile(Self::effective_profile_trace(profile, &provider_metadata));
        let tool_defs = if profile.capabilities.supports_tools {
            self.collect_tool_definitions(None)?
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
                });
            }
            messages.push(Message {
                role: Role::User,
                content: input,
                tool_call_id: None,
            });
            messages
        };

        for _ in 0..8 {
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
                            });
                        }
                        Err(failure) => {
                            if let Some(tool_trace) = failure.tool_trace {
                                trace.tool_calls.push(tool_trace);
                            }
                            if let Some(capability_trace) = failure.capability_trace {
                                trace.add_capability_invocation(capability_trace);
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
            });
        }

        messages.push(Message {
            role: Role::User,
            content: input,
            tool_call_id: None,
        });

        for _ in 0..8 {
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
                            });
                        }
                        Err(failure) => {
                            if let Some(tool_trace) = failure.tool_trace {
                                push_tool_trace(tool_traces, tool_trace);
                            }
                            if let Some(capability_trace) = failure.capability_trace {
                                push_capability_trace(capability_traces, capability_trace);
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
        trace: &mut RunTrace,
    ) -> Result<String> {
        let skill = self
            .ctx
            .skills
            .get(&skill_name)
            .ok_or_else(|| anyhow!("skill not found: {}", skill_name))?;

        self.emit(RunEvent::SkillStarted {
            name: skill_name.clone(),
        });

        let skill_input = serde_json::json!({ "text": input });
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
        skill_traces: &SharedSkillTraceCollector,
    ) -> Result<String> {
        let skill = self
            .ctx
            .skills
            .get(&skill_name)
            .ok_or_else(|| anyhow!("skill not found: {}", skill_name))?;

        self.emit(RunEvent::SkillStarted {
            name: skill_name.clone(),
        });

        let skill_input = serde_json::json!({ "text": input });
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

    fn provider_for_profile(&self, profile: &ProviderProfile) -> Result<Arc<dyn LlmProvider>> {
        match self.ctx.provider_override.clone() {
            Some(provider) => Ok(provider),
            None => self.ctx.profiles.build_provider(Some(&profile.name)),
        }
    }

    fn load_session(
        &self,
        req: &RunRequest,
        profile: &ProviderProfile,
        trace: &mut RunTrace,
    ) -> Result<Option<SessionRecord>> {
        let Some(session_id) = req.session_id.as_deref() else {
            return Ok(None);
        };

        trace.bind_session(session_id.to_owned());

        let mut session = match self.ctx.session_store.load(session_id)? {
            Some(session) => session,
            None => SessionRecord::new(
                session_id,
                session_title_from_input(&req.input),
                profile.name.clone(),
                profile.provider_type.clone(),
                profile.model.clone(),
            ),
        };

        session.set_runtime_binding(
            profile.name.clone(),
            profile.provider_type.clone(),
            profile.model.clone(),
        );
        if let Some(ingress) = req.ingress.as_ref() {
            session.bind_ingress_context(ingress);
        }
        session.set_last_run_id(trace.run_id.clone());

        if session.transcript.is_empty() {
            if let Some(system) = req.system.as_ref() {
                session.append_message(TranscriptRole::System, system.clone(), None);
            }
        }

        self.ctx.session_store.save(&session)?;
        Ok(Some(session))
    }

    fn append_session_message(
        &self,
        session: &mut SessionRecord,
        role: TranscriptRole,
        content: impl Into<String>,
        tool_call_id: Option<String>,
    ) -> Result<()> {
        session.append_message(role, content, tool_call_id);
        self.ctx.session_store.save(session)?;
        Ok(())
    }

    fn session_messages(session: &SessionRecord) -> Vec<Message> {
        session
            .transcript
            .iter()
            .map(|message| Message {
                role: match message.role {
                    TranscriptRole::System => Role::System,
                    TranscriptRole::User => Role::User,
                    TranscriptRole::Assistant => Role::Assistant,
                    TranscriptRole::Tool => Role::Tool,
                },
                content: message.content.clone(),
                tool_call_id: message.tool_call_id.clone(),
            })
            .collect()
    }

    fn session_context_chars(session: &SessionRecord) -> usize {
        session
            .transcript
            .iter()
            .map(|message| message.content.chars().count())
            .sum()
    }

    fn transcript_fragments(session: &SessionRecord) -> Vec<String> {
        session
            .transcript
            .iter()
            .map(|message| {
                format!(
                    "{}: {}",
                    match message.role {
                        TranscriptRole::System => "system",
                        TranscriptRole::User => "user",
                        TranscriptRole::Assistant => "assistant",
                        TranscriptRole::Tool => "tool",
                    },
                    message.content
                )
            })
            .collect()
    }

    fn extract_session_references(input: &str) -> Vec<String> {
        let mut references = Vec::new();
        let mut remaining = input;
        let prefix = "[[session:";

        while let Some(start) = remaining.find(prefix) {
            let candidate = &remaining[start + prefix.len()..];
            let Some(end) = candidate.find("]]") else {
                break;
            };
            let session_id = candidate[..end].trim();
            if !session_id.is_empty() && !references.iter().any(|existing| existing == session_id) {
                references.push(session_id.to_owned());
            }
            remaining = &candidate[end + 2..];
        }

        references
    }

    fn augment_input_with_reference_context(input: &str, reference_contexts: &[String]) -> String {
        if reference_contexts.is_empty() {
            return input.to_owned();
        }

        format!(
            "{}

Referenced session context:
{}",
            input,
            reference_contexts
                .iter()
                .map(|context| format!("- {}", context))
                .collect::<Vec<_>>()
                .join(
                    "
"
                )
        )
    }

    fn lookup_session_reference_summary(&self, session_id: &str) -> Result<Option<String>> {
        if let Some(memory) = self.ctx.memory_store.load_session(session_id)? {
            if let Some(summary) = memory.summary.or(memory.compressed_context) {
                return Ok(Some(summary));
            }
        }

        let Some(session) = self.ctx.session_store.load(session_id)? else {
            return Ok(None);
        };
        let fragments = Self::transcript_fragments(&session);
        if fragments.is_empty() {
            return Ok(None);
        }

        Ok(Some(summarize_fragments(
            &fragments,
            self.ctx.memory_policy.note_char_budget,
        )))
    }

    fn resolve_cross_session_contexts(
        &self,
        mut session: Option<&mut SessionRecord>,
        input: &str,
        trace: &mut RunTrace,
    ) -> Result<Vec<String>> {
        let mut contexts = Vec::new();
        for session_id in Self::extract_session_references(input) {
            let Some(summary) = self.lookup_session_reference_summary(&session_id)? else {
                continue;
            };
            trace.add_memory_read(MemoryReadTrace {
                session_id: session_id.clone(),
                source: "cross_session_reference".to_owned(),
                preview: Self::truncate_preview(&summary, 180),
                tags: vec!["explicit_reference".to_owned()],
            });

            if let Some(session_ref) = session.as_deref_mut() {
                session_ref.record_reference(session_id.clone(), "explicit_session_reference");
                self.ctx.session_store.save(session_ref)?;
            }

            contexts.push(format!("session {} => {}", session_id, summary));
        }

        Ok(contexts)
    }

    fn session_messages_for_provider(
        &self,
        session: &SessionRecord,
        reference_contexts: &[String],
        trace: &mut RunTrace,
    ) -> Result<Vec<Message>> {
        let transcript_messages = Self::session_messages(session);
        let mut reference_messages = reference_contexts
            .iter()
            .map(|context| Message {
                role: Role::System,
                content: format!(
                    "Referenced session context:
{}",
                    context
                ),
                tool_call_id: None,
            })
            .collect::<Vec<_>>();

        if let Some(summary) = session.memory.latest_summary.as_deref() {
            trace.add_memory_read(MemoryReadTrace {
                session_id: session.id.clone(),
                source: "session_summary".to_owned(),
                preview: Self::truncate_preview(summary, 180),
                tags: vec!["session".to_owned()],
            });
        }

        let compression = compress_fragments(
            &Self::transcript_fragments(session),
            &self.ctx.memory_policy,
        );
        if !compression.compressed {
            let mut messages = transcript_messages;
            let insert_at = if matches!(
                messages.first().map(|message| message.role),
                Some(Role::System)
            ) {
                1
            } else {
                0
            };
            messages.splice(insert_at..insert_at, reference_messages.drain(..));
            return Ok(messages);
        }

        let summary = session
            .memory
            .latest_summary
            .clone()
            .unwrap_or_else(|| compression.summary.clone());
        let mut messages = Vec::new();
        let mut recent_messages = transcript_messages;
        if matches!(
            recent_messages.first().map(|message| message.role),
            Some(Role::System)
        ) {
            messages.push(recent_messages.remove(0));
        }
        messages.push(Message {
            role: Role::System,
            content: format!(
                "Compressed conversation summary:
{}",
                summary
            ),
            tool_call_id: None,
        });
        if let Some(compressed_context) = session.memory.compressed_context.as_deref() {
            trace.add_memory_read(MemoryReadTrace {
                session_id: session.id.clone(),
                source: "compressed_context".to_owned(),
                preview: Self::truncate_preview(compressed_context, 180),
                tags: vec!["compression".to_owned()],
            });
        }
        trace.bind_compression(CompressionTrace {
            original_message_count: compression.original_message_count,
            kept_recent_count: compression.kept_recent_count,
            summary_preview: Self::truncate_preview(&compression.summary, 180),
        });
        messages.extend(reference_messages);
        let recent_start = recent_messages
            .len()
            .saturating_sub(compression.kept_recent_count);
        messages.extend(recent_messages.into_iter().skip(recent_start));
        Ok(messages)
    }

    fn persist_session_memory(
        &self,
        session: &mut SessionRecord,
        trace: &mut RunTrace,
    ) -> Result<()> {
        let fragments = Self::transcript_fragments(session);
        if fragments.is_empty() {
            return Ok(());
        }

        let summary = summarize_fragments(&fragments, self.ctx.memory_policy.summary_char_budget);
        let compression = compress_fragments(&fragments, &self.ctx.memory_policy);
        let compressed_context = compression.compressed.then(|| compression.summary.clone());
        let mut record = self
            .ctx
            .memory_store
            .load_session(&session.id)?
            .unwrap_or_else(|| SessionMemoryRecord::new(session.id.clone()));
        record.set_summary(Some(summary.clone()));
        record.set_compressed_context(compressed_context.clone());
        record.record_entry(
            MemoryEntryKind::Summary,
            summary.clone(),
            vec!["session_summary".to_owned()],
        );
        trace.add_memory_write(MemoryWriteTrace {
            session_id: session.id.clone(),
            kind: "summary".to_owned(),
            preview: Self::truncate_preview(&summary, 180),
            tags: vec!["session".to_owned()],
        });
        if let Some(compressed) = compressed_context.clone() {
            record.record_entry(
                MemoryEntryKind::Compression,
                compressed.clone(),
                vec!["compressed_context".to_owned()],
            );
            trace.add_memory_write(MemoryWriteTrace {
                session_id: session.id.clone(),
                kind: "compression".to_owned(),
                preview: Self::truncate_preview(&compressed, 180),
                tags: vec!["compression".to_owned()],
            });
        }
        for reference in &session.references {
            if !record.related_sessions.contains(&reference.session_id) {
                record.link_session(reference.session_id.clone());
                record.record_entry(
                    MemoryEntryKind::CrossSession,
                    format!("{} ({})", reference.session_id, reference.reason),
                    vec!["cross_session".to_owned()],
                );
                trace.add_memory_write(MemoryWriteTrace {
                    session_id: session.id.clone(),
                    kind: "cross_session".to_owned(),
                    preview: format!("{} ({})", reference.session_id, reference.reason),
                    tags: vec!["cross_session".to_owned()],
                });
            }
        }
        self.ctx.memory_store.save_session(&record)?;

        session.set_memory_state(
            Some(summary),
            compressed_context,
            record.entries.len(),
            compression.compressed,
        );
        self.ctx.session_store.save(session)?;
        Ok(())
    }

    fn rebind_session_profile(
        &self,
        session: &mut SessionRecord,
        profile: &ProviderProfile,
    ) -> Result<()> {
        session.set_runtime_binding(
            profile.name.clone(),
            profile.provider_type.clone(),
            profile.model.clone(),
        );
        self.ctx.session_store.save(session)?;
        Ok(())
    }

    fn model_selection_trace(
        scope: impl Into<String>,
        requested_profile: Option<String>,
        profile: &ProviderProfile,
        reason: impl Into<String>,
    ) -> ModelSelectionTrace {
        ModelSelectionTrace {
            scope: scope.into(),
            requested_profile,
            selected_profile: profile.name.clone(),
            selected_model: profile.model.clone(),
            reason: reason.into(),
            context_window_chars: profile.capabilities.context_window_chars,
            budget_tier: profile.capabilities.budget_tier.clone(),
        }
    }

    fn effective_profile_trace(
        profile: &ProviderProfile,
        metadata: &ProviderTransportMetadata,
    ) -> EffectiveProfileTrace {
        EffectiveProfileTrace {
            profile: profile.name.clone(),
            provider_type: profile.provider_type.clone(),
            model: profile.model.clone(),
            base_url: metadata
                .base_url
                .clone()
                .or_else(|| profile.base_url.clone()),
            api_key_env: profile.api_key_env.clone(),
            api_key_present: profile.api_key_present(),
            timeout_ms: metadata.timeout_ms,
            max_retries: metadata.max_retries,
            supports_tools: profile.capabilities.supports_tools,
            supports_tool_call_shadow_messages: metadata.supports_tool_call_shadow_messages,
        }
    }

    fn provider_metadata_from_profile(profile: &ProviderProfile) -> ProviderTransportMetadata {
        let (timeout_ms, max_retries, supports_tool_call_shadow_messages) =
            match profile.provider_type.as_str() {
                "anthropic" => (60_000, 2, true),
                "ollama" => (90_000, 1, false),
                "mock" => (0, 0, false),
                _ => (45_000, 2, false),
            };

        let base_url = profile
            .base_url
            .clone()
            .or_else(|| match profile.provider_type.as_str() {
                "openai" | "openai-compatible" => Some("https://api.openai.com/v1".to_owned()),
                "anthropic" => Some("https://api.anthropic.com/v1".to_owned()),
                "ollama" => Some("http://127.0.0.1:11434".to_owned()),
                _ => None,
            });

        ProviderTransportMetadata {
            provider_type: profile.provider_type.clone(),
            base_url,
            timeout_ms,
            max_retries,
            supports_tool_call_shadow_messages,
        }
    }

    fn provider_attempt_trace(attempt: &ProviderAttempt) -> ProviderAttemptTrace {
        ProviderAttemptTrace {
            attempt: attempt.attempt,
            max_attempts: attempt.max_attempts,
            status: attempt.status.clone(),
            error_kind: attempt.error_kind.clone(),
            status_code: attempt.status_code,
            retryable: attempt.retryable,
            message: attempt.message.clone(),
        }
    }

    fn provider_failure_trace(error: &ProviderError) -> ProviderFailureTrace {
        ProviderFailureTrace {
            kind: error.kind_label().to_owned(),
            status_code: error.status_code,
            retryable: error.retryable,
            message: error.public_message().to_owned(),
        }
    }

    fn trace_provider_attempts(&self, trace: &mut RunTrace, attempts: &[ProviderAttempt]) {
        for attempt in attempts {
            trace.add_provider_attempt(Self::provider_attempt_trace(attempt));
        }
    }

    fn trace_provider_error(
        &self,
        profile: &ProviderProfile,
        trace: &mut RunTrace,
        error: &ProviderError,
    ) {
        self.trace_provider_attempts(trace, &error.attempts);
        trace.bind_provider_failure(Self::provider_failure_trace(error));
        warn!(
            run_id = %trace.run_id,
            provider_type = %profile.provider_type,
            profile = %profile.name,
            model = %profile.model,
            error_kind = %error.kind_label(),
            status_code = ?error.status_code,
            retryable = error.retryable,
            "provider call failed"
        );
    }

    fn emit_provider_retry_events(&self, profile: &ProviderProfile, attempts: &[ProviderAttempt]) {
        for attempt in attempts.iter().filter(|attempt| attempt.status == "retry") {
            self.emit(RunEvent::ProviderRetry {
                provider_type: profile.provider_type.clone(),
                profile: profile.name.clone(),
                model: profile.model.clone(),
                attempt: attempt.attempt,
                max_attempts: attempt.max_attempts,
                kind: attempt
                    .error_kind
                    .clone()
                    .unwrap_or_else(|| "unknown".to_owned()),
                status_code: attempt.status_code,
                retryable: attempt.retryable,
                error: attempt
                    .message
                    .clone()
                    .unwrap_or_else(|| "provider retry".to_owned()),
            });
        }
    }

    async fn maybe_dispatch_tool_via_node(
        &self,
        session_id: Option<&str>,
        tool_name: &str,
        tool_input: &serde_json::Value,
        metadata: &ToolMetadata,
        timeout: Duration,
    ) -> Result<NodeToolDispatchOutcome> {
        let Some(router) = self.ctx.node_router.as_ref() else {
            return Ok(NodeToolDispatchOutcome::NotHandled);
        };
        let Some(capability) = metadata.capability.node.capability.clone() else {
            return Ok(NodeToolDispatchOutcome::NotHandled);
        };

        router
            .dispatch(NodeToolExecutionRequest {
                session_id: session_id.map(ToOwned::to_owned),
                tool_name: tool_name.to_owned(),
                capability,
                input: tool_input.clone(),
                timeout,
            })
            .await
    }

    fn node_failure_status(message: &str) -> &'static str {
        if message.contains("permission denied") {
            "node_permission_denied"
        } else if message.contains("node stale") {
            "node_stale"
        } else if message.contains("node unavailable") || message.contains("no node is available") {
            "node_unavailable"
        } else {
            "failed"
        }
    }

    async fn invoke_tool_with_guardrails(
        &self,
        session_id: Option<&str>,
        tool_name: String,
        call_id: String,
        tool_input: serde_json::Value,
    ) -> std::result::Result<ToolExecutionOutcome, ToolExecutionFailure> {
        self.emit(RunEvent::ToolCalling {
            name: tool_name.clone(),
            call_id: call_id.clone(),
        });

        let tool = match self.ctx.tools.get(&tool_name) {
            Some(tool) => tool,
            None => {
                let error = anyhow!("tool not found: {}", tool_name);
                self.emit(RunEvent::ToolFailed {
                    name: tool_name,
                    call_id,
                    error: error.to_string(),
                });
                return Err(ToolExecutionFailure {
                    error,
                    tool_trace: None,
                    capability_trace: None,
                });
            }
        };

        let metadata = tool.metadata().clone();
        let started_at = Utc::now();
        let job_id = Uuid::new_v4().to_string();
        self.emit(RunEvent::CapabilityJobQueued {
            job_id: job_id.clone(),
            name: tool_name.clone(),
            kind: metadata.capability.kind.label().to_owned(),
            risk: metadata.capability.risk.label().to_owned(),
            permission_scopes: Self::permission_scope_labels(&metadata),
        });

        if !metadata.capability.authorized {
            let error = anyhow!("tool '{}' is not authorized for execution", tool_name);
            self.emit(RunEvent::PermissionCheckFailed {
                name: tool_name.clone(),
                call_id: call_id.clone(),
                reason: error.to_string(),
            });
            self.emit(RunEvent::CapabilityJobFailed {
                job_id: job_id.clone(),
                name: tool_name.clone(),
                error: error.to_string(),
            });
            self.emit(RunEvent::ToolFailed {
                name: tool_name.clone(),
                call_id: call_id.clone(),
                error: error.to_string(),
            });
            return Err(self.build_tool_failure(
                error, job_id, call_id, tool_name, &metadata, tool_input, started_at, None, None,
                None, "rejected",
            ));
        }

        if !metadata.capability.healthy {
            let error = anyhow!("tool '{}' is not healthy", tool_name);
            self.emit(RunEvent::PermissionCheckFailed {
                name: tool_name.clone(),
                call_id: call_id.clone(),
                reason: error.to_string(),
            });
            self.emit(RunEvent::CapabilityJobFailed {
                job_id: job_id.clone(),
                name: tool_name.clone(),
                error: error.to_string(),
            });
            self.emit(RunEvent::ToolFailed {
                name: tool_name.clone(),
                call_id: call_id.clone(),
                error: error.to_string(),
            });
            return Err(self.build_tool_failure(
                error, job_id, call_id, tool_name, &metadata, tool_input, started_at, None, None,
                None, "rejected",
            ));
        }

        self.emit(RunEvent::CapabilityJobStarted {
            job_id: job_id.clone(),
            name: tool_name.clone(),
        });

        let attempts = usize::from(metadata.capability.execution.retry_limit) + 1;
        let timeout = Duration::from_millis(metadata.capability.execution.timeout_ms.max(1));

        if metadata.capability.routes_via_node() {
            match self
                .maybe_dispatch_tool_via_node(
                    session_id,
                    &tool_name,
                    &tool_input,
                    &metadata,
                    timeout,
                )
                .await
            {
                Ok(NodeToolDispatchOutcome::Completed(execution)) => {
                    let finished_at = Utc::now();
                    let node_trace = NodeTraceContext {
                        node_id: Some(execution.node_id),
                        capability_route: Some(execution.route),
                        disconnect_context: execution.disconnect_context,
                    };
                    let result = execution.result;
                    let output = result.content.clone();
                    let tool_trace = ToolTrace {
                        call_id: Some(call_id.clone()),
                        name: tool_name.clone(),
                        source: metadata.source.clone(),
                        input: tool_input,
                        output: Some(output.clone()),
                        node_id: node_trace.node_id.clone(),
                        capability_route: node_trace.capability_route.clone(),
                        disconnect_context: node_trace.disconnect_context.clone(),
                        started_at,
                        finished_at: Some(finished_at),
                    };
                    let capability_trace = Self::capability_trace(
                        &job_id,
                        &call_id,
                        &tool_name,
                        &metadata,
                        result.audit.as_ref(),
                        started_at,
                        finished_at,
                        "success",
                        None,
                        Some(output.as_str()),
                        Some(&node_trace),
                    );
                    self.emit(RunEvent::CapabilityJobFinished {
                        job_id: job_id.clone(),
                        name: tool_name.clone(),
                        status: "success".to_owned(),
                        summary: capability_trace.summary.clone(),
                    });
                    self.emit(RunEvent::ToolFinished {
                        name: tool_name,
                        call_id,
                    });
                    return Ok(ToolExecutionOutcome {
                        output,
                        tool_trace,
                        capability_trace,
                    });
                }
                Ok(NodeToolDispatchOutcome::Failed(node_error)) => {
                    let status = Self::node_failure_status(&node_error.message);
                    let error = anyhow!(node_error.message.clone());
                    let node_trace = NodeTraceContext {
                        node_id: node_error.node_id,
                        capability_route: node_error.route,
                        disconnect_context: node_error.disconnect_context,
                    };
                    self.emit(RunEvent::CapabilityJobFailed {
                        job_id: job_id.clone(),
                        name: tool_name.clone(),
                        error: error.to_string(),
                    });
                    self.emit(RunEvent::ToolFailed {
                        name: tool_name.clone(),
                        call_id: call_id.clone(),
                        error: error.to_string(),
                    });
                    return Err(self.build_tool_failure(
                        error,
                        job_id,
                        call_id,
                        tool_name,
                        &metadata,
                        tool_input,
                        started_at,
                        None,
                        None,
                        Some(node_trace),
                        status,
                    ));
                }
                Ok(NodeToolDispatchOutcome::NotHandled) => {
                    if metadata.capability.node.require_node {
                        let capability = metadata
                            .capability
                            .node
                            .capability
                            .as_deref()
                            .unwrap_or(tool_name.as_str());
                        let error = anyhow!(
                            "node route required for capability '{}' but no node is available",
                            capability
                        );
                        self.emit(RunEvent::CapabilityJobFailed {
                            job_id: job_id.clone(),
                            name: tool_name.clone(),
                            error: error.to_string(),
                        });
                        self.emit(RunEvent::ToolFailed {
                            name: tool_name.clone(),
                            call_id: call_id.clone(),
                            error: error.to_string(),
                        });
                        return Err(self.build_tool_failure(
                            error,
                            job_id,
                            call_id,
                            tool_name,
                            &metadata,
                            tool_input,
                            started_at,
                            None,
                            None,
                            None,
                            "node_unavailable",
                        ));
                    }
                }
                Err(err) => {
                    self.emit(RunEvent::CapabilityJobFailed {
                        job_id: job_id.clone(),
                        name: tool_name.clone(),
                        error: err.to_string(),
                    });
                    self.emit(RunEvent::ToolFailed {
                        name: tool_name.clone(),
                        call_id: call_id.clone(),
                        error: err.to_string(),
                    });
                    return Err(self.build_tool_failure(
                        err, job_id, call_id, tool_name, &metadata, tool_input, started_at, None,
                        None, None, "failed",
                    ));
                }
            }
        }

        for attempt in 1..=attempts {
            let attempt_result = tokio::time::timeout(timeout, tool.call(tool_input.clone())).await;
            match attempt_result {
                Ok(Ok(result)) if !result.is_error => {
                    let finished_at = Utc::now();
                    let output = result.content.clone();
                    let tool_trace = ToolTrace {
                        call_id: Some(call_id.clone()),
                        name: tool_name.clone(),
                        source: metadata.source.clone(),
                        input: tool_input,
                        output: Some(output.clone()),
                        node_id: None,
                        capability_route: None,
                        disconnect_context: None,
                        started_at,
                        finished_at: Some(finished_at),
                    };
                    let capability_trace = Self::capability_trace(
                        &job_id,
                        &call_id,
                        &tool_name,
                        &metadata,
                        result.audit.as_ref(),
                        started_at,
                        finished_at,
                        "success",
                        None,
                        Some(output.as_str()),
                        None,
                    );
                    self.emit(RunEvent::CapabilityJobFinished {
                        job_id: job_id.clone(),
                        name: tool_name.clone(),
                        status: "success".to_owned(),
                        summary: capability_trace.summary.clone(),
                    });
                    self.emit(RunEvent::ToolFinished {
                        name: tool_name,
                        call_id,
                    });
                    return Ok(ToolExecutionOutcome {
                        output,
                        tool_trace,
                        capability_trace,
                    });
                }
                Ok(Ok(result)) => {
                    let error = anyhow!(result.content.clone());
                    if attempt < attempts {
                        self.emit(RunEvent::CapabilityJobRetried {
                            job_id: job_id.clone(),
                            name: tool_name.clone(),
                            attempt: attempt as u8,
                            error: error.to_string(),
                        });
                        continue;
                    }
                    self.emit(RunEvent::CapabilityJobFailed {
                        job_id: job_id.clone(),
                        name: tool_name.clone(),
                        error: error.to_string(),
                    });
                    self.emit(RunEvent::ToolFailed {
                        name: tool_name.clone(),
                        call_id: call_id.clone(),
                        error: error.to_string(),
                    });
                    return Err(self.build_tool_failure(
                        error,
                        job_id,
                        call_id,
                        tool_name,
                        &metadata,
                        tool_input,
                        started_at,
                        Some(result.content),
                        result.audit.as_ref(),
                        None,
                        "failed",
                    ));
                }
                Ok(Err(err)) => {
                    if attempt < attempts {
                        self.emit(RunEvent::CapabilityJobRetried {
                            job_id: job_id.clone(),
                            name: tool_name.clone(),
                            attempt: attempt as u8,
                            error: err.to_string(),
                        });
                        continue;
                    }
                    self.emit(RunEvent::CapabilityJobFailed {
                        job_id: job_id.clone(),
                        name: tool_name.clone(),
                        error: err.to_string(),
                    });
                    self.emit(RunEvent::ToolFailed {
                        name: tool_name.clone(),
                        call_id: call_id.clone(),
                        error: err.to_string(),
                    });
                    return Err(self.build_tool_failure(
                        err, job_id, call_id, tool_name, &metadata, tool_input, started_at, None,
                        None, None, "failed",
                    ));
                }
                Err(_) => {
                    let error = anyhow!(
                        "tool '{}' timed out after {}ms",
                        tool_name,
                        metadata.capability.execution.timeout_ms.max(1)
                    );
                    if attempt < attempts {
                        self.emit(RunEvent::CapabilityJobRetried {
                            job_id: job_id.clone(),
                            name: tool_name.clone(),
                            attempt: attempt as u8,
                            error: error.to_string(),
                        });
                        continue;
                    }
                    self.emit(RunEvent::CapabilityJobFailed {
                        job_id: job_id.clone(),
                        name: tool_name.clone(),
                        error: error.to_string(),
                    });
                    self.emit(RunEvent::ToolFailed {
                        name: tool_name.clone(),
                        call_id: call_id.clone(),
                        error: error.to_string(),
                    });
                    return Err(self.build_tool_failure(
                        error,
                        job_id,
                        call_id,
                        tool_name,
                        &metadata,
                        tool_input,
                        started_at,
                        None,
                        None,
                        None,
                        "timed_out",
                    ));
                }
            }
        }

        unreachable!("tool attempts should always return success or failure")
    }

    fn build_tool_failure(
        &self,
        error: anyhow::Error,
        job_id: String,
        call_id: String,
        tool_name: String,
        metadata: &ToolMetadata,
        tool_input: serde_json::Value,
        started_at: chrono::DateTime<Utc>,
        output: Option<String>,
        audit: Option<&CapabilityAudit>,
        node_trace: Option<NodeTraceContext>,
        status: &str,
    ) -> ToolExecutionFailure {
        let finished_at = Utc::now();
        let tool_trace = ToolTrace {
            call_id: Some(call_id.clone()),
            name: tool_name.clone(),
            source: metadata.source.clone(),
            input: tool_input,
            output: output
                .clone()
                .or_else(|| Some(format!("[runtime tool failure] {}", error))),
            node_id: node_trace.as_ref().and_then(|trace| trace.node_id.clone()),
            capability_route: node_trace
                .as_ref()
                .and_then(|trace| trace.capability_route.clone()),
            disconnect_context: node_trace
                .as_ref()
                .and_then(|trace| trace.disconnect_context.clone()),
            started_at,
            finished_at: Some(finished_at),
        };
        let capability_trace = Self::capability_trace(
            &job_id,
            &call_id,
            &tool_name,
            metadata,
            audit,
            started_at,
            finished_at,
            status,
            Some(error.to_string()),
            output.as_deref(),
            node_trace.as_ref(),
        );

        ToolExecutionFailure {
            error,
            tool_trace: Some(tool_trace),
            capability_trace: Some(capability_trace),
        }
    }

    fn capability_trace(
        job_id: &str,
        call_id: &str,
        tool_name: &str,
        metadata: &ToolMetadata,
        audit: Option<&CapabilityAudit>,
        started_at: chrono::DateTime<Utc>,
        finished_at: chrono::DateTime<Utc>,
        status: &str,
        error: Option<String>,
        fallback_summary: Option<&str>,
        node_trace: Option<&NodeTraceContext>,
    ) -> CapabilityInvocationTrace {
        let summary = audit
            .map(|audit| audit.side_effect_summary.clone())
            .or_else(|| fallback_summary.map(|value| Self::truncate_preview(value, 180)))
            .unwrap_or_else(|| format!("{} {}", tool_name, status));

        CapabilityInvocationTrace {
            job_id: job_id.to_owned(),
            call_id: Some(call_id.to_owned()),
            tool_name: tool_name.to_owned(),
            kind: metadata.capability.kind.clone(),
            permission_scopes: metadata.capability.permission_scopes.clone(),
            risk: metadata.capability.risk.clone(),
            status: status.to_owned(),
            summary,
            target: audit.and_then(|audit| audit.target.clone()),
            node_id: node_trace.and_then(|trace| trace.node_id.clone()),
            capability_route: node_trace.and_then(|trace| trace.capability_route.clone()),
            disconnect_context: node_trace.and_then(|trace| trace.disconnect_context.clone()),
            started_at,
            finished_at: Some(finished_at),
            error,
        }
    }

    fn permission_scope_labels(metadata: &ToolMetadata) -> Vec<String> {
        metadata
            .capability
            .permission_scopes
            .iter()
            .map(|scope| scope.label().to_owned())
            .collect()
    }

    fn collect_tool_definitions(
        &self,
        allowlist: Option<&[String]>,
    ) -> Result<Vec<ToolDefinition>> {
        match allowlist {
            Some(names) => names
                .iter()
                .map(|name| {
                    let tool = self
                        .ctx
                        .tools
                        .get(name)
                        .ok_or_else(|| anyhow!("tool not found: {}", name))?;
                    let metadata = tool.metadata();
                    if !tool_is_visible_to_model(metadata) {
                        bail!("tool is not authorized or healthy: {}", name);
                    }
                    Ok(tool_definition_from_metadata(metadata))
                })
                .collect(),
            None => Ok(self
                .ctx
                .tools
                .iter()
                .filter_map(|tool| {
                    let metadata = tool.metadata();
                    tool_is_visible_to_model(metadata)
                        .then(|| tool_definition_from_metadata(metadata))
                })
                .collect()),
        }
    }
}
