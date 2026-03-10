use assert_cmd::Command;
use serde_json::Value;
use tempfile::tempdir;

fn parse_stdout_json(output: &[u8]) -> Value {
    serde_json::from_slice(output).expect("stdout json")
}

#[test]
#[allow(deprecated)]
fn mcp_add_list_check_toggle_remove_flow() {
    let temp = tempdir().expect("tempdir");
    let command_path = std::env::current_exe().expect("current exe");
    let command_path = command_path.to_string_lossy().to_string();

    let add = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args([
                "--project-state",
                "--json",
                "mcp",
                "add",
                "--name",
                "local-mcp",
                "--command",
                &command_path,
                "--arg",
                "--version",
                "--env",
                "MCP_MODE=test",
                "--env-from",
                "MCP_PATH=PATH",
                "--cwd",
                temp.path().to_str().expect("cwd"),
            ])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_eq!(add["ok"], true);
    let server_id = add["server"]["id"].as_str().expect("server id").to_string();

    let list = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args(["--project-state", "--json", "mcp", "list"])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_eq!(list["ok"], true);
    let servers = list["servers"].as_array().expect("servers array");
    assert_eq!(servers.len(), 1);
    assert_eq!(servers[0]["id"], server_id.as_str());
    assert_eq!(servers[0]["env_from"]["MCP_PATH"], "PATH");

    let check = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args(["--project-state", "--json", "mcp", "check", &server_id])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_eq!(check["ok"], true);
    assert_eq!(check["all"], false);
    assert_eq!(check["healthy"], true);
    assert_eq!(check["check"]["env_refs"][0]["key"], "MCP_PATH");
    assert_eq!(check["check"]["env_refs"][0]["source"], "PATH");
    assert_eq!(check["check"]["env_refs"][0]["present"], true);

    let show = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args(["--project-state", "--json", "mcp", "show", &server_id])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_eq!(show["ok"], true);
    assert_eq!(show["server"]["id"], server_id.as_str());
    assert_eq!(show["server"]["env_from"]["MCP_PATH"], "PATH");

    let diagnose_report_path = temp.path().join("reports").join("mcp-diagnose.json");
    let diagnose = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args([
                "--project-state",
                "--json",
                "mcp",
                "diagnose",
                &server_id,
                "--timeout-ms",
                "300",
                "--report-out",
                diagnose_report_path.to_string_lossy().as_ref(),
            ])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_eq!(diagnose["ok"], true);
    assert_eq!(diagnose["server"]["id"], server_id.as_str());
    assert_eq!(diagnose["protocol_probe"]["attempted"], true);
    assert!(diagnose["recommendations"].is_array());
    assert_eq!(
        diagnose["report_out"],
        diagnose_report_path.to_string_lossy().to_string()
    );
    assert!(
        diagnose_report_path.exists(),
        "diagnose report should be written when --report-out is provided"
    );

    let check_all = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args(["--project-state", "--json", "mcp", "check", "--all"])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_eq!(check_all["ok"], true);
    assert_eq!(check_all["all"], true);
    assert_eq!(check_all["deep"], false);
    assert_eq!(check_all["checked"], 1);
    assert_eq!(check_all["healthy"], 1);
    assert_eq!(check_all["unhealthy"], 0);

    let deep_report_path = temp.path().join("reports").join("mcp-check-deep.json");
    let deep_check_all = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args([
                "--project-state",
                "--json",
                "mcp",
                "check",
                "--all",
                "--deep",
                "--timeout-ms",
                "300",
                "--report-out",
                deep_report_path.to_string_lossy().as_ref(),
            ])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_eq!(deep_check_all["ok"], true);
    assert_eq!(deep_check_all["all"], true);
    assert_eq!(deep_check_all["deep"], true);
    assert_eq!(deep_check_all["checked"], 1);
    assert!(deep_check_all["protocol_ok"].as_u64().is_some());
    assert!(deep_check_all["protocol_failed"].as_u64().is_some());
    assert!(deep_check_all["probe_skipped"].as_u64().is_some());
    assert!(
        deep_report_path.exists(),
        "deep check report should be written when --report-out is provided"
    );

    let disable = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args(["--project-state", "--json", "mcp", "disable", &server_id])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_eq!(disable["ok"], true);
    assert_eq!(disable["server"]["enabled"], false);

    let disabled_check = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args(["--project-state", "--json", "mcp", "check", &server_id])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_eq!(disabled_check["ok"], true);
    assert_eq!(disabled_check["all"], false);
    assert_eq!(disabled_check["healthy"], false);
    assert!(
        disabled_check["check"]["issues"]
            .as_array()
            .expect("issues")
            .iter()
            .any(|entry| entry.as_str().unwrap_or_default().contains("disabled"))
    );

    let repair_report_path = temp.path().join("reports").join("mcp-repair.json");
    let repair = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args([
                "--project-state",
                "--json",
                "mcp",
                "repair",
                &server_id,
                "--timeout-ms",
                "300",
                "--report-out",
                repair_report_path.to_string_lossy().as_ref(),
            ])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_eq!(repair["ok"], true);
    assert_eq!(repair["checked"], 1);
    assert_eq!(repair["changed"], 1);
    assert_eq!(
        repair["results"][0]["actions"]
            .as_array()
            .expect("actions")
            .iter()
            .filter_map(|value| value.as_str())
            .collect::<Vec<_>>(),
        vec!["enabled_server"]
    );
    assert_eq!(repair["results"][0]["before"]["check_healthy"], false);
    assert_eq!(repair["results"][0]["after"]["check_healthy"], true);
    assert!(
        repair_report_path.exists(),
        "repair report should be written when --report-out is provided"
    );

    let check_all_without_id = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args(["--project-state", "--json", "mcp", "check"])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_eq!(check_all_without_id["ok"], true);
    assert_eq!(check_all_without_id["all"], true);
    assert_eq!(check_all_without_id["checked"], 1);

    let remove = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args(["--project-state", "--json", "mcp", "remove", &server_id])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_eq!(remove["ok"], true);
    assert_eq!(remove["removed"], true);

    let list_after = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args(["--project-state", "--json", "mcp", "list"])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_eq!(list_after["ok"], true);
    assert!(
        list_after["servers"]
            .as_array()
            .expect("servers")
            .is_empty()
    );
}

