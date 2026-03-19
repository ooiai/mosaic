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
- local mock command routing for stage-2 control actions

This matches the repository architecture in `AGENTS.md`: `cli/` launches the workflow, while reusable terminal UI internals live in the dedicated TUI crate.

Current keyboard model:

- `Tab` / `Shift+Tab`: cycle focus between panes
- `j` / `k` or arrow keys: move inside the focused pane
- `i`: jump to the composer
- `Enter`: submit from the composer
- `Ctrl+L`: toggle the observability panel
- `Esc`: leave the composer and return focus to sessions
- `q` or `Ctrl+C`: quit

Current local mock commands:

- `/help`: show the available local control commands in the timeline
- `/logs`: toggle the observability panel
- `/gateway connect` or `/gateway disconnect`: update local gateway connectivity state
- `/runtime <status>`: update the control-plane runtime status label
- `/session state <active|waiting|degraded>`: update the selected session state
- `/session model <name>`: update the selected session model label

Composer drafts are session-scoped, so switching sessions no longer risks sending a partially typed instruction to the wrong target.
