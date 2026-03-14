use std::path::PathBuf;
use std::sync::Arc;

use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use uuid::Uuid;

use mosaic_core::audit::{AuditStore, CommandAudit};
use mosaic_core::config::ProfileConfig;
use mosaic_core::error::{MosaicError, Result};
use mosaic_core::provider::{ChatMessage, ChatRequest, ChatResponse, ChatRole, Provider};
use mosaic_core::session::{EventKind, SessionRuntimeMetadata, SessionStore};
use mosaic_tools::{RunCommandOutput, ToolContext, ToolExecutor};

pub type AgentEventCallback = Arc<dyn Fn(AgentEvent) + Send + Sync>;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AgentEvent {
    User {
        session_id: String,
        text: String,
    },
    Assistant {
        session_id: String,
        text: String,
    },
    ToolCall {
        session_id: String,
        name: String,
        args: Value,
    },
    ToolResult {
        session_id: String,
        name: String,
        result: Value,
    },
    Error {
        session_id: String,
        message: String,
    },
}

#[derive(Clone)]
pub struct AgentRunOptions {
    pub session_id: Option<String>,
    pub session_metadata: SessionRuntimeMetadata,
    pub cwd: PathBuf,
    pub yes: bool,
    pub interactive: bool,
    pub event_callback: Option<AgentEventCallback>,
}

impl std::fmt::Debug for AgentRunOptions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AgentRunOptions")
            .field("session_id", &self.session_id)
            .field("session_metadata", &self.session_metadata)
            .field("cwd", &self.cwd)
            .field("yes", &self.yes)
            .field("interactive", &self.interactive)
            .field("event_callback", &self.event_callback.is_some())
            .finish()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentRunResult {
    pub session_id: String,
    pub response: String,
    pub turns: u32,
}

#[derive(Clone)]
pub struct AgentRunner {
    provider: Arc<dyn Provider>,
    profile: ProfileConfig,
    session_store: SessionStore,
    audit_store: AuditStore,
    tools: ToolExecutor,
    system_prompt: String,
}

impl AgentRunner {
    pub fn new(
        provider: Arc<dyn Provider>,
        profile: ProfileConfig,
        session_store: SessionStore,
        audit_store: AuditStore,
        tools: ToolExecutor,
    ) -> Self {
        Self {
            provider,
            profile,
            session_store,
            audit_store,
            tools,
            system_prompt: SYSTEM_PROMPT.to_string(),
        }
    }

    pub fn with_system_prompt(
        provider: Arc<dyn Provider>,
        profile: ProfileConfig,
        session_store: SessionStore,
        audit_store: AuditStore,
        tools: ToolExecutor,
        system_prompt: String,
    ) -> Self {
        let prompt = if system_prompt.trim().is_empty() {
            SYSTEM_PROMPT.to_string()
        } else {
            system_prompt
        };
        Self {
            provider,
            profile,
            session_store,
            audit_store,
            tools,
            system_prompt: prompt,
        }
    }

    pub fn session_store(&self) -> &SessionStore {
        &self.session_store
    }

    pub fn profile(&self) -> &ProfileConfig {
        &self.profile
    }

