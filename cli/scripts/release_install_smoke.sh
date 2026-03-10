#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<USAGE
Usage: $0 [--version <tag>]

Builds a local release-archive fixture and validates installer behavior:
  - install.sh local assets path (`--assets-dir`) with `--release-only`
  - install.sh release-only failure path when archive is missing
  - install.ps1 syntax parsing (if pwsh is available)
USAGE
}

CLI_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

resolve_default_version() {
  local cargo_toml="$CLI_DIR/crates/mosaic-cli/Cargo.toml"
  local parsed=""
  if [[ -f "$cargo_toml" ]]; then
    parsed="$(awk -F'"' '/^version[[:space:]]*=[[:space:]]*"/ { print $2; exit }' "$cargo_toml" || true)"
  fi
  if [[ -n "$parsed" ]]; then
    printf 'v%s' "$parsed"
  else
    printf 'v0.2.0-beta.6'
  fi
}

VERSION="$(resolve_default_version)"
while [[ $# -gt 0 ]]; do
  case "$1" in
    --version)
      VERSION="${2:-}"
      shift 2
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

for required_cmd in bash tar awk grep find mktemp cp; do
  if ! command -v "$required_cmd" >/dev/null 2>&1; then
    echo "error: required command '$required_cmd' is not available in PATH" >&2
    exit 1
  fi
done

detect_platform() {
  local os arch
  os="$(uname -s)"
  arch="$(uname -m)"
  case "$os" in
    Darwin)
      case "$arch" in
        arm64|aarch64) echo "darwin-arm64" ;;
        x86_64) echo "darwin-x64" ;;
        *) echo "unsupported" ;;
      esac
      ;;
    Linux)
      case "$arch" in
        x86_64) echo "linux-x64" ;;
        *) echo "unsupported" ;;
      esac
      ;;
    *)
      echo "unsupported"
      ;;
  esac
}

PLATFORM="$(detect_platform)"
if [[ "$PLATFORM" == "unsupported" ]]; then
  echo "release_install_smoke: SKIP (unsupported platform $(uname -s)/$(uname -m))"
  exit 0
fi

TMP_DIR="$(mktemp -d)"
ASSETS_DIR="$TMP_DIR/release-assets"
INSTALL_DIR="$TMP_DIR/install/bin"
mkdir -p "$ASSETS_DIR" "$INSTALL_DIR"

cleanup() {
  rm -rf "$TMP_DIR"
}
trap cleanup EXIT

ARCHIVE_NAME="mosaic-${VERSION}-${PLATFORM}.tar.gz"
FIXTURE_DIR="$ASSETS_DIR/mosaic-${VERSION}-${PLATFORM}"
mkdir -p "$FIXTURE_DIR"

cat > "$FIXTURE_DIR/mosaic" <<'BINARY'
#!/usr/bin/env bash
if [[ "${1:-}" == "--help" ]]; then
  echo "mosaic smoke fixture help"
  exit 0
fi
echo "mosaic smoke fixture"
BINARY
chmod +x "$FIXTURE_DIR/mosaic"

for doc in README.md install.sh install.ps1 parity-map.md regression-runbook.md distribution.md; do
  printf 'fixture\n' > "$FIXTURE_DIR/$doc"
done

(cd "$ASSETS_DIR" && tar -czf "$ARCHIVE_NAME" "mosaic-${VERSION}-${PLATFORM}")
rm -rf "$FIXTURE_DIR"

bash "$CLI_DIR/install.sh" \
  --version "$VERSION" \
  --assets-dir "$ASSETS_DIR" \
  --install-dir "$INSTALL_DIR" \
  --release-only \
  > "$TMP_DIR/install-ok.log"

if [[ ! -x "$INSTALL_DIR/mosaic" ]]; then
  echo "error: install.sh did not produce executable binary at $INSTALL_DIR/mosaic" >&2
  exit 1
fi

if ! "$INSTALL_DIR/mosaic" --help | grep -Fq "mosaic smoke fixture help"; then
  echo "error: installed fixture binary output mismatch" >&2
  exit 1
fi

EMPTY_ASSETS="$TMP_DIR/release-assets-empty"
mkdir -p "$EMPTY_ASSETS"
set +e
bash "$CLI_DIR/install.sh" \
  --version "$VERSION" \
  --assets-dir "$EMPTY_ASSETS" \
  --install-dir "$TMP_DIR/install-empty/bin" \
  --release-only \
  > "$TMP_DIR/install-fail.log" 2>&1
install_fail_exit=$?
set -e
if [[ "$install_fail_exit" -eq 0 ]]; then
  echo "error: install.sh unexpectedly succeeded with missing archive in release-only mode" >&2
  exit 1
fi
if ! grep -Fq "release install failed and --release-only is set" "$TMP_DIR/install-fail.log"; then
  echo "error: install.sh failure output did not include release-only guardrail message" >&2
  exit 1
fi

if command -v pwsh >/dev/null 2>&1; then
  cat > "$TMP_DIR/parse-install-ps1.ps1" <<'PS'
param([string]$Path)
$tokens = $null
$errors = $null
[System.Management.Automation.Language.Parser]::ParseFile($Path, [ref]$tokens, [ref]$errors) | Out-Null
if ($errors.Count -gt 0) {
    Write-Error $errors[0].Message
    exit 1
}
PS
  pwsh -NoLogo -NoProfile -File "$TMP_DIR/parse-install-ps1.ps1" -Path "$CLI_DIR/install.ps1" > /dev/null
fi

echo "release_install_smoke: OK"
