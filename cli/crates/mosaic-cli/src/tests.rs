use super::*;
use std::collections::BTreeMap;
use std::collections::HashSet;
use std::sync::{Arc, Mutex};

struct StubProvider {
    network_fail_models: HashSet<String>,
    auth_fail_models: HashSet<String>,
    calls: Arc<Mutex<Vec<String>>>,
}

#[async_trait::async_trait]
impl Provider for StubProvider {
    async fn list_models(&self) -> Result<Vec<ModelInfo>> {
        Ok(Vec::new())
    }

    async fn chat(&self, request: ChatRequest) -> Result<ChatResponse> {
        self.calls
            .lock()
            .expect("calls lock")
            .push(request.model.clone());
        if self.auth_fail_models.contains(&request.model) {
            return Err(MosaicError::Auth("invalid api key".to_string()));
        }
        if self.network_fail_models.contains(&request.model) {
            return Err(MosaicError::Network("upstream unavailable".to_string()));
        }
        Ok(ChatResponse {
            content: request.model,
        })
    }

    async fn health(&self) -> Result<ProviderHealth> {
        Ok(ProviderHealth {
            ok: true,
            latency_ms: Some(1),
            detail: "ok".to_string(),
        })
    }
}

#[test]
fn cli_accepts_legacy_aliases() {
    let alias_ask = Cli::try_parse_from(["mosaic", "message", "hello"]).unwrap();
    assert!(matches!(alias_ask.command, Commands::Ask(_)));

    let alias_chat = Cli::try_parse_from(["mosaic", "agent"]).unwrap();
    assert!(matches!(alias_chat.command, Commands::Chat(_)));

    let alias_setup = Cli::try_parse_from(["mosaic", "onboard"]).unwrap();
    assert!(matches!(alias_setup.command, Commands::Setup(_)));

    let alias_config = Cli::try_parse_from(["mosaic", "config", "--show"]).unwrap();
    assert!(matches!(alias_config.command, Commands::Configure(_)));

    let alias_sessions = Cli::try_parse_from(["mosaic", "sessions", "list"]).unwrap();
    assert!(matches!(alias_sessions.command, Commands::Session(_)));

    let alias_daemon = Cli::try_parse_from(["mosaic", "daemon", "status"]).unwrap();
    assert!(matches!(alias_daemon.command, Commands::Gateway(_)));

    let alias_node = Cli::try_parse_from(["mosaic", "node", "list"]).unwrap();
    assert!(matches!(alias_node.command, Commands::Nodes(_)));

    let alias_acp = Cli::try_parse_from(["mosaic", "acp", "get"]).unwrap();
    assert!(matches!(alias_acp.command, Commands::Approvals(_)));
}

#[test]
fn resolve_effective_model_uses_alias_mapping() {
    let mut profile = ModelProfileConfig {
        aliases: BTreeMap::new(),
        fallbacks: vec![],
    };
    profile
        .aliases
        .insert("fast".to_string(), "gpt-4o-mini".to_string());
    let (effective, used_alias) = resolve_effective_model(&profile, "FAST");
    assert_eq!(effective, "gpt-4o-mini");
    assert_eq!(used_alias.as_deref(), Some("fast"));
}

#[tokio::test]
async fn model_routing_provider_falls_back_on_network_error() {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let provider = StubProvider {
        network_fail_models: ["primary".to_string()].into_iter().collect(),
        auth_fail_models: HashSet::new(),
        calls: calls.clone(),
    };
    let provider = ModelRoutingProvider::new(
        Arc::new(provider),
        vec!["backup".to_string(), "backup".to_string()],
    );

    let response = provider
        .chat(ChatRequest {
            model: "primary".to_string(),
            temperature: 0.2,
            messages: Vec::new(),
        })
        .await
        .expect("fallback succeeds");

    assert_eq!(response.content, "backup");
    assert_eq!(
        calls.lock().expect("calls lock").as_slice(),
        &["primary".to_string(), "backup".to_string()]
    );
}

#[tokio::test]
async fn model_routing_provider_does_not_retry_auth_error() {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let provider = StubProvider {
        network_fail_models: HashSet::new(),
        auth_fail_models: ["primary".to_string()].into_iter().collect(),
        calls: calls.clone(),
    };
    let provider = ModelRoutingProvider::new(Arc::new(provider), vec!["backup".to_string()]);

    let err = provider
        .chat(ChatRequest {
            model: "primary".to_string(),
            temperature: 0.2,
            messages: Vec::new(),
        })
        .await
        .expect_err("auth should fail without fallback");

    assert!(matches!(err, MosaicError::Auth(_)));
    assert_eq!(
        calls.lock().expect("calls lock").as_slice(),
        &["primary".to_string()]
    );
}
