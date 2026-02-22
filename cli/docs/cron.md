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

## Safety

- Cron-triggered hooks still use approvals/sandbox policy checks.
- Under default approvals mode (`confirm`), non-interactive execution typically needs `--yes`.

## Storage

- Jobs definition file: `.mosaic/data/cron-jobs.json`
- Execution event logs: `.mosaic/data/cron-events/<job-id>.jsonl`
