use assert_cmd::Command;
use serde_json::Value;
use tempfile::tempdir;

#[test]
#[allow(deprecated)]
fn gateway_skeleton_flow() {
    let temp = tempdir().expect("tempdir");

    let status_before = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .env("MOSAIC_GATEWAY_TEST_MODE", "1")
        .args(["--project-state", "--json", "gateway", "status"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let status_before: Value = serde_json::from_slice(&status_before).expect("status before json");
    assert_eq!(status_before["ok"], true);
    assert_eq!(status_before["running"], false);

    let run_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .env("MOSAIC_GATEWAY_TEST_MODE", "1")
        .args([
            "--project-state",
            "--json",
            "gateway",
            "run",
            "--host",
            "0.0.0.0",
            "--port",
            "8789",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let run_json: Value = serde_json::from_slice(&run_output).expect("run json");
    assert_eq!(run_json["ok"], true);
    assert_eq!(run_json["gateway"]["host"], "0.0.0.0");
    assert_eq!(run_json["gateway"]["port"], 8789);

    let status_after = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .env("MOSAIC_GATEWAY_TEST_MODE", "1")
        .args(["--project-state", "--json", "gateway", "status"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let status_after: Value = serde_json::from_slice(&status_after).expect("status after json");
    assert_eq!(status_after["running"], true);

    let stop_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .env("MOSAIC_GATEWAY_TEST_MODE", "1")
        .args(["--project-state", "--json", "gateway", "stop"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let stop_json: Value = serde_json::from_slice(&stop_output).expect("stop json");
    assert_eq!(stop_json["ok"], true);
    assert_eq!(stop_json["gateway"]["running"], false);
}

#[test]
#[allow(deprecated)]
fn channels_real_send_flow() {
    let temp = tempdir().expect("tempdir");

    let list_initial = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args(["--project-state", "--json", "channels", "list"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let list_initial: Value = serde_json::from_slice(&list_initial).expect("list initial json");
    assert_eq!(list_initial["ok"], true);
    assert_eq!(
        list_initial["channels"]
            .as_array()
            .expect("channels array")
            .len(),
        0
    );

    let add_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "channels",
            "add",
            "--name",
            "local-slack",
            "--kind",
            "mock",
            "--endpoint",
            "https://example.test",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let add_json: Value = serde_json::from_slice(&add_output).expect("add json");
    let channel_id = add_json["channel"]["id"]
        .as_str()
        .expect("channel id")
        .to_string();
    assert_eq!(add_json["channel"]["name"], "local-slack");

    let login_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .env("TEST_CHANNEL_TOKEN", "mock-token")
        .args([
            "--project-state",
            "--json",
            "channels",
            "login",
            &channel_id,
            "--token-env",
            "TEST_CHANNEL_TOKEN",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let login_json: Value = serde_json::from_slice(&login_output).expect("login json");
    assert_eq!(login_json["ok"], true);
    assert_eq!(login_json["token_present"], true);

    let list_after = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args(["--project-state", "--json", "channels", "list"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let list_after: Value = serde_json::from_slice(&list_after).expect("list after json");
    let channels = list_after["channels"].as_array().expect("channels array");
    assert_eq!(channels.len(), 1);
    assert_eq!(channels[0]["id"], channel_id);
    assert!(channels[0]["last_login_at"].is_string());

    let send_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "channels",
            "send",
            &channel_id,
            "--text",
            "hello-channel",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let send_json: Value = serde_json::from_slice(&send_output).expect("send json");
    assert_eq!(send_json["ok"], true);
    let path = send_json["path"].as_str().expect("event path");
    let event_content = std::fs::read_to_string(path).expect("event file content");
    assert!(event_content.contains("hello-channel"));
}
