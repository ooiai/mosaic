# Beta Release Guide (CLI)

This guide defines how to freeze scope, run readiness gates, and package an internal beta build.

## 1) Freeze Scope

Scope source of truth:

- `/Users/jerrychir/Desktop/dev/coding/ooiai/mosaic/planing.md`
- `/Users/jerrychir/Desktop/dev/coding/ooiai/mosaic/cli/docs/parity-map.md`

For beta, treat command surface as frozen. New features are deferred after release unless they are blocking fixes.

## 2) Readiness Gate

Run full beta gate:

```bash
cd /Users/jerrychir/Desktop/dev/coding/ooiai/mosaic/cli
./scripts/beta_release_check.sh
```

It runs:

1. `cargo fmt --all --check`
2. `cargo check -p mosaic-cli`
3. `cargo clippy -p mosaic-cli -- -D warnings`
4. `cargo test --workspace`
5. Contract gates: `command_surface`, `help_snapshot`, `error_codes`, `json_contract`, `json_contract_modules`
6. End-to-end smoke: `SKIP_WORKSPACE_TESTS=1 ./scripts/from_scratch_smoke.sh`
7. Regression inventory refresh: `./scripts/update_regression_catalog.sh`

Reports:

- `cli/reports/beta-readiness-<timestamp>.log`
- `cli/reports/beta-readiness-latest.log`

## 3) Package Internal Beta

```bash
cd /Users/jerrychir/Desktop/dev/coding/ooiai/mosaic/cli
./scripts/package_beta.sh --version v0.2.0-beta.1
```

Output:

- unpacked: `cli/dist/v0.2.0-beta.1/mosaic-cli-v0.2.0-beta.1-<os>-<arch>/`
- archive: `cli/dist/v0.2.0-beta.1/mosaic-cli-v0.2.0-beta.1-<os>-<arch>.tar.gz`

## 4) Internal Beta Checklist

1. Scope freeze merged to `main`.
2. Beta readiness gate is green.
3. Release note document generated with known limitations.
4. At least one clean install/launch check on target machines.
5. Bug triage window defined before widening rollout.

## 5) Definition of 100% (Beta)

A module is treated as 100% for beta when all are true:

1. Command surface exists and is documented.
2. JSON success/failure contracts are covered.
3. Error codes are covered.
4. Included in from-scratch smoke path.
5. No failing items in beta readiness report.

Anything outside beta scope is tracked as post-beta backlog and does not block release.

## 6) Cross-platform Distribution

For installable delivery (macOS/Linux/Windows), use:

- workflow: `.github/workflows/cli-release.yml`
- packaging script: `cli/scripts/package_release_asset.sh`
- manifest generator: `cli/scripts/update_distribution_manifests.sh`
- installers: `cli/install.sh`, `cli/install.ps1`

Release assets include:

- `mosaic-<tag>-darwin-arm64.tar.gz`
- `mosaic-<tag>-darwin-x64.tar.gz`
- `mosaic-<tag>-linux-x64.tar.gz`
- `mosaic-<tag>-windows-x64.zip`
- `mosaic.rb` (Homebrew), `mosaic.json` (Scoop), `SHA256SUMS`
