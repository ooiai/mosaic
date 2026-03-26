use std::{collections::BTreeMap, env, future::Future, sync::Arc, time::Duration};

use anyhow::{Result, anyhow, bail};
use async_trait::async_trait;
use mosaic_config::{MosaicConfig, ProviderProfileConfig, ProviderType, parse_provider_type};
use mosaic_tool_core::ToolMetadata;
use reqwest::Client;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use tokio::time::sleep;
use tracing::warn;

const DEFAULT_MAX_RETRIES: u8 = 2;
const DEFAULT_TIMEOUT_MS: u64 = 45_000;
const ANTHROPIC_TIMEOUT_MS: u64 = 60_000;
const OLLAMA_TIMEOUT_MS: u64 = 90_000;
const TOOL_CALL_SHADOW_PREFIX: &str = "__mosaic_tool_calls__:";
const AZURE_CHAT_COMPLETIONS_API_VERSION: &str = "2024-10-21";
const ANTHROPIC_VERSION_HEADER: &str = "2023-06-01";

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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProviderTransportMetadata {
    pub provider_type: String,
    pub base_url: Option<String>,
    pub timeout_ms: u64,
    pub max_retries: u8,
    pub supports_tool_call_shadow_messages: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProviderAttempt {
    pub attempt: u8,
    pub max_attempts: u8,
    pub status: String,
    pub error_kind: Option<String>,
    pub status_code: Option<u16>,
    pub retryable: bool,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProviderCompletion {
    pub response: CompletionResponse,
    #[serde(default)]
    pub attempts: Vec<ProviderAttempt>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProviderErrorKind {
    Auth,
    Timeout,
    RateLimited,
    Unavailable,
    InvalidRequest,
    Transport,
    Response,
    Unsupported,
    Unknown,
}

impl ProviderErrorKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Auth => "auth",
            Self::Timeout => "timeout",
            Self::RateLimited => "rate_limited",
            Self::Unavailable => "unavailable",
            Self::InvalidRequest => "invalid_request",
            Self::Transport => "transport",
            Self::Response => "response",
            Self::Unsupported => "unsupported",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProviderError {
    pub kind: ProviderErrorKind,
    pub provider_type: String,
    pub profile_name: String,
    pub model: String,
    pub message: String,
    pub public_message: String,
    pub status_code: Option<u16>,
    pub retryable: bool,
    #[serde(default)]
    pub attempts: Vec<ProviderAttempt>,
}

impl ProviderError {
    fn new(
        kind: ProviderErrorKind,
        provider_type: impl Into<String>,
        profile_name: impl Into<String>,
        model: impl Into<String>,
        message: impl Into<String>,
        status_code: Option<u16>,
        retryable: bool,
    ) -> Self {
        let provider_type = provider_type.into();
        let profile_name = profile_name.into();
        let model = model.into();
        let message = redact_provider_message(&message.into());
        let mut public_message = match kind {
            ProviderErrorKind::Auth => format!(
                "{} provider authentication failed for profile '{}'",
                provider_type, profile_name
            ),
            ProviderErrorKind::Timeout => format!(
                "{} provider request timed out for profile '{}'",
                provider_type, profile_name
            ),
            ProviderErrorKind::RateLimited => format!(
                "{} provider rate limit reached for profile '{}'",
                provider_type, profile_name
            ),
            ProviderErrorKind::Unavailable => format!(
                "{} provider is temporarily unavailable for profile '{}'",
                provider_type, profile_name
            ),
            ProviderErrorKind::InvalidRequest => format!(
                "{} provider rejected the request for profile '{}'",
                provider_type, profile_name
            ),
            ProviderErrorKind::Transport => format!(
                "{} provider transport failed for profile '{}'",
                provider_type, profile_name
            ),
            ProviderErrorKind::Response => format!(
                "{} provider returned an invalid response for profile '{}'",
                provider_type, profile_name
            ),
            ProviderErrorKind::Unsupported => format!(
                "{} provider is not supported for profile '{}'",
                provider_type, profile_name
            ),
            ProviderErrorKind::Unknown => format!(
                "{} provider request failed for profile '{}'",
                provider_type, profile_name
            ),
        };
        if let Some(status_code) = status_code {
            public_message.push_str(&format!(" (status {status_code})"));
        }

        Self {
            kind,
            provider_type,
            profile_name,
            model,
            message,
            public_message,
            status_code,
            retryable,
            attempts: Vec::new(),
        }
    }

    pub fn public_message(&self) -> &str {
        &self.public_message
    }

    pub fn kind_label(&self) -> &'static str {
        self.kind.as_str()
    }

    fn with_attempts(mut self, attempts: Vec<ProviderAttempt>) -> Self {
        self.attempts = attempts;
        self
    }
}

impl std::fmt::Display for ProviderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.public_message)
    }
}

impl std::error::Error for ProviderError {}

#[async_trait]
pub trait LlmProvider: Send + Sync {
    fn metadata(&self) -> ProviderTransportMetadata;

    fn tool_call_shadow_message(&self, _tool_calls: &[ToolCall]) -> Option<Message> {
        None
    }

