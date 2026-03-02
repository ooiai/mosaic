# Ops (Logs / System)

This document covers the lightweight operational commands used for local observability and event signaling.

## Commands

```bash
# unified logs across system/hooks/cron/channels/webhooks/plugins/browser
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

# aggregated observability report (logs + events + policy + doctor)
mosaic --project-state --json observability report --tail 100 --event-tail 50 --audit-tail 100 --compare-window 100
mosaic --project-state --json observability export --out .mosaic/reports/obs.json --tail 100 --event-tail 50 --audit-tail 100 --compare-window 100 --no-doctor

# include plugin soak metrics in observability output
mosaic --project-state --json observability report --plugin-soak-report ./cli/reports/plugin-soak-latest.log --no-doctor
```

## Notes

- `logs` is the fastest way to inspect cross-module activity after `channels send`, `plugins run`, `system event`, `webhooks resolve`, `cron tick`, or `browser open`.
- `system event` is also the trigger path for enabled hooks and cron-linked workflows.
- `system list` reads directly from the system event stream and is useful when you need event-only history.
- `observability report/export` provides structured diagnostics snapshots for automation or incident triage, including gateway runtime health, channels delivery telemetry, alert rollups (`alerts.total/warning/critical/suppressed`) with suppression controls, SLO status (`slo.gateway`, `slo.channels`), safety audit summaries, optional window diffs (`--compare-window`), optional parsed plugin soak metrics (`--plugin-soak-report`), and plugin soak time-series history deltas (`current_vs_previous`) with retention controls.
- Use `--json` for scripts and CI checks.
