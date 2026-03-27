# mosaic-control-protocol

`mosaic-control-protocol` defines the HTTP and SSE payload contract for the Mosaic control plane.

## Positioning

This crate is the DTO and event schema layer shared between `mosaic-gateway`, `mosaic-sdk`, CLI attach flows, and TUI remote mode.

## Architecture Layer

Control Plane Layer.

## Responsibilities

- Define DTOs for health, readiness, metrics, runs, sessions, adapters, cron, capability jobs, incidents, and errors.
- Define submission payloads such as `RunSubmission`, `InboundMessage`, `ExecJobRequest`, and `WebhookJobRequest`.
- Define the event stream contract through `GatewayEvent` and `EventStreamEnvelope`.
- Re-export ingress context through `IngressTrace`.

## Out of Scope

- HTTP routing or server behavior.
- SDK request execution.
- Runtime orchestration or provider transport.
- CLI/TUI presentation.

## Public Boundary

- Health/ops DTOs: `HealthResponse`, `ReadinessResponse`, `MetricsResponse`, `GatewayAuditEventDto`, `ReplayWindowResponse`, `IncidentBundleDto`.
- Submission DTOs: `RunSubmission`, `InboundMessage`, `ExecJobRequest`, `WebhookJobRequest`, `CronRegistrationRequest`.
- Run/session DTOs: `RunResponse`, `RunSummaryDto`, `RunDetailDto`, `SessionSummaryDto`, `SessionDetailDto`, `SessionRunDto`, `SessionChannelDto`, `TranscriptMessageDto`.
- Event contract: `GatewayEvent`, `EventStreamEnvelope`.

## Why This Is In `crates/`

The protocol is shared by server, SDK, CLI, TUI, and tests. Keeping it in `crates/` prevents each surface from redefining control-plane payloads independently.

## Relationships

- Upstream crates: `mosaic-inspect` and `mosaic-runtime` provide reused trace and event types embedded in protocol messages.
- Downstream crates: `mosaic-gateway` serves these payloads, `mosaic-sdk` consumes them, and `cli`/`mosaic-tui` attach through them.
- Runtime/control-plane coupling: this crate should reflect the control-plane contract, not runtime implementation details.

## Minimal Use

```rust
use mosaic_control_protocol::RunSubmission;

let body = RunSubmission {
    system: None,
    input: "hello".to_owned(),
    skill: None,
    workflow: None,
    session_id: Some("demo".to_owned()),
    profile: None,
    ingress: None,
};
let json = serde_json::to_string(&body)?;
```

## Testing

```bash
cargo test -p mosaic-control-protocol
```

## Current Limitations

- The protocol is tightly aligned with the current Gateway surface.
- Version negotiation is still implicit through workspace release cadence.
- DTOs are intentionally verbose rather than deeply normalized.

## Roadmap

- Preserve backwards-compatible DTO evolution as gateway features expand.
- Add clearer versioning guidance once multiple remote clients exist.
- Keep protocol types thin and serialization-focused.
