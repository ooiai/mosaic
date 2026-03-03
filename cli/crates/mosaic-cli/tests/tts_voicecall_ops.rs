use std::fs;

use assert_cmd::Command;
use serde_json::Value;
use tempfile::tempdir;

fn parse_stdout_json(output: &[u8]) -> Value {
    serde_json::from_slice(output).expect("stdout json")
}

#[test]
#[allow(deprecated)]
fn tts_voices_and_speak_flow() {
    let temp = tempdir().expect("tempdir");

    let voices = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args(["--project-state", "--json", "tts", "voices"])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_eq!(voices["ok"], true);
    assert!(
        voices["voices"]
            .as_array()
            .expect("voices array")
            .iter()
            .any(|value| value == "alloy")
    );

    let out_path = temp.path().join("tts-out.txt");
    let speak = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args([
                "--project-state",
                "--json",
                "tts",
                "speak",
                "--text",
                "hello tts",
                "--voice",
                "alloy",
                "--format",
                "txt",
                "--out",
                out_path.to_str().expect("out path"),
            ])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_eq!(speak["ok"], true);
    assert_eq!(speak["voice"], "alloy");
    assert_eq!(speak["format"], "txt");
    assert!(out_path.exists());
    let payload = fs::read_to_string(&out_path).expect("tts payload");
    assert!(payload.contains("mosaic-tts-mock"));
}

#[test]
#[allow(deprecated)]
fn voicecall_start_send_history_stop_flow() {
    let temp = tempdir().expect("tempdir");

    let start = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args([
                "--project-state",
                "--json",
                "voicecall",
                "start",
                "--target",
                "ops-room",
            ])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_eq!(start["ok"], true);
    assert_eq!(start["state"]["active"], true);

    let send = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args([
                "--project-state",
                "--json",
                "voicecall",
                "send",
                "--text",
                "hello call",
            ])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_eq!(send["ok"], true);

    let history = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args([
                "--project-state",
                "--json",
                "voicecall",
                "history",
                "--tail",
                "10",
            ])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_eq!(history["ok"], true);
    let events = history["events"].as_array().expect("events array");
    assert!(events.iter().any(|event| event["direction"] == "outbound"));

    let stop = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args(["--project-state", "--json", "voicecall", "stop"])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_eq!(stop["ok"], true);
    assert_eq!(stop["state"]["active"], false);
}

#[test]
#[allow(deprecated)]
fn voicecall_send_without_start_returns_validation_error() {
    let temp = tempdir().expect("tempdir");

    Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "voicecall",
            "send",
            "--text",
            "hello call",
        ])
        .assert()
        .failure()
        .code(7);
}
