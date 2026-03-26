use std::{
    collections::HashMap,
    io::{BufRead, BufReader, Write},
    process::{Child, ChildStdin, ChildStdout, Command, Stdio},
    sync::{Arc, Mutex},
};

use anyhow::{Context, Result, anyhow, bail};
use async_trait::async_trait;
use futures::executor::block_on;
use mosaic_tool_core::{Tool, ToolMetadata, ToolRegistry, ToolResult};
use serde::{Deserialize, Serialize};

#[async_trait]
pub trait McpTransport: Send + Sync {
    async fn request(&self, method: &str, params: serde_json::Value) -> Result<serde_json::Value>;
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct McpServerSpec {
    pub name: String,
    pub command: String,
    pub args: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct McpRemoteTool {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct McpRegisteredTool {
    pub server_name: String,
    pub remote_tool_name: String,
    pub qualified_name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
}

impl McpRegisteredTool {
    fn from_remote(server_name: &str, tool: &McpRemoteTool) -> Self {
        let metadata = ToolMetadata::mcp(
            server_name,
            tool.name.clone(),
            tool.description.clone(),
            tool.input_schema.clone(),
        );

        Self {
            server_name: server_name.to_owned(),
            remote_tool_name: tool.name.clone(),
            qualified_name: metadata.name,
            description: tool.description.clone(),
            input_schema: tool.input_schema.clone(),
        }
    }
}

pub struct McpClient {
    transport: Arc<dyn McpTransport>,
}

impl McpClient {
    pub fn new(transport: Arc<dyn McpTransport>) -> Self {
        Self { transport }
    }

    pub async fn list_tools(&self) -> Result<Vec<McpRemoteTool>> {
        let value = self
            .transport
            .request("tools/list", serde_json::json!({}))
            .await?;

        parse_tool_list_result(value)
    }

    pub async fn call_tool(&self, name: &str, args: serde_json::Value) -> Result<ToolResult> {
        let value = self
            .transport
            .request(
                "tools/call",
                serde_json::json!({
                    "name": name,
                    "arguments": args,
                }),
            )
            .await?;

        parse_tool_call_result(value)
    }
}

pub struct StdioMcpTransport {
    server_name: String,
    state: Mutex<StdioTransportState>,
}

struct StdioTransportState {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    next_id: u64,
}

impl StdioMcpTransport {
    pub fn start(spec: &McpServerSpec) -> Result<Self> {
        let mut child = Command::new(&spec.command)
            .args(&spec.args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .with_context(|| {
                format!(
                    "failed to start MCP server '{}' with command {}",
                    spec.name, spec.command
                )
            })?;

        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| anyhow!("failed to capture stdin for MCP server '{}'", spec.name))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| anyhow!("failed to capture stdout for MCP server '{}'", spec.name))?;

        Ok(Self {
            server_name: spec.name.clone(),
            state: Mutex::new(StdioTransportState {
                child,
                stdin,
                stdout: BufReader::new(stdout),
                next_id: 1,
            }),
        })
    }

    fn request_blocking(
        &self,
        method: &str,
        params: serde_json::Value,
    ) -> Result<serde_json::Value> {
        let mut state = self
            .state
            .lock()
            .expect("stdio MCP state lock should not be poisoned");

        if let Some(status) = state.child.try_wait()? {
            bail!(
                "MCP server '{}' exited before handling {}: {}",
                self.server_name,
                method,
                status
            );
        }

        let request_id = state.next_id;
        state.next_id += 1;

        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": request_id,
            "method": method,
            "params": params,
        });
        let encoded = serde_json::to_string(&request)?;
        state.stdin.write_all(encoded.as_bytes())?;
        state.stdin.write_all(b"\n")?;
        state.stdin.flush()?;

        let mut line = String::new();
        let bytes = state.stdout.read_line(&mut line)?;
        if bytes == 0 {
            if let Some(status) = state.child.try_wait()? {
                bail!(
                    "MCP server '{}' exited while waiting for {} response: {}",
                    self.server_name,
                    method,
                    status
                );
            }

            bail!(
                "MCP server '{}' closed stdout unexpectedly during {}",
                self.server_name,
                method
            );
        }

        let response: JsonRpcResponse =
            serde_json::from_str(line.trim_end()).with_context(|| {
                format!(
                    "invalid JSON response from MCP server '{}' during {}",
                    self.server_name, method
                )
            })?;

        if response.id != request_id {
            bail!(
                "MCP server '{}' returned mismatched response id {} for request {}",
                self.server_name,
                response.id,
                request_id
            );
        }

        if let Some(error) = response.error {
            bail!(
                "MCP server '{}' {} failed: {}",
                self.server_name,
                method,
                error.message
            );
        }

        response.result.ok_or_else(|| {
            anyhow!(
                "MCP server '{}' returned no result for {}",
                self.server_name,
                method
            )
        })
    }
}

