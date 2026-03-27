use super::*;

impl AgentRuntime {
    pub(crate) fn truncate_preview(value: &str, limit: usize) -> String {
        if value.chars().count() <= limit {
            return value.to_owned();
        }

        let truncated: String = value.chars().take(limit).collect();
        format!("{truncated}...")
    }

    pub(crate) fn failure_trace(trace: &RunTrace, error: &anyhow::Error) -> RunFailureTrace {
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

    pub(crate) fn emit_output_deltas(&self, trace: &mut RunTrace, output: &str) {
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

    pub(crate) fn fail_run<T>(
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
}
