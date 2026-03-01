use assert_cmd::Command;
use serde_json::Value;
use tempfile::tempdir;

#[test]
#[allow(deprecated)]
fn configure_get_set_unset_flow() {
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

    let set_base_url = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "configure",
            "set",
            "provider.base_url",
            "https://example.test/v1",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let set_base_url: Value = serde_json::from_slice(&set_base_url).expect("set base_url json");
    assert_eq!(set_base_url["ok"], true);
    assert_eq!(set_base_url["action"], "set");
    assert_eq!(set_base_url["key"], "provider.base_url");
    assert_eq!(set_base_url["value"], "https://example.test/v1");

    let get_base_url = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "configure",
            "get",
            "provider.base_url",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let get_base_url: Value = serde_json::from_slice(&get_base_url).expect("get base_url json");
    assert_eq!(get_base_url["ok"], true);
    assert_eq!(get_base_url["action"], "get");
    assert_eq!(get_base_url["value"], "https://example.test/v1");

    let set_guard_mode = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "configure",
            "set",
            "tools.run.guard_mode",
            "unrestricted",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let set_guard_mode: Value =
        serde_json::from_slice(&set_guard_mode).expect("set guard mode json");
    assert_eq!(set_guard_mode["ok"], true);
    assert_eq!(set_guard_mode["value"], "unrestricted");

    let set_tools_enabled = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "configure",
            "set",
            "tools.enabled",
            "false",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let set_tools_enabled: Value =
        serde_json::from_slice(&set_tools_enabled).expect("set tools.enabled json");
    assert_eq!(set_tools_enabled["ok"], true);
    assert_eq!(set_tools_enabled["value"], false);

    let unset_base_url = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "configure",
            "unset",
            "provider.base_url",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let unset_base_url: Value =
        serde_json::from_slice(&unset_base_url).expect("unset base_url json");
    assert_eq!(unset_base_url["ok"], true);
    assert_eq!(unset_base_url["action"], "unset");
    assert_eq!(unset_base_url["value"], "https://api.openai.com");

    let get_base_url_after_unset = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "configure",
            "get",
            "provider.base_url",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let get_base_url_after_unset: Value =
        serde_json::from_slice(&get_base_url_after_unset).expect("get base_url unset json");
    assert_eq!(get_base_url_after_unset["ok"], true);
    assert_eq!(get_base_url_after_unset["value"], "https://api.openai.com");
}
