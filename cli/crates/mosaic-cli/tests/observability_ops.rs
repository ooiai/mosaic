use assert_cmd::Command;
use serde_json::Value;
use std::fs;
use tempfile::tempdir;

#[test]
#[allow(deprecated)]
fn observability_report_json_includes_logs_events_and_doctor_summary() {
    let temp = tempdir().expect("tempdir");

    let audit_dir = temp.path().join(".mosaic/data/audit");
    fs::create_dir_all(&audit_dir).expect("create audit dir");
    fs::write(
        audit_dir.join("commands.jsonl"),
        "{\"id\":\"aud-1\",\"ts\":\"2026-03-01T00:00:00Z\",\"session_id\":\"s-1\",\"command\":\"cargo test --workspace\",\"cwd\":\"/tmp\",\"approved_by\":\"approval_allowlist\",\"exit_code\":0,\"duration_ms\":40}\n\
{\"id\":\"aud-2\",\"ts\":\"2026-03-01T00:01:00Z\",\"session_id\":\"s-1\",\"command\":\"curl https://example.com\",\"cwd\":\"/tmp\",\"approved_by\":\"flag_yes\",\"exit_code\":1,\"duration_ms\":80}\n",
    )
    .expect("write audit log");

    let _ = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "system",
            "event",
            "observability_smoke",
            "--data",
            "{\"suite\":\"observability\"}",
        ])
        .assert()
        .success();

    let output = Command::cargo_bin("mosaic")
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
            "--audit-tail",
            "1",
            "--compare-window",
            "1",
            "--event-name",
            "observability_smoke",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output).expect("observability report json");
    assert_eq!(json["ok"], true);
    assert_eq!(json["report"]["summary"]["doctor_included"], true);
    assert!(
        json["report"]["summary"]["doctor_ok"].as_u64().is_some(),
        "doctor_ok summary must be present"
    );
    assert!(
        json["report"]["system_events"]
            .as_array()
            .expect("system events array")
            .iter()
            .any(|event| event["name"].as_str() == Some("observability_smoke")),
        "report should include filtered system event"
    );
    assert_eq!(
        json["report"]["safety_audit"]["summary"]["total_entries"],
        1
    );
    assert_eq!(
        json["report"]["safety_audit"]["summary"]["blocked_if_restricted"],
        1
    );
    assert_eq!(
        json["report"]["safety_audit"]["comparison"]["enabled"],
        true
    );
    assert_eq!(
        json["report"]["safety_audit"]["comparison"]["available"],
        true
    );
    assert_eq!(json["report"]["summary"]["safety_compare_available"], true);
    assert_eq!(json["report"]["summary"]["safety_failed_delta"], 1);
    assert_eq!(json["report"]["summary"]["gateway_running"], false);
    assert_eq!(json["report"]["summary"]["gateway_endpoint_healthy"], false);
    assert_eq!(json["report"]["summary"]["channels_total"], 0);
    assert_eq!(json["report"]["summary"]["channel_events_count"], 0);
    assert_eq!(json["report"]["summary"]["channel_failed_events"], 0);
    assert_eq!(json["report"]["summary"]["mcp_configured"], 0);
    assert_eq!(json["report"]["summary"]["mcp_healthy"], 0);
    assert_eq!(json["report"]["summary"]["mcp_unhealthy"], 0);
    assert_eq!(json["report"]["summary"]["tts_events_count"], 0);
    assert_eq!(json["report"]["summary"]["tts_total_bytes_written"], 0);
    assert_eq!(json["report"]["summary"]["voicecall_active"], false);
    assert_eq!(json["report"]["summary"]["voicecall_messages_sent"], 0);
    assert_eq!(json["report"]["summary"]["voicecall_events_count"], 0);
    assert_eq!(json["report"]["summary"]["voicecall_delivery_events"], 0);
    assert_eq!(json["report"]["summary"]["voicecall_failed_events"], 0);
    assert_eq!(json["report"]["realtime"]["summary"]["tts_events_count"], 0);
    assert_eq!(
        json["report"]["realtime"]["summary"]["voicecall_events_count"],
        0
    );
    assert_eq!(
        json["report"]["realtime"]["summary"]["voicecall_delivery_events"],
        0
    );
    assert_eq!(
        json["report"]["realtime"]["summary"]["voicecall_failed_events"],
        0
    );
    assert!(
        json["report"]["realtime"]["paths"]["voicecall_state_file"]
            .as_str()
            .is_some()
    );
    assert!(
        json["report"]["realtime"]["paths"]["tts_events_file"]
            .as_str()
            .is_some()
    );
    assert_eq!(json["report"]["mcp"]["summary"]["configured"], 0);
    assert_eq!(json["report"]["mcp"]["summary"]["healthy"], 0);
    assert_eq!(json["report"]["mcp"]["summary"]["unhealthy"], 0);
    assert!(
        json["report"]["summary"]["alerts_total"]
            .as_u64()
            .unwrap_or(0)
            >= 1,
        "gateway down should produce at least one alert"
    );
    assert!(
        json["report"]["summary"]["alerts_warning"]
            .as_u64()
            .is_some()
    );
    assert!(
        json["report"]["summary"]["alerts_critical"]
            .as_u64()
            .is_some()
    );
    assert!(
        json["report"]["alerts"]["items"]
            .as_array()
            .expect("alerts array")
            .iter()
            .any(|item| item["id"].as_str() == Some("gateway.not_running"))
    );
    assert!(
        json["report"]["gateway"]["paths"]["state_file"]
            .as_str()
            .is_some()
    );
    assert!(
        json["report"]["channels"]["paths"]["channels_file"]
            .as_str()
            .is_some()
    );
    assert_eq!(json["report"]["summary"]["plugin_soak_available"], false);
    assert_eq!(json["report"]["summary"]["plugin_soak_status"], "missing");
    assert_eq!(json["report"]["summary"]["plugin_soak_history_count"], 0);
    assert_eq!(
        json["report"]["summary"]["plugin_soak_delta_available"],
        false
    );
    assert_eq!(
        json["report"]["summary"]["plugin_soak_completion_ratio_delta"],
        0.0
    );
}

