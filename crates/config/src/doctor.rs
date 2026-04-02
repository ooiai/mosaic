use super::*;
use mosaic_sandbox_core::{SandboxCleanupPolicy, SandboxManager, SandboxSettings};

pub fn doctor_mosaic_config(config: &MosaicConfig, cwd: impl AsRef<Path>) -> DoctorReport {
    let cwd = cwd.as_ref();
    let validation = validate_mosaic_config(config);
    let mut checks = Vec::new();

    let session_root = cwd.join(&config.session_store.root_dir);
    checks.push(path_check(
        &session_root,
        DoctorCategory::Storage,
        "session store directory",
        true,
    ));

    let runs_root = cwd.join(&config.inspect.runs_dir);
    checks.push(path_check(
        &runs_root,
        DoctorCategory::Storage,
        "run trace directory",
        true,
    ));

    let audit_root = cwd.join(&config.audit.root_dir);
    checks.push(path_check(
        &audit_root,
        DoctorCategory::Storage,
        "audit directory",
        true,
    ));

    let sandbox = SandboxManager::new(
        cwd,
        SandboxSettings {
            base_dir: PathBuf::from(&config.sandbox.base_dir),
            python: mosaic_sandbox_core::PythonSandboxSettings {
                strategy: config.sandbox.python.strategy,
                install: config.sandbox.python.install.clone(),
            },
            node: mosaic_sandbox_core::NodeSandboxSettings {
                strategy: config.sandbox.node.strategy,
                install: config.sandbox.node.install.clone(),
            },
            cleanup: SandboxCleanupPolicy {
                run_workdirs_after_hours: config.sandbox.cleanup.run_workdirs_after_hours,
                attachments_after_hours: config.sandbox.cleanup.attachments_after_hours,
            },
        },
    );
    let sandbox_paths = sandbox.paths();
    checks.push(path_check(
        &sandbox_paths.root,
        DoctorCategory::Sandbox,
        "sandbox root directory",
        true,
    ));
    checks.push(path_check(
        &sandbox_paths.work_runs,
        DoctorCategory::Sandbox,
        "sandbox run work directory",
        true,
    ));
    checks.push(path_check(
        &sandbox_paths.attachments,
        DoctorCategory::Sandbox,
        "sandbox attachment directory",
        true,
    ));
    for status in sandbox.runtime_statuses() {
        checks.push(DoctorCheck {
            status: if status.available {
                DoctorStatus::Ok
            } else {
                DoctorStatus::Warning
            },
            category: DoctorCategory::Sandbox,
            message: format!(
                "sandbox runtime kind={} strategy={} available={} detail={}",
                status.kind.label(),
                status.strategy,
                status.available,
                status.detail.unwrap_or_else(|| "<none>".to_owned()),
            ),
        });
    }
    let env_count = sandbox
        .list_envs()
        .map(|envs| envs.len())
        .unwrap_or_default();
    checks.push(DoctorCheck {
        status: DoctorStatus::Ok,
        category: DoctorCategory::Sandbox,
        message: format!(
            "sandbox layout base_dir={} env_count={} cleanup(run_workdirs_after_hours={}, attachments_after_hours={}) python_install(enabled={} timeout_ms={} retry_limit={} allowed_sources={}) node_install(enabled={} timeout_ms={} retry_limit={} allowed_sources={})",
            sandbox_paths.root.display(),
            env_count,
            config.sandbox.cleanup.run_workdirs_after_hours,
            config.sandbox.cleanup.attachments_after_hours,
            config.sandbox.python.install.enabled,
            config.sandbox.python.install.timeout_ms,
            config.sandbox.python.install.retry_limit,
            if config.sandbox.python.install.allowed_sources.is_empty() {
                "<none>".to_owned()
            } else {
                config
                    .sandbox
                    .python
                    .install
                    .allowed_sources
                    .iter()
                    .map(|source| source.label().to_owned())
                    .collect::<Vec<_>>()
                    .join(",")
            },
            config.sandbox.node.install.enabled,
            config.sandbox.node.install.timeout_ms,
            config.sandbox.node.install.retry_limit,
            if config.sandbox.node.install.allowed_sources.is_empty() {
                "<none>".to_owned()
            } else {
                config
                    .sandbox
                    .node
                    .install
                    .allowed_sources
                    .iter()
                    .map(|source| source.label().to_owned())
                    .collect::<Vec<_>>()
                    .join(",")
            },
        ),
    });

    checks.push(secret_env_check(
        DoctorCategory::Auth,
        config.auth.operator_token_env.as_deref(),
        "operator auth token",
        config.deployment.profile == "production",
    ));
    checks.push(secret_env_check(
        DoctorCategory::Auth,
        config.auth.webchat_shared_secret_env.as_deref(),
        "webchat ingress shared secret",
        false,
    ));
    checks.push(secret_env_check(
        DoctorCategory::Auth,
        config.auth.telegram_secret_token_env.as_deref(),
        "telegram ingress secret token",
        false,
    ));
    for (name, bot) in &config.telegram.bots {
        checks.push(secret_env_check(
            DoctorCategory::Auth,
            Some(bot.bot_token_env.as_str()),
            &format!("telegram bot '{}' token", name),
            bot.enabled,
        ));
        checks.push(secret_env_check(
            DoctorCategory::Auth,
            bot.webhook_secret_token_env.as_deref(),
            &format!("telegram bot '{}' webhook secret", name),
            false,
        ));
        checks.push(DoctorCheck {
            status: if bot.enabled {
                DoctorStatus::Ok
            } else {
                DoctorStatus::Warning
            },
            category: DoctorCategory::Gateway,
            message: format!(
                "telegram bot '{}' route={} profile={} attachments={}",
                name,
                bot.route_key(name),
                bot.default_profile
                    .as_deref()
                    .unwrap_or(&config.active_profile),
                bot.attachments
                    .as_ref()
                    .map(|attachments| attachment_policy_summary(attachments))
                    .unwrap_or_else(|| "workspace default".to_owned()),
            ),
        });
        checks.push(DoctorCheck {
            status: DoctorStatus::Ok,
            category: DoctorCategory::Gateway,
            message: format!(
                "telegram bot '{}' capability scope tools={} skills={} workflows={}",
                name,
                scope_summary(&bot.allowed_tools),
                scope_summary(&bot.allowed_skills),
                scope_summary(&bot.allowed_workflows),
            ),
        });
    }

    for manifest in &config.extensions.manifests {
        let manifest_path = cwd.join(&manifest.path);
        checks.push(path_check(
            &manifest_path,
            DoctorCategory::Extensions,
            "extension manifest",
            false,
        ));
    }

    for skill in &config.skills {
        if skill.skill_type != "markdown_pack" {
            continue;
        }
        let Some(path) = skill
            .path
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        else {
            continue;
        };
        let skill_path = cwd.join(path);
        checks.push(path_check(
            &skill_path,
            DoctorCategory::Extensions,
            &format!("markdown skill pack '{}'", skill.name),
            false,
        ));
        checks.push(path_check(
            &skill_path.join("SKILL.md"),
            DoctorCategory::Extensions,
            &format!("markdown skill pack '{}' SKILL.md", skill.name),
            false,
        ));
    }

    for (name, profile) in &config.profiles {
        let Some(provider_type) = parse_provider_type(&profile.provider_type) else {
            checks.push(DoctorCheck {
                status: DoctorStatus::Error,
                category: DoctorCategory::Providers,
                message: format!(
                    "profile '{}' uses unsupported provider type '{}'",
                    name, profile.provider_type
                ),
            });
            continue;
        };

        match provider_type {
            ProviderType::Mock => checks.push(DoctorCheck {
                status: DoctorStatus::Error,
                category: DoctorCategory::Providers,
                message: format!(
                    "profile '{}' uses test-only provider type 'mock'; replace it with openai, azure, anthropic, ollama, or openai-compatible",
                    name,
                ),
            }),
            _ => {
                let configured_base_url = profile
                    .base_url
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty());
                match configured_base_url.or_else(|| provider_type.default_base_url()) {
                    Some(base_url) => checks.push(DoctorCheck {
                        status: DoctorStatus::Ok,
                        category: DoctorCategory::Providers,
                        message: if configured_base_url.is_some() {
                            format!(
                                "profile '{}' uses {} base URL {}",
                                name, provider_type, base_url
                            )
                        } else {
                            format!(
                                "profile '{}' defaults to {} base URL {}",
                                name, provider_type, base_url
                            )
                        },
                    }),
                    None => checks.push(DoctorCheck {
                        status: DoctorStatus::Error,
                        category: DoctorCategory::Providers,
                        message: format!(
                            "profile '{}' requires an explicit {} base_url",
                            name, provider_type
                        ),
                    }),
                }

                if provider_type.requires_api_key() {
                    match profile
                        .api_key_env
                        .as_deref()
                        .map(str::trim)
                        .filter(|value| !value.is_empty())
                    {
                        Some(api_key_env) if env::var(api_key_env).is_ok() => {
                            checks.push(DoctorCheck {
                                status: DoctorStatus::Ok,
                                category: DoctorCategory::Providers,
                                message: format!(
                                    "profile '{}' has {} available in the environment",
                                    name, api_key_env
                                ),
                            })
                        }
                        Some(api_key_env) => checks.push(DoctorCheck {
                            status: if name == &config.active_profile {
                                DoctorStatus::Error
                            } else {
                                DoctorStatus::Warning
                            },
                            category: DoctorCategory::Providers,
                            message: format!(
                                "profile '{}' expects environment variable {} to be set",
                                name, api_key_env
                            ),
                        }),
                        None => checks.push(DoctorCheck {
                            status: DoctorStatus::Error,
                            category: DoctorCategory::Providers,
                            message: format!(
                                "profile '{}' is missing api_key_env for {}",
                                name, provider_type
                            ),
                        }),
                    }
                } else {
                    checks.push(DoctorCheck {
                        status: DoctorStatus::Ok,
                        category: DoctorCategory::Providers,
                        message: format!("profile '{}' does not require API credentials", name),
                    });
                }
            }
        }

        let capabilities = infer_profile_capabilities(&profile.provider_type, &profile.model);
        checks.push(DoctorCheck {
            status: DoctorStatus::Ok,
            category: DoctorCategory::Providers,
            message: format!(
                "profile '{}' multimodal capabilities vision={} documents={} audio={} video={} preferred_attachment_mode={}",
                name,
                capabilities.supports_vision,
                capabilities.supports_documents,
                capabilities.supports_audio,
                capabilities.supports_video,
                capabilities.preferred_attachment_mode.label(),
            ),
        });
    }

    checks.push(DoctorCheck {
        status: if config.attachments.policy.enabled {
            DoctorStatus::Ok
        } else {
            DoctorStatus::Warning
        },
        category: DoctorCategory::Gateway,
        message: format!(
            "workspace attachment routing default={} processor={} multimodal_profile={} specialized_processor_profile={} kinds={} max_attachment_size_mb={}",
            config.attachments.routing.default.mode.label(),
            option_string(config.attachments.routing.default.processor.clone()),
            option_string(config.attachments.routing.default.multimodal_profile.clone()),
            option_string(
                config
                    .attachments
                    .routing
                    .default
                    .specialized_processor_profile
                    .clone(),
            ),
            if config
                .attachments
                .routing
                .default
                .allowed_attachment_kinds
                .is_empty()
            {
                "<all>".to_owned()
            } else {
                config
                    .attachments
                    .routing
                    .default
                    .allowed_attachment_kinds
                    .iter()
                    .map(|kind| kind.label())
                    .collect::<Vec<_>>()
                    .join(", ")
            },
            option_u64(config.attachments.routing.default.max_attachment_size_mb),
        ),
    });

    DoctorReport { validation, checks }
}

