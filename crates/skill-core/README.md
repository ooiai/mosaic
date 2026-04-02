# mosaic-skill-core

`mosaic-skill-core` defines the reusable skill layer for Mosaic, including native skills, manifest-backed skills, and markdown skill packs.

## Positioning

This crate is the skill boundary between runtime orchestration and reusable capability modules. It lets Mosaic register native Rust skills, declarative manifest skills, and directory-based `SKILL.md` markdown skill packs under one consistent interface.

## Architecture Layer

Agent Runtime Layer.

## Responsibilities

- Define the `Skill` trait, `SkillContext`, and `SkillOutput`.
- Carry `SkillMetadata`, compatibility, and capability flags.
- Provide `SkillRegistry` for native, manifest-backed, and markdown-pack skills.
- Parse and represent skill manifests through `SkillManifest` and `ManifestSkillStep`.
- Execute manifest-backed skills through `ManifestSkill`.
- Parse and load markdown skill packs through `MarkdownSkillPack`.
- Ship the built-in native `SummarizeSkill`.

## Out of Scope

- Multi-step workflow orchestration across skills and prompt steps.
- Provider transport behavior and model scheduling.
- Tool execution metadata or gateway routing.
- Session persistence or memory storage.

## Public Boundary

- Traits and types: `Skill`, `SkillContext`, `SkillOutput`.
- Registry: `SkillRegistry`, `RegisteredSkill`.
- Metadata: `SkillMetadata`, `SkillCapabilities`, `SkillCompatibility`.
- Native skill: `SummarizeSkill`.
- Manifest support: `SkillManifest`, `ManifestSkillStep`, `ManifestSkill`.
- Markdown pack support: `MarkdownSkillPack`, `MarkdownSkillFrontmatter`, `SkillSourceKind`.
- Markdown pack execution records: `MarkdownSkillExecutionRecord`, `MarkdownScriptExecutionRecord`.

## Why This Is In `crates/`

Skills are consumed by runtime execution, extension loading, bootstrap, and tests. The boundary is broader than one CLI command, so it belongs in `crates/` where native, manifest-backed, and markdown-pack skill behavior can stay reusable and testable.

## Relationships

- Upstream crates: `mosaic-tool-core` provides tool metadata and execution dependencies exposed to skill implementations.
- Downstream crates: `mosaic-runtime` invokes skills during runs and workflows; `mosaic-extension-core` loads external skills into the registry; `cli` bootstraps built-in skills but should not own skill logic.
- Runtime/control-plane coupling: `runtime` decides when a skill runs, `gateway` exposes runs that may involve skills, and `cli` surfaces those flows. This crate should only describe and execute skills once selected.

## Capability Taxonomy

Inside Mosaic taxonomy, skills are:

- `route_kind=skill`

The source is one of:

- `capability_source_kind=native_skill`
- `capability_source_kind=manifest_skill`
- `capability_source_kind=markdown_skill_pack`

Skills are reusable capability units. They are not MCP adapters, not node routes, and not workflow graphs.

## Sandbox Relationship

Skills may carry sandbox bindings, but this crate does not create or manage sandbox environments itself.

- `mosaic-skill-core` describes skill metadata and execution contracts
- `mosaic-runtime` decides when a skill runs
- `mosaic-sandbox-core` resolves the workspace-local execution environment

This keeps skill semantics separate from env lifecycle and policy enforcement.

## Minimal Use

```rust
use std::sync::Arc;
use mosaic_skill_core::{SkillRegistry, SummarizeSkill};

let mut skills = SkillRegistry::new();
skills.register(Arc::new(SummarizeSkill));
let summarize = skills.get("summarize").expect("skill should exist");
let metadata = summarize.metadata();
```

## Testing

```bash
cargo test -p mosaic-skill-core
```

Tests cover native registration, manifest-backed execution, markdown skill pack loading, and markdown packs that exercise templates, references, helper scripts, and sandbox-linked execution.

## Current Limitations

- Native skill inventory is intentionally small today.
- Manifest execution is sequential and intentionally conservative.
- Markdown skill packs now support templates, references, and helper scripts, but dependency isolation and env lifecycle still belong to `mosaic-sandbox-core`.
- Skill metadata is compatible with extensions, but richer version negotiation is still shallow.

## Roadmap

- Add more first-party native skills without pushing orchestration logic into the crate.
- Expand manifest compatibility/version checks for extension hot reload.
- Keep separating declarative manifest parsing from execution helpers as the skill surface grows.
