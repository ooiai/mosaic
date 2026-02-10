# Agents Runtime (V3 Minimal)

This module adds multi-agent configuration and lightweight route bindings for CLI execution.

## Commands

```bash
# Inspect agents
mosaic --project-state agents list
mosaic --project-state agents show <agent_id>

# Create/remove agents
mosaic --project-state agents add --name Writer --id writer --profile default --set-default --route ask
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
2. Route binding (`ask` or `chat`)
3. Default agent (`agents default <id>`)
4. Fallback to CLI `--profile` configuration

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

All commands support `--json` and use the common envelope:

```json
{ "ok": true, "...": "..." }
```
