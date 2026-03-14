---
name: mosaic-tui-repair
description: Use when changing `mosaic tui`, ratatui or crossterm behavior, key handling, slash commands, session or agent pickers, inspector rendering, or refactoring the TUI structure.
---

# Mosaic TUI repair

Use this skill for fullscreen TUI bugs, refactors, or UX changes.

## Files to read first

1. `AGENT.md`
2. `cli/crates/mosaic-cli/src/tui_command.rs`
3. `cli/crates/mosaic-tui/src/lib.rs`
4. `cli/crates/mosaic-cli/src/runtime_context.rs`
5. `cli/crates/mosaic-core/src/session.rs`
6. `cli/crates/mosaic-core/src/state.rs`

## Current architecture

- `mosaic-cli` owns TUI command parsing and mode selection.
- `handle_tui` splits interactive fullscreen mode from non-interactive `--prompt` mode.
- `mosaic-tui/src/lib.rs` currently contains most state, input handling, async event flow, and rendering in a single file.
- Runtime/session rebinding should continue to flow through `build_runtime_from_selector`.

## Behavior that must stay intact

- `mosaic tui --prompt ...` must keep working as a non-interactive path.
- `--json` is only valid for non-interactive TUI mode.
- `--project-state` must continue reading/writing `.mosaic/`.
- Session resume must preserve session-bound runtime metadata.
- Agent switching currently resets the session when there is existing history.
- The status line should continue surfacing profile, agent, session, and policy context.

## Recommended refactor order

If the task is structural, prefer extraction before UX changes:

1. Split state/events/commands/rendering out of `cli/crates/mosaic-tui/src/lib.rs`.
2. Keep behavior stable while moving code.
3. Only after extraction, change layouts, overlays, scrolling, or keybindings.

Good initial split candidates:

- `events.rs`
- `commands.rs`
- `pickers.rs`
- `render.rs`
- `state.rs`

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
