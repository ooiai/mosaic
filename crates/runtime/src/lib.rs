pub mod events;

use std::sync::Arc;

use anyhow::anyhow;
use chrono::Utc;
use mosaic_inspect::{EffectiveProfileTrace, RunTrace, SkillTrace, ToolTrace};
use mosaic_provider::{
    LlmProvider, Message, ProviderProfile, ProviderProfileRegistry, Role, ToolDefinition,
};
use mosaic_session_core::{
    SessionRecord, SessionStore, TranscriptRole, session_title_from_input,
};
use mosaic_skill_core::{SkillContext, SkillRegistry};
use mosaic_tool_core::ToolRegistry;

use crate::events::{RunEvent, SharedRunEventSink};

pub struct RuntimeContext {
    pub profiles: Arc<ProviderProfileRegistry>,
    pub provider_override: Option<Arc<dyn LlmProvider>>,
    pub session_store: Arc<dyn SessionStore>,
    pub tools: Arc<ToolRegistry>,
    pub skills: Arc<SkillRegistry>,
    pub event_sink: SharedRunEventSink,
}

pub struct RunRequest {
    pub system: Option<String>,
    pub input: String,
    pub skill: Option<String>,
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

        let profile = match self.ctx.profiles.resolve(req.profile.as_deref()) {
            Ok(profile) => profile,
            Err(err) => return self.fail_run(trace, err),
        };
        trace.bind_effective_profile(Self::effective_profile_trace(&profile));

        let mut session = match self.load_session(&req, &profile, &mut trace) {
            Ok(session) => session,
            Err(err) => return self.fail_run(trace, err),
        };

        if let Some(skill_name) = req.skill.clone() {
            return self.run_skill(req, skill_name, profile, trace, session).await;
        }

        if let Some(session_ref) = session.as_mut() {
            if let Err(err) = self.append_session_message(session_ref, TranscriptRole::User, req.input.clone(), None) {
                return self.fail_run(trace, err);
            }
        }

        let mut messages = if let Some(session_ref) = session.as_ref() {
            Self::session_messages(session_ref)
        } else {
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
            messages
        };

        let provider = match self.ctx.provider_override.clone() {
            Some(provider) => provider,
            None => match self.ctx.profiles.build_provider(Some(&profile.name)) {
                Ok(provider) => provider,
                Err(err) => return self.fail_run(trace, err),
            },
        };
        let tool_defs = self.collect_tool_definitions();
        let provider_tools = (!tool_defs.is_empty()).then_some(tool_defs.as_slice());

        for _ in 0..8 {
            self.emit(RunEvent::ProviderRequest {
                tool_count: tool_defs.len(),
                message_count: messages.len(),
            });

            let response = match provider.complete(&messages, provider_tools).await {
                Ok(response) => response,
                Err(err) => return self.fail_run(trace, err),
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
                                name: tool_name,
                                call_id,
                                error: err.to_string(),
                            });
                            return self.fail_run(trace, err);
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

                            if let Some(session_ref) = session.as_mut() {
                                if let Err(err) = self.append_session_message(
                                    session_ref,
                                    TranscriptRole::Tool,
                                    result.content.clone(),
                                    Some(call_id.clone()),
                                ) {
                                    return self.fail_run(trace, err);
                                }
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
                            return self.fail_run(trace, err);
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

                if let Some(session_ref) = session.as_mut() {
                    if let Err(err) = self.append_session_message(
                        session_ref,
                        TranscriptRole::Assistant,
                        message.content.clone(),
                        None,
                    ) {
                        return self.fail_run(trace, err);
                    }
                }

                trace.finish_ok(message.content.clone());

                return Ok(RunResult {
                    output: message.content,
                    trace,
                });
            }