pub(crate) fn env_var_present(name: Option<&str>) -> bool {
    name.map(str::trim)
        .filter(|value| !value.is_empty())
        .is_some_and(|value| env::var(value).is_ok())
}

fn secret_env_check(
    category: DoctorCategory,
    env_name: Option<&str>,
    label: &str,
    required: bool,
) -> DoctorCheck {
    match env_name.map(str::trim).filter(|value| !value.is_empty()) {
        Some(name) => {
            if env::var(name).is_ok() {
                DoctorCheck {
                    status: DoctorStatus::Ok,
                    category,
                    message: format!("{label} is configured via {name}"),
                }
            } else {
                DoctorCheck {
                    status: if required {
                        DoctorStatus::Error
                    } else {
                        DoctorStatus::Warning
                    },
                    category,
                    message: format!("{label} expects environment variable {name} to be set"),
                }
            }
        }
        None => DoctorCheck {
            status: if required {
                DoctorStatus::Error
            } else {
                DoctorStatus::Warning
            },
            category,
            message: if required {
                format!("{label} is not configured")
            } else {
                format!("{label} is not configured; this surface is currently unauthenticated")
            },
        },
    }
}

fn path_check(
    path: &Path,
    category: DoctorCategory,
    label: &str,
    create_if_missing: bool,
) -> DoctorCheck {
    if path.exists() {
        if path.is_dir() {
            DoctorCheck {
                status: DoctorStatus::Ok,
                category,
                message: format!("{label} is ready at {}", path.display()),
            }
        } else {
            DoctorCheck {
                status: DoctorStatus::Error,
                category,
                message: format!(
                    "{label} path {} exists but is not a directory",
                    path.display()
                ),
            }
        }
    } else {
        DoctorCheck {
            status: DoctorStatus::Warning,
            category,
            message: if create_if_missing {
                format!(
                    "{label} does not exist yet at {} and will be created on demand",
                    path.display()
                )
            } else {
                format!("{label} does not exist at {}", path.display())
            },
        }
    }
}

