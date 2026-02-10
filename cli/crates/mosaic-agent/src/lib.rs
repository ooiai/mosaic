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
use mosaic_core::session::{EventKind, SessionStore};
use mosaic_tools::{RunCommandOutput, ToolContext, ToolExecutor};

#[derive(Debug, Clone)]
pub struct AgentRunOptions {
    pub session_id: Option<String>,
    pub cwd: PathBuf,
    pub yes: bool,
    pub interactive: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentRunResult {
    pub session_id: String,
    pub response: String,
    pub turns: u32,
}

pub struct AgentRunner {
    provider: Arc<dyn Provider>,
    profile: ProfileConfig,
    session_store: SessionStore,
    audit_store: AuditStore,
    tools: ToolExecutor,
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
        }
    }

    pub fn session_store(&self) -> &SessionStore {
        &self.session_store
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

        let user_event =
            SessionStore::build_event(&session_id, EventKind::User, json!({ "text": prompt }));
        self.session_store.append_event(&user_event)?;

        let mut turns = 0u32;
        loop {
            turns += 1;
            if turns > self.profile.agent.max_turns {
                return Err(MosaicError::Validation(format!(
                    "agent exceeded max_turns={}",
                    self.profile.agent.max_turns
                )));
            }

            let messages = self.build_messages_for_session(&session_id)?;
            let request = ChatRequest {
                model: self.profile.provider.model.clone(),
                temperature: self.profile.agent.temperature,
                messages,
            };
            let response = self.provider.chat(request).await?;
            if let Some(tool_call) = parse_tool_call(&response) {
                self.handle_tool_call(&session_id, tool_call, &options)?;
                continue;
            }

            let assistant_event = SessionStore::build_event(
                &session_id,
                EventKind::Assistant,
                json!({ "text": response.content }),
            );
            self.session_store.append_event(&assistant_event)?;
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
            content: SYSTEM_PROMPT.to_string(),
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
        let tool_call_event = SessionStore::build_event(
            session_id,
            EventKind::ToolCall,
            json!({ "name": tool_call.name, "args": tool_call.args }),
        );
        self.session_store.append_event(&tool_call_event)?;
        let tool_context = ToolContext {
            cwd: options.cwd.clone(),
            yes: options.yes,
            interactive: options.interactive,
        };
        let result = self
            .tools
            .execute(&tool_call.name, tool_call.args.clone(), &tool_context)?;

        if tool_call.name == "run_cmd" {
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

        let tool_result_event = SessionStore::build_event(
            session_id,
            EventKind::ToolResult,
            json!({ "name": tool_call.name, "result": result }),
        );
        self.session_store.append_event(&tool_result_event)?;
        Ok(())
    }
}

const SYSTEM_PROMPT: &str = r#"
You are Mosaic CLI agent.
When you need a local tool, respond with EXACT JSON:
{"tool_call":{"name":"read_file|write_file|search_text|run_cmd","args":{...}}}
If no tool is needed, answer directly with plain text.
"#;

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
        let tools = ToolExecutor::new(profile.tools.run.guard_mode.clone());
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
                    cwd: temp.path().to_path_buf(),
                    yes: false,
                    interactive: false,
                },
            )
            .await
            .unwrap_err();
        assert!(
            err.to_string().contains("requires confirmation"),
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
                    cwd: temp.path().to_path_buf(),
                    yes: true,
                    interactive: false,
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
}
