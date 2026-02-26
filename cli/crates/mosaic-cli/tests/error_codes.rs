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
fn channels_add_default_parse_mode_on_non_telegram_returns_validation_exit_code() {
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
            "term-default-parse",
            "--kind",
            "terminal",
            "--default-parse-mode",
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
fn channels_update_without_fields_returns_validation_exit_code() {
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
            "slack-updatable",
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

    let output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "channels",
            "update",
            &channel_id,
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
fn channels_update_default_parse_mode_on_non_telegram_returns_validation_exit_code() {
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
            "term-updatable",
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
            "update",
            &channel_id,
            "--default-parse-mode",
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
fn channels_import_invalid_json_returns_validation_exit_code() {
    let temp = tempdir().expect("tempdir");
    let broken = temp.path().join("broken-channels.json");
    std::fs::write(&broken, "{not-valid-json").expect("write broken json");

    let output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "channels",
            "import",
            "--file",
            broken.to_str().expect("broken path"),
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
fn channels_rotate_token_env_without_target_returns_validation_exit_code() {
    let temp = tempdir().expect("tempdir");
    let output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "channels",
            "rotate-token-env",
            "--to",
            "MOSAIC_ROTATED_TOKEN",
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
fn channels_rotate_token_env_with_empty_from_returns_validation_exit_code() {
    let temp = tempdir().expect("tempdir");
    let output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "channels",
            "rotate-token-env",
            "--all",
            "--from",
            "",
            "--to",
            "MOSAIC_ROTATED_TOKEN",
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

#[test]
#[allow(deprecated)]
fn gateway_call_without_runtime_returns_gateway_unavailable_exit_code() {
    let temp = tempdir().expect("tempdir");
    let output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .env("MOSAIC_GATEWAY_TEST_MODE", "1")
        .args(["--project-state", "--json", "gateway", "call", "status"])
        .assert()
        .failure()
        .code(8)
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output).expect("json output");
    assert_eq!(json["ok"], false);
    assert_eq!(json["error"]["code"], "gateway_unavailable");
}

#[test]
#[allow(deprecated)]
fn nodes_run_without_yes_returns_approval_required_exit_code() {
    let temp = tempdir().expect("tempdir");
    let output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "nodes",
            "run",
            "local",
            "--command",
            "echo hello",
        ])
        .assert()
        .failure()
        .code(11)
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output).expect("json output");
    assert_eq!(json["ok"], false);
    assert_eq!(json["error"]["code"], "approval_required");
}

#[test]
#[allow(deprecated)]
fn nodes_run_with_restricted_sandbox_returns_sandbox_denied_exit_code() {
    let temp = tempdir().expect("tempdir");
    let policy_dir = temp.path().join(".mosaic/policy");
    std::fs::create_dir_all(&policy_dir).expect("create policy dir");
    std::fs::write(
        policy_dir.join("sandbox.toml"),
        "version = 1\nprofile = \"restricted\"\n",
    )
    .expect("write sandbox policy");

    let output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "nodes",
            "run",
            "local",
            "--command",
            "curl https://example.com",
        ])
        .assert()
        .failure()
        .code(12)
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output).expect("json output");
    assert_eq!(json["ok"], false);
    assert_eq!(json["error"]["code"], "sandbox_denied");
}

#[test]
#[allow(deprecated)]
fn agents_show_missing_returns_validation_exit_code() {
    let temp = tempdir().expect("tempdir");
    let output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args(["--project-state", "--json", "agents", "show", "missing-agent"])
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
fn security_audit_missing_path_returns_validation_exit_code() {
    let temp = tempdir().expect("tempdir");
    let output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "security",
            "audit",
            "--path",
            "missing-security-target",
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
fn reset_without_yes_returns_approval_required_exit_code() {
    let temp = tempdir().expect("tempdir");
    let output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args(["--project-state", "--json", "reset"])
        .assert()
        .failure()
        .code(11)
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output).expect("json output");
    assert_eq!(json["ok"], false);
    assert_eq!(json["error"]["code"], "approval_required");
}

#[test]
#[allow(deprecated)]
fn uninstall_without_yes_returns_approval_required_exit_code() {
    let temp = tempdir().expect("tempdir");
    let output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args(["--project-state", "--json", "uninstall"])
        .assert()
        .failure()
        .code(11)
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output).expect("json output");
    assert_eq!(json["ok"], false);
    assert_eq!(json["error"]["code"], "approval_required");
}

#[test]
#[allow(deprecated)]
fn docs_unknown_topic_returns_validation_exit_code() {
    let temp = tempdir().expect("tempdir");
    let output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args(["--json", "docs", "unknown-topic"])
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
fn dns_unresolvable_host_returns_network_exit_code() {
    let temp = tempdir().expect("tempdir");
    let output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args(["--json", "dns", "resolve", "nonexistent.invalid"])
        .assert()
        .failure()
        .code(4)
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output).expect("json output");
    assert_eq!(json["ok"], false);
    assert_eq!(json["error"]["code"], "network");
}

#[test]
#[allow(deprecated)]
fn qr_png_without_output_returns_validation_exit_code() {
    let temp = tempdir().expect("tempdir");
    let output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args(["--json", "qr", "encode", "hello", "--render", "png"])
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
