# mosaic-tui

`mosaic-tui` implements the chat-first terminal operator surface for Mosaic.

## Positioning

This crate owns terminal rendering, keyboard interaction, interactive session state, slash-command discoverability, inline capability event rendering, and event-buffer handling for the operator console.

## Architecture Layer

Control Plane Layer.

## Responsibilities

- Render and run the single-column chat shell through `run`, `run_with_event_buffer`, and `run_interactive_session`.
- Hold interactive operator state in `app` and render the transcript/composer/popup layout in `ui`.
- Bridge runtime events into the transcript through `TuiEventBuffer` and `TuiEventSink`.
- Support local and remote gateway-backed interactive sessions through `InteractiveGateway` and `InteractiveSessionContext`.
- Surface tools, skills, workflows, and provider/runtime progress inline in the conversation stream.

## Out of Scope

- CLI argument parsing and bootstrap assembly.
- Gateway server behavior.
- Runtime orchestration internals.
- Provider transport details.

## Public Boundary

- Modules: `app`, `mock`, `ui`.
- Types: `InteractiveGateway`, `TuiEventBuffer`, `TuiEventSink`, `InteractiveSessionContext`.
- Entry functions: `build_tui_event_buffer`, `build_tui_event_sink`, `run`, `run_with_event_buffer`, `run_until_complete_with_event_buffer`, `run_interactive_session`.

## Why This Is In `crates/`

The terminal operator console is a reusable surface that `cli` launches but should not implement inline. Keeping it in `crates/` makes the UI testable and keeps the binary focused on composition.

## Relationships

- Upstream crates: `mosaic-control-protocol`, `mosaic-runtime`, `mosaic-gateway`, `mosaic-sdk`, `mosaic-session-core`, and `mosaic-node-protocol`.
- Downstream crates: `cli` launches this crate as the operator console.
- Runtime/control-plane coupling: `gateway` and SDK provide session/run state, `runtime` emits events, and `cli` assembles the context. This crate should focus on UI behavior, not orchestration.

## Minimal Use

```rust
use mosaic_tui::run;

run(false)?;
```

For gateway-backed sessions, build an `InteractiveSessionContext` and call `run_interactive_session`.

## Testing

```bash
cargo test -p mosaic-tui
```

## Current Limitations

- The UI is terminal-only.
- Telegram remains the strongest release-grade real GUI acceptance lane while the TUI hardens.
- Command completion currently uses a popup plus `Tab` accept model; richer inline argument completion is still evolving.

## Roadmap

- Keep improving the operator console without pushing view logic back into `cli`.
- Strengthen remote attach, transcript inspection, and inline operator diagnostics.
- Preserve a clear split between UI state, rendering, gateway interaction, and bootstrap/composition.
