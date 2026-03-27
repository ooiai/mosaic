# mosaic-config

`mosaic-config` owns Mosaic configuration loading, layered merge behavior, validation, doctor output, and redacted operator views.

## Positioning

This crate is the configuration boundary for the workspace. It keeps config schema and validation policy out of CLI command glue and out of runtime/gateway execution logic.

## Architecture Layer

Configuration and Extension Layer.

## Responsibilities

- Define `AppConfig`, `MosaicConfig`, provider profile config, deployment/auth/audit/observability policy structs, and extension manifest references.
- Load YAML config and extension manifests from disk.
- Merge default, user, workspace, environment, and CLI override layers.
- Validate config through `validate_mosaic_config`.
- Produce operational diagnostics through `doctor_mosaic_config`.
- Produce redacted operator views through `redact_mosaic_config`.

## Out of Scope

- Runtime orchestration or provider HTTP calls.
- Gateway session routing or run supervision.
- TUI rendering or CLI command dispatch.
- Extension loading beyond parsing and configuration policy.

## Public Boundary

- Core config types: `MosaicConfig`, `AppConfig`, `ProviderProfileConfig`, `LoadConfigOptions`, `LoadedMosaicConfig`.
- Validation and doctor types: `ValidationReport`, `ValidationIssue`, `DoctorReport`, `DoctorCheck`, `DoctorSummary`.
- Helper entrypoints: `load_from_file`, `load_mosaic_config`, `save_mosaic_config`, `init_workspace_config`, `validate_mosaic_config`, `doctor_mosaic_config`, `redact_mosaic_config`.
- Provider helpers: `ProviderType`, `parse_provider_type`, `supported_provider_types`, `ACTIVE_PROFILE_ENV`.

## Why This Is In `crates/`

Configuration is shared by bootstrap, CLI setup/config commands, gateway startup, provider profile construction, and tests. It is a stable cross-cutting concern, so it belongs in `crates/` rather than inside one command path.

## Relationships

- Upstream crates: `mosaic-skill-core` and `mosaic-workflow` contribute manifest/workflow types embedded in app config.
- Downstream crates: `cli` exposes setup/config flows, `mosaic-provider` builds profiles from config, `mosaic-gateway` bootstraps service state from loaded config, and `mosaic-extension-core` consumes extension manifest references.
- Runtime/control-plane coupling: `cli` explains config, `gateway` uses it to assemble control-plane state, and `runtime` should only receive already-resolved configuration through context.

## Minimal Use

```rust
use mosaic_config::{LoadConfigOptions, load_mosaic_config, validate_mosaic_config};

let loaded = load_mosaic_config(&LoadConfigOptions::default())?;
let report = validate_mosaic_config(&loaded.config);
assert!(!report.has_errors());
```

## Testing

```bash
cargo test -p mosaic-config
```

Tests cover layered merge behavior, validation, doctor output, extension manifest parsing, and example config compatibility.

## Current Limitations

- Configuration is file-based and synchronous.
- Schema evolution is conservative and currently centered on one schema version.
- Doctor output focuses on current workspace/operator surfaces, not every future deployment topology.

## Roadmap

- Keep the schema explicit while adding richer policy surfaces.
- Expand config-source diagnostics and compatibility tooling for upgrades.
- Preserve a small public boundary even as more deployment and extension features land.
