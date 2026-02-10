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
