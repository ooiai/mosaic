#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<USAGE
Usage: $0 --version <version> [--skip-check]

Examples:
  $0 --version v0.2.0-beta.1
  $0 --version v0.2.0-beta.1 --skip-check
USAGE
}

VERSION=""
SKIP_CHECK=0

while [[ $# -gt 0 ]]; do
  case "$1" in
    --version)
      VERSION="${2:-}"
      shift 2
      ;;
    --skip-check)
      SKIP_CHECK=1
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "error: unknown argument '$1'" >&2
      usage >&2
      exit 1
      ;;
  esac
done

if [[ -z "$VERSION" ]]; then
  echo "error: --version is required" >&2
  usage >&2
  exit 1
fi

CLI_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ROOT_DIR="$(cd "$CLI_DIR/.." && pwd)"
cd "$CLI_DIR"

if [[ "$SKIP_CHECK" != "1" ]]; then
  "$CLI_DIR/scripts/beta_release_check.sh"
fi

cargo build --release -p mosaic-cli

OS="$(uname -s | tr '[:upper:]' '[:lower:]')"
ARCH="$(uname -m | tr '[:upper:]' '[:lower:]')"
DIST_DIR="$CLI_DIR/dist/${VERSION}"
PKG_NAME="mosaic-cli-${VERSION}-${OS}-${ARCH}"
PKG_DIR="$DIST_DIR/$PKG_NAME"
mkdir -p "$PKG_DIR"

cp "$CLI_DIR/target/release/mosaic" "$PKG_DIR/"
cp "$CLI_DIR/README.md" "$PKG_DIR/"
cp "$CLI_DIR/docs/regression-runbook.md" "$PKG_DIR/"
cp "$CLI_DIR/docs/parity-map.md" "$PKG_DIR/"
cp "$CLI_DIR/docs/beta-release.md" "$PKG_DIR/"

cat > "$PKG_DIR/RELEASE-NOTES.md" <<NOTES
# ${VERSION}

## Scope
- CLI-only beta build
- Command surface freeze based on /Users/jerrychir/Desktop/dev/coding/ooiai/mosaic/planing.md

## Verification
- beta readiness report: cli/reports/beta-readiness-latest.log
- regression catalog: cli/docs/regression-catalog.md

## Install
\`chmod +x mosaic && ./mosaic --help\`
NOTES

(
  cd "$DIST_DIR"
  tar -czf "${PKG_NAME}.tar.gz" "$PKG_NAME"
)

echo "package dir: $PKG_DIR"
echo "archive: $DIST_DIR/${PKG_NAME}.tar.gz"
echo "source root: $ROOT_DIR"
