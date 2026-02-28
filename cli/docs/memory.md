# Memory Runtime (V3 bootstrap)

`mosaic memory` provides local text indexing and search.

## Commands

```bash
mosaic --project-state memory index --path .
mosaic --project-state memory search "rust agent"
mosaic --project-state memory status
mosaic --project-state memory clear
```

## Index behavior

- Walks files under `--path` (default: current directory)
- Skips common heavy folders (`.git`, `target`, `node_modules`, `.pnpm-store`, `.mosaic`)
- Stores index as JSONL at `.mosaic/data/memory/index.jsonl`
- Stores status at `.mosaic/data/memory/status.json`

## Tuning options

`memory index` supports:

- `--max-files` (default `500`)
- `--max-file-size` in bytes (default `262144`)
- `--max-content-bytes` per document (default `16384`)

## Search output

`memory search` returns:

- `total_hits`
- ranked `hits` with `path`, `score`, and `snippet`

Supports `--json` for machine-readable output.

## Clear behavior

- `memory clear` removes:
  - `.mosaic/data/memory/index.jsonl`
  - `.mosaic/data/memory/status.json`
- Running `memory status` after clear returns `indexed_documents=0`.
