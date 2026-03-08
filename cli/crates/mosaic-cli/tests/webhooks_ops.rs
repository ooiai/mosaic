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
fn webhooks_add_resolve_and_logs_flow() {
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
            "deploy-hook",
            "--event",
            "deploy",
            "--command",
            "echo webhook-hook-ok",
        ])
        .assert()
        .success();

    let add_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "webhooks",
            "add",
            "--name",
            "deploy-webhook",
            "--event",
            "deploy",
            "--path",
            "/inbound/deploy",
            "--method",
            "post",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let add_json: Value = serde_json::from_slice(&add_output).expect("add json");
    assert_eq!(add_json["ok"], true);
    let webhook_id = add_json["webhook"]["id"]
        .as_str()
        .expect("webhook id")
        .to_string();

    let resolve_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--yes",
            "--json",
            "webhooks",
            "resolve",
            "--path",
            "/inbound/deploy",
            "--method",
            "post",
            "--data",
            "{\"release\":\"2026.02\"}",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let resolve_json: Value = serde_json::from_slice(&resolve_output).expect("resolve json");
    assert_eq!(resolve_json["ok"], true);
    assert_eq!(resolve_json["result"]["ok"], true);
    assert_eq!(resolve_json["result"]["hooks_triggered"], 1);
    assert_eq!(resolve_json["result"]["hooks_failed"], 0);

    let trigger_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--yes",
            "--json",
            "webhooks",
            "trigger",
            &webhook_id,
            "--data",
            "{\"source\":\"manual\"}",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let trigger_json: Value = serde_json::from_slice(&trigger_output).expect("trigger json");
    assert_eq!(trigger_json["ok"], true);
    assert_eq!(trigger_json["result"]["trigger"], "manual");

    let list_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args(["--project-state", "--json", "webhooks", "list"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let list_json: Value = serde_json::from_slice(&list_output).expect("list json");
    assert_eq!(list_json["ok"], true);
    let webhooks = list_json["webhooks"].as_array().expect("webhooks array");
    assert_eq!(webhooks.len(), 1);
    assert_eq!(webhooks[0]["id"], webhook_id);
    assert_eq!(webhooks[0]["last_result"]["ok"], true);

    let logs_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "webhooks",
            "logs",
            "--webhook",
            &webhook_id,
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
            .any(|item| item["trigger"].as_str() == Some("resolve"))
    );
    assert!(
        events
            .iter()
            .any(|item| item["trigger"].as_str() == Some("manual"))
    );
}

#[test]
#[allow(deprecated)]
fn webhooks_secret_validation_flow() {
    let temp = tempdir().expect("tempdir");
    setup_project(&temp);

    let add_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "webhooks",
            "add",
            "--name",
            "secret-webhook",
            "--event",
            "deploy",
            "--path",
            "/secret",
            "--method",
            "post",
            "--secret-env",
            "MOSAIC_WEBHOOK_SECRET",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let add_json: Value = serde_json::from_slice(&add_output).expect("add json");
    assert_eq!(add_json["ok"], true);

    let missing_env_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "webhooks",
            "resolve",
            "--path",
            "/secret",
            "--method",
            "post",
        ])
        .assert()
        .failure()
        .code(3)
        .get_output()
        .stdout
        .clone();
    let missing_env_json: Value =
        serde_json::from_slice(&missing_env_output).expect("missing env json");
    assert_eq!(missing_env_json["ok"], false);
    assert_eq!(missing_env_json["error"]["code"], "auth");

    let missing_secret_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .env("MOSAIC_WEBHOOK_SECRET", "top-secret")
        .args([
            "--project-state",
            "--json",
            "webhooks",
            "resolve",
            "--path",
            "/secret",
            "--method",
            "post",
        ])
        .assert()
        .failure()
        .code(3)
        .get_output()
        .stdout
        .clone();
    let missing_secret_json: Value =
        serde_json::from_slice(&missing_secret_output).expect("missing secret json");
    assert_eq!(missing_secret_json["ok"], false);
    assert_eq!(missing_secret_json["error"]["code"], "auth");

    let mismatch_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .env("MOSAIC_WEBHOOK_SECRET", "top-secret")
        .args([
            "--project-state",
            "--json",
            "webhooks",
            "resolve",
            "--path",
            "/secret",
            "--method",
            "post",
            "--secret",
            "wrong-secret",
        ])
        .assert()
        .failure()
        .code(3)
        .get_output()
        .stdout
        .clone();
    let mismatch_json: Value = serde_json::from_slice(&mismatch_output).expect("mismatch json");
    assert_eq!(mismatch_json["ok"], false);
    assert_eq!(mismatch_json["error"]["code"], "auth");

    let ok_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .env("MOSAIC_WEBHOOK_SECRET", "top-secret")
        .args([
            "--project-state",
            "--json",
            "webhooks",
            "resolve",
            "--path",
            "/secret",
            "--method",
            "post",
            "--secret",
            "top-secret",
            "--data",
            "{\"ok\":true}",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let ok_json: Value = serde_json::from_slice(&ok_output).expect("ok json");
    assert_eq!(ok_json["ok"], true);
    assert_eq!(ok_json["result"]["ok"], true);
}

#[test]
#[allow(deprecated)]
fn webhooks_disabled_not_resolved() {
    let temp = tempdir().expect("tempdir");
    setup_project(&temp);

    Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "webhooks",
            "add",
            "--name",
            "disabled-wh",
            "--event",
            "deploy",
            "--path",
            "/disabled",
            "--method",
            "post",
            "--disabled",
        ])
        .assert()
        .success();

    let output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "webhooks",
            "resolve",
            "--path",
            "/disabled",
            "--method",
            "post",
        ])
        .assert()
        .failure()
        .code(7)
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output).expect("error json");
    assert_eq!(json["ok"], false);
    assert_eq!(json["error"]["code"], "validation");
}

