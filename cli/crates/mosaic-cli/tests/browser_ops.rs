use assert_cmd::Command;
use serde_json::Value;
use std::fs;
use tempfile::{TempDir, tempdir};

#[allow(deprecated)]
fn run_json(temp: &TempDir, args: &[&str]) -> Value {
    let output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args(args)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    serde_json::from_slice(&output).expect("json output")
}

#[test]
#[allow(deprecated)]
fn browser_open_history_show_and_clear_flow() {
    let temp = tempdir().expect("tempdir");

    let open_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "browser",
            "open",
            "--url",
            "mock://ok?title=Mock+Docs",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let open_json: Value = serde_json::from_slice(&open_output).expect("open json");
    assert_eq!(open_json["ok"], true);
    assert_eq!(open_json["visit"]["ok"], true);
    assert_eq!(open_json["visit"]["http_status"], 200);
    assert_eq!(open_json["visit"]["title"], "Mock Docs");
    let visit_id = open_json["visit"]["id"]
        .as_str()
        .expect("visit id")
        .to_string();

    let history_output = Command::cargo_bin("mosaic")
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
        .stdout
        .clone();
    let history_json: Value = serde_json::from_slice(&history_output).expect("history json");
    assert_eq!(history_json["ok"], true);
    assert!(
        history_json["visits"]
            .as_array()
            .expect("visits array")
            .iter()
            .any(|item| item["id"].as_str() == Some(&visit_id))
    );

    let show_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args(["--project-state", "--json", "browser", "show", &visit_id])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let show_json: Value = serde_json::from_slice(&show_output).expect("show json");
    assert_eq!(show_json["ok"], true);
    assert_eq!(show_json["visit"]["id"], visit_id);

    let clear_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args(["--project-state", "--json", "browser", "clear", &visit_id])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let clear_json: Value = serde_json::from_slice(&clear_output).expect("clear json");
    assert_eq!(clear_json["ok"], true);
    assert_eq!(clear_json["removed"], 1);
}

#[test]
#[allow(deprecated)]
fn browser_open_mock_error_status_is_recorded() {
    let temp = tempdir().expect("tempdir");

    let open_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "browser",
            "open",
            "--url",
            "mock://404",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let open_json: Value = serde_json::from_slice(&open_output).expect("open json");
    assert_eq!(open_json["ok"], true);
    assert_eq!(open_json["visit"]["ok"], false);
    assert_eq!(open_json["visit"]["http_status"], 404);
}

#[test]
#[allow(deprecated)]
fn browser_open_invalid_url_returns_validation_error() {
    let temp = tempdir().expect("tempdir");

    let output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "browser",
            "open",
            "--url",
            "not-a-url",
        ])
        .assert()
        .failure()
        .code(7)
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output).expect("error json");
    assert_eq!(json["ok"], false);
    assert_eq!(json["error"]["code"], "validation");
}

