use assert_cmd::Command;
use serde_json::Value;
use tempfile::tempdir;

#[test]
#[allow(deprecated)]
fn approvals_and_sandbox_commands_json_contract() {
    let temp = tempdir().expect("tempdir");

    let approvals_get = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args(["--project-state", "--json", "approvals", "get"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let approvals_get: Value = serde_json::from_slice(&approvals_get).expect("approvals get");
    assert_eq!(approvals_get["ok"], true);
    assert_eq!(approvals_get["policy"]["mode"], "confirm");

    let approvals_set = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args(["--project-state", "--json", "approvals", "set", "deny"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let approvals_set: Value = serde_json::from_slice(&approvals_set).expect("approvals set");
    assert_eq!(approvals_set["policy"]["mode"], "deny");

    let approvals_allowlist = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "approvals",
            "allowlist",
            "add",
            "cargo test",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let approvals_allowlist: Value =
        serde_json::from_slice(&approvals_allowlist).expect("approvals allowlist add");
    assert!(
        approvals_allowlist["policy"]["allowlist"]
            .as_array()
            .expect("allowlist")
            .iter()
            .any(|item| item.as_str() == Some("cargo test"))
    );

    let sandbox_list = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args(["--project-state", "--json", "sandbox", "list"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let sandbox_list: Value = serde_json::from_slice(&sandbox_list).expect("sandbox list");
    assert_eq!(sandbox_list["ok"], true);
    assert!(sandbox_list["profiles"].is_array());

    let sandbox_explain = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "sandbox",
            "explain",
            "--profile",
            "restricted",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let sandbox_explain: Value = serde_json::from_slice(&sandbox_explain).expect("sandbox explain");
    assert_eq!(sandbox_explain["profile"]["profile"], "restricted");
}

#[test]
#[allow(deprecated)]
fn system_event_and_logs_flow() {
    let temp = tempdir().expect("tempdir");

    let system_event = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "system",
            "event",
            "deploy_start",
            "--data",
            "{\"env\":\"ci\"}",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let system_event: Value = serde_json::from_slice(&system_event).expect("system event");
    assert_eq!(system_event["ok"], true);
    assert_eq!(system_event["event"]["name"], "deploy_start");

    let logs_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args(["--project-state", "--json", "logs", "--tail", "20"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let logs_json: Value = serde_json::from_slice(&logs_output).expect("logs");
    assert_eq!(logs_json["ok"], true);
    assert!(
        logs_json["logs"]
            .as_array()
            .expect("logs array")
            .iter()
            .any(|entry| entry["source"].as_str() == Some("system"))
    );
}
