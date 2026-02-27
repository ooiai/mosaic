use assert_cmd::Command;
use serde_json::Value;
use tempfile::tempdir;

#[allow(deprecated)]
fn setup_project(temp: &tempfile::TempDir) {
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

#[test]
#[allow(deprecated)]
fn ask_prompt_file_reads_prompt() {
    let temp = tempdir().expect("tempdir");
    setup_project(&temp);

    let prompt_path = temp.path().join("ask-prompt.txt");
    std::fs::write(&prompt_path, "summarize prompt file flow").expect("write prompt file");

    let output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .env("MOSAIC_MOCK_CHAT_RESPONSE", "prompt-file-ask-ok")
        .args([
            "--project-state",
            "--json",
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
    assert_eq!(json["response"], "prompt-file-ask-ok");
}

#[test]
#[allow(deprecated)]
fn chat_json_prompt_file_reads_prompt() {
    let temp = tempdir().expect("tempdir");
    setup_project(&temp);

    let prompt_path = temp.path().join("chat-prompt.txt");
    std::fs::write(&prompt_path, "hello from chat prompt file").expect("write prompt file");

    let output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .env("MOSAIC_MOCK_CHAT_RESPONSE", "prompt-file-chat-ok")
        .args([
            "--project-state",
            "--json",
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
    assert_eq!(json["response"], "prompt-file-chat-ok");
}

#[test]
#[allow(deprecated)]
fn chat_script_json_runs_multiple_prompts() {
    let temp = tempdir().expect("tempdir");
    setup_project(&temp);

    let script_path = temp.path().join("chat-script.txt");
    std::fs::write(&script_path, "first prompt\n\nsecond prompt\n").expect("write script file");

    let output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .env("MOSAIC_MOCK_CHAT_RESPONSE", "chat-script-ok")
        .args([
            "--project-state",
            "--json",
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

    let runs = json["runs"].as_array().expect("runs array");
    assert_eq!(runs.len(), 2);
    assert_eq!(runs[0]["prompt"], "first prompt");
    assert_eq!(runs[0]["response"], "chat-script-ok");
    assert_eq!(runs[1]["prompt"], "second prompt");
    assert_eq!(runs[1]["response"], "chat-script-ok");
}

#[test]
#[allow(deprecated)]
fn ask_script_json_runs_multiple_prompts() {
    let temp = tempdir().expect("tempdir");
    setup_project(&temp);

    let script_path = temp.path().join("ask-script.txt");
    std::fs::write(&script_path, "first ask prompt\n\nsecond ask prompt\n")
        .expect("write script file");

    let output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .env("MOSAIC_MOCK_CHAT_RESPONSE", "ask-script-ok")
        .args([
            "--project-state",
            "--json",
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

    let runs = json["runs"].as_array().expect("runs array");
    assert_eq!(runs.len(), 2);
    assert_eq!(runs[0]["prompt"], "first ask prompt");
    assert_eq!(runs[0]["response"], "ask-script-ok");
    assert_eq!(runs[1]["prompt"], "second ask prompt");
    assert_eq!(runs[1]["response"], "ask-script-ok");
}
