use assert_cmd::Command;
use serde_json::{Value, json};
use std::path::PathBuf;
use tempfile::tempdir;

fn schema_of(value: &Value) -> Value {
    match value {
        Value::Null => Value::String("null".to_string()),
        Value::Bool(_) => Value::String("bool".to_string()),
        Value::Number(_) => Value::String("number".to_string()),
        Value::String(_) => Value::String("string".to_string()),
        Value::Array(items) => {
            let item_schema = items.first().map(schema_of);
            match item_schema {
                Some(item) => json!({
                    "type": "array",
                    "items": [item],
                }),
                None => json!({
                    "type": "array",
                    "items": [],
                }),
            }
        }
        Value::Object(map) => {
            let mut out = serde_json::Map::new();
            for (key, entry) in map {
                out.insert(key.clone(), schema_of(entry));
            }
            Value::Object(out)
        }
    }
}

fn parse_stdout_json(output: &[u8]) -> Value {
    serde_json::from_slice(output).expect("stdout json")
}

fn test_snapshot_path(relative: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join(relative)
}

fn assert_json_snapshot(relative: &str, actual: &Value) {
    let path = test_snapshot_path(relative);
    if std::env::var_os("MOSAIC_UPDATE_SNAPSHOTS").is_some() {
        let rendered = serde_json::to_string_pretty(actual).expect("render snapshot");
        std::fs::write(&path, rendered).expect("write snapshot");
    }

    let expected_raw = std::fs::read_to_string(&path).expect("read snapshot");
    let expected: Value = serde_json::from_str(&expected_raw).expect("parse snapshot");
    assert_eq!(*actual, expected);
}

fn assert_success_envelope(payload: &Value) {
    assert_eq!(payload["ok"], true);
    assert!(
        payload.get("error").is_none() || payload["error"].is_null(),
        "success payload should not include error envelope: {payload}"
    );
}

#[test]
#[allow(deprecated)]
fn json_channels_module_schema_matches_snapshot() {
    let temp = tempdir().expect("tempdir");

    let setup = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args([
                "--project-state",
                "--json",
                "setup",
                "--base-url",
                "mock://mock-model",
                "--model",
                "mock-model",
            ])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_success_envelope(&setup);

    let add = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args([
                "--project-state",
                "--json",
                "channels",
                "add",
                "--name",
                "contract-slack",
                "--kind",
                "slack_webhook",
                "--endpoint",
                "mock-http://200",
            ])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_success_envelope(&add);
    let channel_id = add["channel"]["id"]
        .as_str()
        .expect("channel id")
        .to_string();

    let list = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args(["--project-state", "--json", "channels", "list"])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_success_envelope(&list);

    let status = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args(["--project-state", "--json", "channels", "status"])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_success_envelope(&status);

    let capabilities = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args([
                "--project-state",
                "--json",
                "channels",
                "capabilities",
                "--target",
                &channel_id,
            ])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_success_envelope(&capabilities);

    let resolve = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args([
                "--project-state",
                "--json",
                "channels",
                "resolve",
                "--channel",
                "slack_webhook",
                "contract",
            ])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_success_envelope(&resolve);

    let test_probe = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args(["--project-state", "--json", "channels", "test", &channel_id])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_success_envelope(&test_probe);

    let send = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args([
                "--project-state",
                "--json",
                "channels",
                "send",
                &channel_id,
                "--text",
                "module schema hello",
            ])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_success_envelope(&send);

    let logs = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args([
                "--project-state",
                "--json",
                "channels",
                "logs",
                "--channel",
                &channel_id,
                "--tail",
                "10",
            ])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_success_envelope(&logs);

    let logs_summary = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args([
                "--project-state",
                "--json",
                "channels",
                "logs",
                "--channel",
                &channel_id,
                "--tail",
                "10",
                "--summary",
            ])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_success_envelope(&logs_summary);

    let actual_schema = json!({
        "add": schema_of(&add),
        "list": schema_of(&list),
        "status": schema_of(&status),
        "capabilities": schema_of(&capabilities),
        "resolve": schema_of(&resolve),
        "test": schema_of(&test_probe),
        "send": schema_of(&send),
        "logs": schema_of(&logs),
        "logs_summary": schema_of(&logs_summary),
    });
    assert_json_snapshot("snapshots/json_module_channels_schema.json", &actual_schema);
}

