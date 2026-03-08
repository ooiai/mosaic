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
fn cron_add_tick_run_and_logs_flow() {
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
            "cron-hook",
            "--event",
            "deploy",
            "--command",
            "echo cron-hook-fired",
        ])
        .assert()
        .success();

    let add_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "cron",
            "add",
            "--name",
            "deploy-cron",
            "--event",
            "deploy",
            "--every",
            "1",
            "--data",
            "{\"source\":\"cron\"}",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let add_json: Value = serde_json::from_slice(&add_output).expect("add json");
    assert_eq!(add_json["ok"], true);
    let job_id = add_json["job"]["id"].as_str().expect("job id").to_string();

    let tick_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args(["--project-state", "--yes", "--json", "cron", "tick"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let tick_json: Value = serde_json::from_slice(&tick_output).expect("tick json");
    assert_eq!(tick_json["ok"], true);
    assert_eq!(tick_json["triggered"], 1);
    assert_eq!(tick_json["ok_count"], 1);
    assert_eq!(tick_json["failed_count"], 0);

    let run_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--yes",
            "--json",
            "cron",
            "run",
            &job_id,
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
    assert_eq!(run_json["result"]["trigger"], "manual");
    assert_eq!(run_json["result"]["ok"], true);

    let list_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args(["--project-state", "--json", "cron", "list"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let list_json: Value = serde_json::from_slice(&list_output).expect("list json");
    assert_eq!(list_json["ok"], true);
    let jobs = list_json["jobs"].as_array().expect("jobs array");
    assert_eq!(jobs.len(), 1);
    assert_eq!(jobs[0]["id"], job_id);
    assert_eq!(jobs[0]["run_count"], 2);
    assert_eq!(jobs[0]["last_result"]["ok"], true);

    let logs_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "cron",
            "logs",
            "--job",
            &job_id,
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
            .any(|item| item["trigger"].as_str() == Some("tick"))
    );
    assert!(
        events
            .iter()
            .any(|item| item["trigger"].as_str() == Some("manual"))
    );
}

#[test]
#[allow(deprecated)]
fn cron_disabled_and_limit_validation_flow() {
    let temp = tempdir().expect("tempdir");
    setup_project(&temp);

    let add_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "cron",
            "add",
            "--name",
            "disabled-cron",
            "--event",
            "deploy",
            "--every",
            "2",
            "--disabled",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let add_json: Value = serde_json::from_slice(&add_output).expect("add json");
    let job_id = add_json["job"]["id"].as_str().expect("job id");

    let tick_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args(["--project-state", "--json", "cron", "tick"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let tick_json: Value = serde_json::from_slice(&tick_output).expect("tick json");
    assert_eq!(tick_json["triggered"], 0);

    Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args(["--project-state", "cron", "enable", job_id])
        .assert()
        .success();

    let tick_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args(["--project-state", "--json", "cron", "tick"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let tick_json: Value = serde_json::from_slice(&tick_output).expect("tick json");
    assert_eq!(tick_json["triggered"], 1);

    let error_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args(["--project-state", "--json", "cron", "tick", "--limit", "0"])
        .assert()
        .failure()
        .code(7)
        .get_output()
        .stdout
        .clone();
    let error_json: Value = serde_json::from_slice(&error_output).expect("error json");
    assert_eq!(error_json["ok"], false);
    assert_eq!(error_json["error"]["code"], "validation");
}

#[test]
#[allow(deprecated)]
fn cron_logs_summary_and_since_minutes_filter() {
    let temp = tempdir().expect("tempdir");
    setup_project(&temp);

    let add_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "cron",
            "add",
            "--name",
            "summary-cron",
            "--event",
            "deploy",
            "--every",
            "1",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let add_json: Value = serde_json::from_slice(&add_output).expect("add json");
    let job_id = add_json["job"]["id"].as_str().expect("job id").to_string();

    Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args(["--project-state", "--yes", "--json", "cron", "run", &job_id])
        .assert()
        .success();

    let summary_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "cron",
            "logs",
            "--job",
            &job_id,
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
        .join(".mosaic/data/cron-events")
        .join(format!("{job_id}.jsonl"));
    let raw = fs::read_to_string(&events_path).expect("read cron events");
    let mut lines = raw
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).expect("parse cron event"))
        .collect::<Vec<_>>();
    lines[0]["ts"] = Value::String("2000-01-01T00:00:00Z".to_string());
    let rewritten = format!(
        "{}\n",
        lines
            .into_iter()
            .map(|item| serde_json::to_string(&item).expect("encode cron event"))
            .collect::<Vec<_>>()
            .join("\n")
    );
    fs::write(&events_path, rewritten).expect("write cron events");

    let since_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "cron",
            "logs",
            "--job",
            &job_id,
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
fn cron_memory_cleanup_event_applies_policy() {
    let temp = tempdir().expect("tempdir");
    setup_project(&temp);

    let alpha_root = temp.path().join("alpha");
    let beta_root = temp.path().join("beta");
    std::fs::create_dir_all(&alpha_root).expect("create alpha");
    std::fs::create_dir_all(&beta_root).expect("create beta");
    std::fs::write(alpha_root.join("a.md"), "alpha namespace memory").expect("write alpha");
    std::fs::write(beta_root.join("b.md"), "beta namespace memory").expect("write beta");

    Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "memory",
            "index",
            "--path",
            "alpha",
            "--namespace",
            "alpha",
        ])
        .assert()
        .success();
    std::thread::sleep(std::time::Duration::from_millis(20));
    Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "memory",
            "index",
            "--path",
            "beta",
            "--namespace",
            "beta",
        ])
        .assert()
        .success();

    Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "memory",
            "policy",
            "set",
            "--enabled",
            "true",
            "--max-namespaces",
            "1",
            "--min-interval-minutes",
            "60",
        ])
        .assert()
        .success();

    let add_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "cron",
            "add",
            "--name",
            "memory-cleanup",
            "--event",
            "mosaic.memory.cleanup",
            "--every",
            "1",
            "--data",
            "{\"force\":true}",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let add_json: Value = serde_json::from_slice(&add_output).expect("cron add json");
    let job_id = add_json["job"]["id"].as_str().expect("job id");

    let run_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args(["--project-state", "--yes", "--json", "cron", "run", job_id])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let run_json: Value = serde_json::from_slice(&run_output).expect("cron run json");
    assert_eq!(run_json["ok"], true);
    assert_eq!(run_json["result"]["ok"], true);
    assert_eq!(run_json["result"]["event"], "mosaic.memory.cleanup");

    let status_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "memory",
            "status",
            "--all-namespaces",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let status_json: Value = serde_json::from_slice(&status_output).expect("memory status json");
    let namespaces = status_json["namespaces"]
        .as_array()
        .expect("namespaces array");
    assert!(namespaces.iter().any(|item| item["namespace"] == "beta"));
    assert!(!namespaces.iter().any(|item| item["namespace"] == "alpha"));

    let policy_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args(["--project-state", "--json", "memory", "policy", "get"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let policy_json: Value = serde_json::from_slice(&policy_output).expect("policy get json");
    assert!(policy_json["policy"]["last_run_at"].is_string());
    assert_eq!(policy_json["policy"]["last_run_removed_count"], 1);
}

#[test]
#[allow(deprecated)]
fn cron_memory_cleanup_event_respects_policy_interval() {
    let temp = tempdir().expect("tempdir");
    setup_project(&temp);

    let alpha_root = temp.path().join("alpha");
    let beta_root = temp.path().join("beta");
    std::fs::create_dir_all(&alpha_root).expect("create alpha");
    std::fs::create_dir_all(&beta_root).expect("create beta");
    std::fs::write(alpha_root.join("a.md"), "alpha namespace memory").expect("write alpha");
    std::fs::write(beta_root.join("b.md"), "beta namespace memory").expect("write beta");

    Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "memory",
            "index",
            "--path",
            "alpha",
            "--namespace",
            "alpha",
        ])
        .assert()
        .success();
    std::thread::sleep(std::time::Duration::from_millis(20));
    Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "memory",
            "index",
            "--path",
            "beta",
            "--namespace",
            "beta",
        ])
        .assert()
        .success();

    Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "memory",
            "policy",
            "set",
            "--enabled",
            "true",
            "--max-namespaces",
            "1",
            "--min-interval-minutes",
            "120",
        ])
        .assert()
        .success();

    let add_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "cron",
            "add",
            "--name",
            "memory-cleanup",
            "--event",
            "mosaic.memory.cleanup",
            "--every",
            "1",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let add_json: Value = serde_json::from_slice(&add_output).expect("cron add json");
    let job_id = add_json["job"]["id"].as_str().expect("job id");

    let first_run_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args(["--project-state", "--yes", "--json", "cron", "run", job_id])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let first_run_json: Value = serde_json::from_slice(&first_run_output).expect("first run json");
    assert_eq!(first_run_json["ok"], true);
    assert_eq!(first_run_json["result"]["ok"], true);

    let first_policy_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args(["--project-state", "--json", "memory", "policy", "get"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let first_policy_json: Value =
        serde_json::from_slice(&first_policy_output).expect("first policy get json");
    let first_last_run_at = first_policy_json["policy"]["last_run_at"]
        .as_str()
        .expect("first last_run_at")
        .to_string();

    let second_run_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args(["--project-state", "--yes", "--json", "cron", "run", job_id])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let second_run_json: Value =
        serde_json::from_slice(&second_run_output).expect("second run json");
    assert_eq!(second_run_json["ok"], true);
    assert_eq!(second_run_json["result"]["ok"], true);

    let second_policy_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args(["--project-state", "--json", "memory", "policy", "get"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let second_policy_json: Value =
        serde_json::from_slice(&second_policy_output).expect("second policy get json");
    let second_last_run_at = second_policy_json["policy"]["last_run_at"]
        .as_str()
        .expect("second last_run_at");

    assert_eq!(second_last_run_at, first_last_run_at);
}

#[test]
#[allow(deprecated)]
fn cron_replay_plan_and_apply_flow() {
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
            "cron-fail-hook",
            "--event",
            "deploy",
            "--command",
            "false",
        ])
        .assert()
        .success();

    let add_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "cron",
            "add",
            "--name",
            "replay-cron",
            "--event",
            "deploy",
            "--every",
            "1",
            "--data",
            "{\"source\":\"cron-replay\"}",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let add_json: Value = serde_json::from_slice(&add_output).expect("add json");
    let job_id = add_json["job"]["id"].as_str().expect("job id").to_string();

    let run_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args(["--project-state", "--yes", "--json", "cron", "run", &job_id])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let run_json: Value = serde_json::from_slice(&run_output).expect("run json");
    assert_eq!(run_json["ok"], true);
    assert_eq!(run_json["result"]["ok"], false);
    Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args(["--project-state", "--yes", "--json", "cron", "run", &job_id])
        .assert()
        .success();

    let replay_plan_report = temp.path().join("cron-replay-plan.json");
    let replay_plan_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "cron",
            "replay",
            "--job",
            &job_id,
            "--tail",
            "20",
            "--limit",
            "5",
            "--batch-size",
            "1",
            "--reason",
            "hook_failures",
            "--retryable-only",
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
    assert_eq!(replay_plan_json["candidates"][0]["reason"], "hook_failures");
    assert_eq!(replay_plan_json["candidates"][0]["retryable"], true);
    let replay_plan_report_json: Value = serde_json::from_str(
        &fs::read_to_string(&replay_plan_report).expect("read replay plan report"),
    )
    .expect("parse replay plan report");
    assert_eq!(replay_plan_report_json["selected_candidates"], 2);

    let hooks_path = temp.path().join(".mosaic/data/hooks.json");
    let hooks_raw = fs::read_to_string(&hooks_path).expect("read hooks");
    let mut hooks_json: Value = serde_json::from_str(&hooks_raw).expect("parse hooks");
    hooks_json[0]["command"] = Value::String("echo cron-replay-ok".to_string());
    fs::write(
        &hooks_path,
        serde_json::to_string_pretty(&hooks_json).expect("encode hooks"),
    )
    .expect("write hooks");

    let replay_apply_report = temp.path().join("cron-replay-apply.json");
    let replay_apply_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--yes",
            "--json",
            "cron",
            "replay",
            "--job",
            &job_id,
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
        .args([
            "--project-state",
            "--json",
            "cron",
            "logs",
            "--job",
            &job_id,
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
