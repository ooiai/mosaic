# Observability

This document covers the `observability` command group for structured diagnostics export.

## Commands

```bash
# aggregate logs + system events + policy + safety audit + doctor summary
mosaic --project-state --json observability report --tail 100 --event-tail 50 --audit-tail 100 --compare-window 100

# filter by log source and system event name
mosaic --project-state --json observability report --source system --event-name deploy --audit-tail 50 --compare-window 50

# export report to file
mosaic --project-state --json observability export --out .mosaic/reports/obs.json --audit-tail 100 --compare-window 100

# skip doctor checks for faster report generation
mosaic --project-state --json observability report --no-doctor --audit-tail 50 --compare-window 50

# include parsed plugin soak metrics (explicit path)
mosaic --project-state --json observability report --plugin-soak-report ./cli/reports/plugin-soak-latest.log --no-doctor

# export report and keep plugin soak time-series history in state data
mosaic --project-state --json observability export --out .mosaic/reports/obs.json --plugin-soak-report ./cli/reports/plugin-soak-latest.log --no-doctor
```

## Report Structure

- `summary`: log/event counts, gateway/channels runtime counters (`gateway_running`, `gateway_endpoint_healthy`, `channels_total`, `channel_events_count`, `channel_failed_events`), alert counters (`alerts_total`, `alerts_warning`, `alerts_critical`, `alerts_suppressed`), SLO booleans (`slo_gateway_met`, `slo_channels_met`), safety-audit counts, doctor check counts, safety diff deltas (`safety_failed_delta`, `safety_failure_rate_delta`), and plugin soak history indicators (`plugin_soak_history_count`, `plugin_soak_history_pruned`, `plugin_soak_delta_available`, `plugin_soak_completion_ratio_delta`).
- `policy`: current approvals mode + sandbox profile.
- `logs`: unified log tail (same source as `mosaic logs`, including `plugin:*` event streams).
- `system_events`: filtered event stream (same source as `mosaic system list`).
- `gateway`: runtime status snapshot (running/process_alive/endpoint_healthy, selected target, state/service file metadata, and error envelope when unavailable).
- `channels`: delivery telemetry snapshot from `channels status` + channel event tail aggregation (`delivery_status`, `event_kinds`, `http_status`, recent events).
- `alerts`: alert-friendly rollup items derived from gateway/channels/safety/plugin-soak metrics, with `total/warning/critical` counters, suppression config (`min_severity`, `suppress_ids`), and `suppressed` audit section.
- `slo`: windowed SLO view for gateway and channels (`target`, `ratio`, `samples`, `met`) for automation gating.
- `safety_audit`: command audit summary, optional `comparison` window (`--compare-window`), and recent command decisions under current policy.
- `plugin_soak`: parsed metrics from `plugin_resource_soak.sh` output. Includes `summary`, derived `rates`, `trend` deltas (`completion_ratio`, `event_line_drift`), and `history` (`current_run`, `latest`, `current_vs_previous`, `retention.max_samples`, `retention.pruned`).
- `doctor.checks`: optional diagnostics checks (`--no-doctor` disables collection).

When `--plugin-soak-report` is omitted, observability attempts auto-discovery (`cli/reports/plugin-soak-latest.log`, `reports/plugin-soak-latest.log`) and reports `plugin_soak.status=missing` if no report is found.
Plugin soak history is stored at `.mosaic/data/reports/plugin-soak-history.jsonl` (or XDG data equivalent) and is appended only when the current report has `plugin_soak.available=true`.

## Alert Threshold Tuning

`report.alerts.thresholds` reflects effective values. Optional environment overrides:

- `MOSAIC_OBS_ALERT_CHANNEL_FAILURE_WARN` (default `0.1`)
- `MOSAIC_OBS_ALERT_CHANNEL_FAILURE_CRITICAL` (default `0.5`)
- `MOSAIC_OBS_ALERT_SAFETY_FAILURE_WARN` (default `0.25`)
- `MOSAIC_OBS_ALERT_SAFETY_FAILURE_CRITICAL` (default `0.5`)
- `MOSAIC_OBS_ALERT_PLUGIN_COMPLETION_MIN` (default `1.0`)

Optional suppression controls:

- `MOSAIC_OBS_ALERT_MIN_SEVERITY` (`warning` or `critical`, default `warning`)
- `MOSAIC_OBS_ALERT_SUPPRESS_IDS` (comma-separated alert IDs, e.g. `gateway.not_running,plugin_soak.incomplete`)

## SLO + Retention Tuning

- `MOSAIC_OBS_SLO_WINDOW` (default `20`)
- `MOSAIC_OBS_SLO_GATEWAY_TARGET` (default `0.99`)
- `MOSAIC_OBS_SLO_CHANNELS_TARGET` (default `0.99`)
- `MOSAIC_OBS_PLUGIN_SOAK_HISTORY_MAX_SAMPLES` (default `200`)