#[test]
#[allow(deprecated)]
fn webhooks_logs_summary_and_since_minutes_filter() {
    let temp = tempdir().expect("tempdir");
    setup_project(&temp);

    let add_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "webhooks",
            "add",
            "--name",
            "summary-webhook",
            "--event",
            "deploy",
            "--path",
            "/summary",
            "--method",
            "post",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let add_json: Value = serde_json::from_slice(&add_output).expect("add json");
    let webhook_id = add_json["webhook"]["id"]
        .as_str()
        .expect("webhook id")
        .to_string();

    Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--yes",
            "--json",
            "webhooks",
            "trigger",
            &webhook_id,
        ])
        .assert()
        .success();

    let summary_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "webhooks",
            "logs",
            "--webhook",
            &webhook_id,
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
    assert_eq!(summary_json["summary"]["by_method"]["POST"], 1);

    let events_path = temp
        .path()
        .join(".mosaic/data/webhook-events")
        .join(format!("{webhook_id}.jsonl"));
    let raw = fs::read_to_string(&events_path).expect("read webhook events");
    let mut lines = raw
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).expect("parse webhook event"))
        .collect::<Vec<_>>();
    lines[0]["ts"] = Value::String("2000-01-01T00:00:00Z".to_string());
    let rewritten = format!(
        "{}\n",
        lines
            .into_iter()
            .map(|item| serde_json::to_string(&item).expect("encode webhook event"))
            .collect::<Vec<_>>()
            .join("\n")
    );
    fs::write(&events_path, rewritten).expect("write webhook events");

    let since_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "webhooks",
            "logs",
            "--webhook",
            &webhook_id,
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
fn webhooks_replay_plan_and_apply_flow() {
    let temp = tempdir().expect("tempdir");
    setup_project(&temp);

    let add_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "webhooks",
            "add",
            "--name",
            "replay-secret-webhook",
            "--event",
            "deploy",
            "--path",
            "/replay-secret",
            "--method",
            "post",
            "--secret-env",
            "MOSAIC_WEBHOOK_SECRET",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let add_json: Value = serde_json::from_slice(&add_output).expect("add json");
    let webhook_id = add_json["webhook"]["id"]
        .as_str()
        .expect("webhook id")
        .to_string();

    Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .env("MOSAIC_WEBHOOK_SECRET", "top-secret")
        .args([
            "--project-state",
            "--json",
            "webhooks",
            "trigger",
            &webhook_id,
        ])
        .assert()
        .failure()
        .code(3);
    Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .env("MOSAIC_WEBHOOK_SECRET", "top-secret")
        .args([
            "--project-state",
            "--json",
            "webhooks",
            "trigger",
            &webhook_id,
        ])
        .assert()
        .failure()
        .code(3);

    let replay_plan_report = temp.path().join("webhooks-replay-plan.json");
    let replay_plan_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .env("MOSAIC_WEBHOOK_SECRET", "top-secret")
        .args([
            "--project-state",
            "--json",
            "webhooks",
            "replay",
            "--webhook",
            &webhook_id,
            "--tail",
            "20",
            "--limit",
            "5",
            "--batch-size",
            "1",
            "--reason",
            "auth",
            "--report-out",
            replay_plan_report.to_string_lossy().as_ref(),
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let replay_plan_json: Value = serde_json::from_slice(&replay_plan_output).expect("replay");
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
        0
    );
    assert_eq!(
        replay_plan_json["recovery_diagnostics"]["non_retryable_candidates"],
        2
    );
    assert_eq!(
        replay_plan_json["recovery_diagnostics"]["suggested_strategy"],
        "inspect_non_retryable_first"
    );
    assert_eq!(replay_plan_json["candidates"][0]["requires_secret"], true);
    assert_eq!(replay_plan_json["candidates"][0]["reason"], "auth");
    assert_eq!(replay_plan_json["candidates"][0]["retryable"], false);
    let replay_plan_report_json: Value = serde_json::from_str(
        &fs::read_to_string(&replay_plan_report).expect("read replay plan report"),
    )
    .expect("parse replay plan report");
    assert_eq!(replay_plan_report_json["selected_candidates"], 2);

    let replay_apply_report = temp.path().join("webhooks-replay-apply.json");
    let replay_apply_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .env("MOSAIC_WEBHOOK_SECRET", "top-secret")
        .args([
            "--project-state",
            "--yes",
            "--json",
            "webhooks",
            "replay",
            "--webhook",
            &webhook_id,
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
        serde_json::from_slice(&replay_apply_output).expect("replay apply");
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
        .env("MOSAIC_WEBHOOK_SECRET", "top-secret")
        .args([
            "--project-state",
            "--json",
            "webhooks",
            "logs",
            "--webhook",
            &webhook_id,
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
