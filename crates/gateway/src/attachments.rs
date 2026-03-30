use std::{
    fs,
    path::{Path, PathBuf},
    time::{Duration, SystemTime},
};

use anyhow::Result;
use mosaic_channel_telegram::TelegramOutboundClient;
use mosaic_control_protocol::{AttachmentFailureTrace, ChannelAttachment, RunSubmission};

use super::*;

pub(crate) async fn prepare_submission_attachments(
    components: &GatewayRuntimeComponents,
    mut submission: RunSubmission,
) -> RunSubmission {
    let Some(ingress) = submission.ingress.as_mut() else {
        return submission;
    };
    if ingress.attachments.is_empty() {
        return submission;
    }

    cleanup_attachment_cache(components);

    let mut prepared = Vec::with_capacity(ingress.attachments.len());
    let mut failures = ingress.attachment_failures.clone();
    for attachment in ingress.attachments.clone() {
        let (attachment, attachment_failures) =
            prepare_attachment(components, ingress, attachment).await;
        prepared.push(attachment);
        failures.extend(attachment_failures);
    }

    ingress.attachments = prepared;
    ingress.attachment_failures = failures;

    if submission.input.trim().is_empty() && !ingress.attachments.is_empty() {
        submission.input = default_attachment_prompt(&ingress.attachments);
    }

    submission
}

async fn prepare_attachment(
    components: &GatewayRuntimeComponents,
    ingress: &IngressTrace,
    mut attachment: ChannelAttachment,
) -> (ChannelAttachment, Vec<AttachmentFailureTrace>) {
    let mut failures = Vec::new();
    let policy = &components.attachments.policy;

    if !policy.enabled {
        failures.push(AttachmentFailureTrace {
            attachment_id: attachment.id.clone(),
            stage: "policy".to_owned(),
            kind: "disabled".to_owned(),
            message: "attachment downloads are disabled by policy".to_owned(),
        });
        return (attachment, failures);
    }

    if let Some(size) = attachment.size_bytes {
        if size > policy.max_size_bytes {
            failures.push(AttachmentFailureTrace {
                attachment_id: attachment.id.clone(),
                stage: "policy".to_owned(),
                kind: "size_limit".to_owned(),
                message: format!(
                    "attachment exceeds max_size_bytes ({} > {})",
                    size, policy.max_size_bytes
                ),
            });
            return (attachment, failures);
        }
    }

    if !mime_is_allowed(policy, attachment.mime_type.as_deref()) {
        failures.push(AttachmentFailureTrace {
            attachment_id: attachment.id.clone(),
            stage: "policy".to_owned(),
            kind: "mime_not_allowed".to_owned(),
            message: format!(
                "attachment mime_type '{}' is not allowed",
                attachment
                    .mime_type
                    .clone()
                    .unwrap_or_else(|| "unknown".to_owned())
            ),
        });
        return (attachment, failures);
    }

    let Some(file_id) = telegram_file_id(ingress.channel.as_deref(), &attachment) else {
        return (attachment, failures);
    };

    let client = match telegram_client(components, ingress, policy.download_timeout_ms) {
        Ok(Some(client)) => client,
        Ok(None) => {
            failures.push(AttachmentFailureTrace {
                attachment_id: attachment.id.clone(),
                stage: "download".to_owned(),
                kind: "telegram_client_unavailable".to_owned(),
                message: "telegram bot token is not configured for this bot instance".to_owned(),
            });
            return (attachment, failures);
        }
        Err(err) => {
            failures.push(AttachmentFailureTrace {
                attachment_id: attachment.id.clone(),
                stage: "download".to_owned(),
                kind: "telegram_client_error".to_owned(),
                message: err.to_string(),
            });
            return (attachment, failures);
        }
    };

    let file = match client.get_file(&file_id).await {
        Ok(file) => file,
        Err(err) => {
            failures.push(AttachmentFailureTrace {
                attachment_id: attachment.id.clone(),
                stage: "resolve".to_owned(),
                kind: "telegram_get_file_failed".to_owned(),
                message: err.to_string(),
            });
            return (attachment, failures);
        }
    };

    if attachment.size_bytes.is_none() {
        attachment.size_bytes = file.file_size;
    }
    if let Some(size) = attachment.size_bytes {
        if size > policy.max_size_bytes {
            failures.push(AttachmentFailureTrace {
                attachment_id: attachment.id.clone(),
                stage: "policy".to_owned(),
                kind: "size_limit".to_owned(),
                message: format!(
                    "attachment exceeds max_size_bytes ({} > {})",
                    size, policy.max_size_bytes
                ),
            });
            return (attachment, failures);
        }
    }

    let Some(file_path) = file.file_path else {
        failures.push(AttachmentFailureTrace {
            attachment_id: attachment.id.clone(),
            stage: "resolve".to_owned(),
            kind: "telegram_file_path_missing".to_owned(),
            message: "telegram getFile response did not include file_path".to_owned(),
        });
        return (attachment, failures);
    };

    let bytes = match client.download_file_bytes(&file_path).await {
        Ok(bytes) => bytes,
        Err(err) => {
            failures.push(AttachmentFailureTrace {
                attachment_id: attachment.id.clone(),
                stage: "download".to_owned(),
                kind: "telegram_download_failed".to_owned(),
                message: err.to_string(),
            });
            return (attachment, failures);
        }
    };

    let cache_path = match cache_path_for_attachment(components, &attachment, &file_path) {
        Ok(path) => path,
        Err(err) => {
            failures.push(AttachmentFailureTrace {
                attachment_id: attachment.id.clone(),
                stage: "cache".to_owned(),
                kind: "cache_path_error".to_owned(),
                message: err.to_string(),
            });
            return (attachment, failures);
        }
    };

    if let Some(parent) = cache_path.parent() {
        if let Err(err) = fs::create_dir_all(parent) {
            failures.push(AttachmentFailureTrace {
                attachment_id: attachment.id.clone(),
                stage: "cache".to_owned(),
                kind: "cache_dir_error".to_owned(),
                message: err.to_string(),
            });
            return (attachment, failures);
        }
    }
    if let Err(err) = fs::write(&cache_path, &bytes) {
        failures.push(AttachmentFailureTrace {
            attachment_id: attachment.id.clone(),
            stage: "cache".to_owned(),
            kind: "cache_write_failed".to_owned(),
            message: err.to_string(),
        });
        return (attachment, failures);
    }

    attachment.remote_url = Some(format!("telegram:file_path:{file_path}"));
    attachment.local_cache_path = Some(cache_path.display().to_string());
    (attachment, failures)
}

