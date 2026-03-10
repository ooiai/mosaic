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
fn tts_diagnose_report_out_flow() {
    let temp = tempdir().expect("tempdir");
    let out_path = temp.path().join("tts-diag.txt");
    let report_path = temp.path().join("reports").join("tts-diagnose.json");

    let diagnose = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args([
                "--project-state",
                "--json",
                "tts",
                "diagnose",
                "--voice",
                "alloy",
                "--format",
                "txt",
                "--text",
                "diag smoke",
                "--out",
                out_path.to_str().expect("diag out"),
                "--timeout-ms",
                "2000",
                "--report-out",
                report_path.to_str().expect("report path"),
            ])
            .assert()
            .success()
            .get_output()
            .stdout,
    );

    assert_eq!(diagnose["ok"], true);
    assert_eq!(diagnose["voice"], "alloy");
    assert_eq!(diagnose["format"], "txt");
    assert_eq!(diagnose["timeout_ms"], 2000);
    assert_eq!(diagnose["output"], out_path.display().to_string());
    assert_eq!(diagnose["report_path"], report_path.display().to_string());
    assert!(
        diagnose["checks"]
            .as_array()
            .expect("checks array")
            .iter()
            .any(|value| value["name"] == "synthesis_probe" && value["ok"] == true)
    );

    assert!(out_path.exists());
    assert!(report_path.exists());

    let report: Value =
        serde_json::from_slice(&fs::read(&report_path).expect("read report")).expect("report json");
    assert_eq!(report["module"], "tts");
    assert_eq!(report["command"], "tts diagnose");
    assert_eq!(report["result"]["ok"], true);

    let events_path = temp
        .path()
        .join(".mosaic")
        .join("data")
        .join("tts-events.jsonl");
    let events = fs::read_to_string(events_path).expect("tts events");
    assert!(events.contains("\"voice\":\"alloy\""));
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
fn voicecall_event_log_redacts_secret_payload() {
    let temp = tempdir().expect("tempdir");

    Command::cargo_bin("mosaic")
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
        .success();

    Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "voicecall",
            "send",
            "--text",
            "token=sk-live-secret-12345678901234567890",
        ])
        .assert()
        .success();

    let events_path = temp
        .path()
        .join(".mosaic")
        .join("data")
        .join("voicecall-events.jsonl");
    let events = fs::read_to_string(events_path).expect("voicecall events");
    assert!(events.contains("token=[REDACTED]"));
    assert!(!events.contains("sk-live-secret-12345678901234567890"));
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

#[test]
#[allow(deprecated)]
fn voicecall_send_routes_via_bound_channel() {
    let temp = tempdir().expect("tempdir");

    let add_channel = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args([
                "--project-state",
                "--json",
                "channels",
                "add",
                "--name",
                "voice-terminal",
                "--kind",
                "terminal",
            ])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    let channel_id = add_channel["channel"]["id"]
        .as_str()
        .expect("channel id")
        .to_string();

    parse_stdout_json(
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
                "--channel-id",
                &channel_id,
            ])
            .assert()
            .success()
            .get_output()
            .stdout,
    );

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
                "voicecall via channel",
            ])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_eq!(send["ok"], true);
    assert_eq!(send["channel_delivery"]["channel_id"], channel_id);
    assert_eq!(send["channel_delivery"]["delivered_via"], "terminal");
    assert_eq!(send["channel_delivery"]["http_status"], 200);

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
    let events = history["events"].as_array().expect("events array");
    let outbound = events
        .iter()
        .find(|event| event["direction"] == "channel_outbound")
        .expect("channel outbound event");
    assert_eq!(outbound["channel_id"], channel_id);
    assert_eq!(outbound["delivered_via"], "terminal");
    assert_eq!(outbound["delivery_status"], "success");
}
