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

fn assert_failure_envelope(payload: &Value) {
    assert_eq!(payload["ok"], false);
    assert!(payload["error"].is_object(), "missing error object: {payload}");
    assert!(payload["error"]["code"].is_string(), "missing error.code");
    assert!(payload["error"]["message"].is_string(), "missing error.message");
    assert!(payload["error"]["exit_code"].is_number(), "missing error.exit_code");
}

#[test]
#[allow(deprecated)]
fn json_success_envelope_schema_matches_snapshot() {
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

    let models = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args(["--project-state", "--json", "models", "list"])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_success_envelope(&models);

    let ask = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .env("MOSAIC_MOCK_CHAT_RESPONSE", "json-contract")
            .args(["--project-state", "--json", "ask", "hello"])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_success_envelope(&ask);

    let channels_list = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args(["--project-state", "--json", "channels", "list"])
            .assert()
            .success()
            .get_output()
            .stdout,
    );
    assert_success_envelope(&channels_list);

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

    let gateway_status = parse_stdout_json(
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
    assert_success_envelope(&gateway_status);

    let actual_schema = json!({
        "setup": schema_of(&setup),
        "models_list": schema_of(&models),
        "ask": schema_of(&ask),
        "channels_list": schema_of(&channels_list),
        "status": schema_of(&status),
        "health": schema_of(&health),
        "doctor": schema_of(&doctor),
        "gateway_status": schema_of(&gateway_status),
    });
    let expected_schema: Value = serde_json::from_str(include_str!("snapshots/json_success_schema.json"))
        .expect("expected json success schema");
    assert_eq!(actual_schema, expected_schema);
}

#[test]
#[allow(deprecated)]
fn json_failure_envelope_schema_matches_snapshot() {
    let temp = tempdir().expect("tempdir");

    let ask_failure = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args(["--project-state", "--json", "ask", "hello"])
            .assert()
            .failure()
            .code(2)
            .get_output()
            .stdout,
    );
    assert_failure_envelope(&ask_failure);

    let approval_failure = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .args([
                "--project-state",
                "--json",
                "nodes",
                "run",
                "local",
                "--command",
                "echo hi",
            ])
            .assert()
            .failure()
            .code(11)
            .get_output()
            .stdout,
    );
    assert_failure_envelope(&approval_failure);

    let gateway_failure = parse_stdout_json(
        &Command::cargo_bin("mosaic")
            .expect("binary")
            .current_dir(temp.path())
            .env("MOSAIC_GATEWAY_TEST_MODE", "1")
            .args(["--project-state", "--json", "gateway", "call", "status"])
            .assert()
            .failure()
            .code(8)
            .get_output()
            .stdout,
    );
    assert_failure_envelope(&gateway_failure);

    let actual_schema = json!({
        "ask_config_error": schema_of(&ask_failure),
        "nodes_approval_error": schema_of(&approval_failure),
        "gateway_unavailable_error": schema_of(&gateway_failure),
    });
    let expected_schema: Value = serde_json::from_str(include_str!("snapshots/json_failure_schema.json"))
        .expect("expected json failure schema");
    assert_eq!(actual_schema, expected_schema);
}
