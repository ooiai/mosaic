---
name: mosaic-project-state-debugging
description: Use when debugging `.mosaic`, `--project-state`, session routing, config loading, XDG versus project-local state, or state-dependent ask, chat, or tui bugs.
---

# Mosaic project-state debugging

Use this skill for bugs that only reproduce with `--project-state` or depend on persisted repository state.

## Read these files first

1. `AGENT.md`
2. `cli/crates/mosaic-core/src/state.rs`
3. `cli/crates/mosaic-core/src/session.rs`
4. `cli/crates/mosaic-core/src/config.rs`
5. `cli/crates/mosaic-cli/src/runtime_context.rs`
6. Relevant command handler in `cli/crates/mosaic-cli/src/*_command.rs`

## Key facts

- Default mode stores config under XDG paths such as `~/.config/mosaic/` and data under `~/.local/share/mosaic/`.
- Project mode stores everything under `./.mosaic/`.
- Session events are persisted as JSONL under `.mosaic/data/sessions/`.
- Runtime metadata in sessions is used for agent/profile rebinding when resuming work.

## Debugging workflow

1. Inspect `.mosaic/config.toml` and any relevant `.mosaic/policy/*.toml`.
2. Inspect `.mosaic/data/sessions/*.jsonl` when session behavior differs from expectation.
3. Check whether the bug happens in project mode only or also in XDG mode.
4. Trace runtime construction through `build_runtime` / `build_runtime_from_selector`.
5. Preserve the existing directory layout and file shapes unless the task explicitly requires a migration.

## Common pitfalls

- Breaking the distinction between XDG mode and project mode.
- Forgetting that resumed sessions may carry agent/profile metadata.
- Fixing the top-level CLI parser when the real bug is in state loading or runtime rebinding.
- Changing state filenames or directories without auditing all readers.

## Validation ideas

Use real repro commands from the repo root whenever possible, for example:

```bash
cargo run --manifest-path cli/Cargo.toml -p mosaic-cli -- --project-state ask "summarize this repository"
cargo run --manifest-path cli/Cargo.toml -p mosaic-cli -- --project-state tui --prompt "hello"
```

Then add focused tests in `cli/` if the bug touches config, session routing, or command behavior.
