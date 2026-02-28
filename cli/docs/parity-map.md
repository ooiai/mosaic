# Mosaic CLI Coverage Map

This file tracks Mosaic CLI command/module coverage and pending gaps.

## Coverage

| Module | Mosaic Command Surface | Status |
| --- | --- | --- |
| Core agent | `setup` (`onboard` alias), `configure` (`config` alias), `models` (includes `list --query/--limit` and `resolve`), `ask` (`message` alias, supports stdin via `-`, plus `--prompt-file/--script` including `-` stdin source), `chat` (`agent` alias, REPL `/status` `/agent` `/session` `/new`, supports `--prompt -`, `--prompt-file`, `--script`), `session` (`sessions` alias) | Partial |
| Gateway | `gateway install/start/restart/status/health/probe/discover/call/stop/uninstall` (`daemon` alias) | Partial |
| Channels | `channels add/update/list/status/login/send/test/logs/capabilities/resolve/export/import/rotate-token-env/remove/logout` | Partial |
| Nodes/devices/pairing | `nodes` (`node` alias), `devices`, `pairing list/request/approve/reject` | Partial |
| Hooks | `hooks list/add/remove/enable/disable/run/logs` | Partial |
| Cron | `cron list/add/remove/enable/disable/run/tick/logs` | Partial |
| Webhooks | `webhooks list/add/remove/enable/disable/trigger/resolve/logs` | Partial |
| Browser | `browser start/stop/status/open/navigate/history/tabs/show/focus/snapshot/screenshot/close/clear` | Partial |
| Ops | `logs` (tail/follow/source), `system event/presence/list` (tail/name filter) | Partial |
| Policy | `approvals` (`acp` alias, includes `check` and `allowlist list`), `sandbox get/set/check/list/explain` | Partial |
| Memory | `memory index/search/status/clear` | Partial |
| Security | `security audit`, `security baseline` | Partial |
| Agents | `agents list/add/update/show/remove/default/route` | Partial |
| Plugins/skills | `plugins`, `skills` minimal runtime (`list --source/info/check/install/remove`) | Partial |
| Diagnostics | `dashboard` (operational snapshot), `status`, `health`, `doctor`, `directory` (`--ensure`, `--check-writable`), `completion shell/install` | Partial |
| Maintenance | `update` (semantic compare), `reset`, top-level `uninstall` | Partial |
| Discovery | `docs [topic]`, `dns resolve <host> [--port]` | Partial |
| UX shim | `tui` (routes to existing chat runtime) | Partial |
| QR | `qr encode`, `qr pairing` with payload/ascii/png render | Partial |
| Clawbot | `clawbot ask` (supports `--prompt-file` and `--script`, including stdin source `-`), `clawbot chat` (supports `--prompt-file` and `--script`), `clawbot send` (supports `--text-file`, including stdin source `-`), `clawbot status` | Partial |

## Regression Anchors

- Full test/doc inventory: `docs/regression-catalog.md`
- Full runbook: `docs/regression-runbook.md`
- Full from-scratch CLI smoke: `scripts/from_scratch_smoke.sh`
- Change logs: `../../WORKLOG.md` and `docs/progress.md`
