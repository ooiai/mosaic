use std::{path::PathBuf, sync::Arc};

use mosaic_config::{ExtensionManifestRef, ExtensionsConfig, MosaicConfig};
use mosaic_extension_core::{load_extension_set, validate_extension_set};
use mosaic_scheduler_core::FileCronStore;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("extension-core crate should live under crates/")
        .parent()
        .expect("repo root should exist")
        .to_path_buf()
}

#[test]
fn extension_set_loads_example_manifest_into_public_registries() {
    let root = repo_root();
    let mut config = MosaicConfig::default();
    config.extensions = ExtensionsConfig {
        manifests: vec![ExtensionManifestRef {
            path: "examples/extensions/time-and-summary.yaml".to_owned(),
            version_pin: Some("0.1.0".to_owned()),
            enabled: true,
        }],
    };

    let report = validate_extension_set(&config, None, &root);
    assert!(
        report.is_ok(),
        "extension validation issues: {:?}",
        report.issues
    );

    let loaded = load_extension_set(
        &config,
        None,
        &root,
        Arc::new(FileCronStore::new(root.join(".tmp-extension-cron"))),
    )
    .expect("extension set should load");

    assert!(loaded.skills.get("summarize_notes").is_some());
    assert!(loaded.workflows.get("summarize_operator_note").is_some());
}
