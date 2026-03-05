#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT_DIR"

REPORT_DIR=""
REPORT_PREFIX="docs-check"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --report-dir)
      REPORT_DIR="${2:-}"
      shift 2
      ;;
    --report-prefix)
      REPORT_PREFIX="${2:-}"
      shift 2
      ;;
    -h|--help)
      cat <<USAGE
Usage: site/scripts/check_docs.sh [options]

Options:
  --report-dir <path>      output directory for reports
  --report-prefix <name>   report filename prefix (default: docs-check)
  -h, --help               show this help
USAGE
      exit 0
      ;;
    *)
      echo "error: unknown argument: $1" >&2
      exit 1
      ;;
  esac
done

if [[ -n "$REPORT_DIR" ]]; then
  mkdir -p "$REPORT_DIR"
  SYNTAX_LOG="$REPORT_DIR/${REPORT_PREFIX}-syntax.log"
  LINKS_LOG="$REPORT_DIR/${REPORT_PREFIX}-links.log"
  SUMMARY_JSON="$REPORT_DIR/${REPORT_PREFIX}-summary.json"
else
  TMP_DIR="$(mktemp -d "${TMPDIR:-/tmp}/mosaic-docs-check.XXXXXX")"
  trap 'rm -rf "$TMP_DIR"' EXIT
  SYNTAX_LOG="$TMP_DIR/syntax.log"
  LINKS_LOG="$TMP_DIR/links.log"
  SUMMARY_JSON="$TMP_DIR/summary.json"
fi

echo "[docs-check] validating docs.js syntax"
if node --check site/assets/docs.js >"$SYNTAX_LOG" 2>&1; then
  SYNTAX_OK=1
  echo "[docs-check] syntax ok" >>"$SYNTAX_LOG"
else
  SYNTAX_OK=0
  cat "$SYNTAX_LOG"
fi

echo "[docs-check] validating local href links"
MISSING=0
CHECKED=0
: >"$LINKS_LOG"
while IFS=$'\t' read -r src href; do
  case "$href" in
    http*|mailto:*|\#*|javascript:*)
      continue
      ;;
  esac

  CHECKED=$((CHECKED + 1))

  if [[ "$src" == site/cn/* ]]; then
    base="site/cn"
  else
    base="site"
  fi

  href_no_frag="${href%%#*}"
  href_no_query="${href_no_frag%%\?*}"

  case "$href_no_query" in
    ../*) target="site/${href_no_query#../}" ;;
    ./*)  target="$base/${href_no_query#./}" ;;
    *)    target="$base/$href_no_query" ;;
  esac

  if [[ ! -e "$target" ]]; then
    echo "missing link target: ${src} -> ${href} (resolved: ${target})" | tee -a "$LINKS_LOG"
    MISSING=$((MISSING + 1))
  fi
done < <(perl -nle 'while(/href="([^"]+)"/g){print "$ARGV\t$1"}' site/*.html site/cn/*.html)

if [[ "$MISSING" -eq 0 ]]; then
  echo "no missing local links found (checked=$CHECKED)" >>"$LINKS_LOG"
fi

if [[ "$MISSING" -eq 0 ]]; then
  LINKS_OK=1
else
  LINKS_OK=0
fi

if [[ "$SYNTAX_OK" -eq 1 && "$LINKS_OK" -eq 1 ]]; then
  OK=true
else
  OK=false
fi

cat >"$SUMMARY_JSON" <<JSON
{
  "ok": $OK,
  "ts": "$(date -u +"%Y-%m-%dT%H:%M:%SZ")",
  "syntax_ok": $([[ "$SYNTAX_OK" -eq 1 ]] && echo true || echo false),
  "links_ok": $([[ "$LINKS_OK" -eq 1 ]] && echo true || echo false),
  "checked_links": $CHECKED,
  "missing_links": $MISSING,
  "syntax_log": "$SYNTAX_LOG",
  "links_log": "$LINKS_LOG"
}
JSON

if [[ "$OK" != "true" ]]; then
  echo "[docs-check] failed: syntax_ok=$SYNTAX_OK links_ok=$LINKS_OK missing_links=$MISSING"
  echo "[docs-check] summary: $SUMMARY_JSON"
  exit 1
fi

echo "[docs-check] summary: $SUMMARY_JSON"
echo "[docs-check] ok"
