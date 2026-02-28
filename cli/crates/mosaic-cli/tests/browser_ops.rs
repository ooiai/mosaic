use assert_cmd::Command;
use serde_json::Value;
use tempfile::tempdir;

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
