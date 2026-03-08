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
mosaic --project-state hooks logs [--hook <hook-id>] [--tail <n>] [--since-minutes <n>] [--summary]
mosaic --project-state hooks replay [--hook <hook-id>] [--tail <n>] [--limit <n>] [--batch-size <n>] [--since-minutes <n>] [--reason <reason>...] [--retryable-only] [--report-out <path>] [--apply] [--max-apply <n>] [--stop-on-error]
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

## Logs Inspection

- `hooks logs --since-minutes <n>` filters events to recent records (`ts >= now - n minutes`).
- `hooks logs --summary` returns aggregate counters:
  - `total`, `ok`, `failed`
  - `by_trigger` (manual/system_event)
  - `by_delivery_status`

## Failed Event Replay

- `hooks replay` only selects failed hook events (`ok=false`) from local event logs.
- Default mode is preview only (`apply=false`) and returns replay candidates + suggested commands.
- `--reason` supports replay filtering by failure classification:
  - `approval_required`, `sandbox_denied`, `auth`, `validation`, `tool`, `hook_failures`, `unknown`
- `--retryable-only` keeps only retryable failures (`tool`, `hook_failures`) in replay candidates.
- `--batch-size <n>` adds `batch_plan` metadata so large backlogs can be replayed in controlled chunks.
- `--report-out <path>` exports replay plan/apply result JSON for postmortem and automation.
- Use `--apply` to execute selected candidates. This requires `--yes`.
- `--max-apply <n>` caps apply attempts for partial replay rollouts.
- `--stop-on-error` stops replay at the first failed replay execution.
- Replay JSON includes `recovery_diagnostics` (`reason_histogram`, retryable split, backlog age, suggested strategy) for operator triage.
