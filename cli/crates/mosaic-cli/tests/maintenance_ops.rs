use std::fs;

use assert_cmd::Command;
use serde_json::Value;
use tempfile::tempdir;

#[test]
#[allow(deprecated)]
fn update_without_check_reports_current_version() {
    let output = Command::cargo_bin("mosaic")
        .expect("binary")
        .args(["--json", "update"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output).expect("json");
    assert_eq!(json["ok"], true);
    assert_eq!(json["checked"], false);
    assert!(json["current_version"].is_string());
}

#[test]
#[allow(deprecated)]
fn update_check_uses_mock_source() {
    let output = Command::cargo_bin("mosaic")
        .expect("binary")
        .args(["--json", "update", "--check", "--source", "mock://v9.9.9"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output).expect("json");
    assert_eq!(json["ok"], true);
    assert_eq!(json["checked"], true);
    assert_eq!(json["latest_version"], "v9.9.9");
    assert_eq!(json["source"], "mock://v9.9.9");
    assert_eq!(json["update_available"], true);
}

#[test]
#[allow(deprecated)]
fn reset_requires_yes() {
    let output = Command::cargo_bin("mosaic")
        .expect("binary")
        .args(["--json", "reset"])
        .assert()
        .failure()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output).expect("json");
    assert_eq!(json["ok"], false);
    assert_eq!(json["error"]["code"], "approval_required");
}

#[test]
#[allow(deprecated)]
fn uninstall_requires_yes() {
    let output = Command::cargo_bin("mosaic")
        .expect("binary")
        .args(["--json", "uninstall"])
        .assert()
        .failure()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output).expect("json");
    assert_eq!(json["ok"], false);
    assert_eq!(json["error"]["code"], "approval_required");
}

#[test]
#[allow(deprecated)]
fn reset_with_yes_clears_state_and_reinitializes() {
    let temp = tempdir().expect("tempdir");

    Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "setup",
            "--base-url",
            "mock://mock-model",
            "--model",
            "mock-model",
        ])
        .assert()
        .success();

    Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .env("MOSAIC_MOCK_CHAT_RESPONSE", "reset-seed")
        .args(["--project-state", "ask", "seed state"])
        .assert()
        .success();

    let output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args(["--project-state", "--yes", "--json", "reset"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output).expect("json");
    assert_eq!(json["ok"], true);
    assert_eq!(json["reinitialized"], true);

    let state_root = temp.path().join(".mosaic");
    assert!(state_root.exists());
    assert!(state_root.join("data").exists());

    let status = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args(["--project-state", "--json", "status"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let status_json: Value = serde_json::from_slice(&status).expect("status json");
    assert_eq!(status_json["ok"], true);
    assert_eq!(status_json["configured"], false);
}

#[test]
#[allow(deprecated)]
fn uninstall_with_yes_removes_project_state_root() {
    let temp = tempdir().expect("tempdir");

    Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "setup",
            "--base-url",
            "mock://mock-model",
            "--model",
            "mock-model",
        ])
        .assert()
        .success();

    let state_root = temp.path().join(".mosaic");
    assert!(state_root.exists());

    let output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args(["--project-state", "--yes", "--json", "uninstall"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output).expect("json");
    assert_eq!(json["ok"], true);
    assert_eq!(json["mode"], "project");
    assert!(!state_root.exists());

    let removed = json["removed_dirs"].as_array().expect("removed dirs array");
    assert!(
        removed
            .iter()
            .any(|item| item.as_str().unwrap_or_default().ends_with("/.mosaic")),
        "removed_dirs should include project .mosaic root"
    );

    let result = fs::metadata(&state_root);
    assert!(result.is_err());
}
