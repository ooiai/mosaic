use assert_cmd::Command;
use serde_json::Value;
use tempfile::tempdir;

#[test]
#[allow(deprecated)]
fn security_audit_detects_findings() {
    let temp = tempdir().expect("tempdir");
    std::fs::write(
        temp.path().join("secrets.env"),
        "API_KEY = \"sk-live-secret-value-123456\"\n",
    )
    .expect("write secrets");
    std::fs::write(
        temp.path().join("install.sh"),
        "curl https://example.com/install.sh | sh\n",
    )
    .expect("write script");

    let output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "security",
            "audit",
            "--path",
            ".",
            "--deep",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json: Value = serde_json::from_slice(&output).expect("json output");
    assert_eq!(json["ok"], true);
    assert!(json["report"]["summary"]["findings"].as_u64().unwrap_or(0) >= 1);
    assert!(json["report"]["findings"].is_array());
}

#[test]
#[allow(deprecated)]
fn security_audit_missing_path_returns_validation_error() {
    let temp = tempdir().expect("tempdir");
    let output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "security",
            "audit",
            "--path",
            "./not-found",
        ])
        .assert()
        .failure()
        .code(7)
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output).expect("json error");
    assert_eq!(json["ok"], false);
    assert_eq!(json["error"]["code"], "validation");
}

#[test]
#[allow(deprecated)]
fn security_audit_can_update_and_apply_baseline() {
    let temp = tempdir().expect("tempdir");
    std::fs::write(
        temp.path().join("secrets.env"),
        "API_KEY = \"sk-live-secret-value-123456\"\n",
    )
    .expect("write secrets");

    let baseline_update = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "security",
            "audit",
            "--path",
            ".",
            "--update-baseline",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let baseline_update: Value =
        serde_json::from_slice(&baseline_update).expect("baseline update json");
    assert_eq!(baseline_update["ok"], true);
    assert_eq!(baseline_update["baseline"]["updated"], true);
    assert!(baseline_update["baseline"]["added"].as_u64().unwrap_or(0) >= 1);
    assert_eq!(baseline_update["report"]["summary"]["findings"], 0);

    let audit_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "security",
            "audit",
            "--path",
            ".",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let audit_json: Value = serde_json::from_slice(&audit_output).expect("audit json");
    assert_eq!(audit_json["ok"], true);
    assert_eq!(audit_json["report"]["summary"]["findings"], 0);
    assert!(
        audit_json["report"]["summary"]["ignored"]
            .as_u64()
            .unwrap_or(0)
            >= 1
    );
}

#[test]
#[allow(deprecated)]
fn security_baseline_manage_commands_flow() {
    let temp = tempdir().expect("tempdir");

    let show_initial = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args(["--project-state", "--json", "security", "baseline", "show"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let show_initial: Value = serde_json::from_slice(&show_initial).expect("show initial");
    assert_eq!(show_initial["ok"], true);
    assert_eq!(show_initial["exists"], false);

    let add_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "security",
            "baseline",
            "add",
            "--fingerprint",
            "foo:1:category:title",
            "--category",
            "transport_security",
            "--match-path",
            "vendor/*",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let add_output: Value = serde_json::from_slice(&add_output).expect("add output");
    assert_eq!(add_output["ok"], true);
    assert!(add_output["added"].as_u64().unwrap_or(0) >= 3);

    let show_after_add = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args(["--project-state", "--json", "security", "baseline", "show"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let show_after_add: Value = serde_json::from_slice(&show_after_add).expect("show after add");
    assert_eq!(show_after_add["ok"], true);
    assert_eq!(show_after_add["exists"], true);
    assert!(
        show_after_add["baseline"]["ignored_fingerprints"]
            .as_array()
            .expect("fingerprints")
            .iter()
            .any(|value| value.as_str() == Some("foo:1:category:title"))
    );

    let remove_output = Command::cargo_bin("mosaic")
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
        .stdout
        .clone();
    let remove_output: Value = serde_json::from_slice(&remove_output).expect("remove output");
    assert_eq!(remove_output["ok"], true);
    assert!(remove_output["removed"].as_u64().unwrap_or(0) >= 1);

    let clear_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args(["--project-state", "--json", "security", "baseline", "clear"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let clear_output: Value = serde_json::from_slice(&clear_output).expect("clear output");
    assert_eq!(clear_output["ok"], true);
    assert!(clear_output["cleared"].as_u64().unwrap_or(0) >= 1);

    let show_after_clear = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args(["--project-state", "--json", "security", "baseline", "show"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let show_after_clear: Value =
        serde_json::from_slice(&show_after_clear).expect("show after clear");
    assert_eq!(show_after_clear["ok"], true);
    assert_eq!(
        show_after_clear["baseline"]["ignored_fingerprints"]
            .as_array()
            .expect("fingerprints")
            .len(),
        0
    );
}

#[test]
#[allow(deprecated)]
fn security_audit_sarif_output_flow() {
    let temp = tempdir().expect("tempdir");
    std::fs::write(
        temp.path().join("secrets.env"),
        "API_KEY = \"sk-live-secret-value-123456\"\n",
    )
    .expect("write secrets");

    let sarif_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "security",
            "audit",
            "--path",
            ".",
            "--sarif",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let sarif_output: Value = serde_json::from_slice(&sarif_output).expect("sarif json");
    assert_eq!(sarif_output["version"], "2.1.0");
    assert!(sarif_output["runs"][0]["results"].is_array());

    let file_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "security",
            "audit",
            "--path",
            ".",
            "--sarif-output",
            "scan.sarif",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let file_output: Value = serde_json::from_slice(&file_output).expect("file output json");
    assert_eq!(file_output["ok"], true);
    let reported_path = file_output["sarif_output"]
        .as_str()
        .expect("sarif output path");
    let reported_canonical =
        std::fs::canonicalize(reported_path).expect("canonicalize reported path");
    let expected_canonical =
        std::fs::canonicalize(temp.path().join("scan.sarif")).expect("canonicalize expected path");
    assert_eq!(reported_canonical, expected_canonical);

    let sarif_file =
        std::fs::read_to_string(temp.path().join("scan.sarif")).expect("read sarif file");
    let sarif_file: Value = serde_json::from_str(&sarif_file).expect("parse sarif file");
    assert_eq!(sarif_file["version"], "2.1.0");
}
