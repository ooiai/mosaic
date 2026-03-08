# Knowledge Runtime (CLI)

`mosaic knowledge` provides source ingestion and retrieval-assisted Q&A over the local memory index.

## Commands

```bash
mosaic --project-state knowledge ingest --source local_md --path docs --namespace knowledge
mosaic --project-state knowledge ingest --source local_md --path docs --namespace knowledge --report-out ./reports/knowledge-local-ingest.json
mosaic --project-state knowledge ingest --source http --url https://example.com/doc.md --namespace knowledge
mosaic --project-state knowledge ingest --source http --url https://example.com/doc.md --header "Authorization=Bearer $TOKEN" --http-retries 3 --http-retry-backoff-ms 200 --namespace knowledge
mosaic --project-state knowledge ingest --source http --url https://example.com/doc.md --header-env "Authorization=MOSAIC_DOC_TOKEN" --namespace knowledge
mosaic --project-state knowledge ingest --source http --url-file ./urls.txt --continue-on-error --report-out ./reports/knowledge-http-ingest.json --namespace knowledge
mosaic --project-state knowledge ingest --source mcp --mcp-server <server-id> --namespace knowledge
mosaic --project-state knowledge search "retry policy" --namespace knowledge --limit 20 --min-score 4
mosaic --project-state knowledge ask "How should retries work?" --namespace knowledge --top-k 8 --min-score 6
mosaic --project-state knowledge ask "How should retries work?" --namespace knowledge --top-k 8 --min-score 6 --references-only
mosaic --project-state --json knowledge evaluate --query "retry policy" --query "sandbox defaults" --namespace knowledge --top-k 8 --min-score 6 --report-out ./reports/knowledge-eval.json
mosaic --project-state --json knowledge evaluate --query "retry policy" --namespace knowledge --history-window 20
mosaic --project-state --json knowledge evaluate --query "retry policy" --namespace knowledge --baseline ./.mosaic/data/knowledge-eval-baselines/knowledge.json --update-baseline
mosaic --project-state --json knowledge evaluate --query "retry policy" --namespace knowledge --baseline ./.mosaic/data/knowledge-eval-baselines/knowledge.json --max-coverage-drop 0.05 --max-avg-top-score-drop 1.0 --fail-on-regression
mosaic --project-state --json knowledge datasets list
mosaic --project-state --json knowledge datasets list --namespace knowledge
mosaic --project-state --json knowledge datasets remove knowledge --dry-run
mosaic --project-state --json knowledge datasets remove knowledge
```

## Sources

- `local_md`
  - recursively scans markdown files (`.md`, `.markdown`, `.mdx`)
  - skips common heavy folders (`.git`, `node_modules`, `target`, `.mosaic`, `.pnpm-store`)
- `http`
  - fetches markdown/text from `--url` and/or `--url-file`
  - supports `http://` and `https://`
  - supports custom headers via `--header KEY=VALUE` and `--header-env KEY=ENV`
  - retries retryable failures with `--http-retries` + `--http-retry-backoff-ms`
  - supports partial-success mode with `--continue-on-error`
- `mcp`
  - uses configured MCP server `cwd` as markdown root
  - optional `--mcp-path` for subdirectory

`knowledge ingest --report-out <path>` is supported for all source types and exports a unified ingest report.

## Large Local Markdown Recommendations

- use `--incremental` for repeat runs
- keep namespace scoped per domain (`knowledge`, `product`, `ops`, etc.)
- tune ingestion controls:
  - `--max-files`
  - `--max-file-size`
  - `--max-chunk-bytes`
  - `--chunk-overlap-bytes`
- tune index controls:
  - `--max-content-bytes`
  - `--stale-after-hours`
  - `--retain-missing`

## Ingest Pipeline Notes

- documents are staged into:
  - project mode: `.mosaic-knowledge/sources/<namespace>/<source>/`
  - xdg mode: `<xdg-root>/knowledge/sources/<namespace>/<source>/`
- markdown is chunked into bounded-size documents to avoid giant single-record entries
- unchanged staged chunks are reused between runs
- stale staged chunks are removed before indexing

## Retrieval-Assisted Ask

- `knowledge ask` does:
  1. `memory search` in target namespace
  2. prompt augmentation with top-k snippets
  3. normal agent run with approvals/sandbox/tooling policy unchanged

- JSON output includes `references` for traceability.
- `knowledge ask --references-only` skips model calls and returns retrieval references only (no provider dependency).
- `--min-score` filters weak retrieval hits before prompt augmentation and before references-only output.

## Retrieval Evaluation

- `knowledge evaluate` runs retrieval-only scoring over multiple queries and returns:
  - per-query `hit_count`, `top_score`, `top_paths`
  - aggregate `coverage_rate`, `avg_hits`, `avg_top_score`, `p50_top_score`, `p90_top_score`
- query input supports:
  - repeatable `--query`
  - `--query-file` (one query per line, supports `#` comments)
- use `--report-out` to export the full JSON evaluation artifact.
- history/trend:
  - every evaluate run appends one JSONL sample to `.mosaic/data/knowledge-eval-history/<namespace>.jsonl` (or XDG equivalent)
  - output includes `history.previous`, `history.delta_vs_previous`, and window aggregates
  - `--history-window` controls trend window size used in output summaries
- baseline/regression workflow:
  - `--update-baseline` writes current evaluate result as baseline
  - default baseline path: `.mosaic/data/knowledge-eval-baselines/<namespace>.json` (or XDG equivalent)
  - `--baseline <path>` overrides baseline location
  - `--max-coverage-drop` and `--max-avg-top-score-drop` define allowed regression thresholds
  - `--fail-on-regression` exits non-zero when baseline comparison regresses

## Dataset Lifecycle

- `knowledge datasets list` enumerates knowledge namespaces and artifact status:
  - staged source root (`.mosaic-knowledge/sources/<namespace>/...`)
  - memory index/status files
  - evaluate baseline file
  - evaluate history sample count
- `knowledge datasets remove <namespace>` removes a namespace's staged sources + memory index/status + evaluate baseline/history artifacts.
- use `--dry-run` first to preview what would be removed.
- `namespace=default` is protected and cannot be removed.
