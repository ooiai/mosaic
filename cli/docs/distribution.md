# Distribution (macOS/Linux/Windows)

This document defines how Mosaic CLI is distributed as installable binaries across macOS, Linux, and Windows.

## Release assets

Each release tag publishes platform assets:

- `mosaic-<version>-darwin-arm64.tar.gz`
- `mosaic-<version>-darwin-x64.tar.gz`
- `mosaic-<version>-linux-x64.tar.gz`
- `mosaic-<version>-windows-x64.zip`
- Per-asset checksums: `*.sha256`
- Aggregate checksums: `SHA256SUMS`

Auxiliary install assets:

- `mosaic.rb` (Homebrew formula)
- `mosaic.json` (Scoop manifest)
- `install.sh` (Linux/macOS installer)
- `install.ps1` (Windows installer)

## Install methods

### macOS (Homebrew)

```bash
brew install https://github.com/ooiai/mosaic/releases/latest/download/mosaic.rb
```

### Linux / macOS (script)

```bash
curl -fsSL https://github.com/ooiai/mosaic/releases/latest/download/install.sh | bash
```

Pin version:

```bash
curl -fsSL https://github.com/ooiai/mosaic/releases/latest/download/install.sh | bash -s -- --version v0.2.0-beta.5
```

### Windows (Scoop or installer script)

Scoop:

```powershell
scoop install https://github.com/ooiai/mosaic/releases/latest/download/mosaic.json
```

Installer script:

```powershell
irm https://github.com/ooiai/mosaic/releases/latest/download/install.ps1 | iex
```

## Release pipeline

Workflow: `.github/workflows/cli-release.yml`

- Triggered by:
  - tag push: `v*`
  - manual dispatch with `version`
- Builds target binaries and assets on Linux/macOS/Windows
- Generates `mosaic.rb`, `mosaic.json`, and `SHA256SUMS`
- Publishes all assets into the GitHub release

## Local maintainer commands

Create one platform asset locally after building target binary:

```bash
./cli/scripts/package_release_asset.sh --version v0.2.0-beta.5 --target aarch64-apple-darwin
```

Generate manifests/checksums from collected release assets:

```bash
./cli/scripts/update_distribution_manifests.sh \
  --version v0.2.0-beta.5 \
  --assets-dir ./release-assets \
  --output-dir ./release-assets
```