#[test]
#[allow(deprecated)]
fn browser_runtime_status_snapshot_and_screenshot_flow() {
    let temp = tempdir().expect("tempdir");

    let start_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args(["--project-state", "--json", "browser", "start"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let start_json: Value = serde_json::from_slice(&start_output).expect("start json");
    assert_eq!(start_json["ok"], true);
    assert_eq!(start_json["state"]["running"], true);

    let navigate_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "browser",
            "navigate",
            "--url",
            "mock://ok?title=Runtime+Browser",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let navigate_json: Value = serde_json::from_slice(&navigate_output).expect("navigate json");
    assert_eq!(navigate_json["ok"], true);
    let visit_id = navigate_json["visit"]["id"]
        .as_str()
        .expect("visit id")
        .to_string();
    assert_eq!(navigate_json["active_visit_id"], visit_id);

    let tabs_output = Command::cargo_bin("mosaic")
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
        .stdout
        .clone();
    let tabs_json: Value = serde_json::from_slice(&tabs_output).expect("tabs json");
    assert_eq!(tabs_json["ok"], true);
    assert_eq!(tabs_json["active_visit_id"], visit_id);
    assert!(
        tabs_json["visits"]
            .as_array()
            .expect("visits")
            .iter()
            .any(|item| item["id"] == visit_id)
    );

    let snapshot_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args(["--project-state", "--json", "browser", "snapshot"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let snapshot_json: Value = serde_json::from_slice(&snapshot_output).expect("snapshot json");
    assert_eq!(snapshot_json["ok"], true);
    assert_eq!(snapshot_json["snapshot"]["visit_id"], visit_id);

    let screenshot_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args(["--project-state", "--json", "browser", "screenshot"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let screenshot_json: Value =
        serde_json::from_slice(&screenshot_output).expect("screenshot json");
    assert_eq!(screenshot_json["ok"], true);
    let output_path = screenshot_json["output"]
        .as_str()
        .expect("screenshot output path");
    let screenshot_content =
        std::fs::read_to_string(output_path).expect("read screenshot artifact");
    assert!(screenshot_content.contains("MOSAIC_BROWSER_SCREENSHOT_V1"));
    assert!(screenshot_content.contains(&visit_id));

    let status_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args(["--project-state", "--json", "browser", "status"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let status_json: Value = serde_json::from_slice(&status_output).expect("status json");
    assert_eq!(status_json["ok"], true);
    assert_eq!(status_json["state"]["running"], true);
    assert_eq!(status_json["active_visit_id"], visit_id);
    assert_eq!(status_json["latest_visit"]["id"], visit_id);

    let focus_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args(["--project-state", "--json", "browser", "focus", &visit_id])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let focus_json: Value = serde_json::from_slice(&focus_output).expect("focus json");
    assert_eq!(focus_json["ok"], true);
    assert_eq!(focus_json["active_visit_id"], visit_id);

    let close_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args(["--project-state", "--json", "browser", "close", &visit_id])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let close_json: Value = serde_json::from_slice(&close_output).expect("close json");
    assert_eq!(close_json["ok"], true);
    assert_eq!(close_json["removed"], 1);

    let stop_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args(["--project-state", "--json", "browser", "stop"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let stop_json: Value = serde_json::from_slice(&stop_output).expect("stop json");
    assert_eq!(stop_json["ok"], true);
    assert_eq!(stop_json["state"]["running"], false);
}

#[test]
#[allow(deprecated)]
fn browser_diagnose_reports_and_repairs_runtime_drift() {
    let temp = tempdir().expect("tempdir");

    let first_visit = run_json(
        &temp,
        &[
            "--project-state",
            "--json",
            "browser",
            "navigate",
            "--url",
            "mock://ok?title=Diagnose+First",
        ],
    );
    let first_visit_id = first_visit["visit"]["id"]
        .as_str()
        .expect("first visit id")
        .to_string();

    let second_visit = run_json(
        &temp,
        &[
            "--project-state",
            "--json",
            "browser",
            "navigate",
            "--url",
            "mock://ok?title=Diagnose+Second",
        ],
    );
    let _second_visit_id = second_visit["visit"]["id"]
        .as_str()
        .expect("second visit id")
        .to_string();

    let failed_visit = run_json(
        &temp,
        &[
            "--project-state",
            "--json",
            "browser",
            "navigate",
            "--url",
            "mock://404",
        ],
    );
    assert_eq!(failed_visit["visit"]["ok"], false);
    let failed_visit_id = failed_visit["visit"]["id"]
        .as_str()
        .expect("failed visit id")
        .to_string();

    run_json(
        &temp,
        &[
            "--project-state",
            "--json",
            "browser",
            "screenshot",
            &first_visit_id,
        ],
    );
    run_json(
        &temp,
        &[
            "--project-state",
            "--json",
            "browser",
            "screenshot",
            &failed_visit_id,
        ],
    );

    let artifacts_dir = temp.path().join(".mosaic/data/browser-screenshots");
    let stale_artifact = artifacts_dir.join(format!("{first_visit_id}.txt"));
    let corrupt_artifact = artifacts_dir.join(format!("{failed_visit_id}.txt"));
    fs::write(&corrupt_artifact, "BROKEN_SCREENSHOT_ARTIFACT").expect("write corrupt artifact");
    let orphan_artifact = artifacts_dir.join("orphan-artifact.txt");
    fs::write(
        &orphan_artifact,
        "MOSAIC_BROWSER_SCREENSHOT_V1\nvisit_id=orphan-artifact\n",
    )
    .expect("write orphan artifact");

    let history_path = temp.path().join(".mosaic/data/browser-history.json");
    let mut history_json: Value =
        serde_json::from_slice(&fs::read(&history_path).expect("read browser history"))
            .expect("parse browser history");
    let history_array = history_json
        .as_array_mut()
        .expect("browser history must be array");
    for item in history_array.iter_mut() {
        if item["id"] == first_visit_id {
            item["ts"] = Value::String("2000-01-01T00:00:00Z".to_string());
        }
    }
    let mut duplicate = history_array
        .iter()
        .find(|item| item["id"] == first_visit_id)
        .expect("first visit entry")
        .clone();
    duplicate["url"] = Value::String("not-a-url".to_string());
    duplicate["ts"] = Value::String("2000-01-01T00:00:00Z".to_string());
    history_array.push(duplicate);
    fs::write(
        &history_path,
        serde_json::to_vec_pretty(&history_json).expect("serialize history"),
    )
    .expect("write browser history drift");

    let state_path = temp.path().join(".mosaic/data/browser-state.json");
    let mut state_json: Value = serde_json::from_slice(&fs::read(&state_path).expect("read state"))
        .expect("parse browser state");
    state_json["running"] = Value::Bool(true);
    state_json["started_at"] = Value::Null;
    state_json["active_visit_id"] = Value::String("missing-visit".to_string());
    fs::write(
        &state_path,
        serde_json::to_vec_pretty(&state_json).expect("serialize state"),
    )
    .expect("write browser state drift");

    let diagnose = run_json(
        &temp,
        &[
            "--project-state",
            "--json",
            "browser",
            "diagnose",
            "--stale-after-minutes",
            "1",
            "--probe-url",
            "mock://ok?title=Probe+Healthy",
            "--probe-url",
            "mock://404",
            "--probe-timeout-ms",
            "1500",
            "--artifact-max-age-hours",
            "1",
        ],
    );
    assert_eq!(diagnose["ok"], true);
    assert_eq!(diagnose["repair"], false);
    assert_eq!(diagnose["summary"]["duplicate_ids"], 1);
    assert_eq!(diagnose["summary"]["invalid_urls"], 1);
    assert_eq!(diagnose["summary"]["network_failures"], 1);
    assert_eq!(
        diagnose["summary"]["network_failure_classes"]["http_4xx"],
        1
    );
    assert_eq!(diagnose["summary"]["probe_count"], 2);
    assert_eq!(diagnose["summary"]["probe_failures"], 1);
    assert_eq!(diagnose["summary"]["probe_failure_classes"]["http_4xx"], 1);
    assert_eq!(diagnose["summary"]["active_visit_missing"], true);
    assert_eq!(diagnose["summary"]["orphan_screenshot_artifacts"], 1);
    assert_eq!(diagnose["summary"]["corrupt_screenshot_artifacts"], 1);
    assert_eq!(diagnose["summary"]["stale_screenshot_artifacts"], 1);
    assert_eq!(diagnose["summary"]["actions_applied"], 0);

    let issue_kinds = diagnose["issues"]
        .as_array()
        .expect("issues array")
        .iter()
        .filter_map(|issue| issue["kind"].as_str())
        .collect::<Vec<_>>();
    assert!(
        issue_kinds.contains(&"duplicate_visit_id"),
        "expected duplicate_visit_id issue: {diagnose}"
    );
    assert!(
        issue_kinds.contains(&"invalid_visit_url"),
        "expected invalid_visit_url issue: {diagnose}"
    );
    assert!(
        issue_kinds.contains(&"active_visit_missing"),
        "expected active_visit_missing issue: {diagnose}"
    );
    assert!(
        issue_kinds.contains(&"running_state_missing_started_at"),
        "expected running_state_missing_started_at issue: {diagnose}"
    );
    assert!(
        issue_kinds.contains(&"network_failures_present"),
        "expected network_failures_present issue: {diagnose}"
    );
    assert!(
        issue_kinds.contains(&"probe_failures_present"),
        "expected probe_failures_present issue: {diagnose}"
    );
    assert!(
        issue_kinds.contains(&"screenshot_orphan_artifact"),
        "expected screenshot_orphan_artifact issue: {diagnose}"
    );
    assert!(
        issue_kinds.contains(&"screenshot_corrupt_artifact"),
        "expected screenshot_corrupt_artifact issue: {diagnose}"
    );
    assert!(
        issue_kinds.contains(&"screenshot_stale_artifact"),
        "expected screenshot_stale_artifact issue: {diagnose}"
    );

    let repair = run_json(
        &temp,
        &[
            "--project-state",
            "--json",
            "browser",
            "diagnose",
            "--stale-after-minutes",
            "1",
            "--probe-url",
            "mock://ok?title=Probe+Healthy",
            "--probe-url",
            "mock://404",
            "--probe-timeout-ms",
            "1500",
            "--artifact-max-age-hours",
            "1",
            "--repair",
        ],
    );
    assert_eq!(repair["ok"], true);
    assert_eq!(repair["repair"], true);
    assert_eq!(repair["summary"]["saved_history"], true);
    assert_eq!(repair["summary"]["saved_state"], true);
    assert_eq!(repair["summary"]["saved_artifacts"], true);
    assert!(
        repair["summary"]["actions_applied"].as_u64().unwrap_or(0) >= 6,
        "expected at least six repair actions: {repair}"
    );

    let repaired_state: Value = serde_json::from_slice(&fs::read(&state_path).expect("read state"))
        .expect("parse repaired state");
    assert_eq!(
        repaired_state["active_visit_id"],
        Value::String(failed_visit_id.clone())
    );
    assert!(
        repaired_state["started_at"].as_str().is_some(),
        "expected started_at to be repaired: {repaired_state}"
    );

    let repaired_history: Value =
        serde_json::from_slice(&fs::read(&history_path).expect("read repaired history"))
            .expect("parse repaired history");
    let repaired_array = repaired_history
        .as_array()
        .expect("repaired history should be array");
    assert_eq!(repaired_array.len(), 3);
    assert!(
        repaired_array.iter().all(|item| item["url"] != "not-a-url"),
        "duplicate malformed url entry should be removed after dedupe: {repaired_history}"
    );
    assert!(
        !stale_artifact.exists(),
        "stale artifact should be removed in repair mode"
    );
    assert!(
        !corrupt_artifact.exists(),
        "corrupt artifact should be removed in repair mode"
    );
    assert!(
        !orphan_artifact.exists(),
        "orphan artifact should be removed in repair mode"
    );
}