fn scope_summary(values: &[String]) -> String {
    if values.is_empty() {
        "<all>".to_owned()
    } else {
        values.join(", ")
    }
}

fn attachment_policy_summary(target: &AttachmentRoutingTargetConfig) -> String {
    format!(
        "mode={} processor={} multimodal_profile={} specialized_processor_profile={}",
        target.mode.label(),
        option_string(target.processor.clone()),
        option_string(target.multimodal_profile.clone()),
        option_string(target.specialized_processor_profile.clone()),
    )
}

fn option_string(value: Option<String>) -> String {
    value.unwrap_or_else(|| "<none>".to_owned())
}

fn option_u64(value: Option<u64>) -> String {
    value
        .map(|value| value.to_string())
        .unwrap_or_else(|| "<none>".to_owned())
}

struct InferredAttachmentCapabilities {
    supports_vision: bool,
    supports_documents: bool,
    supports_audio: bool,
    supports_video: bool,
    preferred_attachment_mode: AttachmentRouteModeConfig,
}

fn infer_profile_capabilities(provider_type: &str, model: &str) -> InferredAttachmentCapabilities {
    let provider_type = parse_provider_type(provider_type);
    let normalized = model.to_ascii_lowercase();
    let supports_vision = if model == "mock" {
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
    let supports_documents = if model == "mock" {
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
        supports_audio: if model == "mock" {
            false
        } else {
            normalized.contains("audio")
                && !matches!(provider_type, Some(ProviderType::Ollama) | None)
        },
        supports_video: if model == "mock" {
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
