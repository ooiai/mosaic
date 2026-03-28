use std::env;

use anyhow::{Result, anyhow, bail};
use mosaic_channel_telegram::{TelegramOutboundClient, TelegramWebhookConfig, TelegramWebhookInfo};
use mosaic_control_protocol::{AdapterStatusDto, ChannelDeliveryStatus};

use crate::{
    AdapterCommand, TelegramAdapterCommand, TelegramWebhookCommand, build_gateway_handle,
    ensure_loaded_config, gateway_client_from_loaded,
};

pub(crate) async fn adapter_cmd(attach: Option<String>, command: AdapterCommand) -> Result<()> {
    match command {
        AdapterCommand::Status => {
            let adapters = list_adapters(attach).await?;
            print_adapter_statuses(&adapters)
        }
        AdapterCommand::Doctor => {
            let adapters = list_adapters(attach).await?;
            print_adapter_doctor(&adapters)
        }
        AdapterCommand::Telegram { command } => {
            if attach.is_some() {
                bail!(
                    "remote attach does not support direct Telegram API management; run `mosaic adapter telegram ...` locally with the bot token env configured"
                );
            }
            telegram_cmd(command).await
        }
    }
}

async fn list_adapters(attach: Option<String>) -> Result<Vec<AdapterStatusDto>> {
    if let Some(url) = attach {
        let loaded = ensure_loaded_config(None)?;
        return gateway_client_from_loaded(&loaded, url)
            .list_adapters()
            .await
            .map_err(Into::into);
    }

    let loaded = ensure_loaded_config(None)?;
    let gateway = build_gateway_handle(&loaded, None)?;
    Ok(gateway.list_adapter_statuses())
}

async fn telegram_cmd(command: TelegramAdapterCommand) -> Result<()> {
    let loaded = ensure_loaded_config(None)?;
    let client = telegram_client_from_env()?;

    match command {
        TelegramAdapterCommand::Webhook { command } => match command {
            TelegramWebhookCommand::Set {
                url,
                secret_token,
                allowed_updates,
                drop_pending_updates,
            } => {
                let url = resolve_telegram_webhook_url(url)?;
                let secret_token = resolve_telegram_secret_token(&loaded, secret_token)?;
                let info = set_telegram_webhook(
                    &client,
                    url,
                    secret_token,
                    allowed_updates,
                    drop_pending_updates,
                )
                .await?;
                print_telegram_webhook_info("telegram webhook updated", &info)?;
                crate::print_next_steps([
                    "mosaic adapter telegram webhook info",
                    "mosaic adapter status",
                ]);
                Ok(())
            }
            TelegramWebhookCommand::Info => {
                let info = fetch_telegram_webhook_info(&client).await?;
                print_telegram_webhook_info("telegram webhook", &info)
            }
            TelegramWebhookCommand::Delete {
                drop_pending_updates,
            } => {
                delete_telegram_webhook(&client, drop_pending_updates).await?;
                println!("telegram webhook deleted");
                println!("drop_pending_updates: {}", drop_pending_updates);
                crate::print_next_steps([
                    "mosaic adapter telegram webhook info",
                    "mosaic adapter status",
                ]);
                Ok(())
            }
        },
        TelegramAdapterCommand::TestSend {
            chat_id,
            text,
            thread_id,
            reply_to,
        } => {
            let delivery =
                send_telegram_test_message(&client, chat_id, text, thread_id, reply_to).await;
            println!("telegram outbound test:");
            println!("  chat_id: {}", chat_id);
            println!("  thread_id: {:?}", thread_id);
            println!("  reply_to: {:?}", reply_to);
            println!("  status: {}", delivery.result.status.label());
            println!(
                "  provider_message_id: {:?}",
                delivery.result.provider_message_id
            );
            println!("  retries: {}", delivery.result.retry_count);
            println!("  error_kind: {:?}", delivery.result.error_kind);
            println!("  error: {:?}", delivery.result.error);
            if delivery.result.status == ChannelDeliveryStatus::Failed {
                bail!(
                    "telegram outbound test failed; check the bot token, chat id, and adapter readiness"
                );
            }
            crate::print_next_steps([
                "mosaic adapter status",
                "mosaic adapter telegram webhook info",
            ]);
            Ok(())
        }
    }
}

