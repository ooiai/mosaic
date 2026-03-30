use super::*;

pub fn validate_mosaic_config(config: &MosaicConfig) -> ValidationReport {
    let mut report = ValidationReport::default();

    validate_transport_policy(
        &mut report,
        "provider_defaults",
        &config.provider_defaults,
        None,
    );

    if config.schema_version != CURRENT_SCHEMA_VERSION {
        report.push(
            ValidationLevel::Error,
            "schema_version",
            format!(
                "unsupported schema_version {}; expected {}",
                config.schema_version, CURRENT_SCHEMA_VERSION
            ),
        );
    }

    if config.profiles.is_empty() {
        report.push(
            ValidationLevel::Error,
            "profiles",
            "at least one provider profile must be configured",
        );
    }

    if config.deployment.profile.trim().is_empty() {
        report.push(
            ValidationLevel::Error,
            "deployment.profile",
            "deployment profile must not be empty",
        );
    } else if !matches!(
        config.deployment.profile.as_str(),
        "local" | "staging" | "production"
    ) {
        report.push(
            ValidationLevel::Error,
            "deployment.profile",
            format!(
                "unsupported deployment profile '{}': expected local, staging, or production",
                config.deployment.profile
            ),
        );
    }

    if config.deployment.workspace_name.trim().is_empty() {
        report.push(
            ValidationLevel::Error,
            "deployment.workspace_name",
            "deployment workspace_name must not be empty",
        );
    }

    for (field, value) in [
        (
            "auth.operator_token_env",
            config.auth.operator_token_env.as_deref(),
        ),
        (
            "auth.webchat_shared_secret_env",
            config.auth.webchat_shared_secret_env.as_deref(),
        ),
        (
            "auth.telegram_secret_token_env",
            config.auth.telegram_secret_token_env.as_deref(),
        ),
    ] {
        if value.is_some_and(|value| value.trim().is_empty()) {
            report.push(
                ValidationLevel::Error,
                field,
                "environment variable name must not be empty when provided",
            );
        }
    }

    for (name, bot) in &config.telegram.bots {
        let field_prefix = format!("telegram.bots.{name}");
        if name.trim().is_empty() {
            report.push(
                ValidationLevel::Error,
                field_prefix.clone(),
                "telegram bot name must not be empty",
            );
        }
        if bot.bot_token_env.trim().is_empty() {
            report.push(
                ValidationLevel::Error,
                format!("{field_prefix}.bot_token_env"),
                "bot_token_env must not be empty",
            );
        }
        if bot
            .webhook_secret_token_env
            .as_deref()
            .is_some_and(|value| value.trim().is_empty())
        {
            report.push(
                ValidationLevel::Error,
                format!("{field_prefix}.webhook_secret_token_env"),
                "environment variable name must not be empty when provided",
            );
        }
        if bot
            .route_key
            .as_deref()
            .is_some_and(|value| value.trim().is_empty())
        {
            report.push(
                ValidationLevel::Error,
                format!("{field_prefix}.route_key"),
                "route_key must not be empty when provided",
            );
        }
        if let Some(path) = bot
            .webhook_path
            .as_deref()
            .map(str::trim)
            .filter(|v| !v.is_empty())
        {
            if !path.starts_with('/') {
                report.push(
                    ValidationLevel::Error,
                    format!("{field_prefix}.webhook_path"),
                    "webhook_path must start with '/'",
                );
            }
        }
        if let Some(default_profile) = bot
            .default_profile
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            if !config.profiles.contains_key(default_profile) {
                report.push(
                    ValidationLevel::Error,
                    format!("{field_prefix}.default_profile"),
                    format!(
                        "telegram bot '{}' default_profile '{}' does not match any configured profile",
                        name, default_profile
                    ),
                );
            }
        }
    }

    if !config.profiles.contains_key(&config.active_profile) {
        report.push(
            ValidationLevel::Error,
            "active_profile",
            format!(
                "active_profile '{}' does not match any configured profile",
                config.active_profile
            ),
        );
    }

    for (name, profile) in &config.profiles {
        let field_prefix = format!("profiles.{name}");

        if profile.provider_type.trim().is_empty() {
            report.push(
                ValidationLevel::Error,
                format!("{field_prefix}.type"),
                "provider type must not be empty",
            );
            continue;
        }

        let provider_type = match parse_provider_type(&profile.provider_type) {
            Some(provider_type) => provider_type,
            None => {
                report.push(
                    ValidationLevel::Error,
                    format!("{field_prefix}.type"),
                    format!(
                        "unsupported provider type '{}': expected one of {}",
                        profile.provider_type,
                        supported_provider_types().join(", ")
                    ),
                );
                continue;
            }
        };

        if profile.model.trim().is_empty() {
            report.push(
                ValidationLevel::Error,
                format!("{field_prefix}.model"),
                "model must not be empty",
            );
        }

        if profile
            .base_url
            .as_deref()
            .is_some_and(|value| value.trim().is_empty())
        {
            report.push(
                ValidationLevel::Error,
                format!("{field_prefix}.base_url"),
                "base_url must not be empty when provided",
            );
        }

        if profile
            .api_key_env
            .as_deref()
            .is_some_and(|value| value.trim().is_empty())
        {
            report.push(
                ValidationLevel::Error,
                format!("{field_prefix}.api_key_env"),
                "environment variable name must not be empty when provided",
            );
        }

        if provider_type.requires_explicit_base_url()
            && profile
                .base_url
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .is_none()
        {
            report.push(
                ValidationLevel::Error,
                format!("{field_prefix}.base_url"),
                format!("{} profiles require base_url", provider_type),
            );
        }

        if provider_type.requires_api_key()
            && profile
                .api_key_env
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .is_none()
        {
            report.push(
                ValidationLevel::Error,
                format!("{field_prefix}.api_key_env"),
                format!("{} profiles require api_key_env", provider_type),
            );
        }

        validate_transport_policy(
            &mut report,
            &format!("{field_prefix}.transport"),
            &profile.transport,
            Some(&profile.vendor),
        );

        validate_vendor_policy(
            &mut report,
            &field_prefix,
            provider_type,
            &profile.vendor,
            &profile.transport,
        );
    }

    if config.session_store.root_dir.trim().is_empty() {
        report.push(
            ValidationLevel::Error,
            "session_store.root_dir",
            "session store root directory must not be empty",
        );
    }

    if config.inspect.runs_dir.trim().is_empty() {
        report.push(
            ValidationLevel::Error,
            "inspect.runs_dir",
            "inspect runs directory must not be empty",
        );
    }

    if config.audit.root_dir.trim().is_empty() {
        report.push(
            ValidationLevel::Error,
            "audit.root_dir",
            "audit root directory must not be empty",
        );
    }

    if config.audit.retention_days == 0 {
        report.push(
            ValidationLevel::Error,
            "audit.retention_days",
            "audit retention_days must be greater than zero",
        );
    }

    if config.audit.event_replay_window == 0 {
        report.push(
            ValidationLevel::Error,
            "audit.event_replay_window",
            "audit event_replay_window must be greater than zero",
        );
    }

    if config.observability.slow_consumer_lag_threshold == 0 {
        report.push(
            ValidationLevel::Error,
            "observability.slow_consumer_lag_threshold",
            "observability slow_consumer_lag_threshold must be greater than zero",
        );
    }

    if config.runtime.max_provider_round_trips == 0 {
        report.push(
            ValidationLevel::Error,
            "runtime.max_provider_round_trips",
            "runtime max_provider_round_trips must be greater than zero",
        );
    }

    if config.runtime.max_workflow_provider_round_trips == 0 {
        report.push(
            ValidationLevel::Error,
            "runtime.max_workflow_provider_round_trips",
            "runtime max_workflow_provider_round_trips must be greater than zero",
        );
    }

    for (idx, tool) in config.tools.iter().enumerate() {
        if tool.name.trim().is_empty() {
            report.push(
                ValidationLevel::Error,
                format!("tools.{idx}.name"),
                "tool name must not be empty",
            );
        }
        if tool.tool_type.trim().is_empty() {
            report.push(
                ValidationLevel::Error,
                format!("tools.{idx}.type"),
                "tool type must not be empty",
            );
        }
        if tool
            .allowed_channels
            .iter()
            .any(|channel| channel.trim().is_empty())
        {
            report.push(
                ValidationLevel::Error,
                format!("tools.{idx}.allowed_channels"),
                "allowed_channels entries must not be empty",
            );
        }
    }

    for (idx, skill) in config.skills.iter().enumerate() {
        if skill.name.trim().is_empty() {
            report.push(
                ValidationLevel::Error,
                format!("skills.{idx}.name"),
                "skill name must not be empty",
            );
        }
        if skill.skill_type.trim().is_empty() {
            report.push(
                ValidationLevel::Error,
                format!("skills.{idx}.type"),
                "skill type must not be empty",
            );
        }
        if skill
            .allowed_channels
            .iter()
            .any(|channel| channel.trim().is_empty())
        {
            report.push(
                ValidationLevel::Error,
                format!("skills.{idx}.allowed_channels"),
                "allowed_channels entries must not be empty",
            );
        }
    }

    for (idx, workflow) in config.workflows.iter().enumerate() {
        if workflow.name.trim().is_empty() {
            report.push(
                ValidationLevel::Error,
                format!("workflows.{idx}.name"),
                "workflow name must not be empty",
            );
        }
        if workflow
            .visibility
            .allowed_channels
            .iter()
            .any(|channel| channel.trim().is_empty())
        {
            report.push(
                ValidationLevel::Error,
                format!("workflows.{idx}.allowed_channels"),
                "allowed_channels entries must not be empty",
            );
        }
    }

    if config.deployment.profile == "production"
        && config
            .auth
            .operator_token_env
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .is_none()
    {
        report.push(
            ValidationLevel::Error,
            "auth.operator_token_env",
            "production deployment requires auth.operator_token_env",
        );
    }

    if config.deployment.profile == "production" && !config.audit.redact_inputs {
        report.push(
            ValidationLevel::Warning,
            "audit.redact_inputs",
            "production deployment should keep audit.redact_inputs enabled",
        );
    }

    for (idx, manifest) in config.extensions.manifests.iter().enumerate() {
        if manifest.path.trim().is_empty() {
            report.push(
                ValidationLevel::Error,
                format!("extensions.manifests.{idx}.path"),
                "extension manifest path must not be empty",
            );
        }

        if manifest
            .version_pin
            .as_deref()
            .is_some_and(|version| version.trim().is_empty())
        {
            report.push(
                ValidationLevel::Error,
                format!("extensions.manifests.{idx}.version_pin"),
                "extension manifest version_pin must not be empty when provided",
            );
        }
    }

    report
}