#[test]
#[allow(deprecated)]
fn mcp_repair_clear_missing_cwd_flow() {
    let temp = tempdir().expect("tempdir");
    let command_path = std::env::current_exe().expect("current exe");
    let command_path = command_path.to_string_lossy().to_string();
    let missing_cwd = temp.path().join("missing-cwd");

    let add = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args([
                "--project-state",
                "--json",
                "mcp",
                "add",
                "--name",
                "cwd-missing",
                "--command",
                &command_path,
                "--cwd",
                missing_cwd.to_string_lossy().as_ref(),
            ])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    let server_id = add["server"]["id"].as_str().expect("server id").to_string();

    let repair_without_flag = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args(["--project-state", "--json", "mcp", "repair", &server_id])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_eq!(repair_without_flag["ok"], true);
    assert_eq!(repair_without_flag["changed"], 0);

    let repair_with_flag = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args([
                "--project-state",
                "--json",
                "mcp",
                "repair",
                &server_id,
                "--clear-missing-cwd",
            ])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_eq!(repair_with_flag["ok"], true);
    assert_eq!(repair_with_flag["changed"], 1);
    assert!(
        repair_with_flag["results"][0]["actions"]
            .as_array()
            .expect("actions")
            .iter()
            .any(|value| value.as_str() == Some("cleared_missing_cwd"))
    );

    let check = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args(["--project-state", "--json", "mcp", "check", &server_id])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_eq!(check["ok"], true);
    assert_eq!(check["check"]["cwd_exists"], true);
    assert!(
        check["check"]["issues"]
            .as_array()
            .expect("issues")
            .iter()
            .all(|entry| !entry.as_str().unwrap_or_default().contains("cwd"))
    );
}

#[test]
#[allow(deprecated)]
fn mcp_update_replaces_runtime_fields_and_reports_noop() {
    let temp = tempdir().expect("tempdir");
    let command_path = std::env::current_exe().expect("current exe");
    let command_path = command_path.to_string_lossy().to_string();

    let add = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args([
                "--project-state",
                "--json",
                "mcp",
                "add",
                "--name",
                "update-me",
                "--command",
                &command_path,
                "--arg",
                "--version",
                "--env",
                "MCP_MODE=before",
                "--env-from",
                "MCP_PATH=PATH",
                "--cwd",
                temp.path().to_str().expect("cwd"),
            ])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    let server_id = add["server"]["id"].as_str().expect("server id").to_string();

    let update = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args([
                "--project-state",
                "--json",
                "mcp",
                "update",
                &server_id,
                "--name",
                "updated-mcp",
                "--clear-args",
                "--env",
                "MCP_MODE=after",
                "--env-from",
                "OPENAI_API_KEY=PATH",
                "--clear-cwd",
                "--disable",
            ])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_eq!(update["ok"], true);
    assert_eq!(update["changed"], true);
    assert_eq!(update["server"]["name"], "updated-mcp");
    assert!(
        update["server"]["args"]
            .as_array()
            .expect("args")
            .is_empty()
    );
    assert_eq!(update["server"]["env"]["MCP_MODE"], "after");
    assert!(update["server"]["env"]["MCP_PATH"].is_null());
    assert_eq!(update["server"]["env_from"]["OPENAI_API_KEY"], "PATH");
    assert!(update["server"]["env_from"]["MCP_PATH"].is_null());
    assert!(update["server"]["cwd"].is_null());
    assert_eq!(update["server"]["enabled"], false);

    let show = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args(["--project-state", "--json", "mcp", "show", &server_id])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_eq!(show["server"]["name"], "updated-mcp");
    assert_eq!(show["server"]["env_from"]["OPENAI_API_KEY"], "PATH");
    assert!(show["server"]["cwd"].is_null());
    assert_eq!(show["server"]["enabled"], false);

    let noop = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args([
                "--project-state",
                "--json",
                "mcp",
                "update",
                &server_id,
                "--name",
                "updated-mcp",
            ])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_eq!(noop["ok"], true);
    assert_eq!(noop["changed"], false);
}

