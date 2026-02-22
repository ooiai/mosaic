# Hooks (CLI)

`hooks` provides local event-driven automation for Mosaic CLI.

## Commands

```bash
mosaic --project-state hooks list [--event <event-name>]
mosaic --project-state hooks add --name <name> --event <event-name> --command "<shell command>" [--disabled]
mosaic --project-state hooks remove <hook-id>
mosaic --project-state hooks enable <hook-id>
mosaic --project-state hooks disable <hook-id>
mosaic --project-state --yes hooks run <hook-id> [--data '<json>']
mosaic --project-state hooks logs [--hook <hook-id>] [--tail <n>]
```

## Auto Trigger From System Events

When you run:

```bash
mosaic --project-state --yes system event deploy --data '{"version":"1.0.0"}'
```

all enabled hooks whose `event` is `deploy` are executed automatically.

## Safety

- Hook command execution uses the same runtime guard as `run_cmd`.
- Sandbox and approvals policies are enforced.
- Under default approvals mode (`confirm`), non-interactive execution requires `--yes`.

## Storage

- Hooks definition file: `.mosaic/data/hooks.json`
- Hook execution events: `.mosaic/data/hook-events/<hook-id>.jsonl`
