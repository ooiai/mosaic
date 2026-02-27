use std::fs;

use assert_cmd::Command;
use serde_json::Value;
use tempfile::tempdir;

#[test]
#[allow(deprecated)]
fn completion_shell_outputs_script() {
    let output = Command::cargo_bin("mosaic")
        .expect("binary")
        .args(["completion", "shell", "bash"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let script = String::from_utf8(output).expect("utf8");
    assert!(script.contains("_mosaic"));
    assert!(script.contains("complete"));
}

#[test]
#[allow(deprecated)]
fn completion_install_writes_script_file() {
    let temp = tempdir().expect("tempdir");
    let dir = temp.path().join("completions");

    let output = Command::cargo_bin("mosaic")
        .expect("binary")
        .args([
            "--json",
            "completion",
            "install",
            "zsh",
            "--dir",
            dir.to_str().expect("dir str"),
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output).expect("json");
    assert_eq!(json["ok"], true);
    let path = json["path"].as_str().expect("path");
    assert!(
        path.ends_with("/_mosaic"),
        "unexpected completion path: {path}"
    );
    let content = fs::read_to_string(path).expect("completion file");
    assert!(content.contains("#compdef mosaic"));
}

#[test]
#[allow(deprecated)]
fn directory_reports_project_state_paths_in_json() {
    let temp = tempdir().expect("tempdir");

    let output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args(["--project-state", "--json", "directory"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output).expect("json");
    assert_eq!(json["ok"], true);
    assert_eq!(json["mode"], "project");
    let config_path = json["paths"]["config_path"].as_str().expect("config_path");
    assert!(config_path.ends_with(".mosaic/config.toml"));
}

#[test]
#[allow(deprecated)]
fn dashboard_reports_operational_panels() {
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

    fs::write(temp.path().join("memory-doc.txt"), "dashboard memory test").expect("write doc");

    Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "memory",
            "index",
            "--path",
            ".",
            "--max-files",
            "32",
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
            "add",
            "--name",
            "term",
            "--kind",
            "terminal",
        ])
        .assert()
        .success();

    let dashboard_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args(["--project-state", "--json", "dashboard"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let dashboard_json: Value = serde_json::from_slice(&dashboard_output).expect("dashboard json");
    assert_eq!(dashboard_json["ok"], true);
    assert_eq!(dashboard_json["configured"], true);
    assert_eq!(dashboard_json["dashboard"]["config"]["configured"], true);
    assert_eq!(
        dashboard_json["dashboard"]["config"]["model"],
        Value::String("mock-model".to_string())
    );
    assert_eq!(dashboard_json["dashboard"]["channels"]["total"], 1);
    assert!(
        dashboard_json["dashboard"]["memory"]["indexed_documents"]
            .as_u64()
            .expect("memory indexed_documents")
            >= 1
    );
    assert!(dashboard_json["dashboard"]["presence"]["hostname"].is_string());
    assert!(
        dashboard_json["warnings"]
            .as_array()
            .expect("warnings array")
            .is_empty()
    );
}
