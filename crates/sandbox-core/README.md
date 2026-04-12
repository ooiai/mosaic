# mosaic-sandbox-core

`mosaic-sandbox-core` owns the workspace-local sandbox layout, sandbox environment identities, and sandbox lifecycle helpers used by runtime, CLI, and diagnostics.

## Positioning

This crate is the shared sandbox boundary for Mosaic. It keeps workspace-local execution environment layout and sandbox status logic out of CLI glue, config rendering, and runtime orchestration code.

## Architecture Layer

Capability Execution Layer.

## Responsibilities

- Define sandbox kinds, scopes, bindings, strategies, and environment records.
- Resolve the `.mosaic/sandbox/` directory layout for one workspace.
- Create and inspect workspace-local sandbox environments and per-run work directories.
- Persist sandbox environment metadata so CLI, doctor, and inspect can reason about the same state.
- Prepare isolated Python environments and Node workspace directories inside the workspace sandbox root.

## Out of Scope

- Provider orchestration or Gateway routing.
- Tool or skill policy decisions.
- Network package installation policy beyond creating isolated local environment roots.
- Human-facing CLI rendering.

## Public Boundary

- Core types: `SandboxKind`, `SandboxScope`, `SandboxBinding`, `SandboxSettings`, `SandboxPaths`.
- Strategy types: `PythonEnvStrategy`, `NodeEnvStrategy`, `SandboxCleanupPolicy`.
- Lifecycle types: `SandboxEnvRecord`, `SandboxEnvStatus`, `SandboxRuntimeStatus`, `SandboxCleanReport`.
- Manager entrypoint: `SandboxManager`.

## Why This Is In `crates/`

Sandbox layout and lifecycle are shared by config doctor flows, runtime execution tracing, inspect output, and operator CLI commands. They are a stable cross-cutting concern and should not be buried in one command path or runtime module.

## Relationships

- Upstream crates: none. This crate is intentionally infrastructure-focused.
- Downstream crates: `mosaic-config` uses the public types in schema and diagnostics, `mosaic-runtime` binds capabilities to sandbox environments, `mosaic-gateway` carries sandbox configuration into runtime contexts, and `cli` exposes operator sandbox commands.
- Runtime/control-plane coupling: runtime decides when a capability needs a sandbox binding; this crate resolves where that binding lives and how the workspace-local env is represented.

## Minimal Use

```rust
use mosaic_sandbox_core::{SandboxBinding, SandboxKind, SandboxManager, SandboxScope, SandboxSettings};

let manager = SandboxManager::new("/workspace", SandboxSettings::default());
manager.ensure_layout()?;
let record = manager.ensure_env(&SandboxBinding::new(
    SandboxKind::Python,
    "notes",
    SandboxScope::Capability,
    vec!["python".to_owned()],
))?;
assert!(record.env_dir.exists());
```

## Testing

```bash
cargo test -p mosaic-sandbox-core
```

Tests cover workspace-local layout creation, Python and Node environment preparation, rebuild behavior, and workspace isolation.

## Current Limitations

- Python support creates a local `venv`, but does not automatically install remote packages.
- Node support prepares isolated workspace directories and dependency manifests, but does not run package manager installs yet.
- Shell and processor environments currently focus on layout and traceability rather than full execution adapters.

## Roadmap

- Add richer dependency-install lifecycle with explicit network and policy gates.
- Expand processor and shell environment preparation as more execution targets land.
- Keep the workspace-local sandbox contract stable while runtime and capability layers deepen.

