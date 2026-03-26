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
    RunTrace, SkillTrace, ToolTrace, WorkflowStepTrace,
};
use mosaic_memory::{
    MemoryEntryKind, MemoryPolicy, MemoryStore, SessionMemoryRecord, compress_fragments,
    summarize_fragments,
};
use mosaic_node_protocol::{NodeRouter, NodeToolDispatchOutcome, NodeToolExecutionRequest};
use mosaic_provider::{
    LlmProvider, Message, ProviderProfile, ProviderProfileRegistry, Role, SchedulingIntent,
    SchedulingRequest, ToolDefinition, tool_definition_from_metadata, tool_is_visible_to_model,
    validate_step_tools_support,
};
use mosaic_session_core::{SessionRecord, SessionStore, TranscriptRole, session_title_from_input};
use mosaic_skill_core::{SkillContext, SkillRegistry};
use mosaic_tool_core::{CapabilityAudit, ToolMetadata, ToolRegistry};
use mosaic_workflow::{
    Workflow, WorkflowObserver, WorkflowRegistry, WorkflowRunner, WorkflowStep,
    WorkflowStepExecutor, WorkflowStepKind,
};
use uuid::Uuid;

use crate::events::{RunEvent, SharedRunEventSink};

type SharedToolTraceCollector = Arc<Mutex<Vec<ToolTrace>>>;
type SharedSkillTraceCollector = Arc<Mutex<Vec<SkillTrace>>>;
type SharedModelSelectionCollector = Arc<Mutex<Vec<ModelSelectionTrace>>>;
type SharedCapabilityTraceCollector = Arc<Mutex<Vec<CapabilityInvocationTrace>>>;

struct ToolExecutionOutcome {
    output: String,
    tool_trace: ToolTrace,
    capability_trace: CapabilityInvocationTrace,
}

struct ToolExecutionFailure {
    error: anyhow::Error,
    tool_trace: Option<ToolTrace>,
    capability_trace: Option<CapabilityInvocationTrace>,
}

#[derive(Debug, Clone, Default)]
struct NodeTraceContext {
    node_id: Option<String>,
    capability_route: Option<String>,
    disconnect_context: Option<String>,
}

pub struct RuntimeContext {
    pub profiles: Arc<ProviderProfileRegistry>,
    pub provider_override: Option<Arc<dyn LlmProvider>>,
    pub session_store: Arc<dyn SessionStore>,
    pub memory_store: Arc<dyn MemoryStore>,
    pub memory_policy: MemoryPolicy,
    pub tools: Arc<ToolRegistry>,
    pub skills: Arc<SkillRegistry>,
    pub workflows: Arc<WorkflowRegistry>,
    pub node_router: Option<Arc<dyn NodeRouter>>,
    pub active_extensions: Vec<ExtensionTrace>,
    pub event_sink: SharedRunEventSink,
}

pub struct RunRequest {
    pub system: Option<String>,
    pub input: String,
    pub skill: Option<String>,
    pub workflow: Option<String>,
    pub session_id: Option<String>,
    pub profile: Option<String>,
    pub ingress: Option<IngressTrace>,
}

#[derive(Debug)]
pub struct RunResult {
    pub output: String,
    pub trace: RunTrace,
}

#[derive(Debug)]
pub struct RunError {
    source: anyhow::Error,
    trace: RunTrace,
}

impl RunError {
    fn new(source: anyhow::Error, trace: RunTrace) -> Self {
        Self { source, trace }
    }

    pub fn trace(&self) -> &RunTrace {
        &self.trace
    }

    pub fn into_parts(self) -> (anyhow::Error, RunTrace) {
        (self.source, self.trace)
    }
}

impl std::fmt::Display for RunError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.source)
    }
}

impl std::error::Error for RunError {}

