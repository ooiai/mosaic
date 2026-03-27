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

use crate::{
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
    let registry = ProviderProfileRegistry::from_config(&config).expect("registry should build");

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
    let provider = OllamaProvider::new("ollama".to_owned(), base_url, None, "qwen3:14b".to_owned());

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
