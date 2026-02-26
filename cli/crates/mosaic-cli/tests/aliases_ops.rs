use assert_cmd::Command;
use serde_json::Value;
use tempfile::tempdir;

#[test]
#[allow(deprecated)]
fn onboard_and_message_alias_flow() {
    let temp = tempdir().expect("tempdir");

    Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "onboard",
            "--base-url",
            "mock://mock-model",
            "--model",
            "mock-model",
        ])
        .assert()
        .success();

    let message_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .env("MOSAIC_MOCK_CHAT_RESPONSE", "alias-message-ok")
        .args(["--project-state", "--json", "message", "hello alias"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let message_json: Value = serde_json::from_slice(&message_output).expect("message json");
    assert_eq!(message_json["ok"], true);
    assert_eq!(message_json["response"], "alias-message-ok");

    let session_id = message_json["session_id"]
        .as_str()
        .expect("session id")
        .to_string();
    let show_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args(["--project-state", "--json", "session", "show", &session_id])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let show_json: Value = serde_json::from_slice(&show_output).expect("show json");
    let events = show_json["events"].as_array().expect("events array");
    assert!(events.len() >= 2);
}

#[test]
#[allow(deprecated)]
fn agent_alias_json_prompt_flow() {
    let temp = tempdir().expect("tempdir");

    Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "onboard",
            "--base-url",
            "mock://mock-model",
            "--model",
            "mock-model",
        ])
        .assert()
        .success();

    let output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .env("MOSAIC_MOCK_CHAT_RESPONSE", "alias-agent-ok")
        .args([
            "--project-state",
            "--json",
            "agent",
            "--prompt",
            "agent alias prompt",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output).expect("agent json");
    assert_eq!(json["ok"], true);
    assert_eq!(json["response"], "alias-agent-ok");
    assert!(json["session_id"].is_string());
}
