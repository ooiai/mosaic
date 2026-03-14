#![cfg(unix)]

use std::io::{Read, Write};
use std::time::Duration;

use assert_cmd::Command;
use portable_pty::{Child, CommandBuilder, ExitStatus, PtySize, native_pty_system};
use tempfile::tempdir;

#[test]
#[allow(deprecated)]
fn tui_interactive_supports_send_new_session_and_exit() {
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

    let bin = assert_cmd::cargo::cargo_bin("mosaic");
    let pty = native_pty_system();
    let pair = pty
        .openpty(PtySize {
            rows: 40,
            cols: 140,
            pixel_width: 0,
            pixel_height: 0,
        })
        .expect("open pty");

    let mut cmd = CommandBuilder::new(bin);
    cmd.cwd(temp.path());
    cmd.env("MOSAIC_MOCK_CHAT_RESPONSE", "tui-pty-ok");
    cmd.arg("--project-state");
    cmd.arg("tui");

    let mut child = pair.slave.spawn_command(cmd).expect("spawn tui");
    drop(pair.slave);
    let mut reader = pair.master.try_clone_reader().expect("clone reader");
    let mut writer = pair.master.take_writer().expect("writer");
    let drain = std::thread::spawn(move || {
        let mut buf = [0u8; 4096];
        loop {
            match reader.read(&mut buf) {
                Ok(0) => break,
                Ok(_) => {}
                Err(_) => break,
            }
        }
    });

    std::thread::sleep(Duration::from_millis(300));
    writer.write_all(b"hello from pty").expect("write prompt");
    writer.write_all(b"\r").expect("send enter");
    writer.flush().expect("flush prompt");

    let sessions_dir = temp.path().join(".mosaic/data/sessions");
    let started = std::time::Instant::now();
    while started.elapsed() < Duration::from_secs(6) {
        let found = std::fs::read_dir(&sessions_dir)
            .ok()
            .map(|entries| {
                entries.flatten().any(|entry| {
                    std::fs::read_to_string(entry.path())
                        .map(|content| content.contains("tui-pty-ok"))
                        .unwrap_or(false)
                })
            })
            .unwrap_or(false);
        if found {
            break;
        }
        std::thread::sleep(Duration::from_millis(120));
    }

    writer.write_all(&[0x0e]).expect("ctrl+n");
    writer.flush().expect("flush ctrl+n");

    std::thread::sleep(Duration::from_millis(300));
    writer.write_all(b"q").expect("send q");
    writer.flush().expect("flush q");

    let status = wait_with_timeout(child.as_mut(), Duration::from_secs(8));
    drop(writer);
    let _ = drain.join();
    assert_eq!(status.exit_code(), 0, "unexpected exit status: {status:?}");

    let entries = std::fs::read_dir(&sessions_dir)
        .expect("sessions dir")
        .flatten()
        .collect::<Vec<_>>();
    assert!(
        !entries.is_empty(),
        "expected session jsonl files in {}",
        sessions_dir.display()
    );

    let found = entries.iter().any(|entry| {
        std::fs::read_to_string(entry.path())
            .map(|content| content.contains("tui-pty-ok"))
            .unwrap_or(false)
    });
    assert!(found, "expected persisted assistant reply from TUI run");
}

#[test]
#[allow(deprecated)]
fn tui_interactive_json_mode_returns_validation_error() {
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

    let bin = assert_cmd::cargo::cargo_bin("mosaic");
    let pty = native_pty_system();
    let pair = pty
        .openpty(PtySize {
            rows: 20,
            cols: 100,
            pixel_width: 0,
            pixel_height: 0,
        })
        .expect("open pty");

    let mut cmd = CommandBuilder::new(bin);
    cmd.cwd(temp.path());
    cmd.arg("--project-state");
    cmd.arg("--json");
    cmd.arg("tui");

    let mut child = pair.slave.spawn_command(cmd).expect("spawn tui");
    drop(pair.slave);

    let status = wait_with_timeout(child.as_mut(), Duration::from_secs(8));
    assert_eq!(status.exit_code(), 7, "unexpected exit status: {status:?}");
}

