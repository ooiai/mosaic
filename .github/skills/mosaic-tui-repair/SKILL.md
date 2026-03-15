---
name: mosaic-tui-repair
description: Use when changing `mosaic tui`, ratatui or crossterm behavior, key handling, slash commands, session or agent pickers, inspector rendering, or refactoring the TUI structure.
---

# Mosaic TUI repair

Use this skill for fullscreen TUI bugs, refactors, or UX changes.

## Files to read first

1. `AGENT.md`
2. `specs/cli/assets/copolit_startup.png` (for startup-screen alignment work)
3. `cli/crates/mosaic-cli/src/tui_command.rs`
4. `cli/crates/mosaic-tui/src/lib.rs`
5. `cli/crates/mosaic-tui/src/commands.rs`
6. `cli/crates/mosaic-tui/src/events.rs`
7. `cli/crates/mosaic-tui/src/keys.rs`
8. `cli/crates/mosaic-tui/src/render.rs`
9. `cli/crates/mosaic-tui/src/state.rs`
10. `cli/crates/mosaic-tui/src/pickers.rs`
11. `cli/crates/mosaic-cli/src/runtime_context.rs`
12. `cli/crates/mosaic-core/src/session.rs`
13. `cli/crates/mosaic-core/src/state.rs`

## Current architecture

- `mosaic-cli` owns TUI command parsing and mode selection.
- `handle_tui` splits interactive fullscreen mode from non-interactive `--prompt` mode.
- `mosaic-tui/src/lib.rs` still owns the main loop and task spawning.
- `mosaic-tui/src/events.rs` owns the app-event bridge.
- `mosaic-tui/src/commands.rs` owns slash-command parsing.
- `mosaic-tui/src/keys.rs` owns raw keyboard dispatch and shortcut routing.
- `mosaic-tui/src/render.rs` owns layout, overlays, inspector/session/agent picker rendering, and status-line composition.
- `mosaic-tui/src/render.rs` also owns the single-canvas Copilot-style layout, the startup screen aligned to `specs/cli/assets/copolit_startup.png`, the bottom composer/footer, slash-command suggestion popup, and modal pickers/overlays.
- `mosaic-tui/src/state.rs` owns `TuiState`, reducers, session replay, and agent-event application.
- `mosaic-tui/src/pickers.rs` owns input-command handling plus session/agent picker and switching helpers.
- Runtime/session rebinding should continue to flow through `build_runtime_from_selector`.

## Behavior that must stay intact

- `mosaic tui --prompt ...` must keep working as a non-interactive path.
- `--json` is only valid for non-interactive TUI mode.
- `--project-state` must continue reading/writing `.mosaic/`.
- Session resume must preserve session-bound runtime metadata.
- Startup-screen work may still resume the latest session under the hood, but the initial fullscreen surface can intentionally stay on the startup view until the user switches focus or starts a turn.
- Agent switching currently resets the session when there is existing history.
- The status line should continue surfacing profile, agent, session, and policy context.

## Recommended refactor order

If the task is structural, prefer extraction before UX changes:

1. Keep `events.rs`, `commands.rs`, `keys.rs`, and `render.rs` as their dedicated homes.
2. Keep `state.rs` and `pickers.rs` as their dedicated homes too.
3. Keep behavior stable while moving code.
4. Only after extraction, change layouts, overlays, scrolling, or keybindings.

Good next split candidates:

- `widgets.rs`
- render-only helpers shared across overlays/widgets
- task-spawn helpers if `lib.rs` grows again

## Codex reference

Use `https://github.com/openai/codex/tree/main/codex-rs/tui` as a modularization reference, not as a behavior blueprint.

Useful ideas to borrow:

- keep `lib.rs` as orchestration
- isolate event types and event senders
- isolate pickers and rendering widgets
- add reusable render/test helpers

Do not copy Codex runtime assumptions into Mosaic; reuse Mosaic session/state/runtime code.

## Validation

Run focused checks first:

```bash
cd cli
cargo test -p mosaic-cli --test tui_ops
cargo test -p mosaic-cli --test tui_interactive
cargo test -p mosaic-tui
```

Then, if the change is broader:

```bash
cd cli
cargo test -p mosaic-cli
```
