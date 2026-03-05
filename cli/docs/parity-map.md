# Mosaic CLI Coverage Map

This file tracks Mosaic CLI command/module coverage and pending gaps.

## Coverage

| Module | Mosaic Command Surface | Status |
| --- | --- | --- |
| Core agent | `setup` (`onboard` alias), `configure` (`config` alias, supports `keys/get/set/unset/patch/preview/template`, with `patch --target-profile`, grouped diff output, JSON/TOML template generation), `models` (includes `list --query/--limit` and `resolve`), `ask` (`message` alias, supports stdin via `-`, plus `--prompt-file/--script` including `-` stdin source), `chat` (`agent` alias, REPL `/status` `/agent` `/session` `/new`, supports `--prompt -`, `--prompt-file`, `--script`), `session` (`sessions` alias) | Partial |
| Gateway | `gateway install/start/restart/status/health/probe/discover/call/stop/uninstall` (`daemon` alias), with protocol checks in `gateway health --verbose` (`gateway_discover`, `gateway_protocol_methods`, `gateway_call_status`) | Partial |
| MCP | `mcp list/add/show/check/enable/disable/remove` with local registry + readiness checks (`check --all` batch summary) | Partial |
| Channels | `channels add/update/list/status/login/send/test/logs/capabilities/resolve/export/import/rotate-token-env/remove/logout` | Partial |
| Nodes/devices/pairing | `nodes` (`node` alias), `devices`, `pairing list/request/approve/reject` | Partial |
| Hooks | `hooks list/add/remove/enable/disable/run/logs` | Partial |
| Cron | `cron list/add/remove/enable/disable/run/tick/logs` | Partial |
| Webhooks | `webhooks list/add/remove/enable/disable/trigger/resolve/logs` | Partial |
| Realtime | `tts voices/speak`, `voicecall start/status/send/history/stop` | Partial |
| Browser | `browser start/stop/status/open/navigate/history/tabs/show/focus/snapshot/screenshot/close/clear` | Partial |
| Ops | `logs` (tail/follow/source; includes `plugin:*` event streams), `observability report/export` (includes gateway runtime snapshot + channels delivery telemetry + alert rollups + suppression controls + SLO windows + safety audit summary via `--audit-tail`, optional window diff via `--compare-window`, optional plugin soak trend parsing via `--plugin-soak-report`, plus persisted plugin soak history with retention + `current_vs_previous` deltas), `system event/presence/list` (tail/name filter) | Partial |
| Policy | `approvals` (`acp` alias, includes `check` and `allowlist list`), `sandbox get/set/check/list/explain`, `safety get/check/report` (includes command audit summary via `--audit-tail` and optional diff via `--compare-window`) | Partial |
| Memory | `memory index/search/status/clear/prune/policy` (`index` supports namespace lifecycle + refresh controls via `--namespace/--incremental/--stale-after-hours/--retain-missing` with reuse/reindex/remove counters; `status --all-namespaces`; `prune --max-namespaces/--max-age-hours/--max-documents-per-namespace` with reason breakdown fields; `policy get/set/apply` persists cleanup policy + interval guard; built-in `mosaic.memory.cleanup` / `memory.cleanup` event integration for cron/system/webhook runtime; `search` uses phrase+token+path relevance scoring) | Partial |
| Security | `security audit` (supports `--min-severity/--category/--top` filter dimensions; includes TLS-disable/weak-hash/default-secret checks), `security baseline` | Partial |
| Agents | `agents list/add/update/show/remove/default/route` | Partial |
| Plugins/skills | `plugins` runtime (`list --source/info/check/install/enable/disable/doctor/run/remove`; `run` supports timeout/output-guard/resource-limits/sandbox/approvals policy resolution, unix CPU RLIMIT + configurable cpu-watchdog (`cpu_watchdog_ms`) fallback including non-unix CPU-only enforcement, memory rlimit pre-enforcement on supported unix targets (`RLIMIT_AS` linux/android, `RLIMIT_DATA` BSD) for safe thresholds, non-unix `max_rss_kb` validation guard, plus event+metrics logging), `skills` runtime (`list --source/info/check/install/remove`) | Partial |
| Diagnostics | `dashboard` (operational snapshot), `status`, `health`, `doctor`, `directory` (`--ensure`, `--check-writable`), `completion shell/install` | Partial |
| Maintenance | `update` (semantic compare), `reset`, top-level `uninstall` | Partial |
| Discovery | `docs [topic]`, `dns resolve <host> [--port]` | Partial |
| Distribution | Cross-platform release workflow (`.github/workflows/cli-release.yml`), install scripts (`install.sh`/`install.ps1`), generated Homebrew/Scoop manifests (`mosaic.rb`/`mosaic.json`) | Partial |
| UX shim | `tui` (routes to existing chat runtime) | Partial |
| QR | `qr encode`, `qr pairing` with payload/ascii/png render | Partial |
| Clawbot | `clawbot ask` (supports `--prompt-file` and `--script`, including stdin source `-`), `clawbot chat` (supports `--prompt-file` and `--script`), `clawbot send` (supports `--text-file`, including stdin source `-`), `clawbot status` | Partial |

## Regression Anchors

- Full test/doc inventory: `docs/regression-catalog.md`
- Full runbook: `docs/regression-runbook.md`
- Full from-scratch CLI smoke: `scripts/from_scratch_smoke.sh`
- Change logs: `../../WORKLOG.md` and `docs/progress.md`