impl Drop for StdioMcpTransport {
    fn drop(&mut self) {
        if let Ok(mut state) = self.state.lock() {
            let _ = state.child.kill();
            let _ = state.child.wait();
        }
    }
}

#[async_trait]
impl McpTransport for StdioMcpTransport {
    async fn request(&self, method: &str, params: serde_json::Value) -> Result<serde_json::Value> {
        self.request_blocking(method, params)
    }
}

pub struct McpToolAdapter {
    pub metadata: ToolMetadata,
    pub client: Arc<McpClient>,
    pub remote_name: String,
}

impl McpToolAdapter {
    pub fn new(server_name: &str, remote_tool: &McpRemoteTool, client: Arc<McpClient>) -> Self {
        Self {
            metadata: ToolMetadata::mcp(
                server_name,
                remote_tool.name.clone(),
                remote_tool.description.clone(),
                remote_tool.input_schema.clone(),
            ),
            client,
            remote_name: remote_tool.name.clone(),
        }
    }
}

#[async_trait]
impl Tool for McpToolAdapter {
    fn metadata(&self) -> &ToolMetadata {
        &self.metadata
    }

    async fn call(&self, input: serde_json::Value) -> Result<ToolResult> {
        self.client.call_tool(&self.remote_name, input).await
    }
}

pub struct McpServerHandle {
    spec: McpServerSpec,
    client: Arc<McpClient>,
}

impl McpServerHandle {
    pub fn start(spec: McpServerSpec) -> Result<Self> {
        let transport: Arc<dyn McpTransport> = Arc::new(StdioMcpTransport::start(&spec)?);
        let client = Arc::new(McpClient::new(transport));

        Ok(Self { spec, client })
    }

    pub fn server_name(&self) -> &str {
        &self.spec.name
    }

    pub fn discover_tools(&self) -> Result<Vec<McpRemoteTool>> {
        block_on(self.client.list_tools())
    }

    pub fn register_tools(&self, registry: &mut ToolRegistry) -> Result<Vec<McpRegisteredTool>> {
        let tools = self.discover_tools()?;
        let mut registered = Vec::with_capacity(tools.len());

        for tool in tools {
            let registration = McpRegisteredTool::from_remote(&self.spec.name, &tool);
            registry.register(Arc::new(McpToolAdapter::new(
                &self.spec.name,
                &tool,
                self.client.clone(),
            )));
            registered.push(registration);
        }

        Ok(registered)
    }
}

#[derive(Default)]
pub struct McpServerManager {
    servers: HashMap<String, Arc<McpServerHandle>>,
}

impl McpServerManager {
    pub fn start(specs: &[McpServerSpec]) -> Result<Self> {
        let mut servers = HashMap::new();

        for spec in specs {
            if servers.contains_key(&spec.name) {
                bail!("duplicate MCP server name: {}", spec.name);
            }

            let handle = Arc::new(McpServerHandle::start(spec.clone())?);
            servers.insert(spec.name.clone(), handle);
        }

        Ok(Self { servers })
    }

    pub fn is_empty(&self) -> bool {
        self.servers.is_empty()
    }

