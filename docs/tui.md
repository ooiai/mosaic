# TUI Guide

The Mosaic TUI is now a chat-first terminal operator surface.

It uses one main transcript, one composer, and one slash-command popup. Session browsing, model changes, inspect output, tool runs, skill runs, workflow runs, and gateway status all render back into the same conversation stream.

Telegram is still the strongest release-grade real GUI lane, but the TUI no longer depends on a multi-pane local mock console model.

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

## Interaction model

- The center of the screen is the transcript.
- The composer is the only primary input.
- Type a normal message and press `Enter` to submit a real turn.
- Type `/` to open the command popup.
- Keep typing to filter commands.
- Press `Tab` to accept the highlighted command.
- Press `Enter` to execute the completed command.

## What the TUI shows

- active session id and workspace in the header
- active profile/model and current run state
- transcript-backed conversation history
- inline tool, skill, workflow, provider, and system events
- inline MCP, node-routed, and workflow execution summaries with concise capability proof
- streaming assistant output
- inline operator cards for session/model/gateway/adapter/node/inspect/sandbox commands

## Basic keys

- `Enter`: send the current draft or execute the current slash command
- `/`: open the command popup
- `Tab`: accept the highlighted command completion
- `Up` / `Down`: move in the command popup
- `PageUp` / `PageDown`: scroll the transcript
- `Esc`: clear the draft or close the command popup
- `F1` or `?` on an empty composer: inject the command reference into the transcript
- `Ctrl+C`: quit

## Slash commands

- `/help`
- `/session list`
- `/session new <id>`
- `/session switch <id>`
- `/session show`
- `/model list`
- `/model show`
- `/model use <profile>`
- `/gateway status`
- `/adapter status`
- `/node list`
- `/node show <id>`
- `/sandbox status`
- `/sandbox inspect <env>`
- `/sandbox rebuild <env>`
- `/sandbox clean`
- `/run stop`
- `/run retry`
- `/inspect last`
- `/tool <name> <input>`
- `/skill <name> <input>`
- `/workflow <name> <input>`

For markdown skill packs, the slash popup now supports name completion after `/skill `.
Example:

```text
/skill op<Tab>
```

This completes to the registered markdown pack name and the resulting transcript blocks include concise pack provenance such as template, reference, script, and sandbox usage when available.

## Capability proof in the transcript

The TUI is now a local operator proof lane for capability execution.

Use these commands to understand what the runtime actually did without leaving the chat surface:

- `/tool <name> <input>`: show builtin, MCP, or node-routed tool execution inline
- `/workflow <name> <input>`: show workflow step execution inline
- `/adapter status`: show adapter readiness and outbound state
- `/node list`: show registered nodes, health, affinity, and disconnect state
- `/node show <id>`: show one node's declared capabilities
- `/inspect last`: render capability proof summaries inline for the last run

When a tool, MCP server, node-routed tool, skill, or workflow runs, the transcript includes concise execution summaries such as:

- route kind
- capability source
- execution target
- orchestration owner
- failure origin when a run fails

## Typical first run

1. start the TUI
2. type a normal message and press `Enter`
3. watch provider and capability events stream inline
4. type `/session show` or `/inspect last`
5. stay in the same transcript instead of leaving for another page

## Remote attach notes

When you use `--attach`, the TUI reads session state and runtime events from the remote Gateway URL. Slash commands that mutate run state, such as `/run stop`, `/run retry`, and explicit `/tool` / `/skill` / `/workflow`, are sent to the attached Gateway instead of being simulated locally.
