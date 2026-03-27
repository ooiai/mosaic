use std::{env, path::PathBuf};

use mosaic_mcp_core::{McpClient, McpServerSpec, StdioMcpTransport};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("mcp-core crate should live under crates/")
        .parent()
        .expect("repo root should exist")
        .to_path_buf()
}

#[tokio::test]
async fn real_mock_stdio_server_runs_over_subprocess_transport_when_enabled() {
    if env::var("MOSAIC_REAL_TESTS").ok().as_deref() != Some("1") {
        eprintln!("skipping real MCP stdio test: set MOSAIC_REAL_TESTS=1");
        return;
    }

    let root = repo_root();
    let transport = StdioMcpTransport::start(&McpServerSpec {
        name: "filesystem".to_owned(),
        command: "python3".to_owned(),
        args: vec![
            root.join("scripts/mock_mcp_server.py")
                .display()
                .to_string(),
            "filesystem".to_owned(),
        ],
    })
    .expect("stdio MCP transport should start");
    let client = McpClient::new(std::sync::Arc::new(transport));

    let tools = client.list_tools().await.expect("tool list should succeed");
    assert_eq!(tools.len(), 1);

    let result = client
        .call_tool(
            "read_file",
            serde_json::json!({ "path": root.join("README.md").display().to_string() }),
        )
        .await
        .expect("remote call should succeed");

    assert!(result.content.contains("# Mosaic"));
}
