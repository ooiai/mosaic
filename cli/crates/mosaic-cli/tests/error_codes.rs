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
