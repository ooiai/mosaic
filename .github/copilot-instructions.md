# Copilot instructions for `mosaic`

This repository is a Rust-first local agent runtime. The main implementation surface is the CLI workspace under `cli/`.

## Where to start

- Read `AGENT.md` first for the current repository handoff summary.
- Read `README.md` for product-level context.
- For CLI, runtime, agent, or TUI work, switch into `cli/` and use `cli/README.md` as the deeper reference.

## Custom repository agents

This repository now has two custom agents under `.github/agents/`:

- `mosaic-general`
  - Broad default for repository orientation, cross-surface changes, docs work, and first-pass task triage.
  - Start here if the task scope is still unclear or clearly touches more than one top-level surface.
- `miagent`
  - Specialist for `cli/`, `mosaic tui`, runtime/state internals, `--project-state`, `.mosaic`, and Copilot asset maintenance tied to those systems.
  - Switch here once the task narrows to deep CLI/TUI behavior or state-routing issues.

If you are unsure, start with `mosaic-general` and move to `miagent` after the root cause is confirmed inside the CLI/runtime/TUI path.

## Repository focus

- `cli/` is the primary code path for most tasks.
- `apps/web/` and `apps/macos/` are app shells around the CLI.
- `server/` is a separate Rust workspace and should not be mixed into CLI changes unless the task requires it.

## Important architecture rules

- Preserve the distinction between default XDG state and `--project-state`.
  - XDG state uses directories like `~/.config/mosaic/` and `~/.local/share/mosaic/`.
  - Project-local state lives under `./.mosaic/`.
- Reuse runtime builders in `cli/crates/mosaic-cli/src/runtime_context.rs`.
  - Prefer `build_runtime` or `build_runtime_from_selector`.
  - Do not recreate runtime wiring inside feature crates unless absolutely necessary.
- Session persistence flows through `cli/crates/mosaic-core/src/session.rs` and JSONL event storage.
- Tool execution flows through `cli/crates/mosaic-tools/src/lib.rs`.

## TUI-specific rules

If the task touches `mosaic tui`:

- Start with:
  - `cli/crates/mosaic-cli/src/tui_command.rs`
  - `cli/crates/mosaic-tui/src/lib.rs`
  - `cli/crates/mosaic-core/src/state.rs`
  - `cli/crates/mosaic-core/src/session.rs`
- Preserve existing behavior for:
  - non-interactive `mosaic tui --prompt ...`
  - `--json` only for non-interactive mode
  - session resume and runtime rebinding
  - `--project-state` behavior
- Prefer extracting `mosaic-tui/src/lib.rs` into smaller modules before making major UX changes.

## Generated repository skills

This repository includes repo-scoped Copilot skills under `.github/skills/`. Load or consult the relevant one when appropriate:

- `mosaic-repo-guide`
  - Use for general repository orientation, Rust CLI structure, and common build/test commands.
- `mosaic-tui-repair`
  - Use for `mosaic tui`, ratatui/crossterm behavior, key handling, slash commands, pickers, inspector, and TUI refactors.
- `mosaic-project-state-debugging`
  - Use for `.mosaic`, `--project-state`, config/session routing, or state-dependent `ask`, `chat`, and `tui` bugs.

## Validation

Prefer existing repository commands:

```bash
make cli-build
make cli-quality
make cli-test
```

For focused TUI work:

```bash
cd cli
cargo test -p mosaic-cli --test tui_ops
cargo test -p mosaic-cli --test tui_interactive
cargo test -p mosaic-tui
```

For general CLI work:

```bash
cd cli
cargo build -p mosaic-cli
cargo test --workspace
```

## Change safety

- Do not change unrelated crates just to make a local fix compile.
- Do not silently break JSON output contracts.
- Do not break project-state file layout without auditing all readers/writers.
- If you significantly change runtime wiring, TUI module layout, or state layout, update `AGENT.md` and any relevant skill files in the same session.