    pub async fn ask(&self, prompt: &str, options: AgentRunOptions) -> Result<AgentRunResult> {
        if prompt.trim().is_empty() {
            return Err(MosaicError::Validation(
                "prompt cannot be empty".to_string(),
            ));
        }

        self.session_store.ensure_dirs()?;
        self.audit_store.ensure_dirs()?;

        let session_id = match options.session_id.clone() {
            Some(session_id) => {
                let path = self.session_store.session_path(&session_id);
                if !path.exists() {
                    return Err(MosaicError::Config(format!(
                        "session '{session_id}' was not found"
                    )));
                }
                session_id
            }
            None => self.session_store.create_session_id(),
        };
        let emit_error = |message: String| {
            self.emit_event(
                &options,
                AgentEvent::Error {
                    session_id: session_id.clone(),
                    message,
                },
            );
        };

        self.ensure_runtime_metadata(&session_id, &options.session_metadata)
            .inspect_err(|err| {
                emit_error(err.to_string());
            })?;

        let user_event =
            SessionStore::build_event(&session_id, EventKind::User, json!({ "text": prompt }));
        self.session_store
            .append_event(&user_event)
            .inspect_err(|err| {
                emit_error(err.to_string());
            })?;
        self.emit_event(
            &options,
            AgentEvent::User {
                session_id: session_id.clone(),
                text: prompt.to_string(),
            },
        );

        let mut turns = 0u32;
        loop {
            turns += 1;
            if turns > self.profile.agent.max_turns {
                let err = MosaicError::Validation(format!(
                    "agent exceeded max_turns={}",
                    self.profile.agent.max_turns
                ));
                emit_error(err.to_string());
                return Err(err);
            }

            let messages = self
                .build_messages_for_session(&session_id)
                .inspect_err(|err| {
                    emit_error(err.to_string());
                })?;
            let request = ChatRequest {
                model: self.profile.provider.model.clone(),
                temperature: self.profile.agent.temperature,
                messages,
            };
            let response = self.provider.chat(request).await.inspect_err(|err| {
                emit_error(err.to_string());
            })?;
            if let Some(tool_call) = parse_tool_call(&response) {
                self.handle_tool_call(&session_id, tool_call, &options)
                    .inspect_err(|err| {
                        emit_error(err.to_string());
                    })?;
                continue;
            }

            let assistant_event = SessionStore::build_event(
                &session_id,
                EventKind::Assistant,
                json!({ "text": response.content }),
            );
            self.session_store
                .append_event(&assistant_event)
                .inspect_err(|err| {
                    emit_error(err.to_string());
                })?;
            self.emit_event(
                &options,
                AgentEvent::Assistant {
                    session_id: session_id.clone(),
                    text: response.content.clone(),
                },
            );
            return Ok(AgentRunResult {
                session_id,
                response: response.content,
                turns,
            });
        }
    }

    fn build_messages_for_session(&self, session_id: &str) -> Result<Vec<ChatMessage>> {
        let mut messages = vec![ChatMessage {
            role: ChatRole::System,
            content: self.system_prompt.clone(),
        }];
        let events = self.session_store.read_events(session_id)?;
        for event in events {
            match event.kind {
                EventKind::User => {
                    if let Some(text) = event.payload.get("text").and_then(|v| v.as_str()) {
                        messages.push(ChatMessage {
                            role: ChatRole::User,
                            content: text.to_string(),
                        });
                    }
                }
                EventKind::Assistant => {
                    if let Some(text) = event.payload.get("text").and_then(|v| v.as_str()) {
                        messages.push(ChatMessage {
                            role: ChatRole::Assistant,
                            content: text.to_string(),
                        });
                    }
                }
                EventKind::ToolResult => {
                    let text = event
                        .payload
                        .get("result")
                        .map_or_else(|| "{}".to_string(), |value| value.to_string());
                    let name = event
                        .payload
                        .get("name")
                        .and_then(|value| value.as_str())
                        .unwrap_or("unknown_tool");
                    messages.push(ChatMessage {
                        role: ChatRole::User,
                        content: format!("TOOL_RESULT {name}\n{text}"),
                    });
                }
                EventKind::ToolCall | EventKind::System | EventKind::Error => {}
            }
        }
        Ok(messages)
    }

    fn ensure_runtime_metadata(
        &self,
        session_id: &str,
        metadata: &SessionRuntimeMetadata,
    ) -> Result<()> {
        let latest = self.session_store.latest_runtime_metadata(session_id)?;
        if latest.as_ref() == Some(metadata) {
            return Ok(());
        }
        let event = SessionStore::build_runtime_metadata_event(session_id, metadata);
        self.session_store.append_event(&event)
    }

    fn handle_tool_call(
        &self,
        session_id: &str,
        tool_call: ParsedToolCall,
        options: &AgentRunOptions,
    ) -> Result<()> {
        if !self.profile.tools.enabled {
            return Err(MosaicError::Tool(
                "tools are disabled in the current profile".to_string(),
            ));
        }
        let tool_name = tool_call.name;
        let tool_args = tool_call.args;

        let tool_call_event = SessionStore::build_event(
            session_id,
            EventKind::ToolCall,
            json!({ "name": tool_name, "args": tool_args }),
        );
        self.session_store.append_event(&tool_call_event)?;
        self.emit_event(
            options,
            AgentEvent::ToolCall {
                session_id: session_id.to_string(),
                name: tool_name.clone(),
                args: tool_args.clone(),
            },
        );
        let tool_context = ToolContext {
            cwd: options.cwd.clone(),
            yes: options.yes,
            interactive: options.interactive,
        };
        let result = self.tools.execute(&tool_name, tool_args, &tool_context)?;

        if tool_name == "run_cmd" {
            let parsed: RunCommandOutput = serde_json::from_value(result.clone())?;
            self.audit_store.append_command(&CommandAudit {
                id: Uuid::new_v4().to_string(),
                ts: Utc::now(),
                session_id: session_id.to_string(),
                command: parsed.command,
                cwd: parsed.cwd,
                approved_by: parsed.approved_by,
                exit_code: parsed.exit_code,
                duration_ms: parsed.duration_ms,
            })?;
        }

        let result_payload = result.clone();
        let tool_result_event = SessionStore::build_event(
            session_id,
            EventKind::ToolResult,
            json!({ "name": tool_name, "result": result }),
        );
        self.session_store.append_event(&tool_result_event)?;
        self.emit_event(
            options,
            AgentEvent::ToolResult {
                session_id: session_id.to_string(),
                name: tool_name,
                result: result_payload,
            },
        );
        Ok(())
    }

