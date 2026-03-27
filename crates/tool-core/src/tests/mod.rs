use std::{
    fs, process,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::{SystemTime, UNIX_EPOCH},
};

use chrono::DateTime;
use futures::executor::block_on;
use mosaic_scheduler_core::{CronStore, FileCronStore};

use crate::{
    CapabilityKind, CapabilityMetadata, CronRegisterTool, EchoTool, ExecTool, ReadFileTool,
    TimeNowTool, Tool, ToolMetadata, ToolRegistry, ToolSource, mcp_tool_name,
};

static COUNTER: AtomicU64 = AtomicU64::new(0);

fn temp_file_path(label: &str) -> std::path::PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock should be after epoch")
        .as_nanos();
    let count = COUNTER.fetch_add(1, Ordering::Relaxed);

    std::env::temp_dir().join(format!(
        "mosaic-tool-core-{label}-{}-{nanos}-{count}.txt",
        process::id()
    ))
}

fn temp_dir(label: &str) -> std::path::PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock should be after epoch")
        .as_nanos();
    let count = COUNTER.fetch_add(1, Ordering::Relaxed);

    std::env::temp_dir().join(format!(
        "mosaic-tool-core-{label}-{}-{nanos}-{count}",
        process::id()
    ))
}

#[test]
fn builtin_echo_tool_is_registered_and_callable() {
    let mut registry = ToolRegistry::new();
    registry.register(Arc::new(EchoTool::new()));

    let tool = registry.get("echo").expect("echo tool should exist");
    let result = block_on(tool.call(serde_json::json!({ "text": "hello" })))
        .expect("echo tool should succeed");

    assert_eq!(result.content, "hello");
    assert_eq!(
        result.structured,
        Some(serde_json::json!({ "text": "hello" }))
    );
    assert!(!result.is_error);
}

#[test]
fn time_now_tool_returns_iso_timestamp() {
    let tool = TimeNowTool::new();
    let result = block_on(tool.call(serde_json::json!({}))).expect("time_now tool should succeed");

    let parsed = DateTime::parse_from_rfc3339(&result.content)
        .expect("time_now tool should return RFC3339 timestamp");

    assert_eq!(
        parsed.with_timezone(&chrono::Utc).to_rfc3339(),
        result.content
    );
    assert!(!result.is_error);
}

#[test]
fn read_file_tool_reads_text_files_within_allowed_root() {
    let dir = temp_dir("read-file");
    fs::create_dir_all(&dir).expect("temp dir should be created");
    let path = dir.join("example.txt");
    fs::write(&path, "hello from file").expect("temp file should be written");

    let tool = ReadFileTool::new_with_allowed_roots(vec![dir.clone()]);
    let result = block_on(tool.call(serde_json::json!({
        "path": path.display().to_string()
    })))
    .expect("read_file tool should succeed");

    assert_eq!(result.content, "hello from file");
    assert_eq!(
        result.audit.expect("audit should exist").kind,
        CapabilityKind::File
    );
}

#[test]
fn read_file_tool_rejects_paths_outside_allowed_roots() {
    let dir = temp_dir("read-file-guard");
    fs::create_dir_all(&dir).expect("temp dir should be created");
    let path = temp_file_path("blocked");
    fs::write(&path, "blocked").expect("temp file should be written");
    let tool = ReadFileTool::new_with_allowed_roots(vec![dir]);

    let err = block_on(tool.call(serde_json::json!({
        "path": path.display().to_string()
    })))
    .expect_err("read_file should reject paths outside the allowed roots");

    assert!(
        err.to_string()
            .contains("outside the allowed capability roots")
    );
}

#[tokio::test]
async fn exec_tool_runs_local_command() {
    let tool = ExecTool::new(vec![std::env::current_dir().expect("cwd should exist")]);
    let result = tool
        .call(serde_json::json!({
            "command": "pwd"
        }))
        .await
        .expect("exec tool should run");

    assert!(!result.content.is_empty());
    assert!(!result.is_error);
    assert_eq!(
        result.audit.expect("audit should exist").kind,
        CapabilityKind::Exec
    );
}

#[tokio::test]
async fn cron_register_tool_persists_registration() {
    let store: Arc<dyn CronStore> = Arc::new(FileCronStore::new(temp_dir("cron")));
    let tool = CronRegisterTool::new(store.clone());

    let result = tool
        .call(serde_json::json!({
            "id": "nightly",
            "schedule": "0 0 * * *",
            "input": "run nightly",
            "session_id": "demo"
        }))
        .await
        .expect("cron register should succeed");

    assert!(!result.is_error);
    assert!(
        store
            .load("nightly")
            .expect("load should succeed")
            .is_some()
    );
}

#[test]
fn mcp_tool_name_uses_expected_prefix() {
    assert_eq!(
        mcp_tool_name("filesystem", "read_file"),
        "mcp.filesystem.read_file"
    );
}

#[test]
fn builtin_tool_metadata_defaults_to_builtin_source() {
    let meta = ToolMetadata::builtin("echo", "Echo", serde_json::json!({}));
    assert_eq!(meta.source, ToolSource::Builtin);
    assert_eq!(meta.capability.kind, CapabilityKind::Utility);
}

#[test]
fn mcp_tool_metadata_captures_server_context() {
    let meta = ToolMetadata::mcp(
        "filesystem",
        "read_file",
        "Read files over MCP",
        serde_json::json!({ "type": "object" }),
    );

    assert_eq!(meta.name, "mcp.filesystem.read_file");
    assert_eq!(meta.source.label(), "mcp");
    assert_eq!(meta.source.server_name(), Some("filesystem"));
    assert_eq!(meta.source.remote_tool_name(), Some("read_file"));
}

#[test]
fn node_routed_tool_metadata_exposes_capability_preferences() {
    let read_file = ReadFileTool::new_with_allowed_roots(Vec::new());
    let exec = ExecTool::new(Vec::new());

    assert_eq!(
        read_file.metadata().capability.node.capability.as_deref(),
        Some("read_file")
    );
    assert!(read_file.metadata().capability.node.prefer_node);
    assert!(!read_file.metadata().capability.node.require_node);

    assert_eq!(
        exec.metadata().capability.node.capability.as_deref(),
        Some("exec_command")
    );
    assert!(exec.metadata().capability.node.prefer_node);
    assert!(!exec.metadata().capability.node.require_node);
}

#[test]
fn capability_metadata_can_describe_stubbed_capabilities() {
    let meta = CapabilityMetadata::abstraction(CapabilityKind::Browser);

    assert_eq!(meta.kind, CapabilityKind::Browser);
    assert!(!meta.authorized);
    assert!(!meta.healthy);
}