fn default_attachment_prompt(attachments: &[ChannelAttachment]) -> String {
    let kinds = attachments
        .iter()
        .map(|attachment| attachment.kind.label())
        .collect::<Vec<_>>();
    format!("Please analyze the attached item(s): {}.", kinds.join(", "))
}

fn telegram_file_id(channel: Option<&str>, attachment: &ChannelAttachment) -> Option<String> {
    if channel != Some("telegram") {
        return None;
    }
    attachment
        .source_ref
        .as_deref()
        .and_then(|value| value.strip_prefix("telegram:file_id:"))
        .map(ToOwned::to_owned)
}

fn telegram_client(
    components: &GatewayRuntimeComponents,
    ingress: &IngressTrace,
    timeout_ms: u64,
) -> Result<Option<TelegramOutboundClient>> {
    let Some(bot) = resolved_telegram_bot_by_name(components, ingress.bot_name.as_deref()) else {
        return Ok(None);
    };
    telegram_outbound_client_for_bot_with_settings(
        &bot,
        Duration::from_millis(timeout_ms.max(1)),
        0,
    )
}

fn cache_path_for_attachment(
    components: &GatewayRuntimeComponents,
    attachment: &ChannelAttachment,
    file_path: &str,
) -> Result<PathBuf> {
    let root = attachment_cache_root(components);
    let extension = Path::new(file_path)
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("bin");
    Ok(root.join(format!(
        "{}.{}",
        sanitize_filename(&attachment.id),
        sanitize_filename(extension)
    )))
}

fn attachment_cache_root(components: &GatewayRuntimeComponents) -> PathBuf {
    let path = PathBuf::from(&components.attachments.policy.cache_dir);
    if path.is_absolute() {
        path
    } else {
        components.workspace_root.join(path)
    }
}

fn cleanup_attachment_cache(components: &GatewayRuntimeComponents) {
    let root = attachment_cache_root(components);
    let cutoff = SystemTime::now()
        .checked_sub(Duration::from_secs(
            components
                .attachments
                .policy
                .cleanup_after_hours
                .saturating_mul(3600),
        ))
        .unwrap_or(SystemTime::UNIX_EPOCH);

    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let Ok(metadata) = entry.metadata() else {
            continue;
        };
        let Ok(modified) = metadata.modified() else {
            continue;
        };
        if modified < cutoff {
            let _ = fs::remove_file(entry.path());
        }
    }
}

fn sanitize_filename(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '-') {
                ch
            } else {
                '_'
            }
        })
        .collect()
}

