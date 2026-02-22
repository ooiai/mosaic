use assert_cmd::Command;
use serde_json::Value;
use tempfile::tempdir;

#[test]
#[allow(deprecated)]
fn models_status_set_aliases_and_fallbacks_flow() {
    let temp = tempdir().expect("tempdir");

    Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "setup",
            "--base-url",
            "mock://mock-model",
            "--model",
            "mock-model",
        ])
        .assert()
        .success();

    let aliases_set = Command::cargo_bin("mosaic")
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
        .stdout
        .clone();
    let aliases_set_json: Value = serde_json::from_slice(&aliases_set).expect("aliases json");
    assert_eq!(aliases_set_json["ok"], true);
    assert_eq!(aliases_set_json["aliases"]["fast"], "mock-model");

    Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "configure",
            "--model",
            "fast",
            "--base-url",
            "mock://mock-model",
            "--api-key-env",
            "OPENAI_API_KEY",
        ])
        .assert()
        .success();

    let status_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args(["--project-state", "--json", "models", "status"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let status_json: Value = serde_json::from_slice(&status_output).expect("status json");
    assert_eq!(status_json["ok"], true);
    assert_eq!(status_json["current_model"], "fast");
    assert_eq!(status_json["effective_model"], "mock-model");
    assert_eq!(status_json["used_alias"], "fast");

    let set_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args(["--project-state", "--json", "models", "set", "fast"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let set_json: Value = serde_json::from_slice(&set_output).expect("set json");
    assert_eq!(set_json["ok"], true);
    assert_eq!(set_json["requested_model"], "fast");
    assert_eq!(set_json["effective_model"], "mock-model");
    assert_eq!(set_json["used_alias"], "fast");

    let fallbacks_add = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "models",
            "fallbacks",
            "add",
            "backup-model",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let fallbacks_add_json: Value =
        serde_json::from_slice(&fallbacks_add).expect("fallback add json");
    assert_eq!(fallbacks_add_json["ok"], true);
    assert_eq!(fallbacks_add_json["fallbacks"][0], "backup-model");

    let fallbacks_list = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args(["--project-state", "--json", "models", "fallbacks", "list"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let fallbacks_list_json: Value =
        serde_json::from_slice(&fallbacks_list).expect("fallback list json");
    assert_eq!(fallbacks_list_json["ok"], true);
    assert_eq!(fallbacks_list_json["fallbacks"][0], "backup-model");
}
