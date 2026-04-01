use super::*;

pub fn redact_mosaic_config(config: &MosaicConfig) -> RedactedMosaicConfig {
    let profiles = config
        .profiles
        .iter()
        .map(|(name, profile)| {
            let capabilities = infer_profile_capabilities(profile);
            RedactedProfileView {
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
                supports_vision: capabilities.supports_vision,
                supports_documents: capabilities.supports_documents,
                supports_audio: capabilities.supports_audio,
                supports_video: capabilities.supports_video,
                preferred_attachment_mode: capabilities.preferred_attachment_mode,
            }
        })
        .collect();
    let mut telegram_bots = config
        .telegram
        .bots
        .iter()
        .map(|(name, bot)| RedactedTelegramBotView {
            name: name.clone(),
            enabled: bot.enabled,
            route_key: bot.route_key(name),
            webhook_path: bot.webhook_path(name),
            bot_token_env: bot.bot_token_env.clone(),
            bot_token_present: env_var_present(Some(bot.bot_token_env.as_str())),
            webhook_secret_token_env: bot.webhook_secret_token_env.clone(),
            webhook_secret_token_present: env_var_present(bot.webhook_secret_token_env.as_deref()),
            default_profile: bot.default_profile.clone(),
            allowed_tools: bot.allowed_tools.clone(),
            allowed_skills: bot.allowed_skills.clone(),
            allowed_workflows: bot.allowed_workflows.clone(),
            attachments: bot
                .attachments
                .as_ref()
                .map(|route| redacted_attachment_route_view(&format!("bot:{name}"), route)),
        })
        .collect::<Vec<_>>();
    telegram_bots.sort_by(|left, right| left.name.cmp(&right.name));

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
        sandbox: RedactedSandboxView {
            base_dir: config.sandbox.base_dir.clone(),
            python_strategy: config.sandbox.python.strategy,
            node_strategy: config.sandbox.node.strategy,
            run_workdirs_after_hours: config.sandbox.cleanup.run_workdirs_after_hours,
            attachments_after_hours: config.sandbox.cleanup.attachments_after_hours,
        },
        attachments: RedactedAttachmentView {
            enabled: config.attachments.policy.enabled,
            cache_dir: config.attachments.policy.cache_dir.clone(),
            max_size_bytes: config.attachments.policy.max_size_bytes,
            download_timeout_ms: config.attachments.policy.download_timeout_ms,
            cleanup_after_hours: config.attachments.policy.cleanup_after_hours,
            allowed_mime_types: config.attachments.policy.allowed_mime_types.clone(),
            default_route_mode: config.attachments.routing.default.mode,
            default_processor: config.attachments.routing.default.processor.clone(),
            default_multimodal_profile: config
                .attachments
                .routing
                .default
                .multimodal_profile
                .clone(),
            default_specialized_processor_profile: config
                .attachments
                .routing
                .default
                .specialized_processor_profile
                .clone(),
            default_allowed_attachment_kinds: config
                .attachments
                .routing
                .default
                .allowed_attachment_kinds
                .clone(),
            default_max_attachment_size_mb: config
                .attachments
                .routing
                .default
                .max_attachment_size_mb,
            channel_overrides: config
                .attachments
                .routing
                .channel_overrides
                .iter()
                .map(|(scope, route)| {
                    redacted_attachment_route_view(&format!("channel:{scope}"), route)
                })
                .collect(),
            bot_overrides: config
                .attachments
                .routing
                .bot_overrides
                .iter()
                .map(|(scope, route)| {
                    redacted_attachment_route_view(&format!("bot:{scope}"), route)
                })
                .collect(),
        },
        telegram_bots,
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

fn infer_profile_capabilities(profile: &ProviderProfileConfig) -> InferredAttachmentCapabilities {
    let provider_type = parse_provider_type(&profile.provider_type);
    let normalized = profile.model.to_ascii_lowercase();
    let supports_vision = if profile.model == "mock" {
        true
    } else if normalized.contains("vision") || normalized.contains("llava") {
        true
    } else {
        match provider_type {
            Some(ProviderType::OpenAi | ProviderType::Azure | ProviderType::OpenAiCompatible) => {
                normalized.starts_with("gpt-")
            }
            Some(ProviderType::Anthropic) => normalized.starts_with("claude"),
            Some(ProviderType::Ollama) => {
                normalized.contains("vision") || normalized.contains("llava")
            }
            Some(ProviderType::Mock) => true,
            None => false,
        }
    };
    let supports_documents = if profile.model == "mock" {
        true
    } else if normalized.contains("document") || normalized.contains("pdf") {
        true
    } else {
        match provider_type {
            Some(ProviderType::OpenAi | ProviderType::Azure | ProviderType::OpenAiCompatible) => {
                normalized.starts_with("gpt-")
            }
            Some(ProviderType::Anthropic) => normalized.starts_with("claude"),
            Some(ProviderType::Mock) => true,
            Some(ProviderType::Ollama) | None => false,
        }
    };

    InferredAttachmentCapabilities {
        supports_vision,
        supports_documents,
        supports_audio: if profile.model == "mock" {
            false
        } else {
            normalized.contains("audio")
                && !matches!(provider_type, Some(ProviderType::Ollama) | None)
        },
        supports_video: if profile.model == "mock" {
            false
        } else {
            normalized.contains("video")
                && !matches!(provider_type, Some(ProviderType::Ollama) | None)
        },
        preferred_attachment_mode: if supports_vision || supports_documents {
            AttachmentRouteModeConfig::ProviderNative
        } else {
            AttachmentRouteModeConfig::SpecializedProcessor
        },
    }
}

fn redacted_attachment_route_view(
    scope: &str,
    route: &AttachmentRoutingTargetConfig,
) -> RedactedAttachmentRouteView {
    RedactedAttachmentRouteView {
        scope: scope.to_owned(),
        mode: route.mode,
        processor: route.processor.clone(),
        multimodal_profile: route.multimodal_profile.clone(),
        specialized_processor_profile: route.specialized_processor_profile.clone(),
        allowed_attachment_kinds: route.allowed_attachment_kinds.clone(),
        max_attachment_size_mb: route.max_attachment_size_mb,
    }
}

struct InferredAttachmentCapabilities {
    supports_vision: bool,
    supports_documents: bool,
    supports_audio: bool,
    supports_video: bool,
    preferred_attachment_mode: AttachmentRouteModeConfig,
}
