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
