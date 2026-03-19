# WORKLOG

## 2026-03-19

### Stage 1 TUI Completed

- Completed the first terminal control-plane shell according to `stages/stage1_tui.md`.
- Added the root Cargo workspace configuration and defined workspace members plus shared dependency policy.
- Added the `cli/` executable entrypoint and kept it limited to installation entry and top-level startup responsibilities.
- Fixed the architecture boundary according to `AGENTS.md` by moving the terminal UI event loop, view state, keyboard interaction, mock data, and rendering logic into `crates/tui/`.
- The TUI now includes the following core regions:
  - left session list
  - center task/conversation timeline
  - top status bar
  - bottom input box / composer
  - right observability panel
- Implemented the following basic keyboard interactions:
  - `Tab` / `Shift+Tab` to switch focus
  - `j` / `k` or arrow keys to move
  - `i` to focus the composer
  - `Enter` to submit input
  - `Ctrl+L` to toggle the observability panel
  - `q` / `Ctrl+C` to quit
- Added responsive layout adjustments for a standard 80-column terminal so the right panel does not collapse under fixed-width assumptions.
- Added the standard root `Makefile` targets: `install`, `build`, `clean`, and `check`.

### Verification

- `make build`
- `make check`
- `cargo test --workspace`
- `cargo run` launched successfully and was manually exited

### Stage 2 Local Control Commands

- Added session-scoped composer drafts so switching between sessions no longer shares one global in-progress instruction.
- Added local slash-command handling in the TUI for mock control-plane actions:
  - `/help`
  - `/logs`
  - `/gateway connect`
  - `/gateway disconnect`
  - `/runtime <status>`
  - `/session state <active|waiting|degraded>`
  - `/session model <name>`
- Updated the session list and composer copy so saved drafts and command availability are visible in the interface.
- Added focused unit coverage for per-session drafts and command-driven state changes.
- Added an in-app operator help overlay, opened with `F1` or `?` outside the composer, so keyboard shortcuts and local slash commands are discoverable inside the TUI itself.
- Reworked the TUI around the four local reference images:
  - default startup canvas with welcome card, environment summary, bottom context strip, and single-line operator composer
  - `--resume` launch path with a searchable session resume browser
  - slash-command popup surface when the composer draft starts with `/`
  - updated mock session metadata so the resume browser has type, modified, created, and summary columns

### Verification

- `cargo test -p mosaic-tui`
- `cargo check --workspace`
