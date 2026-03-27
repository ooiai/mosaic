use std::{
    fs,
    path::PathBuf,
    process,
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use mosaic_config::{
    LoadConfigOptions, load_from_file, load_mosaic_config, validate_mosaic_config,
};

static COUNTER: AtomicU64 = AtomicU64::new(0);

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("config crate should live under crates/")
        .parent()
        .expect("repo root should exist")
        .to_path_buf()
}

fn temp_dir(label: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock should be after epoch")
        .as_nanos();
    let count = COUNTER.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "mosaic-config-integration-{label}-{}-{nanos}-{count}",
        process::id()
    ))
}

#[test]
fn loads_example_app_and_layered_workspace_config() {
    let root = repo_root();
    let app = load_from_file(root.join("examples/time-now-agent.yaml"))
        .expect("example app config should load");
    assert_eq!(app.provider.provider_type, "openai");
    assert_eq!(app.task.input, "What time is it now?");

    let workspace = temp_dir("workspace");
    fs::create_dir_all(workspace.join(".mosaic")).expect("workspace should exist");
    fs::write(
        workspace.join(".mosaic/config.yaml"),
        r#"
active_profile: gpt-5.4-mini
deployment:
  profile: staging
  workspace_name: integration
"#,
    )
    .expect("workspace config should write");

    let loaded = load_mosaic_config(&LoadConfigOptions {
        cwd: workspace.clone(),
        user_config_path: None,
        workspace_config_path: None,
        overrides: Default::default(),
    })
    .expect("mosaic config should load");
    let report = validate_mosaic_config(&loaded.config);

    assert!(report.is_ok(), "validation issues: {:?}", report.issues);
    assert_eq!(loaded.config.deployment.profile, "staging");
    assert_eq!(loaded.config.deployment.workspace_name, "integration");

    fs::remove_dir_all(workspace).ok();
}
