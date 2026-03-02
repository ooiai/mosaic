use assert_cmd::Command;
use serde_json::Value;
use std::fs;
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

    let approvals_check_confirm = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "approvals",
            "check",
            "--command",
            "echo smoke",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let approvals_check_confirm: Value =
        serde_json::from_slice(&approvals_check_confirm).expect("approvals check confirm");
    assert_eq!(approvals_check_confirm["decision"], "confirm");

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

    let approvals_check_deny = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "approvals",
            "check",
            "--command",
            "echo smoke",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let approvals_check_deny: Value =
        serde_json::from_slice(&approvals_check_deny).expect("approvals check deny");
    assert_eq!(approvals_check_deny["decision"], "deny");

    let approvals_set_allowlist = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args(["--project-state", "--json", "approvals", "set", "allowlist"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let approvals_set_allowlist: Value =
        serde_json::from_slice(&approvals_set_allowlist).expect("approvals set allowlist");
    assert_eq!(approvals_set_allowlist["policy"]["mode"], "allowlist");

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

    let approvals_allowlist_list = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "approvals",
            "allowlist",
            "list",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let approvals_allowlist_list: Value =
        serde_json::from_slice(&approvals_allowlist_list).expect("approvals allowlist list");
    assert!(
        approvals_allowlist_list["policy"]["allowlist"]
            .as_array()
            .expect("allowlist list")
            .iter()
            .any(|item| item.as_str() == Some("cargo test"))
    );

    let approvals_check_auto = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "approvals",
            "check",
            "--command",
            "cargo test --workspace",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let approvals_check_auto: Value =
        serde_json::from_slice(&approvals_check_auto).expect("approvals check auto");
    assert_eq!(approvals_check_auto["decision"], "auto");
    assert_eq!(approvals_check_auto["approved_by"], "approval_allowlist");

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

    let sandbox_get = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args(["--project-state", "--json", "sandbox", "get"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let sandbox_get: Value = serde_json::from_slice(&sandbox_get).expect("sandbox get");
    assert_eq!(sandbox_get["ok"], true);
    assert_eq!(sandbox_get["policy"]["profile"], "standard");

    let sandbox_check_allow = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "sandbox",
            "check",
            "--command",
            "echo smoke",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let sandbox_check_allow: Value =
        serde_json::from_slice(&sandbox_check_allow).expect("sandbox check allow");
    assert_eq!(sandbox_check_allow["decision"], "allow");

    let sandbox_set = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args(["--project-state", "--json", "sandbox", "set", "restricted"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let sandbox_set: Value = serde_json::from_slice(&sandbox_set).expect("sandbox set");
    assert_eq!(sandbox_set["ok"], true);
    assert_eq!(sandbox_set["policy"]["profile"], "restricted");

    let sandbox_check_deny = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "sandbox",
            "check",
            "--command",
            "curl https://example.com",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let sandbox_check_deny: Value =
        serde_json::from_slice(&sandbox_check_deny).expect("sandbox check deny");
    assert_eq!(sandbox_check_deny["decision"], "deny");
    assert!(sandbox_check_deny["reason"].is_string());

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

    let safety_get = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args(["--project-state", "--json", "safety", "get"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let safety_get: Value = serde_json::from_slice(&safety_get).expect("safety get");
    assert_eq!(safety_get["ok"], true);
    assert_eq!(safety_get["approvals"]["policy"]["mode"], "allowlist");
    assert_eq!(safety_get["sandbox"]["policy"]["profile"], "restricted");

    let safety_check_allow = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "safety",
            "check",
            "--command",
            "cargo test --workspace",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let safety_check_allow: Value =
        serde_json::from_slice(&safety_check_allow).expect("safety check allow");
    assert_eq!(safety_check_allow["decision"], "allow");
    assert_eq!(safety_check_allow["approved_by"], "approval_allowlist");
    assert_eq!(safety_check_allow["sandbox"]["decision"], "allow");
    assert_eq!(safety_check_allow["approvals"]["decision"], "auto");

    let audit_dir = temp.path().join(".mosaic/data/audit");
    fs::create_dir_all(&audit_dir).expect("create audit dir");
    fs::write(
        audit_dir.join("commands.jsonl"),
        "{\"id\":\"aud-1\",\"ts\":\"2026-03-01T00:00:00Z\",\"session_id\":\"s-1\",\"command\":\"cargo test --workspace\",\"cwd\":\"/tmp\",\"approved_by\":\"approval_allowlist\",\"exit_code\":0,\"duration_ms\":123}\n\
{\"id\":\"aud-2\",\"ts\":\"2026-03-01T00:01:00Z\",\"session_id\":\"s-1\",\"command\":\"curl https://example.com\",\"cwd\":\"/tmp\",\"approved_by\":\"flag_yes\",\"exit_code\":1,\"duration_ms\":250}\n\
this-is-invalid-json\n\
{\"id\":\"aud-3\",\"ts\":\"2026-03-01T00:02:00Z\",\"session_id\":\"s-1\",\"command\":\"cargo test --workspace\",\"cwd\":\"/tmp\",\"approved_by\":\"approval_allowlist\",\"exit_code\":0,\"duration_ms\":111}\n\
{\"id\":\"aud-4\",\"ts\":\"2026-03-01T00:03:00Z\",\"session_id\":\"s-1\",\"command\":\"curl https://example.com\",\"cwd\":\"/tmp\",\"approved_by\":\"flag_yes\",\"exit_code\":1,\"duration_ms\":200}\n",
    )
    .expect("write audit log");

    let safety_report = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "safety",
            "report",
            "--command",
            "curl https://example.com",
            "--audit-tail",
            "2",
            "--compare-window",
            "2",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let safety_report: Value = serde_json::from_slice(&safety_report).expect("safety report");
    assert_eq!(safety_report["ok"], true);
    assert_eq!(
        safety_report["check"]["decision"], "deny",
        "restricted sandbox should deny network command in safety report"
    );
    assert_eq!(safety_report["audit"]["summary"]["total_entries"], 2);
    assert_eq!(safety_report["audit"]["summary"]["parse_errors"], 1);
    assert_eq!(safety_report["audit"]["comparison"]["enabled"], true);
    assert_eq!(safety_report["audit"]["comparison"]["available"], true);
    assert_eq!(safety_report["audit"]["comparison"]["window"], 2);
    assert_eq!(safety_report["audit"]["comparison"]["delta"]["failed"], 0);
    assert_eq!(
        safety_report["audit"]["summary"]["blocked_if_restricted"],
        1
    );
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

    let system_logs_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "logs",
            "--tail",
            "20",
            "--source",
            "system",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let system_logs_json: Value = serde_json::from_slice(&system_logs_output).expect("system logs");
    assert_eq!(system_logs_json["ok"], true);
    let system_logs = system_logs_json["logs"]
        .as_array()
        .expect("system logs array");
    assert!(!system_logs.is_empty());
    assert!(
        system_logs
            .iter()
            .all(|entry| entry["source"].as_str() == Some("system"))
    );

    let system_list = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "system",
            "list",
            "--tail",
            "20",
            "--name",
            "deploy_start",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let system_list: Value = serde_json::from_slice(&system_list).expect("system list");
    assert_eq!(system_list["ok"], true);
    assert!(
        system_list["events"]
            .as_array()
            .expect("events array")
            .iter()
            .any(|event| event["name"].as_str() == Some("deploy_start"))
    );
    assert!(
        system_list["events"]
            .as_array()
            .expect("events array")
            .iter()
            .all(|event| event["name"].as_str() == Some("deploy_start"))
    );
}

#[test]
#[allow(deprecated)]
fn plugins_run_honors_restricted_sandbox_policy() {
    let temp = tempdir().expect("tempdir");
    let state_root = temp.path().join(".mosaic");
    let plugin_dir = state_root.join("plugins").join("net_hook");
    let hooks_dir = plugin_dir.join("hooks");
    fs::create_dir_all(&hooks_dir).expect("create hooks dir");
    fs::write(
        plugin_dir.join("plugin.toml"),
        "[plugin]\nid = \"net_hook\"\nname = \"Net Hook\"\nversion = \"0.1.0\"\n\n[runtime]\nrun = \"hooks/run.sh\"\nsandbox_profile = \"restricted\"\n",
    )
    .expect("write plugin manifest");
    fs::write(
        hooks_dir.join("run.sh"),
        "#!/bin/sh\ncurl https://example.com >/dev/null 2>&1\n",
    )
    .expect("write hook script");

    let output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args(["--project-state", "--json", "plugins", "run", "net_hook"])
        .assert()
        .failure()
        .code(12)
        .get_output()
        .stdout
        .clone();
    let payload: Value = serde_json::from_slice(&output).expect("plugins sandbox deny output");
    assert_eq!(payload["ok"], false);
    assert_eq!(payload["error"]["code"], "sandbox_denied");
}
