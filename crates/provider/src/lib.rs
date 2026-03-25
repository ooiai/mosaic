use std::{collections::BTreeMap, env, sync::Arc};

use anyhow::{Result, anyhow, bail};
use async_trait::async_trait;
use mosaic_config::{MosaicConfig, ProviderProfileConfig};
use mosaic_tool_core::ToolMetadata;
use reqwest::Client;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum Role {
    System,
    User,
    Assistant,
    Tool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Message {
    pub role: Role,
    pub content: String,
    pub tool_call_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CompletionResponse {
    pub message: Option<Message>,
    #[serde(default)]
    pub tool_calls: Vec<ToolCall>,
    pub finish_reason: Option<String>,
}

#[async_trait]
pub trait LlmProvider: Send + Sync {
    async fn complete(
        &self,
        messages: &[Message],
        tools: Option<&[ToolDefinition]>,
    ) -> Result<CompletionResponse>;
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ModelCapabilities {
    pub supports_tools: bool,
    pub supports_sessions: bool,
    pub family: String,
    pub context_window_chars: usize,
    pub budget_tier: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProviderProfile {
    pub name: String,
    pub provider_type: String,
    pub model: String,
    pub base_url: Option<String>,
    pub api_key_env: Option<String>,
    pub capabilities: ModelCapabilities,
}

impl ProviderProfile {
    pub fn api_key_present(&self) -> bool {
        self.api_key_env
            .as_deref()
            .is_some_and(|env_var| env::var(env_var).is_ok())
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SchedulingIntent {
    InteractiveRun,
    Summary,
    WorkflowStep,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SchedulingRequest {
    pub requested_profile: Option<String>,
    pub intent: SchedulingIntent,
    pub estimated_context_chars: usize,
    pub requires_tools: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ScheduledProfile {
    pub profile: ProviderProfile,
    pub reason: String,
}

pub fn validate_step_tools_support(profile: &ProviderProfile, tool_names: &[String]) -> Result<()> {
    if !tool_names.is_empty() && !profile.capabilities.supports_tools {
        bail!(
            "profile '{}' does not support tool-enabled workflow steps",
            profile.name
        );
    }

    Ok(())
}

pub fn public_error_message(error: &anyhow::Error) -> String {
    redact_provider_message(&error.to_string())
}

pub fn tool_is_visible_to_model(metadata: &ToolMetadata) -> bool {
    metadata.capability.authorized && metadata.capability.healthy
}

pub fn tool_definition_from_metadata(metadata: &ToolMetadata) -> ToolDefinition {
    ToolDefinition {
        name: metadata.name.clone(),
        description: metadata.description.clone(),
        input_schema: metadata.input_schema.clone(),
    }
}

fn redact_provider_message(message: &str) -> String {
    let lower = message.to_ascii_lowercase();
    if let Some(index) = lower.find("bearer ") {
        let prefix = &message[..index + "Bearer ".len()];
        let rest = &message[index + "Bearer ".len()..];
        let suffix = rest
            .find(char::is_whitespace)
            .map(|offset| &rest[offset..])
            .unwrap_or("");
        return format!("{}<redacted>{}", prefix, suffix);
    }

    message.to_owned()
}

#[derive(Debug, Clone)]
pub struct ProviderProfileRegistry {
    active_profile: String,
    profiles: BTreeMap<String, ProviderProfile>,
}

impl ProviderProfileRegistry {
    pub fn from_config(config: &MosaicConfig) -> Result<Self> {
        if config.profiles.is_empty() {
            bail!("no provider profiles configured");
        }

        let profiles = config
            .profiles
            .iter()
            .map(|(name, profile)| (name.clone(), provider_profile_from_config(name, profile)))
            .collect::<BTreeMap<_, _>>();

        if !profiles.contains_key(&config.active_profile) {
            bail!(
                "active profile '{}' does not exist in the provider registry",
                config.active_profile
            );
        }

        Ok(Self {
            active_profile: config.active_profile.clone(),
            profiles,
        })
    }

    pub fn active_profile_name(&self) -> &str {
        &self.active_profile
    }

    pub fn active_profile(&self) -> &ProviderProfile {
        self.profiles
            .get(&self.active_profile)
            .expect("active profile should exist in registry")
    }

    pub fn list(&self) -> Vec<&ProviderProfile> {
        self.profiles.values().collect()
    }

    pub fn get(&self, name: &str) -> Option<&ProviderProfile> {
        self.profiles.get(name)
    }

    pub fn resolve(&self, name: Option<&str>) -> Result<ProviderProfile> {
        let name = name.unwrap_or(&self.active_profile);
        self.profiles
            .get(name)
            .cloned()
            .ok_or_else(|| anyhow!("unknown provider profile: {name}"))
    }

    pub fn build_provider(&self, name: Option<&str>) -> Result<Arc<dyn LlmProvider>> {
        let profile = self.resolve(name)?;
        build_provider_from_profile(&profile)
    }

    pub fn schedule(&self, request: SchedulingRequest) -> Result<ScheduledProfile> {
        if let Some(requested_profile) = request.requested_profile.as_deref() {
            let profile = self.resolve(Some(requested_profile))?;
            if request.requires_tools && !profile.capabilities.supports_tools {
                bail!(
                    "profile '{}' does not support tool-enabled runs",
                    profile.name
                );
            }
            return Ok(ScheduledProfile {
                profile,
                reason: "requested_profile".to_owned(),
            });
        }

        let mut candidates = self
            .list()
            .into_iter()
            .filter(|profile| !request.requires_tools || profile.capabilities.supports_tools)
            .cloned()
            .collect::<Vec<_>>();

        if candidates.is_empty() {
            bail!("no provider profiles satisfy the requested runtime constraints");
        }

        let active = self.active_profile().clone();
        let active_available = candidates.iter().any(|profile| profile.name == active.name);
        let long_context = request.estimated_context_chars
            > active.capabilities.context_window_chars.saturating_mul(3) / 4;

        let selected = match request.intent {
            SchedulingIntent::Summary => candidates
                .into_iter()
                .min_by_key(|profile| {
                    (
                        budget_rank(&profile.capabilities.budget_tier),
                        profile.capabilities.context_window_chars,
                    )
                })
                .expect("summary scheduling should have candidates"),
            SchedulingIntent::WorkflowStep => {
                if long_context {
                    candidates
                        .into_iter()
                        .max_by_key(|profile| profile.capabilities.context_window_chars)
                        .expect("workflow scheduling should have candidates")
                } else if active_available {
                    active.clone()
                } else {
                    candidates.remove(0)
                }
            }
            SchedulingIntent::InteractiveRun => {
                if long_context {
                    candidates
                        .into_iter()
                        .max_by_key(|profile| profile.capabilities.context_window_chars)
                        .expect("interactive scheduling should have candidates")
                } else if active_available {
                    active.clone()
                } else {
                    candidates.remove(0)
                }
            }
        };

        let reason = if selected.name == active.name {
            "active_profile"
        } else if long_context {
            "expanded_context_window"
        } else if matches!(request.intent, SchedulingIntent::Summary) {
            "summary_budget_policy"
        } else {
            "capability_policy"
        };

        Ok(ScheduledProfile {
            profile: selected,
            reason: reason.to_owned(),
        })
    }
}

pub struct MockProvider;

#[async_trait]
impl LlmProvider for MockProvider {
    async fn complete(
        &self,
        messages: &[Message],
        tools: Option<&[ToolDefinition]>,
    ) -> Result<CompletionResponse> {
        let last = messages.last().ok_or_else(|| anyhow!("no messages"))?;

        match last.role {
            Role::User => {
                let content = last.content.to_lowercase();
                let time_tool = resolve_tool_name(tools, "time_now");
                let read_file_tool = resolve_tool_name(tools, "read_file");

                if content.contains("time") {
                    if let Some(tool_name) = time_tool {
                        return Ok(CompletionResponse {
                            message: None,
                            tool_calls: vec![ToolCall {
                                id: format!("call_mock_{}", tool_name.replace('.', "_")),
                                name: tool_name,
                                arguments: serde_json::json!({}),
                            }],
                            finish_reason: Some("tool_calls".to_owned()),
                        });
                    }
                }

                if content.contains("read") || content.contains("file") {
                    if let Some(tool_name) = read_file_tool {
                        return Ok(CompletionResponse {
                            message: None,
                            tool_calls: vec![ToolCall {
                                id: format!("call_mock_{}", tool_name.replace('.', "_")),
                                name: tool_name,
                                arguments: serde_json::json!({
                                    "path": "README.md"
                                }),
                            }],
                            finish_reason: Some("tool_calls".to_owned()),
                        });
                    }
                }

                Ok(CompletionResponse {
                    message: Some(Message {
                        role: Role::Assistant,
                        content: format!("mock response: {}", last.content),
                        tool_call_id: None,
                    }),
                    tool_calls: vec![],
                    finish_reason: Some("stop".to_owned()),
                })
            }
            Role::Tool => {
                let reply = match infer_tool_name_from_call_id(last.tool_call_id.as_deref()) {
                    Some("time_now") => format!("The current time is: {}", last.content),
                    Some("read_file") => {
                        let preview = preview_text(&last.content, 220);
                        format!("I read the file successfully. Preview:\n{}", preview)
                    }
                    _ => format!("Tool returned:\n{}", last.content),
                };

                Ok(CompletionResponse {
                    message: Some(Message {
                        role: Role::Assistant,
                        content: reply,
                        tool_call_id: None,
                    }),
                    tool_calls: vec![],
                    finish_reason: Some("stop".to_owned()),
                })
            }
            _ => Ok(CompletionResponse {
                message: Some(Message {
                    role: Role::Assistant,
                    content: "mock response".to_owned(),
                    tool_call_id: None,
                }),
                tool_calls: vec![],
                finish_reason: Some("stop".to_owned()),
            }),
        }
    }
}

pub struct OpenAiCompatibleProvider {
    client: Client,
    base_url: String,
    api_key: String,
    model: String,
}

pub fn build_provider_from_profile(profile: &ProviderProfile) -> Result<Arc<dyn LlmProvider>> {
    match profile.provider_type.as_str() {
        "mock" => Ok(Arc::new(MockProvider)),
        "openai-compatible" => {
            let api_key_env = profile
                .api_key_env
                .as_deref()
                .ok_or_else(|| anyhow!("profile '{}' is missing api_key_env", profile.name))?;
            let api_key = env::var(api_key_env).map_err(|_| {
                anyhow!(
                    "profile '{}' expects environment variable {} to be set",
                    profile.name,
                    api_key_env
                )
            })?;

            Ok(Arc::new(OpenAiCompatibleProvider::new(
                profile
                    .base_url
                    .clone()
                    .unwrap_or_else(|| "https://api.openai.com/v1".to_owned()),
                api_key,
                profile.model.clone(),
            )))
        }
        other => bail!("unsupported provider type: {other}"),
    }
}

impl OpenAiCompatibleProvider {
    pub fn new(base_url: String, api_key: String, model: String) -> Self {
        Self {
            client: Client::new(),
            base_url,
            api_key,
            model,
        }
    }
}

#[async_trait]
impl LlmProvider for OpenAiCompatibleProvider {
    async fn complete(
        &self,
        messages: &[Message],
        tools: Option<&[ToolDefinition]>,
    ) -> Result<CompletionResponse> {
        let url = format!("{}/chat/completions", self.base_url.trim_end_matches('/'));

        let req_messages: Vec<serde_json::Value> = messages
            .iter()
            .map(|message| match message.role {
                Role::Tool => serde_json::json!({
                    "role": role_to_api(&message.role),
                    "content": message.content,
                    "tool_call_id": message.tool_call_id,
                }),
                _ => serde_json::json!({
                    "role": role_to_api(&message.role),
                    "content": message.content,
                }),
            })
            .collect();

        let req_tools = tools.map(|defs| {
            defs.iter()
                .map(|tool| {
                    serde_json::json!({
                        "type": "function",
                        "function": {
                            "name": tool.name,
                            "description": tool.description,
                            "parameters": tool.input_schema,
                        }
                    })
                })
                .collect::<Vec<_>>()
        });

        let body = if let Some(req_tools) = req_tools.filter(|defs| !defs.is_empty()) {
            serde_json::json!({
                "model": self.model,
                "messages": req_messages,
                "tools": req_tools,
                "tool_choice": "auto",
            })
        } else {
            serde_json::json!({
                "model": self.model,
                "messages": req_messages,
            })
        };

        let response: ApiChatCompletionResponse = self
            .client
            .post(url)
            .bearer_auth(&self.api_key)
            .json(&body)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;

        let choice = response
            .choices
            .into_iter()
            .next()
            .ok_or_else(|| anyhow!("empty choices in provider response"))?;

        let tool_calls = choice
            .message
            .tool_calls
            .unwrap_or_default()
            .into_iter()
            .map(|tool_call| {
                let arguments =
                    serde_json::from_str::<serde_json::Value>(&tool_call.function.arguments)
                        .unwrap_or_else(|_| serde_json::json!({}));

                ToolCall {
                    id: tool_call.id,
                    name: tool_call.function.name,
                    arguments,
                }
            })
            .collect::<Vec<_>>();

        let message = if tool_calls.is_empty() {
            Some(Message {
                role: Role::Assistant,
                content: choice.message.content.unwrap_or_default(),
                tool_call_id: None,
            })
        } else {
            None
        };

        Ok(CompletionResponse {
            message,
            tool_calls,
            finish_reason: choice.finish_reason,
        })
    }
}

fn role_to_api(role: &Role) -> &'static str {
    match role {
        Role::System => "system",
        Role::User => "user",
        Role::Assistant => "assistant",
        Role::Tool => "tool",
    }
}

fn resolve_tool_name(tools: Option<&[ToolDefinition]>, canonical: &str) -> Option<String> {
    let defs = tools?;

    defs.iter()
        .find(|tool| tool.name == canonical)
        .map(|tool| tool.name.clone())
        .or_else(|| {
            defs.iter()
                .find(|tool| tool.name.ends_with(&format!(".{canonical}")))
                .map(|tool| tool.name.clone())
        })
}

fn infer_tool_name_from_call_id(call_id: Option<&str>) -> Option<&'static str> {
    match call_id {
        Some(id) if id.contains("time_now") => Some("time_now"),
        Some(id) if id.contains("read_file") => Some("read_file"),
        _ => None,
    }
}

fn preview_text(value: &str, limit: usize) -> String {
    if value.chars().count() <= limit {
        return value.to_string();
    }

    let truncated: String = value.chars().take(limit).collect();
    format!("{truncated}...")
}

fn provider_profile_from_config(name: &str, config: &ProviderProfileConfig) -> ProviderProfile {
    ProviderProfile {
        name: name.to_owned(),
        provider_type: config.provider_type.clone(),
        model: config.model.clone(),
        base_url: config.base_url.clone(),
        api_key_env: config.api_key_env.clone(),
        capabilities: infer_model_capabilities(&config.provider_type, &config.model),
    }
}

fn infer_model_capabilities(provider_type: &str, model: &str) -> ModelCapabilities {
    let family = if model == "mock" {
        "mock".to_owned()
    } else if model.starts_with("gpt-5.4") {
        "gpt-5.4".to_owned()
    } else if model.starts_with("gpt-") {
        model.split('-').take(2).collect::<Vec<_>>().join("-")
    } else {
        provider_type.to_owned()
    };

    let (context_window_chars, budget_tier) = infer_context_budget(model);

    ModelCapabilities {
        supports_tools: matches!(provider_type, "mock" | "openai-compatible"),
        supports_sessions: true,
        family,
        context_window_chars,
        budget_tier: budget_tier.to_owned(),
    }
}

fn infer_context_budget(model: &str) -> (usize, &'static str) {
    if model == "mock" {
        (64_000, "medium")
    } else if model.contains("mini") {
        (32_000, "small")
    } else if model.starts_with("gpt-5.4") {
        (128_000, "large")
    } else if model.starts_with("gpt-") {
        (64_000, "medium")
    } else {
        (24_000, "small")
    }
}

fn budget_rank(value: &str) -> usize {
    match value {
        "small" => 0,
        "medium" => 1,
        "large" => 2,
        _ => 3,
    }
}

#[derive(Debug, Deserialize)]
struct ApiChatCompletionResponse {
    choices: Vec<ApiChoice>,
}

#[derive(Debug, Deserialize)]
struct ApiChoice {
    message: ApiMessage,
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ApiMessage {
    content: Option<String>,
    tool_calls: Option<Vec<ApiToolCall>>,
}

#[derive(Debug, Deserialize)]
struct ApiToolCall {
    id: String,
    function: ApiFunctionCall,
}

#[derive(Debug, Deserialize)]
struct ApiFunctionCall {
    name: String,
    arguments: String,
}

#[cfg(test)]
mod tests {
    use futures::executor::block_on;
    use mosaic_config::{MosaicConfig, ProviderProfileConfig};
    use mosaic_tool_core::{CapabilityKind, CapabilityMetadata, ToolMetadata};

    use super::{
        LlmProvider, Message, MockProvider, ProviderProfileRegistry, Role, SchedulingIntent,
        SchedulingRequest, ToolDefinition, public_error_message, tool_definition_from_metadata,
        tool_is_visible_to_model,
    };

    fn time_tool_definition() -> ToolDefinition {
        ToolDefinition {
            name: "time_now".to_owned(),
            description: "Return the current UTC timestamp".to_owned(),
            input_schema: serde_json::json!({ "type": "object", "properties": {} }),
        }
    }

    fn read_file_tool_definition() -> ToolDefinition {
        ToolDefinition {
            name: "read_file".to_owned(),
            description: "Read a UTF-8 text file from disk".to_owned(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": { "path": { "type": "string" } },
                "required": ["path"],
            }),
        }
    }

    fn mcp_read_file_tool_definition() -> ToolDefinition {
        ToolDefinition {
            name: "mcp.filesystem.read_file".to_owned(),
            description: "Read a UTF-8 text file from disk via MCP".to_owned(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": { "path": { "type": "string" } },
                "required": ["path"],
            }),
        }
    }

    #[test]
    fn tool_visibility_requires_authorized_and_healthy_capability() {
        let visible = ToolMetadata::builtin("echo", "Echo", serde_json::json!({}));
        let hidden = ToolMetadata::builtin("exec_command", "Exec", serde_json::json!({}))
            .with_capability(CapabilityMetadata {
                kind: CapabilityKind::Exec,
                authorized: false,
                ..CapabilityMetadata::exec()
            });

        assert!(tool_is_visible_to_model(&visible));
        assert!(!tool_is_visible_to_model(&hidden));
        assert_eq!(tool_definition_from_metadata(&visible).name, "echo");
    }

    #[test]
    fn mock_provider_replies_to_the_last_message_when_no_tool_is_needed() {
        let response = block_on(MockProvider.complete(
            &[
                Message {
                    role: Role::System,
                    content: "system".to_owned(),
                    tool_call_id: None,
                },
                Message {
                    role: Role::User,
                    content: "hello".to_owned(),
                    tool_call_id: None,
                },
            ],
            None,
        ))
        .expect("mock provider should succeed");

        assert_eq!(
            response.message.expect("message should exist").content,
            "mock response: hello"
        );
        assert_eq!(response.finish_reason.as_deref(), Some("stop"));
        assert!(response.tool_calls.is_empty());
    }

    #[test]
    fn mock_provider_emits_time_now_tool_call_when_tool_is_available() {
        let tools = vec![time_tool_definition()];

        let response = block_on(MockProvider.complete(
            &[Message {
                role: Role::User,
                content: "What time is it now?".to_owned(),
                tool_call_id: None,
            }],
            Some(&tools),
        ))
        .expect("mock provider should succeed");

        assert!(response.message.is_none());
        assert_eq!(response.finish_reason.as_deref(), Some("tool_calls"));
        assert_eq!(response.tool_calls.len(), 1);
        assert_eq!(response.tool_calls[0].name, "time_now");
    }

    #[test]
    fn mock_provider_emits_read_file_tool_call_when_tool_is_available() {
        let tools = vec![read_file_tool_definition()];

        let response = block_on(MockProvider.complete(
            &[Message {
                role: Role::User,
                content: "Read a file for me.".to_owned(),
                tool_call_id: None,
            }],
            Some(&tools),
        ))
        .expect("mock provider should succeed");

        assert!(response.message.is_none());
        assert_eq!(response.tool_calls.len(), 1);
        assert_eq!(response.tool_calls[0].name, "read_file");
        assert_eq!(
            response.tool_calls[0].arguments,
            serde_json::json!({ "path": "README.md" })
        );
    }

    #[test]
    fn mock_provider_finalizes_after_tool_output() {
        let response = block_on(MockProvider.complete(
            &[Message {
                role: Role::Tool,
                content: "2026-03-20T12:00:00Z".to_owned(),
                tool_call_id: Some("call_mock_time_now".to_owned()),
            }],
            None,
        ))
        .expect("mock provider should succeed");

        assert_eq!(
            response.message.expect("message should exist").content,
            "The current time is: 2026-03-20T12:00:00Z"
        );
    }

    #[test]
    fn mock_provider_uses_file_preview_after_read_file_tool_output() {
        let response = block_on(MockProvider.complete(
            &[Message {
                role: Role::Tool,
                content: "abcdefghijklmnopqrstuvwxyz".repeat(12),
                tool_call_id: Some("call_mock_read_file".to_owned()),
            }],
            Some(&[read_file_tool_definition()]),
        ))
        .expect("mock provider should succeed");

        let message = response.message.expect("message should exist");
        assert!(
            message
                .content
                .starts_with("I read the file successfully. Preview:\n")
        );
        assert!(message.content.ends_with("..."));
    }

    #[test]
    fn mock_provider_can_target_remote_mcp_tools_by_suffix() {
        let response = block_on(MockProvider.complete(
            &[Message {
                role: Role::User,
                content: "Read a file for me.".to_owned(),
                tool_call_id: None,
            }],
            Some(&[mcp_read_file_tool_definition()]),
        ))
        .expect("mock provider should succeed");

        assert!(response.message.is_none());
        assert_eq!(response.tool_calls.len(), 1);
        assert_eq!(response.tool_calls[0].name, "mcp.filesystem.read_file");
    }

    #[test]
    fn mock_provider_falls_back_when_requested_tool_is_unavailable() {
        let response = block_on(MockProvider.complete(
            &[Message {
                role: Role::User,
                content: "What time is it?".to_owned(),
                tool_call_id: None,
            }],
            Some(&[read_file_tool_definition()]),
        ))
        .expect("mock provider should succeed");

        assert!(response.tool_calls.is_empty());
        assert_eq!(
            response.message.expect("message should exist").content,
            "mock response: What time is it?"
        );
    }

    fn registry_for_scheduling() -> ProviderProfileRegistry {
        let mut config = MosaicConfig::default();
        config.profiles.clear();
        config.active_profile = "mini".to_owned();
        config.profiles.insert(
            "mini".to_owned(),
            ProviderProfileConfig {
                provider_type: "mock".to_owned(),
                model: "gpt-5.4-mini".to_owned(),
                base_url: None,
                api_key_env: None,
            },
        );
        config.profiles.insert(
            "large".to_owned(),
            ProviderProfileConfig {
                provider_type: "mock".to_owned(),
                model: "gpt-5.4".to_owned(),
                base_url: None,
                api_key_env: None,
            },
        );
        ProviderProfileRegistry::from_config(&config).expect("registry should build")
    }

    #[test]
    fn summary_scheduling_prefers_lower_budget_profile() {
        let registry = registry_for_scheduling();

        let scheduled = registry
            .schedule(SchedulingRequest {
                requested_profile: None,
                intent: SchedulingIntent::Summary,
                estimated_context_chars: 2_000,
                requires_tools: false,
            })
            .expect("summary schedule should succeed");

        assert_eq!(scheduled.profile.name, "mini");
        assert_eq!(scheduled.profile.capabilities.budget_tier, "small");
    }

    #[test]
    fn interactive_scheduling_expands_to_larger_context_window() {
        let registry = registry_for_scheduling();

        let scheduled = registry
            .schedule(SchedulingRequest {
                requested_profile: None,
                intent: SchedulingIntent::InteractiveRun,
                estimated_context_chars: 40_000,
                requires_tools: false,
            })
            .expect("interactive schedule should succeed");

        assert_eq!(scheduled.profile.name, "large");
        assert_eq!(scheduled.reason, "expanded_context_window");
    }

    #[test]
    fn public_error_message_redacts_bearer_tokens() {
        let message = public_error_message(&anyhow::anyhow!(
            "upstream provider rejected Authorization: Bearer sk-test-secret with status 401"
        ));

        assert!(message.contains("Bearer <redacted>"));
        assert!(!message.contains("sk-test-secret"));
    }
}
