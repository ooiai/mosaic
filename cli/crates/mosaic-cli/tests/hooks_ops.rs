use assert_cmd::Command;
use serde_json::Value;
use tempfile::tempdir;

fn setup_project(temp: &tempfile::TempDir) {
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
}

#[test]
#[allow(deprecated)]
fn hooks_add_run_and_system_event_flow() {
    let temp = tempdir().expect("tempdir");
    setup_project(&temp);

    let add_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "hooks",
            "add",
            "--name",
            "notify-deploy",
            "--event",
            "deploy",
            "--command",
            "echo hook-ok",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let add_json: Value = serde_json::from_slice(&add_output).expect("add json");
    assert_eq!(add_json["ok"], true);
    let hook_id = add_json["hook"]["id"]
        .as_str()
        .expect("hook id")
        .to_string();

    let run_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--yes",
            "--json",
            "hooks",
            "run",
            &hook_id,
            "--data",
            "{\"source\":\"manual\"}",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let run_json: Value = serde_json::from_slice(&run_output).expect("run json");
    assert_eq!(run_json["ok"], true);
    assert_eq!(run_json["result"]["ok"], true);
    assert_eq!(run_json["result"]["exit_code"], 0);

    let event_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--yes",
            "--json",
            "system",
            "event",
            "deploy",
            "--data",
            "{\"version\":\"1.0.0\"}",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let event_json: Value = serde_json::from_slice(&event_output).expect("event json");
    assert_eq!(event_json["ok"], true);
    assert_eq!(event_json["hooks"]["triggered"], 1);
    assert_eq!(event_json["hooks"]["ok"], 1);
    assert_eq!(event_json["hooks"]["failed"], 0);

    let list_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args(["--project-state", "--json", "hooks", "list"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let list_json: Value = serde_json::from_slice(&list_output).expect("list json");
    assert_eq!(list_json["ok"], true);
    let hooks = list_json["hooks"].as_array().expect("hooks array");
    assert_eq!(hooks.len(), 1);
    assert_eq!(hooks[0]["id"], hook_id);
    assert_eq!(hooks[0]["last_result"]["ok"], true);
    assert!(hooks[0]["last_triggered_at"].is_string());

    let logs_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "hooks",
            "logs",
            "--hook",
            &hook_id,
            "--tail",
            "10",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let logs_json: Value = serde_json::from_slice(&logs_output).expect("logs json");
    assert_eq!(logs_json["ok"], true);
    let events = logs_json["events"].as_array().expect("events array");
    assert!(events.len() >= 2);
    assert!(
        events
            .iter()
            .any(|item| item["trigger"].as_str() == Some("manual"))
    );
    assert!(
        events
            .iter()
            .any(|item| item["trigger"].as_str() == Some("system_event"))
    );
}

#[test]
#[allow(deprecated)]
fn hooks_run_without_yes_returns_approval_required() {
    let temp = tempdir().expect("tempdir");
    setup_project(&temp);

    let add_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "hooks",
            "add",
            "--name",
            "approval-check",
            "--event",
            "deploy",
            "--command",
            "echo need-approval",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let add_json: Value = serde_json::from_slice(&add_output).expect("add json");
    let hook_id = add_json["hook"]["id"].as_str().expect("hook id");

    let error_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args(["--project-state", "--json", "hooks", "run", hook_id])
        .assert()
        .failure()
        .code(11)
        .get_output()
        .stdout
        .clone();
    let error_json: Value = serde_json::from_slice(&error_output).expect("error json");
    assert_eq!(error_json["ok"], false);
    assert_eq!(error_json["error"]["code"], "approval_required");
}

#[test]
#[allow(deprecated)]
fn disabled_hooks_are_not_triggered_by_system_event() {
    let temp = tempdir().expect("tempdir");
    setup_project(&temp);

    Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "hooks",
            "add",
            "--name",
            "disabled-hook",
            "--event",
            "deploy",
            "--command",
            "echo disabled",
            "--disabled",
        ])
        .assert()
        .success();

    let event_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--yes",
            "--json",
            "system",
            "event",
            "deploy",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let event_json: Value = serde_json::from_slice(&event_output).expect("event json");
    assert_eq!(event_json["ok"], true);
    assert_eq!(event_json["hooks"]["triggered"], 0);
}