    pub fn server_count(&self) -> usize {
        self.servers.len()
    }

    pub fn list_servers(&self) -> Vec<String> {
        let mut servers = self.servers.keys().cloned().collect::<Vec<_>>();
        servers.sort();
        servers
    }

    pub fn register_tools(&self, registry: &mut ToolRegistry) -> Result<Vec<McpRegisteredTool>> {
        let mut server_names = self.servers.keys().cloned().collect::<Vec<_>>();
        server_names.sort();

        let mut registered = Vec::new();
        for server_name in server_names {
            let handle = self
                .servers
                .get(&server_name)
                .expect("MCP server handle should exist for sorted key");
            registered.extend(handle.register_tools(registry)?);
        }

        Ok(registered)
    }
}

#[derive(Debug, Deserialize)]
struct JsonRpcResponse {
    id: u64,
    result: Option<serde_json::Value>,
    error: Option<JsonRpcError>,
}

#[derive(Debug, Deserialize)]
struct JsonRpcError {
    message: String,
}

fn parse_tool_list_result(value: serde_json::Value) -> Result<Vec<McpRemoteTool>> {
    let tools = match value {
        serde_json::Value::Array(tools) => tools,
        serde_json::Value::Object(mut object) => object
            .remove("tools")
            .and_then(|tools| tools.as_array().cloned())
            .ok_or_else(|| anyhow!("MCP tools/list response did not contain a tools array"))?,
        other => {
            bail!(
                "MCP tools/list response must be an array or object, got {}",
                other
            )
        }
    };

    tools
        .into_iter()
        .map(|tool| {
            let name = tool
                .get("name")
                .and_then(serde_json::Value::as_str)
                .ok_or_else(|| anyhow!("MCP tool descriptor is missing name"))?;
            let description = tool
                .get("description")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default();
            let input_schema = tool
                .get("inputSchema")
                .or_else(|| tool.get("input_schema"))
                .cloned()
                .unwrap_or_else(|| serde_json::json!({ "type": "object" }));

            Ok(McpRemoteTool {
                name: name.to_owned(),
                description: description.to_owned(),
                input_schema,
            })
        })
        .collect()
}

fn parse_tool_call_result(value: serde_json::Value) -> Result<ToolResult> {
    let content = value
        .get("content")
        .map(render_mcp_content)
        .transpose()?
        .filter(|text| !text.is_empty())
        .unwrap_or_else(|| value.to_string());
    let structured = value
        .get("structuredContent")
        .cloned()
        .or_else(|| value.get("structured_content").cloned())
        .or_else(|| Some(value.clone()));

    if value
        .get("isError")
        .or_else(|| value.get("is_error"))
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false)
    {
        bail!("MCP tool call returned an error: {}", content);
    }

    Ok(ToolResult {
        content,
        structured,
        is_error: false,
        audit: None,
    })
}

