use std::sync::Arc;

use anyhow::{Result, anyhow};
use chrono::Utc;
use mosaic_inspect::{RunTrace, SkillTrace};
use mosaic_provider::{LlmProvider, Message, Role};
use mosaic_skill_core::{SkillContext, SkillRegistry};
use mosaic_tool_core::ToolRegistry;

pub struct RuntimeContext {
    pub provider: Arc<dyn LlmProvider>,
    pub tools: Arc<ToolRegistry>,
    pub skills: Arc<SkillRegistry>,
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

    pub async fn run(&self, req: RunRequest) -> Result<RunResult> {
        let mut trace = RunTrace::new(req.input.clone());

        if let Some(skill_name) = req.skill.clone() {
            let skill = match self.ctx.skills.get(&skill_name) {
                Some(skill) => skill,
                None => {
                    let err = anyhow!("skill not found: {skill_name}");
                    trace.finish_err(err.to_string());
                    return Err(err);
                }
            };

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

            return match skill.execute(skill_input, &ctx).await {
                Ok(out) => {
                    if let Some(last) = trace.skill_calls.last_mut() {
                        last.output = Some(out.content.clone());
                        last.finished_at = Some(Utc::now());
                    }
                    trace.finish_ok(out.content.clone());
                    Ok(RunResult {
                        output: out.content,
                        trace,
                    })
                }
                Err(err) => {
                    if let Some(last) = trace.skill_calls.last_mut() {
                        last.finished_at = Some(Utc::now());
                    }
                    trace.finish_err(err.to_string());
                    Err(err)
                }
            };
        }

        let mut messages = Vec::new();

        if let Some(system) = req.system {
            messages.push(Message {
                role: Role::System,
                content: system,
            });
        }

        messages.push(Message {
            role: Role::User,
            content: req.input.clone(),
        });

        let resp = match self.ctx.provider.complete(&messages).await {
            Ok(resp) => resp,
            Err(err) => {
                trace.finish_err(err.to_string());
                return Err(err);
            }
        };

        if let Some(message) = resp.message {
            trace.finish_ok(message.content.clone());
            return Ok(RunResult {
                output: message.content,
                trace,
            });
        }

        let err = anyhow!("empty completion response");
        trace.finish_err(err.to_string());
        Err(err)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use anyhow::{Result, anyhow};
    use async_trait::async_trait;
    use mosaic_provider::{CompletionResponse, LlmProvider, Message, MockProvider};
    use mosaic_skill_core::{SkillRegistry, SummarizeSkill};
    use mosaic_tool_core::ToolRegistry;

    use super::{AgentRuntime, RunRequest, RuntimeContext};

    struct EmptyProvider;

    #[async_trait]
    impl LlmProvider for EmptyProvider {
        async fn complete(&self, _messages: &[Message]) -> Result<CompletionResponse> {
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
        async fn complete(&self, _messages: &[Message]) -> Result<CompletionResponse> {
            Err(anyhow!("provider failure"))
        }
    }

    fn runtime_with_provider(provider: Arc<dyn LlmProvider>) -> AgentRuntime {
        AgentRuntime::new(RuntimeContext {
            provider,
            tools: Arc::new(ToolRegistry::new()),
            skills: Arc::new(SkillRegistry::new()),
        })
    }

    #[tokio::test]
    async fn provider_only_run_returns_mock_output() {
        let runtime = runtime_with_provider(Arc::new(MockProvider));

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
    }

    #[tokio::test]
    async fn skill_dispatch_records_a_skill_trace() {
        let mut skills = SkillRegistry::new();
        skills.register(Arc::new(SummarizeSkill));

        let runtime = AgentRuntime::new(RuntimeContext {
            provider: Arc::new(MockProvider),
            tools: Arc::new(ToolRegistry::new()),
            skills: Arc::new(skills),
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

        assert!(err.to_string().contains("empty completion response"));
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
