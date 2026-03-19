# Mosaic CLI TUI

`cli/` is the composition root for launching the operator surface.

It owns:

- binary installation target
- top-level runtime composition

The actual terminal UI implementation now lives in `crates/tui/`, which owns:

- terminal event loop behavior
- view state and mocked operator data
- keyboard routing
- rendering for the startup canvas, resume browser, command palette, and help overlay
- local mock command routing for control-plane actions

This matches the repository architecture in `AGENTS.md`: `cli/` launches the workflow, while reusable terminal UI internals live in the dedicated TUI crate.

Current launch modes:

- `mosaic`: open the startup canvas with the bottom operator composer ready
- `mosaic --resume`: open the session resume browser first

Current keyboard model:

- `Tab` / `Shift+Tab`: cycle focus in the console, or cycle scope tabs in resume mode
- `j` / `k` or arrow keys: move inside the active stream or resume list
- `Enter`: submit from the composer
- `r`: open the session resume browser from the console
- `F1`: open or close the in-app operator help overlay
- `?`: open or close the help overlay outside the composer
- `Ctrl+L`: toggle the observability panel
- `Esc`: leave help, resume, or search mode
- `Ctrl+C`: quit

Current reference-driven surfaces:

- startup canvas with a welcome card, environment summary, bottom context strip, and composer
- resume browser with scope tabs plus searchable mock session history
- slash-command popup when the composer draft starts with `/`

The help overlay mirrors the keyboard model and local slash-command reference inside the TUI, so operators do not need to leave the terminal to discover the available mock control actions.

Current local mock commands:

- `/help`: show the available local control commands in the timeline
- `/logs`: toggle the observability panel
- `/gateway connect` or `/gateway disconnect`: update local gateway connectivity state
- `/runtime <status>`: update the control-plane runtime status label
- `/session state <active|waiting|degraded>`: update the selected session state
- `/session model <name>`: update the selected session model label

Composer drafts are session-scoped, so switching sessions no longer risks sending a partially typed instruction to the wrong target.
