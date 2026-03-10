#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<USAGE
Usage: $0 --version <version> [options]

Options:
  --version <tag>              Release tag (for example: v0.2.0-beta.6)
  --repo <owner/repo>          GitHub repo (default: ooiai/mosaic)
  --token-env <ENV>            Optional env var name holding GitHub token
  --notes <path>               Local release notes path to check (default: cli/docs/release-notes-<version>.md)
  --release-json-file <path>   Use a local release JSON file instead of GitHub API
  --json                       Emit JSON result
  -h, --help                   Show help

Examples:
  $0 --version v0.2.0-beta.6
  $0 --version v0.2.0-beta.6 --repo ooiai/mosaic --json
  $0 --version v0.2.0-beta.6 --token-env GITHUB_TOKEN
  $0 --version v0.2.0-beta.6 --release-json-file /tmp/release.json
USAGE
}

VERSION=""
REPO="ooiai/mosaic"
TOKEN_ENV=""
NOTES_PATH=""
RELEASE_JSON_FILE=""
JSON_MODE=0

while [[ $# -gt 0 ]]; do
  case "$1" in
    --version)
      VERSION="${2:-}"
      shift 2
      ;;
    --repo)
      REPO="${2:-}"
      shift 2
      ;;
    --token-env)
      TOKEN_ENV="${2:-}"
      shift 2
      ;;
    --notes)
      NOTES_PATH="${2:-}"
      shift 2
      ;;
    --release-json-file)
      RELEASE_JSON_FILE="${2:-}"
      shift 2
      ;;
    --json)
      JSON_MODE=1
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
if [[ -z "$NOTES_PATH" ]]; then
  NOTES_PATH="$CLI_DIR/docs/release-notes-${VERSION}.md"
