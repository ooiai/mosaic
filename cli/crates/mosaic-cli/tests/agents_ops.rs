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

    let update_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "agents",
            "update",
            "writer",
            "--name",
            "Writer V2",
            "--clear-temperature",
            "--tools-enabled",
            "false",
            "--route",
            "chat",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let update_json: Value = serde_json::from_slice(&update_output).expect("update json");
    assert_eq!(update_json["ok"], true);
    assert_eq!(update_json["agent"]["name"], "Writer V2");
    assert!(update_json["agent"]["temperature"].is_null());
    assert_eq!(update_json["agent"]["tools_enabled"], false);

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
    let listed_writer = list_json["agents"]
        .as_array()
        .expect("agents array")
        .iter()
        .find(|item| item["id"].as_str() == Some("writer"))
        .expect("writer in agents list");
    assert_eq!(listed_writer["is_default"], true);
    assert_eq!(
        listed_writer["route_keys"]
            .as_array()
            .expect("route_keys")
            .len(),
        2
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

    let resolve_chat_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "agents",
            "route",
            "resolve",
            "--route",
            "chat",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let resolve_chat_json: Value =
        serde_json::from_slice(&resolve_chat_output).expect("resolve chat json");
    assert_eq!(resolve_chat_json["ok"], true);
    assert_eq!(resolve_chat_json["agent_id"], "writer");

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
fn agents_list_text_includes_default_and_route_tags() {
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
            "--set-default",
            "--route",
            "ask",
        ])
        .assert()
        .success();

    Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "agents",
            "route",
            "set",
            "chat",
            "writer",
        ])
        .assert()
        .success();

    let output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args(["--project-state", "agents", "list"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let stdout = String::from_utf8(output).expect("utf8");
    assert!(stdout.contains("default"));
    assert!(stdout.contains("routes=ask,chat") || stdout.contains("routes=chat,ask"));
}

#[test]
#[allow(deprecated)]
fn agents_current_explains_runtime_resolution_precedence() {
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
            "route",
            "set",
            "chat",
            "writer",
        ])
        .assert()
        .success();

    let seeded = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .env("MOSAIC_MOCK_CHAT_RESPONSE", "seed reviewer")
        .args([
            "--project-state",
            "--json",
            "chat",
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
    let seeded_json: Value = serde_json::from_slice(&seeded).expect("seeded json");
    let session_id = seeded_json["session_id"]
        .as_str()
        .expect("session id")
        .to_string();

    let session_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "agents",
            "current",
            "--route",
            "chat",
            "--session",
            &session_id,
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let session_json: Value = serde_json::from_slice(&session_output).expect("session current");
    assert_eq!(session_json["resolution_source"], "session_runtime");
    assert_eq!(session_json["session_agent_id"], "reviewer");
    assert_eq!(session_json["route_agent_id"], "writer");
    assert_eq!(session_json["default_agent_id"], "reviewer");
    assert_eq!(session_json["resolved_agent_id"], "reviewer");

    let route_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "agents",
            "current",
            "--route",
            "chat",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let route_json: Value = serde_json::from_slice(&route_output).expect("route current");
    assert_eq!(route_json["resolution_source"], "route_binding");
    assert_eq!(route_json["resolved_agent_id"], "writer");

    let explicit_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "agents",
            "current",
            "--agent",
            "writer",
            "--route",
            "chat",
            "--session",
            &session_id,
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let explicit_json: Value = serde_json::from_slice(&explicit_output).expect("explicit current");
    assert_eq!(explicit_json["resolution_source"], "explicit_agent");
    assert_eq!(explicit_json["resolved_agent_id"], "writer");
}

#[test]
#[allow(deprecated)]
fn ask_session_resume_prefers_session_runtime_agent_over_current_default() {
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
        .env("MOSAIC_MOCK_CHAT_RESPONSE", "writer-first")
        .args([
            "--project-state",
            "--json",
            "ask",
            "--agent",
            "writer",
            "hello writer",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let first_json: Value = serde_json::from_slice(&first_output).expect("first ask json");
    let session_id = first_json["session_id"]
        .as_str()
        .expect("session id")
        .to_string();
    assert_eq!(first_json["agent_id"], "writer");

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

    let second_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .env("MOSAIC_MOCK_CHAT_RESPONSE", "writer-second")
        .args([
            "--project-state",
            "--json",
            "ask",
            "--session",
            &session_id,
            "resume writer session",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let second_json: Value = serde_json::from_slice(&second_output).expect("second ask json");
    assert_eq!(second_json["agent_id"], "writer");
    assert_eq!(second_json["session_id"], session_id);

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
    assert_eq!(show_json["runtime"]["agent_id"], "writer");
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

#[test]
#[allow(deprecated)]
fn agents_update_conflicting_flags_returns_validation_error() {
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
        ])
        .assert()
        .success();

    let output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "agents",
            "update",
            "writer",
            "--model",
            "mock-model",
            "--clear-model",
        ])
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

#[test]
#[allow(deprecated)]
fn agents_add_rejects_missing_skill() {
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
        .args([
            "--project-state",
            "--json",
            "agents",
            "add",
            "--id",
            "writer",
            "--name",
            "Writer",
            "--skill",
            "missing-skill",
        ])
        .assert()
        .failure()
        .code(7)
        .get_output()
        .stdout
        .clone();

    let json: Value = serde_json::from_slice(&output).expect("json error");
    assert_eq!(json["ok"], false);
    assert_eq!(json["error"]["code"], "validation");
    assert!(
        json["error"]["message"]
            .as_str()
            .unwrap_or_default()
            .contains("missing-skill")
    );
}

#[test]
#[allow(deprecated)]
fn agents_with_skill_injects_skill_prompt() {
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

    let skill_dir = temp.path().join(".mosaic").join("skills").join("writer");
    std::fs::create_dir_all(&skill_dir).expect("create skill dir");
    std::fs::write(
        skill_dir.join("SKILL.md"),
        "# Writer\nAlways answer with concise bullet points.",
    )
    .expect("write skill");

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
            "--skill",
            "writer",
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
    assert_eq!(add_json["agent"]["skills"], serde_json::json!(["writer"]));

    let capture_path = temp.path().join("mock-chat-request.json");
    let ask_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .env("MOSAIC_MOCK_CHAT_RESPONSE", "agent-skill-ok")
        .env("MOSAIC_MOCK_CHAT_CAPTURE_PATH", &capture_path)
        .args(["--project-state", "--json", "ask", "hello from skill agent"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let ask_json: Value = serde_json::from_slice(&ask_output).expect("ask json");
    assert_eq!(ask_json["ok"], true);
    assert_eq!(ask_json["response"], "agent-skill-ok");
    assert_eq!(ask_json["agent_id"], "writer");

    let captured: Value =
        serde_json::from_slice(&std::fs::read(&capture_path).expect("read capture"))
            .expect("capture json");
    let system_prompt = captured["messages"][0]["content"]
        .as_str()
        .expect("system prompt");
    assert!(system_prompt.contains("BEGIN AGENT SKILL: writer"));
    assert!(system_prompt.contains("Always answer with concise bullet points"));
}

#[test]
#[allow(deprecated)]
fn ask_session_resume_keeps_original_agent_when_default_changes() {
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
            "--set-default",
            "--route",
            "ask",
        ])
        .assert()
        .success();

    let first_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .env("MOSAIC_MOCK_CHAT_RESPONSE", "writer-first")
        .args(["--project-state", "--json", "ask", "hello writer"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let first_json: Value = serde_json::from_slice(&first_output).expect("first ask json");
    let session_id = first_json["session_id"]
        .as_str()
        .expect("session id")
        .to_string();
    assert_eq!(first_json["agent_id"], "writer");

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
            "--set-default",
            "--route",
            "ask",
        ])
        .assert()
        .success();

    let second_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .env("MOSAIC_MOCK_CHAT_RESPONSE", "writer-resumed")
        .args([
            "--project-state",
            "--json",
            "ask",
            "--session",
            &session_id,
            "resume writer session",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let second_json: Value = serde_json::from_slice(&second_output).expect("second ask json");
    assert_eq!(second_json["agent_id"], "writer");
    assert_eq!(second_json["session_id"], session_id);

    let show_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args(["--project-state", "--json", "session", "show", &session_id])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let show_json: Value = serde_json::from_slice(&show_output).expect("show json");
    assert_eq!(show_json["runtime"]["agent_id"], "writer");
    assert_eq!(show_json["runtime"]["profile_name"], "default");
}
