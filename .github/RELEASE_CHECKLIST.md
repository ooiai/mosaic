# Mosaic CLI Release Checklist

This checklist is the canonical release gate for the Rust CLI (`cli/`) and distribution assets.

## 1. Scope And Version

- [ ] Confirm release scope is CLI-only (no unrelated web/desktop changes)
- [ ] Confirm target version tag (for example: `v0.2.0-beta.6`)
- [ ] Confirm `cli/Cargo.toml` and `cli/Cargo.lock` version alignment
- [ ] Confirm `README.md`, `README_CN.md`, and `cli/README.md` mention the new release where needed

## 2. Quality Gates (must pass)

- [ ] `cd cli && cargo test --workspace`
- [ ] `cd cli && cargo clippy -p mosaic-cli -- -D warnings`
- [ ] `cd cli && ./scripts/run_regression_suite.sh`
- [ ] `bash site/scripts/check_docs.sh --report-dir reports --report-prefix release-docs-check`
- [ ] Verify no failing snapshots/help contracts (`json_contract*`, `help_snapshot`, `command_surface`)

## 3. Feature-Specific Gates (if touched)

- [ ] TUI changes: run `cargo test -p mosaic-tui -p mosaic-cli --test tui_ops --test tui_interactive`
- [ ] Channels/gateway changes: run `cargo test -p mosaic-cli --test gateway_channels`
- [ ] Policy/runtime changes: run `cargo test -p mosaic-cli --test policy_ops --test error_codes`
- [ ] Knowledge/memory changes: run `cargo test -p mosaic-cli --test knowledge_ops --test memory_ops`

## 4. Release Notes And Logs

- [ ] Generate draft release notes from `WORKLOG.md`:
  - `cd cli && ./scripts/release_notes_from_worklog.sh --version <tag> --out docs/release-notes-<tag>.md`
- [ ] Manually prune noisy entries and ensure user-facing wording
- [ ] Ensure migration/breaking-change section is explicit (or state "none")
- [ ] Update `cli/docs/progress.md` with final verification summary

## 5. Packaging And Manifests

- [ ] Build release binaries for all targets via workflow or local matrix
- [ ] Optional local one-command prepare:
  - `cd cli && ./scripts/release_prepare.sh --version <tag>`
- [ ] Optional local release tooling smoke:
  - `cd cli && ./scripts/release_tooling_smoke.sh --version <tag>`
- [ ] Optional local release install smoke:
  - `cd cli && ./scripts/release_install_smoke.sh --version <tag>`
- [ ] Verify packaged archive internals:
  - `cd cli && ./scripts/release_verify_archives.sh --version <tag> --assets-dir <dir>`
- [ ] Verify assembled release assets directory:
  - `cd cli && ./scripts/release_verify_assets.sh --version <tag> --assets-dir <dir>`
- [ ] Smoke installer against assembled local assets:
  - `cd cli && ./install.sh --version <tag> --assets-dir <dir> --install-dir <tmp-bin> --release-only`
- [ ] Package release assets:
  - `cd cli && ./scripts/package_release_asset.sh --version <tag> --target <triple>`
- [ ] Generate Homebrew/Scoop manifests and checksums:
  - `cd cli && ./scripts/update_distribution_manifests.sh --version <tag> --assets-dir <dir> --output-dir <dir>`
- [ ] Verify checksum files (`*.sha256`, `SHA256SUMS`) are present and consistent

## 6. Publish

- [ ] Push tag (`git tag <tag> && git push origin <tag>`) or run `workflow_dispatch`
- [ ] Confirm `.github/workflows/cli-release.yml` completed on all matrix jobs
- [ ] Confirm release page contains all archives/manifests/installers
- [ ] Smoke install from release assets:
  - macOS: Homebrew formula URL install
  - Linux/macOS: `install.sh`
  - Windows: `install.ps1`

## 7. Post-Release

- [ ] Run one post-release smoke:
  - `mosaic --version`
  - `mosaic --help`
  - `mosaic --project-state --json status`
- [ ] Verify published release assets by tag:
  - `cd cli && ./scripts/release_publish_check.sh --version <tag>`
- [ ] Record release note link and verification summary into `WORKLOG.md`
- [ ] Open next milestone items in `planing.md`

## 8. Hotfix Process

- [ ] Branch from `main` using `codex/hotfix-<tag>`
- [ ] Apply minimal fix and rerun targeted + full critical tests
- [ ] Tag patch (`vX.Y.Z` or next beta tag)
- [ ] Publish with same packaging/verification flow