#[test]
#[allow(deprecated)]
fn json_channels_admin_module_schema_matches_snapshot() {
    let temp = tempdir().expect("tempdir");

    let add = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args([
                "--project-state",
                "--json",
                "channels",
                "add",
                "--name",
                "contract-admin-telegram",
                "--kind",
                "telegram_bot",
                "--chat-id=-1001234567890",
                "--endpoint",
                "mock-http://200",
                "--token-env",
                "MOSAIC_TELEGRAM_BOT_TOKEN_OLD",
            ])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_success_envelope(&add);
    let channel_id = add["channel"]["id"]
        .as_str()
        .expect("channel id")
        .to_string();

    let update = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args([
                "--project-state",
                "--json",
                "channels",
                "update",
                &channel_id,
                "--name",
                "contract-admin-updated",
                "--default-parse-mode",
                "markdown",
                "--default-title",
                "Contract Title",
                "--default-block",
                "line one",
                "--default-block",
                "line two",
                "--default-metadata",
                "{\"suite\":\"channels-admin\"}",
            ])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_success_envelope(&update);

    let login = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args([
                "--project-state",
                "--json",
                "channels",
                "login",
                &channel_id,
            ])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_success_envelope(&login);

    let export_path = temp.path().join("channels-admin-export.json");
    let export = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args([
                "--project-state",
                "--json",
                "channels",
                "export",
                "--out",
                export_path.to_str().expect("export path"),
            ])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_success_envelope(&export);

    let remove_before_import = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args([
                "--project-state",
                "--json",
                "channels",
                "remove",
                &channel_id,
            ])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_success_envelope(&remove_before_import);

    let import_report_path = temp.path().join("channels-admin-import-report.json");
    let import = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args([
                "--project-state",
                "--json",
                "channels",
                "import",
                "--file",
                export_path.to_str().expect("export path"),
                "--report-out",
                import_report_path.to_str().expect("import report path"),
            ])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_success_envelope(&import);

    let list_after_import = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args(["--project-state", "--json", "channels", "list"])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_success_envelope(&list_after_import);
    let imported_channel_id = list_after_import["channels"]
        .as_array()
        .expect("channels list")
        .first()
        .and_then(|entry| entry["id"].as_str())
        .expect("imported channel id")
        .to_string();

    let rotate_dry_run = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args([
                "--project-state",
                "--json",
                "channels",
                "rotate-token-env",
                "--channel",
                &imported_channel_id,
                "--to",
                "MOSAIC_TELEGRAM_BOT_TOKEN_NEW",
                "--dry-run",
            ])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_success_envelope(&rotate_dry_run);

    let rotate_report_path = temp.path().join("channels-admin-rotate-report.json");
    let rotate_apply = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args([
                "--project-state",
                "--json",
                "channels",
                "rotate-token-env",
                "--channel",
                &imported_channel_id,
                "--from",
                "MOSAIC_TELEGRAM_BOT_TOKEN_OLD",
                "--to",
                "MOSAIC_TELEGRAM_BOT_TOKEN_NEW",
                "--report-out",
                rotate_report_path.to_str().expect("rotation report path"),
            ])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_success_envelope(&rotate_apply);

    let logout = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args([
                "--project-state",
                "--json",
                "channels",
                "logout",
                &imported_channel_id,
            ])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_success_envelope(&logout);

    let remove_after_logout = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args([
                "--project-state",
                "--json",
                "channels",
                "remove",
                &imported_channel_id,
            ])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_success_envelope(&remove_after_logout);

    let list_final = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args(["--project-state", "--json", "channels", "list"])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_success_envelope(&list_final);

    let actual_schema = json!({
        "add": schema_of(&add),
        "update": schema_of(&update),
        "login": schema_of(&login),
        "export": schema_of(&export),
        "remove_before_import": schema_of(&remove_before_import),
        "import": schema_of(&import),
        "list_after_import": schema_of(&list_after_import),
        "rotate_dry_run": schema_of(&rotate_dry_run),
        "rotate_apply": schema_of(&rotate_apply),
        "logout": schema_of(&logout),
        "remove_after_logout": schema_of(&remove_after_logout),
        "list_final": schema_of(&list_final),
    });
    assert_json_snapshot(
        "snapshots/json_module_channels_admin_schema.json",
        &actual_schema,
    );
}

#[test]
#[allow(deprecated)]
fn json_gateway_module_schema_matches_snapshot() {
    let temp = tempdir().expect("tempdir");

    let status = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .env("MOSAIC_GATEWAY_TEST_MODE", "1")
            .args(["--project-state", "--json", "gateway", "status"])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_success_envelope(&status);

    let start = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .env("MOSAIC_GATEWAY_TEST_MODE", "1")
            .args(["--project-state", "--json", "gateway", "start"])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_success_envelope(&start);

    let probe = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .env("MOSAIC_GATEWAY_TEST_MODE", "1")
            .args(["--project-state", "--json", "gateway", "probe"])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_success_envelope(&probe);

    let discover = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .env("MOSAIC_GATEWAY_TEST_MODE", "1")
            .args(["--project-state", "--json", "gateway", "discover"])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_success_envelope(&discover);

    let call_status = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .env("MOSAIC_GATEWAY_TEST_MODE", "1")
            .args(["--project-state", "--json", "gateway", "call", "status"])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_success_envelope(&call_status);

    let diagnose = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .env("MOSAIC_GATEWAY_TEST_MODE", "1")
            .args([
                "--project-state",
                "--json",
                "gateway",
                "diagnose",
                "--method",
                "status",
            ])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_success_envelope(&diagnose);

    let stop = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .env("MOSAIC_GATEWAY_TEST_MODE", "1")
            .args(["--project-state", "--json", "gateway", "stop"])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_success_envelope(&stop);

    let actual_schema = json!({
        "status": schema_of(&status),
        "start": schema_of(&start),
        "probe": schema_of(&probe),
        "discover": schema_of(&discover),
        "call_status": schema_of(&call_status),
        "diagnose": schema_of(&diagnose),
        "stop": schema_of(&stop),
    });
    assert_json_snapshot("snapshots/json_module_gateway_schema.json", &actual_schema);
}

#[test]
#[allow(deprecated)]
fn json_gateway_admin_module_schema_matches_snapshot() {
    let temp = tempdir().expect("tempdir");

    let install = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .env("MOSAIC_GATEWAY_TEST_MODE", "1")
            .args([
                "--project-state",
                "--json",
                "gateway",
                "install",
                "--host",
                "127.0.0.1",
                "--port",
                "9898",
            ])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_success_envelope(&install);

    let start = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .env("MOSAIC_GATEWAY_TEST_MODE", "1")
            .args(["--project-state", "--json", "gateway", "start"])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_success_envelope(&start);

    let status_deep_installed = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .env("MOSAIC_GATEWAY_TEST_MODE", "1")
            .args(["--project-state", "--json", "gateway", "status", "--deep"])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_success_envelope(&status_deep_installed);

    let health_verbose = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .env("MOSAIC_GATEWAY_TEST_MODE", "1")
            .args([
                "--project-state",
                "--json",
                "gateway",
                "health",
                "--verbose",
            ])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_success_envelope(&health_verbose);

    let restart = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .env("MOSAIC_GATEWAY_TEST_MODE", "1")
            .args([
                "--project-state",
                "--json",
                "gateway",
                "restart",
                "--port",
                "9899",
            ])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_success_envelope(&restart);

    let status_deep_restarted = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .env("MOSAIC_GATEWAY_TEST_MODE", "1")
            .args(["--project-state", "--json", "gateway", "status", "--deep"])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_success_envelope(&status_deep_restarted);

    let uninstall = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .env("MOSAIC_GATEWAY_TEST_MODE", "1")
            .args(["--project-state", "--json", "gateway", "uninstall"])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_success_envelope(&uninstall);

    let status_deep_uninstalled = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .env("MOSAIC_GATEWAY_TEST_MODE", "1")
            .args(["--project-state", "--json", "gateway", "status", "--deep"])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_success_envelope(&status_deep_uninstalled);

    let actual_schema = json!({
        "install": schema_of(&install),
        "start": schema_of(&start),
        "status_deep_installed": schema_of(&status_deep_installed),
        "health_verbose": schema_of(&health_verbose),
        "restart": schema_of(&restart),
        "status_deep_restarted": schema_of(&status_deep_restarted),
        "uninstall": schema_of(&uninstall),
        "status_deep_uninstalled": schema_of(&status_deep_uninstalled),
    });
    assert_json_snapshot(
        "snapshots/json_module_gateway_admin_schema.json",
        &actual_schema,
    );
}

