use assert_cmd::Command;
use serde_json::Value;
use tempfile::tempdir;

#[test]
#[allow(deprecated)]
fn ask_without_setup_returns_config_exit_code() {
    let temp = tempdir().expect("tempdir");
    Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args(["--project-state", "ask", "hello"])
        .assert()
        .failure()
        .code(2);
}

#[test]
#[allow(deprecated)]
fn channels_add_with_unsupported_kind_returns_channel_unsupported_exit_code() {
    let temp = tempdir().expect("tempdir");
    let output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "channels",
            "add",
            "--name",
            "bad-kind",
            "--kind",
            "unknown_kind",
        ])
        .assert()
        .failure()
        .code(10)
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output).expect("json output");
    assert_eq!(json["ok"], false);
    assert_eq!(json["error"]["code"], "channel_unsupported");
}

#[test]
#[allow(deprecated)]
fn channels_add_telegram_without_chat_id_returns_validation_exit_code() {
    let temp = tempdir().expect("tempdir");
    let output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "channels",
            "add",
            "--name",
            "tg-missing-chat",
            "--kind",
            "telegram_bot",
        ])
        .assert()
        .failure()
        .code(7)
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output).expect("json output");
    assert_eq!(json["ok"], false);
    assert_eq!(json["error"]["code"], "validation");
}

#[test]
#[allow(deprecated)]
fn channels_send_telegram_without_token_returns_auth_exit_code() {
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
            "tg-auth",
            "--kind",
            "telegram_bot",
            "--chat-id=-1001234",
            "--endpoint",
            "mock-http://200",
            "--token-env",
            "MOSAIC_TEST_MISSING_TELEGRAM_TOKEN",
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

    let output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "channels",
            "send",
            &channel_id,
            "--text",
            "hello",
        ])
        .assert()
        .failure()
        .code(3)
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output).expect("json output");
    assert_eq!(json["ok"], false);
    assert_eq!(json["error"]["code"], "auth");
}

#[test]
#[allow(deprecated)]
fn channels_send_parse_mode_on_non_telegram_returns_validation_exit_code() {
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
            "term",
            "--kind",
            "terminal",
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

    let output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "channels",
            "send",
            &channel_id,
            "--text",
            "hello",
            "--parse-mode",
            "markdown",
        ])
        .assert()
        .failure()
        .code(7)
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output).expect("json output");
    assert_eq!(json["ok"], false);
    assert_eq!(json["error"]["code"], "validation");
}

#[test]
#[allow(deprecated)]
fn gateway_call_unknown_method_returns_gateway_protocol_exit_code() {
    let temp = tempdir().expect("tempdir");
    Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .env("MOSAIC_GATEWAY_TEST_MODE", "1")
        .args(["--project-state", "--json", "gateway", "run"])
        .assert()
        .success();

    let output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .env("MOSAIC_GATEWAY_TEST_MODE", "1")
        .args([
            "--project-state",
            "--json",
            "gateway",
            "call",
            "unknown_method",
        ])
        .assert()
        .failure()
        .code(9)
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output).expect("json output");
    assert_eq!(json["ok"], false);
    assert_eq!(json["error"]["code"], "gateway_protocol");
}
