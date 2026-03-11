use assert_cmd::Command;
use serde_json::Value;
use tempfile::tempdir;

#[test]
#[allow(deprecated)]
fn tui_prompt_json_flow() {
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

    let output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .env("MOSAIC_MOCK_CHAT_RESPONSE", "tui-ok")
        .args([
            "--project-state",
            "--json",
            "tui",
            "--prompt",
            "hello from tui",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output).expect("json");
    assert_eq!(json["ok"], true);
    assert_eq!(json["response"], "tui-ok");
    assert!(json["session_id"].is_string());
}

#[test]
#[allow(deprecated)]
fn tui_non_interactive_without_prompt_returns_validation() {
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

    let output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args(["--project-state", "--json", "tui"])
        .assert()
        .failure()
        .code(7)
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output).expect("json");
    assert_eq!(json["ok"], false);
    assert_eq!(json["error"]["code"], "validation");
}

#[test]
#[allow(deprecated)]
fn tui_prompt_json_supports_session_resume() {
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

    Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "agents",
            "add",
            "--id",
            "writer",
            "--name",
            "Writer",
            "--model",
            "mock-model",
        ])
        .assert()
        .success();

    let first: Value = serde_json::from_slice(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .env("MOSAIC_MOCK_CHAT_RESPONSE", "first-tui")
            .args([
                "--project-state",
                "--json",
                "tui",
                "--agent",
                "writer",
                "--prompt",
                "first prompt",
            ])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone(),
    )
    .expect("first json");
    let session_id = first["session_id"]
        .as_str()
        .expect("session id")
        .to_string();
    assert_eq!(first["agent_id"], "writer");

    Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "agents",
            "add",
            "--id",
            "reviewer",
            "--name",
            "Reviewer",
            "--model",
            "mock-model",
            "--set-default",
        ])
        .assert()
        .success();

    let second: Value = serde_json::from_slice(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .env("MOSAIC_MOCK_CHAT_RESPONSE", "second-tui")
            .args([
                "--project-state",
                "--json",
                "tui",
                "--session",
                &session_id,
                "--prompt",
                "second prompt",
            ])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone(),
    )
    .expect("second json");
    assert_eq!(second["ok"], true);
    assert_eq!(second["session_id"], session_id);
    assert_eq!(second["response"], "second-tui");
    assert_eq!(second["agent_id"], "writer");
}

#[test]
#[allow(deprecated)]
fn tui_prompt_resume_keeps_original_agent_when_default_changes() {
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

    Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "agents",
            "add",
            "--id",
            "writer",
            "--name",
            "Writer",
            "--set-default",
        ])
        .assert()
        .success();

    let first: Value = serde_json::from_slice(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .env("MOSAIC_MOCK_CHAT_RESPONSE", "writer-tui")
            .args([
                "--project-state",
                "--json",
                "tui",
                "--prompt",
                "writer tui prompt",
            ])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone(),
    )
    .expect("first tui json");
    let session_id = first["session_id"]
        .as_str()
        .expect("session id")
        .to_string();
    assert_eq!(first["agent_id"], "writer");

    Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "agents",
            "add",
            "--id",
            "reviewer",
            "--name",
            "Reviewer",
            "--set-default",
        ])
        .assert()
        .success();

    let resumed: Value = serde_json::from_slice(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .env("MOSAIC_MOCK_CHAT_RESPONSE", "writer-tui-resumed")
            .args([
                "--project-state",
                "--json",
                "tui",
                "--session",
                &session_id,
                "--prompt",
                "resume tui prompt",
            ])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone(),
    )
    .expect("resumed tui json");
    assert_eq!(resumed["session_id"], session_id);
    assert_eq!(resumed["agent_id"], "writer");
}
