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
