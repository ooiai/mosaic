use assert_cmd::Command;
use serde_json::Value;
use tempfile::tempdir;

#[test]
#[allow(deprecated)]
fn gateway_skeleton_flow() {
    let temp = tempdir().expect("tempdir");

    let status_before = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .env("MOSAIC_GATEWAY_TEST_MODE", "1")
        .args(["--project-state", "--json", "gateway", "status"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let status_before: Value = serde_json::from_slice(&status_before).expect("status before json");
    assert_eq!(status_before["ok"], true);
    assert_eq!(status_before["running"], false);

    let run_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .env("MOSAIC_GATEWAY_TEST_MODE", "1")
        .args([
            "--project-state",
            "--json",
            "gateway",
            "run",
            "--host",
            "0.0.0.0",
            "--port",
            "8789",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let run_json: Value = serde_json::from_slice(&run_output).expect("run json");
    assert_eq!(run_json["ok"], true);
    assert_eq!(run_json["gateway"]["host"], "0.0.0.0");
    assert_eq!(run_json["gateway"]["port"], 8789);

    let status_after = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .env("MOSAIC_GATEWAY_TEST_MODE", "1")
        .args(["--project-state", "--json", "gateway", "status"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let status_after: Value = serde_json::from_slice(&status_after).expect("status after json");
    assert_eq!(status_after["running"], true);

    let stop_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .env("MOSAIC_GATEWAY_TEST_MODE", "1")
        .args(["--project-state", "--json", "gateway", "stop"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let stop_json: Value = serde_json::from_slice(&stop_output).expect("stop json");
    assert_eq!(stop_json["ok"], true);
    assert_eq!(stop_json["gateway"]["running"], false);
}

#[test]
#[allow(deprecated)]
fn channels_real_send_flow() {
    let temp = tempdir().expect("tempdir");

    let list_initial = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args(["--project-state", "--json", "channels", "list"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let list_initial: Value = serde_json::from_slice(&list_initial).expect("list initial json");
    assert_eq!(list_initial["ok"], true);
    assert_eq!(
        list_initial["channels"]
            .as_array()
            .expect("channels array")
            .len(),
        0
    );

    let add_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "channels",
            "add",
            "--name",
            "local-slack",
            "--kind",
            "slack_webhook",
            "--endpoint",
            "mock-http://200",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let add_json: Value = serde_json::from_slice(&add_output).expect("add json");
    let channel_id = add_json["channel"]["id"]
        .as_str()
        .expect("channel id")
        .to_string();
    assert_eq!(add_json["channel"]["name"], "local-slack");
    assert_eq!(add_json["channel"]["kind"], "slack_webhook");
    assert!(add_json["channel"]["endpoint_masked"].is_string());

    let test_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args(["--project-state", "--json", "channels", "test", &channel_id])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let test_json: Value = serde_json::from_slice(&test_output).expect("test json");
    assert_eq!(test_json["ok"], true);
    assert_eq!(test_json["probe"], true);
    assert_eq!(test_json["kind"], "test_probe");

    let list_after_test = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args(["--project-state", "--json", "channels", "list"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let list_after_test: Value = serde_json::from_slice(&list_after_test).expect("list after test");
    let channels = list_after_test["channels"]
        .as_array()
        .expect("channels array");
    assert_eq!(channels.len(), 1);
    assert_eq!(channels[0]["id"], channel_id);
    assert!(channels[0]["last_send_at"].is_null());
    assert!(channels[0]["endpoint_masked"].is_string());

    let send_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "channels",
            "send",
            &channel_id,
            "--text",
            "hello-channel",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let send_json: Value = serde_json::from_slice(&send_output).expect("send json");
    assert_eq!(send_json["ok"], true);
    assert_eq!(send_json["kind"], "message");
    assert_eq!(send_json["attempts"], 1);
    let path = send_json["event_path"].as_str().expect("event path");
    let event_content = std::fs::read_to_string(path).expect("event file content");
    let events = event_content
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).expect("event line json"))
        .collect::<Vec<_>>();
    assert_eq!(events.len(), 2);
    assert_eq!(events[0]["kind"], "test_probe");
    assert_eq!(events[1]["kind"], "message");
    assert_eq!(events[1]["delivery_status"], "success");
    assert!(
        events[1]["text_preview"]
            .as_str()
            .expect("text_preview")
            .contains("hello-channel")
    );

    let list_after_send = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args(["--project-state", "--json", "channels", "list"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let list_after_send: Value = serde_json::from_slice(&list_after_send).expect("list after send");
    let channels_after_send = list_after_send["channels"]
        .as_array()
        .expect("channels array");
    assert!(channels_after_send[0]["last_send_at"].is_string());
    assert!(channels_after_send[0]["last_error"].is_null());
}

#[test]
#[allow(deprecated)]
fn channels_discord_webhook_flow() {
    let temp = tempdir().expect("tempdir");

    let add_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "channels",
            "add",
            "--name",
            "discord-alerts",
            "--kind",
            "discord",
            "--endpoint",
            "mock-http://200",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let add_json: Value = serde_json::from_slice(&add_output).expect("add discord json");
    let channel_id = add_json["channel"]["id"]
        .as_str()
        .expect("channel id")
        .to_string();
    assert_eq!(add_json["channel"]["kind"], "discord_webhook");

    let send_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "channels",
            "send",
            &channel_id,
            "--text",
            "hello-discord",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let send_json: Value = serde_json::from_slice(&send_output).expect("send discord json");
    assert_eq!(send_json["ok"], true);
    assert_eq!(send_json["delivered_via"], "discord_webhook");

    let list_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args(["--project-state", "--json", "channels", "list"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let list_json: Value = serde_json::from_slice(&list_output).expect("list json");
    let channels = list_json["channels"].as_array().expect("channels array");
    assert_eq!(channels.len(), 1);
    assert_eq!(channels[0]["kind"], "discord_webhook");
    assert!(channels[0]["last_send_at"].is_string());
}

#[test]
#[allow(deprecated)]
fn channels_terminal_alias_flow() {
    let temp = tempdir().expect("tempdir");

    let add_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "channels",
            "add",
            "--name",
            "term-stdout",
            "--kind",
            "stdout",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let add_json: Value = serde_json::from_slice(&add_output).expect("add terminal json");
    let channel_id = add_json["channel"]["id"]
        .as_str()
        .expect("channel id")
        .to_string();
    assert_eq!(add_json["channel"]["kind"], "terminal");
    assert_eq!(add_json["channel"]["target_masked"], "terminal://local");

    let send_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "channels",
            "send",
            &channel_id,
            "--text",
            "hello terminal",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let send_json: Value = serde_json::from_slice(&send_output).expect("send terminal json");
    assert_eq!(send_json["ok"], true);
    assert_eq!(send_json["delivered_via"], "terminal");
    assert_eq!(send_json["target_masked"], "terminal://local");

    let terminal_capabilities = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "channels",
            "capabilities",
            "--channel",
            "terminal",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let terminal_capabilities: Value =
        serde_json::from_slice(&terminal_capabilities).expect("terminal capabilities");
    assert_eq!(
        terminal_capabilities["capabilities"][0]["supports_parse_mode"],
        false
    );
    assert_eq!(
        terminal_capabilities["capabilities"][0]["supports_idempotency_key"],
        true
    );
}

#[test]
#[allow(deprecated)]
fn channels_telegram_bot_flow() {
    let temp = tempdir().expect("tempdir");

    let add_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "channels",
            "add",
            "--name",
            "tg-alerts",
            "--kind",
            "telegram",
            "--chat-id=-1001234567890",
            "--endpoint",
            "mock-http://200",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let add_json: Value = serde_json::from_slice(&add_output).expect("add telegram json");
    let channel_id = add_json["channel"]["id"]
        .as_str()
        .expect("channel id")
        .to_string();
    assert_eq!(add_json["channel"]["kind"], "telegram_bot");
    assert!(
        add_json["channel"]["target_masked"]
            .as_str()
            .expect("target masked")
            .starts_with("telegram://***")
    );

    let send_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .env("MOSAIC_TELEGRAM_BOT_TOKEN", "test-token")
        .env("MOSAIC_CHANNELS_TELEGRAM_MIN_INTERVAL_MS", "300")
        .args([
            "--project-state",
            "--json",
            "channels",
            "send",
            &channel_id,
            "--text",
            "hello telegram",
            "--parse-mode",
            "markdown_v2",
            "--title",
            "Release Notice",
            "--block",
            "build=42",
            "--metadata",
            "{\"env\":\"staging\"}",
            "--idempotency-key",
            "release-42",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let send_json: Value = serde_json::from_slice(&send_output).expect("send telegram json");
    assert_eq!(send_json["ok"], true);
    assert_eq!(send_json["delivered_via"], "telegram_bot");
    assert_eq!(send_json["attempts"], 1);
    assert_eq!(send_json["parse_mode"], "MarkdownV2");
    assert_eq!(send_json["idempotency_key"], "release-42");
    assert_eq!(send_json["deduplicated"], false);
    assert!(send_json["rate_limited_ms"].is_number());

    let telegram_capabilities = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "channels",
            "capabilities",
            "--channel",
            "telegram_bot",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let telegram_capabilities: Value =
        serde_json::from_slice(&telegram_capabilities).expect("telegram capabilities");
    assert_eq!(
        telegram_capabilities["capabilities"][0]["supports_parse_mode"],
        true
    );
    assert_eq!(
        telegram_capabilities["capabilities"][0]["supports_rate_limit_report"],
        true
    );

    let dedup_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .env("MOSAIC_TELEGRAM_BOT_TOKEN", "test-token")
        .env("MOSAIC_CHANNELS_TELEGRAM_MIN_INTERVAL_MS", "300")
        .args([
            "--project-state",
            "--json",
            "channels",
            "send",
            &channel_id,
            "--text",
            "hello telegram",
            "--parse-mode",
            "markdown_v2",
            "--title",
            "Release Notice",
            "--block",
            "build=42",
            "--idempotency-key",
            "release-42",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let dedup_json: Value = serde_json::from_slice(&dedup_output).expect("dedup telegram json");
    assert_eq!(dedup_json["deduplicated"], true);
    assert_eq!(dedup_json["attempts"], 0);
    let dedup_event_path = dedup_json["event_path"].as_str().expect("dedup event path");
    let dedup_events = std::fs::read_to_string(dedup_event_path).expect("dedup event file");
    let dedup_last = dedup_events
        .lines()
        .last()
        .expect("dedup last line")
        .to_string();
    let dedup_last: Value = serde_json::from_str(&dedup_last).expect("dedup event json");
    assert_eq!(dedup_last["delivery_status"], "deduplicated");
    assert_eq!(dedup_last["idempotency_key"], "release-42");

    let add_defaults_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "channels",
            "add",
            "--name",
            "tg-defaults",
            "--kind",
            "telegram_bot",
            "--chat-id=-1000000000009",
            "--endpoint",
            "mock-http://200",
            "--default-parse-mode",
            "markdown",
            "--default-title",
            "Default Header",
            "--default-block",
            "service=mosaic",
            "--default-metadata",
            "{\"env\":\"prod\"}",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let add_defaults_json: Value =
        serde_json::from_slice(&add_defaults_output).expect("add defaults json");
    let defaults_channel_id = add_defaults_json["channel"]["id"]
        .as_str()
        .expect("defaults channel id")
        .to_string();
    assert_eq!(add_defaults_json["channel"]["has_template_defaults"], true);

    let send_defaults_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .env("MOSAIC_TELEGRAM_BOT_TOKEN", "test-token")
        .args([
            "--project-state",
            "--json",
            "channels",
            "send",
            &defaults_channel_id,
            "--text",
            "from defaults",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let send_defaults_json: Value =
        serde_json::from_slice(&send_defaults_output).expect("send defaults json");
    assert_eq!(send_defaults_json["parse_mode"], "Markdown");
    let defaults_event_path = send_defaults_json["event_path"]
        .as_str()
        .expect("defaults event path");
    let defaults_events =
        std::fs::read_to_string(defaults_event_path).expect("defaults event file content");
    let defaults_last = defaults_events
        .lines()
        .last()
        .expect("defaults event line")
        .to_string();
    let defaults_last: Value = serde_json::from_str(&defaults_last).expect("defaults event json");
    assert!(
        defaults_last["text_preview"]
            .as_str()
            .expect("defaults preview")
            .contains("Default Header")
    );

    let add_retry_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "channels",
            "add",
            "--name",
            "tg-retry",
            "--kind",
            "telegram_bot",
            "--chat-id=-1000000000001",
            "--endpoint",
            "mock-http://500,500,200",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let add_retry_json: Value = serde_json::from_slice(&add_retry_output).expect("add retry json");
    let retry_channel_id = add_retry_json["channel"]["id"]
        .as_str()
        .expect("retry channel id")
        .to_string();

    let retry_send_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .env("MOSAIC_TELEGRAM_BOT_TOKEN", "test-token")
        .args([
            "--project-state",
            "--json",
            "channels",
            "send",
            &retry_channel_id,
            "--text",
            "retry telegram",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let retry_send_json: Value =
        serde_json::from_slice(&retry_send_output).expect("retry telegram json");
    assert_eq!(retry_send_json["attempts"], 3);
    assert_eq!(retry_send_json["http_status"], 200);

    let add_429_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "channels",
            "add",
            "--name",
            "tg-429",
            "--kind",
            "telegram_bot",
            "--chat-id=-1000000000002",
            "--endpoint",
            "mock-http://429,200",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let add_429_json: Value = serde_json::from_slice(&add_429_output).expect("add 429 json");
    let channel_429_id = add_429_json["channel"]["id"]
        .as_str()
        .expect("429 channel id")
        .to_string();

    let send_429_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .env("MOSAIC_TELEGRAM_BOT_TOKEN", "test-token")
        .args([
            "--project-state",
            "--json",
            "channels",
            "send",
            &channel_429_id,
            "--text",
            "tg 429 recover",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let send_429_json: Value = serde_json::from_slice(&send_429_output).expect("429 send json");
    assert_eq!(send_429_json["attempts"], 2);
    assert_eq!(send_429_json["http_status"], 200);
}

#[test]
#[allow(deprecated)]
fn channels_retry_policy_mock_http() {
    let temp = tempdir().expect("tempdir");

    let add_5xx = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "channels",
            "add",
            "--name",
            "retry-5xx",
            "--kind",
            "slack_webhook",
            "--endpoint",
            "mock-http://500,500,200",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let add_5xx: Value = serde_json::from_slice(&add_5xx).expect("add 5xx channel");
    let retry_5xx_id = add_5xx["channel"]["id"]
        .as_str()
        .expect("retry 5xx channel id")
        .to_string();

    let send_5xx = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "channels",
            "send",
            &retry_5xx_id,
            "--text",
            "retry me",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let send_5xx: Value = serde_json::from_slice(&send_5xx).expect("send 5xx");
    assert_eq!(send_5xx["attempts"], 3);
    assert_eq!(send_5xx["http_status"], 200);

    let add_4xx = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "channels",
            "add",
            "--name",
            "fail-4xx",
            "--kind",
            "slack_webhook",
            "--endpoint",
            "mock-http://429",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let add_4xx: Value = serde_json::from_slice(&add_4xx).expect("add 4xx channel");
    let fail_4xx_id = add_4xx["channel"]["id"]
        .as_str()
        .expect("fail 4xx channel id")
        .to_string();

    let fail_4xx = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "channels",
            "send",
            &fail_4xx_id,
            "--text",
            "will fail",
        ])
        .assert()
        .failure()
        .code(4)
        .get_output()
        .stdout
        .clone();
    let fail_4xx: Value = serde_json::from_slice(&fail_4xx).expect("fail 4xx json");
    assert_eq!(fail_4xx["ok"], false);
    let fail_4xx_event_path = temp
        .path()
        .join(".mosaic/data/channel-events")
        .join(format!("{fail_4xx_id}.jsonl"));
    let fail_4xx_events = std::fs::read_to_string(&fail_4xx_event_path).expect("4xx event file");
    let fail_4xx_last = fail_4xx_events
        .lines()
        .last()
        .expect("4xx event line")
        .to_string();
    let fail_4xx_last: Value = serde_json::from_str(&fail_4xx_last).expect("4xx event json");
    assert_eq!(fail_4xx_last["attempt"], 1);
    assert_eq!(fail_4xx_last["http_status"], 429);
    assert_eq!(fail_4xx_last["delivery_status"], "failed");

    let list_after_4xx = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args(["--project-state", "--json", "channels", "list"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let list_after_4xx: Value = serde_json::from_slice(&list_after_4xx).expect("list after 4xx");
    let failed_channel = list_after_4xx["channels"]
        .as_array()
        .expect("channels array")
        .iter()
        .find(|channel| channel["id"].as_str() == Some(fail_4xx_id.as_str()))
        .expect("failed channel in list");
    assert!(failed_channel["last_error"].is_string());

    let add_timeout = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "channels",
            "add",
            "--name",
            "retry-timeout",
            "--kind",
            "slack_webhook",
            "--endpoint",
            "mock-http://timeout,timeout,200",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let add_timeout: Value = serde_json::from_slice(&add_timeout).expect("add timeout channel");
    let retry_timeout_id = add_timeout["channel"]["id"]
        .as_str()
        .expect("timeout channel id")
        .to_string();

    let send_timeout = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "channels",
            "send",
            &retry_timeout_id,
            "--text",
            "timeout retry",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let send_timeout: Value = serde_json::from_slice(&send_timeout).expect("timeout send");
    assert_eq!(send_timeout["attempts"], 3);
    assert_eq!(send_timeout["http_status"], 200);
}

#[test]
#[allow(deprecated)]
fn gateway_probe_discover_call_flow() {
    let temp = tempdir().expect("tempdir");

    Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .env("MOSAIC_GATEWAY_TEST_MODE", "1")
        .args([
            "--project-state",
            "--json",
            "gateway",
            "run",
            "--host",
            "127.0.0.1",
            "--port",
            "8787",
        ])
        .assert()
        .success();

    let probe_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .env("MOSAIC_GATEWAY_TEST_MODE", "1")
        .args(["--project-state", "--json", "gateway", "probe"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let probe_json: Value = serde_json::from_slice(&probe_output).expect("probe json");
    assert_eq!(probe_json["ok"], true);
    assert_eq!(probe_json["probe"]["ok"], true);

    let discover_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .env("MOSAIC_GATEWAY_TEST_MODE", "1")
        .args(["--project-state", "--json", "gateway", "discover"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let discover_json: Value = serde_json::from_slice(&discover_output).expect("discover json");
    let methods = discover_json["discovery"]["methods"]
        .as_array()
        .expect("methods");
    assert!(methods.iter().any(|value| value.as_str() == Some("status")));

    let call_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .env("MOSAIC_GATEWAY_TEST_MODE", "1")
        .args(["--project-state", "--json", "gateway", "call", "status"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let call_json: Value = serde_json::from_slice(&call_output).expect("call json");
    assert_eq!(call_json["ok"], true);
    assert_eq!(call_json["data"]["service"], "mosaic-gateway");

    Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .env("MOSAIC_GATEWAY_TEST_MODE", "1")
        .args(["--project-state", "--json", "gateway", "stop"])
        .assert()
        .success();
}

#[test]
#[allow(deprecated)]
fn channels_ops_commands_flow() {
    let temp = tempdir().expect("tempdir");

    let add_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "channels",
            "add",
            "--name",
            "ops-alerts",
            "--kind",
            "slack",
            "--endpoint",
            "mock-http://200",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let add_json: Value = serde_json::from_slice(&add_output).expect("add json");
    let channel_id = add_json["channel"]["id"]
        .as_str()
        .expect("channel id")
        .to_string();

    let update_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "channels",
            "update",
            &channel_id,
            "--name",
            "ops-alerts-v2",
            "--default-title",
            "Ops Header",
            "--default-block",
            "region=us-east-1",
            "--default-metadata",
            "{\"service\":\"mosaic\"}",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let update_json: Value = serde_json::from_slice(&update_output).expect("update json");
    assert_eq!(update_json["channel"]["name"], "ops-alerts-v2");
    assert_eq!(update_json["channel"]["has_template_defaults"], true);

    let send_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "channels",
            "send",
            &channel_id,
            "--text",
            "ops message",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let send_json: Value = serde_json::from_slice(&send_output).expect("send json");
    assert_eq!(send_json["ok"], true);
    let event_path = send_json["event_path"].as_str().expect("event path");
    let event_text = std::fs::read_to_string(event_path).expect("event file");
    let last_line = event_text.lines().last().expect("event line");
    let last_event: Value = serde_json::from_str(last_line).expect("event json");
    let preview = last_event["text_preview"].as_str().expect("preview");
    assert!(preview.contains("Ops Header"));
    assert!(preview.contains("region=us-east-1"));

    Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "channels",
            "update",
            &channel_id,
            "--clear-defaults",
        ])
        .assert()
        .success();

    let status_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args(["--project-state", "--json", "channels", "status"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let status_json: Value = serde_json::from_slice(&status_output).expect("status json");
    assert_eq!(status_json["status"]["total_channels"], 1);

    let logs_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "channels",
            "logs",
            "--channel",
            &channel_id,
            "--tail",
            "10",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let logs_json: Value = serde_json::from_slice(&logs_output).expect("logs json");
    assert!(
        logs_json["events"]
            .as_array()
            .expect("events")
            .iter()
            .any(|event| event["channel_id"].as_str() == Some(channel_id.as_str()))
    );

    let capabilities_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "channels",
            "capabilities",
            "--target",
            &channel_id,
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let capabilities_json: Value =
        serde_json::from_slice(&capabilities_output).expect("capabilities json");
    assert_eq!(
        capabilities_json["capabilities"][0]["kind"],
        "slack_webhook"
    );

    let resolve_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "channels",
            "resolve",
            "--channel",
            "slack",
            "ops",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let resolve_json: Value = serde_json::from_slice(&resolve_output).expect("resolve json");
    assert_eq!(resolve_json["entries"][0]["id"], channel_id);

    Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "channels",
            "logout",
            &channel_id,
        ])
        .assert()
        .success();

    Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "channels",
            "remove",
            &channel_id,
        ])
        .assert()
        .success();

    let list_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args(["--project-state", "--json", "channels", "list"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let list_json: Value = serde_json::from_slice(&list_output).expect("list json");
    assert_eq!(
        list_json["channels"]
            .as_array()
            .expect("channels array")
            .len(),
        0
    );
}

#[test]
#[allow(deprecated)]
fn channels_export_import_flow() {
    let src = tempdir().expect("source tempdir");
    let dst = tempdir().expect("destination tempdir");
    let export_path = src.path().join("channels-export.json");

    let add_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(src.path())
        .args([
            "--project-state",
            "--json",
            "channels",
            "add",
            "--name",
            "src-slack",
            "--kind",
            "slack_webhook",
            "--endpoint",
            "mock-http://200",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let add_json: Value = serde_json::from_slice(&add_output).expect("add json");
    let channel_id = add_json["channel"]["id"]
        .as_str()
        .expect("channel id")
        .to_string();

    Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(src.path())
        .args([
            "--project-state",
            "--json",
            "channels",
            "export",
            "--out",
            export_path.to_str().expect("export path str"),
        ])
        .assert()
        .success();
    let exported_raw = std::fs::read_to_string(&export_path).expect("read export");
    let mut exported_json: Value = serde_json::from_str(&exported_raw).expect("parse export");
    assert_eq!(exported_json["schema"], "mosaic.channels.export.v1");
    assert_eq!(
        exported_json["channels_file"]["channels"]
            .as_array()
            .expect("channels array")
            .len(),
        1
    );

    let first_import = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(dst.path())
        .args([
            "--project-state",
            "--json",
            "channels",
            "import",
            "--file",
            export_path.to_str().expect("import path str"),
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let first_import: Value = serde_json::from_slice(&first_import).expect("first import json");
    assert_eq!(first_import["summary"]["imported"], 1);
    assert_eq!(first_import["summary"]["updated"], 0);
    assert_eq!(first_import["summary"]["skipped"], 0);
    assert_eq!(first_import["summary"]["dry_run"], false);

    let second_import = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(dst.path())
        .args([
            "--project-state",
            "--json",
            "channels",
            "import",
            "--file",
            export_path.to_str().expect("import path str"),
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let second_import: Value = serde_json::from_slice(&second_import).expect("second import json");
    assert_eq!(second_import["summary"]["imported"], 0);
    assert_eq!(second_import["summary"]["updated"], 0);
    assert_eq!(second_import["summary"]["skipped"], 1);
    assert_eq!(second_import["summary"]["dry_run"], false);

    exported_json["channels_file"]["channels"][0]["endpoint"] =
        Value::String("mock-http://500".to_string());
    let updated_export =
        serde_json::to_string_pretty(&exported_json).expect("serialize updated export");
    std::fs::write(&export_path, updated_export).expect("write updated export");

    let dry_run_import = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(dst.path())
        .args([
            "--project-state",
            "--json",
            "channels",
            "import",
            "--file",
            export_path.to_str().expect("import path str"),
            "--replace",
            "--dry-run",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let dry_run_import: Value =
        serde_json::from_slice(&dry_run_import).expect("dry run import json");
    assert_eq!(dry_run_import["summary"]["imported"], 0);
    assert_eq!(dry_run_import["summary"]["updated"], 1);
    assert_eq!(dry_run_import["summary"]["skipped"], 0);
    assert_eq!(dry_run_import["summary"]["dry_run"], true);

    let send_after_dry_run = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(dst.path())
        .args([
            "--project-state",
            "--json",
            "channels",
            "send",
            &channel_id,
            "--text",
            "check dry-run endpoint",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let send_after_dry_run: Value =
        serde_json::from_slice(&send_after_dry_run).expect("send after dry run json");
    assert_eq!(send_after_dry_run["ok"], true);

    let replace_import = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(dst.path())
        .args([
            "--project-state",
            "--json",
            "channels",
            "import",
            "--file",
            export_path.to_str().expect("import path str"),
            "--replace",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let replace_import: Value =
        serde_json::from_slice(&replace_import).expect("replace import json");
    assert_eq!(replace_import["summary"]["imported"], 0);
    assert_eq!(replace_import["summary"]["updated"], 1);
    assert_eq!(replace_import["summary"]["skipped"], 0);
    assert_eq!(replace_import["summary"]["dry_run"], false);

    let send = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(dst.path())
        .args([
            "--project-state",
            "--json",
            "channels",
            "send",
            &channel_id,
            "--text",
            "check replaced endpoint",
        ])
        .assert()
        .failure()
        .code(4)
        .get_output()
        .stdout
        .clone();
    let send_json: Value = serde_json::from_slice(&send).expect("send json");
    assert_eq!(send_json["error"]["code"], "network");
}
