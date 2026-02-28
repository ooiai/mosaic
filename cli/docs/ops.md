# Ops (Logs / System)

This document covers the lightweight operational commands used for local observability and event signaling.

## Commands

```bash
# unified logs across system/hooks/cron/channels/webhooks/browser
mosaic --project-state logs --tail 100
mosaic --project-state --json logs --tail 200
mosaic --project-state --json logs --tail 200 --source system
mosaic --project-state --json logs --follow

# emit a system event (hooks/webhooks/cron pipelines can consume it)
mosaic --project-state system event deploy --data '{"version":"1.0.0","env":"staging"}'
mosaic --project-state --json system event deploy --data '{"version":"1.0.0","env":"staging"}'

# runtime presence probe
mosaic --project-state system presence
mosaic --project-state --json system presence

# list recent system events
mosaic --project-state system list --tail 50
mosaic --project-state --json system list --tail 50
mosaic --project-state --json system list --tail 50 --name deploy
```

## Notes

- `logs` is the fastest way to inspect cross-module activity after `channels send`, `system event`, `webhooks resolve`, `cron tick`, or `browser open`.
- `system event` is also the trigger path for enabled hooks and cron-linked workflows.
- `system list` reads directly from the system event stream and is useful when you need event-only history.
- Use `--json` for scripts and CI checks.