#[test]
#[allow(deprecated)]
fn json_security_module_schema_matches_snapshot() {
    let temp = tempdir().expect("tempdir");
    let audit_dir = temp.path().join("audit_target");
    std::fs::create_dir_all(&audit_dir).expect("create audit dir");
    std::fs::write(
        audit_dir.join("insecure.txt"),
        "insecure endpoint: http://example.internal/resource\n",
    )
    .expect("write insecure fixture");

    let baseline_show = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args(["--project-state", "--json", "security", "baseline", "show"])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_success_envelope(&baseline_show);

    let audit = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args([
                "--project-state",
                "--json",
                "security",
                "audit",
                "--path",
                audit_dir.to_str().expect("audit dir path"),
            ])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_success_envelope(&audit);

    let actual_schema = json!({
        "baseline_show": schema_of(&baseline_show),
        "audit": schema_of(&audit),
    });
    assert_json_snapshot("snapshots/json_module_security_schema.json", &actual_schema);
}

#[test]
#[allow(deprecated)]
fn json_security_baseline_module_schema_matches_snapshot() {
    let temp = tempdir().expect("tempdir");

    let show_initial = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args(["--project-state", "--json", "security", "baseline", "show"])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_success_envelope(&show_initial);

    let add = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args([
                "--project-state",
                "--json",
                "security",
                "baseline",
                "add",
                "--fingerprint",
                "contract:1:security:baseline",
                "--category",
                "transport_security",
                "--match-path",
                "vendor/*",
            ])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_success_envelope(&add);

    let show_after_add = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args(["--project-state", "--json", "security", "baseline", "show"])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_success_envelope(&show_after_add);

    let remove = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args([
                "--project-state",
                "--json",
                "security",
                "baseline",
                "remove",
                "--category",
                "transport_security",
            ])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_success_envelope(&remove);

    let clear = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args(["--project-state", "--json", "security", "baseline", "clear"])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_success_envelope(&clear);

    let show_after_clear = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args(["--project-state", "--json", "security", "baseline", "show"])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_success_envelope(&show_after_clear);

    let actual_schema = json!({
        "show_initial": schema_of(&show_initial),
        "add": schema_of(&add),
        "show_after_add": schema_of(&show_after_add),
        "remove": schema_of(&remove),
        "clear": schema_of(&clear),
        "show_after_clear": schema_of(&show_after_clear),
    });
    assert_json_snapshot(
        "snapshots/json_module_security_baseline_schema.json",
        &actual_schema,
    );
}

#[test]
#[allow(deprecated)]
fn json_agents_module_schema_matches_snapshot() {
    let temp = tempdir().expect("tempdir");

    let setup = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args([
                "--project-state",
                "--json",
                "setup",
                "--base-url",
                "mock://mock-model",
                "--model",
                "mock-model",
            ])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_success_envelope(&setup);

    let list_before = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args(["--project-state", "--json", "agents", "list"])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_success_envelope(&list_before);

    let add = parse_stdout_json(
        &Command::cargo_bin("mosaic")
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
                "--set-default",
                "--route",
                "ask",
            ])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_success_envelope(&add);

    let list_after = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args(["--project-state", "--json", "agents", "list"])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_success_envelope(&list_after);

    let show = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args(["--project-state", "--json", "agents", "show", "writer"])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_success_envelope(&show);

    let route_resolve = parse_stdout_json(
        &Command::cargo_bin("mosaic")
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
            .stdout,
    );
    assert_success_envelope(&route_resolve);

    let actual_schema = json!({
        "list_before": schema_of(&list_before),
        "add": schema_of(&add),
        "list_after": schema_of(&list_after),
        "show": schema_of(&show),
        "route_resolve": schema_of(&route_resolve),
    });
    let expected_schema: Value =
        serde_json::from_str(include_str!("snapshots/json_module_agents_schema.json"))
            .expect("expected module agents schema");
    assert_eq!(actual_schema, expected_schema);
}

#[test]
#[allow(deprecated)]
fn json_nodes_pairing_module_schema_matches_snapshot() {
    let temp = tempdir().expect("tempdir");

    let nodes_list = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args(["--project-state", "--json", "nodes", "list"])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_success_envelope(&nodes_list);

    let pairing_request_approve = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args([
                "--project-state",
                "--json",
                "pairing",
                "request",
                "--device",
                "contract-dev-approve",
                "--node",
                "local",
                "--reason",
                "contract-approve",
            ])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_success_envelope(&pairing_request_approve);
    let approve_request_id = pairing_request_approve["request"]["id"]
        .as_str()
        .expect("approve request id");

    let pairing_approve = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args([
                "--project-state",
                "--json",
                "pairing",
                "approve",
                approve_request_id,
            ])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_success_envelope(&pairing_approve);

    let pairing_request_reject = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args([
                "--project-state",
                "--json",
                "pairing",
                "request",
                "--device",
                "contract-dev-reject",
                "--node",
                "local",
                "--reason",
                "contract-reject",
            ])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_success_envelope(&pairing_request_reject);
    let reject_request_id = pairing_request_reject["request"]["id"]
        .as_str()
        .expect("reject request id");

    let pairing_reject = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args([
                "--project-state",
                "--json",
                "pairing",
                "reject",
                reject_request_id,
                "--reason",
                "contract-rejected",
            ])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_success_envelope(&pairing_reject);

    let pairing_list_rejected = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args([
                "--project-state",
                "--json",
                "pairing",
                "list",
                "--status",
                "rejected",
            ])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_success_envelope(&pairing_list_rejected);

    let nodes_status_local = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args(["--project-state", "--json", "nodes", "status", "local"])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_success_envelope(&nodes_status_local);

    let nodes_status_summary = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args(["--project-state", "--json", "nodes", "status"])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_success_envelope(&nodes_status_summary);

    let actual_schema = json!({
        "nodes_list": schema_of(&nodes_list),
        "pairing_request_approve": schema_of(&pairing_request_approve),
        "pairing_approve": schema_of(&pairing_approve),
        "pairing_request_reject": schema_of(&pairing_request_reject),
        "pairing_reject": schema_of(&pairing_reject),
        "pairing_list_rejected": schema_of(&pairing_list_rejected),
        "nodes_status_local": schema_of(&nodes_status_local),
        "nodes_status_summary": schema_of(&nodes_status_summary),
    });
    let expected_schema: Value = serde_json::from_str(include_str!(
        "snapshots/json_module_nodes_pairing_schema.json"
    ))
    .expect("expected module nodes/pairing schema");
    assert_eq!(actual_schema, expected_schema);
}

