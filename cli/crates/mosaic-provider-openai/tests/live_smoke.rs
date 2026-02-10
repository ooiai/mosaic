use mosaic_core::provider::{ChatMessage, ChatRequest, ChatRole, Provider};
use mosaic_provider_openai::OpenAiCompatibleProvider;

#[tokio::test]
async fn live_models_and_chat_smoke() {
    if std::env::var("LIVE").ok().as_deref() != Some("1") {
        eprintln!("skipping live smoke test because LIVE!=1");
        return;
    }
    if std::env::var("OPENAI_API_KEY").is_err() {
        eprintln!("skipping live smoke test because OPENAI_API_KEY is missing");
        return;
    }

    let base_url =
        std::env::var("OPENAI_BASE_URL").unwrap_or_else(|_| "https://api.openai.com".to_string());
    let model = std::env::var("OPENAI_MODEL").unwrap_or_else(|_| "gpt-4o-mini".to_string());
    let key = std::env::var("OPENAI_API_KEY").expect("key checked");
    let provider =
        OpenAiCompatibleProvider::new(base_url, key).expect("provider initialization should work");

    let models = provider.list_models().await.expect("list_models");
    assert!(!models.is_empty(), "provider returned no models");

    let response = provider
        .chat(ChatRequest {
            model,
            temperature: 0.0,
            messages: vec![ChatMessage {
                role: ChatRole::User,
                content: "Reply with exactly: pong".to_string(),
            }],
        })
        .await
        .expect("chat");
    assert!(
        !response.content.trim().is_empty(),
        "chat response should not be empty"
    );
}