fn print_adapter_statuses(adapters: &[AdapterStatusDto]) -> Result<()> {
    println!("adapter summary:");
    println!("  adapters: {}", adapters.len());
    println!(
        "  errors: {}",
        adapters
            .iter()
            .filter(|adapter| adapter.status == "error")
            .count()
    );
    println!(
        "  warnings: {}",
        adapters
            .iter()
            .filter(|adapter| adapter.status == "warning")
            .count()
    );
    if adapters.is_empty() {
        return Ok(());
    }

    println!("adapters:");
    for adapter in adapters {
        println!(
            "  - {} | channel={} | transport={} | path={} | status={} | outbound_ready={}",
            adapter.name,
            adapter.channel,
            adapter.transport,
            adapter.ingress_path,
            adapter.status,
            adapter.outbound_ready,
        );
        if !adapter.capabilities.is_empty() {
            println!("    capabilities: {}", adapter.capabilities.join(", "));
        }
        println!("    {}", adapter.detail);
        if adapter.name == "telegram" {
            println!(
                "    operator: use `mosaic adapter telegram webhook info` and `mosaic adapter telegram test-send --chat-id <chat-id> \"hello\"`"
            );
        }
    }

    Ok(())
}

fn print_adapter_doctor(adapters: &[AdapterStatusDto]) -> Result<()> {
    println!("adapter doctor:");
    print_adapter_statuses(adapters)?;
    if adapters.iter().any(|adapter| adapter.status == "error") {
        bail!("adapter doctor found errors");
    }
    println!("adapter doctor: ok");
    crate::print_next_steps([
        "mosaic adapter telegram webhook info",
        "mosaic adapter telegram test-send --chat-id <chat-id> \"hello from mosaic\"",
    ]);
    Ok(())
}

fn telegram_client_from_env() -> Result<TelegramOutboundClient> {
    TelegramOutboundClient::from_env()?
        .ok_or_else(|| {
            anyhow!(
                "Telegram bot token is not configured; set MOSAIC_TELEGRAM_BOT_TOKEN or TELEGRAM_BOT_TOKEN"
            )
        })
}

fn resolve_telegram_webhook_url(explicit_url: Option<String>) -> Result<String> {
    if let Some(url) = explicit_url {
        return Ok(url);
    }

    let base_url = env::var("MOSAIC_PUBLIC_WEBHOOK_BASE_URL")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| {
            anyhow!(
                "missing Telegram webhook URL: pass --url or set MOSAIC_PUBLIC_WEBHOOK_BASE_URL"
            )
        })?;
    Ok(format!(
        "{}/ingress/telegram",
        base_url.trim_end_matches('/')
    ))
}

fn resolve_telegram_secret_token(
    loaded: &mosaic_config::LoadedMosaicConfig,
    override_value: Option<String>,
) -> Result<Option<String>> {
    if let Some(value) = override_value {
        return Ok(Some(value));
    }

    if let Some(env_name) = loaded.config.auth.telegram_secret_token_env.as_deref() {
        let value = env::var(env_name)
            .ok()
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| {
                anyhow!(
                    "telegram secret token env `{}` is configured but not set",
                    env_name
                )
            })?;
        return Ok(Some(value));
    }

    Ok(env::var("MOSAIC_TELEGRAM_SECRET_TOKEN")
        .ok()
        .filter(|value| !value.trim().is_empty()))
}

async fn set_telegram_webhook(
    client: &TelegramOutboundClient,
    url: String,
    secret_token: Option<String>,
    allowed_updates: Vec<String>,
    drop_pending_updates: bool,
) -> Result<TelegramWebhookInfo> {
    let allowed_updates = if allowed_updates.is_empty() {
        vec!["message".to_owned()]
    } else {
        allowed_updates
    };
    client
        .set_webhook(TelegramWebhookConfig {
            url,
            secret_token,
            allowed_updates,
            drop_pending_updates,
        })
        .await
}

async fn fetch_telegram_webhook_info(
    client: &TelegramOutboundClient,
) -> Result<TelegramWebhookInfo> {
    client.get_webhook_info().await
}

async fn delete_telegram_webhook(
    client: &TelegramOutboundClient,
    drop_pending_updates: bool,
) -> Result<()> {
    client.delete_webhook(drop_pending_updates).await
}

async fn send_telegram_test_message(
    client: &TelegramOutboundClient,
    chat_id: i64,
    text: String,
    thread_id: Option<i64>,
    reply_to: Option<i64>,
) -> mosaic_control_protocol::ChannelDeliveryTrace {
    client
        .send_test_message(chat_id, text, thread_id, reply_to)
        .await
}

fn print_telegram_webhook_info(label: &str, info: &TelegramWebhookInfo) -> Result<()> {
    println!("{label}:");
    println!("  url: {}", info.url);
    println!("  pending_update_count: {}", info.pending_update_count);
    println!("  has_custom_certificate: {}", info.has_custom_certificate);
    println!("  last_error_date: {:?}", info.last_error_date);
    println!("  last_error_message: {:?}", info.last_error_message);
    println!(
        "  last_synchronization_error_date: {:?}",
        info.last_synchronization_error_date
    );
    println!("  max_connections: {:?}", info.max_connections);
    println!("  ip_address: {:?}", info.ip_address);
    println!("  allowed_updates: {:?}", info.allowed_updates);
    Ok(())
}