#[test]
#[allow(deprecated)]
fn json_models_module_schema_matches_snapshot() {
    let temp = tempdir().expect("tempdir");

    let setup = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args([
                "--project-state",
                "--json",
                "setup",
                "--base-url",
                "mock://mock-model",
                "--model",
                "mock-model",
            ])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_success_envelope(&setup);

    let list = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args(["--project-state", "--json", "models", "list"])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_success_envelope(&list);

    let list_filtered = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args([
                "--project-state",
                "--json",
                "models",
                "list",
                "--query",
                "mock",
                "--limit",
                "1",
            ])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_success_envelope(&list_filtered);

    let status_before = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args(["--project-state", "--json", "models", "status"])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_success_envelope(&status_before);

    let aliases_set = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args([
                "--project-state",
                "--json",
                "models",
                "aliases",
                "set",
                "fast",
                "mock-model",
            ])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_success_envelope(&aliases_set);

    let fallbacks_add = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args([
                "--project-state",
                "--json",
                "models",
                "fallbacks",
                "add",
                "mock-fallback",
            ])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_success_envelope(&fallbacks_add);

    let resolve_alias = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args(["--project-state", "--json", "models", "resolve", "fast"])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_success_envelope(&resolve_alias);

    let set = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args(["--project-state", "--json", "models", "set", "fast"])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_success_envelope(&set);

    let status_after = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args(["--project-state", "--json", "models", "status"])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_success_envelope(&status_after);

    let actual_schema = json!({
        "list": schema_of(&list),
        "list_filtered": schema_of(&list_filtered),
        "status_before": schema_of(&status_before),
        "aliases_set": schema_of(&aliases_set),
        "fallbacks_add": schema_of(&fallbacks_add),
        "resolve_alias": schema_of(&resolve_alias),
        "set": schema_of(&set),
        "status_after": schema_of(&status_after),
    });
    assert_json_snapshot("snapshots/json_module_models_schema.json", &actual_schema);
}

#[test]
#[allow(deprecated)]
fn json_core_agent_module_schema_matches_snapshot() {
    let temp = tempdir().expect("tempdir");

    let setup = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args([
                "--project-state",
                "--json",
                "setup",
                "--base-url",
                "mock://mock-model",
                "--model",
                "mock-model",
            ])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_success_envelope(&setup);

    let configure_show = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args(["--project-state", "--json", "configure", "--show"])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_success_envelope(&configure_show);

    let configure_keys = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args(["--project-state", "--json", "configure", "keys"])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_success_envelope(&configure_keys);

    let configure_set = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args([
                "--project-state",
                "--json",
                "configure",
                "set",
                "tools.enabled",
                "false",
            ])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_success_envelope(&configure_set);

    let patch_file = temp.path().join("contract-config-patch.json");
    std::fs::write(
        &patch_file,
        r#"{
  "provider": { "model": "mock-model-v2" },
  "tools": { "enabled": true }
}"#,
    )
    .expect("write patch file");

    let configure_patch = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args([
                "--project-state",
                "--json",
                "configure",
                "patch",
                "--file",
                patch_file.to_string_lossy().as_ref(),
                "--set",
                "agent.max_turns=10",
            ])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_success_envelope(&configure_patch);

    let configure_preview = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args([
                "--project-state",
                "--json",
                "configure",
                "preview",
                "--target-profile",
                "migration",
                "--set",
                "provider.model=preview-model",
            ])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_success_envelope(&configure_preview);

    let configure_template = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args([
                "--project-state",
                "--json",
                "configure",
                "template",
                "--target-profile",
                "migration",
                "--format",
                "json",
            ])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_success_envelope(&configure_template);

    let configure_get = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args([
                "--project-state",
                "--json",
                "configure",
                "get",
                "tools.enabled",
            ])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_success_envelope(&configure_get);

    let configure_unset = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args([
                "--project-state",
                "--json",
                "configure",
                "unset",
                "tools.enabled",
            ])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_success_envelope(&configure_unset);

    let ask = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .env("MOSAIC_MOCK_CHAT_RESPONSE", "core-agent-ask")
            .args(["--project-state", "--json", "ask", "core agent ask"])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_success_envelope(&ask);
    let ask_session_id = ask["session_id"]
        .as_str()
        .expect("ask session id")
        .to_string();

    let chat_prompt = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .env("MOSAIC_MOCK_CHAT_RESPONSE", "core-agent-chat")
            .args([
                "--project-state",
                "--json",
                "chat",
                "--prompt",
                "core agent chat prompt",
            ])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_success_envelope(&chat_prompt);

    let session_list = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args(["--project-state", "--json", "session", "list"])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_success_envelope(&session_list);

    let session_show = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args([
                "--project-state",
                "--json",
                "session",
                "show",
                &ask_session_id,
            ])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_success_envelope(&session_show);

    let session_clear = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args([
                "--project-state",
                "--json",
                "session",
                "clear",
                &ask_session_id,
            ])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_success_envelope(&session_clear);

    let status = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args(["--project-state", "--json", "status"])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_success_envelope(&status);

    let health = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args(["--project-state", "--json", "health"])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_success_envelope(&health);

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
    assert_success_envelope(&doctor);

    let actual_schema = json!({
        "setup": schema_of(&setup),
        "configure_show": schema_of(&configure_show),
        "configure_keys": schema_of(&configure_keys),
        "configure_set": schema_of(&configure_set),
        "configure_patch": schema_of(&configure_patch),
        "configure_preview": schema_of(&configure_preview),
        "configure_template": schema_of(&configure_template),
        "configure_get": schema_of(&configure_get),
        "configure_unset": schema_of(&configure_unset),
        "ask": schema_of(&ask),
        "chat_prompt": schema_of(&chat_prompt),
        "session_list": schema_of(&session_list),
        "session_show": schema_of(&session_show),
        "session_clear": schema_of(&session_clear),
        "status": schema_of(&status),
        "health": schema_of(&health),
        "doctor": schema_of(&doctor),
    });
    assert_json_snapshot(
        "snapshots/json_module_core_agent_schema.json",
        &actual_schema,
    );
}

