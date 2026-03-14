# AGENT.md

This file is for future coding sessions working in `mosaic`, especially sessions that need to touch the Rust CLI or the TUI.

## 1. Repository intent

`mosaic` is a Rust-first local agent runtime with a large CLI surface. The main product path today is the CLI workspace under `cli/`.

At the repo root:

- `cli/`: primary Rust workspace; this is where almost all agent/runtime/TUI work happens.
- `apps/`: app shells around the CLI.
  - `apps/web/`: Vite/React web app.
  - `apps/macos/`: Swift macOS wrapper/app bundle flow.
- `packages/`: shared frontend packages (`ui`, `workbench`).
- `server/`: separate Rust workspace for backend/OpenAPI-related services.
- `site/`: static docs site pages.
- `skills/`: auxiliary skill content and scaffolding.
- `specs/`: spec material and supporting references.
- `README.md`: user-facing entry doc.
- `cli/README.md`: much deeper CLI command and workflow reference.
- `Makefile`: the fastest place to discover supported build/test workflows.

### Copilot helper assets

- `.github/copilot-instructions.md`
  - Shared bootstrap guidance for future Copilot sessions.
- `.github/skills/`
  - Repository-scoped skills for repo orientation, TUI repair, and project-state debugging.
- `.github/agents/mosaic-general.agent.md`
  - Broad default custom agent for repo-wide exploration, cross-surface work, and documentation/Copilot guidance tasks.
- `.github/agents/miagent.agent.md`
  - Deep specialist custom agent for CLI, TUI, runtime, and project-state work.

If the task scope is unclear or spans multiple top-level areas, start with `mosaic-general`. If the task narrows to `cli/`, `mosaic tui`, or `.mosaic` / `--project-state` behavior, switch to `miagent`.

If the task is about `mosaic --project-state ...`, start in `cli/`.

## 2. Commands worth knowing first

Repo-level commands:

```bash
make cli-build
make cli-build-release
make cli-run args='--project-state tui'
make cli-install
make cli-test
make cli-quality
make cli-regression
make web-build
make macos-dev
```

Direct Rust commands:

```bash
cd cli
cargo build -p mosaic-cli
cargo run -p mosaic-cli -- --project-state tui
cargo test --workspace
```

For TUI-only work, these are the most useful checks:

```bash
cd cli
cargo test -p mosaic-cli --test tui_ops
cargo test -p mosaic-cli --test tui_interactive
cargo test -p mosaic-tui
```

If you need a full safety net, prefer `make cli-quality` before `make cli-test`.

## 3. Rust CLI workspace map

The Rust workspace lives in `cli/Cargo.toml`. The most relevant crates are:

- `cli/crates/mosaic-cli`: top-level binary crate. Command parsing and `handle_*` dispatch live here.
- `cli/crates/mosaic-core`: shared config, state paths, sessions, audit, errors, model/profile primitives.
- `cli/crates/mosaic-agent`: agent loop, session playback, provider calls, tool-call handling.
- `cli/crates/mosaic-tools`: local tools used by the agent (`read_file`, `write_file`, `search_text`, `run_cmd`).
- `cli/crates/mosaic-tui`: fullscreen terminal UI implementation.
- `cli/crates/mosaic-agents`: saved agent definitions and route resolution.
- `cli/crates/mosaic-memory`: memory/indexing features.
- `cli/crates/mosaic-security`: security and audit helpers.
- `cli/crates/mosaic-mcp`: MCP server registry and health/diagnose flows.
- `cli/crates/mosaic-provider-openai`: OpenAI-compatible provider implementation.
- `cli/crates/mosaic-ops`, `mosaic-gateway`, `mosaic-channels`, `mosaic-plugins`: supporting operational/runtime modules.

### The CLI control path

The most important files/symbols:

- `cli/crates/mosaic-cli/src/main.rs`
  - `run(cli: Cli) -> Result<()>`
  - central `match` dispatch for all commands
- `cli/crates/mosaic-cli/src/cli_schema.rs`
  - `Cli`
  - `Commands`
  - command arg structs such as `TuiArgs`
- `cli/crates/mosaic-cli/src/runtime_context.rs`
  - `resolve_state_paths`
  - `build_runtime`
  - `build_runtime_from_selector`
- `cli/crates/mosaic-agent/src/lib.rs`
  - `AgentRunner`
  - `AgentRunOptions`
  - `AgentEvent`
- `cli/crates/mosaic-tools/src/lib.rs`
  - `ToolExecutor`
  - tool argument validation and shell execution rules
