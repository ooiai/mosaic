use assert_cmd::Command;
use serde_json::Value;
use std::fs;
use tempfile::{TempDir, tempdir};

#[allow(deprecated)]
fn run_json(temp: &TempDir, args: &[&str]) -> Value {
    let output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args(args)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    serde_json::from_slice(&output).expect("json output")
}

#[test]
#[allow(deprecated)]
fn nodes_devices_pairing_flow() {
    let temp = tempdir().expect("tempdir");

    let nodes_list = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args(["--project-state", "--json", "nodes", "list"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let nodes_list: Value = serde_json::from_slice(&nodes_list).expect("nodes list");
    assert_eq!(nodes_list["ok"], true);
    assert_eq!(nodes_list["nodes"][0]["id"], "local");

    let request_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "pairing",
            "request",
            "--device",
            "dev-1",
            "--node",
            "local",
            "--reason",
            "integration test",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let request_json: Value = serde_json::from_slice(&request_output).expect("request json");
    let request_id = request_json["request"]["id"]
        .as_str()
        .expect("request id")
        .to_string();
    assert_eq!(request_json["request"]["status"], "pending");

    let pending_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "pairing",
            "list",
            "--status",
            "pending",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let pending_json: Value = serde_json::from_slice(&pending_output).expect("pending json");
    assert_eq!(pending_json["requests"][0]["id"], request_id);

    let approve_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "pairing",
            "approve",
            &request_id,
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let approve_json: Value = serde_json::from_slice(&approve_output).expect("approve json");
    assert_eq!(approve_json["request"]["status"], "approved");
    assert_eq!(approve_json["device"]["status"], "approved");

    let reject_request_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "pairing",
            "request",
            "--device",
            "dev-2",
            "--node",
            "local",
            "--reason",
            "reject me",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let reject_request_json: Value =
        serde_json::from_slice(&reject_request_output).expect("reject request json");
    let reject_request_id = reject_request_json["request"]["id"]
        .as_str()
        .expect("reject request id")
        .to_string();
    assert_eq!(reject_request_json["request"]["status"], "pending");

    let reject_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "pairing",
            "reject",
            &reject_request_id,
            "--reason",
            "policy denied",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let reject_json: Value = serde_json::from_slice(&reject_output).expect("reject json");
    assert_eq!(reject_json["request"]["status"], "rejected");
    assert_eq!(reject_json["request"]["reason"], "policy denied");

    let rejected_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "pairing",
            "list",
            "--status",
            "rejected",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let rejected_json: Value = serde_json::from_slice(&rejected_output).expect("rejected json");
    assert_eq!(rejected_json["requests"][0]["id"], reject_request_id);

    let rotate_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args(["--project-state", "--json", "devices", "rotate", "dev-1"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let rotate_json: Value = serde_json::from_slice(&rotate_output).expect("rotate json");
    assert_eq!(rotate_json["device"]["token_version"], 2);

    Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .env("MOSAIC_GATEWAY_TEST_MODE", "1")
        .args(["--project-state", "--json", "gateway", "start"])
        .assert()
        .success();

    let run_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .env("MOSAIC_GATEWAY_TEST_MODE", "1")
        .args([
            "--project-state",
            "--yes",
            "--json",
            "nodes",
            "run",
            "local",
            "--command",
            "echo hello",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let run_json: Value = serde_json::from_slice(&run_output).expect("run json");
    assert_eq!(run_json["accepted"], true);
    assert_eq!(run_json["status"], "accepted");

    let invoke_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .env("MOSAIC_GATEWAY_TEST_MODE", "1")
        .args([
            "--project-state",
            "--json",
            "nodes",
            "invoke",
            "local",
            "status",
            "--params",
            r#"{"detail":true}"#,
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let invoke_json: Value = serde_json::from_slice(&invoke_output).expect("invoke json");
    assert_eq!(invoke_json["result"]["ok"], true);

    Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .env("MOSAIC_GATEWAY_TEST_MODE", "1")
        .args(["--project-state", "--json", "gateway", "stop"])
        .assert()
        .success();

    let status_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args(["--project-state", "--json", "nodes", "status", "local"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let status_json: Value = serde_json::from_slice(&status_output).expect("status json");
    assert_eq!(status_json["pairings"]["total"], 2);
    assert_eq!(status_json["pairings"]["pending"], 0);
    assert_eq!(status_json["pairings"]["approved"], 1);
    assert_eq!(status_json["pairings"]["rejected"], 1);

    let revoke_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "devices",
            "revoke",
            "dev-1",
            "--reason",
            "cleanup",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let revoke_json: Value = serde_json::from_slice(&revoke_output).expect("revoke json");
    assert_eq!(revoke_json["device"]["status"], "revoked");
}

#[test]
#[allow(deprecated)]
fn nodes_diagnose_reports_and_repairs_operational_issues() {
    let temp = tempdir().expect("tempdir");

    let approve_request = run_json(
        &temp,
        &[
            "--project-state",
            "--json",
            "pairing",
            "request",
            "--device",
            "diag-dev-approve",
            "--node",
            "local",
            "--reason",
            "approve baseline",
        ],
    );
    let approve_request_id = approve_request["request"]["id"]
        .as_str()
        .expect("approve request id")
        .to_string();
    run_json(
        &temp,
        &[
            "--project-state",
            "--json",
            "pairing",
            "approve",
            &approve_request_id,
        ],
    );
    run_json(
        &temp,
        &[
            "--project-state",
            "--json",
            "devices",
            "reject",
            "diag-dev-approve",
            "--reason",
            "force mismatch",
        ],
    );

    run_json(
        &temp,
        &[
            "--project-state",
            "--json",
            "devices",
            "approve",
            "diag-dev-blocked",
        ],
    );
    let blocked_request = run_json(
        &temp,
        &[
            "--project-state",
            "--json",
            "pairing",
            "request",
            "--device",
            "diag-dev-blocked",
            "--node",
            "local",
            "--reason",
            "blocked pending",
        ],
    );
    let blocked_request_id = blocked_request["request"]["id"]
        .as_str()
        .expect("blocked request id")
        .to_string();
    run_json(
        &temp,
        &[
            "--project-state",
            "--json",
            "devices",
            "reject",
            "diag-dev-blocked",
            "--reason",
            "force blocked",
        ],
    );

    let nodes_path = temp.path().join(".mosaic/data/nodes.json");
    let mut nodes_json: Value =
        serde_json::from_slice(&fs::read(&nodes_path).expect("read nodes json for diagnose test"))
            .expect("parse nodes json");
    let local_node = nodes_json
        .as_array_mut()
        .expect("nodes should be array")
        .iter_mut()
        .find(|node| node["id"] == "local")
        .expect("local node");
    local_node["last_seen_at"] = Value::String("2000-01-01T00:00:00Z".to_string());
    local_node["updated_at"] = Value::String("2000-01-01T00:00:00Z".to_string());
    fs::write(
        &nodes_path,
        serde_json::to_vec_pretty(&nodes_json).expect("serialize nodes json"),
    )
    .expect("write stale nodes json");

    let diagnose = run_json(
        &temp,
        &[
            "--project-state",
            "--json",
            "nodes",
            "diagnose",
            "local",
            "--stale-after-minutes",
            "30",
        ],
    );
    assert_eq!(diagnose["ok"], true);
    assert_eq!(diagnose["repair"], false);
    assert_eq!(diagnose["summary"]["stale_online_nodes"], 1);
    assert_eq!(diagnose["summary"]["approved_pairing_device_mismatch"], 1);
    assert_eq!(diagnose["summary"]["pending_pairing_blocked_device"], 1);
    assert_eq!(diagnose["summary"]["actions_applied"], 0);

    let issue_kinds = diagnose["issues"]
        .as_array()
        .expect("diagnose issues array")
        .iter()
        .filter_map(|issue| issue["kind"].as_str())
        .collect::<Vec<_>>();
    assert!(
        issue_kinds.contains(&"stale_online_node"),
        "expected stale_online_node issue in diagnose output: {diagnose}"
    );
    assert!(
        issue_kinds.contains(&"approved_pairing_device_mismatch"),
        "expected approved_pairing_device_mismatch issue in diagnose output: {diagnose}"
    );
    assert!(
        issue_kinds.contains(&"pending_pairing_blocked_device"),
        "expected pending_pairing_blocked_device issue in diagnose output: {diagnose}"
    );

    let repair = run_json(
        &temp,
        &[
            "--project-state",
            "--json",
            "nodes",
            "diagnose",
            "local",
            "--stale-after-minutes",
            "30",
            "--repair",
            "--report-out",
            ".mosaic/reports/nodes-diagnose.json",
        ],
    );
    assert_eq!(repair["ok"], true);
    assert_eq!(repair["repair"], true);
    assert_eq!(repair["summary"]["actions_applied"], 3);
    assert_eq!(repair["summary"]["saved_nodes"], true);
    assert_eq!(repair["summary"]["saved_devices"], true);
    assert_eq!(repair["summary"]["saved_pairings"], true);
    assert_eq!(repair["report_out"], ".mosaic/reports/nodes-diagnose.json");

    let status = run_json(
        &temp,
        &["--project-state", "--json", "nodes", "status", "local"],
    );
    assert_eq!(status["node"]["status"], "offline");

    let devices = run_json(&temp, &["--project-state", "--json", "devices", "list"]);
    let repaired_device = devices["devices"]
        .as_array()
        .expect("devices list")
        .iter()
        .find(|device| device["id"] == "diag-dev-approve")
        .expect("repaired device");
    assert_eq!(repaired_device["status"], "approved");

    let rejected = run_json(
        &temp,
        &[
            "--project-state",
            "--json",
            "pairing",
            "list",
            "--status",
            "rejected",
        ],
    );
    let rejected_ids = rejected["requests"]
        .as_array()
        .expect("rejected requests")
        .iter()
        .filter_map(|request| request["id"].as_str())
        .collect::<Vec<_>>();
    assert!(
        rejected_ids.iter().any(|id| *id == blocked_request_id),
        "expected blocked request to be auto-rejected after repair: {rejected}"
    );

    let report_path = temp.path().join(".mosaic/reports/nodes-diagnose.json");
    let report_json: Value =
        serde_json::from_slice(&fs::read(&report_path).expect("read diagnose report"))
            .expect("parse diagnose report");
    assert_eq!(report_json["summary"]["actions_applied"], 3);

    let events_path = temp.path().join(".mosaic/data/nodes-events.jsonl");
    let events_raw = fs::read_to_string(events_path).expect("read nodes events");
    assert!(
        events_raw.contains("\"action\":\"diagnose\""),
        "expected diagnose event in telemetry log"
    );
    assert!(
        events_raw.contains("\"action\":\"request\""),
        "expected pairing request event in telemetry log"
    );
    assert!(
        events_raw.contains("\"action\":\"approve\""),
        "expected approve event in telemetry log"
    );
}