#[test]
#[allow(deprecated)]
fn json_ops_policy_module_schema_matches_snapshot() {
    let temp = tempdir().expect("tempdir");

    let approvals_get = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args(["--project-state", "--json", "approvals", "get"])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_success_envelope(&approvals_get);

    let approvals_set = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args(["--project-state", "--json", "approvals", "set", "deny"])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_success_envelope(&approvals_set);

    let approvals_allowlist_add = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args([
                "--project-state",
                "--json",
                "approvals",
                "allowlist",
                "add",
                "cargo test",
            ])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_success_envelope(&approvals_allowlist_add);

    let approvals_allowlist_list = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args([
                "--project-state",
                "--json",
                "approvals",
                "allowlist",
                "list",
            ])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_success_envelope(&approvals_allowlist_list);

    let approvals_check = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args([
                "--project-state",
                "--json",
                "approvals",
                "check",
                "--command",
                "echo contract",
            ])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_success_envelope(&approvals_check);

    let sandbox_list = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args(["--project-state", "--json", "sandbox", "list"])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_success_envelope(&sandbox_list);

    let sandbox_get = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args(["--project-state", "--json", "sandbox", "get"])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_success_envelope(&sandbox_get);

    let sandbox_set = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args(["--project-state", "--json", "sandbox", "set", "restricted"])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_success_envelope(&sandbox_set);

    let sandbox_check = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args([
                "--project-state",
                "--json",
                "sandbox",
                "check",
                "--command",
                "curl https://example.com",
            ])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_success_envelope(&sandbox_check);

    let sandbox_explain = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args([
                "--project-state",
                "--json",
                "sandbox",
                "explain",
                "--profile",
                "restricted",
            ])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_success_envelope(&sandbox_explain);

    let safety_get = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args(["--project-state", "--json", "safety", "get"])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_success_envelope(&safety_get);

    let safety_check = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args([
                "--project-state",
                "--json",
                "safety",
                "check",
                "--command",
                "cargo test --workspace",
            ])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_success_envelope(&safety_check);

    let safety_report = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args([
                "--project-state",
                "--json",
                "safety",
                "report",
                "--command",
                "curl https://example.com",
            ])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_success_envelope(&safety_report);

    let system_event = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args([
                "--project-state",
                "--json",
                "system",
                "event",
                "contract_event",
                "--data",
                "{\"suite\":\"ops-policy\"}",
            ])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_success_envelope(&system_event);

    let system_presence = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args(["--project-state", "--json", "system", "presence"])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_success_envelope(&system_presence);

    let system_list = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args([
                "--project-state",
                "--json",
                "system",
                "list",
                "--tail",
                "20",
                "--name",
                "contract_event",
            ])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_success_envelope(&system_list);

    let logs = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args(["--project-state", "--json", "logs", "--tail", "20"])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_success_envelope(&logs);

    let observability_report = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args([
                "--project-state",
                "--json",
                "observability",
                "report",
                "--tail",
                "20",
                "--event-tail",
                "20",
            ])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_success_envelope(&observability_report);

    let observability_export = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args([
                "--project-state",
                "--json",
                "observability",
                "export",
                "--out",
                "observability-contract.json",
                "--tail",
                "20",
                "--event-tail",
                "20",
                "--no-doctor",
            ])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_success_envelope(&observability_export);

    let actual_schema = json!({
        "approvals_get": schema_of(&approvals_get),
        "approvals_set": schema_of(&approvals_set),
        "approvals_allowlist_add": schema_of(&approvals_allowlist_add),
        "approvals_allowlist_list": schema_of(&approvals_allowlist_list),
        "approvals_check": schema_of(&approvals_check),
        "sandbox_list": schema_of(&sandbox_list),
        "sandbox_get": schema_of(&sandbox_get),
        "sandbox_set": schema_of(&sandbox_set),
        "sandbox_check": schema_of(&sandbox_check),
        "sandbox_explain": schema_of(&sandbox_explain),
        "safety_get": schema_of(&safety_get),
        "safety_check": schema_of(&safety_check),
        "safety_report": schema_of(&safety_report),
        "system_event": schema_of(&system_event),
        "system_presence": schema_of(&system_presence),
        "system_list": schema_of(&system_list),
        "logs": schema_of(&logs),
        "observability_report": schema_of(&observability_report),
        "observability_export": schema_of(&observability_export),
    });
    assert_json_snapshot(
        "snapshots/json_module_ops_policy_schema.json",
        &actual_schema,
    );
}

