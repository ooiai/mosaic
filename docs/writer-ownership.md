# Writer Ownership

This guide defines which non-TUI subsystem is allowed to write which durable facts.

The goal is simple:

- `gateway` owns control-plane lifecycle facts
- `runtime` owns transcript, reference, and memory facts
- `session-core` owns the durable data model only
- `inspect` owns trace rendering and schema only

If a change cannot point at the writer listed below, the boundary is probably wrong.

## Gateway Writers

`mosaic-gateway` is the only layer that should author control-plane lifecycle state.

Gateway writes:

- run registry records under `.mosaic/runs`
- `gateway_run_id`, `correlation_id`, and `session_route`
- session run lifecycle fields such as `current_gateway_run_id`, `current_correlation_id`, `run.status`, `last_error`, and `last_failure_kind`
- audit log, replay window, and incident bundle state
- adapter/operator auth outcomes and replay-visible event envelopes

Gateway may forward runtime results, but it should not become the author of transcript or memory facts.

## Runtime Writers

`mosaic-runtime` is the only layer that should author agent execution facts.

Runtime writes:

- transcript messages
- `last_run_id`
- provider/profile binding selected for the run
- ingress-derived channel context on the session
- memory summaries, compressed context, memory write traces, and cross-session references
- provider/tool/skill/workflow/capability traces inside `RunTrace`

Runtime must not stamp gateway lifecycle identifiers such as `gateway_run_id` or `correlation_id` into session state.

## Session-Core Boundary

`mosaic-session-core` owns durable structs and persistence helpers only.

It provides methods such as:

- `set_gateway_binding`
- `set_run_state`
- `append_message`
- `set_memory_state`

Those methods do not decide which subsystem should call them. The caller decides. That caller must follow the ownership rules in this document.

## Inspect Boundary

`mosaic-inspect` owns the trace schema and report rendering.

Inspect may:

- deserialize saved traces
- render summaries and verbose reports
- expose compatibility for old trace files

Inspect must not decide where a fact comes from or backfill session/memory state on its own.

## Interaction Entry Consistency

Channel adapters normalize external payloads before they become core events.

Current baseline:

- Telegram normalization stays in `mosaic-channel-telegram`
- Webchat normalization lives in `mosaic-gateway::ingress`

Both paths must end in the same interaction-entry shape before session routing or runtime execution.

## Regression Anchors

The following tests protect this boundary today:

- `crates/runtime/src/tests/mod.rs`: runtime session persistence keeps gateway lifecycle ids unset
- `crates/gateway/src/tests/mod.rs`: gateway submission stamps control-plane lifecycle metadata
- `crates/gateway/src/ingress.rs`: webchat normalization matches the shared interaction-entry shape
