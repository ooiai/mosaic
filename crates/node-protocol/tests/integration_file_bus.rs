use std::{
    fs,
    path::PathBuf,
    process,
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use mosaic_node_protocol::{
    FileNodeStore, NodeCapabilityDeclaration, NodeCommandDispatch, NodeRegistration,
};
use mosaic_tool_core::{CapabilityKind, PermissionScope, ToolRiskLevel};

static COUNTER: AtomicU64 = AtomicU64::new(0);

fn temp_dir(label: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock should be after epoch")
        .as_nanos();
    let count = COUNTER.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "mosaic-node-integration-{label}-{}-{nanos}-{count}",
        process::id()
    ))
}

#[test]
fn file_node_store_persists_registration_and_dispatch_files() {
    let dir = temp_dir("node");
    let store = FileNodeStore::new(&dir);
    let registration = NodeRegistration::new(
        "node-a",
        "Node A",
        "file-bus",
        "headless",
        vec![NodeCapabilityDeclaration {
            name: "read_file".to_owned(),
            kind: CapabilityKind::File,
            permission_scopes: vec![PermissionScope::LocalRead],
            risk: ToolRiskLevel::Medium,
        }],
    );

    store
        .register_node(&registration)
        .expect("node registration should persist");
    store
        .dispatch_command(&NodeCommandDispatch::new(
            "node-a",
            Some("demo".to_owned()),
            "read_file",
            "read_file",
            serde_json::json!({"tool": "read_file"}),
        ))
        .expect("dispatch should persist");

    let loaded = store
        .load_node("node-a")
        .expect("node load should succeed")
        .expect("node should exist");
    let dispatches = store
        .pending_commands("node-a")
        .expect("dispatches should load");

    assert_eq!(loaded.node_id, "node-a");
    assert_eq!(dispatches.len(), 1);
    assert_eq!(dispatches[0].capability, "read_file");

    fs::remove_dir_all(dir).ok();
}