#[test]
#[allow(deprecated)]
fn json_automation_module_schema_matches_snapshot() {
    let temp = tempdir().expect("tempdir");

    let setup = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args([
                "--project-state",
                "--json",
                "setup",
                "--base-url",
                "mock://mock-model",
                "--model",
                "mock-model",
            ])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_success_envelope(&setup);

    let hook_add = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args([
                "--project-state",
                "--json",
                "hooks",
                "add",
                "--name",
                "contract-hook",
                "--event",
                "deploy",
                "--command",
                "echo contract-hook",
            ])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_success_envelope(&hook_add);
    let hook_id = hook_add["hook"]["id"]
        .as_str()
        .expect("hook id")
        .to_string();

    let hook_run = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args([
                "--project-state",
                "--yes",
                "--json",
                "hooks",
                "run",
                &hook_id,
                "--data",
                "{\"source\":\"manual\"}",
            ])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_success_envelope(&hook_run);

    let hook_logs = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args([
                "--project-state",
                "--json",
                "hooks",
                "logs",
                "--hook",
                &hook_id,
                "--tail",
                "10",
            ])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_success_envelope(&hook_logs);

    let webhook_add = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args([
                "--project-state",
                "--json",
                "webhooks",
                "add",
                "--name",
                "contract-webhook",
                "--event",
                "deploy",
                "--path",
                "/contract/deploy",
                "--method",
                "post",
            ])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_success_envelope(&webhook_add);
    let webhook_id = webhook_add["webhook"]["id"]
        .as_str()
        .expect("webhook id")
        .to_string();

    let webhook_resolve = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args([
                "--project-state",
                "--yes",
                "--json",
                "webhooks",
                "resolve",
                "--path",
                "/contract/deploy",
                "--method",
                "post",
                "--data",
                "{\"kind\":\"resolve\"}",
            ])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_success_envelope(&webhook_resolve);

    let webhook_logs = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args([
                "--project-state",
                "--json",
                "webhooks",
                "logs",
                "--webhook",
                &webhook_id,
                "--tail",
                "10",
            ])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_success_envelope(&webhook_logs);

    let cron_add = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args([
                "--project-state",
                "--json",
                "cron",
                "add",
                "--name",
                "contract-cron",
                "--event",
                "deploy",
                "--every",
                "1",
                "--data",
                "{\"source\":\"contract\"}",
            ])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_success_envelope(&cron_add);
    let cron_job_id = cron_add["job"]["id"]
        .as_str()
        .expect("cron job id")
        .to_string();

    let cron_tick = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args(["--project-state", "--yes", "--json", "cron", "tick"])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_success_envelope(&cron_tick);

    let cron_run = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args([
                "--project-state",
                "--yes",
                "--json",
                "cron",
                "run",
                &cron_job_id,
                "--data",
                "{\"trigger\":\"manual\"}",
            ])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_success_envelope(&cron_run);

    let cron_logs = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args([
                "--project-state",
                "--json",
                "cron",
                "logs",
                "--job",
                &cron_job_id,
                "--tail",
                "10",
            ])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_success_envelope(&cron_logs);

    let actual_schema = json!({
        "hook_add": schema_of(&hook_add),
        "hook_run": schema_of(&hook_run),
        "hook_logs": schema_of(&hook_logs),
        "webhook_add": schema_of(&webhook_add),
        "webhook_resolve": schema_of(&webhook_resolve),
        "webhook_logs": schema_of(&webhook_logs),
        "cron_add": schema_of(&cron_add),
        "cron_tick": schema_of(&cron_tick),
        "cron_run": schema_of(&cron_run),
        "cron_logs": schema_of(&cron_logs),
    });
    assert_json_snapshot(
        "snapshots/json_module_automation_schema.json",
        &actual_schema,
    );
}

