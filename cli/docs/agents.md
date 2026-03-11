# Agents Runtime (V3 Minimal)

This module adds multi-agent configuration and lightweight route bindings for CLI execution.

## Commands

```bash
# Inspect agents
mosaic --project-state agents list
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

When a session is first created, Mosaic persists runtime metadata in the session stream. Later `ask/chat/tui --session <id>` resumes the same agent automatically unless you explicitly override it with `--agent`.

Inside `mosaic chat`, you can also switch agents mid-REPL with `/agent <agent_id>`. If the current chat already has a session, Mosaic resets to a new session before applying the new agent so conversation history never changes agent in place.

## Session Runtime Metadata

Use `session show` to inspect the runtime binding for an existing conversation:

```bash
mosaic --project-state --json session show <session_id>
mosaic --project-state session resume <session_id>
mosaic --project-state chat
# inside REPL:
/agent writer
```

`session show --json` now includes:

- `runtime.profile_name`
- `runtime.agent_id`

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