fi
if [[ "$NOTES_PATH" != /* ]]; then
  NOTES_PATH="$CLI_DIR/$NOTES_PATH"
fi

json_escape() {
  local s="$1"
  s="${s//\\/\\\\}"
  s="${s//\"/\\\"}"
  s="${s//$'\n'/\\n}"
  s="${s//$'\r'/\\r}"
  s="${s//$'\t'/\\t}"
  printf '%s' "$s"
}

sha256_valid() {
  local value="$1"
  [[ "$value" =~ ^[0-9a-fA-F]{64}$ ]]
}

download_text() {
  local url="$1"
  curl -fsSL "$url"
}

find_entry() {
  local name="$1"
  shift
  local entry n v
  for entry in "$@"; do
    n="${entry%%$'\t'*}"
    v="${entry#*$'\t'}"
    if [[ "$n" == "$name" ]]; then
      echo "$v"
      return 0
    fi
  done
  return 1
}

has_entry() {
  local name="$1"
  shift
  local entry n
  for entry in "$@"; do
    n="${entry%%$'\t'*}"
    if [[ "$n" == "$name" ]]; then
      return 0
    fi
  done
  return 1
}

add_error() {
  errors+=("$1")
}

add_warning() {
  warnings+=("$1")
}

to_lower() {
  local value="$1"
  printf '%s' "$value" | tr '[:upper:]' '[:lower:]'
}

errors=()
warnings=()

release_json=""
if [[ -n "$RELEASE_JSON_FILE" ]]; then
  if [[ ! -f "$RELEASE_JSON_FILE" ]]; then
    echo "error: release json file not found: $RELEASE_JSON_FILE" >&2
    exit 1
  fi
  release_json="$(cat "$RELEASE_JSON_FILE")"
else
  API_URL="https://api.github.com/repos/${REPO}/releases/tags/${VERSION}"
  CURL_ARGS=(-fsSL -H "Accept: application/vnd.github+json")
  if [[ -n "$TOKEN_ENV" ]]; then
    token_value="${!TOKEN_ENV:-}"
    if [[ -n "$token_value" ]]; then
      CURL_ARGS+=( -H "Authorization: Bearer ${token_value}" )
    else
      add_warning "token env '$TOKEN_ENV' is empty"
    fi
  fi
  release_json="$(curl "${CURL_ARGS[@]}" "$API_URL" 2>/tmp/mosaic-release-check-curl.err || true)"
  if [[ -z "$release_json" ]]; then
    err_msg="$(cat /tmp/mosaic-release-check-curl.err 2>/dev/null || true)"
    echo "error: failed to fetch release API for ${REPO}@${VERSION}" >&2
    if [[ -n "$err_msg" ]]; then
      echo "$err_msg" >&2
    fi
    rm -f /tmp/mosaic-release-check-curl.err
    exit 1
  fi
  rm -f /tmp/mosaic-release-check-curl.err
fi

parse_output="$(RELEASE_JSON_PAYLOAD="$release_json" node <<'NODE'
const fs = require('fs');
const raw = process.env.RELEASE_JSON_PAYLOAD || '';
let release;
try {
  release = JSON.parse(raw);
} catch (err) {
  console.error(`parse_error\t${String(err.message || err)}`);
  process.exit(2);
}
if (release && release.message === 'Not Found') {
  console.error('not_found\tRelease tag not found');
  process.exit(3);
}
if (!release || typeof release !== 'object') {
  console.error('parse_error\tRelease payload is not an object');
  process.exit(2);
}
const tag = release.tag_name || '';
const html = release.html_url || '';
const published = release.published_at || '';
const assets = Array.isArray(release.assets) ? release.assets : [];
console.log(`meta\t${tag}\t${html}\t${published}\t${assets.length}`);
for (const asset of assets) {
  const name = asset && asset.name ? String(asset.name) : '';
  const url = asset && asset.browser_download_url ? String(asset.browser_download_url) : '';
  console.log(`asset\t${name}\t${url}`);
}
NODE
)" || parse_status=$?
parse_status="${parse_status:-0}"
if [[ "$parse_status" != "0" ]]; then
  if [[ "$parse_status" == "3" ]]; then
    echo "error: release ${VERSION} not found in ${REPO}" >&2
  else
    echo "error: unable to parse release payload for ${REPO}@${VERSION}" >&2
  fi
  exit 1
fi

release_tag=""
release_url=""
published_at=""
asset_count=0
asset_entries=()
while IFS=$'\t' read -r kind a b c d; do
  case "$kind" in
    meta)
      release_tag="$a"
      release_url="$b"
      published_at="$c"
      asset_count="$d"
      ;;
    asset)
      if [[ -n "$a" ]]; then
        asset_entries+=("$a"$'\t'"$b")
      fi
      ;;
  esac
done <<< "$parse_output"

if [[ "$release_tag" != "$VERSION" ]]; then
  add_error "release tag mismatch: expected ${VERSION}, got ${release_tag}"
fi

required_archives=(
  "mosaic-${VERSION}-darwin-arm64.tar.gz"
  "mosaic-${VERSION}-darwin-x64.tar.gz"
  "mosaic-${VERSION}-linux-x64.tar.gz"
  "mosaic-${VERSION}-windows-x64.zip"
)
required_sidecars=(
  "mosaic-${VERSION}-darwin-arm64.tar.gz.sha256"
  "mosaic-${VERSION}-darwin-x64.tar.gz.sha256"
  "mosaic-${VERSION}-linux-x64.tar.gz.sha256"
  "mosaic-${VERSION}-windows-x64.zip.sha256"
)
required_aux=(
  "mosaic.rb"
  "mosaic.json"
  "SHA256SUMS"
  "install.sh"
  "install.ps1"
)

missing_assets=()
for name in "${required_archives[@]}" "${required_sidecars[@]}" "${required_aux[@]}"; do
  if ! has_entry "$name" "${asset_entries[@]-}"; then
    missing_assets+=("$name")
    add_error "missing release asset: $name"
  fi
done

sidecar_hash_entries=()
for idx in "${!required_archives[@]}"; do
  archive_name="${required_archives[$idx]}"
  sidecar_name="${required_sidecars[$idx]}"
  sidecar_url="$(find_entry "$sidecar_name" "${asset_entries[@]-}" || true)"
  if [[ -z "$sidecar_url" ]]; then
    continue
  fi

  sidecar_text="$(download_text "$sidecar_url" 2>/dev/null || true)"
  if [[ -z "$sidecar_text" ]]; then
    add_error "unable to download sidecar checksum: $sidecar_name"
    continue
  fi

  sidecar_line="$(printf '%s\n' "$sidecar_text" | awk 'NF{print; exit}')"
  sidecar_hash="$(printf '%s\n' "$sidecar_line" | awk '{print $1}')"
  sidecar_file="$(printf '%s\n' "$sidecar_line" | awk '{print $2}')"
  sidecar_file="$(basename "$sidecar_file")"

  if ! sha256_valid "$sidecar_hash"; then
    add_error "invalid checksum format in sidecar: $sidecar_name"
    continue
  fi
  if [[ "$sidecar_file" != "$archive_name" ]]; then
    add_error "sidecar filename mismatch: $sidecar_name points to $sidecar_file"
    continue
  fi

  sidecar_hash_entries+=("$archive_name"$'\t'"$(to_lower "$sidecar_hash")")
done

sums_url="$(find_entry "SHA256SUMS" "${asset_entries[@]-}" || true)"
if [[ -n "$sums_url" ]]; then
  sums_text="$(download_text "$sums_url" 2>/dev/null || true)"
  if [[ -z "$sums_text" ]]; then
    add_error "unable to download SHA256SUMS"
  else
    for archive_name in "${required_archives[@]}"; do
      sum_hash="$(printf '%s\n' "$sums_text" | awk -v a="$archive_name" '$2==a{print $1; exit}')"
      if [[ -z "$sum_hash" ]]; then
        add_error "SHA256SUMS missing entry: $archive_name"
        continue
      fi
      if ! sha256_valid "$sum_hash"; then
        add_error "invalid checksum format in SHA256SUMS for $archive_name"
        continue
      fi
      side_hash="$(find_entry "$archive_name" "${sidecar_hash_entries[@]-}" || true)"
      if [[ -n "$side_hash" && "$(to_lower "$sum_hash")" != "$side_hash" ]]; then
        add_error "checksum mismatch between sidecar and SHA256SUMS for $archive_name"
      fi
    done
  fi
fi

version_stripped="${VERSION#v}"
rb_url="$(find_entry "mosaic.rb" "${asset_entries[@]-}" || true)"
if [[ -n "$rb_url" ]]; then
  rb_text="$(download_text "$rb_url" 2>/dev/null || true)"
  if [[ -z "$rb_text" ]]; then
    add_error "unable to download mosaic.rb"
  else
    if ! printf '%s' "$rb_text" | grep -Fq "version \"${version_stripped}\""; then
      add_error "mosaic.rb version mismatch (expected ${version_stripped})"
    fi
    rb_expected_urls=(
      "https://github.com/${REPO}/releases/download/${VERSION}/mosaic-${VERSION}-darwin-arm64.tar.gz"
      "https://github.com/${REPO}/releases/download/${VERSION}/mosaic-${VERSION}-darwin-x64.tar.gz"
      "https://github.com/${REPO}/releases/download/${VERSION}/mosaic-${VERSION}-linux-x64.tar.gz"
    )
    for expected in "${rb_expected_urls[@]}"; do
      if ! printf '%s' "$rb_text" | grep -Fq "$expected"; then
        add_error "mosaic.rb missing URL: $expected"
      fi
    done
  fi
fi

json_url="$(find_entry "mosaic.json" "${asset_entries[@]-}" || true)"
if [[ -n "$json_url" ]]; then
  json_text="$(download_text "$json_url" 2>/dev/null || true)"
  if [[ -z "$json_text" ]]; then
    add_error "unable to download mosaic.json"
  else
    if ! printf '%s' "$json_text" | grep -Fq "\"version\": \"${version_stripped}\""; then
      add_error "mosaic.json version mismatch (expected ${version_stripped})"
    fi
    expected_windows_url="https://github.com/${REPO}/releases/download/${VERSION}/mosaic-${VERSION}-windows-x64.zip"
    if ! printf '%s' "$json_text" | grep -Fq "$expected_windows_url"; then
      add_error "mosaic.json missing URL: $expected_windows_url"
    fi

    windows_hash="$(find_entry "mosaic-${VERSION}-windows-x64.zip" "${sidecar_hash_entries[@]-}" || true)"
    if [[ -n "$windows_hash" ]]; then
      if ! printf '%s' "$json_text" | grep -Fq "\"hash\": \"${windows_hash}\""; then
        add_error "mosaic.json windows hash mismatch"
      fi
    fi
  fi
fi

if [[ ! -f "$NOTES_PATH" ]]; then
  add_warning "local notes file not found: $NOTES_PATH"
fi

ok=true
if [[ ${#errors[@]} -gt 0 ]]; then
  ok=false
fi

if [[ "$JSON_MODE" == "1" ]]; then
  printf '{\n'
  printf '  "ok": %s,\n' "$ok"
  printf '  "repo": "%s",\n' "$(json_escape "$REPO")"
  printf '  "version": "%s",\n' "$(json_escape "$VERSION")"
  printf '  "release_tag": "%s",\n' "$(json_escape "$release_tag")"
  printf '  "release_url": "%s",\n' "$(json_escape "$release_url")"
  printf '  "published_at": "%s",\n' "$(json_escape "$published_at")"
  printf '  "assets_count": %s,\n' "$asset_count"

  printf '  "missing_assets": ['
  idx=0
  for msg in "${missing_assets[@]-}"; do
    if [[ -z "$msg" ]]; then
      continue
    fi
    if [[ $idx -gt 0 ]]; then
      printf ', '
    fi
    printf '"%s"' "$(json_escape "$msg")"
    idx=$((idx + 1))
  done
  printf '],\n'

  printf '  "errors": ['
  idx=0
  for msg in "${errors[@]-}"; do
    if [[ -z "$msg" ]]; then
      continue
    fi
    if [[ $idx -gt 0 ]]; then
      printf ', '
    fi
    printf '"%s"' "$(json_escape "$msg")"
    idx=$((idx + 1))
  done
  printf '],\n'

  printf '  "warnings": ['
  idx=0
  for msg in "${warnings[@]-}"; do
    if [[ -z "$msg" ]]; then
      continue
    fi
    if [[ $idx -gt 0 ]]; then
      printf ', '
    fi
    printf '"%s"' "$(json_escape "$msg")"
    idx=$((idx + 1))
  done
  printf ']\n'
  printf '}\n'
else
  echo "release publish check"
  echo "repo: $REPO"
  echo "version: $VERSION"
  echo "release_tag: $release_tag"
  echo "release_url: $release_url"
  echo "published_at: $published_at"
  echo "assets_count: $asset_count"
  echo "notes: $NOTES_PATH"

  if [[ "$ok" == "true" ]]; then
    echo "status: OK"
  else
    echo "status: FAILED"
  fi

  if [[ ${#missing_assets[@]} -gt 0 ]]; then
    echo "missing_assets:"
    for msg in "${missing_assets[@]}"; do
      echo "- $msg"
    done
  fi

  if [[ ${#errors[@]} -gt 0 ]]; then
    echo "errors:"
    for msg in "${errors[@]}"; do
      echo "- $msg"
    done
  fi

  if [[ ${#warnings[@]} -gt 0 ]]; then
    echo "warnings:"
    for msg in "${warnings[@]}"; do
      echo "- $msg"
    done
  fi
fi

if [[ "$ok" != "true" ]]; then
  exit 1
fi
