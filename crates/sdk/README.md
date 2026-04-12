# mosaic-sdk

`mosaic-sdk` is the Rust client for talking to a Gateway over HTTP and SSE.

## Positioning

This crate gives code outside the Gateway an API client boundary instead of forcing each consumer to hand-roll HTTP requests and SSE parsing.

## Architecture Layer

Control Plane Layer.

## Responsibilities

- Expose `GatewayClient` for health, runs, sessions, adapters, capability jobs, cron, ingress, replay, and incident APIs.
- Expose `GatewayEventStream` for SSE event subscription.
- Reuse `mosaic-control-protocol` DTOs so client and server stay aligned.

## Out of Scope

- Running the Gateway server.
- Runtime orchestration.
- TUI rendering or CLI argument parsing.
- Provider transport logic.

## Public Boundary

- Client types: `GatewayClient`, `GatewayEventStream`.

## Why This Is In `crates/`

Remote Gateway access is shared by CLI attach flows, TUI remote mode, external integrations, and tests. That makes it a reusable control-plane boundary rather than a binary-only helper.

## Relationships

- Upstream crates: `mosaic-control-protocol` provides DTOs; `mosaic-channel-telegram` provides Telegram ingress payload types.
- Downstream crates: `cli` and `mosaic-tui` can attach to a remote gateway through this crate; external code can automate the same control surface.
- Runtime/control-plane coupling: this crate only talks to the control plane. It should not know about runtime internals beyond the DTO contract.

## Minimal Use

```rust
use mosaic_sdk::GatewayClient;

let client = GatewayClient::new("http://127.0.0.1:8080");
let health = client.health().await?;
let sessions = client.list_sessions().await?;
```

## Testing

```bash
cargo test -p mosaic-sdk
```

## Current Limitations

- The client is Rust-only; other languages would need their own SDK.
- Retry and reconnection policy are intentionally light.
- SSE support is event-envelope oriented rather than a full reactive abstraction.

## Roadmap

- Add more ergonomic helpers for long-lived remote operator flows.
- Expand auth and retry ergonomics as gateway deployments harden.
- Keep the SDK thin so the control protocol remains the real contract.