#[test]
#[allow(deprecated)]
fn observability_report_includes_mcp_unhealthy_alerts() {
    let temp = tempdir().expect("tempdir");

    Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "mcp",
            "add",
            "--name",
            "bad-mcp",
            "--command",
            "__missing_command__",
        ])
        .assert()
        .success();

    let output = Command::cargo_bin("mosaic")
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
            "--audit-tail",
            "20",
            "--compare-window",
            "1",
            "--no-doctor",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output).expect("observability report json");
    assert_eq!(json["ok"], true);
    assert_eq!(json["report"]["summary"]["mcp_configured"], 1);
    assert_eq!(json["report"]["summary"]["mcp_unhealthy"], 1);
    assert!(
        json["report"]["alerts"]["items"]
            .as_array()
            .expect("alerts array")
            .iter()
            .any(|item| item["id"].as_str() == Some("mcp.unhealthy_servers")),
        "expected mcp.unhealthy_servers alert"
    );
}

#[test]
#[allow(deprecated)]
fn observability_report_tracks_mcp_history_and_regression_hints() {
    let temp = tempdir().expect("tempdir");

    let first_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .env("MOSAIC_OBS_MCP_INCIDENT_WINDOW", "2")
        .env("MOSAIC_OBS_MCP_REPEAT_HINT_THRESHOLD", "2")
        .env("MOSAIC_OBS_MCP_RATIO_REGRESSION_WARN", "0.1")
        .env("MOSAIC_OBS_ALERT_MCP_RATIO_DELTA_WARN", "0.1")
        .args([
            "--project-state",
            "--json",
            "observability",
            "report",
            "--tail",
            "20",
            "--event-tail",
            "20",
            "--audit-tail",
            "20",
            "--no-doctor",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let first: Value = serde_json::from_slice(&first_output).expect("first report json");
    assert_eq!(first["ok"], true);
    assert_eq!(first["report"]["summary"]["mcp_history_count"], 1);
    assert_eq!(first["report"]["summary"]["mcp_delta_available"], false);

    Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "mcp",
            "add",
            "--name",
            "bad-history-mcp",
            "--command",
            "__missing_history_command__",
        ])
        .assert()
        .success();

    let second_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .env("MOSAIC_OBS_MCP_INCIDENT_WINDOW", "2")
        .env("MOSAIC_OBS_MCP_REPEAT_HINT_THRESHOLD", "2")
        .env("MOSAIC_OBS_MCP_RATIO_REGRESSION_WARN", "0.1")
        .env("MOSAIC_OBS_ALERT_MCP_RATIO_DELTA_WARN", "0.1")
        .args([
            "--project-state",
            "--json",
            "observability",
            "report",
            "--tail",
            "20",
            "--event-tail",
            "20",
            "--audit-tail",
            "20",
            "--no-doctor",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let second: Value = serde_json::from_slice(&second_output).expect("second report json");
    assert_eq!(second["ok"], true);
    assert_eq!(second["report"]["summary"]["mcp_history_count"], 2);
    assert_eq!(second["report"]["summary"]["mcp_delta_available"], true);
    assert!(
        second["report"]["summary"]["mcp_unhealthy_ratio_delta"]
            .as_f64()
            .unwrap_or(0.0)
            >= 1.0
    );
    assert!(
        second["report"]["mcp"]["history"]["incident_hints"]
            .as_array()
            .expect("mcp incident hints")
            .iter()
            .any(|hint| hint["id"].as_str() == Some("mcp.unhealthy_ratio_regression"))
    );
    assert!(
        second["report"]["alerts"]["items"]
            .as_array()
            .expect("alerts")
            .iter()
            .any(|item| item["id"].as_str() == Some("mcp.unhealthy_ratio_regression"))
    );

    let third_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .env("MOSAIC_OBS_MCP_INCIDENT_WINDOW", "2")
        .env("MOSAIC_OBS_MCP_REPEAT_HINT_THRESHOLD", "2")
        .env("MOSAIC_OBS_MCP_RATIO_REGRESSION_WARN", "0.1")
        .env("MOSAIC_OBS_ALERT_MCP_RATIO_DELTA_WARN", "0.1")
        .args([
            "--project-state",
            "--json",
            "observability",
            "report",
            "--tail",
            "20",
            "--event-tail",
            "20",
            "--audit-tail",
            "20",
            "--no-doctor",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let third: Value = serde_json::from_slice(&third_output).expect("third report json");
    assert_eq!(third["ok"], true);
    assert_eq!(third["report"]["summary"]["mcp_history_count"], 3);
    assert!(
        third["report"]["summary"]["mcp_unhealthy_streak"]
            .as_u64()
            .unwrap_or(0)
            >= 2
    );
    assert!(
        third["report"]["summary"]["mcp_incident_hints"]
            .as_u64()
            .unwrap_or(0)
            >= 1
    );
    assert!(
        third["report"]["mcp"]["history"]["incident_hints"]
            .as_array()
            .expect("mcp incident hints")
            .iter()
            .any(|hint| hint["id"].as_str() == Some("mcp.unhealthy_streak"))
    );

    let history_path = temp
        .path()
        .join(".mosaic/data/reports/observability-mcp-history.jsonl");
    let history_raw = fs::read_to_string(history_path).expect("read mcp history file");
    assert_eq!(
        history_raw
            .lines()
            .filter(|line| !line.trim().is_empty())
            .count(),
        3
    );
}

#[test]
#[allow(deprecated)]
fn observability_report_includes_gateway_failure_telemetry_alerts() {
    let temp = tempdir().expect("tempdir");

    Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .env("MOSAIC_GATEWAY_TEST_MODE", "1")
        .args(["--project-state", "--json", "gateway", "run"])
        .assert()
        .success();

    Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .env("MOSAIC_GATEWAY_TEST_MODE", "1")
        .args(["--project-state", "--json", "gateway", "call", "status"])
        .assert()
        .success();
    Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .env("MOSAIC_GATEWAY_TEST_MODE", "1")
        .args([
            "--project-state",
            "--json",
            "gateway",
            "call",
            "unknown.method",
        ])
        .assert()
        .failure()
        .code(9);

    let output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .env("MOSAIC_OBS_ALERT_GATEWAY_FAILURE_WARN", "0.0")
        .env("MOSAIC_OBS_ALERT_GATEWAY_FAILURE_CRITICAL", "0.0")
        .args([
            "--project-state",
            "--json",
            "observability",
            "report",
            "--tail",
            "20",
            "--event-tail",
            "20",
            "--audit-tail",
            "20",
            "--no-doctor",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output).expect("observability report json");
    assert_eq!(json["ok"], true);
    assert!(
        json["report"]["summary"]["gateway_events_count"]
            .as_u64()
            .unwrap_or(0)
            >= 2
    );
    assert!(
        json["report"]["summary"]["gateway_failed_events"]
            .as_u64()
            .unwrap_or(0)
            >= 1
    );
    assert!(
        json["report"]["gateway"]["telemetry"]["failure_rate"]
            .as_f64()
            .unwrap_or(0.0)
            > 0.0
    );
    assert!(
        json["report"]["gateway"]["recent_events"]
            .as_array()
            .expect("recent gateway events")
            .iter()
            .any(|item| item["success"].as_bool() == Some(false))
    );
    assert!(
        json["report"]["alerts"]["items"]
            .as_array()
            .expect("alerts array")
            .iter()
            .any(|item| item["id"].as_str() == Some("gateway.request_failures")),
        "expected gateway.request_failures alert"
    );
    assert_eq!(
        json["report"]["alerts"]["thresholds"]["gateway_failure_warn"],
        0.0
    );
    assert_eq!(
        json["report"]["alerts"]["thresholds"]["gateway_failure_critical"],
        0.0
    );
}

#[test]
#[allow(deprecated)]
fn observability_report_tracks_gateway_history_and_regression_hints() {
    let temp = tempdir().expect("tempdir");

    Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .env("MOSAIC_GATEWAY_TEST_MODE", "1")
        .args(["--project-state", "--json", "gateway", "run"])
        .assert()
        .success();
    Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .env("MOSAIC_GATEWAY_TEST_MODE", "1")
        .args(["--project-state", "--json", "gateway", "call", "status"])
        .assert()
        .success();

    let first_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .env("MOSAIC_OBS_GATEWAY_INCIDENT_WINDOW", "2")
        .env("MOSAIC_OBS_GATEWAY_REPEAT_HINT_THRESHOLD", "2")
        .env("MOSAIC_OBS_GATEWAY_FAILURE_REGRESSION_WARN", "0.1")
        .env("MOSAIC_OBS_ALERT_GATEWAY_FAILURE_DELTA_WARN", "0.1")
        .args([
            "--project-state",
            "--json",
            "observability",
            "report",
            "--tail",
            "20",
            "--event-tail",
            "20",
            "--audit-tail",
            "20",
            "--no-doctor",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let first: Value = serde_json::from_slice(&first_output).expect("first report json");
    assert_eq!(first["ok"], true);
    assert_eq!(first["report"]["summary"]["gateway_history_count"], 1);
    assert_eq!(first["report"]["summary"]["gateway_delta_available"], false);

    Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .env("MOSAIC_GATEWAY_TEST_MODE", "1")
        .args([
            "--project-state",
            "--json",
            "gateway",
            "call",
            "unknown.method",
        ])
        .assert()
        .failure()
        .code(9);

    let second_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .env("MOSAIC_OBS_GATEWAY_INCIDENT_WINDOW", "2")
        .env("MOSAIC_OBS_GATEWAY_REPEAT_HINT_THRESHOLD", "2")
        .env("MOSAIC_OBS_GATEWAY_FAILURE_REGRESSION_WARN", "0.1")
        .env("MOSAIC_OBS_ALERT_GATEWAY_FAILURE_DELTA_WARN", "0.1")
        .args([
            "--project-state",
            "--json",
            "observability",
            "report",
            "--tail",
            "20",
            "--event-tail",
            "20",
            "--audit-tail",
            "20",
            "--no-doctor",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let second: Value = serde_json::from_slice(&second_output).expect("second report json");
    assert_eq!(second["ok"], true);
    assert_eq!(second["report"]["summary"]["gateway_history_count"], 2);
    assert_eq!(second["report"]["summary"]["gateway_delta_available"], true);
    assert!(
        second["report"]["summary"]["gateway_failure_rate_delta"]
            .as_f64()
            .unwrap_or(0.0)
            >= 0.5
    );
    assert!(
        second["report"]["gateway"]["history"]["incident_hints"]
            .as_array()
            .expect("gateway incident hints")
            .iter()
            .any(|hint| hint["id"].as_str() == Some("gateway.failure_rate_regression"))
    );
    assert!(
        second["report"]["alerts"]["items"]
            .as_array()
            .expect("alerts")
            .iter()
            .any(|item| item["id"].as_str() == Some("gateway.failure_rate_regression"))
    );

    Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .env("MOSAIC_GATEWAY_TEST_MODE", "1")
        .args([
            "--project-state",
            "--json",
            "gateway",
            "call",
            "unknown.method",
        ])
        .assert()
        .failure()
        .code(9);

    let third_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .env("MOSAIC_OBS_GATEWAY_INCIDENT_WINDOW", "2")
        .env("MOSAIC_OBS_GATEWAY_REPEAT_HINT_THRESHOLD", "2")
        .env("MOSAIC_OBS_GATEWAY_FAILURE_REGRESSION_WARN", "0.1")
        .env("MOSAIC_OBS_ALERT_GATEWAY_FAILURE_DELTA_WARN", "0.1")
        .env("MOSAIC_OBS_ALERT_GATEWAY_FAILURE_WARN", "0.1")
        .args([
            "--project-state",
            "--json",
            "observability",
            "report",
            "--tail",
            "20",
            "--event-tail",
            "20",
            "--audit-tail",
            "20",
            "--no-doctor",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let third: Value = serde_json::from_slice(&third_output).expect("third report json");
    assert_eq!(third["ok"], true);
    assert_eq!(third["report"]["summary"]["gateway_history_count"], 3);
    assert!(
        third["report"]["summary"]["gateway_incident_hints"]
            .as_u64()
            .unwrap_or(0)
            >= 1
    );
    assert!(
        third["report"]["gateway"]["history"]["incident_hints"]
            .as_array()
            .expect("gateway incident hints")
            .iter()
            .any(|hint| hint["id"].as_str() == Some("gateway.failure_repeated"))
    );

    let history_path = temp
        .path()
        .join(".mosaic/data/reports/observability-gateway-history.jsonl");
    let history_raw = fs::read_to_string(history_path).expect("read gateway history file");
    assert_eq!(
        history_raw
            .lines()
            .filter(|line| !line.trim().is_empty())
            .count(),
        3
    );
}

#[test]
#[allow(deprecated)]
fn observability_report_includes_voicecall_telemetry_and_failure_alerts() {
    let temp = tempdir().expect("tempdir");

    let add_channel = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "channels",
            "add",
            "--name",
            "voice-obs-terminal",
            "--kind",
            "terminal",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let add_channel_json: Value = serde_json::from_slice(&add_channel).expect("channels add json");
    let channel_id = add_channel_json["channel"]["id"]
        .as_str()
        .expect("channel id")
        .to_string();

    Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "voicecall",
            "start",
            "--target",
            "obs-room",
            "--channel-id",
            &channel_id,
        ])
        .assert()
        .success();
    Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "voicecall",
            "send",
            "--text",
            "voicecall observability success",
        ])
        .assert()
        .success();
    Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args(["--project-state", "--json", "voicecall", "stop"])
        .assert()
        .success();

    Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "voicecall",
            "start",
            "--target",
            "obs-room-fail",
            "--channel-id",
            "ch_missing_voicecall_target",
        ])
        .assert()
        .success();
    Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "voicecall",
            "send",
            "--text",
            "voicecall observability failure",
        ])
        .assert()
        .failure()
        .code(2);

    let output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .env("MOSAIC_OBS_ALERT_VOICECALL_FAILURE_WARN", "0.0")
        .env("MOSAIC_OBS_ALERT_VOICECALL_FAILURE_CRITICAL", "0.0")
        .args([
            "--project-state",
            "--json",
            "observability",
            "report",
            "--tail",
            "20",
            "--event-tail",
            "20",
            "--audit-tail",
            "20",
            "--no-doctor",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output).expect("observability report json");
    assert_eq!(json["ok"], true);
    assert!(
        json["report"]["summary"]["voicecall_events_count"]
            .as_u64()
            .unwrap_or(0)
            >= 2
    );
    assert!(
        json["report"]["summary"]["voicecall_delivery_events"]
            .as_u64()
            .unwrap_or(0)
            >= 2
    );
    assert!(
        json["report"]["summary"]["voicecall_failed_events"]
            .as_u64()
            .unwrap_or(0)
            >= 1
    );
    assert_eq!(json["report"]["summary"]["voicecall_active"], true);
    assert!(
        json["report"]["realtime"]["recent_voicecall_events"]
            .as_array()
            .expect("recent voicecall events")
            .iter()
            .any(|item| item["direction"].as_str() == Some("error"))
    );
    assert!(
        json["report"]["alerts"]["items"]
            .as_array()
            .expect("alerts array")
            .iter()
            .any(|item| item["id"].as_str() == Some("voicecall.delivery_failures")),
        "expected voicecall.delivery_failures alert"
    );
    assert_eq!(
        json["report"]["alerts"]["thresholds"]["voicecall_failure_warn"],
        0.0
    );
    assert_eq!(
        json["report"]["alerts"]["thresholds"]["voicecall_failure_critical"],
        0.0
    );
}

#[test]
#[allow(deprecated)]
fn observability_export_writes_json_file() {
    let temp = tempdir().expect("tempdir");
    let output_path = temp.path().join("observability-report.json");

    let output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "observability",
            "export",
            "--out",
            output_path.to_str().expect("utf8 path"),
            "--tail",
            "10",
            "--event-tail",
            "10",
            "--audit-tail",
            "10",
            "--no-doctor",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output).expect("observability export json");
    assert_eq!(json["ok"], true);
    assert_eq!(json["report"]["summary"]["doctor_included"], false);
    assert_eq!(
        json["export_path"],
        output_path.display().to_string(),
        "response should return export path"
    );

    let report_raw = std::fs::read_to_string(&output_path).expect("read exported report");
    let report: Value = serde_json::from_str(&report_raw).expect("parse exported report");
    assert!(
        report["generated_at"].as_str().is_some(),
        "exported report should include generated timestamp"
    );
    assert!(
        report["summary"]["logs_count"].as_u64().is_some(),
        "exported report should include logs summary count"
    );
    assert_eq!(report["filters"]["compare_window"], 0);
    assert!(report["summary"]["gateway_running"].as_bool().is_some());
    assert!(
        report["summary"]["gateway_endpoint_healthy"]
            .as_bool()
            .is_some()
    );
    assert!(report["summary"]["channels_total"].as_u64().is_some());
    assert!(report["summary"]["channel_events_count"].as_u64().is_some());
    assert!(
        report["summary"]["channel_failed_events"]
            .as_u64()
            .is_some()
    );
    assert!(report["summary"]["alerts_total"].as_u64().is_some());
    assert!(report["summary"]["alerts_warning"].as_u64().is_some());
    assert!(report["summary"]["alerts_critical"].as_u64().is_some());
    assert_eq!(report["summary"]["plugin_soak_available"], false);
    assert_eq!(report["summary"]["plugin_soak_status"], "missing");
    assert_eq!(report["summary"]["plugin_soak_history_count"], 0);
    assert_eq!(report["summary"]["plugin_soak_delta_available"], false);
    assert_eq!(report["summary"]["plugin_soak_completion_ratio_delta"], 0.0);
    assert!(
        report["safety_audit"]["summary"]["total_entries"]
            .as_u64()
            .is_some(),
        "exported report should include safety audit summary"
    );
}

#[test]
#[allow(deprecated)]
fn observability_report_includes_channel_runtime_telemetry() {
    let temp = tempdir().expect("tempdir");

    let add_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "channels",
            "add",
            "--name",
            "obs-slack",
            "--kind",
            "slack_webhook",
            "--endpoint",
            "mock-http://200",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let add_json: Value = serde_json::from_slice(&add_output).expect("channels add json");
    let channel_id = add_json["channel"]["id"]
        .as_str()
        .expect("channel id")
        .to_string();

    let _ = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "channels",
            "send",
            &channel_id,
            "--text",
            "observability channels telemetry",
        ])
        .assert()
        .success();

    let output = Command::cargo_bin("mosaic")
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
            "--audit-tail",
            "20",
            "--no-doctor",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output).expect("observability report json");
    assert_eq!(json["ok"], true);
    assert_eq!(json["report"]["summary"]["channels_total"], 1);
    assert!(
        json["report"]["summary"]["channel_events_count"]
            .as_u64()
            .unwrap_or(0)
            >= 1
    );
    assert_eq!(json["report"]["summary"]["channel_failed_events"], 0);
    assert_eq!(
        json["report"]["channels"]["summary"]["delivery_status"]["success"],
        1
    );
    assert!(
        json["report"]["summary"]["alerts_total"]
            .as_u64()
            .unwrap_or(0)
            >= 1,
        "gateway not running should still contribute warning alert"
    );
    assert!(
        json["report"]["channels"]["recent_events"]
            .as_array()
            .map_or(0, |items| items.len())
            >= 1
    );
}