- `cli/crates/mosaic-core/src/state.rs`
  - `StatePaths`
  - XDG/project-state directory layout
- `cli/crates/mosaic-core/src/session.rs`
  - `SessionStore`
  - JSONL event persistence

## 4. State model and why it matters

Many bugs depend on whether the CLI runs in default XDG mode or project-local mode.

`cli/crates/mosaic-core/src/state.rs` defines:

- XDG mode:
  - config under `~/.config/mosaic/`
  - data under `~/.local/share/mosaic/`
- project mode:
  - everything under `./.mosaic/`

Important project-state paths:

```text
.mosaic/
  config.toml
  models.toml
  policy/
    approvals.toml
    sandbox.toml
  data/
    sessions/
    audit/commands.jsonl
    system-events.jsonl
```

When debugging TUI or session routing, inspect `.mosaic/data/sessions/*.jsonl` and the active `.mosaic/config.toml`.

## 5. Current TUI architecture in Mosaic

### Entry point

The `tui` command is wired in:

- `cli/crates/mosaic-cli/src/main.rs`
  - `Commands::Tui(args) => handle_tui(&cli, args).await`
- `cli/crates/mosaic-cli/src/cli_schema.rs`
  - `struct TuiArgs { session, prompt, agent, focus, no_inspector }`

The command handler is:

- `cli/crates/mosaic-cli/src/tui_command.rs`
  - `handle_tui`
  - `build_tui_runtime`
  - `resolve_tui_prompt`
  - `resolve_tui_initial_session_id`

### Interactive vs non-interactive

`handle_tui` splits behavior into two modes:

- Interactive mode:
  - requires real TTY stdin/stdout
  - no `--prompt`
  - calls `mosaic_tui::run_tui(...)`
- Non-interactive mode:
  - used for `mosaic tui --prompt ...`
  - reuses the normal `agent.ask(...)` path
  - supports JSON output

Important guardrail: `--json` is only valid with non-interactive TUI mode.

### Runtime building

`build_tui_runtime` does not assemble the world manually. It delegates to:

- `build_runtime_from_selector` in `runtime_context.rs`

That function loads:

- config (`ConfigManager`)
- session store (`SessionStore`)
- agent routing (`AgentStore`)
- model routing (`ModelRoutingStore`)
- approval/sandbox policy
- tool executor
- agent skills
- provider

Then it returns `RuntimeContext`, which is adapted into `TuiRuntime`.

This is the correct reuse point. If a future session needs to fix TUI runtime bugs, do not fork runtime initialization inside `mosaic-tui`; keep using `build_runtime_from_selector`.

### TUI implementation shape

The actual TUI lives almost entirely in one file:

- `cli/crates/mosaic-tui/src/lib.rs`

At the time of writing it is about 1558 lines long, which is the main structural smell.

Key pieces inside that file:

- `TuiFocus`
- `TuiRuntime`
- `TuiState`
- `AppEvent`
- `run_tui`
- `run_app`
- `handle_key`
- `load_selected_session`
- `parse_input_command`
- `handle_input_command`
- `toggle_agent_picker`
- `toggle_session_picker`
- `switch_active_agent`
- `spawn_agent_task`
- `render`
- `render_compact`
- `render_wide`
- `render_sessions`
- `render_messages`
- `render_input`
- `render_status`
- overlay/picker render helpers

### Current interaction model

The current TUI supports:

- focus cycling between messages/input/sessions/inspector
- session list on the left
- message pane in the center
- optional inspector pane on the right
- agent picker and session picker overlays
- slash-style input commands:
  - `/agent`
  - `/agents`
  - `/agent <id>`
  - `/session`
  - `/session <id>`
  - `/new`
  - `/status`
- keyboard shortcuts:
  - `Tab`
  - `Ctrl+N`
  - `Ctrl+R`
  - `Ctrl+I`
  - `Ctrl+A`
  - `Ctrl+S`
  - `?`
  - `q`

### TUI event flow

The event model is simple:

1. `handle_key` decides whether input is navigation, slash command, or prompt submit.
2. Prompt submit calls `spawn_agent_task`.
3. `spawn_agent_task` runs `AgentRunner::ask(...)` on a `tokio::spawn`.
4. Agent events are pushed through a `tokio::sync::mpsc::UnboundedSender<AppEvent>`.
5. `run_app` drains that channel and updates `TuiState`.
6. `render(...)` redraws the full screen every loop tick.

This works, but state/event/render/input responsibilities are still tightly coupled in a single file.

