use assert_cmd::Command;
use serde_json::Value;
use tempfile::tempdir;

#[test]
#[allow(deprecated)]
fn plugins_and_skills_list_info_check_flow() {
    let temp = tempdir().expect("tempdir");
    let state_root = temp.path().join(".mosaic");
    let plugin_dir = state_root.join("plugins").join("demo");
    let skill_dir = state_root.join("skills").join("writer");
    std::fs::create_dir_all(&plugin_dir).expect("create plugin dir");
    std::fs::create_dir_all(&skill_dir).expect("create skill dir");
    std::fs::write(
        plugin_dir.join("plugin.toml"),
        "[plugin]\nid = \"demo\"\nname = \"Demo Plugin\"\nversion = \"0.1.0\"\n",
    )
    .expect("write plugin manifest");
    std::fs::write(
        skill_dir.join("SKILL.md"),
        "# Writer\nCreate short release notes.\n",
    )
    .expect("write skill file");

    let plugins_list = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args(["--project-state", "--json", "plugins", "list"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let plugins_list: Value = serde_json::from_slice(&plugins_list).expect("plugins list json");
    assert_eq!(plugins_list["ok"], true);
    assert!(
        plugins_list["plugins"]
            .as_array()
            .expect("plugins array")
            .iter()
            .any(|item| item["id"].as_str() == Some("demo"))
    );

    let plugins_info = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args(["--project-state", "--json", "plugins", "info", "demo"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let plugins_info: Value = serde_json::from_slice(&plugins_info).expect("plugins info json");
    assert_eq!(plugins_info["ok"], true);
    assert_eq!(plugins_info["plugin"]["id"], "demo");
    assert_eq!(plugins_info["plugin"]["manifest_valid"], true);

    let plugins_check = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args(["--project-state", "--json", "plugins", "check", "demo"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let plugins_check: Value = serde_json::from_slice(&plugins_check).expect("plugins check json");
    assert_eq!(plugins_check["ok"], true);
    assert_eq!(plugins_check["report"]["ok"], true);
    assert_eq!(plugins_check["report"]["checked"], 1);

    let skills_list = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args(["--project-state", "--json", "skills", "list"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let skills_list: Value = serde_json::from_slice(&skills_list).expect("skills list json");
    assert_eq!(skills_list["ok"], true);
    assert!(
        skills_list["skills"]
            .as_array()
            .expect("skills array")
            .iter()
            .any(|item| item["id"].as_str() == Some("writer"))
    );

    let skills_info = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args(["--project-state", "--json", "skills", "info", "writer"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let skills_info: Value = serde_json::from_slice(&skills_info).expect("skills info json");
    assert_eq!(skills_info["ok"], true);
    assert_eq!(skills_info["skill"]["id"], "writer");
    assert_eq!(skills_info["skill"]["title"], "Writer");

    let skills_check = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args(["--project-state", "--json", "skills", "check", "writer"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let skills_check: Value = serde_json::from_slice(&skills_check).expect("skills check json");
    assert_eq!(skills_check["ok"], true);
    assert_eq!(skills_check["report"]["ok"], true);
    assert_eq!(skills_check["report"]["checked"], 1);
}

#[test]
#[allow(deprecated)]
fn plugins_info_missing_returns_validation_error() {
    let temp = tempdir().expect("tempdir");
    let output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args(["--project-state", "--json", "plugins", "info", "missing"])
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
fn plugins_and_skills_install_remove_flow() {
    let temp = tempdir().expect("tempdir");
    let plugin_source = temp.path().join("sample-plugin");
    let skill_source = temp.path().join("writer");
    std::fs::create_dir_all(&plugin_source).expect("create plugin source");
    std::fs::create_dir_all(&skill_source).expect("create skill source");
    std::fs::write(
        plugin_source.join("plugin.toml"),
        "[plugin]\nid = \"sample_plugin\"\nname = \"Sample Plugin\"\nversion = \"0.1.0\"\n",
    )
    .expect("write plugin manifest");
    std::fs::write(
        skill_source.join("SKILL.md"),
        "# Writer\nGenerate concise notes.\n",
    )
    .expect("write skill file");

    let plugin_install = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "plugins",
            "install",
            "--path",
            "sample-plugin",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let plugin_install: Value =
        serde_json::from_slice(&plugin_install).expect("plugin install json");
    assert_eq!(plugin_install["ok"], true);
    assert_eq!(plugin_install["installed"]["id"], "sample_plugin");

    let skill_install = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "skills",
            "install",
            "--path",
            "writer",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let skill_install: Value = serde_json::from_slice(&skill_install).expect("skill install json");
    assert_eq!(skill_install["ok"], true);
    assert_eq!(skill_install["installed"]["id"], "writer");

    let plugin_remove = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "plugins",
            "remove",
            "sample_plugin",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let plugin_remove: Value = serde_json::from_slice(&plugin_remove).expect("plugin remove json");
    assert_eq!(plugin_remove["ok"], true);
    assert_eq!(plugin_remove["removed"], true);

    let skill_remove = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args(["--project-state", "--json", "skills", "remove", "writer"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let skill_remove: Value = serde_json::from_slice(&skill_remove).expect("skill remove json");
    assert_eq!(skill_remove["ok"], true);
    assert_eq!(skill_remove["removed"], true);
}
