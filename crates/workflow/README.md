# mosaic-workflow

`mosaic-workflow` defines Mosaic workflow data structures and the generic sequential workflow runner.

## Positioning

This crate gives Mosaic a reusable workflow abstraction that is independent from the runtime's concrete provider/tool/session wiring.

## Architecture Layer

Agent Runtime Layer.

## Responsibilities

- Define `Workflow`, `WorkflowStep`, and `WorkflowStepKind`.
- Define workflow metadata and compatibility through `WorkflowMetadata` and `WorkflowCompatibility`.
- Provide `WorkflowRegistry`.
- Provide the generic runner through `WorkflowRunner`.
- Define execution/observer contracts through `WorkflowStepExecutor` and `WorkflowObserver`.

## Out of Scope

- Concrete prompt execution, tool loops, or session updates.
- Gateway routing or HTTP ingress.
- TUI or CLI rendering.
- Provider transport logic.

## Public Boundary

- Types: `Workflow`, `WorkflowStep`, `WorkflowStepKind`, `WorkflowContext`, `WorkflowStepExecution`, `WorkflowRunResult`, `WorkflowMetadata`, `RegisteredWorkflow`, `WorkflowRegistry`.
- Traits: `WorkflowStepExecutor`, `WorkflowObserver`.
- Runner: `WorkflowRunner`, `NoopWorkflowObserver`.

## Why This Is In `crates/`

Workflow semantics are reused by runtime, extensions, config parsing, tests, and examples. That makes workflow definition a reusable contract, not a CLI-only feature.

## Relationships

- Upstream crates: none; this crate is intentionally runtime-agnostic.
- Downstream crates: `mosaic-runtime` provides a concrete `WorkflowStepExecutor`, `mosaic-config` embeds workflows in app config, and `mosaic-extension-core` registers workflows from extensions.
- Runtime/control-plane coupling: this crate describes and runs workflow structure, while runtime owns the concrete step side effects and gateway owns run supervision.

## Capability Taxonomy

Inside Mosaic taxonomy, workflows are:

- `route_kind=workflow`
- `execution_target=workflow_engine`

Workflows are orchestration units. They may call prompt steps, tools, and skills, but they are not themselves tools or MCP adapters.

## Sandbox Relationship

Workflows are not sandbox environments.

They may invoke steps whose concrete execution touches tools, skills, MCP servers, or sandboxed envs, but the workflow crate only models orchestration structure.

## Minimal Use

```rust
use mosaic_workflow::{WorkflowRegistry, WorkflowRunner};

let mut registry = WorkflowRegistry::new();
registry.register(workflow);
let result = WorkflowRunner::run(&workflow, "input".to_owned(), &executor).await?;
```

## Testing

```bash
cargo test -p mosaic-workflow
```

## Current Limitations

- Workflows are sequential today.
- Compatibility policy is intentionally simple.
- Runtime-specific concerns are delegated to callers.

## Roadmap

- Preserve a generic runner while runtime features grow around it.
- Expand compatibility metadata as extension and upgrade flows mature.
- Keep workflow definition decoupled from provider and gateway specifics.
