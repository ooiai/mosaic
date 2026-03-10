# Regression Runbook

This runbook describes how to run and maintain full CLI regression checks.

## 1) Update Full Inventory (all docs + all tests)

```bash
cd cli
./scripts/update_regression_catalog.sh
```

Generated file:

- `docs/regression-catalog.md`

The catalog includes:

- Every Markdown document under `docs/`
- Every Rust test case discovered by `cargo test --workspace -- --list`

## 2) Run Full Regression Suite

```bash
cd cli
./scripts/run_regression_suite.sh
# optional: append concise entries to ../../WORKLOG.md and docs/progress.md after success
./scripts/run_regression_suite.sh --worklog-summary "Nightly full regression"
```

This executes:

1. Catalog refresh (`update_regression_catalog.sh`)
2. `cargo test --workspace`
3. End-to-end full-module smoke script (`scripts/from_scratch_smoke.sh`)

Reports:

- Timestamped log: `reports/regression-<UTCSTAMP>.log`
- Latest symlink/copy: `reports/regression-latest.log`

## 3) Quality Gate (lint/idioms)

```bash
cd cli
cargo clippy -p mosaic-cli -- -D warnings
```

## 4) Fast Targeted Regression

```bash
cd cli
cargo test -p mosaic-cli --test cron_ops
cargo test -p mosaic-cli --test hooks_ops
cargo test -p mosaic-cli --test gateway_channels
cargo test -p mosaic-cli --test command_surface
cargo test -p mosaic-cli --test aliases_ops
cargo test -p mosaic-cli --test ask_stdin_ops
cargo test -p mosaic-cli --test chat_repl_ops
cargo test -p mosaic-cli --test prompt_file_ops
cargo test -p mosaic-cli --test qr_clawbot_ops
cargo test -p mosaic-cli --test error_codes
cargo test -p mosaic-cli --test json_contract
cargo test -p mosaic-cli --test json_contract_modules
cargo test -p mosaic-cli --test help_snapshot
cargo test -p mosaic-cli --test webhooks_ops
cargo test -p mosaic-cli --test browser_ops
cargo test -p mosaic-cli --test models_ops
cargo test -p mosaic-cli --test nodes_devices_pairing
cargo test -p mosaic-cli --test mcp_ops
cargo test -p mosaic-mcp
```

## 5) Full Smoke Only (skip workspace tests)

```bash
cd cli
SKIP_WORKSPACE_TESTS=1 ./scripts/from_scratch_smoke.sh
```

## 6) Plugin Runtime Soak (resource policies)

```bash
cd cli
ITERATIONS=200 ./scripts/plugin_resource_soak.sh
```

Useful overrides:

- `CPU_TIMEOUT_MS=5000`
- `MOSAIC_BIN=/path/to/mosaic`
- `KEEP_TMP=1`

## 7) CI Alignment

For local parity with CI test gates:

```bash
cd cli
./scripts/run_regression_suite.sh
```

`run_regression_suite.sh` now includes:

- `update_regression_catalog.sh`
- `release_tooling_smoke.sh`
- `release_install_smoke.sh`
- `cargo test --workspace`
- `from_scratch_smoke.sh` (with `SKIP_WORKSPACE_TESTS=1`)

CI uploads latest regression report artifact:

- `rust-cli-regression-report` (file: `cli/reports/regression-latest.log`)
- `rust-cli-Linux-binary` (file: `cli/target/release/mosaic`)
- `rust-cli-macOS-binary` (file: `cli/target/release/mosaic`)
- `rust-cli-Windows-binary` (file: `cli/target/release/mosaic.exe`)
- `rust-cli-plugin-soak-report` (file: `cli/reports/plugin-soak-latest.log`, produced by nightly schedule or manual `workflow_dispatch` with `run_plugin_soak=true`)

## 8) Pre-merge Checklist

1. Run `./scripts/update_regression_catalog.sh`
2. Run `./scripts/release_tooling_smoke.sh`
3. Run `./scripts/release_install_smoke.sh`
4. Run `./scripts/run_regression_suite.sh`
5. Run `cargo clippy -p mosaic-cli -- -D warnings`
6. Confirm `docs/regression-catalog.md` is updated and committed
7. Confirm `reports/regression-latest.log` shows no failures
8. Append concise change note:

```bash
cd cli
./scripts/worklog_append.sh --summary "What changed" --tests "cargo test --workspace"
```

Canonical log files:

- `../../WORKLOG.md`
- `docs/progress.md`

## 9) Beta Freeze / Package

```bash
cd cli
./scripts/beta_release_check.sh
./scripts/package_beta.sh --version <version>
```

## 10) Cross-platform Release Packaging

```bash
cd cli

# package one target (after building that target)
./scripts/package_release_asset.sh --version <version> --target aarch64-apple-darwin

# generate Homebrew/Scoop manifests from collected assets
./scripts/update_distribution_manifests.sh \
  --version <version> \
  --assets-dir ./dist/<version> \
  --output-dir ./dist/<version>
```

GitHub release automation:

- workflow: `../.github/workflows/cli-release.yml`
- release assets include:
  - platform archives (`darwin/linux/windows`)
  - checksums (`*.sha256`, `SHA256SUMS`)
  - installers (`install.sh`, `install.ps1`)
  - package manifests (`mosaic.rb`, `mosaic.json`)

## 11) Release Notes Draft

```bash
cd cli
./scripts/release_notes_from_worklog.sh \
  --version <version> \
  --out docs/release-notes-<version>.md
```

Optional filters:

- `--from-date 2026-03-01T00:00:00Z`
- `--max-entries 30`

## 12) Local Release Prepare (One Command)

```bash
cd cli
./scripts/release_prepare.sh --version <version>
```

Optional release-tooling smoke:

```bash
cd cli
./scripts/release_tooling_smoke.sh --version <version>
./scripts/release_install_smoke.sh --version <version>
```

Common variants:

- `--target aarch64-apple-darwin`
- `--skip-check` (skip beta readiness checks)
- `--skip-archive-check` (skip archive content verification when `--assets-dir` is set)
- `--skip-verify` (skip assets verification when `--assets-dir` is set)
- `--assets-dir ../release-assets --output-dir ../release-assets`
- `--summary-out reports/release-prepare-summary.json`
- `--summary-out` is written in both normal and dry-run modes (dry-run fields are marked as planned)
- `--assets-dir` requires a full release matrix (`darwin-arm64/darwin-x64/linux-x64/windows-x64`) for the same version

## 13) Release Archive Verification (Standalone)

```bash
cd cli
./scripts/release_verify_archives.sh --version <version> --assets-dir <dir>
./scripts/release_verify_archives.sh --version <version> --assets-dir <dir> --json
```

## 14) Release Asset Verification (Standalone)

```bash
cd cli
./scripts/release_verify_assets.sh --version <version> --assets-dir <dir>
./scripts/release_verify_assets.sh --version <version> --assets-dir <dir> --json
```

## 15) Installer Smoke From Assembled Assets

```bash
cd cli
./install.sh --version <version> --assets-dir <dir> --install-dir <tmp-bin-dir> --release-only
```

## 16) Published Release Verification

```bash
cd cli
./scripts/release_publish_check.sh --version <version>
./scripts/release_publish_check.sh --version <version> --repo ooiai/mosaic --json
```
