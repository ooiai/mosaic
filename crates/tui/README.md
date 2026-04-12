# mosaic-tui

`mosaic-tui` implements the chat-first single-shell terminal operator surface for Mosaic.

## Positioning

This crate owns terminal rendering, keyboard interaction, interactive session state, slash-command discoverability, inline capability event rendering, and event-buffer handling for the operator shell.

Product role:

- TUI = primary local operator shell
- Telegram = primary external human-facing channel lane
- CLI = scripted/operator automation surface

The current target is Codex-style local operator UX parity: dynamic turns, bottom-pane control, slash-driven interaction, and release-quality local acceptance guidance.
That parity target is only considered closed when a real PTY/operator acceptance run proves startup input, slash popup, direct chat, capability-backed execution, detail inspection, and retry/cancel behavior in the shell itself.

## Architecture Layer

Control Plane Layer.

## Responsibilities

- Render and run the single-column chat shell through `run`, `run_with_event_buffer`, and `run_interactive_session`.
- Hold interactive operator state in `app` and render the transcript/composer/popup layout in `ui`.
- Keep all operator actions inside the transcript rather than exposing persistent session/model/inspect panes.
- Bridge runtime events into the transcript through `TuiEventBuffer` and `TuiEventSink`.
- Support local and remote gateway-backed interactive sessions through `InteractiveGateway` and `InteractiveSessionContext`.
- Surface tools, skills, workflows, and provider/runtime progress inline in the conversation stream.
- Default capability feedback to collapsed execution cards and transcript-native failure guidance.
- Keep one active assistant turn mutable so streaming output, provider/tool/MCP/skill/workflow progress, sandbox preparation, and terminal failure/completion all attach to the same transcript cell.
- Keep the in-progress assistant turn outside committed history so the main transcript and transcript overlay can render a stable history plus one live active cell, closer to Codex's history-cell model.
- Keep nested execution evidence in a dedicated turn-detail overlay so the main transcript stays compact while `Ctrl+O` still reveals the full attached turn context.
- Provide a transcript overlay for full-shell history inspection so the compact local shell can stay dense without losing access to the full conversation buffer.
- Treat bare slash commands such as `/help`, `/new`, `/session ...`, and `/model ...` as the canonical local command contract while preserving `/mosaic ...` as a compatibility alias.

## Out of Scope

- CLI argument parsing and bootstrap assembly.
- Gateway server behavior.
- Runtime orchestration internals.
- Provider transport details.

## Public Boundary

- Modules: `app`, `bottom_pane`, `chat_widget`, `command_popup`, `composer`, `mock`, `overlays`, `status_bar`, `transcript`, `ui`.
- Types: `InteractiveGateway`, `TuiEventBuffer`, `TuiEventSink`, `InteractiveSessionContext`.
- Entry functions: `build_tui_event_buffer`, `build_tui_event_sink`, `run`, `run_with_event_buffer`, `run_until_complete_with_event_buffer`, `run_interactive_session`.

## Internal Shell Architecture

The local shell is now split so future Codex-style parity work has stable boundaries:

- `transcript`: transcript cells and scroll state
- `bottom_pane`: composer-adjacent state such as mode and command selection
- `overlays`: transient shell overlays such as the slash popup and turn-detail modal
- `chat_widget`: transcript rendering
- `composer`: bottom input rendering
- `status_bar`: compact header rendering

This keeps background session refresh separate from composer draft ownership and makes later dynamic-turn UX work safer.
The current UX goal is also denser shell chrome: compact header treatment, bottom-pane-centered command discovery, explicit task-running plus busy / send-disabled state, and transcript-first readability.
The current transcript goal is Codex-style dynamic turn evolution with `Ctrl+O` turn-detail overlays instead of append-only event spam.
The shell now has an explicit internal mode model (`idle`, `composing`, `command`, `running`, `transcript`, `detail`) so header/composer rendering and key handling can evolve around a real shell state instead of scattered booleans.

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

For local release-oriented proof, pair those tests with the PTY/manual acceptance guidance in [`docs/tui.md`](../../docs/tui.md): immediate typing, slash popup, one full streaming turn, one capability-backed turn, inline detail reveal, cancel/retry, and draft preservation.
If the local workspace cannot produce a successful provider-backed turn, record that limitation explicitly; startup and popup proof alone do not satisfy release-grade local shell acceptance.

## Current Limitations

- The UI is terminal-only.
- Telegram remains the strongest release-grade real GUI acceptance lane while the TUI hardens.
- Command completion currently uses a popup plus `Tab` accept model; richer inline argument completion is still evolving.
- Command discovery is canonical bare slash in the TUI; `/mosaic ...` remains a compatibility shortcut rather than the primary documented contract.

## Roadmap

- Keep improving the operator console without pushing view logic back into `cli`.
- Strengthen remote attach, transcript inspection, and inline operator diagnostics.
- Preserve a clear split between UI state, rendering, gateway interaction, and bootstrap/composition.