pub struct AgentRuntime {
    ctx: RuntimeContext,
}

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

    fn fail_run<T>(
        &self,
        mut trace: RunTrace,
        error: anyhow::Error,
    ) -> std::result::Result<T, RunError> {
        let message = error.to_string();

        self.emit(RunEvent::RunFailed {
            error: message.clone(),
        });
        trace.finish_err(message);

        Err(RunError::new(error, trace))
    }

    pub async fn run(&self, req: RunRequest) -> std::result::Result<RunResult, RunError> {
        let mut trace = RunTrace::new(req.input.clone());
        self.emit(RunEvent::RunStarted {
            input: trace.input.clone(),
        });

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
        trace.bind_effective_profile(Self::effective_profile_trace(&profile));

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

        self.emit(RunEvent::FinalAnswerReady);
        self.emit(RunEvent::RunFinished {
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

        self.emit(RunEvent::FinalAnswerReady);
        self.emit(RunEvent::RunFinished {
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

        self.emit(RunEvent::FinalAnswerReady);
        self.emit(RunEvent::RunFinished {
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
            self.emit(RunEvent::ProviderRequest {
                tool_count: tool_defs.len(),
                message_count: messages.len(),
            });

            let response = provider.complete(&messages, provider_tools).await?;

            if !response.tool_calls.is_empty() {
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
                                messages = self.session_messages_for_provider(
                                    session_ref,
                                    reference_contexts,
                                    trace,
                                )?;
                            } else {
                                messages.push(Message {
                                    role: Role::Tool,
                                    content: outcome.output,
                                    tool_call_id: Some(call_id),
                                });
                            }
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
            self.emit(RunEvent::ProviderRequest {
                tool_count: tool_defs.len(),
                message_count: messages.len(),
            });

            let response = provider.complete(&messages, provider_tools).await?;

            if !response.tool_calls.is_empty() {
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

    fn effective_profile_trace(profile: &ProviderProfile) -> EffectiveProfileTrace {
        EffectiveProfileTrace {
            profile: profile.name.clone(),
            provider_type: profile.provider_type.clone(),
            model: profile.model.clone(),
            api_key_env: profile.api_key_env.clone(),
            api_key_present: profile.api_key_present(),
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

struct RuntimeWorkflowExecutor<'a> {
    runtime: &'a AgentRuntime,
    default_profile: ProviderProfile,
    session_id: Option<String>,
    ingress_channel: Option<String>,
    tool_traces: SharedToolTraceCollector,
    skill_traces: SharedSkillTraceCollector,
    model_selections: SharedModelSelectionCollector,
    capability_traces: SharedCapabilityTraceCollector,
}

impl RuntimeWorkflowExecutor<'_> {
    fn resolve_prompt_profile(
        &self,
        step: &WorkflowStep,
        input: &str,
        tools: &[String],
    ) -> Result<(ProviderProfile, String)> {
        let WorkflowStepKind::Prompt { profile, .. } = &step.kind else {
            return Ok((
                self.default_profile.clone(),
                "workflow_skill_default".to_owned(),
            ));
        };

        let scheduled = self.runtime.ctx.profiles.schedule(SchedulingRequest {
            requested_profile: profile.clone().or(Some(self.default_profile.name.clone())),
            channel: self.ingress_channel.clone(),
            intent: SchedulingIntent::WorkflowStep,
            estimated_context_chars: input.chars().count(),
            requires_tools: !tools.is_empty(),
        })?;

        Ok((scheduled.profile, scheduled.reason))
    }
}

#[async_trait]
impl WorkflowStepExecutor for RuntimeWorkflowExecutor<'_> {
    async fn execute_prompt(
        &self,
        _workflow: &Workflow,
        step: &WorkflowStep,
        input: String,
    ) -> Result<String> {
        let WorkflowStepKind::Prompt { system, tools, .. } = &step.kind else {
            unreachable!("workflow runner routed non-prompt step into execute_prompt")
        };

        let (profile, selection_reason) = self.resolve_prompt_profile(step, &input, tools)?;
        validate_step_tools_support(&profile, tools)?;
        push_model_selection(
            &self.model_selections,
            AgentRuntime::model_selection_trace(
                format!("workflow:{}", step.name),
                match &step.kind {
                    WorkflowStepKind::Prompt { profile, .. } => profile.clone(),
                    WorkflowStepKind::Skill { .. } => None,
                },
                &profile,
                selection_reason,
            ),
        );
        let tool_defs = if profile.capabilities.supports_tools {
            self.runtime.collect_tool_definitions(Some(tools))?
        } else {
            Vec::new()
        };

        self.runtime
            .execute_workflow_prompt_step(
                self.session_id.as_deref(),
                &profile,
                system.clone(),
                input,
                tool_defs,
                &self.tool_traces,
                &self.capability_traces,
            )
            .await
    }

    async fn execute_skill(
        &self,
        _workflow: &Workflow,
        step: &WorkflowStep,
        input: String,
    ) -> Result<String> {
        let WorkflowStepKind::Skill { skill, .. } = &step.kind else {
            unreachable!("workflow runner routed non-skill step into execute_skill")
        };

        self.runtime
            .execute_workflow_skill_step(skill.clone(), input, &self.skill_traces)
            .await
    }
}

struct RuntimeWorkflowObserver<'a> {
    runtime: &'a AgentRuntime,
    trace: &'a mut RunTrace,
}

impl WorkflowObserver for RuntimeWorkflowObserver<'_> {
    fn workflow_started(&mut self, workflow: &Workflow) {
        self.trace.bind_workflow(workflow.name.clone());
        self.runtime.emit(RunEvent::WorkflowStarted {
            name: workflow.name.clone(),
            step_count: workflow.steps.len(),
        });
    }

    fn step_started(&mut self, workflow: &Workflow, step: &WorkflowStep, input: &str) {
        self.runtime.emit(RunEvent::WorkflowStepStarted {
            workflow: workflow.name.clone(),
            step: step.name.clone(),
            kind: step.kind.label().to_owned(),
        });
        self.trace.step_traces.push(WorkflowStepTrace {
            name: step.name.clone(),
            kind: step.kind.label().to_owned(),
            input: input.to_owned(),
            output: None,
            started_at: Utc::now(),
            finished_at: None,
            error: None,
        });
    }

    fn step_finished(
        &mut self,
        workflow: &Workflow,
        step: &WorkflowStep,
        _input: &str,
        output: &str,
    ) {
        self.runtime.emit(RunEvent::WorkflowStepFinished {
            workflow: workflow.name.clone(),
            step: step.name.clone(),
        });

        if let Some(trace) = self
            .trace
            .step_traces
            .iter_mut()
            .rev()
            .find(|trace| trace.name == step.name && trace.finished_at.is_none())
        {
            trace.output = Some(output.to_owned());
            trace.finished_at = Some(Utc::now());
        }
    }

    fn step_failed(
        &mut self,
        workflow: &Workflow,
        step: &WorkflowStep,
        _input: &str,
        error: &anyhow::Error,
    ) {
        self.runtime.emit(RunEvent::WorkflowStepFailed {
            workflow: workflow.name.clone(),
            step: step.name.clone(),
            error: error.to_string(),
        });

        if let Some(trace) = self
            .trace
            .step_traces
            .iter_mut()
            .rev()
            .find(|trace| trace.name == step.name && trace.finished_at.is_none())
        {
            trace.error = Some(error.to_string());
            trace.finished_at = Some(Utc::now());
        }
    }

    fn workflow_finished(&mut self, workflow: &Workflow, _output: &str) {
        self.runtime.emit(RunEvent::WorkflowFinished {
            name: workflow.name.clone(),
        });
    }
}

fn push_tool_trace(collector: &SharedToolTraceCollector, trace: ToolTrace) {
    collector
        .lock()
        .expect("tool trace collector should not be poisoned")
        .push(trace);
}

fn drain_tool_trace_collector(collector: &SharedToolTraceCollector) -> Vec<ToolTrace> {
    std::mem::take(
        &mut *collector
            .lock()
            .expect("tool trace collector should not be poisoned"),
    )
}

fn push_capability_trace(
    collector: &SharedCapabilityTraceCollector,
    trace: CapabilityInvocationTrace,
) {
    collector
        .lock()
        .expect("capability trace collector should not be poisoned")
        .push(trace);
}

fn drain_capability_trace_collector(
    collector: &SharedCapabilityTraceCollector,
) -> Vec<CapabilityInvocationTrace> {
    std::mem::take(
        &mut *collector
            .lock()
            .expect("capability trace collector should not be poisoned"),
    )
}

fn push_skill_trace(collector: &SharedSkillTraceCollector, trace: SkillTrace) {
    collector
        .lock()
        .expect("skill trace collector should not be poisoned")
        .push(trace);
}

fn update_skill_trace<F>(collector: &SharedSkillTraceCollector, name: &str, update: F)
where
    F: FnOnce(&mut SkillTrace),
{
    if let Some(trace) = collector
        .lock()
        .expect("skill trace collector should not be poisoned")
        .iter_mut()
        .rev()
        .find(|trace| trace.name == name && trace.finished_at.is_none())
    {
        update(trace);
    }
}

fn drain_skill_trace_collector(collector: &SharedSkillTraceCollector) -> Vec<SkillTrace> {
    std::mem::take(
        &mut *collector
            .lock()
            .expect("skill trace collector should not be poisoned"),
    )
}

fn push_model_selection(collector: &SharedModelSelectionCollector, trace: ModelSelectionTrace) {
    collector
        .lock()
        .expect("model selection collector should not be poisoned")
        .push(trace);
}

fn drain_model_selection_collector(
    collector: &SharedModelSelectionCollector,
) -> Vec<ModelSelectionTrace> {
    std::mem::take(
        &mut *collector
            .lock()
            .expect("model selection collector should not be poisoned"),
    )
}

#[cfg(test)]
mod tests {
    use std::{
        collections::BTreeMap,
        sync::{Arc, Mutex},
        time::Duration,
    };

    use crate::events::{NoopEventSink, RunEvent, RunEventSink};
    use anyhow::{Result, anyhow};
    use async_trait::async_trait;
    use mosaic_config::{MosaicConfig, ProviderProfileConfig};
    use mosaic_inspect::IngressTrace;
    use mosaic_memory::{MemoryPolicy, MemorySearchHit, MemoryStore, SessionMemoryRecord};
    use mosaic_node_protocol::{
        FileNodeStore, NodeCapabilityDeclaration, NodeCommandResultEnvelope, NodeRegistration,
    };
    use mosaic_provider::{
        CompletionResponse, LlmProvider, Message, MockProvider, ProviderProfileRegistry,
        ToolDefinition,
    };
    use mosaic_session_core::{
        SessionRecord, SessionStore, SessionSummary, TranscriptMessage, TranscriptRole,
    };
    use mosaic_skill_core::{SkillRegistry, SummarizeSkill};
    use mosaic_tool_core::{
        CapabilityKind, PermissionScope, ReadFileTool, TimeNowTool, Tool, ToolMetadata,
        ToolRegistry, ToolResult, ToolRiskLevel, ToolSource,
    };
    use mosaic_workflow::{Workflow, WorkflowRegistry, WorkflowStep, WorkflowStepKind};

    use super::{AgentRuntime, RunRequest, RuntimeContext};

    #[derive(Default)]
    struct VecEventSink {
        events: Mutex<Vec<RunEvent>>,
    }

    impl VecEventSink {
        fn snapshot(&self) -> Vec<RunEvent> {
            self.events
                .lock()
                .expect("event lock should not be poisoned")
                .clone()
        }
    }

    impl RunEventSink for VecEventSink {
        fn emit(&self, event: RunEvent) {
            self.events
                .lock()
                .expect("event lock should not be poisoned")
                .push(event);
        }
    }

    #[derive(Default)]
    struct MemorySessionStore {
        sessions: Mutex<BTreeMap<String, SessionRecord>>,
    }

    impl MemorySessionStore {
        fn get(&self, id: &str) -> Option<SessionRecord> {
            self.sessions
                .lock()
                .expect("session lock should not be poisoned")
                .get(id)
                .cloned()
        }
    }

    impl SessionStore for MemorySessionStore {
        fn load(&self, id: &str) -> Result<Option<SessionRecord>> {
            Ok(self.get(id))
        }

        fn save(&self, session: &SessionRecord) -> Result<()> {
            self.sessions
                .lock()
                .expect("session lock should not be poisoned")
                .insert(session.id.clone(), session.clone());
            Ok(())
        }

        fn list(&self) -> Result<Vec<SessionSummary>> {
            Ok(self
                .sessions
                .lock()
                .expect("session lock should not be poisoned")
                .values()
                .map(SessionRecord::summary)
                .collect())
        }
    }

    #[derive(Default)]
    struct MemoryMemoryStore {
        sessions: Mutex<BTreeMap<String, SessionMemoryRecord>>,
    }

    impl MemoryMemoryStore {
        fn get(&self, id: &str) -> Option<SessionMemoryRecord> {
            self.sessions
                .lock()
                .expect("memory lock should not be poisoned")
                .get(id)
                .cloned()
        }
    }

    impl MemoryStore for MemoryMemoryStore {
        fn load_session(&self, session_id: &str) -> Result<Option<SessionMemoryRecord>> {
            Ok(self.get(session_id))
        }

        fn save_session(&self, record: &SessionMemoryRecord) -> Result<()> {
            self.sessions
                .lock()
                .expect("memory lock should not be poisoned")
                .insert(record.session_id.clone(), record.clone());
            Ok(())
        }

        fn list_sessions(&self) -> Result<Vec<SessionMemoryRecord>> {
            Ok(self
                .sessions
                .lock()
                .expect("memory lock should not be poisoned")
                .values()
                .cloned()
                .collect())
        }

        fn search(&self, query: &str, _tag: Option<&str>) -> Result<Vec<MemorySearchHit>> {
            let query = query.to_ascii_lowercase();
            let mut hits = Vec::new();
            for record in self
                .sessions
                .lock()
                .expect("memory lock should not be poisoned")
                .values()
            {
                if let Some(summary) = record.summary.as_deref() {
                    if query.is_empty() || summary.to_ascii_lowercase().contains(&query) {
                        hits.push(MemorySearchHit {
                            session_id: record.session_id.clone(),
                            kind: "summary".to_owned(),
                            preview: summary.to_owned(),
                            tags: record.tags.clone(),
                            updated_at: record.updated_at,
                        });
                    }
                }
            }
            Ok(hits)
        }
    }

    struct EmptyProvider;

    #[async_trait]
    impl LlmProvider for EmptyProvider {
        async fn complete(
            &self,
            _messages: &[Message],
            _tools: Option<&[ToolDefinition]>,
        ) -> Result<CompletionResponse> {
            Ok(CompletionResponse {
                message: None,
                tool_calls: vec![],
                finish_reason: Some("stop".to_owned()),
            })
        }
    }

    struct FailingSkill;

    #[async_trait]
    impl mosaic_skill_core::Skill for FailingSkill {
        fn name(&self) -> &str {
            "explode"
        }

        async fn execute(
            &self,
            _input: serde_json::Value,
            _ctx: &mosaic_skill_core::SkillContext,
        ) -> Result<mosaic_skill_core::SkillOutput> {
            Err(anyhow!("skill exploded"))
        }
    }

    struct FakeMcpReadFileTool {
        meta: ToolMetadata,
    }

    impl FakeMcpReadFileTool {
        fn new() -> Self {
            Self {
                meta: ToolMetadata::mcp(
                    "filesystem",
                    "read_file",
                    "Read a UTF-8 text file from disk via MCP",
                    serde_json::json!({
                        "type": "object",
                        "properties": {
                            "path": { "type": "string" }
                        },
                        "required": ["path"]
                    }),
                ),
            }
        }
    }

    #[async_trait]
    impl Tool for FakeMcpReadFileTool {
        fn metadata(&self) -> &ToolMetadata {
            &self.meta
        }

        async fn call(&self, input: serde_json::Value) -> Result<ToolResult> {
            let path = input
                .get("path")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("README.md");

            Ok(ToolResult {
                content: format!("remote mcp contents from {path}"),
                structured: Some(serde_json::json!({
                    "path": path,
                    "origin": "mcp",
                })),
                is_error: false,
                audit: None,
            })
        }
    }

    fn runtime_with_provider(
        provider: Arc<dyn LlmProvider>,
        session_store: Arc<dyn SessionStore>,
        event_sink: Arc<dyn RunEventSink>,
    ) -> AgentRuntime {
        runtime_with_provider_and_workflows(
            provider,
            session_store,
            event_sink,
            WorkflowRegistry::new(),
        )
    }

    fn runtime_with_provider_and_workflows(
        provider: Arc<dyn LlmProvider>,
        session_store: Arc<dyn SessionStore>,
        event_sink: Arc<dyn RunEventSink>,
        workflows: WorkflowRegistry,
    ) -> AgentRuntime {
        let mut config = MosaicConfig::default();
        config.active_profile = "mock".to_owned();
        config.profiles.insert(
            "mock".to_owned(),
            ProviderProfileConfig {
                provider_type: "mock".to_owned(),
                model: "mock".to_owned(),
                base_url: None,
                api_key_env: None,
            },
        );

        let profiles =
            ProviderProfileRegistry::from_config(&config).expect("profile registry should build");
        let mut tools = ToolRegistry::new();
        tools.register(Arc::new(TimeNowTool::new()));

        AgentRuntime::new(RuntimeContext {
            profiles: Arc::new(profiles),
            provider_override: Some(provider),
            session_store,
            memory_store: Arc::new(MemoryMemoryStore::default()),
            memory_policy: MemoryPolicy::default(),
            tools: Arc::new(tools),
            skills: Arc::new(SkillRegistry::new()),
            workflows: Arc::new(workflows),
            node_router: None,
            active_extensions: Vec::new(),
            event_sink,
        })
    }

    fn event_names(events: &[RunEvent]) -> Vec<&'static str> {
        events
            .iter()
            .map(|event| match event {
                RunEvent::RunStarted { .. } => "RunStarted",
                RunEvent::WorkflowStarted { .. } => "WorkflowStarted",
                RunEvent::WorkflowStepStarted { .. } => "WorkflowStepStarted",
                RunEvent::WorkflowStepFinished { .. } => "WorkflowStepFinished",
                RunEvent::WorkflowStepFailed { .. } => "WorkflowStepFailed",
                RunEvent::WorkflowFinished { .. } => "WorkflowFinished",
                RunEvent::SkillStarted { .. } => "SkillStarted",
                RunEvent::SkillFinished { .. } => "SkillFinished",
                RunEvent::SkillFailed { .. } => "SkillFailed",
                RunEvent::ProviderRequest { .. } => "ProviderRequest",
                RunEvent::ToolCalling { .. } => "ToolCalling",
                RunEvent::ToolFinished { .. } => "ToolFinished",
                RunEvent::ToolFailed { .. } => "ToolFailed",
                RunEvent::CapabilityJobQueued { .. } => "CapabilityJobQueued",
                RunEvent::CapabilityJobStarted { .. } => "CapabilityJobStarted",
                RunEvent::CapabilityJobRetried { .. } => "CapabilityJobRetried",
                RunEvent::CapabilityJobFinished { .. } => "CapabilityJobFinished",
                RunEvent::CapabilityJobFailed { .. } => "CapabilityJobFailed",
                RunEvent::PermissionCheckFailed { .. } => "PermissionCheckFailed",
                RunEvent::FinalAnswerReady => "FinalAnswerReady",
                RunEvent::RunFinished { .. } => "RunFinished",
                RunEvent::RunFailed { .. } => "RunFailed",
            })
            .collect()
    }

    fn research_workflow() -> Workflow {
        Workflow {
            name: "research_brief".to_owned(),
            description: Some("Draft and summarize a short brief".to_owned()),
            steps: vec![
                WorkflowStep {
                    name: "draft".to_owned(),
                    kind: WorkflowStepKind::Prompt {
                        prompt: "Draft notes for: {{input}}".to_owned(),
                        system: Some("You are a concise researcher.".to_owned()),
                        tools: Vec::new(),
                        profile: None,
                    },
                },
                WorkflowStep {
                    name: "summarize".to_owned(),
                    kind: WorkflowStepKind::Skill {
                        skill: "summarize".to_owned(),
                        input: "{{steps.draft.output}}".to_owned(),
                    },
                },
            ],
        }
    }

    #[tokio::test]
    async fn provider_only_run_returns_mock_output() {
        let sink = Arc::new(VecEventSink::default());
        let runtime = runtime_with_provider(
            Arc::new(MockProvider),
            Arc::new(MemorySessionStore::default()),
            sink.clone(),
        );

        let result = runtime
            .run(RunRequest {
                system: Some("You are helpful.".to_owned()),
                input: "Explain Mosaic.".to_owned(),
                skill: None,
                workflow: None,
                session_id: None,
                profile: None,
                ingress: None,
            })
            .await
            .expect("runtime should succeed");

        assert_eq!(result.output, "mock response: Explain Mosaic.");
        assert_eq!(
            result
                .trace
                .effective_profile
                .as_ref()
                .map(|profile| profile.profile.as_str()),
            Some("mock")
        );

        assert_eq!(
            event_names(&sink.snapshot()),
            vec![
                "RunStarted",
                "ProviderRequest",
                "FinalAnswerReady",
                "RunFinished"
            ]
        );
    }

    #[tokio::test]
    async fn run_records_ingress_metadata_in_trace() {
        let runtime = runtime_with_provider(
            Arc::new(MockProvider),
            Arc::new(MemorySessionStore::default()),
            Arc::new(NoopEventSink),
        );

        let result = runtime
            .run(RunRequest {
                system: Some("You are helpful.".to_owned()),
                input: "hello ingress".to_owned(),
                skill: None,
                workflow: None,
                session_id: Some("ingress-demo".to_owned()),
                profile: None,
                ingress: Some(IngressTrace {
                    kind: "remote_operator".to_owned(),
                    channel: Some("cli".to_owned()),
                    source: Some("mosaic-cli".to_owned()),
                    remote_addr: Some("127.0.0.1".to_owned()),
                    display_name: Some("operator".to_owned()),
                    actor_id: Some("operator-1".to_owned()),
                    thread_id: Some("incident-7".to_owned()),
                    thread_title: Some("Incident 7".to_owned()),
                    reply_target: Some("cli:operator-1".to_owned()),
                    gateway_url: Some("http://127.0.0.1:8080".to_owned()),
                }),
            })
            .await
            .expect("runtime should succeed");

        assert_eq!(result.trace.session_id.as_deref(), Some("ingress-demo"));
        assert_eq!(
            result
                .trace
                .ingress
                .as_ref()
                .map(|ingress| ingress.kind.as_str()),
            Some("remote_operator")
        );
        assert_eq!(
            result
                .trace
                .ingress
                .as_ref()
                .and_then(|ingress| ingress.gateway_url.as_deref()),
            Some("http://127.0.0.1:8080")
        );
        assert_eq!(
            result
                .trace
                .ingress
                .as_ref()
                .and_then(|ingress| ingress.actor_id.as_deref()),
            Some("operator-1")
        );
        assert_eq!(
            result
                .trace
                .ingress
                .as_ref()
                .and_then(|ingress| ingress.thread_id.as_deref()),
            Some("incident-7")
        );
    }

    #[tokio::test]
    async fn session_runs_roundtrip_transcript_messages() {
        let store = Arc::new(MemorySessionStore::default());
        let runtime = runtime_with_provider(
            Arc::new(MockProvider),
            store.clone(),
            Arc::new(NoopEventSink),
        );

        runtime
            .run(RunRequest {
                system: Some("You are helpful.".to_owned()),
                input: "hello".to_owned(),
                skill: None,
                workflow: None,
                session_id: Some("demo".to_owned()),
                profile: None,
                ingress: None,
            })
            .await
            .expect("first run should succeed");

        runtime
            .run(RunRequest {
                system: Some("You are helpful.".to_owned()),
                input: "second turn".to_owned(),
                skill: None,
                workflow: None,
                session_id: Some("demo".to_owned()),
                profile: None,
                ingress: None,
            })
            .await
            .expect("second run should succeed");

        let session = store.get("demo").expect("session should exist");
        let transcript_roles = session
            .transcript
            .iter()
            .map(|message| &message.role)
            .collect::<Vec<_>>();

        assert_eq!(session.provider_profile, "mock");
        assert!(session.last_run_id.is_some());
        assert_eq!(
            transcript_roles,
            vec![
                &TranscriptRole::System,
                &TranscriptRole::User,
                &TranscriptRole::Assistant,
                &TranscriptRole::User,
                &TranscriptRole::Assistant,
            ]
        );
    }

    #[tokio::test]
    async fn tool_loop_executes_time_now_and_records_tool_trace() {
        let sink = Arc::new(VecEventSink::default());
        let runtime = runtime_with_provider(
            Arc::new(MockProvider),
            Arc::new(MemorySessionStore::default()),
            sink.clone(),
        );

        let result = runtime
            .run(RunRequest {
                system: Some("Use tools when needed.".to_owned()),
                input: "What time is it now?".to_owned(),
                skill: None,
                workflow: None,
                session_id: Some("time-demo".to_owned()),
                profile: None,
                ingress: None,
            })
            .await
            .expect("tool loop should succeed");

        assert!(result.output.starts_with("The current time is: "));
        assert_eq!(result.trace.tool_calls.len(), 1);
        assert_eq!(result.trace.session_id.as_deref(), Some("time-demo"));
        assert_eq!(result.trace.tool_calls[0].source, ToolSource::Builtin);
        assert_eq!(result.trace.capability_invocations.len(), 1);
        assert_eq!(result.trace.capability_invocations[0].tool_name, "time_now");
        assert_eq!(
            event_names(&sink.snapshot()),
            vec![
                "RunStarted",
                "ProviderRequest",
                "ToolCalling",
                "CapabilityJobQueued",
                "CapabilityJobStarted",
                "CapabilityJobFinished",
                "ToolFinished",
                "ProviderRequest",
                "FinalAnswerReady",
                "RunFinished",
            ]
        );
    }

    #[tokio::test]
    async fn tool_loop_records_mcp_tool_source_for_remote_tools() {
        let mut config = MosaicConfig::default();
        config.active_profile = "mock".to_owned();
        let profiles =
            ProviderProfileRegistry::from_config(&config).expect("profile registry should build");
        let mut tools = ToolRegistry::new();
        tools.register(Arc::new(FakeMcpReadFileTool::new()));
        let runtime = AgentRuntime::new(RuntimeContext {
            profiles: Arc::new(profiles),
            provider_override: Some(Arc::new(MockProvider)),
            session_store: Arc::new(MemorySessionStore::default()),
            memory_store: Arc::new(MemoryMemoryStore::default()),
            memory_policy: MemoryPolicy::default(),
            tools: Arc::new(tools),
            skills: Arc::new(SkillRegistry::new()),
            workflows: Arc::new(WorkflowRegistry::new()),
            node_router: None,
            active_extensions: Vec::new(),
            event_sink: Arc::new(NoopEventSink),
        });

        let result = runtime
            .run(RunRequest {
                system: Some("Use tools when needed.".to_owned()),
                input: "Read a file for me.".to_owned(),
                skill: None,
                workflow: None,
                session_id: None,
                profile: None,
                ingress: None,
            })
            .await
            .expect("remote MCP tool loop should succeed");

        assert_eq!(result.trace.tool_calls.len(), 1);
        assert_eq!(result.trace.tool_calls[0].name, "mcp.filesystem.read_file");
        assert_eq!(
            result.trace.tool_calls[0].source,
            ToolSource::Mcp {
                server: "filesystem".to_owned(),
                remote_tool: "read_file".to_owned(),
            }
        );
        assert!(
            result
                .output
                .starts_with("I read the file successfully. Preview:\n")
        );
    }

    #[tokio::test]
    async fn tool_loop_routes_read_file_via_node_when_affinity_is_present() {
        let mut config = MosaicConfig::default();
        config.active_profile = "mock".to_owned();
        let profiles =
            ProviderProfileRegistry::from_config(&config).expect("profile registry should build");
        let workspace_root = std::env::temp_dir().join(format!(
            "mosaic-runtime-node-tests-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&workspace_root).expect("workspace root should be created");
        std::fs::write(workspace_root.join("README.md"), "node-routed contents")
            .expect("README should be written");

        let mut tools = ToolRegistry::new();
        tools.register(Arc::new(ReadFileTool::new_with_allowed_roots(vec![
            workspace_root.clone(),
        ])));
        let node_store = Arc::new(FileNodeStore::new(workspace_root.join(".mosaic/nodes")));
        node_store
            .register_node(&NodeRegistration::new(
                "node-a",
                "Headless Node",
                "file-bus",
                "headless",
                vec![NodeCapabilityDeclaration {
                    name: "read_file".to_owned(),
                    kind: CapabilityKind::File,
                    permission_scopes: vec![PermissionScope::LocalRead],
                    risk: ToolRiskLevel::Low,
                }],
            ))
            .expect("node registration should persist");
        node_store
            .attach_session("node-demo", "node-a")
            .expect("node affinity should persist");

        let worker_store = node_store.clone();
        let worker_root = workspace_root.clone();
        let worker = tokio::spawn(async move {
            let tool = ReadFileTool::new_with_allowed_roots(vec![worker_root]);
            loop {
                let pending = worker_store
                    .pending_commands("node-a")
                    .expect("pending commands should load");
                if let Some(dispatch) = pending.into_iter().next() {
                    let result = tool
                        .call(serde_json::json!({
                            "path": workspace_root.join("README.md").display().to_string(),
                        }))
                        .await
                        .expect("node read_file should succeed");
                    worker_store
                        .complete_command(&NodeCommandResultEnvelope::success(&dispatch, result))
                        .expect("node result should persist");
                    break;
                }

                tokio::time::sleep(Duration::from_millis(50)).await;
            }
        });

        let runtime = AgentRuntime::new(RuntimeContext {
            profiles: Arc::new(profiles),
            provider_override: Some(Arc::new(MockProvider)),
            session_store: Arc::new(MemorySessionStore::default()),
            memory_store: Arc::new(MemoryMemoryStore::default()),
            memory_policy: MemoryPolicy::default(),
            tools: Arc::new(tools),
            skills: Arc::new(SkillRegistry::new()),
            workflows: Arc::new(WorkflowRegistry::new()),
            node_router: Some(node_store.clone()),
            active_extensions: Vec::new(),
            event_sink: Arc::new(NoopEventSink),
        });

        let result = runtime
            .run(RunRequest {
                system: Some("Use tools when needed.".to_owned()),
                input: "Read a file for me.".to_owned(),
                skill: None,
                workflow: None,
                session_id: Some("node-demo".to_owned()),
                profile: None,
                ingress: None,
            })
            .await
            .expect("node-routed tool loop should succeed");

        worker.await.expect("node worker should join");

        assert_eq!(result.trace.tool_calls.len(), 1);
        assert_eq!(
            result.trace.tool_calls[0].node_id.as_deref(),
            Some("node-a")
        );
        assert_eq!(
            result.trace.tool_calls[0].capability_route.as_deref(),
            Some("session_affinity")
        );
        assert_eq!(result.trace.capability_invocations.len(), 1);
        assert_eq!(
            result.trace.capability_invocations[0].node_id.as_deref(),
            Some("node-a")
        );
        assert_eq!(
            result.trace.capability_invocations[0]
                .capability_route
                .as_deref(),
            Some("session_affinity")
        );
        assert!(result.output.contains("node-routed contents"));
    }

    #[tokio::test]
    async fn workflow_runs_record_step_trace_and_skill_invocation() {
        let sink = Arc::new(VecEventSink::default());
        let store = Arc::new(MemorySessionStore::default());
        let mut workflows = WorkflowRegistry::new();
        workflows.register(research_workflow());
        let mut config = MosaicConfig::default();
        config.active_profile = "mock".to_owned();
        let profiles =
            ProviderProfileRegistry::from_config(&config).expect("profile registry should build");
        let mut skills = SkillRegistry::new();
        skills.register(Arc::new(SummarizeSkill));
        let mut tools = ToolRegistry::new();
        tools.register(Arc::new(TimeNowTool::new()));

        let runtime = AgentRuntime::new(RuntimeContext {
            profiles: Arc::new(profiles),
            provider_override: Some(Arc::new(MockProvider)),
            session_store: store.clone(),
            memory_store: Arc::new(MemoryMemoryStore::default()),
            memory_policy: MemoryPolicy::default(),
            tools: Arc::new(tools),
            skills: Arc::new(skills),
            workflows: Arc::new(workflows),
            node_router: None,
            active_extensions: Vec::new(),
            event_sink: sink.clone(),
        });

        let result = runtime
            .run(RunRequest {
                system: None,
                input: "Rust async enables efficient concurrency.".to_owned(),
                skill: None,
                workflow: Some("research_brief".to_owned()),
                session_id: Some("workflow-demo".to_owned()),
                profile: None,
                ingress: None,
            })
            .await
            .expect("workflow run should succeed");

        assert_eq!(
            result.trace.workflow_name.as_deref(),
            Some("research_brief")
        );
        assert_eq!(result.trace.step_traces.len(), 2);
        assert_eq!(result.trace.step_traces[0].name, "draft");
        assert_eq!(result.trace.step_traces[0].status(), "success");
        assert_eq!(result.trace.step_traces[1].name, "summarize");
        assert_eq!(result.trace.step_traces[1].status(), "success");
        assert_eq!(result.trace.skill_calls.len(), 1);
        assert!(
            result
                .output
                .starts_with("summary: mock response: Draft notes for:")
        );

        let session = store
            .get("workflow-demo")
            .expect("workflow session should exist");
        assert!(
            session
                .transcript
                .iter()
                .any(|message| message.role == TranscriptRole::Assistant)
        );

        assert_eq!(
            event_names(&sink.snapshot()),
            vec![
                "RunStarted",
                "WorkflowStarted",
                "WorkflowStepStarted",
                "ProviderRequest",
                "WorkflowStepFinished",
                "WorkflowStepStarted",
                "SkillStarted",
                "SkillFinished",
                "WorkflowStepFinished",
                "WorkflowFinished",
                "FinalAnswerReady",
                "RunFinished",
            ]
        );
    }

    #[tokio::test]
    async fn workflow_step_tool_capability_failures_surface_as_run_failures() {
        let sink = Arc::new(VecEventSink::default());
        let store = Arc::new(MemorySessionStore::default());
        let mut config = MosaicConfig::default();
        config.active_profile = "text-only".to_owned();
        config.profiles.clear();
        config.profiles.insert(
            "text-only".to_owned(),
            ProviderProfileConfig {
                provider_type: "plain".to_owned(),
                model: "plain-1".to_owned(),
                base_url: None,
                api_key_env: None,
            },
        );
        let profiles =
            ProviderProfileRegistry::from_config(&config).expect("profile registry should build");
        let mut workflows = WorkflowRegistry::new();
        workflows.register(Workflow {
            name: "tool_step".to_owned(),
            description: None,
            steps: vec![WorkflowStep {
                name: "lookup_time".to_owned(),
                kind: WorkflowStepKind::Prompt {
                    prompt: "What time is it?".to_owned(),
                    system: None,
                    tools: vec!["time_now".to_owned()],
                    profile: None,
                },
            }],
        });
        let mut tools = ToolRegistry::new();
        tools.register(Arc::new(TimeNowTool::new()));

        let runtime = AgentRuntime::new(RuntimeContext {
            profiles: Arc::new(profiles),
            provider_override: Some(Arc::new(MockProvider)),
            session_store: store,
            memory_store: Arc::new(MemoryMemoryStore::default()),
            memory_policy: MemoryPolicy::default(),
            tools: Arc::new(tools),
            skills: Arc::new(SkillRegistry::new()),
            workflows: Arc::new(workflows),
            node_router: None,
            active_extensions: Vec::new(),
            event_sink: sink.clone(),
        });

        let err = runtime
            .run(RunRequest {
                system: None,
                input: "Need the current time".to_owned(),
                skill: None,
                workflow: Some("tool_step".to_owned()),
                session_id: None,
                profile: None,
                ingress: None,
            })
            .await
            .expect_err("tool-capability mismatch should fail");

        assert!(
            err.to_string()
                .contains("does not support tool-enabled workflow steps")
                || err
                    .to_string()
                    .contains("no provider profiles satisfy the requested runtime constraints")
        );
        assert_eq!(
            event_names(&sink.snapshot()),
            vec!["RunStarted", "RunFailed"]
        );
    }

    #[tokio::test]
    async fn empty_provider_response_returns_an_error() {
        let sink = Arc::new(VecEventSink::default());
        let runtime = runtime_with_provider(
            Arc::new(EmptyProvider),
            Arc::new(MemorySessionStore::default()),
            sink.clone(),
        );

        let err = runtime
            .run(RunRequest {
                system: None,
                input: "Explain Mosaic.".to_owned(),
                skill: None,
                workflow: None,
                session_id: None,
                profile: None,
                ingress: None,
            })
            .await
            .expect_err("empty provider response should fail");

        assert!(
            err.to_string()
                .contains("runtime stopped without final assistant message")
        );
        assert_eq!(
            event_names(&sink.snapshot()),
            vec!["RunStarted", "ProviderRequest", "RunFailed"]
        );
    }

    #[tokio::test]
    async fn skill_failures_emit_skill_failed_then_run_failed() {
        let sink = Arc::new(VecEventSink::default());
        let mut config = MosaicConfig::default();
        config.active_profile = "mock".to_owned();

        let profiles =
            ProviderProfileRegistry::from_config(&config).expect("profile registry should build");
        let mut skills = SkillRegistry::new();
        skills.register(Arc::new(FailingSkill));

        let runtime = AgentRuntime::new(RuntimeContext {
            profiles: Arc::new(profiles),
            provider_override: Some(Arc::new(MockProvider)),
            session_store: Arc::new(MemorySessionStore::default()),
            memory_store: Arc::new(MemoryMemoryStore::default()),
            memory_policy: MemoryPolicy::default(),
            tools: Arc::new(ToolRegistry::new()),
            skills: Arc::new(skills),
            workflows: Arc::new(WorkflowRegistry::new()),
            node_router: None,
            active_extensions: Vec::new(),
            event_sink: sink.clone(),
        });

        let err = runtime
            .run(RunRequest {
                system: None,
                input: "boom".to_owned(),
                skill: Some("explode".to_owned()),
                workflow: None,
                session_id: Some("skill-demo".to_owned()),
                profile: None,
                ingress: None,
            })
            .await
            .expect_err("failing skill should fail");

        assert!(err.to_string().contains("skill exploded"));
        assert_eq!(
            event_names(&sink.snapshot()),
            vec!["RunStarted", "SkillStarted", "SkillFailed", "RunFailed"]
        );
    }

    #[tokio::test]
    async fn session_skill_runs_persist_assistant_output() {
        let mut config = MosaicConfig::default();
        config.active_profile = "mock".to_owned();
        let profiles =
            ProviderProfileRegistry::from_config(&config).expect("profile registry should build");
        let store = Arc::new(MemorySessionStore::default());
        let mut skills = SkillRegistry::new();
        skills.register(Arc::new(SummarizeSkill));

        let runtime = AgentRuntime::new(RuntimeContext {
            profiles: Arc::new(profiles),
            provider_override: Some(Arc::new(MockProvider)),
            session_store: store.clone(),
            memory_store: Arc::new(MemoryMemoryStore::default()),
            memory_policy: MemoryPolicy::default(),
            tools: Arc::new(ToolRegistry::new()),
            skills: Arc::new(skills),
            workflows: Arc::new(WorkflowRegistry::new()),
            node_router: None,
            active_extensions: Vec::new(),
            event_sink: Arc::new(NoopEventSink),
        });

        let result = runtime
            .run(RunRequest {
                system: None,
                input: "Rust async enables concurrency.".to_owned(),
                skill: Some("summarize".to_owned()),
                workflow: None,
                session_id: Some("summary-demo".to_owned()),
                profile: None,
                ingress: None,
            })
            .await
            .expect("skill run should succeed");

        let session = store.get("summary-demo").expect("session should exist");
        assert_eq!(result.trace.session_id.as_deref(), Some("summary-demo"));
        assert!(
            session
                .transcript
                .iter()
                .any(|message: &TranscriptMessage| message.content.contains("summary:"))
        );
    }

    #[tokio::test]
    async fn session_runs_persist_memory_and_record_compression_trace() {
        let mut config = MosaicConfig::default();
        config.active_profile = "mock".to_owned();
        let profiles =
            ProviderProfileRegistry::from_config(&config).expect("profile registry should build");
        let store = Arc::new(MemorySessionStore::default());
        let memory_store = Arc::new(MemoryMemoryStore::default());

        let runtime = AgentRuntime::new(RuntimeContext {
            profiles: Arc::new(profiles),
            provider_override: Some(Arc::new(MockProvider)),
            session_store: store.clone(),
            memory_store: memory_store.clone(),
            memory_policy: MemoryPolicy {
                compression_message_threshold: 3,
                recent_message_window: 2,
                summary_char_budget: 160,
                note_char_budget: 120,
            },
            tools: Arc::new(ToolRegistry::new()),
            skills: Arc::new(SkillRegistry::new()),
            workflows: Arc::new(WorkflowRegistry::new()),
            node_router: None,
            active_extensions: Vec::new(),
            event_sink: Arc::new(NoopEventSink),
        });

        runtime
            .run(RunRequest {
                system: Some("You are helpful.".to_owned()),
                input: "first turn".to_owned(),
                skill: None,
                workflow: None,
                session_id: Some("memory-demo".to_owned()),
                profile: None,
                ingress: None,
            })
            .await
            .expect("first run should succeed");

        let result = runtime
            .run(RunRequest {
                system: Some("You are helpful.".to_owned()),
                input: "second turn should reuse compressed context".to_owned(),
                skill: None,
                workflow: None,
                session_id: Some("memory-demo".to_owned()),
                profile: None,
                ingress: None,
            })
            .await
            .expect("second run should succeed");

        let session = store.get("memory-demo").expect("session should exist");
        let memory = memory_store
            .get("memory-demo")
            .expect("memory record should exist");

        assert!(session.memory.latest_summary.is_some());
        assert!(memory.summary.is_some());
        assert!(!result.trace.memory_writes.is_empty());
        assert!(result.trace.compression.is_some());
    }

    #[tokio::test]
    async fn cross_session_reference_records_memory_reads_and_session_links() {
        let mut config = MosaicConfig::default();
        config.active_profile = "mock".to_owned();
        let profiles =
            ProviderProfileRegistry::from_config(&config).expect("profile registry should build");
        let store = Arc::new(MemorySessionStore::default());
        let memory_store = Arc::new(MemoryMemoryStore::default());
        memory_store
            .save_session(&{
                let mut record = SessionMemoryRecord::new("source-session");
                record.set_summary(Some("Source session summary".to_owned()));
                record
            })
            .expect("memory seed should save");

        let runtime = AgentRuntime::new(RuntimeContext {
            profiles: Arc::new(profiles),
            provider_override: Some(Arc::new(MockProvider)),
            session_store: store.clone(),
            memory_store: memory_store.clone(),
            memory_policy: MemoryPolicy::default(),
            tools: Arc::new(ToolRegistry::new()),
            skills: Arc::new(SkillRegistry::new()),
            workflows: Arc::new(WorkflowRegistry::new()),
            node_router: None,
            active_extensions: Vec::new(),
            event_sink: Arc::new(NoopEventSink),
        });

        let result = runtime
            .run(RunRequest {
                system: Some("You are helpful.".to_owned()),
                input: "Please use [[session:source-session]] for context".to_owned(),
                skill: None,
                workflow: None,
                session_id: Some("target-session".to_owned()),
                profile: None,
                ingress: None,
            })
            .await
            .expect("run should succeed");

        let session = store
            .get("target-session")
            .expect("target session should exist");
        assert!(result.trace.memory_reads.iter().any(|read| {
            read.session_id == "source-session" && read.source == "cross_session_reference"
        }));
        assert_eq!(session.references.len(), 1);
        assert_eq!(session.references[0].session_id, "source-session");
    }
}
