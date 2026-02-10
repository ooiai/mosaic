use assert_cmd::Command;
use serde_json::Value;
use tempfile::tempdir;

#[test]
#[allow(deprecated)]
fn agents_add_route_and_ask_flow() {
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

    let add_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "agents",
            "add",
            "--id",
            "writer",
            "--name",
            "Writer",
            "--model",
            "mock-model",
            "--temperature",
            "0.7",
            "--max-turns",
            "10",
            "--set-default",
            "--route",
            "ask",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let add_json: Value = serde_json::from_slice(&add_output).expect("add json");
    assert_eq!(add_json["ok"], true);
    assert_eq!(add_json["agent"]["id"], "writer");
    assert_eq!(add_json["routes"]["default_agent_id"], "writer");

    let list_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args(["--project-state", "--json", "agents", "list"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let list_json: Value = serde_json::from_slice(&list_output).expect("list json");
    assert_eq!(list_json["ok"], true);
    assert!(
        list_json["agents"]
            .as_array()
            .expect("agents array")
            .iter()
            .any(|item| item["id"].as_str() == Some("writer"))
    );

    let resolve_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "agents",
            "route",
            "resolve",
            "--route",
            "ask",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let resolve_json: Value = serde_json::from_slice(&resolve_output).expect("resolve json");
    assert_eq!(resolve_json["ok"], true);
    assert_eq!(resolve_json["agent_id"], "writer");

    let ask_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .env("MOSAIC_MOCK_CHAT_RESPONSE", "agent-route-ok")
        .args(["--project-state", "--json", "ask", "hello from agents"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let ask_json: Value = serde_json::from_slice(&ask_output).expect("ask json");
    assert_eq!(ask_json["ok"], true);
    assert_eq!(ask_json["response"], "agent-route-ok");
    assert_eq!(ask_json["agent_id"], "writer");

    let remove_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args(["--project-state", "--json", "agents", "remove", "writer"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let remove_json: Value = serde_json::from_slice(&remove_output).expect("remove json");
    assert_eq!(remove_json["ok"], true);
    assert_eq!(remove_json["removed"], true);
}

#[test]
#[allow(deprecated)]
fn agents_show_missing_returns_validation_error() {
    let temp = tempdir().expect("tempdir");
    let output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args(["--project-state", "--json", "agents", "show", "missing"])
        .assert()
        .failure()
        .code(7)
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output).expect("json error");
    assert_eq!(json["ok"], false);
    assert_eq!(json["error"]["code"], "validation");
}
