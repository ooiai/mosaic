# mosaic-inspect

`mosaic-inspect` defines the saved run trace schema used across Mosaic for observability, incident review, and operator inspection.

## Positioning

This crate is the trace contract. Runtime and gateway write into it, while CLI, SDK, and operator tooling read from it. Keeping it separate prevents trace semantics from being buried inside runtime or CLI presentation code.

## Architecture Layer

Control Plane Layer.

## Responsibilities

- Define `RunTrace` and all nested trace records for tools, workflows, skills, provider attempts, memory reads/writes, governance, ingress, and side effects.
- Provide summary helpers such as `RunSummary`.
- Save traces to disk through `RunTrace::save_to_default_dir` and `RunTrace::save_to_dir`.
- Preserve backward-compatible trace loading for older run files.

## Out of Scope

- Runtime orchestration or provider transport.
- Gateway ingress/auth logic.
- CLI rendering and final human-readable output formatting.
- TUI state or event streaming.

## Public Boundary

- Root trace types: `RunTrace`, `RunSummary`, `RunLifecycleStatus`, `RunFailureTrace`.
- Sub-traces: `ToolTrace`, `CapabilityInvocationTrace`, `SkillTrace`, `WorkflowStepTrace`, `MemoryReadTrace`, `MemoryWriteTrace`, `CompressionTrace`, `ModelSelectionTrace`, `ProviderAttemptTrace`, `ProviderFailureTrace`, `IngressTrace`, `GovernanceTrace`, `ExtensionTrace`, `ExtensionUsageTrace`.

## Why This Is In `crates/`

The trace schema is shared by runtime writers, gateway incident tooling, CLI inspect commands, SDK DTOs, and tests. It is a cross-cutting contract and should not be owned by any one operator surface.

## Relationships

- Upstream crates: `mosaic-tool-core` contributes tool-source metadata embedded in traces.
- Downstream crates: `mosaic-runtime` records run execution, `mosaic-gateway` records lifecycle/governance context, `cli` reads and renders traces, and `mosaic-control-protocol` reuses ingress-related types.
- Runtime/control-plane coupling: `runtime` and `gateway` write trace facts here, while `cli` and external tooling interpret them. This crate should not own presentation logic or orchestration.

## Minimal Use

```rust
use mosaic_inspect::RunTrace;

let trace = RunTrace::new("hello".to_owned());
let path = trace.save_to_default_dir()?;
```

## Testing

```bash
cargo test -p mosaic-inspect
```

Tests cover save/load, summary output, lifecycle status, ingress binding, and compatibility defaults.

## Current Limitations

- Trace persistence is file-oriented.
- The schema captures current runtime and gateway flows, but not every future distributed control-plane concern.
- Rendering concerns remain outside this crate, so rich operator views depend on downstream consumers.

## Roadmap

- Keep extending trace coverage as runtime, gateway, and capability surfaces expand.
- Preserve backward compatibility for stored traces as fields are added.
- Add more explicit machine-facing helpers for downstream observability tooling.
