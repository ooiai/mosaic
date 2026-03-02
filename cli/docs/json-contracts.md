# JSON Contracts

This document defines JSON output contract coverage for Mosaic CLI.

## Test Suites

- `tests/error_codes.rs`
  - Validates `--json` error envelope with expected error codes and process exit codes.
  - Covers config/auth/validation/gateway/policy/sandbox failures.
- `tests/help_snapshot.rs`
  - Locks `--help` output snapshots for root/channels/gateway.
- `tests/json_contract.rs`
  - Locks baseline success/failure JSON envelope schemas.
  - Snapshot files:
    - `tests/snapshots/json_success_schema.json`
    - `tests/snapshots/json_failure_schema.json`
- `tests/json_contract_modules.rs`
  - Locks module-level success JSON schemas.
  - Covered modules:
    - `core/agent` (`setup`, `configure --show/keys/get/set/unset/patch`, `ask`, `chat --prompt`, `session list/show/clear`, `status`, `health`, `doctor`)
    - `models` (`list` and `list --query/--limit` include `query`, `limit`, `total_models`, `matched_models`, `returned_models`)
    - `channels`
    - `channels/admin` (`update`, `login`, `export`, `import`, `rotate-token-env`, `logout`, `remove`)
    - `gateway`
    - `gateway/admin` (`install`, `start`, `status --deep`, `health --verbose`, `restart`, `uninstall`)
    - `ops/policy` (`approvals get/set/check/allowlist add|list|remove`, `sandbox get/set/check/list/explain`, `safety get/check/report`, `observability report/export`, `system event/presence/list`, `logs`)
    - `automation` (`hooks`, `cron`, `webhooks`)
    - `features` (`browser start/status/navigate/history/tabs/show/focus/snapshot/screenshot/clear/close/stop`, `memory index/search/status/clear`, `plugins` doctor/toggle/run flow including run timeout/output-guard/sandbox/approval/resource-limit/resource-metrics/event-log fields, `plugins/skills` list source filters)
    - `compat/discovery/maintenance` (`docs`, `dns`, `tui`, `qr`, `clawbot`, `directory` + diagnostics flags, `dashboard`, `update` + same-version check, `reset`, `uninstall`)
    - `security`
    - `security/baseline` (`show`, `add`, `remove`, `clear`)
    - `agents`
    - `nodes/pairing`
  - Snapshot files:
    - `tests/snapshots/json_module_core_agent_schema.json`
    - `tests/snapshots/json_module_models_schema.json`
    - `tests/snapshots/json_module_channels_schema.json`
    - `tests/snapshots/json_module_channels_admin_schema.json`
    - `tests/snapshots/json_module_gateway_schema.json`
    - `tests/snapshots/json_module_gateway_admin_schema.json`
    - `tests/snapshots/json_module_ops_policy_schema.json`
    - `tests/snapshots/json_module_automation_schema.json`
    - `tests/snapshots/json_module_features_schema.json`
    - `tests/snapshots/json_module_compat_discovery_maintenance_schema.json`
    - `tests/snapshots/json_module_security_schema.json`
    - `tests/snapshots/json_module_security_baseline_schema.json`
    - `tests/snapshots/json_module_agents_schema.json`
    - `tests/snapshots/json_module_nodes_pairing_schema.json`

## Local Commands

```bash
cd cli
cargo test -p mosaic-cli --test error_codes
cargo test -p mosaic-cli --test json_contract
cargo test -p mosaic-cli --test json_contract_modules
cargo test -p mosaic-cli --test help_snapshot
```

Project-level shortcut:

```bash
make cli-json-contract
```

## Snapshot Update Policy

- Update snapshot files only when JSON contract changes are intentional.
- Keep snapshot changes in the same commit as command/runtime changes.
- Optional local refresh helper:
  - `MOSAIC_UPDATE_SNAPSHOTS=1 cargo test -p mosaic-cli --test json_contract_modules`
- Always run:
  - `make cli-json-contract`
  - `./scripts/run_regression_suite.sh`
