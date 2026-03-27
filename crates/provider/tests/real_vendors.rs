use std::{env, time::Duration};

use mosaic_provider::{
    Message, ModelCapabilities, ProviderProfile, Role, ToolDefinition, build_provider_from_profile,
};

fn real_tests_enabled() -> bool {
    env::var("MOSAIC_REAL_TESTS").ok().as_deref() == Some("1")
}

fn provider_profile(
    name: &str,
    provider_type: &str,
    model: String,
    base_url: Option<String>,
    api_key_env: Option<&str>,
) -> ProviderProfile {
    ProviderProfile {
        name: name.to_owned(),
        provider_type: provider_type.to_owned(),
        model: model.clone(),
        base_url,
        api_key_env: api_key_env.map(str::to_owned),
        capabilities: ModelCapabilities {
            supports_tools: true,
            supports_sessions: true,
            family: provider_type.to_owned(),
            context_window_chars: 128_000,
            budget_tier: "large".to_owned(),
        },
    }
}

async fn run_real_completion(profile: ProviderProfile) {
    let provider = build_provider_from_profile(&profile).expect("provider should build");
    let tools = vec![ToolDefinition {
        name: "echo_tool".to_owned(),
        description: "Echo the supplied text".to_owned(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "text": { "type": "string" }
            },
            "required": ["text"]
        }),
    }];
    let completion = tokio::time::timeout(
        Duration::from_secs(90),
        provider.complete(
            &[Message {
                role: Role::User,
                content: "Reply with pong or call echo_tool with text pong.".to_owned(),
                tool_call_id: None,
            }],
            Some(&tools),
        ),
    )
    .await
    .expect("provider request should complete within timeout")
    .expect("provider request should succeed");

    let has_message = completion
        .response
        .message
        .as_ref()
        .is_some_and(|message| !message.content.trim().is_empty());
    let has_tool_call = !completion.response.tool_calls.is_empty();

    assert!(
        has_message || has_tool_call,
        "provider should return either a message or a tool call"
    );
}

#[tokio::test]
async fn real_openai_completion_if_configured() {
    if !real_tests_enabled() || env::var("OPENAI_API_KEY").is_err() {
        eprintln!("skipping OpenAI real test: missing MOSAIC_REAL_TESTS=1 or OPENAI_API_KEY");
        return;
    }

    run_real_completion(provider_profile(
        "openai",
        "openai",
        env::var("MOSAIC_TEST_OPENAI_MODEL").unwrap_or_else(|_| "gpt-5.4-mini".to_owned()),
        env::var("MOSAIC_TEST_OPENAI_BASE_URL").ok(),
        Some("OPENAI_API_KEY"),
    ))
    .await;
}

#[tokio::test]
async fn real_azure_completion_if_configured() {
    if !real_tests_enabled()
        || env::var("AZURE_OPENAI_API_KEY").is_err()
        || env::var("MOSAIC_TEST_AZURE_BASE_URL").is_err()
    {
        eprintln!(
            "skipping Azure real test: need MOSAIC_REAL_TESTS=1, AZURE_OPENAI_API_KEY, and MOSAIC_TEST_AZURE_BASE_URL"
        );
        return;
    }

    run_real_completion(provider_profile(
        "azure",
        "azure",
        env::var("MOSAIC_TEST_AZURE_MODEL").unwrap_or_else(|_| "gpt-5.4-mini".to_owned()),
        env::var("MOSAIC_TEST_AZURE_BASE_URL").ok(),
        Some("AZURE_OPENAI_API_KEY"),
    ))
    .await;
}

#[tokio::test]
async fn real_anthropic_completion_if_configured() {
    if !real_tests_enabled() || env::var("ANTHROPIC_API_KEY").is_err() {
        eprintln!("skipping Anthropic real test: missing MOSAIC_REAL_TESTS=1 or ANTHROPIC_API_KEY");
        return;
    }

    run_real_completion(provider_profile(
        "anthropic",
        "anthropic",
        env::var("MOSAIC_TEST_ANTHROPIC_MODEL").unwrap_or_else(|_| "claude-sonnet-4-5".to_owned()),
        env::var("MOSAIC_TEST_ANTHROPIC_BASE_URL").ok(),
        Some("ANTHROPIC_API_KEY"),
    ))
    .await;
}

#[tokio::test]
async fn real_ollama_completion_if_configured() {
    if !real_tests_enabled() {
        eprintln!("skipping Ollama real test: missing MOSAIC_REAL_TESTS=1");
        return;
    }

    run_real_completion(provider_profile(
        "ollama",
        "ollama",
        env::var("MOSAIC_TEST_OLLAMA_MODEL").unwrap_or_else(|_| "llama3.1".to_owned()),
        Some(
            env::var("MOSAIC_TEST_OLLAMA_BASE_URL")
                .unwrap_or_else(|_| "http://127.0.0.1:11434".to_owned()),
        ),
        None,
    ))
    .await;
}
