# Mosaic macOS App

Native macOS desktop client for Mosaic.

## Architecture

- `Domain`: app-facing entities and protocols
- `Infrastructure`: CLI bridge, process execution, workspace persistence
- `Features`: view models and state mapping
- `UI`: SwiftUI views and theme tokens
- `MosaicMacApp`: executable entry point

## Build

```bash
cd cli
cargo build --release -p mosaic-cli

cd apps/macos
swift build
swift test
swift run MosaicMacApp
```

## Package

Create a distributable `.app` bundle plus ZIP:

```bash
./apps/macos/scripts/package_app.sh
```

Outputs land in `apps/macos/dist/`:

- `Mosaic.app`
- `Mosaic-macOS.zip`

Optional release env vars:

- `MACOS_SIGN_IDENTITY`
- `MACOS_NOTARIZE=1`
- `MACOS_NOTARY_PROFILE`
- `APP_VERSION`
- `APP_BUILD`

CI will always produce an unsigned `.app` + ZIP. If `MACOS_SIGN_IDENTITY` is available, the packaging step signs the nested CLI and app bundle. If the run is for a Git tag and `MACOS_NOTARY_PROFILE` is also available, the same script notarizes and staples the app before rebuilding the ZIP.

## Runtime

The app embeds a `MosaicRuntimeClient` abstraction. The production implementation shells out to:

```bash
mosaic --project-state --json ...
```

Runtime lookup order:

1. `MOSAIC_CLI_PATH`
2. `Contents/Resources/bin/mosaic` inside a macOS app bundle
3. `./bin/mosaic` next to the built `MosaicMacApp` binary
4. `../../cli/target/release/mosaic` in the monorepo
5. `/usr/local/bin/mosaic`
