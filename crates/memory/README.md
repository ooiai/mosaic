# mosaic-memory

`mosaic-memory` stores compressed session memory, summaries, and cross-session search records for Mosaic.

## Positioning

This crate keeps memory persistence and compression helpers separate from runtime orchestration. Runtime decides when to write memory; this crate defines how memory is represented and stored.

## Architecture Layer

Agent Runtime Layer.

## Responsibilities

- Define `MemoryStore`, `SessionMemoryRecord`, `MemoryEntry`, and `MemorySearchHit`.
- Hold local compression policy in `MemoryPolicy`.
- Provide summary/compression helpers through `summarize_fragments` and `compress_fragments`.
- Persist memory records with `FileMemoryStore`.

## Out of Scope

- Session transcript ownership.
- Provider scheduling or prompt orchestration.
- Gateway routing or audit logic.
- TUI and CLI presentation.

## Public Boundary

- Types: `SessionMemoryRecord`, `MemoryEntry`, `MemoryEntryKind`, `MemorySearchHit`, `CompressionOutcome`, `MemoryPolicy`.
- Store boundary: `MemoryStore`, `FileMemoryStore`.
- Helpers: `summarize_fragments`, `compress_fragments`.

## Why This Is In `crates/`

Memory is shared by runtime execution, gateway summaries, CLI memory commands, and tests. It is reusable state infrastructure with Mosaic semantics, so it should not live inside one command path.

## Relationships

- Upstream crates: none beyond workspace primitives.
- Downstream crates: `mosaic-runtime` reads and writes memory, `mosaic-gateway` exposes memory-backed session views, and `cli` surfaces memory inspection commands.
- Runtime/control-plane coupling: `runtime` decides when memory changes, while `gateway` and `cli` inspect the result. This crate should not choose memory policy timing on its own.

## Minimal Use

```rust
use mosaic_memory::{FileMemoryStore, MemoryStore, SessionMemoryRecord};

let store = FileMemoryStore::new(".mosaic/memory");
let mut record = SessionMemoryRecord::new("demo");
record.set_summary(Some("Short summary".to_owned()));
store.save_session(&record)?;
let loaded = store.load_session("demo")?;
```

## Testing

```bash
cargo test -p mosaic-memory
```

## Current Limitations

- Storage is file-based.
- Compression is heuristic and text-oriented rather than model-aware retrieval.
- Cross-session search is simple substring/tag search today.

## Roadmap

- Add stronger indexing and retrieval options without changing the core record shape.
- Keep the memory store trait small enough for alternate backends.
- Expand summary/compression helpers as runtime context management grows.
