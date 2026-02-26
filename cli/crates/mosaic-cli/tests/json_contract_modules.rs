use assert_cmd::Command;
use serde_json::{Value, json};
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

    let actual_schema = json!({
        "add": schema_of(&add),
        "list": schema_of(&list),
        "status": schema_of(&status),
        "capabilities": schema_of(&capabilities),
        "resolve": schema_of(&resolve),
        "test": schema_of(&test_probe),
        "send": schema_of(&send),
        "logs": schema_of(&logs),
    });
    let expected_schema: Value =
        serde_json::from_str(include_str!("snapshots/json_module_channels_schema.json"))
            .expect("expected module channels schema");
    assert_eq!(actual_schema, expected_schema);
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
        "stop": schema_of(&stop),
    });
    let expected_schema: Value =
        serde_json::from_str(include_str!("snapshots/json_module_gateway_schema.json"))
            .expect("expected module gateway schema");
    assert_eq!(actual_schema, expected_schema);
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
            .args([
                "--project-state",
                "--json",
                "security",
                "baseline",
                "show",
            ])
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
    let expected_schema: Value =
        serde_json::from_str(include_str!("snapshots/json_module_security_schema.json"))
            .expect("expected module security schema");
    assert_eq!(actual_schema, expected_schema);
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
