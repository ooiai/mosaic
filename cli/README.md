# Mosaic CLI TUI

`cli/` is the composition root for launching the operator surface.

It owns:

- binary installation target
- top-level runtime composition

The actual terminal UI implementation now lives in `crates/tui/`, which owns:

- terminal event loop behavior
- view state and mocked operator data
- keyboard routing
- rendering for sessions, task timeline, composer, and observability

This matches the repository architecture in `AGENTS.md`: `cli/` launches the workflow, while reusable terminal UI internals live in the dedicated TUI crate.

Current keyboard model:

- `Tab` / `Shift+Tab`: cycle focus between panes
- `j` / `k` or arrow keys: move inside the focused pane
- `i`: jump to the composer
- `Enter`: submit from the composer
- `Ctrl+L`: toggle the observability panel
- `Esc`: leave the composer and return focus to sessions
- `q` or `Ctrl+C`: quit
