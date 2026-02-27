use assert_cmd::Command;
use serde_json::Value;
use tempfile::tempdir;

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
