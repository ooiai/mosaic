pub mod events;

use std::sync::{Arc, Mutex};

use anyhow::{Result, anyhow};
use async_trait::async_trait;
use chrono::Utc;
use mosaic_inspect::{EffectiveProfileTrace, RunTrace, SkillTrace, ToolTrace, WorkflowStepTrace};
use mosaic_provider::{
    LlmProvider, Message, ProviderProfile, ProviderProfileRegistry, Role, ToolDefinition,
    validate_step_tools_support,
};
use mosaic_session_core::{SessionRecord, SessionStore, TranscriptRole, session_title_from_input};
use mosaic_skill_core::{SkillContext, SkillRegistry};
use mosaic_tool_core::ToolRegistry;
use mosaic_workflow::{
    Workflow, WorkflowObserver, WorkflowRegistry, WorkflowRunner, WorkflowStep,
    WorkflowStepExecutor, WorkflowStepKind,
};

use crate::events::{RunEvent, SharedRunEventSink};

type SharedToolTraceCollector = Arc<Mutex<Vec<ToolTrace>>>;
type SharedSkillTraceCollector = Arc<Mutex<Vec<SkillTrace>>>;

pub struct RuntimeContext {
    pub profiles: Arc<ProviderProfileRegistry>,
    pub provider_override: Option<Arc<dyn LlmProvider>>,
    pub session_store: Arc<dyn SessionStore>,
    pub tools: Arc<ToolRegistry>,
    pub skills: Arc<SkillRegistry>,
    pub workflows: Arc<WorkflowRegistry>,
    pub event_sink: SharedRunEventSink,
}

pub struct RunRequest {
    pub system: Option<String>,
    pub input: String,
    pub skill: Option<String>,
    pub workflow: Option<String>,
    pub session_id: Option<String>,
    pub profile: Option<String>,
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

        let profile = match self.ctx.profiles.resolve(req.profile.as_deref()) {
            Ok(profile) => profile,
            Err(err) => return self.fail_run(trace, err),
        };
        trace.bind_effective_profile(Self::effective_profile_trace(&profile));

