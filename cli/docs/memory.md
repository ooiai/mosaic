# Memory Runtime (V3 bootstrap)

`mosaic memory` provides local text indexing and search.

## Commands

```bash
mosaic --project-state memory index --path .
mosaic --project-state memory index --path . --incremental
mosaic --project-state memory index --path . --namespace ops --incremental --stale-after-hours 24
mosaic --project-state memory search "rust agent"
mosaic --project-state memory search "gateway retry" --namespace ops
mosaic --project-state memory status
mosaic --project-state memory status --all-namespaces
mosaic --project-state memory clear
mosaic --project-state memory prune --max-namespaces 5 --dry-run
mosaic --project-state memory prune --max-documents-per-namespace 1000 --dry-run
mosaic --project-state memory prune --max-namespaces 5 --max-age-hours 168
mosaic --project-state memory policy get
mosaic --project-state memory policy set --enabled true --max-documents-per-namespace 1000 --min-interval-minutes 60
mosaic --project-state memory policy apply
```

## Index behavior

- Walks files under `--path` (default: current directory)
- Skips common heavy folders (`.git`, `target`, `node_modules`, `.pnpm-store`, `.mosaic`)
- Stores index as JSONL at `.mosaic/data/memory/index.jsonl`
- Stores status at `.mosaic/data/memory/status.json`
- Supports logical index segmentation via `--namespace`:
  - `default` namespace keeps legacy paths above
  - non-default namespace uses `.mosaic/data/memory/namespaces/<namespace>/...`

## Tuning options

`memory index` supports:

- `--max-files` (default `500`)
- `--max-file-size` in bytes (default `262144`)
- `--max-content-bytes` per document (default `16384`)
- `--namespace` (default `default`)
- `--incremental` (reuse unchanged indexed documents by `path + size + mtime`)
- `--stale-after-hours` (force refresh for documents older than threshold in incremental mode)
- `--retain-missing` (keep previously indexed docs that are currently missing from disk)

When `--incremental` is enabled, JSON output includes:

- `incremental`
- `reused_documents`
- `reindexed_documents`
- `stale_reindexed_documents`
- `removed_documents`
- `retained_missing_documents`

## Search output

`memory search` returns:

- `total_hits`
- ranked `hits` with `path`, `score`, and `snippet`

Current scoring combines:

- phrase matches in content
- token matches in content
- path signal matches (useful for file-targeted queries like `gateway retry`)

Supports `--json` for machine-readable output.
`memory search/status/clear` also support `--namespace`.
`memory status` supports `--all-namespaces`.

## Clear behavior

- `memory clear` removes:
  - namespace-specific index + status files (`default` or custom namespace)
- Running `memory status` after clear returns `indexed_documents=0`.

## Prune behavior

`memory prune` removes non-default namespaces under retention policy:

- `--max-namespaces <n>` keep only the newest `n` namespaces
- `--max-age-hours <h>` remove namespaces older than `h` hours
- `--max-documents-per-namespace <n>` remove namespaces with indexed document count over `n`
- `--dry-run` preview removal without deleting files

`memory prune` output also includes reason breakdown arrays:

- `removed_due_to_max_namespaces`
- `removed_due_to_max_age_hours`
- `removed_due_to_max_documents_per_namespace`

## Cleanup policy (persistent)

`memory policy` stores cleanup policy in TOML (`.mosaic/policy/memory.toml` in project mode):

- `memory policy get` show current persisted policy
- `memory policy set` update policy fields
- `memory policy apply` execute prune using policy limits

`memory policy set` supports:

- `--enabled <true|false>`
- `--max-namespaces <n>`
- `--max-age-hours <h>`
- `--max-documents-per-namespace <n>`
- `--min-interval-minutes <m>` (skip repeated apply until interval elapsed)
- `--clear-limits` (clear all limit fields before applying new values)

`memory policy apply` supports:

- `--dry-run` (preview only, does not persist `last_run_*`)
- `--force` (bypass interval guard once)

## Cron/Automation integration

Memory cleanup policy can run automatically via built-in system event handlers in cron/webhook/system-event flows.

Event names:

- `mosaic.memory.cleanup` (recommended)
- `memory.cleanup` (compat alias)

Payload options:

- `dry_run` (`boolean`, optional)
- `force` (`boolean`, optional)

Example:

```bash
mosaic --project-state memory policy set \
  --enabled true \
  --max-documents-per-namespace 1000 \
  --min-interval-minutes 60

mosaic --project-state --json cron add \
  --name memory-cleanup \
  --event mosaic.memory.cleanup \
  --every 3600 \
  --data '{"dry_run":false}'

mosaic --project-state --yes --json cron run <job-id>
```