#[test]
#[allow(deprecated)]
fn tui_interactive_without_session_prefers_latest_session_runtime_agent() {
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
        .env("MOSAIC_MOCK_CHAT_RESPONSE", "writer-seed")
        .args([
            "--project-state",
            "--json",
            "tui",
            "--agent",
            "writer",
            "--prompt",
            "seed latest session",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let first_json: serde_json::Value =
        serde_json::from_slice(&first_output).expect("first tui json");
    let session_id = first_json["session_id"]
        .as_str()
        .expect("session id")
        .to_string();

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

    let bin = assert_cmd::cargo::cargo_bin("mosaic");
    let pty = native_pty_system();
    let pair = pty
        .openpty(PtySize {
            rows: 40,
            cols: 140,
            pixel_width: 0,
            pixel_height: 0,
        })
        .expect("open pty");

    let mut cmd = CommandBuilder::new(bin);
    cmd.cwd(temp.path());
    cmd.env("MOSAIC_MOCK_CHAT_RESPONSE", "writer-resume");
    cmd.arg("--project-state");
    cmd.arg("tui");

    let mut child = pair.slave.spawn_command(cmd).expect("spawn tui");
    drop(pair.slave);
    let mut reader = pair.master.try_clone_reader().expect("clone reader");
    let mut writer = pair.master.take_writer().expect("writer");
    let drain = std::thread::spawn(move || {
        let mut buf = [0u8; 4096];
        loop {
            match reader.read(&mut buf) {
                Ok(0) => break,
                Ok(_) => {}
                Err(_) => break,
            }
        }
    });

    std::thread::sleep(Duration::from_millis(300));
    writer.write_all(b"resume latest").expect("write prompt");
    writer.write_all(b"\r").expect("send enter");
    writer.flush().expect("flush prompt");

    let started = std::time::Instant::now();
    while started.elapsed() < Duration::from_secs(6) {
        let show_output = Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args(["--project-state", "--json", "session", "show", &session_id])
            .output()
            .expect("session show");
        if show_output.status.success() {
            let show_json: serde_json::Value =
                serde_json::from_slice(&show_output.stdout).expect("session show json");
            let runtime = &show_json["runtime"];
            let has_reply = show_json["events"]
                .as_array()
                .expect("events")
                .iter()
                .any(|event| event["payload"]["text"].as_str() == Some("writer-resume"));
            if runtime["agent_id"] == "writer" && has_reply {
                break;
            }
        }
        std::thread::sleep(Duration::from_millis(120));
    }

    writer.write_all(b"q").expect("send q");
    writer.flush().expect("flush q");

    let status = wait_with_timeout(child.as_mut(), Duration::from_secs(8));
    drop(writer);
    let _ = drain.join();
    assert_eq!(status.exit_code(), 0, "unexpected exit status: {status:?}");

    let show_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args(["--project-state", "--json", "session", "show", &session_id])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let show_json: serde_json::Value =
        serde_json::from_slice(&show_output).expect("session show json");
    assert_eq!(show_json["runtime"]["agent_id"], "writer");
    assert!(
        show_json["events"]
            .as_array()
            .expect("events")
            .iter()
            .any(|event| event["payload"]["text"].as_str() == Some("writer-resume"))
    );
}

#[test]
#[allow(deprecated)]
fn tui_interactive_agent_command_switches_before_first_turn() {
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

    let bin = assert_cmd::cargo::cargo_bin("mosaic");
    let pty = native_pty_system();
    let pair = pty
        .openpty(PtySize {
            rows: 40,
            cols: 140,
            pixel_width: 0,
            pixel_height: 0,
        })
        .expect("open pty");

    let mut cmd = CommandBuilder::new(bin);
    cmd.cwd(temp.path());
    cmd.env("MOSAIC_MOCK_CHAT_RESPONSE", "tui-agent-switch-before");
    cmd.arg("--project-state");
    cmd.arg("tui");

    let mut child = pair.slave.spawn_command(cmd).expect("spawn tui");
    drop(pair.slave);
    let mut reader = pair.master.try_clone_reader().expect("clone reader");
    let mut writer = pair.master.take_writer().expect("writer");
    let drain = std::thread::spawn(move || {
        let mut buf = [0u8; 4096];
        loop {
            match reader.read(&mut buf) {
                Ok(0) => break,
                Ok(_) => {}
                Err(_) => break,
            }
        }
    });

    std::thread::sleep(Duration::from_millis(300));
    writer
        .write_all(b"/agent writer")
        .expect("write agent command");
    writer.write_all(b"\r").expect("send agent command");
    writer
        .write_all(b"hello after switch")
        .expect("write prompt");
    writer.write_all(b"\r").expect("send prompt");
    writer.flush().expect("flush");

    let started = std::time::Instant::now();
    while started.elapsed() < Duration::from_secs(6) {
        let list_output = Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args(["--project-state", "--json", "session", "list"])
            .output()
            .expect("session list");
        if list_output.status.success() {
            let list_json: serde_json::Value =
                serde_json::from_slice(&list_output.stdout).expect("session list json");
            if let Some(session_id) = list_json["sessions"][0]["session_id"].as_str() {
                let show_output = Command::cargo_bin("mosaic")
                    .expect("binary")
                    .current_dir(temp.path())
                    .args(["--project-state", "--json", "session", "show", session_id])
                    .output()
                    .expect("session show");
                if show_output.status.success() {
                    let show_json: serde_json::Value =
                        serde_json::from_slice(&show_output.stdout).expect("session show json");
                    let has_reply =
                        show_json["events"]
                            .as_array()
                            .expect("events")
                            .iter()
                            .any(|event| {
                                event["payload"]["text"].as_str() == Some("tui-agent-switch-before")
                            });
                    if show_json["runtime"]["agent_id"] == "writer" && has_reply {
                        break;
                    }
                }
            }
        }
        std::thread::sleep(Duration::from_millis(120));
    }

    writer.write_all(b"q").expect("send q");
    writer.flush().expect("flush q");

    let status = wait_with_timeout(child.as_mut(), Duration::from_secs(8));
    drop(writer);
    let _ = drain.join();
    assert_eq!(status.exit_code(), 0, "unexpected exit status: {status:?}");

    let list_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args(["--project-state", "--json", "session", "list"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let list_json: serde_json::Value =
        serde_json::from_slice(&list_output).expect("session list json");
    assert_eq!(list_json["sessions"].as_array().expect("sessions").len(), 1);
    let session_id = list_json["sessions"][0]["session_id"]
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
}

#[test]
#[allow(deprecated)]
fn tui_interactive_agent_command_resets_session_after_history() {
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

    let bin = assert_cmd::cargo::cargo_bin("mosaic");
    let pty = native_pty_system();
    let pair = pty
        .openpty(PtySize {
            rows: 40,
            cols: 140,
            pixel_width: 0,
            pixel_height: 0,
        })
        .expect("open pty");

    let mut cmd = CommandBuilder::new(bin);
    cmd.cwd(temp.path());
    cmd.env("MOSAIC_MOCK_CHAT_RESPONSE", "tui-agent-switch-reset");
    cmd.arg("--project-state");
    cmd.arg("tui");

    let mut child = pair.slave.spawn_command(cmd).expect("spawn tui");
    drop(pair.slave);
    let mut reader = pair.master.try_clone_reader().expect("clone reader");
    let mut writer = pair.master.take_writer().expect("writer");
    let drain = std::thread::spawn(move || {
        let mut buf = [0u8; 4096];
        loop {
            match reader.read(&mut buf) {
                Ok(0) => break,
                Ok(_) => {}
                Err(_) => break,
            }
        }
    });

    std::thread::sleep(Duration::from_millis(300));
    writer.write_all(b"first turn").expect("write first prompt");
    writer.write_all(b"\r").expect("send first prompt");
    writer.flush().expect("flush first prompt");

    let started = std::time::Instant::now();
    while started.elapsed() < Duration::from_secs(6) {
        let list_output = Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args(["--project-state", "--json", "session", "list"])
            .output()
            .expect("session list");
        if list_output.status.success() {
            let list_json: serde_json::Value =
                serde_json::from_slice(&list_output.stdout).expect("session list json");
            if list_json["sessions"].as_array().expect("sessions").len() == 1 {
                break;
            }
        }
        std::thread::sleep(Duration::from_millis(120));
    }

    writer
        .write_all(b"/agent writer")
        .expect("write agent command");
    writer.write_all(b"\r").expect("send agent command");
    writer
        .write_all(b"second turn")
        .expect("write second prompt");
    writer.write_all(b"\r").expect("send second prompt");
    writer.flush().expect("flush second prompt");

    let started = std::time::Instant::now();
    while started.elapsed() < Duration::from_secs(6) {
        let list_output = Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args(["--project-state", "--json", "session", "list"])
            .output()
            .expect("session list");
        if list_output.status.success() {
            let list_json: serde_json::Value =
                serde_json::from_slice(&list_output.stdout).expect("session list json");
            if list_json["sessions"].as_array().expect("sessions").len() == 2 {
                break;
            }
        }
        std::thread::sleep(Duration::from_millis(120));
    }

    writer.write_all(b"q").expect("send q");
    writer.flush().expect("flush q");

    let status = wait_with_timeout(child.as_mut(), Duration::from_secs(8));
    drop(writer);
    let _ = drain.join();
    assert_eq!(status.exit_code(), 0, "unexpected exit status: {status:?}");

    let list_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args(["--project-state", "--json", "session", "list"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let list_json: serde_json::Value =
        serde_json::from_slice(&list_output).expect("session list json");
    let sessions = list_json["sessions"].as_array().expect("sessions");
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
fn tui_interactive_session_selection_reuses_selected_session_agent() {
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
        ])
        .assert()
        .success();

    let writer_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .env("MOSAIC_MOCK_CHAT_RESPONSE", "writer-seed")
        .args([
            "--project-state",
            "--json",
            "tui",
            "--agent",
            "writer",
            "--prompt",
            "seed writer session",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let writer_json: serde_json::Value =
        serde_json::from_slice(&writer_output).expect("writer json");
    let writer_session = writer_json["session_id"]
        .as_str()
        .expect("writer session")
        .to_string();

    let reviewer_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .env("MOSAIC_MOCK_CHAT_RESPONSE", "reviewer-seed")
        .args([
            "--project-state",
            "--json",
            "tui",
            "--agent",
            "reviewer",
            "--prompt",
            "seed reviewer session",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let reviewer_json: serde_json::Value =
        serde_json::from_slice(&reviewer_output).expect("reviewer json");
    let reviewer_session = reviewer_json["session_id"]
        .as_str()
        .expect("reviewer session")
        .to_string();

    let bin = assert_cmd::cargo::cargo_bin("mosaic");
    let pty = native_pty_system();
    let pair = pty
        .openpty(PtySize {
            rows: 40,
            cols: 140,
            pixel_width: 0,
            pixel_height: 0,
        })
        .expect("open pty");

    let mut cmd = CommandBuilder::new(bin);
    cmd.cwd(temp.path());
    cmd.env("MOSAIC_MOCK_CHAT_RESPONSE", "writer-after-select");
    cmd.arg("--project-state");
    cmd.arg("tui");
    cmd.arg("--focus");
    cmd.arg("sessions");
    cmd.arg("--no-inspector");

    let mut child = pair.slave.spawn_command(cmd).expect("spawn tui");
    drop(pair.slave);
    let mut reader = pair.master.try_clone_reader().expect("clone reader");
    let mut writer = pair.master.take_writer().expect("writer");
    let drain = std::thread::spawn(move || {
        let mut buf = [0u8; 4096];
        loop {
            match reader.read(&mut buf) {
                Ok(0) => break,
                Ok(_) => {}
                Err(_) => break,
            }
        }
    });

    std::thread::sleep(Duration::from_millis(300));
    writer.write_all(b"\x1b[B").expect("arrow down");
    writer.write_all(b"\r").expect("select session");
    writer.write_all(b"\t").expect("tab to messages");
    writer.write_all(b"\t").expect("tab to input");
    writer
        .write_all(b"continue selected writer session")
        .expect("write prompt");
    writer.write_all(b"\r").expect("send prompt");
    writer.flush().expect("flush prompt");

    let started = std::time::Instant::now();
    while started.elapsed() < Duration::from_secs(6) {
        let show_output = Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args([
                "--project-state",
                "--json",
                "session",
                "show",
                &writer_session,
            ])
            .output()
            .expect("session show");
        if show_output.status.success() {
            let show_json: serde_json::Value =
                serde_json::from_slice(&show_output.stdout).expect("writer show json");
            let has_reply = show_json["events"]
                .as_array()
                .expect("events")
                .iter()
                .any(|event| event["payload"]["text"].as_str() == Some("writer-after-select"));
            if show_json["runtime"]["agent_id"] == "writer" && has_reply {
                break;
            }
        }
        std::thread::sleep(Duration::from_millis(120));
    }

    writer.write_all(b"q").expect("send q");
    writer.flush().expect("flush q");

    let status = wait_with_timeout(child.as_mut(), Duration::from_secs(8));
    drop(writer);
    let _ = drain.join();
    assert_eq!(status.exit_code(), 0, "unexpected exit status: {status:?}");

    let writer_show_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "session",
            "show",
            &writer_session,
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let writer_show_json: serde_json::Value =
        serde_json::from_slice(&writer_show_output).expect("writer show json");
    assert_eq!(writer_show_json["runtime"]["agent_id"], "writer");
    assert!(
        writer_show_json["events"]
            .as_array()
            .expect("events")
            .iter()
            .any(|event| event["payload"]["text"].as_str() == Some("writer-after-select"))
    );

    let reviewer_show_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "session",
            "show",
            &reviewer_session,
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let reviewer_show_json: serde_json::Value =
        serde_json::from_slice(&reviewer_show_output).expect("reviewer show json");
    assert_eq!(reviewer_show_json["runtime"]["agent_id"], "reviewer");
    assert!(
        !reviewer_show_json["events"]
            .as_array()
            .expect("events")
            .iter()
            .any(|event| event["payload"]["text"].as_str() == Some("writer-after-select"))
    );
}

#[test]
#[allow(deprecated)]
fn tui_interactive_session_picker_reuses_selected_session_agent() {
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
        ])
        .assert()
        .success();

    let writer_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .env("MOSAIC_MOCK_CHAT_RESPONSE", "writer-seed")
        .args([
            "--project-state",
            "--json",
            "tui",
            "--agent",
            "writer",
            "--prompt",
            "seed writer session",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let writer_json: serde_json::Value =
        serde_json::from_slice(&writer_output).expect("writer json");
    let writer_session = writer_json["session_id"]
        .as_str()
        .expect("writer session")
        .to_string();

    let reviewer_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .env("MOSAIC_MOCK_CHAT_RESPONSE", "reviewer-seed")
        .args([
            "--project-state",
            "--json",
            "tui",
            "--agent",
            "reviewer",
            "--prompt",
            "seed reviewer session",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let reviewer_json: serde_json::Value =
        serde_json::from_slice(&reviewer_output).expect("reviewer json");
    let reviewer_session = reviewer_json["session_id"]
        .as_str()
        .expect("reviewer session")
        .to_string();

    let bin = assert_cmd::cargo::cargo_bin("mosaic");
    let pty = native_pty_system();
    let pair = pty
        .openpty(PtySize {
            rows: 40,
            cols: 140,
            pixel_width: 0,
            pixel_height: 0,
        })
        .expect("open pty");

    let mut cmd = CommandBuilder::new(bin);
    cmd.cwd(temp.path());
    cmd.env("MOSAIC_MOCK_CHAT_RESPONSE", "writer-after-picker");
    cmd.arg("--project-state");
    cmd.arg("tui");
    cmd.arg("--no-inspector");

    let mut child = pair.slave.spawn_command(cmd).expect("spawn tui");
    drop(pair.slave);
    let mut reader = pair.master.try_clone_reader().expect("clone reader");
    let mut writer = pair.master.take_writer().expect("writer");
    let drain = std::thread::spawn(move || {
        let mut buf = [0u8; 4096];
        loop {
            match reader.read(&mut buf) {
                Ok(0) => break,
                Ok(_) => {}
                Err(_) => break,
            }
        }
    });

    std::thread::sleep(Duration::from_millis(300));
    writer.write_all(&[0x13]).expect("ctrl+s");
    writer.write_all(b"\x1b[B").expect("arrow down");
    writer.write_all(b"\r").expect("select session");
    writer
        .write_all(b"continue via picker")
        .expect("write prompt");
    writer.write_all(b"\r").expect("send prompt");
    writer.flush().expect("flush prompt");

    let started = std::time::Instant::now();
    while started.elapsed() < Duration::from_secs(6) {
        let show_output = Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args([
                "--project-state",
                "--json",
                "session",
                "show",
                &writer_session,
            ])
            .output()
            .expect("session show");
        if show_output.status.success() {
            let show_json: serde_json::Value =
                serde_json::from_slice(&show_output.stdout).expect("writer show json");
            let has_reply = show_json["events"]
                .as_array()
                .expect("events")
                .iter()
                .any(|event| event["payload"]["text"].as_str() == Some("writer-after-picker"));
            if show_json["runtime"]["agent_id"] == "writer" && has_reply {
                break;
            }
        }
        std::thread::sleep(Duration::from_millis(120));
    }

    writer.write_all(b"q").expect("send q");
    writer.flush().expect("flush q");

    let status = wait_with_timeout(child.as_mut(), Duration::from_secs(8));
    drop(writer);
    let _ = drain.join();
    assert_eq!(status.exit_code(), 0, "unexpected exit status: {status:?}");

    let writer_show_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "session",
            "show",
            &writer_session,
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let writer_show_json: serde_json::Value =
        serde_json::from_slice(&writer_show_output).expect("writer show json");
    assert_eq!(writer_show_json["runtime"]["agent_id"], "writer");
    assert!(
        writer_show_json["events"]
            .as_array()
            .expect("events")
            .iter()
            .any(|event| event["payload"]["text"].as_str() == Some("writer-after-picker"))
    );

    let reviewer_show_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "session",
            "show",
            &reviewer_session,
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let reviewer_show_json: serde_json::Value =
        serde_json::from_slice(&reviewer_show_output).expect("reviewer show json");
    assert_eq!(reviewer_show_json["runtime"]["agent_id"], "reviewer");
    assert!(
        !reviewer_show_json["events"]
            .as_array()
            .expect("events")
            .iter()
            .any(|event| event["payload"]["text"].as_str() == Some("writer-after-picker"))
    );
}

#[test]
#[allow(deprecated)]
fn tui_interactive_session_command_and_new_session_work() {
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

    let writer_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .env("MOSAIC_MOCK_CHAT_RESPONSE", "writer-seed")
        .args([
            "--project-state",
            "--json",
            "tui",
            "--agent",
            "writer",
            "--prompt",
            "seed writer session",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let writer_json: serde_json::Value =
        serde_json::from_slice(&writer_output).expect("writer json");
    let writer_session = writer_json["session_id"]
        .as_str()
        .expect("writer session")
        .to_string();

    let bin = assert_cmd::cargo::cargo_bin("mosaic");
    let pty = native_pty_system();
    let pair = pty
        .openpty(PtySize {
            rows: 40,
            cols: 140,
            pixel_width: 0,
            pixel_height: 0,
        })
        .expect("open pty");

    let mut cmd = CommandBuilder::new(bin);
    cmd.cwd(temp.path());
    cmd.env("MOSAIC_MOCK_CHAT_RESPONSE", "writer-command-continue");
    cmd.arg("--project-state");
    cmd.arg("tui");

    let mut child = pair.slave.spawn_command(cmd).expect("spawn tui");
    drop(pair.slave);
    let mut reader = pair.master.try_clone_reader().expect("clone reader");
    let mut writer = pair.master.take_writer().expect("writer");
    let drain = std::thread::spawn(move || {
        let mut buf = [0u8; 4096];
        loop {
            match reader.read(&mut buf) {
                Ok(0) => break,
                Ok(_) => {}
                Err(_) => break,
            }
        }
    });

    std::thread::sleep(Duration::from_millis(300));
    writer
        .write_all(format!("/session {writer_session}").as_bytes())
        .expect("write session command");
    writer.write_all(b"\r").expect("send session command");
    writer.write_all(b"/status").expect("write status");
    writer.write_all(b"\r").expect("send status");
    writer
        .write_all(b"continue selected session")
        .expect("write prompt");
    writer.write_all(b"\r").expect("send prompt");
    writer.flush().expect("flush");

    let started = std::time::Instant::now();
    while started.elapsed() < Duration::from_secs(6) {
        let show_output = Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args([
                "--project-state",
                "--json",
                "session",
                "show",
                &writer_session,
            ])
            .output()
            .expect("session show");
        if show_output.status.success() {
            let show_json: serde_json::Value =
                serde_json::from_slice(&show_output.stdout).expect("session show json");
            let has_continue =
                show_json["events"]
                    .as_array()
                    .expect("events")
                    .iter()
                    .any(|event| {
                        event["payload"]["text"].as_str() == Some("continue selected session")
                    });
            if has_continue {
                break;
            }
        }
        std::thread::sleep(Duration::from_millis(120));
    }

    writer.write_all(b"/new").expect("write new session");
    writer.write_all(b"\r").expect("send new session");
    writer
        .write_all(b"fresh session turn")
        .expect("write fresh prompt");
    writer.write_all(b"\r").expect("send fresh prompt");
    writer.flush().expect("flush fresh prompt");

    let started = std::time::Instant::now();
    while started.elapsed() < Duration::from_secs(6) {
        let list_output = Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args(["--project-state", "--json", "session", "list"])
            .output()
            .expect("session list");
        if list_output.status.success() {
            let list_json: serde_json::Value =
                serde_json::from_slice(&list_output.stdout).expect("session list json");
            if list_json["sessions"].as_array().expect("sessions").len() >= 2 {
                break;
            }
        }
        std::thread::sleep(Duration::from_millis(120));
    }

    writer.write_all(b"q").expect("send q");
    writer.flush().expect("flush q");

    let status = wait_with_timeout(child.as_mut(), Duration::from_secs(8));
    drop(writer);
    let _ = drain.join();
    assert_eq!(status.exit_code(), 0, "unexpected exit status: {status:?}");

    let writer_show_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "session",
            "show",
            &writer_session,
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let writer_show_json: serde_json::Value =
        serde_json::from_slice(&writer_show_output).expect("writer show json");
    assert_eq!(writer_show_json["runtime"]["agent_id"], "writer");
    assert!(
        writer_show_json["events"]
            .as_array()
            .expect("events")
            .iter()
            .any(|event| event["payload"]["text"].as_str() == Some("writer-command-continue"))
    );

    let list_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args(["--project-state", "--json", "session", "list"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let list_json: serde_json::Value =
        serde_json::from_slice(&list_output).expect("session list json");
    let sessions = list_json["sessions"].as_array().expect("sessions");
    assert_eq!(sessions.len(), 2);
    let fresh_session = sessions
        .iter()
        .find_map(|entry| {
            let session_id = entry["session_id"].as_str()?;
            if session_id != writer_session {
                Some(session_id.to_string())
            } else {
                None
            }
        })
        .expect("fresh session");

    let fresh_show_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "session",
            "show",
            &fresh_session,
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let fresh_show_json: serde_json::Value =
        serde_json::from_slice(&fresh_show_output).expect("fresh show json");
    assert_eq!(fresh_show_json["runtime"]["agent_id"], "writer");
    assert!(
        fresh_show_json["events"]
            .as_array()
            .expect("events")
            .iter()
            .any(|event| event["payload"]["text"].as_str() == Some("fresh session turn"))
    );
    assert!(
        !fresh_show_json["events"]
            .as_array()
            .expect("events")
            .iter()
            .any(|event| event["payload"]["text"].as_str() == Some("continue selected session"))
    );
}

#[test]
#[allow(deprecated)]
fn tui_interactive_agent_picker_switches_before_first_turn() {
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

    let bin = assert_cmd::cargo::cargo_bin("mosaic");
    let pty = native_pty_system();
    let pair = pty
        .openpty(PtySize {
            rows: 40,
            cols: 140,
            pixel_width: 0,
            pixel_height: 0,
        })
        .expect("open pty");

    let mut cmd = CommandBuilder::new(bin);
    cmd.cwd(temp.path());
    cmd.env("MOSAIC_MOCK_CHAT_RESPONSE", "tui-picker-before");
    cmd.arg("--project-state");
    cmd.arg("tui");

    let mut child = pair.slave.spawn_command(cmd).expect("spawn tui");
    drop(pair.slave);
    let mut reader = pair.master.try_clone_reader().expect("clone reader");
    let mut writer = pair.master.take_writer().expect("writer");
    let drain = std::thread::spawn(move || {
        let mut buf = [0u8; 4096];
        loop {
            match reader.read(&mut buf) {
                Ok(0) => break,
                Ok(_) => {}
                Err(_) => break,
            }
        }
    });

    std::thread::sleep(Duration::from_millis(300));
    writer.write_all(&[0x01]).expect("ctrl+a");
    writer.write_all(b"\r").expect("pick agent");
    writer
        .write_all(b"hello after picker switch")
        .expect("write prompt");
    writer.write_all(b"\r").expect("send prompt");
    writer.flush().expect("flush");

    let started = std::time::Instant::now();
    while started.elapsed() < Duration::from_secs(6) {
        let list_output = Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args(["--project-state", "--json", "session", "list"])
            .output()
            .expect("session list");
        if list_output.status.success() {
            let list_json: serde_json::Value =
                serde_json::from_slice(&list_output.stdout).expect("session list json");
            if let Some(session_id) = list_json["sessions"][0]["session_id"].as_str() {
                let show_output = Command::cargo_bin("mosaic")
                    .expect("binary")
                    .current_dir(temp.path())
                    .args(["--project-state", "--json", "session", "show", session_id])
                    .output()
                    .expect("session show");
                if show_output.status.success() {
                    let show_json: serde_json::Value =
                        serde_json::from_slice(&show_output.stdout).expect("session show json");
                    let has_reply =
                        show_json["events"]
                            .as_array()
                            .expect("events")
                            .iter()
                            .any(|event| {
                                event["payload"]["text"].as_str() == Some("tui-picker-before")
                            });
                    if show_json["runtime"]["agent_id"] == "writer" && has_reply {
                        break;
                    }
                }
            }
        }
        std::thread::sleep(Duration::from_millis(120));
    }

    writer.write_all(b"q").expect("send q");
    writer.flush().expect("flush q");

    let status = wait_with_timeout(child.as_mut(), Duration::from_secs(8));
    drop(writer);
    let _ = drain.join();
    assert_eq!(status.exit_code(), 0, "unexpected exit status: {status:?}");

    let list_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args(["--project-state", "--json", "session", "list"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let list_json: serde_json::Value =
        serde_json::from_slice(&list_output).expect("session list json");
    let sessions = list_json["sessions"].as_array().expect("sessions");
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0]["runtime"]["agent_id"], "writer");

    let session_id = sessions[0]["session_id"].as_str().expect("session id");
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
    assert!(
        show_json["events"]
            .as_array()
            .expect("events")
            .iter()
            .any(|event| event["payload"]["text"].as_str() == Some("tui-picker-before"))
    );
}

#[test]
#[allow(deprecated)]
fn tui_interactive_agents_command_opens_picker_and_switches() {
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

    let bin = assert_cmd::cargo::cargo_bin("mosaic");
    let pty = native_pty_system();
    let pair = pty
        .openpty(PtySize {
            rows: 40,
            cols: 140,
            pixel_width: 0,
            pixel_height: 0,
        })
        .expect("open pty");

    let mut cmd = CommandBuilder::new(bin);
    cmd.cwd(temp.path());
    cmd.env("MOSAIC_MOCK_CHAT_RESPONSE", "tui-agents-command");
    cmd.arg("--project-state");
    cmd.arg("tui");

    let mut child = pair.slave.spawn_command(cmd).expect("spawn tui");
    drop(pair.slave);
    let mut reader = pair.master.try_clone_reader().expect("clone reader");
    let mut writer = pair.master.take_writer().expect("writer");
    let drain = std::thread::spawn(move || {
        let mut buf = [0u8; 4096];
        loop {
            match reader.read(&mut buf) {
                Ok(0) => break,
                Ok(_) => {}
                Err(_) => break,
            }
        }
    });

    std::thread::sleep(Duration::from_millis(300));
    writer.write_all(b"/agents").expect("write agents command");
    writer.write_all(b"\r").expect("send agents command");
    writer.write_all(b"\r").expect("select listed agent");
    writer
        .write_all(b"hello after agents command")
        .expect("write prompt");
    writer.write_all(b"\r").expect("send prompt");
    writer.flush().expect("flush");

    let started = std::time::Instant::now();
    while started.elapsed() < Duration::from_secs(6) {
        let list_output = Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args(["--project-state", "--json", "session", "list"])
            .output()
            .expect("session list");
        if list_output.status.success() {
            let list_json: serde_json::Value =
                serde_json::from_slice(&list_output.stdout).expect("session list json");
            if let Some(session_id) = list_json["sessions"][0]["session_id"].as_str() {
                let show_output = Command::cargo_bin("mosaic")
                    .expect("binary")
                    .current_dir(temp.path())
                    .args(["--project-state", "--json", "session", "show", session_id])
                    .output()
                    .expect("session show");
                if show_output.status.success() {
                    let show_json: serde_json::Value =
                        serde_json::from_slice(&show_output.stdout).expect("session show json");
                    let has_reply =
                        show_json["events"]
                            .as_array()
                            .expect("events")
                            .iter()
                            .any(|event| {
                                event["payload"]["text"].as_str() == Some("tui-agents-command")
                            });
                    if show_json["runtime"]["agent_id"] == "writer" && has_reply {
                        break;
                    }
                }
            }
        }
        std::thread::sleep(Duration::from_millis(120));
    }

    writer.write_all(b"q").expect("send q");
    writer.flush().expect("flush q");

    let status = wait_with_timeout(child.as_mut(), Duration::from_secs(8));
    drop(writer);
    let _ = drain.join();
    assert_eq!(status.exit_code(), 0, "unexpected exit status: {status:?}");
}

#[test]
#[allow(deprecated)]
fn tui_interactive_agent_picker_resets_session_after_history() {
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

    let bin = assert_cmd::cargo::cargo_bin("mosaic");
    let pty = native_pty_system();
    let pair = pty
        .openpty(PtySize {
            rows: 40,
            cols: 140,
            pixel_width: 0,
            pixel_height: 0,
        })
        .expect("open pty");

    let mut cmd = CommandBuilder::new(bin);
    cmd.cwd(temp.path());
    cmd.env("MOSAIC_MOCK_CHAT_RESPONSE", "tui-picker-reset");
    cmd.arg("--project-state");
    cmd.arg("tui");

    let mut child = pair.slave.spawn_command(cmd).expect("spawn tui");
    drop(pair.slave);
    let mut reader = pair.master.try_clone_reader().expect("clone reader");
    let mut writer = pair.master.take_writer().expect("writer");
    let drain = std::thread::spawn(move || {
        let mut buf = [0u8; 4096];
        loop {
            match reader.read(&mut buf) {
                Ok(0) => break,
                Ok(_) => {}
                Err(_) => break,
            }
        }
    });

    std::thread::sleep(Duration::from_millis(300));
    writer.write_all(b"first turn").expect("write first prompt");
    writer.write_all(b"\r").expect("send first prompt");
    writer.flush().expect("flush first prompt");

    let started = std::time::Instant::now();
    while started.elapsed() < Duration::from_secs(6) {
        let list_output = Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args(["--project-state", "--json", "session", "list"])
            .output()
            .expect("session list");
        if list_output.status.success() {
            let list_json: serde_json::Value =
                serde_json::from_slice(&list_output.stdout).expect("session list json");
            if list_json["sessions"].as_array().expect("sessions").len() == 1 {
                break;
            }
        }
        std::thread::sleep(Duration::from_millis(120));
    }

    writer.write_all(&[0x01]).expect("ctrl+a");
    writer.write_all(b"\x1b[B").expect("arrow down");
    writer.write_all(b"\r").expect("pick writer");
    writer
        .write_all(b"second turn")
        .expect("write second prompt");
    writer.write_all(b"\r").expect("send second prompt");
    writer.flush().expect("flush second prompt");

    let started = std::time::Instant::now();
    while started.elapsed() < Duration::from_secs(6) {
        let list_output = Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args(["--project-state", "--json", "session", "list"])
            .output()
            .expect("session list");
        if list_output.status.success() {
            let list_json: serde_json::Value =
                serde_json::from_slice(&list_output.stdout).expect("session list json");
            if list_json["sessions"].as_array().expect("sessions").len() == 2 {
                break;
            }
        }
        std::thread::sleep(Duration::from_millis(120));
    }

    writer.write_all(b"q").expect("send q");
    writer.flush().expect("flush q");

    let status = wait_with_timeout(child.as_mut(), Duration::from_secs(8));
    drop(writer);
    let _ = drain.join();
    assert_eq!(status.exit_code(), 0, "unexpected exit status: {status:?}");

    let list_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args(["--project-state", "--json", "session", "list"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let list_json: serde_json::Value =
        serde_json::from_slice(&list_output).expect("session list json");
    let sessions = list_json["sessions"].as_array().expect("sessions");
    assert_eq!(sessions.len(), 2);
    assert_eq!(sessions[0]["runtime"]["agent_id"], "writer");
    assert_eq!(sessions[1]["runtime"]["agent_id"], "reviewer");
}

fn wait_with_timeout(child: &mut dyn Child, timeout: Duration) -> ExitStatus {
    let started = std::time::Instant::now();
    loop {
        if let Some(status) = child.try_wait().expect("try_wait") {
            return status;
        }
        if started.elapsed() >= timeout {
            child.kill().expect("kill child");
            return child.wait().expect("wait after kill");
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}
