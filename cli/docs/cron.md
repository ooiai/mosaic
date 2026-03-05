# Cron (CLI)

`cron` provides local scheduled event execution.

## Commands

```bash
mosaic --project-state cron list [--event <event-name>]
mosaic --project-state cron add --name <name> --event <event-name> --every <seconds> [--data '<json>'] [--disabled]
mosaic --project-state cron remove <job-id>
mosaic --project-state cron enable <job-id>
mosaic --project-state cron disable <job-id>
mosaic --project-state --yes cron run <job-id> [--data '<json>']
mosaic --project-state --yes cron tick [--limit <n>]
mosaic --project-state cron logs [--job <job-id>] [--tail <n>]
```

## Behavior

- `cron tick` executes due jobs where `next_run_at <= now`.
- A cron job emits `system event <event-name>` with its configured JSON `data`.
- Matching enabled hooks are auto-triggered through the same event pipeline.
- After each run, the job updates `last_run_at`, `run_count`, and `next_run_at`.

### Built-in event: `mosaic.memory.cleanup`

- Event aliases:
  - `mosaic.memory.cleanup` (primary)
  - `memory.cleanup` (compat alias)
- Runtime behavior:
  - loads persisted policy from `.mosaic/policy/memory.toml`
  - if policy is disabled, skip silently
  - if `min_interval_minutes` is configured and not elapsed, skip unless forced
  - applies prune using policy limits
- Supported event payload fields:
  - `dry_run` (`boolean`, optional, default `false`)
  - `force` (`boolean`, optional, default `false`)

Example:

```bash
mosaic --project-state memory policy set \
  --enabled true \
  --max-namespaces 5 \
  --min-interval-minutes 60

mosaic --project-state cron add \
  --name memory-cleanup-hourly \
  --event mosaic.memory.cleanup \
  --every 3600 \
  --data '{"dry_run":false}'

mosaic --project-state --yes cron tick
```

## Safety

- Cron-triggered hooks still use approvals/sandbox policy checks.
- Under default approvals mode (`confirm`), non-interactive execution typically needs `--yes`.

## Storage

- Jobs definition file: `.mosaic/data/cron-jobs.json`
- Execution event logs: `.mosaic/data/cron-events/<job-id>.jsonl`
