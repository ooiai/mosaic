#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<USAGE
Generate release notes draft from WORKLOG entries.

Usage:
  $0 --version <tag> [--from-date <ISO8601>] [--max-entries <n>] [--out <path>]

Examples:
  $0 --version v0.2.0-beta.6
  $0 --version v0.2.0-beta.6 --from-date 2026-03-05T00:00:00Z --max-entries 30 \
    --out docs/release-notes-v0.2.0-beta.6.md
USAGE
}

VERSION=""
FROM_DATE=""
MAX_ENTRIES=20
OUT_PATH=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --version)
      VERSION="${2:-}"
      shift 2
      ;;
    --from-date)
      FROM_DATE="${2:-}"
      shift 2
      ;;
    --max-entries)
      MAX_ENTRIES="${2:-}"
      shift 2
      ;;
    --out)
      OUT_PATH="${2:-}"
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

if [[ -z "$VERSION" ]]; then
  echo "error: --version is required" >&2
  usage >&2
  exit 1
fi

if ! [[ "$MAX_ENTRIES" =~ ^[0-9]+$ ]] || [[ "$MAX_ENTRIES" -le 0 ]]; then
  echo "error: --max-entries must be a positive integer" >&2
  exit 1
fi

CLI_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ROOT_DIR="$(cd "$CLI_DIR/.." && pwd)"
WORKLOG_PATH="$ROOT_DIR/WORKLOG.md"

if [[ ! -f "$WORKLOG_PATH" ]]; then
  echo "error: WORKLOG not found at $WORKLOG_PATH" >&2
  exit 1
fi

TMP_ENTRIES="$(mktemp)"
trap 'rm -f "$TMP_ENTRIES"' EXIT

# Parse markdown table rows: | `ts` | summary | tests | files |
awk '
  /^\|[[:space:]]*`/ {
    line=$0
    sub(/^\|[[:space:]]*/, "", line)
    sub(/[[:space:]]*\|[[:space:]]*$/, "", line)
    # Worklog cells escape literal pipes as "\|". Protect them before split.
    gsub(/\\\|/, "\034", line)
    n=split(line, parts, /[[:space:]]*\|[[:space:]]*/)
    if (n >= 4) {
      for (i = 1; i <= n; i++) {
        gsub(/\034/, "|", parts[i])
      }
      ts=parts[1]
      gsub(/^`/, "", ts)
      gsub(/`$/, "", ts)
      summary=parts[2]
      tests=parts[3]
      files=parts[4]
      if (n > 4) {
        for (i = 5; i <= n; i++) {
          files=files " | " parts[i]
        }
      }
      gsub(/\t/, " ", summary)
      gsub(/\t/, " ", tests)
      gsub(/\t/, " ", files)
      print ts "\t" summary "\t" tests "\t" files
    }
  }
' "$WORKLOG_PATH" > "$TMP_ENTRIES"

if [[ ! -s "$TMP_ENTRIES" ]]; then
  echo "error: no parsable worklog entries found in $WORKLOG_PATH" >&2
  exit 1
fi

selected=()
while IFS=$'\t' read -r ts summary tests files; do
  if [[ -n "$FROM_DATE" && "$ts" < "$FROM_DATE" ]]; then
    continue
  fi
  selected+=("${ts}"$'\t'"${summary}"$'\t'"${tests}"$'\t'"${files}")
done < "$TMP_ENTRIES"

if [[ ${#selected[@]} -eq 0 ]]; then
  echo "error: no entries matched current filters" >&2
  exit 1
fi

start_index=0
if [[ ${#selected[@]} -gt $MAX_ENTRIES ]]; then
  start_index=$(( ${#selected[@]} - MAX_ENTRIES ))
fi

window=("${selected[@]:$start_index}")

escape_md() {
  local value="$1"
  value="${value//|/\\|}"
  echo "$value"
}

timestamp_utc="$(date -u +"%Y-%m-%dT%H:%M:%SZ")"

notes="# Mosaic CLI Release Notes '$VERSION'\n\n"
notes+="Generated from WORKLOG.md on $timestamp_utc.\n\n"
notes+="## Scope\n\n"
notes+="- Entries included: ${#window[@]}\n"
if [[ -n "$FROM_DATE" ]]; then
  notes+="- Filter: entries on/after '$FROM_DATE'\n"
else
  notes+="- Filter: latest ${#window[@]} entries\n"
fi
notes+="\n"
notes+="## Highlights\n\n"

for ((i=${#window[@]}-1; i>=0; i--)); do
  entry="${window[$i]}"
  IFS=$'\t' read -r ts summary tests files <<< "$entry"
  summary_escaped="$(escape_md "$summary")"
  notes+="- **$ts**: $summary_escaped\n"
done

notes+="\n## Validation Commands Seen\n\n"

commands=()
command_seen() {
  local needle="$1"
  local existing
  if [[ -z "${commands[*]-}" ]]; then
    return 1
  fi
  for existing in "${commands[@]-}"; do
    if [[ -z "$existing" ]]; then
      continue
    fi
    if [[ "$existing" == "$needle" ]]; then
      return 0
    fi
  done
  return 1
}

for ((i=${#window[@]}-1; i>=0; i--)); do
  entry="${window[$i]}"
  IFS=$'\t' read -r ts summary tests files <<< "$entry"
  IFS=';' read -ra parts <<< "$tests"
  for raw in "${parts[@]}"; do
    cmd="$(echo "$raw" | sed -E 's/^[[:space:]]+//; s/[[:space:]]+$//')"
    cmd="${cmd//\\|/|}"
    if [[ -z "$cmd" || "$cmd" == "-" || "$cmd" == "no-op" ]]; then
      continue
    fi
    if ! command_seen "$cmd"; then
      commands+=("$cmd")
    fi
  done
done

if [[ -z "${commands[*]-}" ]]; then
  notes+="- No explicit verification commands recorded in selected worklog window.\n"
else
  limit=0
  for cmd in "${commands[@]-}"; do
    if [[ -z "$cmd" ]]; then
      continue
    fi
    notes+="- '$(escape_md "$cmd")'\n"
    limit=$((limit + 1))
    if [[ $limit -ge 25 ]]; then
      notes+="- (truncated additional commands)\n"
      break
    fi
  done
fi

notes+="\n## Changed Areas\n\n"
notes+="| Timestamp | Files |\n"
notes+="| --- | --- |\n"
for ((i=${#window[@]}-1; i>=0; i--)); do
  entry="${window[$i]}"
  IFS=$'\t' read -r ts summary tests files <<< "$entry"
  notes+="| $ts | $(escape_md "$files") |\n"
done

notes+="\n## Release Notes Editing Checklist\n\n"
notes+="- Replace internal implementation phrasing with user-facing changes.\n"
notes+="- Call out compatibility or behavior changes explicitly.\n"
notes+="- Keep install/upgrade section aligned with release assets.\n"
notes+="- Add known limitations and next patch plan.\n"

if [[ -n "$OUT_PATH" ]]; then
  if [[ "$OUT_PATH" != /* ]]; then
    OUT_PATH="$CLI_DIR/$OUT_PATH"
  fi
  mkdir -p "$(dirname "$OUT_PATH")"
  printf "%b" "$notes" > "$OUT_PATH"
  echo "written: $OUT_PATH"
else
  printf "%b" "$notes"
fi