#[test]
#[allow(deprecated)]
fn observability_report_supports_alert_suppression_controls() {
    let temp = tempdir().expect("tempdir");

    let output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .env("MOSAIC_OBS_ALERT_SUPPRESS_IDS", "gateway.not_running")
        .args([
            "--project-state",
            "--json",
            "observability",
            "report",
            "--tail",
            "10",
            "--event-tail",
            "10",
            "--audit-tail",
            "10",
            "--no-doctor",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output).expect("observability report json");
    assert_eq!(json["ok"], true);
    assert_eq!(
        json["report"]["alerts"]["suppress_ids"][0],
        "gateway.not_running"
    );
    assert!(
        json["report"]["alerts"]["items"]
            .as_array()
            .expect("visible alerts")
            .iter()
            .all(|item| item["id"].as_str() != Some("gateway.not_running"))
    );
    assert!(
        json["report"]["alerts"]["suppressed"]["count"]
            .as_u64()
            .unwrap_or(0)
            >= 1
    );
    assert!(
        json["report"]["alerts"]["suppressed"]["items"]
            .as_array()
            .expect("suppressed alerts")
            .iter()
            .any(|item| {
                item["id"].as_str() == Some("gateway.not_running")
                    && item["suppressed_reason"].as_str() == Some("id_match")
            })
    );
    assert!(
        json["report"]["summary"]["alerts_suppressed"]
            .as_u64()
            .unwrap_or(0)
            >= 1
    );
}

#[test]
#[allow(deprecated)]
fn observability_report_exposes_slo_windows_and_targets() {
    let temp = tempdir().expect("tempdir");

    let add_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "channels",
            "add",
            "--name",
            "slo-terminal",
            "--kind",
            "terminal",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let add_json: Value = serde_json::from_slice(&add_output).expect("channels add json");
    let channel_id = add_json["channel"]["id"]
        .as_str()
        .expect("channel id")
        .to_string();

    let _ = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "channels",
            "send",
            &channel_id,
            "--text",
            "slo check",
        ])
        .assert()
        .success();

    let output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .env("MOSAIC_OBS_SLO_WINDOW", "5")
        .env("MOSAIC_OBS_SLO_GATEWAY_TARGET", "0.4")
        .env("MOSAIC_OBS_SLO_CHANNELS_TARGET", "0.8")
        .args([
            "--project-state",
            "--json",
            "observability",
            "report",
            "--tail",
            "20",
            "--event-tail",
            "20",
            "--audit-tail",
            "20",
            "--no-doctor",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output).expect("observability report json");
    assert_eq!(json["ok"], true);
    assert_eq!(json["report"]["slo"]["window"], 5);
    assert_eq!(json["report"]["slo"]["gateway"]["target"], 0.4);
    assert_eq!(json["report"]["slo"]["channels"]["target"], 0.8);
    assert_eq!(json["report"]["summary"]["slo_gateway_met"], false);
    assert_eq!(json["report"]["summary"]["slo_channels_met"], true);
}

