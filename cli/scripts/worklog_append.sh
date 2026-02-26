#!/usr/bin/env bash
set -euo pipefail

CLI_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PROJECT_ROOT="$(cd "$CLI_DIR/.." && pwd)"
LOG_FILE="$PROJECT_ROOT/WORKLOG.md"
PROGRESS_FILE="$CLI_DIR/docs/progress.md"

sanitize_cell() {
  local value="$1"
  value="$(printf '%s' "$value" | tr '\n' ' ' | sed 's/[[:space:]]\+/ /g; s/^ //; s/ $//')"
  value="${value//|/\\|}"
  if [[ -z "$value" ]]; then
    printf '%s' "-"
    return
  fi
  printf '%s' "$value"
}

collect_changed_preview() {
  local tmp_changed changed_count preview more_count
  tmp_changed="$(mktemp "${TMPDIR:-/tmp}/mosaic-worklog-changed-XXXXXX.txt")"
  {
    git -C "$PROJECT_ROOT" diff --name-only --relative
    git -C "$PROJECT_ROOT" diff --cached --name-only --relative
    git -C "$PROJECT_ROOT" ls-files --others --exclude-standard
  } | awk 'NF' | sort -u >"$tmp_changed"

  changed_count="$(wc -l <"$tmp_changed" | tr -d ' ')"
  preview="$(
    head -n 8 "$tmp_changed" | awk '
      BEGIN { first = 1 }
      {
        if (!first) {
          printf ", "
        }
        printf "%s", $0
        first = 0
      }
      END {
        if (first) {
          printf "-"
        }
      }
    '
  )"

  if [[ "$changed_count" -gt 8 ]]; then
    more_count=$((changed_count - 8))
    preview="$preview (+${more_count} more)"
  fi

  rm -f "$tmp_changed"
  printf '%s' "$preview"
}

SUMMARY=""
TESTS=""
FILES=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --summary|-s)
      SUMMARY="${2:-}"
      shift 2
      ;;
    --tests|-t)
      TESTS="${2:-}"
      shift 2
      ;;
    --files|-f)
      FILES="${2:-}"
      shift 2
      ;;
    *)
      if [[ -z "$SUMMARY" ]]; then
        SUMMARY="$1"
      elif [[ -z "$TESTS" ]]; then
        TESTS="$1"
      else
        echo "error: unexpected argument '$1'" >&2
        exit 1
      fi
      shift
      ;;
  esac
done

if [[ -z "$SUMMARY" ]]; then
  echo "usage: $0 --summary \"what changed\" [--tests \"cargo test ...\"] [--files \"file1,file2\"]" >&2
  exit 1
fi

TS="$(date -u +"%Y-%m-%dT%H:%M:%SZ")"

if [[ ! -f "$LOG_FILE" ]]; then
  cat >"$LOG_FILE" <<'EOF'
# Mosaic Work Log

Concise timeline of completed work for regression and release review.

| UTC | Summary | Tests | Changed Files |
| --- | --- | --- | --- |

EOF
fi

if [[ ! -f "$PROGRESS_FILE" ]]; then
  cat >"$PROGRESS_FILE" <<'EOF'
# Mosaic CLI Progress Log

Concise per-iteration delivery log for CLI work.

| UTC | Summary | Tests |
| --- | --- | --- |

EOF
fi

if [[ -z "$FILES" ]]; then
  FILES="$(collect_changed_preview)"
fi

SUMMARY="$(sanitize_cell "$SUMMARY")"
TESTS="$(sanitize_cell "$TESTS")"
FILES="$(sanitize_cell "$FILES")"

printf '| `%s` | %s | %s | %s |\n' "$TS" "$SUMMARY" "$TESTS" "$FILES" >>"$LOG_FILE"
printf '| `%s` | %s | %s |\n' "$TS" "$SUMMARY" "$TESTS" >>"$PROGRESS_FILE"

echo "updated: $LOG_FILE"
echo "updated: $PROGRESS_FILE"
