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
                "MCP_TOKEN=test",
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
