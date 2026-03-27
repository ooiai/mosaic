# mosaic-channel-telegram

`mosaic-channel-telegram` normalizes Telegram webhook payloads into Mosaic-ready ingress context.

## Positioning

This crate is a thin channel adapter for Telegram. It keeps Telegram-specific payload parsing out of gateway business logic.

## Architecture Layer

Interaction Entry Layer.

## Responsibilities

- Define Telegram webhook payload structs.
- Normalize message, thread, actor, and reply-target data through `normalize_update`.
- Produce `IngressTrace`-compatible context through `NormalizedTelegramMessage::ingress`.

## Out of Scope

- Gateway HTTP routing or shared-secret auth.
- Runtime orchestration.
- Outbound Telegram sending.
- TUI or CLI rendering.

## Public Boundary

- Types: `TelegramUpdate`, `TelegramMessage`, `TelegramChat`, `TelegramUser`, `NormalizedTelegramMessage`.
- Entry function: `normalize_update`.

## Why This Is In `crates/`

Telegram normalization is reusable by gateway ingress handlers, SDK tests, and future adapter work. It should stay a thin reusable adapter instead of being buried in gateway request handlers.

## Relationships

- Upstream crates: `mosaic-inspect` provides the ingress trace type used by normalized output.
- Downstream crates: `mosaic-gateway` calls `normalize_update`, while CLI and SDK tests can reuse the same normalization behavior.
- Runtime/control-plane coupling: `gateway` uses this crate before submitting a run to `mosaic-runtime`; the runtime should never parse raw Telegram payloads itself.

## Minimal Use

```rust
use mosaic_channel_telegram::normalize_update;

let normalized = normalize_update(update)?;
let ingress = normalized.ingress();
```

## Testing

```bash
cargo test -p mosaic-channel-telegram
```

## Current Limitations

- It only handles the currently supported inbound Telegram payload shapes.
- There is no outbound Telegram abstraction yet.
- Rich media handling is intentionally out of scope today.

## Roadmap

- Expand supported inbound Telegram payload patterns carefully.
- Keep the adapter thin and normalization-focused.
- Preserve a stable bridge from channel payloads into control-plane ingress traces.