        let session = match self.load_session(&req, &profile, &mut trace) {
            Ok(session) => session,
            Err(err) => return self.fail_run(trace, err),
        };

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
                &profile,
                session.as_mut(),
                &mut trace,
            )
            .await
        {
            Ok(output) => output,
            Err(err) => return self.fail_run(trace, err),
        };

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
            .execute_skill_for_trace(skill_name.clone(), req.input, &mut trace)
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
        let runner = WorkflowRunner::new();
        let workflow_input = req.input;

        let workflow_result = {
            let executor = RuntimeWorkflowExecutor {
                runtime: self,
                default_profile: profile,
                tool_traces: tool_traces.clone(),
                skill_traces: skill_traces.clone(),
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
            Self::session_messages(session_ref)
        } else {
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
                    let tool_name = call.name.clone();
                    let tool_input = call.arguments.clone();

                    self.emit(RunEvent::ToolCalling {
                        name: tool_name.clone(),
                        call_id: call_id.clone(),
                    });

                    let tool = match self.ctx.tools.get(&tool_name) {
                        Some(tool) => tool,
                        None => {
                            let err = anyhow!("tool not found: {}", tool_name);
                            self.emit(RunEvent::ToolFailed {
                                name: tool_name,
                                call_id,
                                error: err.to_string(),
                            });
                            return Err(err);
                        }
                    };
                    let tool_source = tool.metadata().source.clone();

                    trace.tool_calls.push(ToolTrace {
                        call_id: Some(call_id.clone()),
                        name: tool_name.clone(),
                        source: tool_source,
                        input: tool_input,
                        output: None,
                        started_at: Utc::now(),
                        finished_at: None,
                    });

                    match tool.call(call.arguments).await {
                        Ok(result) => {
                            if let Some(last) = trace.tool_calls.last_mut() {
                                last.output = Some(result.content.clone());
                                last.finished_at = Some(Utc::now());
                            }

                            self.emit(RunEvent::ToolFinished {
                                name: tool_name.clone(),
                                call_id: call_id.clone(),
                            });

                            if let Some(session_ref) = session.as_deref_mut() {
                                self.append_session_message(
                                    session_ref,
                                    TranscriptRole::Tool,
                                    result.content.clone(),
                                    Some(call_id.clone()),
                                )?;
                                messages = Self::session_messages(session_ref);
                            } else {
                                messages.push(Message {
                                    role: Role::Tool,
                                    content: result.content,
                                    tool_call_id: Some(call_id),
                                });
                            }
                        }
                        Err(err) => {
                            if let Some(last) = trace.tool_calls.last_mut() {
                                last.output = Some(format!("[runtime tool failure] {}", err));
                                last.finished_at = Some(Utc::now());
                            }

                            self.emit(RunEvent::ToolFailed {
                                name: tool_name,
                                call_id,
                                error: err.to_string(),
                            });
                            return Err(err);
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
        profile: &ProviderProfile,
        system: Option<String>,
        input: String,
        tool_defs: Vec<ToolDefinition>,
        tool_traces: &SharedToolTraceCollector,
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
                    let tool_name = call.name.clone();
                    let tool_input = call.arguments.clone();

                    self.emit(RunEvent::ToolCalling {
                        name: tool_name.clone(),
                        call_id: call_id.clone(),
                    });

                    let tool = match self.ctx.tools.get(&tool_name) {
                        Some(tool) => tool,
                        None => {
                            let err = anyhow!("tool not found: {}", tool_name);
                            self.emit(RunEvent::ToolFailed {
                                name: tool_name,
                                call_id,
                                error: err.to_string(),
                            });
                            return Err(err);
                        }
                    };
                    let tool_source = tool.metadata().source.clone();

                    push_tool_trace(
                        tool_traces,
                        ToolTrace {
                            call_id: Some(call_id.clone()),
                            name: tool_name.clone(),
                            source: tool_source,
                            input: tool_input,
                            output: None,
                            started_at: Utc::now(),
                            finished_at: None,
                        },
                    );

                    match tool.call(call.arguments).await {
                        Ok(result) => {
                            update_tool_trace(tool_traces, &call_id, |trace| {
                                trace.output = Some(result.content.clone());
                                trace.finished_at = Some(Utc::now());
                            });

                            self.emit(RunEvent::ToolFinished {
                                name: tool_name.clone(),
                                call_id: call_id.clone(),
                            });

                            messages.push(Message {
                                role: Role::Tool,
                                content: result.content,
                                tool_call_id: Some(call_id),
                            });
                        }
                        Err(err) => {
                            update_tool_trace(tool_traces, &call_id, |trace| {
                                trace.output = Some(format!("[runtime tool failure] {}", err));
                                trace.finished_at = Some(Utc::now());
                            });

                            self.emit(RunEvent::ToolFailed {
                                name: tool_name,
                                call_id,
                                error: err.to_string(),
                            });
                            return Err(err);
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

    fn effective_profile_trace(profile: &ProviderProfile) -> EffectiveProfileTrace {
        EffectiveProfileTrace {
            profile: profile.name.clone(),
            provider_type: profile.provider_type.clone(),
            model: profile.model.clone(),
            api_key_env: profile.api_key_env.clone(),
            api_key_present: profile.api_key_present(),
        }
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
                    let meta = tool.metadata().clone();
                    Ok(ToolDefinition {
                        name: meta.name,
                        description: meta.description,
                        input_schema: meta.input_schema,
                    })
                })
                .collect(),
            None => Ok(self
                .ctx
                .tools
                .iter()
                .map(|tool| {
                    let meta = tool.metadata().clone();
                    ToolDefinition {
                        name: meta.name,
                        description: meta.description,
                        input_schema: meta.input_schema,
                    }
                })
                .collect()),
        }
    }
}

struct RuntimeWorkflowExecutor<'a> {
    runtime: &'a AgentRuntime,
    default_profile: ProviderProfile,
    tool_traces: SharedToolTraceCollector,
    skill_traces: SharedSkillTraceCollector,
}

impl RuntimeWorkflowExecutor<'_> {
    fn resolve_step_profile(&self, step: &WorkflowStep) -> Result<ProviderProfile> {
        match &step.kind {
            WorkflowStepKind::Prompt { profile, .. } => self
                .runtime
                .ctx
                .profiles
                .resolve(profile.as_deref().or(Some(&self.default_profile.name))),
            WorkflowStepKind::Skill { .. } => Ok(self.default_profile.clone()),
        }
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

        let profile = self.resolve_step_profile(step)?;
        validate_step_tools_support(&profile, tools)?;
        let tool_defs = if profile.capabilities.supports_tools {
            self.runtime.collect_tool_definitions(Some(tools))?
        } else {
            Vec::new()
        };

        self.runtime
            .execute_workflow_prompt_step(
                &profile,
                system.clone(),
                input,
                tool_defs,
                &self.tool_traces,
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

fn update_tool_trace<F>(collector: &SharedToolTraceCollector, call_id: &str, update: F)
where
    F: FnOnce(&mut ToolTrace),
{
    if let Some(trace) = collector
        .lock()
        .expect("tool trace collector should not be poisoned")
        .iter_mut()
        .rev()
        .find(|trace| trace.call_id.as_deref() == Some(call_id) && trace.finished_at.is_none())
    {
        update(trace);
    }
}

fn drain_tool_trace_collector(collector: &SharedToolTraceCollector) -> Vec<ToolTrace> {
    std::mem::take(
        &mut *collector
            .lock()
            .expect("tool trace collector should not be poisoned"),
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

#[cfg(test)]
mod tests {
    use std::{
        collections::BTreeMap,
        sync::{Arc, Mutex},
    };

    use crate::events::{NoopEventSink, RunEvent, RunEventSink};
    use anyhow::{Result, anyhow};
    use async_trait::async_trait;
    use mosaic_config::{MosaicConfig, ProviderProfileConfig};
    use mosaic_provider::{
        CompletionResponse, LlmProvider, Message, MockProvider, ProviderProfileRegistry,
        ToolDefinition,
    };
    use mosaic_session_core::{
        SessionRecord, SessionStore, SessionSummary, TranscriptMessage, TranscriptRole,
    };
    use mosaic_skill_core::{SkillRegistry, SummarizeSkill};
    use mosaic_tool_core::{TimeNowTool, Tool, ToolMetadata, ToolRegistry, ToolResult, ToolSource};
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
            tools: Arc::new(tools),
            skills: Arc::new(SkillRegistry::new()),
            workflows: Arc::new(workflows),
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
            })
            .await
            .expect("tool loop should succeed");

        assert!(result.output.starts_with("The current time is: "));
        assert_eq!(result.trace.tool_calls.len(), 1);
        assert_eq!(result.trace.session_id.as_deref(), Some("time-demo"));
        assert_eq!(result.trace.tool_calls[0].source, ToolSource::Builtin);
        assert_eq!(
            event_names(&sink.snapshot()),
            vec![
                "RunStarted",
                "ProviderRequest",
                "ToolCalling",
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
            tools: Arc::new(tools),
            skills: Arc::new(SkillRegistry::new()),
            workflows: Arc::new(WorkflowRegistry::new()),
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
            tools: Arc::new(tools),
            skills: Arc::new(skills),
            workflows: Arc::new(workflows),
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
            tools: Arc::new(tools),
            skills: Arc::new(SkillRegistry::new()),
            workflows: Arc::new(workflows),
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
            })
            .await
            .expect_err("tool-capability mismatch should fail");

        assert!(
            err.to_string()
                .contains("does not support tool-enabled workflow steps")
        );
        assert_eq!(
            event_names(&sink.snapshot()),
            vec![
                "RunStarted",
                "WorkflowStarted",
                "WorkflowStepStarted",
                "WorkflowStepFailed",
                "RunFailed",
            ]
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
            tools: Arc::new(ToolRegistry::new()),
            skills: Arc::new(skills),
            workflows: Arc::new(WorkflowRegistry::new()),
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
            tools: Arc::new(ToolRegistry::new()),
            skills: Arc::new(skills),
            workflows: Arc::new(WorkflowRegistry::new()),
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
}
