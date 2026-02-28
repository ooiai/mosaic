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

    let list_output = Command::cargo_bin("mosaic")
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
        .stdout
        .clone();
    let list_json: Value = serde_json::from_slice(&list_output).expect("models list json");
    assert_eq!(list_json["ok"], true);
    assert_eq!(list_json["query"], "mock");
    assert_eq!(list_json["limit"], 1);
    assert_eq!(list_json["total_models"], 1);
    assert_eq!(list_json["matched_models"], 1);
    assert_eq!(list_json["returned_models"], 1);
    assert_eq!(list_json["models"][0]["id"], "mock-model");

    let list_none_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "models",
            "list",
            "--query",
            "absent-model",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let list_none_json: Value =
        serde_json::from_slice(&list_none_output).expect("models list none json");
    assert_eq!(list_none_json["ok"], true);
    assert_eq!(list_none_json["query"], "absent-model");
    assert_eq!(list_none_json["matched_models"], 0);
    assert_eq!(list_none_json["returned_models"], 0);
    assert!(
        list_none_json["models"]
            .as_array()
            .expect("models array")
            .is_empty()
    );

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

    let resolve_default_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args(["--project-state", "--json", "models", "resolve"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let resolve_default_json: Value =
        serde_json::from_slice(&resolve_default_output).expect("resolve default json");
    assert_eq!(resolve_default_json["ok"], true);
    assert_eq!(resolve_default_json["requested_model"], "fast");
    assert_eq!(resolve_default_json["effective_model"], "mock-model");
    assert_eq!(resolve_default_json["used_alias"], "fast");

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

    let resolve_explicit_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args(["--project-state", "--json", "models", "resolve", "fast"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let resolve_explicit_json: Value =
        serde_json::from_slice(&resolve_explicit_output).expect("resolve explicit json");
    assert_eq!(resolve_explicit_json["ok"], true);
    assert_eq!(resolve_explicit_json["effective_model"], "mock-model");
    assert_eq!(resolve_explicit_json["fallback_chain"][0], "backup-model");
}
