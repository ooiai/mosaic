# mosaic-scheduler-core

`mosaic-scheduler-core` stores cron-style scheduled run registrations for Mosaic.

## Positioning

This crate is the small scheduling persistence boundary used by cron-related capability flows.

## Architecture Layer

Capability Execution Layer.

## Responsibilities

- Define `CronRegistration`.
- Define the `CronStore` trait.
- Provide `FileCronStore` as the current file-backed implementation.

## Out of Scope

- Actually triggering runs over time.
- Runtime orchestration, provider transport, or gateway HTTP behavior.
- TUI and CLI rendering.

## Public Boundary

- Types: `CronRegistration`.
- Store boundary: `CronStore`, `FileCronStore`.

## Why This Is In `crates/`

Cron registrations are shared by tool execution, gateway cron endpoints, CLI cron commands, and tests. The storage contract is reusable, so it belongs in `crates/`.

## Relationships

- Upstream crates: none beyond workspace primitives.
- Downstream crates: `mosaic-tool-core` uses cron registration tools, `mosaic-gateway` exposes cron APIs and triggers, and `cli` surfaces cron management commands.
- Runtime/control-plane coupling: `gateway` decides when a registration becomes a run, while this crate only stores registration state.

## Minimal Use

```rust
use mosaic_scheduler_core::{CronStore, FileCronStore};

let store = FileCronStore::new(".mosaic/cron");
let registrations = store.list()?;
```

## Testing

```bash
cargo test -p mosaic-scheduler-core
```

## Current Limitations

- File-backed only.
- Schedule parsing/validation is intentionally narrow.
- Trigger orchestration is delegated to callers.

## Roadmap

- Keep the store contract small so alternate backends remain possible.
- Improve schedule validation and metadata without bloating the crate.
- Stay focused on persistence, not orchestration.
