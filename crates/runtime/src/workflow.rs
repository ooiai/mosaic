use super::*;

pub(crate) struct RuntimeWorkflowExecutor<'a> {
    pub(crate) runtime: &'a AgentRuntime,
    pub(crate) default_profile: ProviderProfile,
    pub(crate) session_id: Option<String>,
    pub(crate) ingress_channel: Option<String>,
    pub(crate) tool_traces: SharedToolTraceCollector,
    pub(crate) skill_traces: SharedSkillTraceCollector,
    pub(crate) model_selections: SharedModelSelectionCollector,
    pub(crate) capability_traces: SharedCapabilityTraceCollector,
}

impl RuntimeWorkflowExecutor<'_> {
    pub(crate) fn resolve_prompt_profile(
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

pub(crate) struct RuntimeWorkflowObserver<'a> {
    pub(crate) runtime: &'a AgentRuntime,
    pub(crate) trace: &'a mut RunTrace,
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

pub(crate) fn push_tool_trace(collector: &SharedToolTraceCollector, trace: ToolTrace) {
    collector
        .lock()
        .expect("tool trace collector should not be poisoned")
        .push(trace);
}

pub(crate) fn drain_tool_trace_collector(collector: &SharedToolTraceCollector) -> Vec<ToolTrace> {
    std::mem::take(
        &mut *collector
            .lock()
            .expect("tool trace collector should not be poisoned"),
    )
}

pub(crate) fn push_capability_trace(
    collector: &SharedCapabilityTraceCollector,
    trace: CapabilityInvocationTrace,
) {
    collector
        .lock()
        .expect("capability trace collector should not be poisoned")
        .push(trace);
}

pub(crate) fn drain_capability_trace_collector(
    collector: &SharedCapabilityTraceCollector,
) -> Vec<CapabilityInvocationTrace> {
    std::mem::take(
        &mut *collector
            .lock()
            .expect("capability trace collector should not be poisoned"),
    )
}

pub(crate) fn push_skill_trace(collector: &SharedSkillTraceCollector, trace: SkillTrace) {
    collector
        .lock()
        .expect("skill trace collector should not be poisoned")
        .push(trace);
}

pub(crate) fn update_skill_trace<F>(collector: &SharedSkillTraceCollector, name: &str, update: F)
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

pub(crate) fn drain_skill_trace_collector(
    collector: &SharedSkillTraceCollector,
) -> Vec<SkillTrace> {
    std::mem::take(
        &mut *collector
            .lock()
            .expect("skill trace collector should not be poisoned"),
    )
}

pub(crate) fn push_model_selection(
    collector: &SharedModelSelectionCollector,
    trace: ModelSelectionTrace,
) {
    collector
        .lock()
        .expect("model selection collector should not be poisoned")
        .push(trace);
}

pub(crate) fn drain_model_selection_collector(
    collector: &SharedModelSelectionCollector,
) -> Vec<ModelSelectionTrace> {
    std::mem::take(
        &mut *collector
            .lock()
            .expect("model selection collector should not be poisoned"),
    )
}