#[test]
#[allow(deprecated)]
fn json_feature_runtime_module_schema_matches_snapshot() {
    let temp = tempdir().expect("tempdir");
    std::fs::write(temp.path().join("README.txt"), "runtime features contract")
        .expect("write readme");
    std::fs::write(temp.path().join("notes.md"), "browser memory plugin skill")
        .expect("write notes");

    let setup = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args([
                "--project-state",
                "--json",
                "setup",
                "--base-url",
                "mock://mock-model",
                "--model",
                "mock-model",
            ])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_success_envelope(&setup);

    let browser_start = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args(["--project-state", "--json", "browser", "start"])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_success_envelope(&browser_start);

    let browser_open = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args([
                "--project-state",
                "--json",
                "browser",
                "navigate",
                "--url",
                "mock://ok?title=Contract+Browser",
            ])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_success_envelope(&browser_open);
    let visit_id = browser_open["visit"]["id"]
        .as_str()
        .expect("visit id")
        .to_string();

    let browser_status = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args(["--project-state", "--json", "browser", "status"])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_success_envelope(&browser_status);

    let browser_history = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args([
                "--project-state",
                "--json",
                "browser",
                "history",
                "--tail",
                "10",
            ])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_success_envelope(&browser_history);

    let browser_tabs = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args([
                "--project-state",
                "--json",
                "browser",
                "tabs",
                "--tail",
                "10",
            ])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_success_envelope(&browser_tabs);

    let browser_show = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args(["--project-state", "--json", "browser", "show", &visit_id])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_success_envelope(&browser_show);

    let browser_focus = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args(["--project-state", "--json", "browser", "focus", &visit_id])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_success_envelope(&browser_focus);

    let browser_snapshot = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args(["--project-state", "--json", "browser", "snapshot"])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_success_envelope(&browser_snapshot);

    let screenshot_out = temp.path().join("browser-artifacts").join("shot.txt");
    let browser_screenshot = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args([
                "--project-state",
                "--json",
                "browser",
                "screenshot",
                "--out",
                screenshot_out.to_str().expect("screenshot output"),
            ])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_success_envelope(&browser_screenshot);

    let browser_clear = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args(["--project-state", "--json", "browser", "clear", &visit_id])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_success_envelope(&browser_clear);

    let browser_close_all = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args(["--project-state", "--json", "browser", "close", "--all"])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_success_envelope(&browser_close_all);

    let browser_stop = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args(["--project-state", "--json", "browser", "stop"])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_success_envelope(&browser_stop);

    let memory_index = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args([
                "--project-state",
                "--json",
                "memory",
                "index",
                "--path",
                ".",
                "--max-files",
                "100",
            ])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_success_envelope(&memory_index);

    let memory_search = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args([
                "--project-state",
                "--json",
                "memory",
                "search",
                "contract",
                "--limit",
                "5",
            ])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_success_envelope(&memory_search);

    let memory_status = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args(["--project-state", "--json", "memory", "status"])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_success_envelope(&memory_status);

    let memory_status_all = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args([
                "--project-state",
                "--json",
                "memory",
                "status",
                "--all-namespaces",
            ])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_success_envelope(&memory_status_all);

    let memory_policy_get = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args(["--project-state", "--json", "memory", "policy", "get"])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_success_envelope(&memory_policy_get);

    let memory_policy_set = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args([
                "--project-state",
                "--json",
                "memory",
                "policy",
                "set",
                "--enabled",
                "true",
                "--max-namespaces",
                "1",
                "--min-interval-minutes",
                "5",
            ])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_success_envelope(&memory_policy_set);

    let memory_policy_apply = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args([
                "--project-state",
                "--json",
                "memory",
                "policy",
                "apply",
                "--dry-run",
                "--force",
            ])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_success_envelope(&memory_policy_apply);

    let memory_clear = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args(["--project-state", "--json", "memory", "clear"])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_success_envelope(&memory_clear);

    let memory_prune = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args([
                "--project-state",
                "--json",
                "memory",
                "prune",
                "--max-namespaces",
                "1",
                "--dry-run",
            ])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_success_envelope(&memory_prune);

    let plugin_source = temp.path().join("sample-plugin");
    let plugin_hooks = plugin_source.join("hooks");
    let skill_source = temp.path().join("writer");
    std::fs::create_dir_all(&plugin_source).expect("create plugin source");
    std::fs::create_dir_all(&plugin_hooks).expect("create plugin hooks");
    std::fs::create_dir_all(&skill_source).expect("create skill source");
    std::fs::write(
        plugin_source.join("plugin.toml"),
        "[plugin]\nid = \"sample_plugin\"\nname = \"Sample Plugin\"\nversion = \"0.1.0\"\n\n[runtime]\nrun = \"hooks/run.sh\"\ndoctor = \"hooks/doctor.sh\"\n",
    )
    .expect("write plugin manifest");
    std::fs::write(plugin_hooks.join("run.sh"), "#!/bin/sh\necho run-ok\n")
        .expect("write run hook");
    std::fs::write(
        plugin_hooks.join("doctor.sh"),
        "#!/bin/sh\necho doctor-ok\n",
    )
    .expect("write doctor hook");
    std::fs::write(
        skill_source.join("SKILL.md"),
        "# Writer\nGenerate concise notes.\n",
    )
    .expect("write skill file");

    let plugins_install = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args([
                "--project-state",
                "--json",
                "plugins",
                "install",
                "--path",
                "sample-plugin",
            ])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_success_envelope(&plugins_install);

    let plugins_list = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args(["--project-state", "--json", "plugins", "list"])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_success_envelope(&plugins_list);

    let plugins_list_project = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args([
                "--project-state",
                "--json",
                "plugins",
                "list",
                "--source",
                "project",
            ])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_success_envelope(&plugins_list_project);

    let plugins_check = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args([
                "--project-state",
                "--json",
                "plugins",
                "check",
                "sample_plugin",
            ])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_success_envelope(&plugins_check);

    let plugins_disable = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args([
                "--project-state",
                "--json",
                "plugins",
                "disable",
                "sample_plugin",
            ])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_success_envelope(&plugins_disable);

    let plugins_doctor = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args(["--project-state", "--json", "plugins", "doctor"])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_success_envelope(&plugins_doctor);

    let plugins_enable = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args([
                "--project-state",
                "--json",
                "plugins",
                "enable",
                "sample_plugin",
            ])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_success_envelope(&plugins_enable);

    let plugins_run = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args([
                "--project-state",
                "--yes",
                "--json",
                "plugins",
                "run",
                "sample_plugin",
                "--hook",
                "run",
                "--arg",
                "schema",
            ])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_success_envelope(&plugins_run);

    let skills_install = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args([
                "--project-state",
                "--json",
                "skills",
                "install",
                "--path",
                "writer",
            ])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_success_envelope(&skills_install);

    let skills_list = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args(["--project-state", "--json", "skills", "list"])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_success_envelope(&skills_list);

    let skills_list_project = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args([
                "--project-state",
                "--json",
                "skills",
                "list",
                "--source",
                "project",
            ])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_success_envelope(&skills_list_project);

    let skills_check = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args(["--project-state", "--json", "skills", "check", "writer"])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_success_envelope(&skills_check);

    let actual_schema = json!({
        "browser_start": schema_of(&browser_start),
        "browser_open": schema_of(&browser_open),
        "browser_status": schema_of(&browser_status),
        "browser_history": schema_of(&browser_history),
        "browser_tabs": schema_of(&browser_tabs),
        "browser_show": schema_of(&browser_show),
        "browser_focus": schema_of(&browser_focus),
        "browser_snapshot": schema_of(&browser_snapshot),
        "browser_screenshot": schema_of(&browser_screenshot),
        "browser_clear": schema_of(&browser_clear),
        "browser_close_all": schema_of(&browser_close_all),
        "browser_stop": schema_of(&browser_stop),
        "memory_index": schema_of(&memory_index),
        "memory_search": schema_of(&memory_search),
        "memory_status": schema_of(&memory_status),
        "memory_status_all": schema_of(&memory_status_all),
        "memory_policy_get": schema_of(&memory_policy_get),
        "memory_policy_set": schema_of(&memory_policy_set),
        "memory_policy_apply": schema_of(&memory_policy_apply),
        "memory_clear": schema_of(&memory_clear),
        "memory_prune": schema_of(&memory_prune),
        "plugins_install": schema_of(&plugins_install),
        "plugins_list": schema_of(&plugins_list),
        "plugins_list_project": schema_of(&plugins_list_project),
        "plugins_check": schema_of(&plugins_check),
        "plugins_disable": schema_of(&plugins_disable),
        "plugins_doctor": schema_of(&plugins_doctor),
        "plugins_enable": schema_of(&plugins_enable),
        "plugins_run": schema_of(&plugins_run),
        "skills_install": schema_of(&skills_install),
        "skills_list": schema_of(&skills_list),
        "skills_list_project": schema_of(&skills_list_project),
        "skills_check": schema_of(&skills_check),
    });
    assert_json_snapshot("snapshots/json_module_features_schema.json", &actual_schema);
}

