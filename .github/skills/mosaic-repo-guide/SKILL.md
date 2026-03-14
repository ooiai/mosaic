---
name: mosaic-repo-guide
description: Use for work in the Mosaic repository, especially Rust CLI, agent runtime, TUI, build/test commands, and locating the right workspace or file to edit.
---

# Mosaic repository guide

Use this skill when a task involves the `mosaic` repository and you need fast orientation before editing code.

## Start here

1. Read `AGENT.md` for the current handoff summary.
2. Read `README.md` for the product-level overview.
3. If the task is Rust CLI or TUI related, switch into `cli/` and read `cli/README.md`.

## Repository map

- `cli/`: primary Rust workspace; most CLI, runtime, agent, and TUI work happens here.
- `apps/web/`: Vite/React web app.
- `apps/macos/`: Swift macOS wrapper/app bundle flow.
- `packages/`: shared frontend packages such as `ui` and `workbench`.
- `server/`: separate Rust workspace for backend/OpenAPI services.
- `site/`: static docs site.
- `skills/`: repository skill scaffolding and examples used by Mosaic itself.

## Rust crates to know

- `cli/crates/mosaic-cli`: command parsing and `handle_*` entrypoints.
- `cli/crates/mosaic-core`: config, state paths, sessions, audit, errors.
- `cli/crates/mosaic-agent`: agent loop and tool-call handling.
- `cli/crates/mosaic-tools`: `read_file`, `write_file`, `search_text`, `run_cmd`.
- `cli/crates/mosaic-tui`: fullscreen terminal UI.
- `cli/crates/mosaic-agents`, `mosaic-memory`, `mosaic-security`, `mosaic-mcp`: feature modules frequently touched by CLI work.

## Common validation commands

Prefer existing repo commands:

```bash
make cli-build
make cli-quality
make cli-test
make cli-run args='--project-state tui'
```

Or run focused Rust commands in `cli/`:

```bash
cargo build -p mosaic-cli
cargo test --workspace
```

## Important constraints

- Do not break `--project-state` versus default XDG state behavior.
- Reuse runtime builders in `cli/crates/mosaic-cli/src/runtime_context.rs` instead of recreating runtime wiring.
- Preserve JSON-mode contracts for commands that support `--json`.
- Prefer focused tests first, then broader workspace tests.
