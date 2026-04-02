# mosaic-extension-core

`mosaic-extension-core` loads and validates external extension manifests into concrete tool, skill, workflow, and MCP registrations.

## Positioning

This crate is the extension assembly boundary. It lets Mosaic describe extension policy and hot-reloadable packs without forcing `cli` or `gateway` to duplicate extension parsing and registry wiring.

## Architecture Layer

Configuration and Extension Layer.

## Responsibilities

- Define extension status and validation reporting through `ExtensionStatus`, `ExtensionValidationIssue`, and `ExtensionValidationReport`.
- Validate extension packs with `validate_extension_set`.
- Load extension packs into runtime-ready registries with `load_extension_set`.
- Apply policy checks for exec/webhook/cron/MCP exposure.
- Wrap built-in and app-inline content into extension-aware metadata.

## Out of Scope

- Gateway hot reload orchestration itself.
- Runtime orchestration after a tool/skill/workflow is selected.
- Provider transport or session persistence.
- TUI and CLI rendering.

## Public Boundary

- Types: `ExtensionStatus`, `ExtensionValidationIssue`, `ExtensionValidationReport`, `LoadedExtensionSet`.
- Entry functions: `validate_extension_set`, `load_extension_set`.

## Why This Is In `crates/`

Extension policy is shared by bootstrap, gateway reload, CLI validation, and tests. It is a reusable configuration/runtime bridge, not a one-command helper.

## Relationships

- Upstream crates: `mosaic-config`, `mosaic-tool-core`, `mosaic-skill-core`, `mosaic-workflow`, `mosaic-mcp-core`, and `mosaic-scheduler-core`.
- Downstream crates: `mosaic-gateway` owns live reload and active extension state; `cli` exposes validate/reload commands; `mosaic-runtime` receives the resulting registries indirectly through bootstrap.
- Runtime/control-plane coupling: `gateway` supervises reload, `runtime` consumes the loaded registries, and `cli` explains validation state. This crate should not own HTTP or run execution.

## Sandbox Relationship

Extensions may describe sandbox bindings on tools and skills, but this crate does not create environments or enforce runtime policy.

- it preserves sandbox metadata through loading and validation
- runtime and sandbox-core turn that metadata into real execution state

This keeps extension manifests declarative instead of turning them into execution engines.

## Minimal Use

```rust
use mosaic_extension_core::{load_extension_set, validate_extension_set};

let report = validate_extension_set(&config, app_config.as_ref(), workspace_root);
let loaded = load_extension_set(&config, app_config.as_ref(), workspace_root, cron_store)?;
```

## Testing

```bash
cargo test -p mosaic-extension-core
```

## Current Limitations

- Extension lifecycle is still local-workspace oriented.
- Compatibility/version policy is intentionally conservative.
- Built-in and extension content share the same registry model, which keeps things simple but not deeply isolated.

## Roadmap

- Keep improving compatibility checks and rollback-safe reload behavior.
- Expand extension packaging patterns without changing the loaded registry boundary.
- Preserve clear policy enforcement between extension description and gateway/runtime execution.
