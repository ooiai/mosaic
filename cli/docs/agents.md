# Agents Runtime (V3 Minimal)

This module adds multi-agent configuration and lightweight route bindings for CLI execution.

## Commands

```bash
# Inspect agents
mosaic --project-state agents list
mosaic --project-state agents current --route chat
mosaic --project-state agents current --route chat --session <session_id>
mosaic --project-state agents current --agent writer --route chat
mosaic --project-state agents show <agent_id>

# Create/remove agents
mosaic --project-state agents add --name Writer --id writer --profile default --skill writer --set-default --route ask
mosaic --project-state agents update writer --name "Writer V2" --model gpt-4o-mini --route chat
mosaic --project-state agents update writer --skill reviewer
mosaic --project-state agents update writer --clear-skills
mosaic --project-state agents update writer --clear-model --clear-temperature
mosaic --project-state agents remove writer

# Default agent
mosaic --project-state agents default
mosaic --project-state agents default writer

# Route bindings
mosaic --project-state agents route list
mosaic --project-state agents route set ask writer
mosaic --project-state agents route remove ask
mosaic --project-state agents route resolve --route ask
```

## Runtime Resolution Order

`ask/chat` choose agent in this order:

1. `--agent <agent_id>`
2. Session-bound runtime metadata when resuming with `--session <id>`
3. Route binding (`ask` or `chat`)
4. Default agent (`agents default <id>`)
5. Fallback to CLI `--profile` configuration

Use `agents current` to inspect that precedence without running a model turn. The command reports:

- explicit agent input
- session-bound agent/profile metadata
- route-bound agent
- default agent
- final resolved agent/profile
- resolution source (`explicit_agent`, `session_runtime`, `route_binding`, `default_agent`, `cli_profile`)

When a session is first created, Mosaic persists runtime metadata in the session stream. Later `ask/chat/tui --session <id>` resumes the same agent automatically unless you explicitly override it with `--agent`.

Inside `mosaic chat` and interactive `mosaic tui`, you can also switch agents mid-conversation with `/agent <agent_id>`. `/agents` now exposes the configured inventory directly inside the conversation loop: `chat` prints the list inline and `tui` opens the overlay picker. If the current conversation already has a session, Mosaic resets to a new session before applying the new agent so conversation history never changes agent in place.

Inside interactive `mosaic tui`, selecting a different session from the left pane also rebinds the active runtime to that session's persisted agent/profile metadata before the next turn runs, and the sessions pane now shows each session's bound `profile / agent` summary.

The interactive TUI status line also keeps the active runtime visible at all times: current detail, `profile`, `agent`, `session`, and `policy`.

Interactive `mosaic tui` also supports `Ctrl+A` to open an agent picker, so you can switch agents without typing `/agent <id>`. The picker uses the same safety rule as slash-command switching: if the current conversation already has history, Mosaic starts a new session before applying the new agent.

Interactive `mosaic tui` now also supports `Ctrl+S` for an overlay-based session picker plus `/session <id>`, `/new`, and `/status` inside the input pane. That gives the TUI the same session-control path even when the sessions pane is not focused or not visible because the terminal is narrow.

`agents list` and the TUI agent picker now also surface default/route metadata inline, so the operator can see whether an agent is the default and which routes (`ask`, `chat`, and so on) point at it before switching.

## Session Runtime Metadata

Use `session show` to inspect the runtime binding for an existing conversation:

```bash
mosaic --project-state --json session show <session_id>
mosaic --project-state session resume <session_id>
mosaic --project-state chat
# inside REPL:
/agent writer
mosaic --project-state tui
# inside TUI input:
/agent writer
```

`session show --json` now includes:

- `runtime.profile_name`
- `runtime.agent_id`

`session list --json` now also includes a `runtime` object per session summary when runtime metadata is available.

The metadata is stored as a `system` event inside the session JSONL stream and does not get injected back into the model conversation history.

## Data Files

- `.mosaic/data/agents.json`
- `.mosaic/data/agent-routes.json`

## Overrides

Each agent may override these profile fields:

- `provider.model`
- `agent.temperature`
- `agent.max_turns`
- `tools.enabled`
- `tools.run.guard_mode`
- `skills[]` (loaded from local `SKILL.md` and injected into agent system prompt)

`agents update` supports clearing optional overrides:

- `--clear-model`
- `--clear-temperature`
- `--clear-max-turns`
- `--clear-tools-enabled`
- `--clear-guard-mode`
- `--clear-skills`

## Agent Skills

Use `--skill <skill_id>` on `agents add|update` to bind one or more installed skills.

- Skill IDs must exist in `mosaic skills list`.
- At runtime (`ask/chat`), bound `SKILL.md` content is appended to the system prompt.
- If a bound skill is removed later, runtime will fail with a validation error until fixed.

All commands support `--json` and use the common envelope:

```json
{ "ok": true, "...": "..." }
```
