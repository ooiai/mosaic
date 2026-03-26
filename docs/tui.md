# TUI Guide

The Mosaic TUI is the main operator surface for interactive sessions.

## Start the TUI

```bash
mosaic tui
```

Start on a specific session or profile:

```bash
mosaic tui --session support --profile openai
```

Attach to a remote Gateway:

```bash
mosaic tui --attach http://127.0.0.1:8080 --session remote-demo
```

## What the TUI shows

- current session metadata
- runtime timeline
- session transcript-derived history
- gateway health and readiness summary
- extension summary
- node summary
- composer for new turns and slash commands

## Basic keys

- `Enter`: send the current draft
- `Tab`: cycle focus
- `Shift+Tab`: move focus backward
- `j` / `k` or arrows: move in the active list or stream
- `Ctrl+L`: toggle the activity feed
- `F1`: help overlay
- `Esc`: close overlays or search
- `q` or `Ctrl+C`: quit

## Local slash commands

- `/help`
- `/logs`
- `/gateway connect`
- `/gateway disconnect`
- `/runtime <status>`
- `/session state <active|waiting|degraded>`
- `/session model <name>`
- `/model list`
- `/model use <profile>`

## Typical first run

1. start the TUI
2. send a message
3. note the session ID in the header
4. leave the TUI
5. inspect the saved session and trace

```bash
mosaic session show <session-id>
mosaic inspect .mosaic/runs/<run-id>.json
```

## Remote attach notes

When you use `--attach`, the TUI reads session state and event flow from the remote Gateway URL. The `gateway connect` and `gateway disconnect` commands control local refresh and event streaming in the TUI surface; they do not reconfigure the remote server itself.