fn validate_transport_policy(
    report: &mut ValidationReport,
    field_prefix: &str,
    transport: &ProviderTransportPolicyConfig,
    vendor: Option<&ProviderVendorPolicyConfig>,
) {
    if transport.timeout_ms == Some(0) {
        report.push(
            ValidationLevel::Error,
            format!("{field_prefix}.timeout_ms"),
            "timeout_ms must be greater than zero when provided",
        );
    }

    if transport.retry_backoff_ms == Some(0) {
        report.push(
            ValidationLevel::Error,
            format!("{field_prefix}.retry_backoff_ms"),
            "retry_backoff_ms must be greater than zero when provided",
        );
    }

    for (header_name, header_value) in &transport.custom_headers {
        if header_name.trim().is_empty() {
            report.push(
                ValidationLevel::Error,
                format!("{field_prefix}.custom_headers"),
                "custom header names must not be empty",
            );
        }

        if header_value.trim().is_empty() {
            report.push(
                ValidationLevel::Error,
                format!("{field_prefix}.custom_headers.{header_name}"),
                "custom header values must not be empty",
            );
        }

        if matches!(
            header_name.to_ascii_lowercase().as_str(),
            "authorization" | "api-key"
        ) {
            report.push(
                ValidationLevel::Error,
                format!("{field_prefix}.custom_headers.{header_name}"),
                "custom headers must not override authorization or api-key",
            );
        }
    }

    if !transport.custom_headers.is_empty()
        && !vendor.is_some_and(|vendor| vendor.allow_custom_headers)
    {
        report.push(
            ValidationLevel::Error,
            format!("{field_prefix}.custom_headers"),
            "custom headers require vendor.allow_custom_headers=true",
        );
    }
}

