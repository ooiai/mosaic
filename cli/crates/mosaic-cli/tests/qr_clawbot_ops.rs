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
fn clawbot_ask_prompt_file_routes_to_ask_runtime() {
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

    let prompt_path = temp.path().join("clawbot-ask.txt");
    std::fs::write(&prompt_path, "hello from clawbot ask prompt file").expect("write prompt file");

    let output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .env("MOSAIC_MOCK_CHAT_RESPONSE", "clawbot-file-ok")
        .args([
            "--project-state",
            "--json",
            "clawbot",
            "ask",
            "--prompt-file",
            prompt_path.to_str().expect("prompt path"),
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output).expect("json");
    assert_eq!(json["ok"], true);
    assert_eq!(json["response"], "clawbot-file-ok");
    assert!(json["session_id"].is_string());
}

#[test]
#[allow(deprecated)]
fn clawbot_ask_script_routes_to_ask_runtime() {
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

    let script_path = temp.path().join("clawbot-ask-script.txt");
    std::fs::write(&script_path, "first clawbot ask\nsecond clawbot ask\n")
        .expect("write script file");

    let output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .env("MOSAIC_MOCK_CHAT_RESPONSE", "clawbot-ask-script-ok")
        .args([
            "--project-state",
            "--json",
            "clawbot",
            "ask",
            "--script",
            script_path.to_str().expect("script path"),
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output).expect("json");
    assert_eq!(json["ok"], true);
    assert_eq!(json["mode"], "script");
    assert_eq!(json["run_count"], 2);
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
        .args(["--project-state", "--json", "clawbot", "send", "hello send"])
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
fn clawbot_send_text_file_routes_to_ask_runtime() {
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

    let text_path = temp.path().join("clawbot-send.txt");
    std::fs::write(&text_path, "hello from clawbot send file").expect("write text file");

    let output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .env("MOSAIC_MOCK_CHAT_RESPONSE", "clawbot-send-file-ok")
        .args([
            "--project-state",
            "--json",
            "clawbot",
            "send",
            "--text-file",
            text_path.to_str().expect("text path"),
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output).expect("json");
    assert_eq!(json["ok"], true);
    assert_eq!(json["response"], "clawbot-send-file-ok");
    assert!(json["session_id"].is_string());
}

#[test]
#[allow(deprecated)]
fn clawbot_chat_script_routes_to_chat_runtime() {
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

    let script_path = temp.path().join("clawbot-chat-script.txt");
    std::fs::write(&script_path, "first clawbot chat\nsecond clawbot chat\n")
        .expect("write script file");

    let output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .env("MOSAIC_MOCK_CHAT_RESPONSE", "clawbot-chat-script-ok")
        .args([
            "--project-state",
            "--json",
            "clawbot",
            "chat",
            "--script",
            script_path.to_str().expect("script path"),
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output).expect("json");
    assert_eq!(json["ok"], true);
    assert_eq!(json["mode"], "script");
    assert_eq!(json["run_count"], 2);
    assert!(json["session_id"].is_string());
}

#[test]
#[allow(deprecated)]
fn clawbot_chat_prompt_file_routes_to_chat_runtime() {
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

    let prompt_path = temp.path().join("clawbot-chat-prompt.txt");
    std::fs::write(&prompt_path, "hello from clawbot chat prompt file").expect("write prompt file");

    let output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .env("MOSAIC_MOCK_CHAT_RESPONSE", "clawbot-chat-file-ok")
        .args([
            "--project-state",
            "--json",
            "clawbot",
            "chat",
            "--prompt-file",
            prompt_path.to_str().expect("prompt path"),
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output).expect("json");
    assert_eq!(json["ok"], true);
    assert_eq!(json["response"], "clawbot-chat-file-ok");
    assert!(json["session_id"].is_string());
}

#[test]
#[allow(deprecated)]
fn clawbot_ask_script_dash_reads_prompts_from_stdin() {
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
        .env("MOSAIC_MOCK_CHAT_RESPONSE", "clawbot-ask-stdin-script-ok")
        .args([
            "--project-state",
            "--json",
            "clawbot",
            "ask",
            "--script",
            "-",
        ])
        .write_stdin("first clawbot ask stdin\nsecond clawbot ask stdin\n")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output).expect("json");
    assert_eq!(json["ok"], true);
    assert_eq!(json["mode"], "script");
    assert_eq!(json["run_count"], 2);
}

#[test]
#[allow(deprecated)]
fn clawbot_send_text_file_dash_reads_text_from_stdin() {
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
        .env("MOSAIC_MOCK_CHAT_RESPONSE", "clawbot-send-stdin-file-ok")
        .args([
            "--project-state",
            "--json",
            "clawbot",
            "send",
            "--text-file",
            "-",
        ])
        .write_stdin("hello from clawbot send stdin file\n")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output).expect("json");
    assert_eq!(json["ok"], true);
    assert_eq!(json["response"], "clawbot-send-stdin-file-ok");
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