#[cfg(unix)]
#[test]
#[allow(deprecated)]
fn mcp_diagnose_accepts_framed_initialize_response() {
    use std::os::unix::fs::PermissionsExt;

    let temp = tempdir().expect("tempdir");
    let script_path = temp.path().join("framed-mcp.sh");
    std::fs::write(
        &script_path,
        "#!/bin/sh\nbody='{\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{\"capabilities\":{}}}'\nlen=$(printf %s \"$body\" | wc -c | tr -d ' ')\nprintf 'Content-Length: %s\\r\\nContent-Type: application/vscode-jsonrpc; charset=utf-8\\r\\n\\r\\n%s' \"$len\" \"$body\"\nsleep 1\n",
    )
    .expect("write script");
    let mut permissions = std::fs::metadata(&script_path)
        .expect("metadata")
        .permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(&script_path, permissions).expect("set permissions");

    let add = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args([
                "--project-state",
                "--json",
                "mcp",
                "add",
                "--name",
                "framed-mcp",
                "--command",
                script_path.to_string_lossy().as_ref(),
                "--cwd",
                temp.path().to_str().expect("cwd"),
            ])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    let server_id = add["server"]["id"].as_str().expect("server id").to_string();

    let diagnose = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args([
                "--project-state",
                "--json",
                "mcp",
                "diagnose",
                &server_id,
                "--timeout-ms",
                "1000",
            ])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_eq!(diagnose["ok"], true);
    assert_eq!(diagnose["protocol_probe"]["attempted"], true);
    assert_eq!(diagnose["protocol_probe"]["handshake_ok"], true);
    assert_eq!(
        diagnose["protocol_probe"]["initialized_notification_sent"],
        true
    );
    assert_eq!(diagnose["protocol_probe"]["session_ready"], true);
    assert_eq!(diagnose["protocol_probe"]["response_kind"], "result");
    assert_eq!(diagnose["healthy"], true);
}

#[test]
#[allow(deprecated)]
fn mcp_show_missing_server_returns_validation_error() {
    let temp = tempdir().expect("tempdir");

    Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args(["--project-state", "mcp", "show", "missing-server"])
        .assert()
        .failure()
        .code(7);
}

#[test]
#[allow(deprecated)]
fn mcp_add_invalid_env_returns_validation_error() {
    let temp = tempdir().expect("tempdir");

    Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "mcp",
            "add",
            "--name",
            "bad",
            "--command",
            "echo",
            "--env",
            "MISSING_EQUALS",
        ])
        .assert()
        .failure()
        .code(7);
}

#[test]
#[allow(deprecated)]
fn mcp_add_rejects_secret_literal_env_with_guidance() {
    let temp = tempdir().expect("tempdir");

    let output = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args([
                "--project-state",
                "--json",
                "mcp",
                "add",
                "--name",
                "bad",
                "--command",
                "echo",
                "--env",
                "OPENAI_API_KEY=sk-live-secret-12345678901234567890",
            ])
            .assert()
            .failure()
            .code(7)
            .get_output()
            .stdout,
    );
    assert_eq!(output["ok"], false);
    assert_eq!(output["error"]["code"], "validation");
    assert!(
        output["error"]["message"]
            .as_str()
            .unwrap_or_default()
            .contains("process environment that launches Mosaic")
    );
}

