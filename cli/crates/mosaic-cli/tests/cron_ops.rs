use assert_cmd::Command;
use serde_json::Value;
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