#[test]
#[allow(deprecated)]
fn observability_report_tracks_slo_history_and_incident_hints() {
    let temp = tempdir().expect("tempdir");

    let first_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .env("MOSAIC_OBS_SLO_INCIDENT_WINDOW", "2")
        .env("MOSAIC_OBS_ALERT_REPEAT_HINT_THRESHOLD", "2")
        .args([
            "--project-state",
            "--json",
            "observability",
            "report",
            "--tail",
            "10",
            "--event-tail",
            "10",
            "--audit-tail",
            "10",
            "--no-doctor",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let first: Value = serde_json::from_slice(&first_output).expect("first report json");
    assert_eq!(first["ok"], true);
    assert_eq!(first["report"]["summary"]["slo_history_count"], 1);

    let second_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .env("MOSAIC_OBS_SLO_INCIDENT_WINDOW", "2")
        .env("MOSAIC_OBS_ALERT_REPEAT_HINT_THRESHOLD", "2")
        .args([
            "--project-state",
            "--json",
            "observability",
            "report",
            "--tail",
            "10",
            "--event-tail",
            "10",
            "--audit-tail",
            "10",
            "--no-doctor",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let second: Value = serde_json::from_slice(&second_output).expect("second report json");
    assert_eq!(second["ok"], true);
    assert_eq!(second["report"]["summary"]["slo_history_count"], 2);
    assert_eq!(
        second["report"]["slo"]["history"]["current_vs_previous"]["available"],
        true
    );
    assert!(
        second["report"]["summary"]["slo_gateway_unmet_streak"]
            .as_u64()
            .unwrap_or(0)
            >= 2
    );
    assert!(
        second["report"]["summary"]["slo_incident_hints"]
            .as_u64()
            .unwrap_or(0)
            >= 1
    );
    assert!(
        second["report"]["slo"]["history"]["incident_hints"]
            .as_array()
            .expect("incident hints")
            .iter()
            .any(|hint| hint["id"].as_str() == Some("slo.gateway_unmet_streak"))
    );
    assert!(
        second["report"]["slo"]["history"]["incident_hints"]
            .as_array()
            .expect("incident hints")
            .iter()
            .any(|hint| {
                hint["id"].as_str() == Some("alerts.repeated")
                    && hint["alert_id"].as_str() == Some("gateway.not_running")
            })
    );
}

#[test]
#[allow(deprecated)]
fn observability_report_alert_thresholds_can_be_overridden_by_env() {
    let temp = tempdir().expect("tempdir");

    let output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .env("MOSAIC_OBS_ALERT_CHANNEL_FAILURE_WARN", "0.2")
        .env("MOSAIC_OBS_ALERT_CHANNEL_FAILURE_CRITICAL", "0.6")
        .env("MOSAIC_OBS_ALERT_SAFETY_FAILURE_WARN", "0.3")
        .env("MOSAIC_OBS_ALERT_SAFETY_FAILURE_CRITICAL", "0.7")
        .env("MOSAIC_OBS_ALERT_PLUGIN_COMPLETION_MIN", "0.9")
        .env("MOSAIC_OBS_ALERT_VOICECALL_FAILURE_WARN", "0.15")
        .env("MOSAIC_OBS_ALERT_VOICECALL_FAILURE_CRITICAL", "0.55")
        .args([
            "--project-state",
            "--json",
            "observability",
            "report",
            "--tail",
            "10",
            "--event-tail",
            "10",
            "--audit-tail",
            "10",
            "--no-doctor",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output).expect("observability report json");
    assert_eq!(json["ok"], true);
    assert_eq!(
        json["report"]["alerts"]["thresholds"]["channel_failure_warn"],
        0.2
    );
    assert_eq!(
        json["report"]["alerts"]["thresholds"]["channel_failure_critical"],
        0.6
    );
    assert_eq!(
        json["report"]["alerts"]["thresholds"]["safety_failure_warn"],
        0.3
    );
    assert_eq!(
        json["report"]["alerts"]["thresholds"]["safety_failure_critical"],
        0.7
    );
    assert_eq!(
        json["report"]["alerts"]["thresholds"]["plugin_completion_min"],
        0.9
    );
    assert_eq!(
        json["report"]["alerts"]["thresholds"]["voicecall_failure_warn"],
        0.15
    );
    assert_eq!(
        json["report"]["alerts"]["thresholds"]["voicecall_failure_critical"],
        0.55
    );
}

#[test]
#[allow(deprecated)]
fn observability_report_parses_plugin_soak_metrics() {
    let temp = tempdir().expect("tempdir");
    let soak_path = temp.path().join("plugin-soak-latest.log");
    fs::write(
        &soak_path,
        "[soak] completed\n\
iterations=8\n\
ok_runs=8 cpu_failures=8 rss_failures=8\n\
event_lines.ok=8\n\
event_lines.cpuwatch=8\n\
event_lines.rss=8\n\
workspace=/tmp/mosaic-soak\n",
    )
    .expect("write soak log");

    let output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "observability",
            "report",
            "--tail",
            "10",
            "--event-tail",
            "10",
            "--audit-tail",
            "10",
            "--no-doctor",
            "--plugin-soak-report",
            soak_path.to_str().expect("soak path"),
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output).expect("observability report json");
    assert_eq!(json["ok"], true);
    assert_eq!(json["report"]["summary"]["plugin_soak_available"], true);
    assert_eq!(json["report"]["summary"]["plugin_soak_status"], "ok");
    assert_eq!(json["report"]["plugin_soak"]["available"], true);
    assert_eq!(json["report"]["plugin_soak"]["summary"]["iterations"], 8);
    assert_eq!(
        json["report"]["plugin_soak"]["trend"]["completion_ratio"],
        1.0
    );
    assert_eq!(
        json["report"]["plugin_soak"]["trend"]["event_line_drift"]["ok"],
        0
    );
    assert_eq!(json["report"]["summary"]["plugin_soak_history_count"], 1);
    assert_eq!(
        json["report"]["summary"]["plugin_soak_delta_available"],
        false
    );
    assert_eq!(
        json["report"]["plugin_soak"]["history"]["current_vs_previous"]["available"],
        false
    );
}

#[test]
#[allow(deprecated)]
fn observability_report_records_plugin_soak_history_and_delta() {
    let temp = tempdir().expect("tempdir");
    let soak_first = temp.path().join("plugin-soak-first.log");
    let soak_second = temp.path().join("plugin-soak-second.log");

    fs::write(
        &soak_first,
        "[soak] completed\n\
iterations=5\n\
ok_runs=5 cpu_failures=5 rss_failures=5\n\
event_lines.ok=5\n\
event_lines.cpuwatch=5\n\
event_lines.rss=5\n\
workspace=/tmp/mosaic-soak\n",
    )
    .expect("write first soak log");
    fs::write(
        &soak_second,
        "[soak] completed\n\
iterations=8\n\
ok_runs=8 cpu_failures=8 rss_failures=8\n\
event_lines.ok=8\n\
event_lines.cpuwatch=8\n\
event_lines.rss=8\n\
workspace=/tmp/mosaic-soak\n",
    )
    .expect("write second soak log");

    let first_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "observability",
            "report",
            "--tail",
            "10",
            "--event-tail",
            "10",
            "--audit-tail",
            "10",
            "--no-doctor",
            "--plugin-soak-report",
            soak_first.to_str().expect("first soak path"),
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let first: Value = serde_json::from_slice(&first_output).expect("first report json");
    assert_eq!(first["ok"], true);
    assert_eq!(first["report"]["summary"]["plugin_soak_history_count"], 1);
    assert_eq!(
        first["report"]["summary"]["plugin_soak_delta_available"],
        false
    );

    let second_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "observability",
            "report",
            "--tail",
            "10",
            "--event-tail",
            "10",
            "--audit-tail",
            "10",
            "--no-doctor",
            "--plugin-soak-report",
            soak_second.to_str().expect("second soak path"),
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let second: Value = serde_json::from_slice(&second_output).expect("second report json");
    assert_eq!(second["ok"], true);
    assert_eq!(second["report"]["summary"]["plugin_soak_history_count"], 2);
    assert_eq!(
        second["report"]["summary"]["plugin_soak_delta_available"],
        true
    );
    assert_eq!(
        second["report"]["plugin_soak"]["history"]["current_vs_previous"]["delta"]["iterations"],
        3
    );
    assert_eq!(
        second["report"]["summary"]["plugin_soak_completion_ratio_delta"],
        0.0
    );

    let history_path = temp
        .path()
        .join(".mosaic/data/reports/plugin-soak-history.jsonl");
    let history_raw = fs::read_to_string(history_path).expect("read history file");
    assert_eq!(
        history_raw
            .lines()
            .filter(|line| !line.trim().is_empty())
            .count(),
        2
    );
}

#[test]
#[allow(deprecated)]
fn observability_report_enforces_plugin_soak_history_retention() {
    let temp = tempdir().expect("tempdir");
    let soak_first = temp.path().join("plugin-soak-retention-first.log");
    let soak_second = temp.path().join("plugin-soak-retention-second.log");

    fs::write(
        &soak_first,
        "[soak] completed\n\
iterations=3\n\
ok_runs=3 cpu_failures=3 rss_failures=3\n\
event_lines.ok=3\n\
event_lines.cpuwatch=3\n\
event_lines.rss=3\n\
workspace=/tmp/mosaic-soak\n",
    )
    .expect("write first soak log");
    fs::write(
        &soak_second,
        "[soak] completed\n\
iterations=6\n\
ok_runs=6 cpu_failures=6 rss_failures=6\n\
event_lines.ok=6\n\
event_lines.cpuwatch=6\n\
event_lines.rss=6\n\
workspace=/tmp/mosaic-soak\n",
    )
    .expect("write second soak log");

    let first_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .env("MOSAIC_OBS_PLUGIN_SOAK_HISTORY_MAX_SAMPLES", "1")
        .args([
            "--project-state",
            "--json",
            "observability",
            "report",
            "--tail",
            "10",
            "--event-tail",
            "10",
            "--audit-tail",
            "10",
            "--no-doctor",
            "--plugin-soak-report",
            soak_first.to_str().expect("first soak path"),
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let first: Value = serde_json::from_slice(&first_output).expect("first report json");
    assert_eq!(first["ok"], true);
    assert_eq!(first["report"]["summary"]["plugin_soak_history_count"], 1);
    assert_eq!(first["report"]["summary"]["plugin_soak_history_pruned"], 0);

    let second_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .env("MOSAIC_OBS_PLUGIN_SOAK_HISTORY_MAX_SAMPLES", "1")
        .args([
            "--project-state",
            "--json",
            "observability",
            "report",
            "--tail",
            "10",
            "--event-tail",
            "10",
            "--audit-tail",
            "10",
            "--no-doctor",
            "--plugin-soak-report",
            soak_second.to_str().expect("second soak path"),
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let second: Value = serde_json::from_slice(&second_output).expect("second report json");
    assert_eq!(second["ok"], true);
    assert_eq!(second["report"]["summary"]["plugin_soak_history_count"], 1);
    assert_eq!(second["report"]["summary"]["plugin_soak_history_pruned"], 1);
    assert_eq!(
        second["report"]["plugin_soak"]["history"]["retention"]["max_samples"],
        1
    );
    assert_eq!(
        second["report"]["plugin_soak"]["history"]["retention"]["pruned"],
        1
    );
    assert_eq!(
        second["report"]["plugin_soak"]["history"]["current_vs_previous"]["available"],
        true
    );

    let history_path = temp
        .path()
        .join(".mosaic/data/reports/plugin-soak-history.jsonl");
    let history_raw = fs::read_to_string(history_path).expect("read history file");
    assert_eq!(
        history_raw
            .lines()
            .filter(|line| !line.trim().is_empty())
            .count(),
        1
    );
}

#[test]
#[allow(deprecated)]
fn observability_report_detects_plugin_soak_repeated_anomalies() {
    let temp = tempdir().expect("tempdir");
    let soak_first = temp.path().join("plugin-soak-anomaly-first.log");
    let soak_second = temp.path().join("plugin-soak-anomaly-second.log");

    fs::write(
        &soak_first,
        "[soak] completed\n\
iterations=6\n\
ok_runs=6 cpu_failures=6 rss_failures=5\n\
event_lines.ok=6\n\
event_lines.cpuwatch=6\n\
event_lines.rss=3\n\
workspace=/tmp/mosaic-soak\n",
    )
    .expect("write first anomaly soak log");
    fs::write(
        &soak_second,
        "[soak] completed\n\
iterations=6\n\
ok_runs=6 cpu_failures=6 rss_failures=4\n\
event_lines.ok=6\n\
event_lines.cpuwatch=6\n\
event_lines.rss=1\n\
workspace=/tmp/mosaic-soak\n",
    )
    .expect("write second anomaly soak log");

    let _ = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .env("MOSAIC_OBS_PLUGIN_SOAK_INCIDENT_WINDOW", "2")
        .env("MOSAIC_OBS_PLUGIN_SOAK_REPEAT_HINT_THRESHOLD", "2")
        .env("MOSAIC_OBS_PLUGIN_SOAK_DRIFT_ABS_THRESHOLD", "1")
        .env("MOSAIC_OBS_PLUGIN_SOAK_COMPLETION_DROP_WARN", "0.01")
        .args([
            "--project-state",
            "--json",
            "observability",
            "report",
            "--tail",
            "10",
            "--event-tail",
            "10",
            "--audit-tail",
            "10",
            "--no-doctor",
            "--plugin-soak-report",
            soak_first.to_str().expect("first soak path"),
        ])
        .assert()
        .success();

    let second_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .env("MOSAIC_OBS_PLUGIN_SOAK_INCIDENT_WINDOW", "2")
        .env("MOSAIC_OBS_PLUGIN_SOAK_REPEAT_HINT_THRESHOLD", "2")
        .env("MOSAIC_OBS_PLUGIN_SOAK_DRIFT_ABS_THRESHOLD", "1")
        .env("MOSAIC_OBS_PLUGIN_SOAK_COMPLETION_DROP_WARN", "0.01")
        .args([
            "--project-state",
            "--json",
            "observability",
            "report",
            "--tail",
            "10",
            "--event-tail",
            "10",
            "--audit-tail",
            "10",
            "--no-doctor",
            "--plugin-soak-report",
            soak_second.to_str().expect("second soak path"),
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let second: Value = serde_json::from_slice(&second_output).expect("second report json");
    assert_eq!(second["ok"], true);
    assert!(
        second["report"]["summary"]["plugin_soak_incident_hints"]
            .as_u64()
            .unwrap_or(0)
            >= 3
    );
    assert!(
        second["report"]["summary"]["plugin_soak_completion_unmet_streak"]
            .as_u64()
            .unwrap_or(0)
            >= 2
    );
    assert!(
        second["report"]["summary"]["plugin_soak_status_unmet_streak"]
            .as_u64()
            .unwrap_or(0)
            >= 2
    );
    assert!(
        second["report"]["plugin_soak"]["history"]["window"]["recent_drift_count"]
            .as_u64()
            .unwrap_or(0)
            >= 2
    );

    let hints = second["report"]["plugin_soak"]["history"]["incident_hints"]
        .as_array()
        .expect("incident hints");
    assert!(
        hints
            .iter()
            .any(|hint| hint["id"].as_str() == Some("plugin_soak.completion_unmet_streak"))
    );
    assert!(
        hints
            .iter()
            .any(|hint| hint["id"].as_str() == Some("plugin_soak.drift_repeated"))
    );
    assert!(
        hints
            .iter()
            .any(|hint| hint["id"].as_str() == Some("plugin_soak.warnings_repeated"))
    );
    assert!(
        hints
            .iter()
            .any(|hint| hint["id"].as_str() == Some("plugin_soak.completion_regression"))
    );
}
