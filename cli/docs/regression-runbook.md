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
cargo test -p mosaic-cli --test help_snapshot
cargo test -p mosaic-cli --test webhooks_ops
cargo test -p mosaic-cli --test browser_ops
```

## 5) Full Smoke Only (skip workspace tests)

```bash
cd cli
SKIP_WORKSPACE_TESTS=1 ./scripts/from_scratch_smoke.sh
```

## 6) CI Alignment

For local parity with CI test gates:

```bash
cd cli
./scripts/run_regression_suite.sh
```

CI uploads latest regression report artifact:

- `rust-cli-regression-report` (file: `cli/reports/regression-latest.log`)
- `rust-cli-Linux-binary` (file: `cli/target/release/mosaic`)
- `rust-cli-macOS-binary` (file: `cli/target/release/mosaic`)
- `rust-cli-Windows-binary` (file: `cli/target/release/mosaic.exe`)

## 7) Pre-merge Checklist

1. Run `./scripts/update_regression_catalog.sh`
2. Run `./scripts/run_regression_suite.sh`
3. Run `cargo clippy -p mosaic-cli -- -D warnings`
4. Confirm `docs/regression-catalog.md` is updated and committed
5. Confirm `reports/regression-latest.log` shows no failures
6. Append concise change note:

```bash
cd cli
./scripts/worklog_append.sh --summary "What changed" --tests "cargo test --workspace"
```

Canonical log files:

- `../../WORKLOG.md`
- `docs/progress.md`
