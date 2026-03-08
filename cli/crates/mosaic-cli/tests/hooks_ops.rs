use assert_cmd::Command;
use serde_json::Value;
use std::fs;
use tempfile::tempdir;

#[allow(deprecated)]
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

#[test]
#[allow(deprecated)]
fn hooks_logs_summary_and_since_minutes_filter() {
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
            "summary-hook",
            "--event",
            "deploy",
            "--command",
            "echo summary-hook",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let add_json: Value = serde_json::from_slice(&add_output).expect("add json");
    let hook_id = add_json["hook"]["id"]
        .as_str()
        .expect("hook id")
        .to_string();

    Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--yes",
            "--json",
            "hooks",
            "run",
            &hook_id,
        ])
        .assert()
        .success();

    let summary_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "hooks",
            "logs",
            "--hook",
            &hook_id,
            "--summary",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let summary_json: Value = serde_json::from_slice(&summary_output).expect("summary json");
    assert_eq!(summary_json["ok"], true);
    assert_eq!(summary_json["summary"]["total"], 1);
    assert_eq!(summary_json["summary"]["ok"], 1);
    assert_eq!(summary_json["summary"]["failed"], 0);
    assert_eq!(summary_json["summary"]["by_trigger"]["manual"], 1);

    let events_path = temp
        .path()
        .join(".mosaic/data/hook-events")
        .join(format!("{hook_id}.jsonl"));
    let raw = fs::read_to_string(&events_path).expect("read hook events");
    let mut lines = raw
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).expect("parse hook event"))
        .collect::<Vec<_>>();
    lines[0]["ts"] = Value::String("2000-01-01T00:00:00Z".to_string());
    let rewritten = format!(
        "{}\n",
        lines
            .into_iter()
            .map(|item| serde_json::to_string(&item).expect("encode hook event"))
            .collect::<Vec<_>>()
            .join("\n")
    );
    fs::write(&events_path, rewritten).expect("write hook events");

    let since_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "hooks",
            "logs",
            "--hook",
            &hook_id,
            "--summary",
            "--since-minutes",
            "1",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let since_json: Value = serde_json::from_slice(&since_output).expect("since json");
    assert_eq!(since_json["ok"], true);
    assert_eq!(since_json["summary"]["total"], 0);
    assert_eq!(
        since_json["events"].as_array().expect("events array").len(),
        0
    );
}

#[test]
#[allow(deprecated)]
fn hooks_replay_plan_and_apply_flow() {
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
            "replay-hook",
            "--event",
            "deploy",
            "--command",
            "false",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let add_json: Value = serde_json::from_slice(&add_output).expect("add json");
    let hook_id = add_json["hook"]["id"]
        .as_str()
        .expect("hook id")
        .to_string();

    Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--yes",
            "--json",
            "hooks",
            "run",
            &hook_id,
        ])
        .assert()
        .failure();
    Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--yes",
            "--json",
            "hooks",
            "run",
            &hook_id,
        ])
        .assert()
        .failure();

    let replay_plan_report = temp.path().join("hooks-replay-plan.json");
    let replay_plan_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "hooks",
            "replay",
            "--hook",
            &hook_id,
            "--tail",
            "20",
            "--limit",
            "5",
            "--batch-size",
            "1",
            "--reason",
            "tool",
            "--retryable-only",
            "--report-out",
            replay_plan_report.to_string_lossy().as_ref(),
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let replay_plan_json: Value = serde_json::from_slice(&replay_plan_output).expect("replay json");
    assert_eq!(replay_plan_json["ok"], true);
    assert_eq!(replay_plan_json["apply"], false);
    assert_eq!(replay_plan_json["selected_candidates"], 2);
    assert_eq!(
        replay_plan_json["batch_plan"]
            .as_array()
            .expect("batch plan")
            .len(),
        2
    );
    assert_eq!(
        replay_plan_json["recovery_diagnostics"]["selected_candidates"],
        2
    );
    assert_eq!(
        replay_plan_json["recovery_diagnostics"]["retryable_candidates"],
        2
    );
    assert_eq!(
        replay_plan_json["recovery_diagnostics"]["non_retryable_candidates"],
        0
    );
    assert_eq!(
        replay_plan_json["recovery_diagnostics"]["suggested_strategy"],
        "safe_to_apply"
    );
    assert_eq!(replay_plan_json["candidates"][0]["reason"], "tool");
    assert_eq!(replay_plan_json["candidates"][0]["retryable"], true);
    let replay_plan_report_json: Value = serde_json::from_str(
        &fs::read_to_string(&replay_plan_report).expect("read replay plan report"),
    )
    .expect("parse replay plan report");
    assert_eq!(replay_plan_report_json["selected_candidates"], 2);

    let hooks_path = temp.path().join(".mosaic/data/hooks.json");
    let hooks_raw = fs::read_to_string(&hooks_path).expect("read hooks");
    let mut hooks_json: Value = serde_json::from_str(&hooks_raw).expect("parse hooks");
    hooks_json[0]["command"] = Value::String("echo replay-hook-ok".to_string());
    fs::write(
        &hooks_path,
        serde_json::to_string_pretty(&hooks_json).expect("encode hooks"),
    )
    .expect("write hooks");

    let replay_apply_report = temp.path().join("hooks-replay-apply.json");
    let replay_apply_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--yes",
            "--json",
            "hooks",
            "replay",
            "--hook",
            &hook_id,
            "--tail",
            "20",
            "--limit",
            "5",
            "--batch-size",
            "1",
            "--apply",
            "--max-apply",
            "1",
            "--report-out",
            replay_apply_report.to_string_lossy().as_ref(),
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let replay_apply_json: Value =
        serde_json::from_slice(&replay_apply_output).expect("replay apply json");
    assert_eq!(replay_apply_json["ok"], true);
    assert_eq!(replay_apply_json["apply"], true);
    assert_eq!(replay_apply_json["planned_attempts"], 1);
    assert_eq!(replay_apply_json["attempted"], 1);
    assert_eq!(replay_apply_json["succeeded"], 1);
    assert_eq!(replay_apply_json["failed"], 0);
    assert_eq!(replay_apply_json["skipped_due_to_apply_limit"], 1);
    assert_eq!(
        replay_apply_json["recovery_diagnostics"]["selected_candidates"],
        2
    );
    let replay_apply_report_json: Value = serde_json::from_str(
        &fs::read_to_string(&replay_apply_report).expect("read replay apply report"),
    )
    .expect("parse replay apply report");
    assert_eq!(replay_apply_report_json["attempted"], 1);

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
            "20",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let logs_json: Value = serde_json::from_slice(&logs_output).expect("logs json");
    let events = logs_json["events"].as_array().expect("events array");
    assert!(
        events
            .iter()
            .any(|item| item["trigger"].as_str() == Some("replay"))
    );
}
