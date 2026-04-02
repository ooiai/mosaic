# mosaic-channel-telegram

`mosaic-channel-telegram` normalizes Telegram webhook payloads into Mosaic-ready ingress context and sends Telegram-native outbound replies, including command keyboards.

## Positioning

This crate is a thin channel adapter for Telegram. It keeps Telegram-specific payload parsing out of gateway business logic.

## Architecture Layer

Interaction Entry Layer.

## Responsibilities

- Define Telegram webhook payload structs.
- Normalize message, thread, actor, and reply-target data through `normalize_update`.
- Send Telegram outbound replies through `TelegramOutboundClient`.
- Map channel quick-reply markup into Telegram reply keyboards.

## Out of Scope

- Gateway HTTP routing or shared-secret auth.
- Runtime orchestration.
- TUI or CLI rendering.

## Public Boundary

- Types: `TelegramUpdate`, `TelegramMessage`, `TelegramChat`, `TelegramUser`, `TelegramOutboundClient`.
- Entry functions: `normalize_update`, `normalize_update_with_context`.

## Why This Is In `crates/`

Telegram normalization is reusable by gateway ingress handlers, SDK tests, and future adapter work. It should stay a thin reusable adapter instead of being buried in gateway request handlers.

## Relationships

- Upstream crates: `mosaic-inspect` provides the ingress trace type used by normalized output.
- Downstream crates: `mosaic-gateway` calls `normalize_update` and uses `TelegramOutboundClient` for outbound replies, while CLI and SDK tests can reuse the same adapter behavior.
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
- Rich media handling is intentionally narrow and focused on the current image/document lanes.
- Callback-query and inline-keyboard flows are not implemented yet; Telegram command discovery currently uses reply-keyboard shortcuts plus text commands.

## Roadmap

- Expand supported inbound Telegram payload patterns carefully.
- Keep outbound reply semantics and keyboard rendering compatible.
- Keep the adapter thin and normalization-focused.
- Preserve a stable bridge from channel payloads into control-plane ingress traces.
