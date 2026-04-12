# TUI Guide

The Mosaic TUI is now a chat-first single-shell terminal operator surface.

It uses one main transcript, one composer, and one slash-command popup. There are no persistent session, model, inspect, or observability panes. Session browsing, model changes, inspect output, tool runs, skill runs, workflow runs, and gateway status all render back into the same conversation stream.

Telegram is still the strongest release-grade real GUI lane, but the TUI no longer depends on a multi-pane local mock console model.
The TUI is gateway-backed: normal messages and slash commands resolve against the same Gateway/runtime state the CLI uses.
Internally, the shell is now split into transcript, bottom-pane, overlay, and status-bar subsystems so future Codex-style UX work does not keep accumulating inside one large app renderer.
The current shell also favors a denser Codex-style presentation: compact header chrome, a lighter bottom pane, a slash popup anchored to the bottom pane, an explicit task-running indicator, and explicit busy / send-disabled feedback while a run is active.
The transcript now behaves like one evolving assistant turn: streaming text, tool/MCP/skill/workflow progress, sandbox preparation, node waits, and final completion update the same live turn cell instead of flooding the shell with unrelated cards. In-progress turns are kept separate from committed history, which is what lets the transcript overlay show a stable history plus the active live tail.
Nested execution detail no longer depends on giant inline expansions; `Ctrl+O` opens a focused turn-detail overlay that keeps the main transcript compact.
The shell now also exposes an explicit shell mode internally and visually: `idle`, `composing`, `command`, `running`, `transcript`, and `detail`. This keeps header/composer rhythm aligned with what the operator is actually doing instead of inferring UX from unrelated booleans.

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
- The shell does not switch into separate pages or panes for session, model, inspect, or status work.
- Type a normal message and press `Enter` to submit a real turn.
- Type `/` to open the command popup.
- Keep typing to filter commands.
- Press `Tab` to accept the highlighted command.
- Press `Enter` to execute the completed command.
- The popup prefers canonical bare slash commands such as `/help`, `/new`, `/session ...`, and `/model ...`.
- `/mosaic ...` remains supported as a compatibility alias inside the local shell.

## What the TUI shows

- active session id and workspace in the header
- active profile/model and current run state
- transcript-backed conversation history
- compact shell chrome with less persistent border noise
- a dedicated turn-detail overlay for the newest active/executable turn
- a dedicated transcript overlay for full-shell history inspection without leaving the local shell
- no persistent pages or side panes; operator actions return inline into the transcript
- inline tool, skill, workflow, provider, and system events
- collapsed execution cards for tool, MCP, skill, workflow, sandbox, gateway, adapter, and node activity
- one active assistant turn that moves through submitted, queued, streaming, capability-active, waiting-on-capability, failed, canceled, and completed phases
- inline MCP, node-routed, and workflow execution summaries with concise capability proof
- streaming assistant output with **full markdown rendering**: headers, bold/italic, inline code, fenced code blocks with syntax highlighting, bullet lists, numbered lists
- fenced code blocks highlighted by language (bash, rust, python, JSON)
- failure cards that keep the next operator action visible inline
- inline operator cards for session/model/gateway/adapter/node/inspect/sandbox commands
- **rich exec/tool call rows** on the active turn: animated spinner while running, ✓/✗ on completion, output tail for the last several lines
- git branch detection in the status bar subheader alongside token usage counters (input / output)
- inline diff display in the detail overlay for file-patch bodies (+ lines in green, − lines in red)
- **approval overlay**: when a capability call requires operator sign-off, the composer is replaced by a `[y] approve  [n / Esc] deny` prompt showing the tool name, command preview, and risk level

## Basic keys

- `Enter`: send the current draft or execute the current slash command
- `/`: open the command popup
- `Tab`: accept the highlighted command completion
- `Up` / `Down`: move in the command popup
- `PageUp` / `PageDown`: scroll the transcript
- `Esc`: clear the draft or close the command popup
- `F1` or `?` on an empty composer: inject the command reference into the transcript
- `Ctrl+O`: open or close the latest turn detail overlay
- `Ctrl+T`: open or close the transcript overlay
- `Ctrl+C`: quit

When a run is active, the bottom pane shows an explicit busy / send-disabled state and keeps `/run stop` visible in the shell chrome.

