# mosaic-runtime

`mosaic-runtime` is the orchestration engine for one Mosaic run. It binds provider calls, tool loops, skills, workflows, sessions, and memory into a single execution lifecycle.

## Positioning

This crate is where Mosaic turns a submitted request into a stateful agent run. It should stay focused on orchestration, not on HTTP ingress, CLI dispatch, or vendor-specific transport internals.

## Architecture Layer

Agent Runtime Layer.

## Responsibilities

- Define the run entrypoint through `AgentRuntime`, `RuntimeContext`, `RunRequest`, `RunResult`, and `RunError`.
- Emit lifecycle events through `events::RunEvent` and `RunEventSink`.
- Schedule providers, execute prompt/tool loops, dispatch skills, and run workflows.
- Load and update sessions via `mosaic-session-core`.
- Read, summarize, compress, and write memory via `mosaic-memory`.
- Classify provider, workflow, tool, session, and memory failures into inspectable traces.

## Out of Scope

- Gateway auth, HTTP handlers, replay windows, or incident export.
- Provider transport details beyond calling the `mosaic-provider` boundary.
- TUI rendering or CLI argument parsing.
- Long-lived background service management.

## Public Boundary

- Entry types: `AgentRuntime`, `RuntimeContext`, `RunRequest`, `RunResult`, `RunError`.
- Event surface: `events::RunEvent`, `events::RunEventSink`, `events::CompositeEventSink`, `events::SharedRunEventSink`.

## Why This Is In `crates/`

Runtime orchestration is the core reusable behavior of Mosaic. Both `cli` and `mosaic-gateway` submit runs through it, tests need isolated runtime coverage, and TUI/Gateway event consumers depend on a stable runtime event model.

## Relationships

- Upstream crates: `mosaic-provider`, `mosaic-tool-core`, `mosaic-skill-core`, `mosaic-workflow`, `mosaic-session-core`, `mosaic-memory`, `mosaic-node-protocol`, and `mosaic-inspect`.
- Downstream crates: `mosaic-gateway` owns long-lived control-plane coordination around this runtime; `cli` creates local runtime contexts; `mosaic-tui` consumes runtime events through buffers and gateway sessions.
- Runtime/control-plane coupling: `cli` and `gateway` should submit requests and observe results, but the actual run loop belongs here.

## Sandbox Relationship

Runtime is where sandbox selection becomes run behavior.

- it binds tool and skill metadata to sandbox env identity
- it records sandbox decisions in traces
- it should not own sandbox filesystem layout or CLI lifecycle commands

That boundary belongs to `mosaic-sandbox-core` and operator surfaces.

## Minimal Use

```rust
use mosaic_runtime::{AgentRuntime, RunRequest};

let runtime = AgentRuntime::new(ctx);
let result = runtime
    .run(RunRequest {
        run_id: None,
        system: None,
        input: "hello".to_owned(),
        skill: None,
        workflow: None,
        session_id: Some("demo".to_owned()),
        profile: None,
        ingress: None,
    })
    .await?;
```

`ctx` is a fully wired `RuntimeContext` built by CLI or Gateway bootstrap code.

## Testing

```bash
cargo test -p mosaic-runtime
```

Tests cover provider-only runs, session persistence, tool loops, workflow traces, node routing, memory writes, and ingress trace binding.

## Current Limitations

- One run is still executed in-process; distributed orchestration is not first-class.
- Sub-agent orchestration is not yet a dedicated runtime subsystem.
- Streaming output is modeled as chunked events after generation, not true incremental provider streaming.

## Roadmap

- Expand runtime planning beyond the current single-agent orchestration loop.
- Add stronger interruption and cancellation semantics for long-running tool and workflow steps.
- Keep splitting runtime internals into focused modules while preserving the existing public boundary.
