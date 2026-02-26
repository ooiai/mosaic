use assert_cmd::Command;
use serde_json::Value;
use std::fs;
use tempfile::tempdir;

#[test]
#[allow(deprecated)]
fn qr_encode_json_returns_payload() {
    let output = Command::cargo_bin("mosaic")
        .expect("binary")
        .args(["--json", "qr", "encode", "hello world"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output).expect("json");
    assert_eq!(json["ok"], true);
    assert_eq!(json["kind"], "encode");
    assert_eq!(json["value"], "hello world");
    assert_eq!(json["payload"], "mosaic://qr?value=hello%20world");
}

#[test]
#[allow(deprecated)]
fn qr_pairing_json_returns_payload_with_expiry() {
    let output = Command::cargo_bin("mosaic")
        .expect("binary")
        .args([
            "--json",
            "qr",
            "pairing",
            "--device",
            "dev-1",
            "--node",
            "local",
            "--ttl-seconds",
            "60",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output).expect("json");
    assert_eq!(json["ok"], true);
    assert_eq!(json["kind"], "pairing");
    let payload = json["payload"].as_str().expect("payload");
    assert!(payload.contains("mosaic://pairing?"));
    assert!(payload.contains("device=dev-1"));
    assert!(payload.contains("node=local"));
    assert_eq!(json["ttl_seconds"], 60);
}

#[test]
#[allow(deprecated)]
fn qr_encode_ascii_json_returns_ascii_render() {
    let output = Command::cargo_bin("mosaic")
        .expect("binary")
        .args(["--json", "qr", "encode", "hello world", "--render", "ascii"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output).expect("json");
    assert_eq!(json["ok"], true);
    assert_eq!(json["kind"], "encode");
    assert_eq!(json["render"], "ascii");
    let ascii = json["ascii"].as_str().expect("ascii");
    assert!(ascii.contains("██"));
}

#[test]
#[allow(deprecated)]
fn qr_encode_png_json_writes_output_file() {
    let temp = tempdir().expect("tempdir");
    let png_path = temp.path().join("qr").join("payload.png");
    let output = Command::cargo_bin("mosaic")
        .expect("binary")
        .args([
            "--json",
            "qr",
            "encode",
            "hello world",
            "--render",
            "png",
            "--output",
            png_path.to_str().expect("utf8 path"),
            "--module-size",
            "6",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output).expect("json");
    assert_eq!(json["ok"], true);
    assert_eq!(json["render"], "png");
    assert_eq!(
        json["output"].as_str().expect("output path"),
        png_path.to_str().expect("utf8 path")
    );
    let bytes = fs::read(&png_path).expect("read png");
    assert!(bytes.starts_with(&[0x89, b'P', b'N', b'G', b'\r', b'\n', 0x1a, b'\n']));
}

#[test]
#[allow(deprecated)]
fn clawbot_ask_routes_to_ask_runtime() {
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
        .env("MOSAIC_MOCK_CHAT_RESPONSE", "clawbot-ok")
        .args([
            "--project-state",
            "--json",
            "clawbot",
            "ask",
            "hello clawbot",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output).expect("json");
    assert_eq!(json["ok"], true);
    assert_eq!(json["response"], "clawbot-ok");
    assert!(json["session_id"].is_string());
}

#[test]
#[allow(deprecated)]
fn clawbot_send_routes_to_ask_runtime() {
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
        .env("MOSAIC_MOCK_CHAT_RESPONSE", "clawbot-send-ok")
        .args([
            "--project-state",
            "--json",
            "clawbot",
            "send",
            "hello send",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output).expect("json");
    assert_eq!(json["ok"], true);
    assert_eq!(json["response"], "clawbot-send-ok");
    assert!(json["session_id"].is_string());
}

#[test]
#[allow(deprecated)]
fn clawbot_status_routes_to_status_runtime() {
    let output = Command::cargo_bin("mosaic")
        .expect("binary")
        .args(["--json", "clawbot", "status"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output).expect("json");
    assert_eq!(json["ok"], true);
    assert!(json["configured"].is_boolean());
}
