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
mosaic --project-state webhooks logs [--webhook <webhook-id>] [--tail <n>]
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
