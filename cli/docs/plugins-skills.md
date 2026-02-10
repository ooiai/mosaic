# Plugins and Skills (V3 Minimal Framework)

This module provides a local, CLI-first runtime for discovering and validating plugin and skill packages.

## Commands

```bash
# Plugins
mosaic --project-state plugins list
mosaic --project-state plugins info <plugin-id>
mosaic --project-state plugins check [plugin-id]

# Skills
mosaic --project-state skills list
mosaic --project-state skills info <skill-id>
mosaic --project-state skills check [skill-id]
```

## Discovery Roots

The CLI discovers extensions from these roots in priority order:

1. Project state root (`.mosaic/plugins`, `.mosaic/skills` when `--project-state` is used)
2. `$CODEX_HOME/plugins` and `$CODEX_HOME/skills` (if `CODEX_HOME` is set)
3. `~/.codex/plugins` and `~/.codex/skills`

If duplicate IDs exist, earlier roots override later roots.

## Expected File Shape

- Plugin directory:
  - `<plugin-id>/plugin.toml` (recommended)
- Skill directory:
  - `<skill-id>/SKILL.md` (required for discovery)

Minimal `plugin.toml` example:

```toml
[plugin]
id = "demo"
name = "Demo Plugin"
version = "0.1.0"
description = "Example plugin package."
```

## JSON Contracts

All commands support `--json`. Successful command envelope:

```json
{ "ok": true, "...": "..." }
```

`check` returns a report with per-extension checks and summary:

- `report.ok`
- `report.checked`
- `report.failed`
- `report.results[]`

Missing target IDs return validation error (`exit_code=7`).
