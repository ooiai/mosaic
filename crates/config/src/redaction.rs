use super::*;

pub fn redact_mosaic_config(config: &MosaicConfig) -> RedactedMosaicConfig {
    let profiles = config
        .profiles
        .iter()
        .map(|(name, profile)| RedactedProfileView {
            name: name.clone(),
            provider_type: profile.provider_type.clone(),
            usage: parse_provider_type(&profile.provider_type)
                .map(ProviderType::usage)
                .unwrap_or(ProviderUsage::Compatibility),
            model: profile.model.clone(),
            base_url: profile.base_url.clone(),
            api_key_env: profile.api_key_env.clone(),
            api_key_present: profile
                .api_key_env
                .as_deref()
                .is_some_and(|env_var| env::var(env_var).is_ok()),
            timeout_ms: profile.transport.timeout_ms,
            max_retries: profile.transport.max_retries,
            retry_backoff_ms: profile.transport.retry_backoff_ms,
            custom_header_keys: profile.transport.custom_headers.keys().cloned().collect(),
            allow_custom_headers: profile.vendor.allow_custom_headers,
            azure_api_version: profile.vendor.azure_api_version.clone(),
            anthropic_version: profile.vendor.anthropic_version.clone(),
        })
        .collect();

    RedactedMosaicConfig {
        schema_version: config.schema_version,
        active_profile: config.active_profile.clone(),
        profiles,
        provider_defaults: RedactedProviderDefaultsView {
            timeout_ms: config.provider_defaults.timeout_ms,
            max_retries: config.provider_defaults.max_retries,
            retry_backoff_ms: config.provider_defaults.retry_backoff_ms,
        },
        deployment: RedactedDeploymentView {
            profile: config.deployment.profile.clone(),
            workspace_name: config.deployment.workspace_name.clone(),
        },
        auth: RedactedAuthView {
            operator_token_env: config.auth.operator_token_env.clone(),
            operator_token_present: env_var_present(config.auth.operator_token_env.as_deref()),
            webchat_shared_secret_env: config.auth.webchat_shared_secret_env.clone(),
            webchat_shared_secret_present: env_var_present(
                config.auth.webchat_shared_secret_env.as_deref(),
            ),
            telegram_secret_token_env: config.auth.telegram_secret_token_env.clone(),
            telegram_secret_token_present: env_var_present(
                config.auth.telegram_secret_token_env.as_deref(),
            ),
        },
        session_store_root_dir: config.session_store.root_dir.clone(),
        inspect_runs_dir: config.inspect.runs_dir.clone(),
        audit: RedactedAuditView {
            root_dir: config.audit.root_dir.clone(),
            retention_days: config.audit.retention_days,
            event_replay_window: config.audit.event_replay_window,
            redact_inputs: config.audit.redact_inputs,
        },
        observability: RedactedObservabilityView {
            enable_metrics: config.observability.enable_metrics,
            enable_readiness: config.observability.enable_readiness,
            slow_consumer_lag_threshold: config.observability.slow_consumer_lag_threshold,
        },
        runtime: RedactedRuntimePolicyView {
            max_provider_round_trips: config.runtime.max_provider_round_trips,
            max_workflow_provider_round_trips: config.runtime.max_workflow_provider_round_trips,
            continue_after_tool_error: config.runtime.continue_after_tool_error,
        },
        attachments: RedactedAttachmentView {
            enabled: config.attachments.policy.enabled,
            cache_dir: config.attachments.policy.cache_dir.clone(),
            max_size_bytes: config.attachments.policy.max_size_bytes,
            download_timeout_ms: config.attachments.policy.download_timeout_ms,
            cleanup_after_hours: config.attachments.policy.cleanup_after_hours,
            allowed_mime_types: config.attachments.policy.allowed_mime_types.clone(),
            default_route_mode: config.attachments.routing.default.mode,
        },
        extension_manifest_count: config.extensions.manifests.len(),
        policies: RedactedPolicyView {
            allow_exec: config.policies.allow_exec,
            allow_webhook: config.policies.allow_webhook,
            allow_cron: config.policies.allow_cron,
            allow_mcp: config.policies.allow_mcp,
            hot_reload_enabled: config.policies.hot_reload_enabled,
        },
    }
}
