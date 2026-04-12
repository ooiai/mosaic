# mosaic-session-core

`mosaic-session-core` owns persisted conversation state for Mosaic sessions.

## Positioning

This crate defines what a session record looks like, how transcript messages are stored, and how sessions are listed and persisted. It is the session state boundary shared by runtime, gateway, CLI, and TUI flows.

## Architecture Layer

Control Plane Layer.

## Responsibilities

- Define `SessionRecord`, transcript types, gateway/channel metadata, run metadata, memory metadata, and cross-session references.
- Provide the `SessionStore` trait and `FileSessionStore`.
- Validate session ids and derive titles/routes through helpers such as `validate_session_id`, `session_title_from_input`, and `session_route_for_id`.
- Offer `SessionSummary` as the lightweight list/view model used by CLI, gateway, and TUI surfaces.

## Out of Scope

- Provider transport, prompt orchestration, or tool execution.
- Memory compression logic beyond storing derived fields.
- HTTP ingress or audit logging.
- TUI or CLI rendering.

## Public Boundary

- Types: `SessionRecord`, `SessionSummary`, `TranscriptMessage`, `TranscriptRole`, `SessionReference`.
- Metadata: `SessionGatewayMetadata`, `SessionChannelMetadata`, `SessionRunMetadata`, `SessionMemoryMetadata`.
- Store boundary: `SessionStore`, `FileSessionStore`.
- Helpers: `validate_session_id`, `session_title_from_input`, `session_route_for_id`.

## Why This Is In `crates/`

Sessions are shared state, not a CLI-specific implementation detail. `mosaic-runtime`, `mosaic-gateway`, `mosaic-tui`, and CLI session commands all rely on the same session model and storage contract.

## Relationships

- Upstream crates: `mosaic-inspect` contributes ingress and run lifecycle types embedded in session metadata.
- Downstream crates: `mosaic-runtime` reads and updates sessions during runs, `mosaic-gateway` supervises current run state and gateway metadata, `mosaic-tui` and CLI surfaces render summaries and transcripts.
- Runtime/control-plane coupling: `runtime` mutates the conversation state, `gateway` coordinates it across runs, and `cli`/`tui` inspect it. This crate should not decide when a run starts or which provider is used.

## Minimal Use

```rust
use mosaic_session_core::{FileSessionStore, SessionStore, SessionRecord};

let store = FileSessionStore::new(".mosaic/sessions");
let session = SessionRecord::new(
    "demo",
    "Demo",
    "demo-provider",
    "openai",
    "gpt-5.4-mini",
);
store.save(&session)?;
let loaded = store.load("demo")?;
```

## Testing

```bash
cargo test -p mosaic-session-core
```

Tests cover file roundtrips, summaries, id validation, and legacy gateway-route backfill.

## Current Limitations

- Persistence is local file storage only.
- Session indexing is intentionally simple and optimized for workspace-local operation.
- Concurrent multi-process coordination relies on higher layers behaving carefully around file updates.

## Roadmap

- Add stronger operational tooling for larger session stores without changing the core model.
- Keep the session schema explicit as more channel and run metadata is added.
- Preserve a narrow store contract so alternative backends can be added later.