            break;
        }

        let err = anyhow!("runtime stopped without final assistant message");
        self.fail_run(trace, err)
    }

    async fn run_skill(
        &self,
        req: RunRequest,
        skill_name: String,
        _profile: ProviderProfile,
        mut trace: RunTrace,
        mut session: Option<SessionRecord>,
    ) -> std::result::Result<RunResult, RunError> {
        let skill = match self.ctx.skills.get(&skill_name) {
            Some(skill) => skill,
            None => return self.fail_run(trace, anyhow!("skill not found: {}", skill_name)),
        };

        self.emit(RunEvent::SkillStarted {
            name: skill_name.clone(),
        });

        if let Some(session_ref) = session.as_mut() {
            if let Err(err) = self.append_session_message(session_ref, TranscriptRole::User, req.input.clone(), None) {
                return self.fail_run(trace, err);
            }
        }

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

                if let Some(session_ref) = session.as_mut() {
                    if let Err(err) = self.append_session_message(
                        session_ref,
                        TranscriptRole::Assistant,
                        out.content.clone(),
                        None,
                    ) {
                        return self.fail_run(trace, err);
                    }
                }

                self.emit(RunEvent::SkillFinished {
                    name: skill_name,
                });
                self.emit(RunEvent::FinalAnswerReady);
                self.emit(RunEvent::RunFinished {
                    output_preview: Self::truncate_preview(&out.content, 120),
                });

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

                self.emit(RunEvent::SkillFailed {
                    name: skill_name,
                    error: err.to_string(),
                });
                self.fail_run(trace, err)
            }
        }
    }

    fn load_session(
        &self,
        req: &RunRequest,
        profile: &ProviderProfile,
        trace: &mut RunTrace,
    ) -> Result<Option<SessionRecord>, anyhow::Error> {
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
    ) -> Result<(), anyhow::Error> {
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
    use std::{collections::BTreeMap, sync::{Arc, Mutex}};

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
    use mosaic_tool_core::{TimeNowTool, ToolRegistry};

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

    fn runtime_with_provider(
        provider: Arc<dyn LlmProvider>,
        session_store: Arc<dyn SessionStore>,
        event_sink: Arc<dyn RunEventSink>,
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

        let profiles = ProviderProfileRegistry::from_config(&config)
            .expect("profile registry should build");
        let mut tools = ToolRegistry::new();
        tools.register(Arc::new(TimeNowTool::new()));

        AgentRuntime::new(RuntimeContext {
            profiles: Arc::new(profiles),
            provider_override: Some(provider),
            session_store,
            tools: Arc::new(tools),
            skills: Arc::new(SkillRegistry::new()),
            event_sink,
        })
    }

    fn event_names(events: &[RunEvent]) -> Vec<&'static str> {
        events
            .iter()
            .map(|event| match event {
                RunEvent::RunStarted { .. } => "RunStarted",
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
                session_id: None,
                profile: None,
            })
            .await
            .expect("runtime should succeed");

        assert_eq!(result.output, "mock response: Explain Mosaic.");
        assert_eq!(result.trace.effective_profile.as_ref().map(|profile| profile.profile.as_str()), Some("mock"));

        assert_eq!(
            event_names(&sink.snapshot()),
            vec!["RunStarted", "ProviderRequest", "FinalAnswerReady", "RunFinished"]
        );
    }

    #[tokio::test]
    async fn session_runs_roundtrip_transcript_messages() {
        let store = Arc::new(MemorySessionStore::default());
        let runtime = runtime_with_provider(Arc::new(MockProvider), store.clone(), Arc::new(NoopEventSink));

        runtime
            .run(RunRequest {
                system: Some("You are helpful.".to_owned()),
                input: "hello".to_owned(),
                skill: None,
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
        assert_eq!(session.last_run_id.as_deref().is_some(), true);
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
                session_id: Some("time-demo".to_owned()),
                profile: None,
            })
            .await
            .expect("tool loop should succeed");

        assert!(result.output.starts_with("The current time is: "));
        assert_eq!(result.trace.tool_calls.len(), 1);
        assert_eq!(result.trace.session_id.as_deref(), Some("time-demo"));
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
                session_id: None,
                profile: None,
            })
            .await
            .expect_err("empty provider response should fail");

        assert!(err
            .to_string()
            .contains("runtime stopped without final assistant message"));
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

        let profiles = ProviderProfileRegistry::from_config(&config)
            .expect("profile registry should build");
        let mut skills = SkillRegistry::new();
        skills.register(Arc::new(FailingSkill));

        let runtime = AgentRuntime::new(RuntimeContext {
            profiles: Arc::new(profiles),
            provider_override: Some(Arc::new(MockProvider)),
            session_store: Arc::new(MemorySessionStore::default()),
            tools: Arc::new(ToolRegistry::new()),
            skills: Arc::new(skills),
            event_sink: sink.clone(),
        });

        let err = runtime
            .run(RunRequest {
                system: None,
                input: "boom".to_owned(),
                skill: Some("explode".to_owned()),
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
        let profiles = ProviderProfileRegistry::from_config(&config)
            .expect("profile registry should build");
        let store = Arc::new(MemorySessionStore::default());
        let mut skills = SkillRegistry::new();
        skills.register(Arc::new(SummarizeSkill));

        let runtime = AgentRuntime::new(RuntimeContext {
            profiles: Arc::new(profiles),
            provider_override: Some(Arc::new(MockProvider)),
            session_store: store.clone(),
            tools: Arc::new(ToolRegistry::new()),
            skills: Arc::new(skills),
            event_sink: Arc::new(NoopEventSink),
        });

        let result = runtime
            .run(RunRequest {
                system: None,
                input: "Rust async enables concurrency.".to_owned(),
                skill: Some("summarize".to_owned()),
                session_id: Some("summary-demo".to_owned()),
                profile: None,
            })
            .await
            .expect("skill run should succeed");

        let session = store.get("summary-demo").expect("session should exist");
        assert_eq!(result.trace.session_id.as_deref(), Some("summary-demo"));
        assert!(session
            .transcript
            .iter()
            .any(|message: &TranscriptMessage| message.content.contains("summary:")));
    }
}