fn mime_is_allowed(
    policy: &mosaic_config::AttachmentPolicyConfig,
    mime_type: Option<&str>,
) -> bool {
    let Some(mime_type) = mime_type else {
        return true;
    };
    if policy.allowed_mime_types.is_empty() {
        return true;
    }
    policy.allowed_mime_types.iter().any(|pattern| {
        if pattern.ends_with('/') {
            mime_type.starts_with(pattern)
        } else {
            mime_type.eq_ignore_ascii_case(pattern)
        }
    })
}

#[cfg(test)]
mod tests {
    use std::{path::PathBuf, sync::Arc};

    use axum::{Json, Router, routing::get};
    use chrono::Utc;
    use mosaic_channel_telegram::TelegramFile;
    use mosaic_config::{MosaicConfig, ProviderProfileConfig};
    use mosaic_control_protocol::{AttachmentKind, IngressTrace, RunSubmission};
    use mosaic_memory::{FileMemoryStore, MemoryPolicy};
    use mosaic_node_protocol::FileNodeStore;
    use mosaic_provider::{MockProvider, ProviderProfileRegistry};
    use mosaic_scheduler_core::FileCronStore;
    use mosaic_session_core::FileSessionStore;
    use mosaic_skill_core::SkillRegistry;
    use mosaic_tool_core::ToolRegistry;
    use mosaic_workflow::WorkflowRegistry;

    use super::*;

    fn components(
        root: PathBuf,
        configure: impl FnOnce(&mut MosaicConfig),
    ) -> GatewayRuntimeComponents {
        let mut config = MosaicConfig::default();
        config.active_profile = "mock".to_owned();
        config.profiles.insert(
            "mock".to_owned(),
            ProviderProfileConfig {
                provider_type: "mock".to_owned(),
                model: "mock".to_owned(),
                base_url: None,
                api_key_env: None,
                transport: Default::default(),
                attachments: Default::default(),
                vendor: Default::default(),
            },
        );
        configure(&mut config);

        GatewayRuntimeComponents {
            profiles: Arc::new(
                ProviderProfileRegistry::from_config(&config)
                    .expect("profile registry should build"),
            ),
            provider_override: Some(Arc::new(MockProvider)),
            session_store: Arc::new(FileSessionStore::new(root.join("sessions"))),
            memory_store: Arc::new(FileMemoryStore::new(root.join("memory"))),
            memory_policy: MemoryPolicy::default(),
            runtime_policy: config.runtime.clone(),
            attachments: config.attachments.clone(),
            telegram: config.telegram.clone(),
            app_name: None,
            tools: Arc::new(ToolRegistry::new()),
            skills: Arc::new(SkillRegistry::new()),
            workflows: Arc::new(WorkflowRegistry::new()),
            node_store: Arc::new(FileNodeStore::new(root.join("nodes"))),
            mcp_manager: None,
            cron_store: Arc::new(FileCronStore::new(root.join("cron"))),
            workspace_root: root.clone(),
            runs_dir: root.join("runs"),
            audit_root: root.join("audit"),
            extensions: Vec::new(),
            policies: config.policies.clone(),
            deployment: config.deployment.clone(),
            auth: config.auth.clone(),
            audit: config.audit.clone(),
            observability: config.observability.clone(),
        }
    }

    fn submission_with_attachment(attachment: ChannelAttachment) -> RunSubmission {
        RunSubmission {
            system: None,
            input: String::new(),
            tool: None,
            skill: None,
            workflow: None,
            session_id: Some("demo".to_owned()),
            profile: Some("mock".to_owned()),
            ingress: Some(IngressTrace {
                kind: "telegram".to_owned(),
                channel: Some("telegram".to_owned()),
                adapter: Some("telegram_webhook".to_owned()),
                bot_name: None,
                bot_route: None,
                bot_profile: None,
                bot_token_env: None,
                bot_secret_env: None,
                source: Some("telegram".to_owned()),
                remote_addr: None,
                display_name: Some("Operator".to_owned()),
                actor_id: Some("17".to_owned()),
                conversation_id: Some("telegram:chat:1".to_owned()),
                thread_id: None,
                thread_title: None,
                reply_target: Some("telegram:chat:1:message:5".to_owned()),
                message_id: Some("5".to_owned()),
                received_at: Some(Utc::now()),
                raw_event_id: Some("event-1".to_owned()),
                session_hint: Some("demo".to_owned()),
                profile_hint: Some("mock".to_owned()),
                control_command: None,
                original_text: None,
                attachments: vec![attachment],
                attachment_failures: vec![],
                gateway_url: None,
            }),
        }
    }

