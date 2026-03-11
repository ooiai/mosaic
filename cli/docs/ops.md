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
- `observability report/export` provides structured diagnostics snapshots for automation or incident triage, including gateway runtime health, gateway request telemetry from `.mosaic/data/gateway-events.jsonl` (`gateway.telemetry.*` + `gateway.recent_events`), channels delivery telemetry, node/device/pairing lifecycle telemetry from `.mosaic/data/nodes-events.jsonl` (`nodes.summary.*`, `nodes.scopes`, `nodes.actions`, `nodes.recent_events`), MCP registry/check telemetry (`mcp.summary.*`), realtime telemetry (`realtime.summary.*` for `tts`/`voicecall`), alert rollups (`alerts.total/warning/critical/suppressed`) with suppression controls, SLO status (`slo.gateway`, `slo.channels`), persisted SLO history (`slo.history.current_vs_previous`, unmet streaks, repeated-alert incident hints), safety audit summaries, optional window diffs (`--compare-window`), optional parsed plugin soak metrics (`--plugin-soak-report`), and plugin soak time-series history deltas (`current_vs_previous`) with retention controls plus repeated-anomaly hints (`plugin_soak.history.incident_hints`).
- Gateway health now includes persisted history at `.mosaic/data/reports/observability-gateway-history.jsonl` (`gateway.history.*`) with run-level deltas, repeated-failure/not-running hints, and regression signals between consecutive samples.
- Gateway alert thresholds can be tuned via `MOSAIC_OBS_ALERT_GATEWAY_FAILURE_WARN` / `MOSAIC_OBS_ALERT_GATEWAY_FAILURE_CRITICAL` (current failure ratio) and `MOSAIC_OBS_ALERT_GATEWAY_FAILURE_DELTA_WARN` / `MOSAIC_OBS_ALERT_GATEWAY_FAILURE_DELTA_CRITICAL` (failure-ratio regression delta).
- Gateway history retention and hint windows can be tuned via `MOSAIC_OBS_GATEWAY_HISTORY_MAX_SAMPLES`, `MOSAIC_OBS_GATEWAY_INCIDENT_WINDOW`, `MOSAIC_OBS_GATEWAY_REPEAT_HINT_THRESHOLD`, and `MOSAIC_OBS_GATEWAY_FAILURE_REGRESSION_WARN`.
- MCP health now includes persisted history at `.mosaic/data/reports/observability-mcp-history.jsonl` (`mcp.history.*`) with delta/regression and incident hints for long-window degradation tracking.
- MCP alert thresholds can be tuned via `MOSAIC_OBS_ALERT_MCP_UNHEALTHY_WARN` / `MOSAIC_OBS_ALERT_MCP_UNHEALTHY_CRITICAL` (current unhealthy ratio) and `MOSAIC_OBS_ALERT_MCP_RATIO_DELTA_WARN` / `MOSAIC_OBS_ALERT_MCP_RATIO_DELTA_CRITICAL` (regression delta between consecutive samples).
- MCP history retention and hint windows can be tuned via `MOSAIC_OBS_MCP_HISTORY_MAX_SAMPLES`, `MOSAIC_OBS_MCP_INCIDENT_WINDOW`, `MOSAIC_OBS_MCP_REPEAT_HINT_THRESHOLD`, and `MOSAIC_OBS_MCP_RATIO_REGRESSION_WARN`.
- Voicecall delivery alert thresholds can be tuned via `MOSAIC_OBS_ALERT_VOICECALL_FAILURE_WARN` and `MOSAIC_OBS_ALERT_VOICECALL_FAILURE_CRITICAL` (ratio of failed/delivery events in recent voicecall telemetry tail).
- Gateway request-failure alert thresholds can be tuned via `MOSAIC_OBS_ALERT_GATEWAY_FAILURE_WARN` and `MOSAIC_OBS_ALERT_GATEWAY_FAILURE_CRITICAL` (ratio of failed/total recent gateway probe/discover/call/diagnose events).
- Use `--json` for scripts and CI checks.