#[cfg(test)]
mod tests {
    use axum::{
        Json, Router,
        routing::{any, post},
    };
    use serde_json::Value;

    use super::*;

    #[test]
    fn resolves_webhook_url_from_public_base_env() {
        // SAFETY: cli tests in this module do not concurrently mutate or read this env var.
        unsafe {
            env::set_var(
                "MOSAIC_PUBLIC_WEBHOOK_BASE_URL",
                "https://public.example.com/base/",
            );
        }
        let url = resolve_telegram_webhook_url(None).expect("webhook url should resolve");
        assert_eq!(url, "https://public.example.com/base/ingress/telegram");
        // SAFETY: cli tests in this module do not concurrently mutate or read this env var.
        unsafe {
            env::remove_var("MOSAIC_PUBLIC_WEBHOOK_BASE_URL");
        }
    }

    #[tokio::test]
    async fn telegram_webhook_commands_work_against_local_bot_api() {
        let requests = std::sync::Arc::new(tokio::sync::Mutex::new(Vec::<(String, Value)>::new()));
        let app = Router::new()
            .route(
                "/bottest-token/setWebhook",
                post({
                    let requests = requests.clone();
                    move |Json(payload): Json<Value>| {
                        let requests = requests.clone();
                        async move {
                            requests
                                .lock()
                                .await
                                .push(("setWebhook".to_owned(), payload));
                            Json(serde_json::json!({ "ok": true, "result": true }))
                        }
                    }
                }),
            )
            .route(
                "/bottest-token/getWebhookInfo",
                any({
                    let requests = requests.clone();
                    move || {
                        let requests = requests.clone();
                        async move {
                            requests
                                .lock()
                                .await
                                .push(("getWebhookInfo".to_owned(), serde_json::json!({})));
                            Json(serde_json::json!({
                                "ok": true,
                                "result": {
                                    "url": "https://public.example.com/ingress/telegram",
                                    "has_custom_certificate": false,
                                    "pending_update_count": 0,
                                    "allowed_updates": ["message"]
                                }
                            }))
                        }
                    }
                }),
            )
            .route(
                "/bottest-token/deleteWebhook",
                post({
                    let requests = requests.clone();
                    move |Json(payload): Json<Value>| {
                        let requests = requests.clone();
                        async move {
                            requests
                                .lock()
                                .await
                                .push(("deleteWebhook".to_owned(), payload));
                            Json(serde_json::json!({ "ok": true, "result": true }))
                        }
                    }
                }),
            )
            .route(
                "/bottest-token/sendMessage",
                post({
                    let requests = requests.clone();
                    move |Json(payload): Json<Value>| {
                        let requests = requests.clone();
                        async move {
                            requests
                                .lock()
                                .await
                                .push(("sendMessage".to_owned(), payload));
                            Json(serde_json::json!({
                                "ok": true,
                                "result": { "message_id": 77 }
                            }))
                        }
                    }
                }),
            );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("listener should bind");
        let addr = listener.local_addr().expect("addr should exist");
        tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });

        let client = TelegramOutboundClient::new("test-token", format!("http://{addr}"))
            .expect("telegram client should build");

        let info = set_telegram_webhook(
            &client,
            "https://public.example.com/ingress/telegram".to_owned(),
            Some("secret-1".to_owned()),
            vec!["message".to_owned()],
            true,
        )
        .await
        .expect("webhook should set");
        assert_eq!(info.url, "https://public.example.com/ingress/telegram");

        let info = fetch_telegram_webhook_info(&client)
            .await
            .expect("webhook info should load");
        assert_eq!(info.allowed_updates, vec!["message".to_owned()]);

        delete_telegram_webhook(&client, true)
            .await
            .expect("webhook should delete");

        let delivery =
            send_telegram_test_message(&client, 42, "hello from cli".to_owned(), Some(7), None)
                .await;
        assert_eq!(delivery.result.status, ChannelDeliveryStatus::Delivered);

        let requests = requests.lock().await;
        assert_eq!(requests[0].0, "setWebhook");
        assert_eq!(requests[0].1["secret_token"], "secret-1");
        assert_eq!(requests[1].0, "getWebhookInfo");
        assert_eq!(requests[2].0, "getWebhookInfo");
        assert_eq!(requests[3].0, "deleteWebhook");
        assert_eq!(requests[4].0, "sendMessage");
    }
}
