use super::*;
use std::{
    path::PathBuf,
    process,
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

static COUNTER: AtomicU64 = AtomicU64::new(0);

fn temp_dir(label: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock should be after epoch")
        .as_nanos();
    let count = COUNTER.fetch_add(1, Ordering::Relaxed);

    let path = std::env::temp_dir().join(format!(
        "mosaic-config-{label}-{}-{nanos}-{count}",
        process::id()
    ));
    fs::create_dir_all(&path).expect("temp dir should be created");
    path
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("config crate should live under crates/")
        .parent()
        .expect("repo root should exist")
        .to_path_buf()
}

#[test]
fn provider_and_deployment_example_patches_validate_against_defaults() {
    let root = repo_root();
    for rel in [
        "examples/providers/openai.yaml",
        "examples/providers/azure.yaml",
        "examples/providers/ollama.yaml",
        "examples/providers/anthropic.yaml",
        "examples/deployment/production.config.yaml",
    ] {
        let patch = load_config_patch(&root.join(rel)).expect("example patch should load");
        let mut config = MosaicConfig::default();
        merge_patch(&mut config, patch);
        let report = validate_mosaic_config(&config);
        assert!(
            !report.has_errors(),
            "{rel} should validate without errors: {:?}",
            report.issues
        );
    }
}

#[test]
fn workflow_and_gateway_examples_parse_from_disk() {
    let root = repo_root();
    let app = load_from_file(root.join("examples/workflows/research-brief.yaml"))
        .expect("workflow example should load");
    assert_eq!(
        app.app.and_then(|app| app.name).as_deref(),
        Some("research-brief")
    );

    let payload = fs::read_to_string(root.join("examples/gateway/webchat-message.json"))
        .expect("gateway payload should load");
    let parsed: serde_json::Value =
        serde_json::from_str(&payload).expect("gateway payload should parse as JSON");
    assert_eq!(parsed["session_id"], "docs-webchat");
}

#[test]
fn loads_yaml_app_config_from_disk() {
    let dir = temp_dir("app-load");
    let path = dir.join("app.yaml");

    fs::write(
        &path,
        r#"
app:
  name: basic-demo
provider:
  type: openai-compatible
  model: mock
  api_key_env: OPENAI_API_KEY
tools:
  - type: builtin
    name: echo
skills:
  - type: builtin
    name: summarize
agent:
  system: You are helpful.
task:
  input: Explain Mosaic.
"#,
    )
    .expect("fixture should be written");

    let cfg = load_from_file(&path).expect("config should load");

    assert_eq!(cfg.provider.provider_type, "openai-compatible");
    assert_eq!(cfg.tools.len(), 1);
    assert_eq!(cfg.skills.len(), 1);
    assert_eq!(cfg.task.input, "Explain Mosaic.");

    fs::remove_dir_all(dir).ok();
}

#[test]
fn loads_extension_manifest_and_policy_config() {
    let dir = temp_dir("extension-load");
    let workspace = dir.join("workspace.yaml");
    let manifest = dir.join("demo-extension.yaml");

    fs::write(
        &workspace,
        r#"
extensions:
  manifests:
    - path: demo-extension.yaml
      version_pin: 0.2.0
policies:
  allow_exec: false
  allow_webhook: true
  allow_cron: true
  allow_mcp: false
  hot_reload_enabled: true
"#,
    )
    .expect("workspace config should be written");

    fs::write(
        &manifest,
        r#"
name: demo.extension
version: 0.2.0
description: demo manifest
tools:
  - type: builtin
    name: time_now
skills:
  - type: builtin
    name: summarize
workflows: []
"#,
    )
    .expect("extension manifest should be written");

    let loaded = load_mosaic_config(&LoadConfigOptions {
        cwd: dir.clone(),
        user_config_path: None,
        workspace_config_path: Some(workspace.clone()),
        overrides: ConfigOverrides::default(),
    })
    .expect("workspace config should load");
    let manifest =
        load_extension_manifest_from_file(&manifest).expect("extension manifest should parse");

    assert_eq!(loaded.config.extensions.manifests.len(), 1);
    assert!(!loaded.config.policies.allow_exec);
    assert!(!loaded.config.policies.allow_mcp);
    assert_eq!(manifest.name, "demo.extension");
    assert_eq!(manifest.version, "0.2.0");

    fs::remove_dir_all(dir).ok();
}

#[test]
fn layered_mosaic_config_prefers_workspace_over_user_and_cli_over_env() {
    let dir = temp_dir("layered");
    let user = dir.join("user.yaml");
    let workspace = dir.join("workspace.yaml");

    fs::write(
        &user,
        r#"
active_profile: gpt-5.4-mini
profiles:
  custom-user:
    type: mock
    model: mock-user
"#,
    )
    .expect("user config should be written");

    fs::write(
        &workspace,
        r#"
active_profile: gpt-5.4
profiles:
  custom-workspace:
    type: mock
    model: mock-workspace
"#,
    )
    .expect("workspace config should be written");

    // SAFETY: tests in this crate do not spawn threads that read process env.
    unsafe {
        env::set_var(ACTIVE_PROFILE_ENV, "mock");
    }

    let loaded = load_mosaic_config(&LoadConfigOptions {
        cwd: dir.clone(),
        user_config_path: Some(user.clone()),
        workspace_config_path: Some(workspace.clone()),
        overrides: ConfigOverrides {
            active_profile: Some("custom-workspace".to_owned()),
        },
    })
    .expect("layered config should load");

    assert_eq!(loaded.config.active_profile, "custom-workspace");
    assert!(loaded.config.profiles.contains_key("custom-user"));
    assert!(loaded.config.profiles.contains_key("custom-workspace"));
    assert_eq!(loaded.sources.len(), 5);

    // SAFETY: tests in this crate do not spawn threads that read process env.
    unsafe {
        env::remove_var(ACTIVE_PROFILE_ENV);
    }

    fs::remove_dir_all(dir).ok();
}

#[test]
fn validate_reports_invalid_active_profile_and_missing_api_key_env() {
    let mut config = MosaicConfig::default();
    config.active_profile = "missing".to_owned();
    config.profiles.insert(
        "broken".to_owned(),
        ProviderProfileConfig {
            provider_type: "openai-compatible".to_owned(),
            model: "gpt-5.4".to_owned(),
            base_url: Some("https://api.openai.com/v1".to_owned()),
            api_key_env: None,
            transport: Default::default(),
            vendor: Default::default(),
        },
    );

    let report = validate_mosaic_config(&config);

    assert!(report.has_errors());
    assert!(
        report
            .issues
            .iter()
            .any(|issue| issue.field == "active_profile")
    );
    assert!(
        report
            .issues
            .iter()
            .any(|issue| issue.field == "profiles.broken.api_key_env")
    );
}

#[test]
fn validate_reports_missing_azure_base_url() {
    let mut config = MosaicConfig::default();
    config.profiles.insert(
        "azure-broken".to_owned(),
        ProviderProfileConfig {
            provider_type: "azure".to_owned(),
            model: "gpt-5.4".to_owned(),
            base_url: None,
            api_key_env: Some("AZURE_OPENAI_API_KEY".to_owned()),
            transport: Default::default(),
            vendor: Default::default(),
        },
    );

    let report = validate_mosaic_config(&config);

    assert!(report.has_errors());
    assert!(
        report
            .issues
            .iter()
            .any(|issue| issue.field == "profiles.azure-broken.base_url")
    );
}

#[test]
fn doctor_redacts_profiles_and_reports_missing_active_api_key() {
    let mut config = MosaicConfig::default();
    config.active_profile = "gpt-5.4".to_owned();
    let dir = temp_dir("doctor");

    let doctor = doctor_mosaic_config(&config, &dir);
    let redacted = redact_mosaic_config(&config);

    assert!(doctor.has_errors());
    assert!(doctor.checks.iter().any(|check| {
        check.message.contains("OPENAI_API_KEY") && matches!(check.status, DoctorStatus::Error)
    }));
    assert_eq!(redacted.active_profile, "gpt-5.4");
    assert!(
        redacted
            .profiles
            .iter()
            .any(|profile| profile.name == "gpt-5.4" && !profile.api_key_present)
    );

    fs::remove_dir_all(dir).ok();
}

#[test]
fn init_workspace_config_writes_template_and_directories() {
    let dir = temp_dir("init");
    let path = init_workspace_config(&dir, false).expect("workspace init should succeed");
    let content = fs::read_to_string(&path).expect("config should be readable");

    assert!(content.contains("schema_version: 1"));
    assert!(dir.join(".mosaic/sessions").is_dir());
    assert!(dir.join(".mosaic/runs").is_dir());
    assert!(dir.join(".mosaic/extensions").is_dir());

    fs::remove_dir_all(dir).ok();
}
