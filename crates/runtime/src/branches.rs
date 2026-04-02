use std::sync::{Arc, Mutex};

use anyhow::anyhow;
use mosaic_inspect::OrchestrationOwner;
use mosaic_provider::ProviderProfile;
use mosaic_session_core::{SessionRecord, TranscriptRole};
use mosaic_workflow::WorkflowRunner;

use crate::{
    AgentRuntime, RunRequest, RunResult,
    workflow::{
        RuntimeWorkflowExecutor, RuntimeWorkflowObserver, drain_capability_trace_collector,
        drain_model_selection_collector, drain_skill_trace_collector, drain_tool_trace_collector,
    },
};

impl AgentRuntime {
    pub(crate) async fn run_tool(
        &self,
        req: RunRequest,
        tool_name: String,
        mut trace: mosaic_inspect::RunTrace,
        mut session: Option<SessionRecord>,
    ) -> std::result::Result<RunResult, crate::RunError> {
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

        let Some(tool) = self.ctx.tools.get(&tool_name) else {
            return self.fail_run(trace, anyhow!("tool not found: {}", tool_name));
        };
        if let Some(usage) = Self::tool_extension_usage(tool.metadata()) {
            trace.record_extension_usage(usage);
        }

        let attachments = Self::request_attachments(&req);
        let parsed_input = Self::with_attachment_metadata(
            Self::parse_direct_tool_input(tool.metadata(), &req.input),
            &attachments,
        );
        let outcome = match self
            .invoke_tool_with_guardrails(
                session.as_ref().map(|record| record.id.as_str()),
                tool_name.clone(),
                format!("direct_tool_{}", trace.run_id),
                parsed_input,
                trace
                    .sandbox_run
                    .as_ref()
                    .map(|sandbox| std::path::Path::new(&sandbox.workdir)),
                OrchestrationOwner::Runtime,
            )
            .await
        {
            Ok(outcome) => outcome,
            Err(failure) => {
                if let Some(tool_trace) = failure.tool_trace {
                    trace.tool_calls.push(tool_trace);
                }
                if let Some(capability_trace) = failure.capability_trace {
                    trace.add_capability_invocation(capability_trace);
                }
                return self.fail_run(trace, failure.error);
            }
        };

        trace.tool_calls.push(outcome.tool_trace);
        trace.add_capability_invocation(outcome.capability_trace);

        if let Some(session_ref) = session.as_mut() {
            if let Err(err) = self.append_session_message(
                session_ref,
                TranscriptRole::Assistant,
                outcome.output.clone(),
                None,
            ) {
                return self.fail_run(trace, err);
            }
            if let Err(err) = self.persist_session_memory(session_ref, &mut trace) {
                return self.fail_run(trace, err);
            }
        }

        self.emit_output_deltas(&mut trace, &outcome.output);
        self.emit(crate::events::RunEvent::FinalAnswerReady {
            run_id: trace.run_id.clone(),
        });
        self.emit(crate::events::RunEvent::RunFinished {
            run_id: trace.run_id.clone(),
            output_preview: Self::truncate_preview(&outcome.output, 120),
        });
        trace.finish_ok(outcome.output.clone());

        Ok(RunResult {
            output: outcome.output,
            trace,
        })
    }

    pub(crate) async fn run_plain_assistant(
        &self,
        req: RunRequest,
        profile: ProviderProfile,
        mut trace: mosaic_inspect::RunTrace,
        mut session: Option<SessionRecord>,
    ) -> std::result::Result<RunResult, crate::RunError> {
        let reference_contexts =
            match self.resolve_cross_session_contexts(session.as_mut(), &req.input, &mut trace) {
                Ok(contexts) => contexts,
                Err(err) => return self.fail_run(trace, err),
            };
        let attachments = Self::request_attachments(&req);

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
                &attachments,
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
        self.emit(crate::events::RunEvent::FinalAnswerReady {
            run_id: trace.run_id.clone(),
        });
        self.emit(crate::events::RunEvent::RunFinished {
            run_id: trace.run_id.clone(),
            output_preview: Self::truncate_preview(&output, 120),
        });
        trace.finish_ok(output.clone());

        Ok(RunResult { output, trace })
    }

    pub(crate) async fn run_skill(
        &self,
        req: RunRequest,
        skill_name: String,
        mut trace: mosaic_inspect::RunTrace,
        mut session: Option<SessionRecord>,
    ) -> std::result::Result<RunResult, crate::RunError> {
        let reference_contexts =
            match self.resolve_cross_session_contexts(session.as_mut(), &req.input, &mut trace) {
                Ok(contexts) => contexts,
                Err(err) => return self.fail_run(trace, err),
            };
        let attachments = Self::request_attachments(&req);
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
            .execute_skill_for_trace(skill_name.clone(), skill_input, &attachments, &mut trace)
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
        self.emit(crate::events::RunEvent::FinalAnswerReady {
            run_id: trace.run_id.clone(),
        });
        self.emit(crate::events::RunEvent::RunFinished {
            run_id: trace.run_id.clone(),
            output_preview: Self::truncate_preview(&output, 120),
        });
        trace.finish_ok(output.clone());

        Ok(RunResult { output, trace })
    }

    pub(crate) async fn run_workflow(
        &self,
        req: RunRequest,
        workflow_name: String,
        profile: ProviderProfile,
        mut trace: mosaic_inspect::RunTrace,
        mut session: Option<SessionRecord>,
    ) -> std::result::Result<RunResult, crate::RunError> {
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
        let attachments = Self::request_attachments(&req);
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
                ingress_bot_name: req
                    .ingress
                    .as_ref()
                    .and_then(|ingress| ingress.bot_name.clone()),
                attachments: attachments.clone(),
                run_workdir: trace
                    .sandbox_run
                    .as_ref()
                    .map(|sandbox| std::path::PathBuf::from(&sandbox.workdir)),
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
        self.emit(crate::events::RunEvent::FinalAnswerReady {
            run_id: trace.run_id.clone(),
        });
        self.emit(crate::events::RunEvent::RunFinished {
            run_id: trace.run_id.clone(),
            output_preview: Self::truncate_preview(&output, 120),
        });
        trace.finish_ok(output.clone());

        Ok(RunResult { output, trace })
    }
}
