# Webhooks (CLI)

`webhooks` provides route-based event dispatch in local CLI mode.

## Commands

```bash
mosaic --project-state webhooks list [--event <event-name>]
mosaic --project-state webhooks add --name <name> --event <event-name> --path <route-path> [--method post|get|put|patch|delete] [--secret-env <ENV>] [--disabled]
mosaic --project-state webhooks remove <webhook-id>
mosaic --project-state webhooks enable <webhook-id>
mosaic --project-state webhooks disable <webhook-id>
mosaic --project-state --yes webhooks trigger <webhook-id> [--data '<json>'] [--secret <value>]
mosaic --project-state --yes webhooks resolve --path <route-path> [--method post|get|put|patch|delete] [--data '<json>'] [--secret <value>]
mosaic --project-state webhooks logs [--webhook <webhook-id>] [--tail <n>] [--since-minutes <n>] [--summary]
mosaic --project-state webhooks replay [--webhook <webhook-id>] [--tail <n>] [--limit <n>] [--batch-size <n>] [--since-minutes <n>] [--reason <reason>...] [--retryable-only] [--secret <value>] [--report-out <path>] [--apply] [--max-apply <n>] [--stop-on-error]
```

## Behavior

- `resolve` matches enabled webhook by `method + path`.
- On match, webhook dispatches `system event <event-name>`.
- Matching enabled hooks then run through the same event pipeline.

## Secret Validation

- If webhook has `secret_env`, `resolve/trigger` requires `--secret`.
- Expected secret is read from the environment variable named by `secret_env`.
- Secret is never persisted in state files.

## Safety

- Downstream hook command execution still follows approvals/sandbox policies.
- Under default approvals mode (`confirm`), non-interactive execution usually needs `--yes`.

## Storage

- Webhook definitions: `.mosaic/data/webhooks.json`
- Webhook event logs: `.mosaic/data/webhook-events/<webhook-id>.jsonl`

## Logs Inspection

- `webhooks logs --since-minutes <n>` filters events to recent records (`ts >= now - n minutes`).
- `webhooks logs --summary` returns aggregate counters:
  - `total`, `ok`, `failed`
  - `hooks_triggered`, `hooks_ok`, `hooks_failed`
  - `by_trigger` (manual/resolve)
  - `by_method` (GET/POST/PUT/PATCH/DELETE)

## Failed Event Replay

- `webhooks replay` selects failed webhook deliveries (`ok=false`) from local event logs.
- Default mode is preview (`apply=false`) and returns replay candidates.
- `--reason` supports replay filtering by failure classification:
  - `approval_required`, `sandbox_denied`, `auth`, `validation`, `tool`, `hook_failures`, `unknown`
- `--retryable-only` keeps only retryable failures (`tool`, `hook_failures`) in replay candidates.
- `--batch-size <n>` adds `batch_plan` metadata so large backlogs can be replayed in controlled chunks.
- `--report-out <path>` exports replay plan/apply result JSON for postmortem and automation.
- `--apply` replays selected failed deliveries and requires `--yes`.
- `--max-apply <n>` caps apply attempts for partial replay rollouts.
- For secret-protected webhooks:
  - replay uses `--secret` if provided
  - otherwise it attempts to read secret from webhook `secret_env`
- `--stop-on-error` stops replay after the first replay failure.
- Replay JSON includes `recovery_diagnostics` (`reason_histogram`, retryable split, backlog age, suggested strategy) for operator triage.