    fn emit_event(&self, options: &AgentRunOptions, event: AgentEvent) {
        if let Some(callback) = &options.event_callback {
            callback(event);
        }
    }
}

const SYSTEM_PROMPT: &str = r#"
You are Mosaic CLI agent.
When you need a local tool, respond with EXACT JSON only:
{"tool_call":{"name":"read_file","args":{"path":"README.md"}}}
Tool argument schemas:
- read_file: {"path":"relative/or/absolute/path"}
- write_file: {"path":"relative/or/absolute/path","content":"full file contents"}
- search_text: {"query":"text or regex","path":"optional/path","max_results":50}
- run_cmd: {"command":"shell command to execute"}
Prefer read_file and search_text for repository inspection. Use run_cmd only when file tools are insufficient.
If no tool is needed, answer directly with plain text.
"#;

pub fn default_system_prompt() -> &'static str {
    SYSTEM_PROMPT
}

#[derive(Debug, Clone)]
struct ParsedToolCall {
    name: String,
    args: Value,
}

#[derive(Debug, Deserialize)]
struct ToolCallEnvelope {
    tool_call: ToolCallBody,
}

#[derive(Debug, Deserialize)]
struct ToolCallBody {
    name: String,
    args: Value,
}

fn parse_tool_call(response: &ChatResponse) -> Option<ParsedToolCall> {
    let content = strip_markdown_json_fence(response.content.trim());
    let envelope = serde_json::from_str::<ToolCallEnvelope>(content).ok()?;
    Some(ParsedToolCall {
        name: envelope.tool_call.name,
        args: envelope.tool_call.args,
    })
}

