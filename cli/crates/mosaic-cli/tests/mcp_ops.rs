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
    assert_eq!(check_all["checked"], 1);
    assert_eq!(check_all["healthy"], 1);
    assert_eq!(check_all["unhealthy"], 0);

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

    let enable = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args(["--project-state", "--json", "mcp", "enable", &server_id])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_eq!(enable["ok"], true);
    assert_eq!(enable["server"]["enabled"], true);

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
