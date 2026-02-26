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