fn render_mcp_content(value: &serde_json::Value) -> Result<String> {
    match value {
        serde_json::Value::String(text) => Ok(text.clone()),
        serde_json::Value::Array(parts) => Ok(parts
            .iter()
            .filter_map(|part| match part {
                serde_json::Value::String(text) => Some(text.clone()),
                serde_json::Value::Object(map) => map
                    .get("text")
                    .and_then(serde_json::Value::as_str)
                    .map(str::to_owned),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n")),
        other => Ok(other.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use std::{
        fs, process,
        sync::Arc,
        time::{SystemTime, UNIX_EPOCH},
    };

    use futures::executor::block_on;
    use mosaic_tool_core::{ToolRegistry, ToolSource};

    use super::{McpClient, McpServerManager, McpServerSpec, McpToolAdapter, StdioMcpTransport};

    fn script_path() -> std::path::PathBuf {
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../scripts/mock_mcp_server.py")
    }

    fn temp_file_path(label: &str) -> std::path::PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock should be after epoch")
            .as_nanos();

        std::env::temp_dir().join(format!(
            "mosaic-mcp-core-{label}-{}-{nanos}.txt",
            process::id()
        ))
    }

    fn spec(mode: &str) -> McpServerSpec {
        McpServerSpec {
            name: mode.to_owned(),
            command: "python3".to_owned(),
            args: vec![script_path().display().to_string(), mode.to_owned()],
        }
    }

    #[test]
    fn list_tools_discovers_remote_stdio_tools() {
        let transport = Arc::new(
            StdioMcpTransport::start(&spec("filesystem"))
                .expect("filesystem transport should start"),
        );
        let client = McpClient::new(transport);

        let tools = block_on(client.list_tools()).expect("tool discovery should succeed");

        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].name, "read_file");
        assert_eq!(
            tools[0].description,
            "Read a UTF-8 text file from disk via MCP"
        );
    }

    #[test]
    fn call_tool_executes_remote_stdio_tool() {
        let path = temp_file_path("call-tool");
        fs::write(&path, "hello via mcp").expect("temp file should be writable");

        let transport = Arc::new(
            StdioMcpTransport::start(&spec("filesystem"))
                .expect("filesystem transport should start"),
        );
        let client = McpClient::new(transport);

        let result = block_on(client.call_tool(
            "read_file",
            serde_json::json!({ "path": path.to_string_lossy() }),
        ))
        .expect("remote tool call should succeed");

        assert_eq!(result.content, "hello via mcp");
        assert_eq!(
            result.structured,
            Some(serde_json::json!({
                "path": path.to_string_lossy().to_string(),
                "content": "hello via mcp",
            }))
        );

        fs::remove_file(path).ok();
    }

    #[test]
    fn server_manager_registers_mcp_tools_with_qualified_names() {
        let manager =
            McpServerManager::start(&[spec("filesystem")]).expect("MCP manager should start");
        let mut registry = ToolRegistry::new();

        let tools = manager
            .register_tools(&mut registry)
            .expect("MCP tool registration should succeed");
        let tool = registry
            .get("mcp.filesystem.read_file")
            .expect("qualified MCP tool should be registered");

        assert_eq!(manager.server_count(), 1);
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].qualified_name, "mcp.filesystem.read_file");
        assert_eq!(tool.metadata().source.label(), "mcp");
        assert_eq!(tool.metadata().source.server_name(), Some("filesystem"));
        assert_eq!(tool.metadata().source.remote_tool_name(), Some("read_file"));
    }

    #[test]
    fn invalid_server_response_is_reported() {
        let transport = Arc::new(
            StdioMcpTransport::start(&spec("invalid"))
                .expect("invalid-response transport should start"),
        );
        let client = McpClient::new(transport);

        let err = block_on(client.list_tools()).expect_err("invalid response should fail");

        assert!(err.to_string().contains("invalid JSON response"));
    }

    #[test]
    fn server_crash_is_reported() {
        let transport = Arc::new(
            StdioMcpTransport::start(&spec("crash")).expect("crash transport should start"),
        );
        let client = McpClient::new(transport);

        let err = block_on(client.list_tools()).expect_err("crashed server should fail");
        let message = err.to_string();

        assert!(
            message.contains("exited") || message.contains("closed stdout unexpectedly"),
            "unexpected crash message: {message}"
        );
    }

    #[test]
    fn adapter_calls_remote_tool_and_exposes_mcp_source() {
        let transport = Arc::new(
            StdioMcpTransport::start(&spec("filesystem"))
                .expect("filesystem transport should start"),
        );
        let client = Arc::new(McpClient::new(transport));
        let adapter = McpToolAdapter::new(
            "filesystem",
            &super::McpRemoteTool {
                name: "read_file".to_owned(),
                description: "Read a UTF-8 text file from disk via MCP".to_owned(),
                input_schema: serde_json::json!({ "type": "object" }),
            },
            client,
        );

        assert_eq!(
            adapter.metadata.source,
            ToolSource::Mcp {
                server: "filesystem".to_owned(),
                remote_tool: "read_file".to_owned(),
            }
        );
    }
}