#[test]
#[allow(deprecated)]
fn mcp_check_and_doctor_report_missing_env_from_source() {
    let temp = tempdir().expect("tempdir");
    let command_path = std::env::current_exe().expect("current exe");
    let command_path = command_path.to_string_lossy().to_string();
    let missing_env = "MOSAIC_TEST_MCP_MISSING_ENV_SOURCE";
    assert!(
        std::env::var_os(missing_env).is_none(),
        "test requires {missing_env} to be unset"
    );

    let add = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args([
                "--project-state",
                "--json",
                "mcp",
                "add",
                "--name",
                "missing-env",
                "--command",
                &command_path,
                "--arg",
                "--version",
                "--env-from",
                "OPENAI_API_KEY=MOSAIC_TEST_MCP_MISSING_ENV_SOURCE",
                "--cwd",
                temp.path().to_str().expect("cwd"),
            ])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    let server_id = add["server"]["id"].as_str().expect("server id").to_string();

    let check = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args(["--project-state", "--json", "mcp", "check", &server_id])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_eq!(check["ok"], true);
    assert_eq!(check["healthy"], false);
    assert_eq!(check["check"]["env_refs"][0]["key"], "OPENAI_API_KEY");
    assert_eq!(
        check["check"]["env_refs"][0]["source"],
        "MOSAIC_TEST_MCP_MISSING_ENV_SOURCE"
    );
    assert_eq!(check["check"]["env_refs"][0]["present"], false);
    assert!(
        check["check"]["issues"]
            .as_array()
            .expect("issues")
            .iter()
            .any(|entry| entry
                .as_str()
                .unwrap_or_default()
                .contains("MOSAIC_TEST_MCP_MISSING_ENV_SOURCE"))
    );

    let doctor = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args(["--project-state", "--json", "doctor"])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    let checks = doctor["checks"].as_array().expect("doctor checks");
    let mcp_env_refs = checks
        .iter()
        .find(|entry| entry["name"].as_str() == Some("mcp_env_refs"))
        .expect("mcp_env_refs check");
    assert_eq!(mcp_env_refs["status"], "warn");
    assert!(
        mcp_env_refs["detail"]
            .as_str()
            .unwrap_or_default()
            .contains("OPENAI_API_KEY<-MOSAIC_TEST_MCP_MISSING_ENV_SOURCE")
    );

    let diagnose = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args([
                "--project-state",
                "--json",
                "mcp",
                "diagnose",
                &server_id,
                "--timeout-ms",
                "300",
            ])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert!(
        diagnose["recommendations"]
            .as_array()
            .expect("recommendations")
            .iter()
            .any(|entry| entry
                .as_str()
                .unwrap_or_default()
                .contains("--set-env-from OPENAI_API_KEY=<ENV_NAME>"))
    );
}

#[test]
#[allow(deprecated)]
fn mcp_add_invalid_env_from_returns_validation_error() {
    let temp = tempdir().expect("tempdir");

    Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "mcp",
            "add",
            "--name",
            "bad",
            "--command",
            "echo",
            "--env-from",
            "OPENAI_API_KEY=bad-value",
        ])
        .assert()
        .failure()
        .code(7);
}

#[test]
#[allow(deprecated)]
fn mcp_repair_set_env_from_recovers_missing_env_source() {
    let temp = tempdir().expect("tempdir");
    let command_path = std::env::current_exe().expect("current exe");
    let command_path = command_path.to_string_lossy().to_string();

    let add = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args([
                "--project-state",
                "--json",
                "mcp",
                "add",
                "--name",
                "repair-env",
                "--command",
                &command_path,
                "--arg",
                "--version",
                "--env-from",
                "OPENAI_API_KEY=MOSAIC_TEST_MCP_MISSING_ENV_SOURCE",
                "--cwd",
                temp.path().to_str().expect("cwd"),
            ])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    let server_id = add["server"]["id"].as_str().expect("server id").to_string();

    let repair = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args([
                "--project-state",
                "--json",
                "mcp",
                "repair",
                &server_id,
                "--timeout-ms",
                "300",
                "--set-env-from",
                "OPENAI_API_KEY=PATH",
            ])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_eq!(repair["ok"], true);
    assert_eq!(repair["changed"], 1);
    assert_eq!(repair["set_env_from"]["OPENAI_API_KEY"], "PATH");
    assert!(
        repair["results"][0]["actions"]
            .as_array()
            .expect("actions")
            .iter()
            .any(|entry| entry.as_str() == Some("updated_env_from"))
    );
    assert_eq!(
        repair["results"][0]["before"]["env_from"]["OPENAI_API_KEY"],
        "MOSAIC_TEST_MCP_MISSING_ENV_SOURCE"
    );
    assert_eq!(
        repair["results"][0]["after"]["env_from"]["OPENAI_API_KEY"],
        "PATH"
    );
    assert_eq!(repair["results"][0]["after"]["check_healthy"], true);

    let check = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args(["--project-state", "--json", "mcp", "check", &server_id])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_eq!(check["healthy"], true);
    assert_eq!(check["check"]["env_refs"][0]["source"], "PATH");
    assert_eq!(check["check"]["env_refs"][0]["present"], true);
}

#[test]
#[allow(deprecated)]
fn mcp_diagnose_invalid_timeout_returns_validation_error() {
    let temp = tempdir().expect("tempdir");

    Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "mcp",
            "diagnose",
            "missing",
            "--timeout-ms",
            "0",
        ])
        .assert()
        .failure()
        .code(7);
}
