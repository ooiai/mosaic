#![allow(deprecated)]

use assert_cmd::Command;
use serde_json::Value;
use tempfile::TempDir;
use tempfile::tempdir;

fn setup_project(temp: &TempDir) {
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

fn ask_once(temp: &TempDir, response: &str, prompt: &str) -> String {
    let output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .env("MOSAIC_MOCK_CHAT_RESPONSE", response)
        .args(["--project-state", "--json", "ask", prompt])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output).expect("ask json");
    json["session_id"].as_str().expect("session id").to_string()
}

#[test]
#[allow(deprecated)]
fn session_list_orders_latest_first_and_show_text_includes_runtime() {
    let temp = tempdir().expect("tempdir");
    setup_project(&temp);

    let older_session = ask_once(&temp, "older-response", "older prompt");
    let newer_session = ask_once(&temp, "newer-response", "newer prompt");

    let list_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args(["--project-state", "--json", "session", "list"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let list_json: Value = serde_json::from_slice(&list_output).expect("session list json");
    let sessions = list_json["sessions"].as_array().expect("sessions");
    assert_eq!(sessions.len(), 2);
    assert_eq!(sessions[0]["session_id"], newer_session);
    assert_eq!(sessions[1]["session_id"], older_session);
    assert_eq!(sessions[0]["runtime"]["profile_name"], "default");
    assert!(sessions[0]["runtime"]["agent_id"].is_null());

    let list_text_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args(["--project-state", "session", "list"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let list_stdout = String::from_utf8(list_text_output).expect("list stdout utf8");
    assert!(list_stdout.contains("profile=default agent=<none>"));

    let show_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args(["--project-state", "session", "show", &newer_session])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let stdout = String::from_utf8(show_output).expect("stdout utf8");
    assert!(stdout.contains(&format!("Session: {newer_session}")));
    assert!(stdout.contains("Runtime: profile=default agent=<none>"));
    assert!(stdout.contains("newer prompt"));
    assert!(stdout.contains("newer-response"));
}

#[test]
#[allow(deprecated)]
fn session_resume_continues_existing_session() {
    let temp = tempdir().expect("tempdir");
    setup_project(&temp);

    let session_id = ask_once(&temp, "first-response", "first prompt");

    let output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .env("MOSAIC_MOCK_CHAT_RESPONSE", "second-response")
        .args(["--project-state", "session", "resume", &session_id])
        .write_stdin("follow-up prompt\n/exit\n")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let stdout = String::from_utf8(output).expect("stdout utf8");
    assert!(stdout.contains(&format!("Resumed session: {session_id}")));
    assert!(stdout.contains("assistant> second-response"));

    let show_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args(["--project-state", "--json", "session", "show", &session_id])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let show_json: Value = serde_json::from_slice(&show_output).expect("session show json");
    assert!(
        show_json["events"]
            .as_array()
            .expect("events")
            .iter()
            .any(|event| event["payload"]["text"].as_str() == Some("second-response"))
    );
}

#[test]
#[allow(deprecated)]
fn session_clear_supports_single_and_all() {
    let temp = tempdir().expect("tempdir");
    setup_project(&temp);

    let first_session = ask_once(&temp, "first-response", "first prompt");
    let second_session = ask_once(&temp, "second-response", "second prompt");

    let clear_one_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "session",
            "clear",
            &first_session,
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let clear_one_json: Value = serde_json::from_slice(&clear_one_output).expect("clear one json");
    assert_eq!(clear_one_json["removed_session"], first_session);

    let list_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args(["--project-state", "--json", "session", "list"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let list_json: Value = serde_json::from_slice(&list_output).expect("session list json");
    let sessions = list_json["sessions"].as_array().expect("sessions");
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0]["session_id"], second_session);

    let clear_all_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args(["--project-state", "--json", "session", "clear", "--all"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let clear_all_json: Value = serde_json::from_slice(&clear_all_output).expect("clear all json");
    assert_eq!(clear_all_json["removed"], 1);

    let final_list_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args(["--project-state", "session", "list"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let stdout = String::from_utf8(final_list_output).expect("stdout utf8");
    assert!(stdout.contains("No sessions found."));
}
