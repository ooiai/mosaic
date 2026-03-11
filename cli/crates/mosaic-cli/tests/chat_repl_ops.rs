use assert_cmd::Command;

use tempfile::tempdir;

#[test]
#[allow(deprecated)]
fn chat_repl_supports_status_agent_session_commands() {
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
        .env("MOSAIC_MOCK_CHAT_RESPONSE", "chat-repl-ok")
        .args(["--project-state", "chat"])
        .write_stdin("/status\nhello from repl\n/agent\n/session\n/exit\n")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let stdout = String::from_utf8(output).expect("stdout utf8");
    assert!(stdout.contains("Entering chat mode"));
    assert!(stdout.contains("profile: default"));
    assert!(stdout.contains("agent:"));
    assert!(stdout.contains("session: <new session>"));
    assert!(stdout.contains("assistant> chat-repl-ok"));
    assert!(stdout.contains("Bye."));
}

#[test]
#[allow(deprecated)]
fn chat_repl_help_includes_extended_commands() {
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
        .args(["--project-state", "chat"])
        .write_stdin("/help\n/exit\n")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let stdout = String::from_utf8(output).expect("stdout utf8");
    assert!(stdout.contains("/status   Show profile/agent/session"));
    assert!(stdout.contains("/agent    Show active agent"));
    assert!(stdout.contains("/agent ID Switch active agent"));
    assert!(stdout.contains("/new      Start a new chat session"));
}

#[test]
#[allow(deprecated)]
fn chat_repl_new_resets_session_id() {
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
        .env("MOSAIC_MOCK_CHAT_RESPONSE", "chat-repl-reset-ok")
        .args(["--project-state", "chat"])
        .write_stdin("hello first\n/session\n/new\n/session\n/exit\n")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let stdout = String::from_utf8(output).expect("stdout utf8");
    let session_occurrences = stdout.matches("session: ").count();
    assert!(
        stdout.contains("session: <new session>"),
        "missing new-session marker in output: {stdout}"
    );
    assert!(
        stdout.contains("session reset: <new session>"),
        "missing session reset marker in output: {stdout}"
    );
    assert!(
        session_occurrences >= 2,
        "expected at least two session prints in output: {stdout}"
    );
}

#[test]
#[allow(deprecated)]
fn session_resume_prefers_session_runtime_agent_over_current_default() {
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

    let first_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .env("MOSAIC_MOCK_CHAT_RESPONSE", "chat-writer-first")
        .args([
            "--project-state",
            "--json",
            "chat",
            "--agent",
            "writer",
            "--prompt",
            "seed session",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let first_json: serde_json::Value =
        serde_json::from_slice(&first_output).expect("first chat json");
    let session_id = first_json["session_id"]
        .as_str()
        .expect("session id")
        .to_string();
    assert_eq!(first_json["agent_id"], "writer");

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

    let output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args(["--project-state", "session", "resume", &session_id])
        .write_stdin("/agent\n/exit\n")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let stdout = String::from_utf8(output).expect("stdout utf8");
    assert!(stdout.contains(&format!("Resumed session: {session_id}")));
    assert!(stdout.contains("Using agent: writer"));
    assert!(stdout.contains("agent: writer"));
}

#[test]
#[allow(deprecated)]
fn chat_repl_switches_agent_before_first_turn() {
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

    let output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .env("MOSAIC_MOCK_CHAT_RESPONSE", "chat-switch-before-first-turn")
        .args(["--project-state", "chat"])
        .write_stdin("/agent writer\nhello writer\n/session\n/exit\n")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let stdout = String::from_utf8(output).expect("stdout utf8");
    assert!(stdout.contains("agent switched: writer"));
    assert!(!stdout.contains("session reset: <new session>"));
    assert!(stdout.contains("assistant> chat-switch-before-first-turn"));

    let sessions_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args(["--project-state", "--json", "session", "list"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let sessions_json: serde_json::Value =
        serde_json::from_slice(&sessions_output).expect("session list json");
    let session_id = sessions_json["sessions"][0]["session_id"]
        .as_str()
        .expect("session id");

    let show_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args(["--project-state", "--json", "session", "show", session_id])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let show_json: serde_json::Value =
        serde_json::from_slice(&show_output).expect("session show json");
    assert_eq!(show_json["runtime"]["agent_id"], "writer");
    assert_eq!(show_json["runtime"]["profile_name"], "default");
}

#[test]
#[allow(deprecated)]
fn chat_repl_switching_agent_after_first_turn_resets_session() {
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
            "reviewer",
            "--name",
            "Reviewer",
            "--model",
            "mock-model",
            "--set-default",
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

    let output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .env("MOSAIC_MOCK_CHAT_RESPONSE", "chat-switch-after-first-turn")
        .args(["--project-state", "chat"])
        .write_stdin("first turn\n/agent writer\nsecond turn\n/exit\n")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let stdout = String::from_utf8(output).expect("stdout utf8");
    assert!(stdout.contains("agent switched: writer"));
    assert!(stdout.contains("session reset: <new session>"));
    assert_eq!(
        stdout
            .matches("assistant> chat-switch-after-first-turn")
            .count(),
        2
    );

    let sessions_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args(["--project-state", "--json", "session", "list"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let sessions_json: serde_json::Value =
        serde_json::from_slice(&sessions_output).expect("session list json");
    let sessions = sessions_json["sessions"]
        .as_array()
        .expect("sessions array");
    assert_eq!(sessions.len(), 2);

    let newer_session = sessions[0]["session_id"].as_str().expect("newer session");
    let older_session = sessions[1]["session_id"].as_str().expect("older session");

    let newer_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "session",
            "show",
            newer_session,
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let newer_json: serde_json::Value =
        serde_json::from_slice(&newer_output).expect("newer session json");
    assert_eq!(newer_json["runtime"]["agent_id"], "writer");

    let older_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "session",
            "show",
            older_session,
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let older_json: serde_json::Value =
        serde_json::from_slice(&older_output).expect("older session json");
    assert_eq!(older_json["runtime"]["agent_id"], "reviewer");
}

#[test]
#[allow(deprecated)]
fn chat_repl_switch_agent_rejects_unknown_agent() {
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
        .args(["--project-state", "chat"])
        .write_stdin("/agent missing\n/agent\n/exit\n")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let stdout = String::from_utf8(output).expect("stdout utf8");
    assert!(stdout.contains("error: validation error: agent 'missing' not found"));
    assert!(stdout.contains("agent: <none>"));
}
