# Mosaic CLI Coverage Map

This file tracks Mosaic CLI command/module coverage and pending gaps.

## Coverage

| Module | Mosaic Command Surface | Status |
| --- | --- | --- |
| Core agent | `setup` (`onboard` alias), `configure` (`config` alias), `models`, `ask` (`message` alias), `chat` (`agent` alias), `session` (`sessions` alias) | Partial |
| Gateway | `gateway install/start/restart/status/health/probe/discover/call/stop/uninstall` (`daemon` alias) | Partial |
| Channels | `channels add/update/list/status/login/send/test/logs/capabilities/resolve/export/import/rotate-token-env/remove/logout` | Partial |
| Nodes/devices/pairing | `nodes` (`node` alias), `devices`, `pairing list/request/approve` | Partial |
| Hooks | `hooks list/add/remove/enable/disable/run/logs` | Partial |
| Cron | `cron list/add/remove/enable/disable/run/tick/logs` | Partial |
| Webhooks | `webhooks list/add/remove/enable/disable/trigger/resolve/logs` | Partial |
| Browser | `browser open/history/show/clear` | Partial |
| Ops | `logs`, `system event/presence` | Partial |
| Policy | `approvals` (`acp` alias), `sandbox` | Partial |
| Memory | `memory index/search/status` | Partial |
| Security | `security audit`, `security baseline` | Partial |
| Agents | `agents list/add/update/show/remove/default/route` | Partial |
| Plugins/skills | `plugins`, `skills` minimal runtime (`list/info/check/install/remove`) | Partial |
| Diagnostics | `dashboard`, `status`, `health`, `doctor`, `directory`, `completion shell/install` | Partial |
| Maintenance | `update`, `reset`, top-level `uninstall` | Partial |
| Discovery | `docs [topic]`, `dns resolve <host> [--port]` | Partial |
| UX shim | `tui` (routes to existing chat runtime) | Partial |
| QR | `qr encode`, `qr pairing` with payload/ascii/png render | Partial |
| Clawbot | `clawbot ask`, `clawbot chat`, `clawbot send`, `clawbot status` | Partial |

## Regression Anchors

- Full test/doc inventory: `docs/regression-catalog.md`
- Full runbook: `docs/regression-runbook.md`
- Full from-scratch CLI smoke: `scripts/from_scratch_smoke.sh`
- Change logs: `../../WORKLOG.md` and `docs/progress.md`
