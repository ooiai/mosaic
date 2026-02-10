use assert_cmd::Command;
use serde_json::Value;
use tempfile::tempdir;

#[test]
#[allow(deprecated)]
fn setup_models_ask_session_show_flow() {
    let temp = tempdir().expect("tempdir");

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

    let models_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args(["--project-state", "--json", "models", "list"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let models_json: Value = serde_json::from_slice(&models_output).expect("models json");
    assert_eq!(models_json["ok"], true);
    assert_eq!(models_json["models"][0]["id"], "mock-model");

    let ask_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .env("MOSAIC_MOCK_CHAT_RESPONSE", "mock-answer")
        .args(["--project-state", "--json", "ask", "hello"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let ask_json: Value = serde_json::from_slice(&ask_output).expect("ask json");
    assert_eq!(ask_json["ok"], true);
    assert_eq!(ask_json["response"], "mock-answer");
    let session_id = ask_json["session_id"]
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
