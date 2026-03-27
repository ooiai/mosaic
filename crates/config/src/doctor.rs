use super::*;

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

    for manifest in &config.extensions.manifests {
        let manifest_path = cwd.join(&manifest.path);
        checks.push(path_check(
            &manifest_path,
            DoctorCategory::Extensions,
            "extension manifest",
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
                status: DoctorStatus::Ok,
                category: DoctorCategory::Providers,
                message: format!(
                    "profile '{}' uses the mock provider and does not require API credentials",
                    name
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
    }

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
