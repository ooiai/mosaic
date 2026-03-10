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