fn strip_markdown_json_fence(input: &str) -> &str {
    if input.starts_with("```json") && input.ends_with("```") {
        return input
            .trim_start_matches("```json")
            .trim_start_matches('\n')
            .trim_end_matches("```")
            .trim();
    }
    if input.starts_with("```") && input.ends_with("```") {
        return input
            .trim_start_matches("```")
            .trim_start_matches('\n')
            .trim_end_matches("```")
            .trim();
    }
    input
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;
    use std::sync::Mutex;

    use tempfile::tempdir;

    use super::*;
    use mosaic_core::config::{ProfileConfig, RunGuardMode};
    use mosaic_core::provider::{ModelInfo, ProviderHealth};

    #[test]
    fn parse_tool_call_from_plain_json() {
        let response = ChatResponse {
            content: r#"{"tool_call":{"name":"read_file","args":{"path":"README.md"}}}"#
                .to_string(),
        };
        let parsed = parse_tool_call(&response).expect("should parse tool call");
        assert_eq!(parsed.name, "read_file");
        assert_eq!(parsed.args["path"], "README.md");
    }

    #[test]
    fn parse_tool_call_from_fenced_json() {
        let response = ChatResponse {
            content: "```json\n{\"tool_call\":{\"name\":\"run_cmd\",\"args\":{\"command\":\"pwd\"}}}\n```"
                .to_string(),
        };
        let parsed = parse_tool_call(&response).expect("should parse tool call");
        assert_eq!(parsed.name, "run_cmd");
    }

    struct MockProvider {
        responses: Mutex<VecDeque<String>>,
    }

    impl MockProvider {
        fn new(responses: Vec<String>) -> Self {
            Self {
                responses: Mutex::new(VecDeque::from(responses)),
            }
        }
    }

    #[async_trait::async_trait]
    impl Provider for MockProvider {
        async fn list_models(&self) -> Result<Vec<ModelInfo>> {
            Ok(vec![ModelInfo {
                id: "mock-model".to_string(),
                owned_by: Some("mock".to_string()),
            }])
        }

        async fn chat(&self, _request: ChatRequest) -> Result<ChatResponse> {
            let next = self
                .responses
                .lock()
                .expect("lock")
                .pop_front()
                .unwrap_or_else(|| "done".to_string());
            Ok(ChatResponse { content: next })
        }

        async fn health(&self) -> Result<ProviderHealth> {
            Ok(ProviderHealth {
                ok: true,
                latency_ms: Some(1),
                detail: "mock".to_string(),
            })
        }
    }

    fn build_runner(
        provider: Arc<dyn Provider>,
        temp: &tempfile::TempDir,
        guard_mode: RunGuardMode,
    ) -> AgentRunner {
        let mut profile = ProfileConfig::default();
        profile.provider.model = "mock-model".to_string();
        profile.tools.run.guard_mode = guard_mode;
        let store = SessionStore::new(temp.path().join("sessions"));
        let audit = AuditStore::new(
            temp.path().join("audit"),
            temp.path().join("audit/commands.jsonl"),
        );
        let tools = ToolExecutor::new(profile.tools.run.guard_mode.clone(), None);
        AgentRunner::new(provider, profile, store, audit, tools)
    }

    #[tokio::test]
    async fn run_cmd_requires_confirmation_without_yes() {
        let temp = tempdir().expect("tempdir");
        let provider: Arc<dyn Provider> = Arc::new(MockProvider::new(vec![
            r#"{"tool_call":{"name":"run_cmd","args":{"command":"touch guarded.txt"}}}"#
                .to_string(),
        ]));
        let runner = build_runner(provider, &temp, RunGuardMode::ConfirmDangerous);
        let err = runner
            .ask(
                "create a file",
                AgentRunOptions {
                    session_id: None,
                    session_metadata: SessionRuntimeMetadata {
                        agent_id: None,
                        profile_name: "default".to_string(),
                    },
                    cwd: temp.path().to_path_buf(),
                    yes: false,
                    interactive: false,
                    event_callback: None,
                },
            )
            .await
            .unwrap_err();
        assert!(
            err.to_string().contains("requires approval"),
            "unexpected error: {err}"
        );
    }

    #[tokio::test]
    async fn run_cmd_succeeds_with_yes_and_appends_audit() {
        let temp = tempdir().expect("tempdir");
        let provider: Arc<dyn Provider> = Arc::new(MockProvider::new(vec![
            r#"{"tool_call":{"name":"run_cmd","args":{"command":"touch allowed.txt"}}}"#
                .to_string(),
            "done".to_string(),
        ]));
        let runner = build_runner(provider, &temp, RunGuardMode::ConfirmDangerous);
        let result = runner
            .ask(
                "create a file",
                AgentRunOptions {
                    session_id: None,
                    session_metadata: SessionRuntimeMetadata {
                        agent_id: None,
                        profile_name: "default".to_string(),
                    },
                    cwd: temp.path().to_path_buf(),
                    yes: true,
                    interactive: false,
                    event_callback: None,
                },
            )
            .await
            .expect("ask should pass");
        assert_eq!(result.response, "done");
        assert!(temp.path().join("allowed.txt").exists());
        let audit = std::fs::read_to_string(temp.path().join("audit/commands.jsonl"))
            .expect("audit file should exist");
        assert!(audit.contains("touch allowed.txt"));
    }

    #[tokio::test]
    async fn run_cmd_accepts_cmd_alias_in_agent_flow() {
        let temp = tempdir().expect("tempdir");
        let provider: Arc<dyn Provider> = Arc::new(MockProvider::new(vec![
            r#"{"tool_call":{"name":"run_cmd","args":{"cmd":"touch alias.txt"}}}"#.to_string(),
            "done".to_string(),
        ]));
        let runner = build_runner(provider, &temp, RunGuardMode::ConfirmDangerous);
        let result = runner
            .ask(
                "create a file",
                AgentRunOptions {
                    session_id: None,
                    session_metadata: SessionRuntimeMetadata {
                        agent_id: None,
                        profile_name: "default".to_string(),
                    },
                    cwd: temp.path().to_path_buf(),
                    yes: true,
                    interactive: false,
                    event_callback: None,
                },
            )
            .await
            .expect("ask should pass");
        assert_eq!(result.response, "done");
        assert!(temp.path().join("alias.txt").exists());
    }
}
