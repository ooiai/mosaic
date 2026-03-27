# mosaic-tool-core

`mosaic-tool-core` defines the capability execution contract for Mosaic tools and ships the built-in tool set used by the runtime.

## Positioning

This crate is where Mosaic describes what a tool is, how it is registered, how its risk and source are reported, and how built-in tool implementations behave.

## Architecture Layer

Capability Execution Layer.

## Responsibilities

- Define the `Tool` trait and `ToolResult`.
- Carry tool metadata, capability metadata, permission scopes, risk level, and source attribution.
- Provide `ToolRegistry` for runtime/bootstrap composition.
- Implement built-in tools such as `EchoTool`, `ReadFileTool`, `TimeNowTool`, `ExecTool`, `WebhookTool`, and `CronRegisterTool`.
- Preserve metadata that lets runtime, inspect, provider policy, node routing, and MCP integration reason about tools consistently.

## Out of Scope

- Provider scheduling or prompt orchestration.
- Session ownership, transcript persistence, or memory compression.
- Gateway ingress and operator auth.
- Remote MCP server lifecycle or device-node transport protocols.

## Public Boundary

- Traits and results: `Tool`, `ToolResult`.
- Registry: `ToolRegistry`.
- Metadata surface: `ToolMetadata`, `CapabilityMetadata`, `CapabilityKind`, `PermissionScope`, `ToolRiskLevel`.
- Source attribution: `ToolSource`, `mcp_tool_name`.
- Built-ins: `EchoTool`, `ReadFileTool`, `TimeNowTool`, `ExecTool`, `WebhookTool`, `CronRegisterTool`.

## Why This Is In `crates/`

Tools are used by multiple paths: runtime execution, provider tool exposure policy, inspect traces, gateway capability jobs, node routing, and bootstrap. That makes `tool-core` a stable shared module instead of a CLI-only implementation detail.

## Relationships

- Upstream crates: `mosaic-scheduler-core` backs cron registration for the cron tool.
- Downstream crates: `mosaic-runtime` executes tools, `mosaic-provider` translates visible tool metadata into model-facing definitions, `mosaic-gateway` wraps side effects into control-plane jobs, and `cli` bootstraps the default tool set.
- Runtime/control-plane coupling: `runtime` should orchestrate tool usage, `gateway` should supervise side effects, and `cli` should only compose or inspect tools. This crate should not decide when a tool is called.

## Minimal Use

```rust
use std::sync::Arc;
use mosaic_tool_core::{EchoTool, ToolRegistry};

let mut tools = ToolRegistry::new();
tools.register(Arc::new(EchoTool));
let echo = tools.get("echo").expect("echo tool should exist");
```

## Testing

```bash
cargo test -p mosaic-tool-core
```

The tests cover built-in tools, source attribution, permission metadata, and registry behavior.

## Current Limitations

- Tool execution is still synchronous at the trait boundary from the runtime perspective; streaming tool output is not modeled.
- Built-ins cover the current platform baseline but not the full long-term capability surface described in `AGENTS.md`.
- Policy metadata is descriptive; enforcement still depends on runtime and gateway callers honoring it.

## Roadmap

- Add richer side-effect and interruption metadata for long-running tools.
- Keep expanding built-in tool coverage while preserving a stable contract.
- Harden policy helpers so remote capability adapters can share a common enforcement layer.
