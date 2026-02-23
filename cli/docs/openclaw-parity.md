# OpenClaw Parity Map (CLI)

This file tracks Mosaic CLI parity against the targeted OpenClaw-style module set.

## Coverage

| Module | Mosaic Command Surface | Status |
| --- | --- | --- |
| Core agent | `setup`, `configure`, `models`, `ask`, `chat`, `session` | Implemented |
| Gateway | `gateway install/start/restart/status/health/probe/discover/call/stop/uninstall` | Implemented |
| Channels | `channels add/update/list/status/login/send/test/logs/capabilities/resolve/export/import/rotate-token-env/remove/logout` | Implemented |
| Nodes/devices/pairing | `nodes`, `devices`, `pairing list/request/approve` | Implemented |
| Hooks | `hooks list/add/remove/enable/disable/run/logs` | Implemented |
| Cron | `cron list/add/remove/enable/disable/run/tick/logs` | Implemented |
| Webhooks | `webhooks list/add/remove/enable/disable/trigger/resolve/logs` | Implemented |
| Browser | `browser open/history/show/clear` | Implemented |
| Ops | `logs`, `system event/presence` | Implemented |
| Policy | `approvals`, `sandbox` | Implemented |
| Memory | `memory index/search/status` | Implemented |
| Security | `security audit`, `security baseline` | Implemented |
| Agents | `agents list/add/update/show/remove/default/route` | Implemented |
| Plugins/skills | `plugins`, `skills` minimal runtime (`list/info/check/install/remove`) | Implemented |
| Diagnostics | `status`, `health`, `doctor` | Implemented |

## Regression Anchors

- Full test/doc inventory: `docs/regression-catalog.md`
- Full runbook: `docs/regression-runbook.md`
- Full from-scratch CLI smoke: `scripts/from_scratch_smoke.sh`
- Concise change timeline: `../../WORKLOG.md`
