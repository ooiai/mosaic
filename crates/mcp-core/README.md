# mosaic-mcp-core

`mosaic-mcp-core` integrates Model Context Protocol servers into Mosaic tool registration and execution.

## Positioning

This crate is the MCP adapter boundary. It starts stdio MCP servers, discovers remote tools, and exposes them to the rest of Mosaic as normal tool registrations.

## Architecture Layer

Capability Execution Layer.

## Responsibilities

- Define the transport contract through `McpTransport`.
- Describe MCP servers and remote tools with `McpServerSpec`, `McpRemoteTool`, and `McpRegisteredTool`.
- Manage stdio servers through `McpServerManager` and `McpServerHandle`.
- Wrap remote MCP tools in `McpToolAdapter` so runtime can call them through `mosaic-tool-core`.

## Out of Scope

- General tool policy outside MCP integration.
- Gateway auth or HTTP transport.
- Session persistence or memory policy.
- Provider scheduling.

## Public Boundary

- Transport and client: `McpTransport`, `McpClient`, `StdioMcpTransport`.
- Server/model types: `McpServerSpec`, `McpRemoteTool`, `McpRegisteredTool`, `McpServerHandle`, `McpServerManager`.
- Tool adapter: `McpToolAdapter`.

## Why This Is In `crates/`

MCP integration is shared by extension loading, gateway startup, runtime tool execution, and tests. It is a reusable execution adapter, not a CLI-only concern.

## Relationships

- Upstream crates: `mosaic-tool-core` provides the tool contract MCP tools register into.
- Downstream crates: `mosaic-gateway` owns MCP server lifecycle, `mosaic-extension-core` can describe MCP servers from extensions, and `mosaic-runtime` executes the registered tool adapters.
- Runtime/control-plane coupling: `gateway` starts and supervises MCP servers, while `runtime` treats discovered tools like any other callable tool.

## Minimal Use

```rust
use mosaic_mcp_core::{McpServerManager, McpServerSpec};
use mosaic_tool_core::ToolRegistry;

let specs = vec![McpServerSpec {
    name: "filesystem".to_owned(),
    command: "python3".to_owned(),
    args: vec!["scripts/mock_mcp_server.py".to_owned()],
}];
let manager = McpServerManager::start(&specs)?;
let mut tools = ToolRegistry::new();
let registered = manager.register_tools(&mut tools)?;
```

## Testing

```bash
cargo test -p mosaic-mcp-core
```

## Current Limitations

- The implementation is stdio-oriented today.
- Server lifecycle and error recovery are intentionally simple.
- MCP capability policy still depends on higher layers honoring tool metadata.

## Roadmap

- Expand transport options while keeping the tool-facing contract stable.
- Harden server supervision and diagnostics.
- Keep MCP registration aligned with extension and gateway lifecycle expectations.
