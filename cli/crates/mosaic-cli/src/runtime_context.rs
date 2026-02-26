use std::sync::Arc;

use mosaic_agent::AgentRunner;
use mosaic_agents::{AgentStore, agent_routes_path, agents_file_path};
use mosaic_core::audit::AuditStore;
use mosaic_core::config::ConfigManager;
use mosaic_core::error::{MosaicError, Result};
use mosaic_core::models::{ModelProfileConfig, ModelRoutingStore};
use mosaic_core::provider::{ChatRequest, ChatResponse, ModelInfo, Provider, ProviderHealth};
use mosaic_core::session::SessionStore;
use mosaic_core::state::{StateMode, StatePaths};
use mosaic_ops::{ApprovalStore, RuntimePolicy, SandboxStore};
use mosaic_provider_openai::OpenAiCompatibleProvider;
use mosaic_tools::ToolExecutor;

use super::{Cli, PROJECT_STATE_DIR};

pub(super) struct RuntimeContext {
    pub(super) provider: Arc<dyn Provider>,
    pub(super) agent: AgentRunner,
    pub(super) active_agent_id: Option<String>,
    pub(super) active_profile_name: String,
}

pub(super) struct ModelRoutingProvider {
    inner: Arc<dyn Provider>,
    fallback_models: Vec<String>,
}

impl ModelRoutingProvider {
    pub(super) fn new(inner: Arc<dyn Provider>, fallback_models: Vec<String>) -> Self {
        Self {
            inner,
            fallback_models,
        }
    }
}

#[async_trait::async_trait]
impl Provider for ModelRoutingProvider {
    async fn list_models(&self) -> Result<Vec<ModelInfo>> {
        self.inner.list_models().await
    }

    async fn chat(&self, request: ChatRequest) -> Result<ChatResponse> {
        if self.fallback_models.is_empty() {
            return self.inner.chat(request).await;
        }

        let mut attempts = vec![request.model.clone()];
        for model in &self.fallback_models {
            if !attempts.iter().any(|candidate| candidate == model) {
                attempts.push(model.clone());
            }
        }

        let mut last_error: Option<MosaicError> = None;
        for model in &attempts {
            let mut retry_request = request.clone();
            retry_request.model = model.clone();
            match self.inner.chat(retry_request).await {
                Ok(response) => return Ok(response),
                Err(err) => {
                    if matches!(err, MosaicError::Auth(_)) {
                        return Err(err);
                    }
                    last_error = Some(err);
                }
            }
        }

        let chain = attempts.join(" -> ");
        Err(last_error
            .unwrap_or_else(|| MosaicError::Unknown("model fallback failed".to_string()))
            .with_context(format!("chat failed across model chain [{chain}]")))
    }

    async fn health(&self) -> Result<ProviderHealth> {
        self.inner.health().await
    }
}

pub(super) fn resolve_effective_model(
    profile_models: &ModelProfileConfig,
    requested_model: &str,
) -> (String, Option<String>) {
    let normalized_requested = requested_model.trim().to_ascii_lowercase();
    if let Some(target) = profile_models.aliases.get(&normalized_requested) {
        return (target.clone(), Some(normalized_requested));
    }
    (requested_model.trim().to_string(), None)
}

pub(super) fn resolve_state_paths(project_state: bool) -> Result<StatePaths> {
    let mode = if project_state {
        StateMode::Project
    } else {
        StateMode::Xdg
    };
    let cwd = std::env::current_dir().map_err(|err| MosaicError::Io(err.to_string()))?;
    StatePaths::resolve(mode, &cwd, PROJECT_STATE_DIR)
}

pub(super) fn build_runtime(
    cli: &Cli,
    requested_agent_id: Option<&str>,
    route_hint: Option<&str>,
) -> Result<RuntimeContext> {
    let state_paths = resolve_state_paths(cli.project_state)?;
    state_paths.ensure_dirs()?;
    let manager = ConfigManager::new(state_paths.config_path.clone());
    let config = manager.load()?;
    let agent_store = AgentStore::new(
        agents_file_path(&state_paths.data_dir),
        agent_routes_path(&state_paths.data_dir),
    );
    let mut resolved = agent_store.resolve_effective_profile(
        &config,
        &cli.profile,
        requested_agent_id,
        route_hint,
    )?;
    let model_store = ModelRoutingStore::new(state_paths.models_path.clone());
    let profile_models = model_store.profile(&resolved.profile_name)?;
    resolved.profile.provider.model =
        profile_models.resolve_model_ref(&resolved.profile.provider.model);
    let fallback_models = profile_models
        .fallbacks
        .iter()
        .map(|model| profile_models.resolve_model_ref(model))
        .filter(|model| model != &resolved.profile.provider.model)
        .fold(Vec::<String>::new(), |mut acc, model| {
            if !acc.iter().any(|item| item == &model) {
                acc.push(model);
            }
            acc
        });
    let mut provider: Arc<dyn Provider> =
        Arc::new(OpenAiCompatibleProvider::from_profile(&resolved.profile)?);
    if !fallback_models.is_empty() {
        provider = Arc::new(ModelRoutingProvider::new(provider, fallback_models));
    }
    let session_store = SessionStore::new(state_paths.sessions_dir.clone());
    let audit_store = AuditStore::new(
        state_paths.audit_dir.clone(),
        state_paths.audit_log_path.clone(),
    );
    let approval_store = ApprovalStore::new(state_paths.approvals_policy_path.clone());
    let sandbox_store = SandboxStore::new(state_paths.sandbox_policy_path.clone());
    let tool_executor = ToolExecutor::new(
        resolved.profile.tools.run.guard_mode.clone(),
        Some(RuntimePolicy {
            approval: approval_store.load_or_default()?,
            sandbox: sandbox_store.load_or_default()?,
        }),
    );
    let agent = AgentRunner::new(
        provider.clone(),
        resolved.profile.clone(),
        session_store,
        audit_store,
        tool_executor,
    );
    Ok(RuntimeContext {
        provider,
        agent,
        active_agent_id: resolved.agent_id,
        active_profile_name: resolved.profile_name,
    })
}