## Slash commands

Canonical commands:

- `/help [category]`
- `/new [id]`
- `/session list`
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

Compatibility aliases remain supported:

- `/mosaic`
- `/mosaic help [category]`
- `/mosaic session ...`
- `/mosaic model ...`
- `/mosaic gateway ...`
- `/mosaic adapter ...`
- `/mosaic node ...`
- `/mosaic sandbox ...`
- `/mosaic run ...`
- `/mosaic inspect ...`
- `/mosaic tool ...`
- `/mosaic skill ...`
- `/mosaic workflow ...`
- `/profile ...`

For markdown skill packs, the slash popup now supports name completion after `/skill ` and `/mosaic skill `.
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

Those summaries render as collapsed execution cards by default. When something fails, the transcript shows a failure card that preserves the failure origin plus the next operator action, such as `/inspect last`, `/sandbox status`, `/node list`, or `/run retry`.
Use `Ctrl+O` to open the latest turn's detail overlay and inspect provider retries, MCP startup, sandbox preparation, workflow steps, or node waits without losing the compact transcript view.
Use `Ctrl+T` to open a transcript overlay when you want the full shell history in a dedicated layer instead of the normal compact conversation view.

## Typical first run

1. start the TUI
2. type a normal message and press `Enter`
3. watch provider and capability events stream inline without losing the current composer draft
4. type `/session show` or `/inspect last`
5. stay in the same transcript instead of leaving for another page

## Local operator acceptance lane

When TUI UX is in scope, treat this as the release-quality local operator lane:

1. launch `mosaic tui` in a real terminal
2. confirm you can type immediately into the composer
3. confirm `/` opens the slash popup and `Tab` completes the highlighted command
4. submit one normal conversational turn and watch one full assistant response stream in place
5. run one capability-backed turn such as `/tool ...`, `/skill ...`, or `/workflow ...`
6. press `Ctrl+O` to reveal the latest turn's overlay with provider/tool/MCP/sandbox/node/workflow detail
7. press `Ctrl+T` to open the full transcript overlay and confirm the shell returns cleanly to the composer after exit
8. confirm `/run stop` and `/run retry` remain visible and usable when a run is active
9. confirm background session refresh does not clear the draft while the transcript updates

This is the local proof that Mosaic now has a Codex-style operator shell rather than only a chat-first transcript.

## Release-Grade PTY Acceptance Run

For release-oriented local verification, run the shell in a real PTY and record one complete operator flow.

That PTY run must prove:

1. startup lands in the composer and immediate typing works
2. `/` opens the slash popup near the bottom pane
3. `Tab` completes the highlighted slash command
4. `/help` renders inline command guidance
5. one direct chat turn submits through the real Gateway-backed interactive path
6. one streaming assistant turn updates in place
7. one capability-backed turn such as `/tool ...`, `/skill ...`, or `/workflow ...` shows attached activity
8. `Ctrl+O` opens the latest turn detail overlay without losing the draft
9. active runs show explicit busy / send-disabled state and expose `/run stop`
10. retry or cancel works without corrupting the draft or detaching the live turn

Record which steps were actually exercised, plus any missing provider/runtime preconditions that blocked the remaining ones.

### Current documented PTY proof in this workspace

The current local PTY probe already exercised these steps in a real terminal:

- startup landed in the composer
- immediate typing worked
- `/` opened the slash popup
- `Tab` completed `/help`
- `/help` rendered inline command guidance
- one direct chat turn submitted and drove the shell into busy / send-disabled state
- one explicit `/tool read_file README.md` command queued through the shell
- `/run retry` started a new live assistant turn instead of creating a detached transcript notice

That PTY probe did not prove a successful provider-backed streaming completion in this workspace, because the active provider transport failed during the run.
Release-grade TUI sign-off still requires a configured provider/runtime that can demonstrate one successful streaming turn and one successful capability-backed turn.

## Remote attach notes

When you use `--attach`, the TUI reads session state and runtime events from the remote Gateway URL. Slash commands that mutate run state, such as `/run stop`, `/run retry`, and explicit `/tool` / `/skill` / `/workflow`, are sent to the attached Gateway instead of being simulated locally.
