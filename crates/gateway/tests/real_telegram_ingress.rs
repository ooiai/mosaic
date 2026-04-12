mod support;

use std::{env, fs, time::Duration};

use mosaic_gateway::{GatewayHandle, serve_http_with_shutdown};
use tokio::{runtime::Handle, sync::oneshot};

#[tokio::test]
async fn real_telegram_ingress_path_normalizes_and_persists_session_when_enabled() {
    if env::var("MOSAIC_REAL_TESTS").ok().as_deref() != Some("1") {
        eprintln!("skipping real Telegram ingress test: set MOSAIC_REAL_TESTS=1");
        return;
    }

    let root = support::temp_dir("telegram");
    fs::create_dir_all(&root).expect("temp root should exist");
    let mut components = support::build_components(&root);
    if env::var("MOSAIC_TEST_TELEGRAM_SECRET").is_ok() {
        components.auth.telegram_secret_token_env = Some("MOSAIC_TEST_TELEGRAM_SECRET".to_owned());
    }
    let gateway = GatewayHandle::new_local(Handle::current(), components);
    let port = support::free_port();
    let addr = format!("127.0.0.1:{port}")
        .parse()
        .expect("socket addr should parse");
    let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
    let server_gateway = gateway.clone();
    let server = tokio::spawn(async move {
        serve_http_with_shutdown(server_gateway, addr, async move {
            let _ = shutdown_rx.await;
        })
        .await
        .expect("gateway http server should run");
    });

    tokio::time::sleep(Duration::from_millis(150)).await;

    let client = reqwest::Client::new();
    let mut request = client
        .post(format!("http://127.0.0.1:{port}/ingress/telegram"))
        .json(&serde_json::json!({
            "update_id": 9001,
            "message": {
                "message_id": 11,
                "text": "What time is it?",
                "message_thread_id": 7,
                "chat": {
                    "id": -10042,
                    "type": "supergroup",
                    "title": "Control Room"
                },
                "from": {
                    "id": 17,
                    "first_name": "Real",
                    "last_name": "Operator"
                }
            }
        }));
    if let Ok(secret) = env::var("MOSAIC_TEST_TELEGRAM_SECRET") {
        request = request.header("x-telegram-bot-api-secret-token", secret);
    }
    let response = request
        .send()
        .await
        .expect("telegram ingress request should succeed");

    assert!(
        response.status().is_success(),
        "status: {}",
        response.status()
    );
    let session = gateway
        .load_session("telegram--10042-7")
        .expect("session load should succeed")
        .expect("telegram session should exist");
    assert_eq!(session.channel_context.channel.as_deref(), Some("telegram"));
    assert_eq!(
        session.channel_context.reply_target.as_deref(),
        Some("telegram:chat:-10042:thread:7:message:11")
    );

    let _ = shutdown_tx.send(());
    server.await.expect("server task should join");
    fs::remove_dir_all(root).ok();
}