    async fn complete(
        &self,
        messages: &[Message],
        tools: Option<&[ToolDefinition]>,
    ) -> std::result::Result<ProviderCompletion, ProviderError>;
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
    pub channel: Option<String>,
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
    if let Some(provider_error) = error.downcast_ref::<ProviderError>() {
        return provider_error.public_message().to_owned();
    }

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

fn channel_policy_profile(
    candidates: &[ProviderProfile],
    channel: &str,
) -> Option<ProviderProfile> {
    candidates
        .iter()
        .find(|profile| profile.name == channel)
        .cloned()
}

fn redact_provider_message(message: &str) -> String {
    let mut redacted = message.to_owned();
    for prefix in ["Bearer ", "api-key: ", "x-api-key: "] {
        redacted = redact_after_prefix(&redacted, prefix, &[' ', '\n', '\r', '\t', ',', ';']);
    }
    for prefix in [
        "\"api-key\":\"",
        "\"x-api-key\":\"",
        "\"authorization\":\"Bearer ",
    ] {
        redacted = redact_after_prefix(&redacted, prefix, &['\"']);
    }
    redacted
}

fn redact_after_prefix(value: &str, prefix: &str, terminators: &[char]) -> String {
    let remaining = value;
    let mut output = String::new();
    loop {
        let Some(index) = remaining.find(prefix) else {
            output.push_str(remaining);
            break;
        };
        let start = index + prefix.len();
        output.push_str(&remaining[..start]);
        let tail = &remaining[start..];
        let end = tail
            .find(|ch| terminators.contains(&ch))
            .unwrap_or(tail.len());
        output.push_str("<redacted>");
        output.push_str(&tail[end..]);
        break;
    }
    output
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

        if let Some(channel) = request.channel.as_deref() {
            if let Some(profile) = channel_policy_profile(&candidates, channel) {
                return Ok(ScheduledProfile {
                    profile,
                    reason: format!("channel_policy:{}", channel),
                });
            }
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

impl MockProvider {
    fn success_attempt() -> Vec<ProviderAttempt> {
        vec![ProviderAttempt {
            attempt: 1,
            max_attempts: 1,
            status: "success".to_owned(),
            error_kind: None,
            status_code: None,
            retryable: false,
            message: None,
        }]
    }
}

#[async_trait]
impl LlmProvider for MockProvider {
    fn metadata(&self) -> ProviderTransportMetadata {
        ProviderTransportMetadata {
            provider_type: "mock".to_owned(),
            base_url: None,
            timeout_ms: 0,
            max_retries: 0,
            supports_tool_call_shadow_messages: false,
        }
    }

    async fn complete(
        &self,
        messages: &[Message],
        tools: Option<&[ToolDefinition]>,
    ) -> std::result::Result<ProviderCompletion, ProviderError> {
        let last = messages.last().ok_or_else(|| {
            ProviderError::new(
                ProviderErrorKind::InvalidRequest,
                "mock",
                "mock",
                "mock",
                "no messages",
                None,
                false,
            )
        })?;

        let response = match last.role {
            Role::User => {
                let content = last.content.to_lowercase();
                let time_tool = resolve_tool_name(tools, "time_now");
                let read_file_tool = resolve_tool_name(tools, "read_file");

                if content.contains("time") {
                    if let Some(tool_name) = time_tool {
                        CompletionResponse {
                            message: None,
                            tool_calls: vec![ToolCall {
                                id: format!("call_mock_{}", tool_name.replace('.', "_")),
                                name: tool_name,
                                arguments: serde_json::json!({}),
                            }],
                            finish_reason: Some("tool_calls".to_owned()),
                        }
                    } else {
                        CompletionResponse {
                            message: Some(Message {
                                role: Role::Assistant,
                                content: format!("mock response: {}", last.content),
                                tool_call_id: None,
                            }),
                            tool_calls: vec![],
                            finish_reason: Some("stop".to_owned()),
                        }
                    }
                } else if content.contains("read") || content.contains("file") {
                    if let Some(tool_name) = read_file_tool {
                        CompletionResponse {
                            message: None,
                            tool_calls: vec![ToolCall {
                                id: format!("call_mock_{}", tool_name.replace('.', "_")),
                                name: tool_name,
                                arguments: serde_json::json!({ "path": "README.md" }),
                            }],
                            finish_reason: Some("tool_calls".to_owned()),
                        }
                    } else {
                        CompletionResponse {
                            message: Some(Message {
                                role: Role::Assistant,
                                content: format!("mock response: {}", last.content),
                                tool_call_id: None,
                            }),
                            tool_calls: vec![],
                            finish_reason: Some("stop".to_owned()),
                        }
                    }
                } else {
                    CompletionResponse {
                        message: Some(Message {
                            role: Role::Assistant,
                            content: format!("mock response: {}", last.content),
                            tool_call_id: None,
                        }),
                        tool_calls: vec![],
                        finish_reason: Some("stop".to_owned()),
                    }
                }
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

                CompletionResponse {
                    message: Some(Message {
                        role: Role::Assistant,
                        content: reply,
                        tool_call_id: None,
                    }),
                    tool_calls: vec![],
                    finish_reason: Some("stop".to_owned()),
                }
            }
            _ => CompletionResponse {
                message: Some(Message {
                    role: Role::Assistant,
                    content: "mock response".to_owned(),
                    tool_call_id: None,
                }),
                tool_calls: vec![],
                finish_reason: Some("stop".to_owned()),
            },
        };

        Ok(ProviderCompletion {
            response,
            attempts: Self::success_attempt(),
        })
    }
}

#[derive(Clone)]
struct OpenAiStyleProvider {
    client: Client,
    provider_type: String,
    profile_name: String,
    model: String,
    base_url: String,
    auth: RequestAuth,
    metadata: ProviderTransportMetadata,
    endpoint: OpenAiStyleEndpoint,
}

#[derive(Clone, Copy)]
enum OpenAiStyleEndpoint {
    Standard,
    Azure,
    Ollama,
}

#[derive(Clone)]
enum RequestAuth {
    None,
    Bearer(String),
    ApiKey(String),
}

impl OpenAiStyleProvider {
    fn metadata(&self) -> ProviderTransportMetadata {
        self.metadata.clone()
    }

    async fn complete(
        &self,
        messages: &[Message],
        tools: Option<&[ToolDefinition]>,
    ) -> std::result::Result<ProviderCompletion, ProviderError> {
        let url = match self.endpoint {
            OpenAiStyleEndpoint::Standard => openai_chat_completions_url(&self.base_url),
            OpenAiStyleEndpoint::Azure => azure_chat_completions_url(&self.base_url, &self.model),
            OpenAiStyleEndpoint::Ollama => ollama_chat_completions_url(&self.base_url),
        };
        let body = build_openai_style_body(
            &self.model,
            messages,
            tools,
            !matches!(self.endpoint, OpenAiStyleEndpoint::Azure),
        );
        let request = JsonRequest {
            url,
            auth: self.auth.clone(),
            headers: Vec::new(),
            body,
        };
        let metadata = self.metadata();

        let (response, attempts) = execute_with_retry(&metadata, || async {
            send_json_request::<ApiChatCompletionResponse>(
                &self.client,
                &request,
                &self.provider_type,
                &self.profile_name,
                &self.model,
            )
            .await
        })
        .await?;

        Ok(ProviderCompletion {
            response: parse_openai_style_response(
                response,
                &self.provider_type,
                &self.profile_name,
                &self.model,
            )?,
            attempts,
        })
    }
}

pub struct OpenAiProvider {
    inner: OpenAiStyleProvider,
}

impl OpenAiProvider {
    pub fn new(profile_name: String, base_url: String, api_key: String, model: String) -> Self {
        let metadata = ProviderTransportMetadata {
            provider_type: ProviderType::OpenAi.as_str().to_owned(),
            base_url: Some(base_url.clone()),
            timeout_ms: DEFAULT_TIMEOUT_MS,
            max_retries: DEFAULT_MAX_RETRIES,
            supports_tool_call_shadow_messages: false,
        };
        Self {
            inner: OpenAiStyleProvider {
                client: build_http_client(metadata.timeout_ms),
                provider_type: ProviderType::OpenAi.as_str().to_owned(),
                profile_name,
                model,
                base_url,
                auth: RequestAuth::Bearer(api_key),
                metadata,
                endpoint: OpenAiStyleEndpoint::Standard,
            },
        }
    }

    fn from_profile(profile: &ProviderProfile) -> Result<Self> {
        Ok(Self::new(
            profile.name.clone(),
            resolve_base_url(profile, ProviderType::OpenAi)?,
            resolve_api_key(profile, true)?.expect("openai requires api key"),
            profile.model.clone(),
        ))
    }
}

#[async_trait]
impl LlmProvider for OpenAiProvider {
    fn metadata(&self) -> ProviderTransportMetadata {
        self.inner.metadata()
    }

    async fn complete(
        &self,
        messages: &[Message],
        tools: Option<&[ToolDefinition]>,
    ) -> std::result::Result<ProviderCompletion, ProviderError> {
        self.inner.complete(messages, tools).await
    }
}

pub struct OpenAiCompatibleProvider {
    inner: OpenAiStyleProvider,
}

impl OpenAiCompatibleProvider {
    pub fn new(profile_name: String, base_url: String, api_key: String, model: String) -> Self {
        let metadata = ProviderTransportMetadata {
            provider_type: ProviderType::OpenAiCompatible.as_str().to_owned(),
            base_url: Some(base_url.clone()),
            timeout_ms: DEFAULT_TIMEOUT_MS,
            max_retries: DEFAULT_MAX_RETRIES,
            supports_tool_call_shadow_messages: false,
        };
        Self {
            inner: OpenAiStyleProvider {
                client: build_http_client(metadata.timeout_ms),
                provider_type: ProviderType::OpenAiCompatible.as_str().to_owned(),
                profile_name,
                model,
                base_url,
                auth: RequestAuth::Bearer(api_key),
                metadata,
                endpoint: OpenAiStyleEndpoint::Standard,
            },
        }
    }

    fn from_profile(profile: &ProviderProfile) -> Result<Self> {
        Ok(Self::new(
            profile.name.clone(),
            resolve_base_url(profile, ProviderType::OpenAiCompatible)?,
            resolve_api_key(profile, true)?.expect("openai-compatible requires api key"),
            profile.model.clone(),
        ))
    }
}

#[async_trait]
impl LlmProvider for OpenAiCompatibleProvider {
    fn metadata(&self) -> ProviderTransportMetadata {
        self.inner.metadata()
    }

    async fn complete(
        &self,
        messages: &[Message],
        tools: Option<&[ToolDefinition]>,
    ) -> std::result::Result<ProviderCompletion, ProviderError> {
        self.inner.complete(messages, tools).await
    }
}

pub struct AzureProvider {
    inner: OpenAiStyleProvider,
}

impl AzureProvider {
    pub fn new(
        profile_name: String,
        base_url: String,
        api_key: String,
        deployment: String,
    ) -> Self {
        let metadata = ProviderTransportMetadata {
            provider_type: ProviderType::Azure.as_str().to_owned(),
            base_url: Some(base_url.clone()),
            timeout_ms: DEFAULT_TIMEOUT_MS,
            max_retries: DEFAULT_MAX_RETRIES,
            supports_tool_call_shadow_messages: false,
        };
        Self {
            inner: OpenAiStyleProvider {
                client: build_http_client(metadata.timeout_ms),
                provider_type: ProviderType::Azure.as_str().to_owned(),
                profile_name,
                model: deployment,
                base_url,
                auth: RequestAuth::ApiKey(api_key),
                metadata,
                endpoint: OpenAiStyleEndpoint::Azure,
            },
        }
    }

    fn from_profile(profile: &ProviderProfile) -> Result<Self> {
        Ok(Self::new(
            profile.name.clone(),
            resolve_base_url(profile, ProviderType::Azure)?,
            resolve_api_key(profile, true)?.expect("azure requires api key"),
            profile.model.clone(),
        ))
    }
}

#[async_trait]
impl LlmProvider for AzureProvider {
    fn metadata(&self) -> ProviderTransportMetadata {
        self.inner.metadata()
    }

    async fn complete(
        &self,
        messages: &[Message],
        tools: Option<&[ToolDefinition]>,
    ) -> std::result::Result<ProviderCompletion, ProviderError> {
        self.inner.complete(messages, tools).await
    }
}

pub struct OllamaProvider {
    inner: OpenAiStyleProvider,
}

impl OllamaProvider {
    pub fn new(
        profile_name: String,
        base_url: String,
        api_key: Option<String>,
        model: String,
    ) -> Self {
        let metadata = ProviderTransportMetadata {
            provider_type: ProviderType::Ollama.as_str().to_owned(),
            base_url: Some(base_url.clone()),
            timeout_ms: OLLAMA_TIMEOUT_MS,
            max_retries: 1,
            supports_tool_call_shadow_messages: false,
        };
        Self {
            inner: OpenAiStyleProvider {
                client: build_http_client(metadata.timeout_ms),
                provider_type: ProviderType::Ollama.as_str().to_owned(),
                profile_name,
                model,
                base_url,
                auth: api_key
                    .map(RequestAuth::Bearer)
                    .unwrap_or(RequestAuth::None),
                metadata,
                endpoint: OpenAiStyleEndpoint::Ollama,
            },
        }
    }

    fn from_profile(profile: &ProviderProfile) -> Result<Self> {
        Ok(Self::new(
            profile.name.clone(),
            resolve_base_url(profile, ProviderType::Ollama)?,
            resolve_api_key(profile, false)?,
            profile.model.clone(),
        ))
    }
}

#[async_trait]
impl LlmProvider for OllamaProvider {
    fn metadata(&self) -> ProviderTransportMetadata {
        self.inner.metadata()
    }

    async fn complete(
        &self,
        messages: &[Message],
        tools: Option<&[ToolDefinition]>,
    ) -> std::result::Result<ProviderCompletion, ProviderError> {
        self.inner.complete(messages, tools).await
    }
}

pub struct AnthropicProvider {
    client: Client,
    profile_name: String,
    base_url: String,
    api_key: String,
    model: String,
    metadata: ProviderTransportMetadata,
}

impl AnthropicProvider {
    pub fn new(profile_name: String, base_url: String, api_key: String, model: String) -> Self {
        let metadata = ProviderTransportMetadata {
            provider_type: ProviderType::Anthropic.as_str().to_owned(),
            base_url: Some(base_url.clone()),
            timeout_ms: ANTHROPIC_TIMEOUT_MS,
            max_retries: DEFAULT_MAX_RETRIES,
            supports_tool_call_shadow_messages: true,
        };
        Self {
            client: build_http_client(metadata.timeout_ms),
            profile_name,
            base_url,
            api_key,
            model,
            metadata,
        }
    }

    fn from_profile(profile: &ProviderProfile) -> Result<Self> {
        Ok(Self::new(
            profile.name.clone(),
            resolve_base_url(profile, ProviderType::Anthropic)?,
            resolve_api_key(profile, true)?.expect("anthropic requires api key"),
            profile.model.clone(),
        ))
    }
}

#[async_trait]
impl LlmProvider for AnthropicProvider {
    fn metadata(&self) -> ProviderTransportMetadata {
        self.metadata.clone()
    }

    fn tool_call_shadow_message(&self, tool_calls: &[ToolCall]) -> Option<Message> {
        Some(Message {
            role: Role::Assistant,
            content: tool_call_shadow_content(tool_calls),
            tool_call_id: None,
        })
    }

    async fn complete(
        &self,
        messages: &[Message],
        tools: Option<&[ToolDefinition]>,
    ) -> std::result::Result<ProviderCompletion, ProviderError> {
        let url = anthropic_messages_url(&self.base_url);
        let body = build_anthropic_body(&self.model, messages, tools);
        let request = JsonRequest {
            url,
            auth: RequestAuth::ApiKey(self.api_key.clone()),
            headers: vec![(
                "anthropic-version".to_owned(),
                ANTHROPIC_VERSION_HEADER.to_owned(),
            )],
            body,
        };
        let metadata = self.metadata();

        let (response, attempts) = execute_with_retry(&metadata, || async {
            send_json_request::<AnthropicResponse>(
                &self.client,
                &request,
                &metadata.provider_type,
                &self.profile_name,
                &self.model,
            )
            .await
        })
        .await?;

        Ok(ProviderCompletion {
            response: parse_anthropic_response(
                response,
                &metadata.provider_type,
                &self.profile_name,
                &self.model,
            )?,
            attempts,
        })
    }
}

pub fn build_provider_from_profile(profile: &ProviderProfile) -> Result<Arc<dyn LlmProvider>> {
    let provider_type = parse_provider_type(&profile.provider_type)
        .ok_or_else(|| anyhow!("unsupported provider type: {}", profile.provider_type))?;

    let provider: Arc<dyn LlmProvider> = match provider_type {
        ProviderType::Mock => Arc::new(MockProvider),
        ProviderType::OpenAi => Arc::new(OpenAiProvider::from_profile(profile)?),
        ProviderType::Azure => Arc::new(AzureProvider::from_profile(profile)?),
        ProviderType::Anthropic => Arc::new(AnthropicProvider::from_profile(profile)?),
        ProviderType::Ollama => Arc::new(OllamaProvider::from_profile(profile)?),
        ProviderType::OpenAiCompatible => {
            Arc::new(OpenAiCompatibleProvider::from_profile(profile)?)
        }
    };

    Ok(provider)
}

fn build_http_client(timeout_ms: u64) -> Client {
    Client::builder()
        .timeout(Duration::from_millis(timeout_ms))
        .build()
        .expect("provider HTTP client should build")
}

fn resolve_api_key(profile: &ProviderProfile, required: bool) -> Result<Option<String>> {
    match profile.api_key_env.as_deref() {
        Some(env_var) => env::var(env_var).map(Some).map_err(|_| {
            anyhow!(
                "profile '{}' expects environment variable {} to be set",
                profile.name,
                env_var
            )
        }),
        None if required => bail!("profile '{}' is missing api_key_env", profile.name),
        None => Ok(None),
    }
}

fn resolve_base_url(profile: &ProviderProfile, provider_type: ProviderType) -> Result<String> {
    profile
        .base_url
        .clone()
        .or_else(|| provider_type.default_base_url().map(str::to_owned))
        .ok_or_else(|| anyhow!("profile '{}' is missing base_url", profile.name))
}

struct JsonRequest {
    url: String,
    auth: RequestAuth,
    headers: Vec<(String, String)>,
    body: serde_json::Value,
}

async fn send_json_request<T: DeserializeOwned>(
    client: &Client,
    request: &JsonRequest,
    provider_type: &str,
    profile_name: &str,
    model: &str,
) -> std::result::Result<T, ProviderError> {
    let mut builder = client.post(request.url.clone());

    builder = match &request.auth {
        RequestAuth::None => builder,
        RequestAuth::Bearer(token) => builder.bearer_auth(token),
        RequestAuth::ApiKey(token) => builder.header("api-key", token),
    };

    for (key, value) in &request.headers {
        builder = builder.header(key, value);
    }

    let response = builder
        .json(&request.body)
        .send()
        .await
        .map_err(|error| translate_reqwest_error(provider_type, profile_name, model, error))?;

    let status = response.status();
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        return Err(translate_status_error(
            provider_type,
            profile_name,
            model,
            status.as_u16(),
            &body,
        ));
    }

    response
        .json::<T>()
        .await
        .map_err(|error| translate_decode_error(provider_type, profile_name, model, error))
}

async fn execute_with_retry<T, F, Fut>(
    metadata: &ProviderTransportMetadata,
    mut operation: F,
) -> std::result::Result<(T, Vec<ProviderAttempt>), ProviderError>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = std::result::Result<T, ProviderError>>,
{
    let max_attempts = metadata.max_retries.saturating_add(1);
    let mut attempts = Vec::new();

    for attempt in 1..=max_attempts {
        match operation().await {
            Ok(value) => {
                attempts.push(ProviderAttempt {
                    attempt,
                    max_attempts,
                    status: "success".to_owned(),
                    error_kind: None,
                    status_code: None,
                    retryable: false,
                    message: None,
                });
                return Ok((value, attempts));
            }
            Err(error) => {
                let retryable = error.retryable && attempt < max_attempts;
                attempts.push(ProviderAttempt {
                    attempt,
                    max_attempts,
                    status: if retryable { "retry" } else { "failed" }.to_owned(),
                    error_kind: Some(error.kind_label().to_owned()),
                    status_code: error.status_code,
                    retryable: error.retryable,
                    message: Some(error.public_message().to_owned()),
                });
                if retryable {
                    warn!(
                        provider_type = metadata.provider_type,
                        attempt,
                        max_attempts,
                        error_kind = error.kind_label(),
                        retryable = error.retryable,
                        "retrying provider request"
                    );
                    sleep(Duration::from_millis(150 * attempt as u64)).await;
                    continue;
                }

                return Err(error.with_attempts(attempts));
            }
        }
    }

    unreachable!("provider retry loop should always return")
}

fn translate_reqwest_error(
    provider_type: &str,
    profile_name: &str,
    model: &str,
    error: reqwest::Error,
) -> ProviderError {
    let kind = if error.is_timeout() {
        ProviderErrorKind::Timeout
    } else {
        ProviderErrorKind::Transport
    };

    ProviderError::new(
        kind,
        provider_type,
        profile_name,
        model,
        error.to_string(),
        error.status().map(|status| status.as_u16()),
        matches!(
            kind,
            ProviderErrorKind::Timeout | ProviderErrorKind::Transport
        ),
    )
}

fn translate_status_error(
    provider_type: &str,
    profile_name: &str,
    model: &str,
    status_code: u16,
    body: &str,
) -> ProviderError {
    let (kind, retryable) = match status_code {
        401 | 403 => (ProviderErrorKind::Auth, false),
        408 | 504 => (ProviderErrorKind::Timeout, true),
        429 => (ProviderErrorKind::RateLimited, true),
        400..=499 => (ProviderErrorKind::InvalidRequest, false),
        500..=599 => (ProviderErrorKind::Unavailable, true),
        _ => (ProviderErrorKind::Unknown, false),
    };

    ProviderError::new(
        kind,
        provider_type,
        profile_name,
        model,
        if body.trim().is_empty() {
            format!("upstream returned status {status_code}")
        } else {
            format!("status {status_code}: {}", truncate_preview(body, 240))
        },
        Some(status_code),
        retryable,
    )
}

fn translate_decode_error(
    provider_type: &str,
    profile_name: &str,
    model: &str,
    error: reqwest::Error,
) -> ProviderError {
    ProviderError::new(
        ProviderErrorKind::Response,
        provider_type,
        profile_name,
        model,
        error.to_string(),
        None,
        false,
    )
}

fn openai_chat_completions_url(base_url: &str) -> String {
    let trimmed = base_url.trim_end_matches('/');
    if trimmed.ends_with("/chat/completions") {
        trimmed.to_owned()
    } else {
        format!("{trimmed}/chat/completions")
    }
}

fn azure_chat_completions_url(base_url: &str, deployment: &str) -> String {
    let trimmed = base_url.trim_end_matches('/');
    format!(
        "{trimmed}/openai/deployments/{deployment}/chat/completions?api-version={AZURE_CHAT_COMPLETIONS_API_VERSION}"
    )
}

fn ollama_chat_completions_url(base_url: &str) -> String {
    let trimmed = base_url.trim_end_matches('/');
    if trimmed.ends_with("/chat/completions") {
        trimmed.to_owned()
    } else if trimmed.ends_with("/v1") {
        format!("{trimmed}/chat/completions")
    } else {
        format!("{trimmed}/v1/chat/completions")
    }
}

fn anthropic_messages_url(base_url: &str) -> String {
    let trimmed = base_url.trim_end_matches('/');
    if trimmed.ends_with("/messages") {
        trimmed.to_owned()
    } else {
        format!("{trimmed}/messages")
    }
}

fn build_openai_style_body(
    model: &str,
    messages: &[Message],
    tools: Option<&[ToolDefinition]>,
    include_model: bool,
) -> serde_json::Value {
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

    match (include_model, req_tools.filter(|defs| !defs.is_empty())) {
        (true, Some(req_tools)) => serde_json::json!({
            "model": model,
            "messages": req_messages,
            "tools": req_tools,
            "tool_choice": "auto",
        }),
        (true, None) => serde_json::json!({
            "model": model,
            "messages": req_messages,
        }),
        (false, Some(req_tools)) => serde_json::json!({
            "messages": req_messages,
            "tools": req_tools,
            "tool_choice": "auto",
        }),
        (false, None) => serde_json::json!({
            "messages": req_messages,
        }),
    }
}

fn build_anthropic_body(
    model: &str,
    messages: &[Message],
    tools: Option<&[ToolDefinition]>,
) -> serde_json::Value {
    let system = messages
        .iter()
        .filter(|message| matches!(message.role, Role::System))
        .map(|message| message.content.clone())
        .collect::<Vec<_>>()
        .join("\n\n");

    let mut body = serde_json::json!({
        "model": model,
        "max_tokens": 2048,
        "messages": anthropic_messages_from_conversation(messages),
    });

    if !system.is_empty() {
        body["system"] = serde_json::Value::String(system);
    }

    if let Some(req_tools) = tools.filter(|defs| !defs.is_empty()) {
        body["tools"] = serde_json::Value::Array(
            req_tools
                .iter()
                .map(|tool| {
                    serde_json::json!({
                        "name": tool.name,
                        "description": tool.description,
                        "input_schema": tool.input_schema,
                    })
                })
                .collect(),
        );
    }

    body
}

fn anthropic_messages_from_conversation(messages: &[Message]) -> Vec<serde_json::Value> {
    let mut request_messages = Vec::new();

    for message in messages {
        match message.role {
            Role::System => {}
            Role::User => push_anthropic_message(
                &mut request_messages,
                "user",
                serde_json::json!({
                    "type": "text",
                    "text": message.content,
                }),
            ),
            Role::Assistant => {
                if let Some(tool_calls) = parse_tool_call_shadow(&message.content) {
                    for tool_call in tool_calls {
                        push_anthropic_message(
                            &mut request_messages,
                            "assistant",
                            serde_json::json!({
                                "type": "tool_use",
                                "id": tool_call.id,
                                "name": tool_call.name,
                                "input": tool_call.arguments,
                            }),
                        );
                    }
                } else {
                    push_anthropic_message(
                        &mut request_messages,
                        "assistant",
                        serde_json::json!({
                            "type": "text",
                            "text": message.content,
                        }),
                    );
                }
            }
            Role::Tool => push_anthropic_message(
                &mut request_messages,
                "user",
                serde_json::json!({
                    "type": "tool_result",
                    "tool_use_id": message.tool_call_id.clone().unwrap_or_else(|| "unknown".to_owned()),
                    "content": message.content,
                }),
            ),
        }
    }

    request_messages
}

fn push_anthropic_message(
    messages: &mut Vec<serde_json::Value>,
    role: &str,
    block: serde_json::Value,
) {
    if let Some(last) = messages.last_mut() {
        if last.get("role").and_then(|value| value.as_str()) == Some(role) {
            last.get_mut("content")
                .and_then(|value| value.as_array_mut())
                .expect("anthropic message content should be an array")
                .push(block);
            return;
        }
    }

    messages.push(serde_json::json!({
        "role": role,
        "content": [block],
    }));
}

fn tool_call_shadow_content(tool_calls: &[ToolCall]) -> String {
    format!(
        "{TOOL_CALL_SHADOW_PREFIX}{}",
        serde_json::to_string(tool_calls).unwrap_or_else(|_| "[]".to_owned())
    )
}

fn parse_tool_call_shadow(content: &str) -> Option<Vec<ToolCall>> {
    let payload = content.strip_prefix(TOOL_CALL_SHADOW_PREFIX)?;
    serde_json::from_str(payload).ok()
}

fn parse_openai_style_response(
    response: ApiChatCompletionResponse,
    provider_type: &str,
    profile_name: &str,
    model: &str,
) -> std::result::Result<CompletionResponse, ProviderError> {
    let choice = response.choices.into_iter().next().ok_or_else(|| {
        ProviderError::new(
            ProviderErrorKind::Response,
            provider_type,
            profile_name,
            model,
            "empty choices in provider response",
            None,
            false,
        )
    })?;

    let tool_calls = choice
        .message
        .tool_calls
        .unwrap_or_default()
        .into_iter()
        .map(|tool_call| ToolCall {
            id: tool_call.id,
            name: tool_call.function.name,
            arguments: serde_json::from_str::<serde_json::Value>(&tool_call.function.arguments)
                .unwrap_or_else(|_| serde_json::json!({})),
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

fn parse_anthropic_response(
    response: AnthropicResponse,
    provider_type: &str,
    profile_name: &str,
    model: &str,
) -> std::result::Result<CompletionResponse, ProviderError> {
    let mut text_blocks = Vec::new();
    let mut tool_calls = Vec::new();

    for block in response.content {
        match block.kind.as_str() {
            "text" => {
                if let Some(text) = block.text {
                    text_blocks.push(text);
                }
            }
            "tool_use" => {
                tool_calls.push(ToolCall {
                    id: block.id.unwrap_or_else(|| "tool_use_missing_id".to_owned()),
                    name: block
                        .name
                        .unwrap_or_else(|| "tool_use_missing_name".to_owned()),
                    arguments: block.input.unwrap_or_else(|| serde_json::json!({})),
                });
            }
            _ => {}
        }
    }

    if text_blocks.is_empty() && tool_calls.is_empty() {
        return Err(ProviderError::new(
            ProviderErrorKind::Response,
            provider_type,
            profile_name,
            model,
            "anthropic response did not include text or tool_use content",
            None,
            false,
        ));
    }

    Ok(CompletionResponse {
        message: if tool_calls.is_empty() {
            Some(Message {
                role: Role::Assistant,
                content: text_blocks.join("\n"),
                tool_call_id: None,
            })
        } else {
            None
        },
        tool_calls,
        finish_reason: response.stop_reason,
    })
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

fn truncate_preview(value: &str, limit: usize) -> String {
    preview_text(value, limit)
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
    let provider_type = parse_provider_type(provider_type);
    let family = if model == "mock" {
        "mock".to_owned()
    } else if model.starts_with("gpt-5.4") {
        "gpt-5.4".to_owned()
    } else if model.starts_with("gpt-") {
        model.split('-').take(2).collect::<Vec<_>>().join("-")
    } else if model.starts_with("claude") {
        "claude".to_owned()
    } else {
        provider_type
            .map(ProviderType::as_str)
            .unwrap_or("custom")
            .to_owned()
    };

    let (context_window_chars, budget_tier) = infer_context_budget(provider_type, model);

    ModelCapabilities {
        supports_tools: matches!(
            provider_type,
            Some(
                ProviderType::Mock
                    | ProviderType::OpenAi
                    | ProviderType::Azure
                    | ProviderType::Anthropic
                    | ProviderType::Ollama
                    | ProviderType::OpenAiCompatible
            )
        ),
        supports_sessions: true,
        family,
        context_window_chars,
        budget_tier: budget_tier.to_owned(),
    }
}

fn infer_context_budget(provider_type: Option<ProviderType>, model: &str) -> (usize, &'static str) {
    if model == "mock" {
        (64_000, "medium")
    } else if model.starts_with("claude") || matches!(provider_type, Some(ProviderType::Anthropic))
    {
        (180_000, "large")
    } else if model.contains("mini") {
        (32_000, "small")
    } else if model.starts_with("gpt-5.4") {
        (128_000, "large")
    } else if model.starts_with("gpt-") {
        (64_000, "medium")
    } else if matches!(provider_type, Some(ProviderType::Ollama)) {
        (24_000, "small")
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

#[derive(Debug, Deserialize)]
struct AnthropicResponse {
    content: Vec<AnthropicContentBlock>,
    stop_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct AnthropicContentBlock {
    #[serde(rename = "type")]
    kind: String,
    text: Option<String>,
    id: Option<String>,
    name: Option<String>,
    input: Option<serde_json::Value>,
}

#[cfg(test)]
mod tests {
    use std::{
        collections::BTreeMap,
        sync::{Arc, Mutex},
    };

    use axum::{
        Json, Router,
        body::{Body, to_bytes},
        extract::State,
        http::{Request, StatusCode},
        response::IntoResponse,
        routing::any,
    };
    use futures::executor::block_on;
    use mosaic_config::{MosaicConfig, ProviderProfileConfig};
    use mosaic_tool_core::{CapabilityKind, CapabilityMetadata, ToolMetadata};
    use tokio::net::TcpListener;

    use super::{
        AnthropicProvider, AzureProvider, LlmProvider, Message, MockProvider, OllamaProvider,
        OpenAiCompatibleProvider, OpenAiProvider, ProviderError, ProviderErrorKind,
        ProviderProfileRegistry, Role, SchedulingIntent, SchedulingRequest, ToolDefinition,
        public_error_message, tool_definition_from_metadata, tool_is_visible_to_model,
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
            response
                .response
                .message
                .expect("message should exist")
                .content,
            "mock response: hello"
        );
        assert_eq!(response.response.finish_reason.as_deref(), Some("stop"));
        assert!(response.response.tool_calls.is_empty());
        assert_eq!(response.attempts.len(), 1);
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

        assert!(response.response.message.is_none());
        assert_eq!(
            response.response.finish_reason.as_deref(),
            Some("tool_calls")
        );
        assert_eq!(response.response.tool_calls.len(), 1);
        assert_eq!(response.response.tool_calls[0].name, "time_now");
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

        assert!(response.response.message.is_none());
        assert_eq!(response.response.tool_calls.len(), 1);
        assert_eq!(response.response.tool_calls[0].name, "read_file");
        assert_eq!(
            response.response.tool_calls[0].arguments,
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
            response
                .response
                .message
                .expect("message should exist")
                .content,
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

        let message = response.response.message.expect("message should exist");
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

        assert!(response.response.message.is_none());
        assert_eq!(response.response.tool_calls.len(), 1);
        assert_eq!(
            response.response.tool_calls[0].name,
            "mcp.filesystem.read_file"
        );
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

        assert!(response.response.tool_calls.is_empty());
        assert_eq!(
            response
                .response
                .message
                .expect("message should exist")
                .content,
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
                channel: None,
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
                channel: None,
                intent: SchedulingIntent::InteractiveRun,
                estimated_context_chars: 40_000,
                requires_tools: false,
            })
            .expect("interactive schedule should succeed");

        assert_eq!(scheduled.profile.name, "large");
        assert_eq!(scheduled.reason, "expanded_context_window");
    }

    #[test]
    fn channel_policy_prefers_matching_channel_profile() {
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
            "telegram".to_owned(),
            ProviderProfileConfig {
                provider_type: "mock".to_owned(),
                model: "gpt-5.4".to_owned(),
                base_url: None,
                api_key_env: None,
            },
        );
        let registry =
            ProviderProfileRegistry::from_config(&config).expect("registry should build");

        let scheduled = registry
            .schedule(SchedulingRequest {
                requested_profile: None,
                channel: Some("telegram".to_owned()),
                intent: SchedulingIntent::InteractiveRun,
                estimated_context_chars: 200,
                requires_tools: false,
            })
            .expect("channel schedule should succeed");

        assert_eq!(scheduled.profile.name, "telegram");
        assert_eq!(scheduled.reason, "channel_policy:telegram");
    }

    #[derive(Debug, Clone)]
    struct CapturedRequest {
        path: String,
        query: Option<String>,
        headers: BTreeMap<String, String>,
        body: serde_json::Value,
    }

    #[derive(Clone)]
    struct ServerState {
        response_status: StatusCode,
        response_body: serde_json::Value,
        requests: Arc<Mutex<Vec<CapturedRequest>>>,
    }

    async fn capture_request(
        State(state): State<ServerState>,
        request: Request<Body>,
    ) -> impl IntoResponse {
        let (parts, body) = request.into_parts();
        let bytes = to_bytes(body, usize::MAX)
            .await
            .expect("request body should be readable");
        let json_body = serde_json::from_slice::<serde_json::Value>(&bytes)
            .unwrap_or_else(|_| serde_json::Value::Null);
        let headers = parts
            .headers
            .iter()
            .map(|(name, value)| {
                (
                    name.as_str().to_owned(),
                    value.to_str().unwrap_or_default().to_owned(),
                )
            })
            .collect::<BTreeMap<_, _>>();
        state
            .requests
            .lock()
            .expect("request log lock should not be poisoned")
            .push(CapturedRequest {
                path: parts.uri.path().to_owned(),
                query: parts.uri.query().map(str::to_owned),
                headers,
                body: json_body,
            });
        (state.response_status, Json(state.response_body.clone()))
    }

    async fn start_test_server(
        response_status: StatusCode,
        response_body: serde_json::Value,
    ) -> (String, Arc<Mutex<Vec<CapturedRequest>>>) {
        let requests = Arc::new(Mutex::new(Vec::new()));
        let state = ServerState {
            response_status,
            response_body,
            requests: requests.clone(),
        };
        let app = Router::new()
            .route("/{*path}", any(capture_request))
            .with_state(state);
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("listener should bind");
        let addr = listener
            .local_addr()
            .expect("listener should have local addr");
        tokio::spawn(async move {
            axum::serve(listener, app)
                .await
                .expect("test server should stay up");
        });
        (format!("http://{addr}"), requests)
    }

    #[tokio::test]
    async fn openai_provider_formats_tools_and_bearer_auth() {
        let (base_url, requests) = start_test_server(
            StatusCode::OK,
            serde_json::json!({
                "choices": [{
                    "message": { "content": "openai ok" },
                    "finish_reason": "stop"
                }]
            }),
        )
        .await;
        let provider = OpenAiProvider::new(
            "openai".to_owned(),
            format!("{base_url}/v1"),
            "sk-openai".to_owned(),
            "gpt-5.4-mini".to_owned(),
        );

        let completion = provider
            .complete(
                &[Message {
                    role: Role::User,
                    content: "hello".to_owned(),
                    tool_call_id: None,
                }],
                Some(&[read_file_tool_definition()]),
            )
            .await
            .expect("openai provider should succeed");

        assert_eq!(
            completion
                .response
                .message
                .expect("message should exist")
                .content,
            "openai ok"
        );

        let captured = requests
            .lock()
            .expect("request log lock should not be poisoned")
            .clone();
        assert_eq!(captured.len(), 1);
        assert_eq!(captured[0].path, "/v1/chat/completions");
        assert_eq!(
            captured[0].headers.get("authorization").map(String::as_str),
            Some("Bearer sk-openai")
        );
        assert_eq!(captured[0].body["model"], "gpt-5.4-mini");
        assert_eq!(
            captured[0].body["tools"][0]["function"]["name"],
            "read_file"
        );
    }

    #[tokio::test]
    async fn azure_provider_uses_deployment_endpoint_and_api_key_auth() {
        let (base_url, requests) = start_test_server(
            StatusCode::OK,
            serde_json::json!({
                "choices": [{
                    "message": { "content": "azure ok" },
                    "finish_reason": "stop"
                }]
            }),
        )
        .await;
        let provider = AzureProvider::new(
            "azure".to_owned(),
            base_url,
            "azure-secret".to_owned(),
            "demo-deployment".to_owned(),
        );

        let completion = provider
            .complete(
                &[Message {
                    role: Role::User,
                    content: "hello".to_owned(),
                    tool_call_id: None,
                }],
                None,
            )
            .await
            .expect("azure provider should succeed");

        assert_eq!(
            completion
                .response
                .message
                .expect("message should exist")
                .content,
            "azure ok"
        );

        let captured = requests
            .lock()
            .expect("request log lock should not be poisoned")
            .clone();
        assert_eq!(captured.len(), 1);
        assert_eq!(
            captured[0].path,
            "/openai/deployments/demo-deployment/chat/completions"
        );
        assert_eq!(captured[0].query.as_deref(), Some("api-version=2024-10-21"));
        assert_eq!(
            captured[0].headers.get("api-key").map(String::as_str),
            Some("azure-secret")
        );
        assert!(captured[0].body.get("model").is_none());
    }

    #[tokio::test]
    async fn anthropic_provider_formats_messages_tools_and_shadow_tool_calls() {
        let (base_url, requests) = start_test_server(
            StatusCode::OK,
            serde_json::json!({
                "content": [
                    {
                        "type": "tool_use",
                        "id": "toolu_123",
                        "name": "read_file",
                        "input": { "path": "README.md" }
                    }
                ],
                "stop_reason": "tool_use"
            }),
        )
        .await;
        let provider = AnthropicProvider::new(
            "anthropic".to_owned(),
            format!("{base_url}/v1"),
            "anthropic-secret".to_owned(),
            "claude-sonnet-4-5".to_owned(),
        );

        let completion = provider
            .complete(
                &[
                    Message {
                        role: Role::System,
                        content: "You are helpful.".to_owned(),
                        tool_call_id: None,
                    },
                    Message {
                        role: Role::User,
                        content: "Read the workspace readme".to_owned(),
                        tool_call_id: None,
                    },
                ],
                Some(&[read_file_tool_definition()]),
            )
            .await
            .expect("anthropic provider should succeed");

        assert!(completion.response.message.is_none());
        assert_eq!(completion.response.tool_calls.len(), 1);
        assert_eq!(completion.response.tool_calls[0].name, "read_file");
        assert!(
            provider
                .tool_call_shadow_message(&completion.response.tool_calls)
                .expect("shadow message should exist")
                .content
                .starts_with("__mosaic_tool_calls__:")
        );

        let captured = requests
            .lock()
            .expect("request log lock should not be poisoned")
            .clone();
        assert_eq!(captured.len(), 1);
        assert_eq!(captured[0].path, "/v1/messages");
        assert_eq!(
            captured[0].headers.get("api-key").map(String::as_str),
            Some("anthropic-secret")
        );
        assert_eq!(
            captured[0]
                .headers
                .get("anthropic-version")
                .map(String::as_str),
            Some("2023-06-01")
        );
        assert_eq!(captured[0].body["system"], "You are helpful.");
        assert_eq!(captured[0].body["tools"][0]["name"], "read_file");
        assert_eq!(captured[0].body["messages"][0]["role"], "user");
    }

    #[tokio::test]
    async fn ollama_provider_uses_local_v1_endpoint_without_auth_by_default() {
        let (base_url, requests) = start_test_server(
            StatusCode::OK,
            serde_json::json!({
                "choices": [{
                    "message": { "content": "ollama ok" },
                    "finish_reason": "stop"
                }]
            }),
        )
        .await;
        let provider =
            OllamaProvider::new("ollama".to_owned(), base_url, None, "qwen3:14b".to_owned());

        let completion = provider
            .complete(
                &[Message {
                    role: Role::User,
                    content: "hello".to_owned(),
                    tool_call_id: None,
                }],
                None,
            )
            .await
            .expect("ollama provider should succeed");

        assert_eq!(
            completion
                .response
                .message
                .expect("message should exist")
                .content,
            "ollama ok"
        );

        let captured = requests
            .lock()
            .expect("request log lock should not be poisoned")
            .clone();
        assert_eq!(captured.len(), 1);
        assert_eq!(captured[0].path, "/v1/chat/completions");
        assert!(captured[0].headers.get("authorization").is_none());
    }

    #[tokio::test]
    async fn provider_status_errors_translate_to_structured_auth_failures() {
        let (base_url, _requests) = start_test_server(
            StatusCode::UNAUTHORIZED,
            serde_json::json!({ "error": { "message": "bad key" } }),
        )
        .await;
        let provider = OpenAiCompatibleProvider::new(
            "compat".to_owned(),
            format!("{base_url}/v1"),
            "sk-secret".to_owned(),
            "gpt-5.4-mini".to_owned(),
        );

        let error = provider
            .complete(
                &[Message {
                    role: Role::User,
                    content: "hello".to_owned(),
                    tool_call_id: None,
                }],
                None,
            )
            .await
            .expect_err("provider should fail");

        assert_eq!(error.kind, ProviderErrorKind::Auth);
        assert_eq!(error.status_code, Some(401));
        assert!(!error.retryable);
        assert!(error.public_message().contains("authentication failed"));
        assert_eq!(error.attempts.len(), 1);
    }

    #[test]
    fn public_error_message_redacts_bearer_tokens() {
        let message = public_error_message(&anyhow::anyhow!(
            "upstream provider rejected Authorization: Bearer sk-test-secret with status 401"
        ));

        assert!(message.contains("Bearer <redacted>"));
        assert!(!message.contains("sk-test-secret"));
    }

    #[test]
    fn public_error_message_preserves_structured_provider_errors() {
        let error = ProviderError::new(
            ProviderErrorKind::Timeout,
            "openai",
            "gpt-5.4",
            "gpt-5.4",
            "request timed out",
            None,
            true,
        );
        let anyhow_error = anyhow::Error::new(error.clone());

        assert_eq!(public_error_message(&anyhow_error), error.public_message());
    }
}
