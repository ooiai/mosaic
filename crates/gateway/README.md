# mosaic-gateway

`mosaic-gateway` is the long-running control-plane hub for Mosaic. It supervises ingress, session routing, run state, audit/replay, capability jobs, and HTTP/SSE exposure around the runtime.

## Positioning

This crate is the control-plane coordinator. It owns the long-lived service semantics that the runtime should not absorb: routing, audit, replay, adapter status, HTTP handlers, and operator-facing run/session state.

## Architecture Layer

Control Plane Layer.

## Responsibilities

- Hold the local gateway state through `GatewayHandle` and `GatewayRuntimeComponents`.
- Accept run submissions, supervise run lifecycle, and persist run/session metadata.
- Broadcast gateway events and maintain replay/audit state.
- Expose HTTP and SSE entrypoints through `http_router`, `serve_http`, and `serve_http_with_shutdown`.
- Normalize multi-channel ingress into runtime submissions.
- Surface extension reload, capability jobs, cron triggers, node routing state, and incident bundles.

## Out of Scope

- Prompt orchestration or tool loop internals.
- Vendor-specific provider transport logic.
- CLI argument parsing or TUI rendering.
- Direct device execution beyond delegating to capability, MCP, and node boundaries.

## Public Boundary

- Core gateway types: `GatewayHandle`, `GatewayRuntimeComponents`, `GatewayRunResult`, `GatewayRunError`, `GatewaySubmittedRun`.
- Reload and command types: `GatewayReloadSource`, `GatewayExtensionReloadResult`, `GatewayCommand`, `GatewayRunRequest`, `GatewayEventEnvelope`.
- HTTP entrypoints: `http_router`, `serve_http`, `serve_http_with_shutdown`.
- DTO helpers re-exported for consumers: `session_summary_dto`, `session_detail_dto`, `run_response`, `cron_registration_dto`.

## Why This Is In `crates/`

Gateway behavior is shared by local CLI flows, remote SDK clients, TUI attach mode, ingress adapters, and tests. It is the control-plane center of the product, so it must stay reusable and testable outside the CLI binary.

## Relationships

- Upstream crates: `mosaic-runtime` performs the actual run execution; `mosaic-config`, `mosaic-control-protocol`, `mosaic-session-core`, `mosaic-memory`, `mosaic-provider`, `mosaic-tool-core`, `mosaic-skill-core`, `mosaic-workflow`, `mosaic-node-protocol`, `mosaic-extension-core`, and `mosaic-channel-telegram` feed or depend on gateway state.
- Downstream crates: `cli` hosts local and HTTP gateway commands, `mosaic-sdk` calls the HTTP/SSE surface, and `mosaic-tui` can attach to gateway-backed sessions.
- Runtime/control-plane coupling: `runtime` should stay stateless per run, while `gateway` owns submission, replay, and cross-run coordination. `cli` should compose or operate it, not duplicate it.

## Minimal Use

```rust
use tokio::runtime::Handle;
use mosaic_gateway::{GatewayHandle, http_router};

let gateway = GatewayHandle::new_local(Handle::current(), components);
let app = http_router(gateway.clone());
let health = gateway.health();
```

`components` is the fully wired `GatewayRuntimeComponents` assembled during bootstrap.

## Testing

```bash
cargo test -p mosaic-gateway
```

Tests cover local gateway runs, session metadata, extension reload, node affinity, HTTP health/session/run handlers, and SSE event streaming.

## Current Limitations

- Local gateway mode is still workspace-scoped and in-process rather than a full multi-node control plane.
- HTTP auth and audit are intentionally simple compared with a production external control service.
- Adapter coverage is still limited to current ingress implementations.

## Roadmap

- Keep strengthening remote operator and multi-process gateway behavior.
- Expand adapter coverage and gateway-side control surfaces without bloating runtime code.
- Harden incident, replay, and operational policy boundaries for larger deployments.
