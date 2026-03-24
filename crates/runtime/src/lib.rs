pub mod events;

use std::sync::Arc;

use crate::events::{RunEvent, SharedRunEventSink};
use anyhow::{Result, anyhow};
use chrono::Utc;
use mosaic_inspect::{RunTrace, SkillTrace, ToolTrace};
use mosaic_provider::{LlmProvider, Message, Role, ToolDefinition};
use mosaic_skill_core::{SkillContext, SkillRegistry};
use mosaic_tool_core::ToolRegistry;

pub struct RuntimeContext {
    pub provider: Arc<dyn LlmProvider>,
    pub tools: Arc<ToolRegistry>,
    pub skills: Arc<SkillRegistry>,
    pub event_sink: SharedRunEventSink,
}

pub struct RunRequest {
    pub system: Option<String>,
    pub input: String,
    pub skill: Option<String>,
}

#[derive(Debug)]
pub struct RunResult {
    pub output: String,
    pub trace: RunTrace,
}

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
            return value.to_string();
        }

        let truncated: String = value.chars().take(limit).collect();
        format!("{truncated}...")
    }

    pub async fn run(&self, req: RunRequest) -> Result<RunResult> {
        let mut trace = RunTrace::new(req.input.clone());

        self.emit(RunEvent::RunStarted {
            input: trace.input.clone(),
        });

        if let Some(skill_name) = req.skill.clone() {
            let skill = self
                .ctx
                .skills
                .get(&skill_name)
                .ok_or_else(|| anyhow!("skill not found: {}", skill_name))?;

            self.emit(RunEvent::SkillStarted {
                name: skill_name.clone(),
            });

            let skill_input = serde_json::json!({ "text": req.input });

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
                Ok(out) => {
                    if let Some(last) = trace.skill_calls.last_mut() {
                        last.output = Some(out.content.clone());
                        last.finished_at = Some(Utc::now());
                    }

                    self.emit(RunEvent::SkillFinished {
                        name: skill_name.clone(),
                    });
                    self.emit(RunEvent::FinalAnswerReady);
                    self.emit(RunEvent::RunFinished {
                        output_preview: Self::truncate_preview(&out.content, 120),
                    });

                    trace.finish_ok(out.content.clone());

                    return Ok(RunResult {
                        output: out.content,
                        trace,
                    });
                }
                Err(err) => {
                    if let Some(last) = trace.skill_calls.last_mut() {
                        last.finished_at = Some(Utc::now());
                    }

                    self.emit(RunEvent::SkillFailed {
                        name: skill_name.clone(),
                        error: err.to_string(),
                    });
                    self.emit(RunEvent::RunFailed {
                        error: err.to_string(),
                    });

                    trace.finish_err(err.to_string());
                    return Err(err);
                }
            }
        }

        let mut messages = Vec::new();

        if let Some(system) = req.system {
            messages.push(Message {
                role: Role::System,
                content: system,
                tool_call_id: None,
            });
        }

        messages.push(Message {
            role: Role::User,
            content: req.input,
            tool_call_id: None,
        });

        let tool_defs = self.collect_tool_definitions();
        let provider_tools = (!tool_defs.is_empty()).then_some(tool_defs.as_slice());

        for _ in 0..8 {
            self.emit(RunEvent::ProviderRequest {
                tool_count: tool_defs.len(),
                message_count: messages.len(),
            });

            let response = match self.ctx.provider.complete(&messages, provider_tools).await {
                Ok(response) => response,
                Err(err) => {
                    self.emit(RunEvent::RunFailed {
                        error: err.to_string(),
                    });
                    trace.finish_err(err.to_string());
                    return Err(err);
                }
            };

            if !response.tool_calls.is_empty() {
                for call in response.tool_calls {
                    let call_id = call.id.clone();
                    let tool_name = call.name.clone();
                    let tool_input = call.arguments.clone();

                    self.emit(RunEvent::ToolCalling {
                        name: tool_name.clone(),
                        call_id: call_id.clone(),
                    });

                    trace.tool_calls.push(ToolTrace {
                        call_id: Some(call_id.clone()),
                        name: tool_name.clone(),
                        input: tool_input,
                        output: None,
                        started_at: Utc::now(),
                        finished_at: None,
                    });

                    let tool = match self.ctx.tools.get(&tool_name) {
                        Some(tool) => tool,
                        None => {
                            if let Some(last) = trace.tool_calls.last_mut() {
                                last.output = Some(format!(
                                    "[runtime tool failure] tool not found: {}",
                                    tool_name
                                ));
                                last.finished_at = Some(Utc::now());
                            }

                            let err = anyhow!("tool not found: {}", tool_name);
                            self.emit(RunEvent::ToolFailed {
                                name: tool_name.clone(),
                                call_id: call_id.clone(),
                                error: err.to_string(),
                            });
                            self.emit(RunEvent::RunFailed {
                                error: err.to_string(),
                            });
                            trace.finish_err(err.to_string());
                            return Err(err);
                        }
                    };

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

                            messages.push(Message {
                                role: Role::Tool,
                                content: result.content,
                                tool_call_id: Some(call_id),
                            });
                        }
                        Err(err) => {
                            if let Some(last) = trace.tool_calls.last_mut() {
                                last.output = Some(format!("[runtime tool failure] {}", err));
                                last.finished_at = Some(Utc::now());
                            }

                            self.emit(RunEvent::ToolFailed {
                                name: tool_name.clone(),
                                call_id: call_id.clone(),
                                error: err.to_string(),
                            });
                            self.emit(RunEvent::RunFailed {
                                error: err.to_string(),
                            });

                            trace.finish_err(err.to_string());
                            return Err(err);
                        }
                    }
                }

                continue;
            }

            if let Some(message) = response.message {
                self.emit(RunEvent::FinalAnswerReady);
                self.emit(RunEvent::RunFinished {
                    output_preview: Self::truncate_preview(&message.content, 120),
                });
                trace.finish_ok(message.content.clone());

                return Ok(RunResult {
                    output: message.content,
                    trace,
                });
            }

            break;
        }

        let err = anyhow!("runtime stopped without final assistant message");
        self.emit(RunEvent::RunFailed {
            error: err.to_string(),
        });
        trace.finish_err(err.to_string());
        Err(err)
    }

    fn collect_tool_definitions(&self) -> Vec<ToolDefinition> {
        self.ctx
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
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use crate::events::{NoopEventSink, RunEvent, RunEventSink};
    use anyhow::{Result, anyhow};
    use async_trait::async_trait;
    use mosaic_provider::{CompletionResponse, LlmProvider, Message, MockProvider, ToolDefinition};
    use mosaic_skill_core::{SkillRegistry, SummarizeSkill};
    use mosaic_tool_core::{TimeNowTool, ToolRegistry};

    use super::{AgentRuntime, RunRequest, RuntimeContext};

    #[derive(Default)]
    struct RecordingEventSink {
        events: Mutex<Vec<RunEvent>>,
    }

    impl RecordingEventSink {
        fn snapshot(&self) -> Vec<RunEvent> {
            self.events
                .lock()
                .expect("event lock should not be poisoned")
                .clone()
        }
    }

    impl RunEventSink for RecordingEventSink {
        fn emit(&self, event: RunEvent) {
            self.events
                .lock()
                .expect("event lock should not be poisoned")
                .push(event);
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

    struct FailingProvider;

    #[async_trait]
    impl LlmProvider for FailingProvider {
        async fn complete(
            &self,
            _messages: &[Message],
            _tools: Option<&[ToolDefinition]>,
        ) -> Result<CompletionResponse> {
            Err(anyhow!("provider failure"))
        }
    }

    fn runtime_with_provider(provider: Arc<dyn LlmProvider>) -> AgentRuntime {
        AgentRuntime::new(RuntimeContext {
            provider,
            tools: Arc::new(ToolRegistry::new()),
            skills: Arc::new(SkillRegistry::new()),
            event_sink: Arc::new(NoopEventSink),
        })
    }

    #[tokio::test]
    async fn provider_only_run_returns_mock_output() {
        let sink = Arc::new(RecordingEventSink::default());
        let runtime = AgentRuntime::new(RuntimeContext {
            provider: Arc::new(MockProvider),
            tools: Arc::new(ToolRegistry::new()),
            skills: Arc::new(SkillRegistry::new()),
            event_sink: sink.clone(),
        });

        let result = runtime
            .run(RunRequest {
                system: Some("You are helpful.".to_owned()),
                input: "Explain Mosaic.".to_owned(),
                skill: None,
            })
            .await
            .expect("runtime should succeed");

        assert_eq!(result.output, "mock response: Explain Mosaic.");
        assert!(result.trace.error.is_none());

        let events = sink.snapshot();

        assert!(matches!(events.first(), Some(RunEvent::RunStarted { .. })));
        assert!(events.iter().any(|event| matches!(
            event,
            RunEvent::ProviderRequest {
                tool_count: 0,
                message_count: 2,
            }
        )));
        assert!(
            events
                .iter()
                .any(|event| matches!(event, RunEvent::FinalAnswerReady))
        );
        assert!(
            events
                .iter()
                .any(|event| matches!(event, RunEvent::RunFinished { .. }))
        );
    }

    #[tokio::test]
    async fn skill_dispatch_records_a_skill_trace() {
        let mut skills = SkillRegistry::new();
        skills.register(Arc::new(SummarizeSkill));

        let runtime = AgentRuntime::new(RuntimeContext {
            provider: Arc::new(MockProvider),
            tools: Arc::new(ToolRegistry::new()),
            skills: Arc::new(skills),
            event_sink: Arc::new(NoopEventSink),
        });

        let result = runtime
            .run(RunRequest {
                system: None,
                input: "Rust async enables concurrency.".to_owned(),
                skill: Some("summarize".to_owned()),
            })
            .await
            .expect("skill run should succeed");

        assert_eq!(result.output, "summary: Rust async enables concurrency.");
        assert_eq!(result.trace.skill_calls.len(), 1);
        assert_eq!(
            result.trace.skill_calls[0].output.as_deref(),
            Some("summary: Rust async enables concurrency.")
        );
        assert!(result.trace.skill_calls[0].finished_at.is_some());
    }

    #[tokio::test]
    async fn tool_loop_executes_time_now_and_records_tool_trace() {
        let sink = Arc::new(RecordingEventSink::default());
        let mut tools = ToolRegistry::new();
        tools.register(Arc::new(TimeNowTool::new()));

        let runtime = AgentRuntime::new(RuntimeContext {
            provider: Arc::new(MockProvider),
            tools: Arc::new(tools),
            skills: Arc::new(SkillRegistry::new()),
            event_sink: sink.clone(),
        });

        let result = runtime
            .run(RunRequest {
                system: Some("Use tools when needed.".to_owned()),
                input: "What time is it now?".to_owned(),
                skill: None,
            })
            .await
            .expect("tool loop should succeed");

        assert!(result.output.starts_with("The current time is: "));
        assert_eq!(result.trace.tool_calls.len(), 1);
        assert_eq!(
            result.trace.tool_calls[0].call_id.as_deref(),
            Some("call_mock_time_now")
        );
        assert_eq!(result.trace.tool_calls[0].name, "time_now");
        assert!(result.trace.tool_calls[0].finished_at.is_some());
        assert!(result.trace.tool_calls[0].output.is_some());

        let events = sink.snapshot();

        assert!(events.iter().any(|event| matches!(
            event,
            RunEvent::ToolCalling { name, call_id }
                if name == "time_now" && call_id == "call_mock_time_now"
        )));
        assert!(events.iter().any(|event| matches!(
            event,
            RunEvent::ToolFinished { name, call_id }
                if name == "time_now" && call_id == "call_mock_time_now"
        )));
        assert!(
            events
                .iter()
                .any(|event| matches!(event, RunEvent::RunFinished { .. }))
        );
    }

    #[tokio::test]
    async fn missing_skill_returns_an_error() {
        let runtime = runtime_with_provider(Arc::new(MockProvider));

        let err = runtime
            .run(RunRequest {
                system: None,
                input: "Rust async enables concurrency.".to_owned(),
                skill: Some("missing".to_owned()),
            })
            .await
            .expect_err("missing skill should fail");

        assert!(err.to_string().contains("skill not found"));
    }

    #[tokio::test]
    async fn empty_provider_response_returns_an_error() {
        let runtime = runtime_with_provider(Arc::new(EmptyProvider));

        let err = runtime
            .run(RunRequest {
                system: None,
                input: "Explain Mosaic.".to_owned(),
                skill: None,
            })
            .await
            .expect_err("empty provider response should fail");

        assert!(
            err.to_string()
                .contains("runtime stopped without final assistant message")
        );
    }

    #[tokio::test]
    async fn provider_failures_are_propagated() {
        let runtime = runtime_with_provider(Arc::new(FailingProvider));

        let err = runtime
            .run(RunRequest {
                system: None,
                input: "Explain Mosaic.".to_owned(),
                skill: None,
            })
            .await
            .expect_err("provider error should fail");

        assert!(err.to_string().contains("provider failure"));
    }
}