## 6. TUI hotspots and likely pain points

If you are fixing or refactoring TUI behavior, check these first:

1. `cli/crates/mosaic-cli/src/tui_command.rs`
   - mode detection
   - JSON-mode compatibility
   - runtime/session bootstrap

2. `cli/crates/mosaic-tui/src/lib.rs`
   - almost all behavior lives here
   - any UI fix usually touches `handle_key`, `TuiState`, and one of the `render_*` functions

3. `cli/crates/mosaic-core/src/session.rs`
   - session resumption depends on JSONL event shape

4. `cli/crates/mosaic-cli/src/runtime_context.rs`
   - agent/profile/session rebinding logic

5. `cli/crates/mosaic-agents/src/lib.rs`
   - agent route/default resolution

Important behavior to preserve:

- `mosaic tui --prompt ...` must keep working as a non-interactive path.
- `--project-state` must continue reading/writing `.mosaic/`.
- resuming a session should restore the session-bound runtime metadata.
- switching agents from the TUI currently starts a new session when history already exists.
- the status line should continue surfacing profile / agent / session / policy context.

## 7. Codex TUI reference notes

Reference repo:

- `https://github.com/openai/codex/tree/main/codex-rs/tui`

Useful findings from the Codex TUI:

- `codex-rs/tui/src/lib.rs` is mostly orchestration, not a single-file implementation dump.
- The crate is split into many focused modules:
  - `app`
  - `app_event`
  - `app_event_sender`
  - `bottom_pane`
  - `chatwidget`
  - `cwd_prompt`
  - `diff_render`
  - `file_search`
  - `history_cell`
  - `insert_history`
  - `notifications`
  - `resume_picker`
  - `selection_list`
  - `slash_command`
  - `status`
  - `theme_picker`
  - `tooltips`
  - `render`
  - `test_backend`
  - and many more
- The Codex crate also ships dedicated support artifacts:
  - `styles.md`
  - `tooltips.txt`
  - `frames/`
  - `tests/`

### Practical lessons worth borrowing

For Mosaic, the highest-value ideas to borrow are structural, not cosmetic:

1. Split event types and event sending out of the main TUI file.
2. Split rendering from state mutation.
3. Split slash-command parsing from raw keyboard handling.
4. Split session/agent pickers into dedicated modules/widgets.
5. Add reusable render/test helpers instead of asserting everything through one giant integration surface.
6. Keep the library side free of casual stdout/stderr output.

Codex is much broader in scope, so do not copy feature-for-feature. Use it as a modularization reference.

## 8. Recommended next step for TUI repair

If the next session is specifically about "fix the TUI with Codex as reference", the safest sequence is:

### Phase 1: extract without changing behavior

Create small modules under `cli/crates/mosaic-tui/src/` such as:

- `app.rs` or `state.rs`
- `events.rs`
- `commands.rs`
- `pickers.rs`
- `render.rs`
- `widgets.rs`

Start by moving code, not redesigning behavior.

### Phase 2: isolate responsibilities

Move these out of `lib.rs` first:

- `AppEvent`
- `parse_input_command`
- `handle_input_command`
- picker toggles/switching helpers
- render helpers

### Phase 3: add focused tests

After extraction, add tests for:

- slash command parsing
- session/agent picker state transitions
- selected-session rebinding behavior
- non-interactive `tui --prompt` behavior
- rendering invariants if snapshot/frame tests are practical

### Phase 4: only then change UX/layout

Only after behavior is isolated should you change:

- layout composition
- inspector behavior
- status rendering
- overlays/pickers
- scrolling/input ergonomics

## 9. Session guardrails

Before touching CLI/TUI code in a future session:

- Read `README.md`, `cli/README.md`, and this file.
- Inspect `cli/crates/mosaic-cli/src/tui_command.rs`.
- Inspect `cli/crates/mosaic-tui/src/lib.rs`.
- Inspect `cli/crates/mosaic-core/src/state.rs` and `session.rs`.
- Run at least a focused test command before and after changes.

Avoid these mistakes:

- do not rebuild runtime wiring inside `mosaic-tui`
- do not break project-state vs XDG behavior
- do not silently change JSON-mode `tui --prompt` output
- do not bypass `SessionStore` event persistence
- do not mutate unrelated modules just to make TUI compile

## 10. Update policy for this file

If a future session significantly changes:

- TUI module layout
- runtime construction
- state path layout
- key build/test commands

then update this `AGENT.md` in the same session so the next handoff stays accurate.
