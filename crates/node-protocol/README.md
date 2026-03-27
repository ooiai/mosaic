# mosaic-node-protocol

`mosaic-node-protocol` defines the file-backed device node contract used by Mosaic for remote/local execution routing.

## Positioning

This crate is the node transport and affinity boundary. It lets gateway and runtime reason about external execution nodes without pushing node-specific state into the core runtime loop.

## Architecture Layer

Device Node Layer.

## Responsibilities

- Define node health, capability declarations, registration payloads, command dispatch envelopes, results, and affinity records.
- Define the `NodeRouter` trait.
- Provide `FileNodeStore` as the current workspace-local node registry and dispatch store.
- Surface node-based tool execution results through `NodeToolDispatchOutcome`.

## Out of Scope

- Actual tool logic for commands like `read_file` or `exec_command`.
- Gateway HTTP ingress or SDK transport.
- Runtime planning beyond routing a tool call to a node.
- TUI rendering.

## Public Boundary

- Node state: `NodeHealth`, `NodeCapabilityDeclaration`, `NodeRegistration`, `NodeAffinityRecord`, `NodeSelection`.
- Dispatch/result types: `NodeCommandDispatch`, `NodeCommandResultEnvelope`, `NodeToolExecutionRequest`, `NodeToolExecutionResult`, `NodeToolExecutionError`, `NodeToolDispatchOutcome`.
- Store/router boundary: `NodeRouter`, `FileNodeStore`.

## Why This Is In `crates/`

Node semantics are shared by gateway, runtime, CLI node commands, and tests. They are a stable protocol boundary and should not be hidden inside gateway or CLI code.

## Relationships

- Upstream crates: `mosaic-tool-core` contributes tool metadata that nodes may advertise or execute.
- Downstream crates: `mosaic-gateway` owns node lifecycle and affinity; `mosaic-runtime` consults the router for tool execution; CLI node commands operate directly on this store.
- Runtime/control-plane coupling: `gateway` manages nodes and `runtime` routes to them. This crate should not orchestrate runs or decide which provider to use.

## Minimal Use

```rust
use mosaic_node_protocol::FileNodeStore;

let store = FileNodeStore::new(".mosaic/nodes");
let nodes = store.list_nodes()?;
```

## Testing

```bash
cargo test -p mosaic-node-protocol
```

## Current Limitations

- The current node store is file-backed and workspace-local.
- Dispatch is intentionally simple and not designed as a full distributed queue.
- Capability negotiation is explicit but still minimal.

## Roadmap

- Strengthen reconnect, staleness, and affinity handling for long-lived deployments.
- Add transport options beyond the current file-bus approach.
- Keep the router contract stable as device capability breadth expands.
