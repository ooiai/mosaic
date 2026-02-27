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
    - `models`
    - `channels`
    - `channels/admin` (`update`, `login`, `export`, `import`, `rotate-token-env`, `logout`, `remove`)
    - `gateway`
    - `ops/policy` (`approvals`, `sandbox`, `system`, `logs`)
    - `automation` (`hooks`, `cron`, `webhooks`)
    - `features` (`browser`, `memory`, `plugins`, `skills`)
    - `compat/discovery/maintenance` (`docs`, `dns`, `tui`, `qr`, `clawbot`, `directory`, `dashboard`, `update`, `reset`, `uninstall`)
    - `security`
    - `agents`
    - `nodes/pairing`
  - Snapshot files:
    - `tests/snapshots/json_module_models_schema.json`
    - `tests/snapshots/json_module_channels_schema.json`
    - `tests/snapshots/json_module_channels_admin_schema.json`
    - `tests/snapshots/json_module_gateway_schema.json`
    - `tests/snapshots/json_module_ops_policy_schema.json`
    - `tests/snapshots/json_module_automation_schema.json`
    - `tests/snapshots/json_module_features_schema.json`
    - `tests/snapshots/json_module_compat_discovery_maintenance_schema.json`
    - `tests/snapshots/json_module_security_schema.json`
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
