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
mosaic --project-state cron logs [--job <job-id>] [--tail <n>] [--since-minutes <n>] [--summary]
mosaic --project-state cron replay [--job <job-id>] [--tail <n>] [--limit <n>] [--batch-size <n>] [--since-minutes <n>] [--reason <reason>...] [--retryable-only] [--report-out <path>] [--apply] [--max-apply <n>] [--stop-on-error]
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

## Logs Inspection

- `cron logs --since-minutes <n>` filters events to recent records (`ts >= now - n minutes`).
- `cron logs --summary` returns aggregate counters:
  - `total`, `ok`, `failed`
  - `hooks_triggered`, `hooks_ok`, `hooks_failed`
  - `by_trigger` (tick/manual)

## Failed Event Replay

- `cron replay` selects failed cron executions (`ok=false`) from event logs.
- Default behavior is plan mode only (`apply=false`).
- `--reason` supports replay filtering by failure classification:
  - `approval_required`, `sandbox_denied`, `auth`, `validation`, `tool`, `hook_failures`, `unknown`
- `--retryable-only` keeps only retryable failures (`tool`, `hook_failures`) in replay candidates.
- `--batch-size <n>` adds `batch_plan` metadata so large backlogs can be replayed in controlled chunks.
- `--report-out <path>` exports replay plan/apply result JSON for postmortem and automation.
- `--apply` reruns failed executions using recorded event payloads. This requires `--yes`.
- `--max-apply <n>` caps apply attempts for partial replay rollouts.
- `--stop-on-error` terminates replay when the first replay attempt fails.
- Replay JSON includes `recovery_diagnostics` (`reason_histogram`, retryable split, backlog age, suggested strategy) for operator triage.
