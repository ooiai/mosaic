# Distribution (macOS/Linux/Windows)

This document defines how Mosaic CLI is distributed as installable binaries across macOS, Linux, and Windows.

## Release assets

Each release tag publishes platform archives:

- `mosaic-<version>-darwin-arm64.tar.gz`
- `mosaic-<version>-darwin-x64.tar.gz`
- `mosaic-<version>-linux-x64.tar.gz`
- `mosaic-<version>-windows-x64.zip`
- Per-asset checksums: `*.sha256`
- Aggregate checksums: `SHA256SUMS`

Auxiliary assets:

- `mosaic.rb` (Homebrew formula)
- `mosaic.json` (Scoop manifest)
- `install.sh` (Linux/macOS installer)
- `install.ps1` (Windows installer)

## Install methods

### A. Install from release assets

macOS (Homebrew formula URL):

```bash
brew install https://github.com/ooiai/mosaic/releases/download/<version>/mosaic.rb
```

Linux/macOS (release installer script):

```bash
curl -fsSL https://github.com/ooiai/mosaic/releases/download/<version>/install.sh | bash
```

Windows (release installer script):

```powershell
irm https://github.com/ooiai/mosaic/releases/download/<version>/install.ps1 | iex
```

Local release-assets validation (maintainer smoke):

```bash
./cli/install.sh \
  --version <version> \
  --assets-dir <release-assets-dir> \
  --install-dir <tmp-bin-dir> \
  --release-only
```

```powershell
powershell -ExecutionPolicy Bypass -File .\cli\install.ps1 `
  -Version <version> `
  -AssetsDir <release-assets-dir> `
  -InstallDir <tmp-bin-dir> `
  -ReleaseOnly
```

### B. Install from source (main branch)

macOS (tap + formula in repo):

```bash
brew tap ooiai/mosaic https://github.com/ooiai/mosaic
brew install mosaic
```

Linux/macOS:

```bash
curl -fsSL https://raw.githubusercontent.com/ooiai/mosaic/main/cli/install.sh | bash -s -- --from-source
```

Windows:

```powershell
irm https://raw.githubusercontent.com/ooiai/mosaic/main/cli/install.ps1 -OutFile install.ps1
powershell -ExecutionPolicy Bypass -File .\install.ps1 -FromSource
```

## Release pipeline

Workflow: `.github/workflows/cli-release.yml`

- Triggered by tag push (`v*`) or manual dispatch (`version` input)
- Builds binaries on Linux/macOS/Windows
- Packages platform archives and checksums
- Generates Homebrew/Scoop manifests
- Verifies archive internal contents before publish (`release_verify_archives.sh`)
- Verifies release-assets integrity before publish (`release_verify_assets.sh`)
- Performs installer smoke from assembled release assets on Linux runner (`install.sh --assets-dir ... --release-only`)
- Publishes all release artifacts to GitHub Releases

## Maintainer commands

Package one built target:

```bash
./cli/scripts/package_release_asset.sh --version <version> --target <target-triple>
```

Generate manifests/checksums from collected assets:

```bash
./cli/scripts/update_distribution_manifests.sh \
  --version <version> \
  --assets-dir ./release-assets \
  --output-dir ./release-assets
```

Generate release notes draft from worklog:

```bash
./cli/scripts/release_notes_from_worklog.sh \
  --version <version> \
  --out cli/docs/release-notes-<version>.md
```

One-command local release prepare (checks + notes + single-target asset):

```bash
./cli/scripts/release_prepare.sh --version <version>
```

Release tooling smoke (temporary fixture for verifier pass/fail + dry-run summary):

```bash
./cli/scripts/release_tooling_smoke.sh --version <version>
```

Release installer smoke (local assets + release-only guardrail):

```bash
./cli/scripts/release_install_smoke.sh --version <version>
```

Example with explicit target and summary output:

```bash
./cli/scripts/release_prepare.sh \
  --version <version> \
  --target aarch64-apple-darwin \
  --summary-out reports/release-prepare-summary.json
```

Notes:

- `--assets-dir` manifest generation requires all four archives for that version:
  - `darwin-arm64`, `darwin-x64`, `linux-x64`, `windows-x64`
- If any platform archive is missing, `release_prepare.sh` exits with an explicit missing-assets list.
- If `--assets-dir` is set, `release_prepare.sh` also runs `release_verify_archives.sh` unless `--skip-archive-check` is provided.
- If `--assets-dir` is set, `release_prepare.sh` also runs `release_verify_assets.sh` unless `--skip-verify` is provided.
- `--summary-out` always writes JSON output (normal mode: generated/verified flags; dry-run: planned flags).
- `install.sh` supports local archive installation via `--assets-dir`; combine with `--release-only` to prevent source fallback.
- `install.ps1` supports local archive installation via `-AssetsDir`; combine with `-ReleaseOnly` to prevent source fallback.

Standalone archive content verification:

```bash
./cli/scripts/release_verify_archives.sh --version <version> --assets-dir <dir>
./cli/scripts/release_verify_archives.sh --version <version> --assets-dir <dir> --json
```

Standalone asset verification:

```bash
./cli/scripts/release_verify_assets.sh --version <version> --assets-dir <dir>
./cli/scripts/release_verify_assets.sh --version <version> --assets-dir <dir> --json
```

Published release verification (GitHub release tag):

```bash
./cli/scripts/release_publish_check.sh --version <version>
./cli/scripts/release_publish_check.sh --version <version> --repo ooiai/mosaic --json
```
