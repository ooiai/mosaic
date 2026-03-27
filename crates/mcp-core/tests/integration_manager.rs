use std::path::PathBuf;

use mosaic_mcp_core::{McpServerManager, McpServerSpec};
use mosaic_tool_core::ToolRegistry;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("mcp-core crate should live under crates/")
        .parent()
        .expect("repo root should exist")
        .to_path_buf()
}

#[tokio::test]
async fn mcp_server_manager_registers_and_calls_mock_stdio_tool() {
    let root = repo_root();
    let manager = McpServerManager::start(&[McpServerSpec {
        name: "filesystem".to_owned(),
        command: "python3".to_owned(),
        args: vec![
            root.join("scripts/mock_mcp_server.py")
                .display()
                .to_string(),
            "filesystem".to_owned(),
        ],
    }])
    .expect("MCP manager should start");

    let mut registry = ToolRegistry::new();
    let registered = manager
        .register_tools(&mut registry)
        .expect("MCP tools should register");
    assert_eq!(registered.len(), 1);

    let readme = root.join("README.md");
    let tool = registry
        .get("mcp.filesystem.read_file")
        .expect("qualified MCP tool should exist");
    let result = tool
        .call(serde_json::json!({ "path": readme.display().to_string() }))
        .await
        .expect("remote MCP tool should succeed");

    assert!(result.content.contains("# Mosaic"));
    assert!(!result.is_error);
}