    #[tokio::test]
    async fn prepare_submission_attachments_rejects_disallowed_mime_types() {
        let root = std::env::temp_dir().join(format!(
            "mosaic-gateway-attachments-{}",
            uuid::Uuid::new_v4()
        ));
        let components = components(root.clone(), |config| {
            config.attachments.policy.allowed_mime_types = vec!["image/".to_owned()];
        });

        let prepared = prepare_submission_attachments(
            &components,
            submission_with_attachment(ChannelAttachment {
                id: "doc-1".to_owned(),
                kind: AttachmentKind::Document,
                filename: Some("notes.pdf".to_owned()),
                mime_type: Some("application/pdf".to_owned()),
                size_bytes: Some(256),
                source_ref: Some("telegram:file_id:file-doc".to_owned()),
                remote_url: None,
                local_cache_path: None,
                caption: None,
            }),
        )
        .await;

        let ingress = prepared
            .ingress
            .expect("prepared submission should retain ingress");
        assert_eq!(ingress.attachments.len(), 1);
        assert_eq!(ingress.attachment_failures.len(), 1);
        assert_eq!(ingress.attachment_failures[0].kind, "mime_not_allowed");
        assert_eq!(
            prepared.input,
            "Please analyze the attached item(s): document."
        );

        std::fs::remove_dir_all(root).ok();
    }

    #[tokio::test]
    async fn prepare_submission_attachments_downloads_and_caches_telegram_files() {
        let _env_guard = crate::tests::telegram_env_lock()
            .lock()
            .expect("telegram env lock should not be poisoned");
        let api = Router::new()
            .route(
                "/bottest-token/getFile",
                get(|| async {
                    Json(serde_json::json!({
                        "ok": true,
                        "result": TelegramFile {
                            file_id: "photo-1".to_owned(),
                            file_unique_id: Some("uniq-1".to_owned()),
                            file_size: Some(512),
                            file_path: Some("files/demo.jpg".to_owned()),
                        }
                    }))
                }),
            )
            .route(
                "/file/bottest-token/files/demo.jpg",
                get(|| async { "cached-image" }),
            );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("telegram test listener should bind");
        let addr = listener.local_addr().expect("listener addr should resolve");
        tokio::spawn(async move {
            let _ = axum::serve(listener, api).await;
        });

        let previous_token = std::env::var("MOSAIC_TELEGRAM_BOT_TOKEN").ok();
        let previous_base_url = std::env::var("MOSAIC_TELEGRAM_API_BASE_URL").ok();
        unsafe {
            std::env::set_var("MOSAIC_TELEGRAM_BOT_TOKEN", "test-token");
            std::env::set_var("MOSAIC_TELEGRAM_API_BASE_URL", format!("http://{addr}"));
        }

        let root = std::env::temp_dir().join(format!(
            "mosaic-gateway-attachments-{}",
            uuid::Uuid::new_v4()
        ));
        let components = components(root.clone(), |_| {});

        let prepared = prepare_submission_attachments(
            &components,
            submission_with_attachment(ChannelAttachment {
                id: "photo-1".to_owned(),
                kind: AttachmentKind::Image,
                filename: Some("demo.jpg".to_owned()),
                mime_type: Some("image/jpeg".to_owned()),
                size_bytes: Some(512),
                source_ref: Some("telegram:file_id:photo-1".to_owned()),
                remote_url: None,
                local_cache_path: None,
                caption: Some("look at this".to_owned()),
            }),
        )
        .await;

        let ingress = prepared
            .ingress
            .expect("prepared submission should retain ingress");
        assert_eq!(ingress.attachment_failures.len(), 0);
        assert_eq!(ingress.attachments.len(), 1);
        assert_eq!(
            ingress.attachments[0].remote_url.as_deref(),
            Some("telegram:file_path:files/demo.jpg")
        );
        let cache_path = ingress.attachments[0]
            .local_cache_path
            .as_deref()
            .expect("cached attachment path should exist");
        assert!(std::path::Path::new(cache_path).exists());
        assert_eq!(
            prepared.input,
            "Please analyze the attached item(s): image."
        );

        unsafe {
            if let Some(value) = previous_token {
                std::env::set_var("MOSAIC_TELEGRAM_BOT_TOKEN", value);
            } else {
                std::env::remove_var("MOSAIC_TELEGRAM_BOT_TOKEN");
            }
            if let Some(value) = previous_base_url {
                std::env::set_var("MOSAIC_TELEGRAM_API_BASE_URL", value);
            } else {
                std::env::remove_var("MOSAIC_TELEGRAM_API_BASE_URL");
            }
        }
        std::fs::remove_dir_all(root).ok();
    }
}