#[test]
#[allow(deprecated)]
fn json_compat_discovery_maintenance_module_schema_matches_snapshot() {
    let temp = tempdir().expect("tempdir");

    let setup = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args([
                "--project-state",
                "--json",
                "setup",
                "--base-url",
                "mock://mock-model",
                "--model",
                "mock-model",
            ])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_success_envelope(&setup);

    let docs_index = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args(["--json", "docs"])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_success_envelope(&docs_index);

    let docs_topic = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args(["--json", "docs", "gateway"])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_success_envelope(&docs_topic);

    let dns_resolve = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args(["--json", "dns", "resolve", "localhost", "--port", "443"])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_success_envelope(&dns_resolve);

    let tui_prompt = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .env("MOSAIC_MOCK_CHAT_RESPONSE", "compat-tui-ok")
            .args([
                "--project-state",
                "--json",
                "tui",
                "--prompt",
                "compat tui prompt",
            ])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_success_envelope(&tui_prompt);

    let qr_encode_ascii = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args([
                "--json",
                "qr",
                "encode",
                "compat payload",
                "--render",
                "ascii",
            ])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_success_envelope(&qr_encode_ascii);

    let qr_pairing = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args([
                "--project-state",
                "--json",
                "qr",
                "pairing",
                "--device",
                "compat-device",
                "--node",
                "local",
                "--ttl-seconds",
                "300",
            ])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_success_envelope(&qr_pairing);

    let clawbot_send = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .env("MOSAIC_MOCK_CHAT_RESPONSE", "compat-clawbot-ok")
            .args([
                "--project-state",
                "--json",
                "clawbot",
                "send",
                "compat message",
            ])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_success_envelope(&clawbot_send);

    let clawbot_status = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args(["--project-state", "--json", "clawbot", "status"])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_success_envelope(&clawbot_status);

    let directory = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args(["--project-state", "--json", "directory"])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_success_envelope(&directory);

    let directory_diagnostics = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args([
                "--project-state",
                "--json",
                "directory",
                "--ensure",
                "--check-writable",
            ])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_success_envelope(&directory_diagnostics);

    let dashboard = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args(["--project-state", "--json", "dashboard"])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_success_envelope(&dashboard);

    let update_local = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args(["--json", "update"])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_success_envelope(&update_local);

    let update_check = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args(["--json", "update", "--check", "--source", "mock://v9.9.9"])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_success_envelope(&update_check);

    let update_check_same = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args([
                "--json",
                "update",
                "--check",
                "--source",
                concat!("mock://", env!("CARGO_PKG_VERSION")),
            ])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_success_envelope(&update_check_same);

    let reset = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args(["--project-state", "--yes", "--json", "reset"])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_success_envelope(&reset);

    let uninstall = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args(["--project-state", "--yes", "--json", "uninstall"])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_success_envelope(&uninstall);

    let actual_schema = json!({
        "docs_index": schema_of(&docs_index),
        "docs_topic": schema_of(&docs_topic),
        "dns_resolve": schema_of(&dns_resolve),
        "tui_prompt": schema_of(&tui_prompt),
        "qr_encode_ascii": schema_of(&qr_encode_ascii),
        "qr_pairing": schema_of(&qr_pairing),
        "clawbot_send": schema_of(&clawbot_send),
        "clawbot_status": schema_of(&clawbot_status),
        "directory": schema_of(&directory),
        "directory_diagnostics": schema_of(&directory_diagnostics),
        "dashboard": schema_of(&dashboard),
        "update_local": schema_of(&update_local),
        "update_check": schema_of(&update_check),
        "update_check_same": schema_of(&update_check_same),
        "reset": schema_of(&reset),
        "uninstall": schema_of(&uninstall),
    });
    assert_json_snapshot(
        "snapshots/json_module_compat_discovery_maintenance_schema.json",
        &actual_schema,
    );
}

#[test]
#[allow(deprecated)]
fn json_mcp_module_schema_matches_snapshot() {
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
                "contract-mcp",
                "--command",
                &command_path,
                "--arg",
                "--version",
                "--env",
                "MCP_TOKEN=contract",
            ])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_success_envelope(&add);
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
    assert_success_envelope(&list);

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
    assert_success_envelope(&check);

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
    assert_success_envelope(&show);

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
    assert_success_envelope(&check_all);

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
    assert_success_envelope(&disable);

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
    assert_success_envelope(&enable);

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
    assert_success_envelope(&remove);

    let actual_schema = json!({
        "add": schema_of(&add),
        "list": schema_of(&list),
        "check": schema_of(&check),
        "show": schema_of(&show),
        "check_all": schema_of(&check_all),
        "disable": schema_of(&disable),
        "enable": schema_of(&enable),
        "remove": schema_of(&remove),
    });
    assert_json_snapshot("snapshots/json_module_mcp_schema.json", &actual_schema);
}

#[test]
#[allow(deprecated)]
fn json_tts_voicecall_module_schema_matches_snapshot() {
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
    assert_success_envelope(&voices);

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
                "contract tts",
                "--voice",
                "alloy",
                "--format",
                "wav",
            ])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_success_envelope(&speak);

    let start = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args(["--project-state", "--json", "voicecall", "start"])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_success_envelope(&start);

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
                "contract send",
            ])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_success_envelope(&send);

    let history = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args(["--project-state", "--json", "voicecall", "history"])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_success_envelope(&history);

    let status = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args(["--project-state", "--json", "voicecall", "status"])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_success_envelope(&status);

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
    assert_success_envelope(&stop);

    let actual_schema = json!({
        "tts_voices": schema_of(&voices),
        "tts_speak": schema_of(&speak),
        "voicecall_start": schema_of(&start),
        "voicecall_send": schema_of(&send),
        "voicecall_history": schema_of(&history),
        "voicecall_status": schema_of(&status),
        "voicecall_stop": schema_of(&stop),
    });
    assert_json_snapshot(
        "snapshots/json_module_tts_voicecall_schema.json",
        &actual_schema,
    );
}