fn validate_vendor_policy(
    report: &mut ValidationReport,
    field_prefix: &str,
    provider_type: ProviderType,
    vendor: &ProviderVendorPolicyConfig,
    transport: &ProviderTransportPolicyConfig,
) {
    if vendor
        .azure_api_version
        .as_deref()
        .is_some_and(|value| value.trim().is_empty())
    {
        report.push(
            ValidationLevel::Error,
            format!("{field_prefix}.vendor.azure_api_version"),
            "azure_api_version must not be empty when provided",
        );
    }

    if vendor
        .anthropic_version
        .as_deref()
        .is_some_and(|value| value.trim().is_empty())
    {
        report.push(
            ValidationLevel::Error,
            format!("{field_prefix}.vendor.anthropic_version"),
            "anthropic_version must not be empty when provided",
        );
    }

    if vendor.azure_api_version.is_some() && provider_type != ProviderType::Azure {
        report.push(
            ValidationLevel::Error,
            format!("{field_prefix}.vendor.azure_api_version"),
            "azure_api_version is only valid for azure profiles",
        );
    }

    if vendor.anthropic_version.is_some() && provider_type != ProviderType::Anthropic {
        report.push(
            ValidationLevel::Error,
            format!("{field_prefix}.vendor.anthropic_version"),
            "anthropic_version is only valid for anthropic profiles",
        );
    }

    if vendor.allow_custom_headers && transport.custom_headers.is_empty() {
        report.push(
            ValidationLevel::Warning,
            format!("{field_prefix}.vendor.allow_custom_headers"),
            "allow_custom_headers is enabled but no custom headers are configured",
        );
    }
}
