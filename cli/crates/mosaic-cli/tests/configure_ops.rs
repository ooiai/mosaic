use assert_cmd::Command;
use serde_json::Value;
use std::fs;
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

#[test]
#[allow(deprecated)]
fn configure_keys_and_patch_flow() {
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

    let keys_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args(["--project-state", "--json", "configure", "keys"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let keys_json: Value = serde_json::from_slice(&keys_output).expect("keys json");
    assert_eq!(keys_json["ok"], true);
    assert_eq!(keys_json["action"], "keys");
    let keys = keys_json["keys"].as_array().expect("keys array");
    assert_eq!(keys.len(), 7);
    assert!(
        keys.iter()
            .any(|entry| entry["name"] == "provider.base_url")
    );

    let patch_file = temp.path().join("configure-patch.json");
    fs::write(
        &patch_file,
        r#"{
  "provider": { "base_url": "https://patch.example/v1" },
  "agent": { "temperature": 0.9, "max_turns": 12 },
  "tools": { "enabled": false, "run": { "guard_mode": "all_confirm" } }
}"#,
    )
    .expect("write patch file");

    let dry_run_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "configure",
            "patch",
            "--set",
            "provider.model=dry-run-model",
            "--dry-run",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let dry_run_json: Value = serde_json::from_slice(&dry_run_output).expect("dry-run patch json");
    assert_eq!(dry_run_json["ok"], true);
    assert_eq!(dry_run_json["action"], "patch");
    assert_eq!(dry_run_json["dry_run"], true);
    assert_eq!(dry_run_json["saved"], false);
    assert_eq!(dry_run_json["changed_keys"], 1);
    assert_eq!(dry_run_json["updates"][0]["key"], "provider.model");
    assert_eq!(dry_run_json["updates"][0]["group"], "provider");
    assert_eq!(dry_run_json["updates"][0]["from"], "mock-model");
    assert_eq!(dry_run_json["updates"][0]["to"], "dry-run-model");
    assert_eq!(dry_run_json["groups"][0]["group"], "provider");
    assert_eq!(dry_run_json["groups"][0]["updated"], 1);
    assert_eq!(dry_run_json["groups"][0]["changed"], 1);

    let model_after_dry_run = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "configure",
            "get",
            "provider.model",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let model_after_dry_run_json: Value =
        serde_json::from_slice(&model_after_dry_run).expect("model after dry run");
    assert_eq!(model_after_dry_run_json["value"], "mock-model");

    let patch_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "configure",
            "patch",
            "--file",
            patch_file.to_string_lossy().as_ref(),
            "--set",
            "provider.model=gpt-4.1-mini",
            "--set",
            "tools.enabled=true",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let patch_json: Value = serde_json::from_slice(&patch_output).expect("patch json");
    assert_eq!(patch_json["ok"], true);
    assert_eq!(patch_json["action"], "patch");
    assert_eq!(patch_json["dry_run"], false);
    assert_eq!(patch_json["saved"], true);
    assert_eq!(patch_json["updated"], 6);
    assert_eq!(patch_json["changed_keys"], 5);
    assert_eq!(patch_json["file"], patch_file.to_string_lossy().to_string());
    let groups = patch_json["groups"].as_array().expect("groups array");
    let provider_group = groups
        .iter()
        .find(|group| group["group"] == "provider")
        .expect("provider group");
    let agent_group = groups
        .iter()
        .find(|group| group["group"] == "agent")
        .expect("agent group");
    let tools_group = groups
        .iter()
        .find(|group| group["group"] == "tools")
        .expect("tools group");
    assert_eq!(provider_group["updated"], 2);
    assert_eq!(provider_group["changed"], 2);
    assert_eq!(agent_group["updated"], 2);
    assert_eq!(agent_group["changed"], 2);
    assert_eq!(tools_group["updated"], 2);
    assert_eq!(tools_group["changed"], 1);

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
    let get_base_url_json: Value = serde_json::from_slice(&get_base_url).expect("get base url");
    assert_eq!(get_base_url_json["value"], "https://patch.example/v1");

    let get_model = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args(["--project-state", "--json", "configure", "get", "model"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let get_model_json: Value = serde_json::from_slice(&get_model).expect("get model");
    assert_eq!(get_model_json["value"], "gpt-4.1-mini");

    let get_guard_mode = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "configure",
            "get",
            "tools.run.guard_mode",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let get_guard_mode_json: Value = serde_json::from_slice(&get_guard_mode).expect("get guard");
    assert_eq!(get_guard_mode_json["value"], "all_confirm");

    let toml_patch_file = temp.path().join("configure-patch.toml");
    fs::write(
        &toml_patch_file,
        r#"[tools]
enabled = false

[agent]
temperature = 0.1
"#,
    )
    .expect("write toml patch file");

    let toml_patch_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "configure",
            "patch",
            "--file",
            toml_patch_file.to_string_lossy().as_ref(),
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let toml_patch_json: Value = serde_json::from_slice(&toml_patch_output).expect("toml patch");
    assert_eq!(toml_patch_json["ok"], true);
    assert_eq!(toml_patch_json["updated"], 2);
    let toml_groups = toml_patch_json["groups"].as_array().expect("toml groups");
    assert!(
        toml_groups
            .iter()
            .any(|group| group["group"] == "agent" && group["changed"] == 1)
    );
    assert!(
        toml_groups
            .iter()
            .any(|group| group["group"] == "tools" && group["changed"] == 1)
    );

    let get_tools_enabled = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "configure",
            "get",
            "tools.enabled",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let get_tools_enabled_json: Value =
        serde_json::from_slice(&get_tools_enabled).expect("get tools.enabled");
    assert_eq!(get_tools_enabled_json["value"], false);
}

#[test]
#[allow(deprecated)]
fn configure_preview_template_and_target_profile_flow() {
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

    let preview_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "configure",
            "preview",
            "--target-profile",
            "migration",
            "--set",
            "provider.model=migrated-model",
            "--set",
            "tools.enabled=false",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let preview_json: Value = serde_json::from_slice(&preview_output).expect("preview json");
    assert_eq!(preview_json["ok"], true);
    assert_eq!(preview_json["action"], "preview");
    assert_eq!(preview_json["target_profile"], "migration");
    assert_eq!(preview_json["target_profile_exists"], false);
    assert_eq!(preview_json["dry_run"], true);
    assert_eq!(preview_json["saved"], false);
    assert_eq!(preview_json["updated"], 2);
    assert_eq!(preview_json["changed_keys"], 2);

    let config_path = temp.path().join(".mosaic/config.toml");
    let config_raw_before = fs::read_to_string(&config_path).expect("read config before patch");
    assert!(
        !config_raw_before.contains("[profiles.migration"),
        "preview must not persist target profile"
    );

    let patch_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "configure",
            "patch",
            "--target-profile",
            "migration",
            "--set",
            "provider.model=migrated-model",
            "--set",
            "tools.enabled=false",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let patch_json: Value = serde_json::from_slice(&patch_output).expect("patch json");
    assert_eq!(patch_json["ok"], true);
    assert_eq!(patch_json["action"], "patch");
    assert_eq!(patch_json["target_profile"], "migration");
    assert_eq!(patch_json["target_profile_exists"], false);
    assert_eq!(patch_json["dry_run"], false);
    assert_eq!(patch_json["saved"], true);
    assert_eq!(patch_json["updated"], 2);

    let config_raw_after = fs::read_to_string(&config_path).expect("read config after patch");
    assert!(
        config_raw_after.contains("[profiles.migration"),
        "patch with target-profile should persist migration profile"
    );

    let get_migration_model = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--profile",
            "migration",
            "--json",
            "configure",
            "get",
            "provider.model",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let get_migration_model_json: Value =
        serde_json::from_slice(&get_migration_model).expect("get migration model");
    assert_eq!(get_migration_model_json["value"], "migrated-model");

    let template_json_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "configure",
            "template",
            "--target-profile",
            "migration",
            "--format",
            "json",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let template_json: Value =
        serde_json::from_slice(&template_json_output).expect("template json");
    assert_eq!(template_json["ok"], true);
    assert_eq!(template_json["action"], "template");
    assert_eq!(template_json["target_profile"], "migration");
    assert_eq!(template_json["target_profile_exists"], true);
    assert_eq!(template_json["defaults"], false);
    assert_eq!(template_json["format"], "json");
    assert_eq!(
        template_json["template_json"]["provider"]["model"],
        "migrated-model"
    );
    let rendered_template_json: Value = serde_json::from_str(
        template_json["template"]
            .as_str()
            .expect("json template string"),
    )
    .expect("parse rendered json template");
    assert_eq!(
        rendered_template_json["provider"]["model"],
        "migrated-model"
    );

    let template_toml_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "configure",
            "template",
            "--target-profile",
            "migration",
            "--format",
            "toml",
            "--defaults",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let template_toml: Value =
        serde_json::from_slice(&template_toml_output).expect("template toml");
    assert_eq!(template_toml["ok"], true);
    assert_eq!(template_toml["format"], "toml");
    assert_eq!(template_toml["defaults"], true);
    let template_toml_text = template_toml["template"]
        .as_str()
        .expect("toml template string");
    assert!(template_toml_text.contains("model = \"gpt-4o-mini\""));
    assert!(template_toml_text.contains("guard_mode = \"confirm_dangerous\""));
}
