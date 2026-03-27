# Non-TUI Architecture Audit

This document is the `plan_h4` audit artifact for every non-TUI crate in the workspace.

`crates/tui` is explicitly out of scope here.

## Scope

Included crates:

- `cli`
- `crates/config`
- `crates/provider`
- `crates/runtime`
- `crates/gateway`
- `crates/tool-core`
- `crates/skill-core`
- `crates/workflow`
- `crates/memory`
- `crates/session-core`
- `crates/inspect`
- `crates/mcp-core`
- `crates/sdk`
- `crates/control-protocol`
- `crates/node-protocol`
- `crates/extension-core`
- `crates/scheduler-core`
- `crates/channel-telegram`

Excluded crate:

- `crates/tui`

## Method

- Read `AGENTS.md` as the architecture source of truth.
- Re-read the current non-TUI implementation instead of relying on past plan intent.
- Require every conclusion below to point at a concrete code path.
- Classify debt as one of three levels: `mature`, `aligned but concentrated`, or `needs deepening`.

## Executive Summary

- The non-TUI system is already architecturally real. It has a real control plane, runtime, session layer, trace contract, extension loader, provider boundary, capability surface, and device-node contract.
- The main remaining problem is not feature absence. It is concentration of too many responsibilities inside a few large implementation files.
- The highest-priority misalignments are `cli/src/main.rs`, `crates/gateway/src/lib.rs`, `crates/runtime/src/lib.rs`, and `crates/extension-core/src/lib.rs`.
- Session, memory, routing, and inspect facts already flow end-to-end, but write ownership is split between Gateway and Runtime and should be made explicit before the system grows further.
- Channel adapters are thin where they exist. `crates/channel-telegram/src/lib.rs:43-107` is a good baseline. The inconsistency is that webchat ingress still lives inside `crates/gateway/src/http.rs:289-333` instead of a dedicated interaction-entry boundary.

## AGENTS Alignment Matrix

| AGENTS focus | Result | Evidence | Required action |
| --- | --- | --- | --- |
| 1. Each crate should have one clear responsibility | Mostly aligned | `crates/provider/src/lib.rs:1-23`, `crates/tool-core/src/lib.rs:1-18`, `crates/skill-core/src/lib.rs:1-19`, `crates/control-protocol/src/lib.rs:10-220`, `crates/sdk/src/lib.rs:15-207` are narrow facade crates; `crates/gateway/src/lib.rs:523-538` and `crates/runtime/src/lib.rs:240-313` show the big remaining concentration points | Keep mature crates stable; split concentrated internals in `specs/plan_i1.md` |
| 2. Stable logic should live in `crates/`, not drift back into `cli` | Partial | `cli/src/bootstrap.rs:33-160` is a good composition root, but `cli/src/main.rs:575-1258`, `cli/src/main.rs:1314-1409`, and `cli/src/main.rs:1938-2235` still carry command business logic, service loops, and output shaping | Move command families and local node serving out of `main.rs` in `specs/plan_i1.md` |
| 3. Gateway should stay the coordinator, not a God Object | Partial | `crates/gateway/src/lib.rs:523-538` stores events, audit, replay, run registry, capability jobs, and metrics together; `crates/gateway/src/lib.rs:592-940` and `crates/gateway/src/lib.rs:1135-1241` show many unrelated control-plane responsibilities on one type | Split gateway internals by state and behavior while preserving `GatewayHandle` |
| 4. Runtime must do orchestration, not collapse into provider or tool glue | Semantically aligned, structurally concentrated | `crates/runtime/src/lib.rs:240-313` chooses profile and branch, `crates/runtime/src/lib.rs:316-520` runs assistant/skill/workflow paths, `crates/runtime/src/lib.rs:1126-1335` handles cross-session context and memory, and `crates/runtime/src/lib.rs:1487-2012` handles guarded tool and node execution | Keep orchestration in runtime, but split it into phase-oriented modules in `specs/plan_i1.md` |
| 5. Capability, node, extension, and config permissions must stay explicit | Mostly aligned | `crates/runtime/src/lib.rs:1525-2012` enforces capability auth/health/retry/timeout, `crates/node-protocol/src/lib.rs:353-439` keeps node capability selection explicit, and `crates/scheduler-core/src/lib.rs:47-110` keeps cron storage narrow; the gap is `crates/extension-core/src/lib.rs:257-365` plus `crates/extension-core/src/lib.rs:580-713`, where extension loading is mixed with builtin construction | Separate extension loading from builtin catalog assembly in `specs/plan_i1.md` |
| 6. Session, memory, routing, and inspect should form one fact flow | Partial | Gateway writes queued/running lifecycle into session state at `crates/gateway/src/lib.rs:1160-1168` and `crates/gateway/src/lib.rs:1395-1415`; Runtime writes references and memory state at `crates/runtime/src/lib.rs:1147-1173` and `crates/runtime/src/lib.rs:1266-1335`; session owns durable fields at `crates/session-core/src/lib.rs:184-280`; inspect owns the trace schema at `crates/inspect/src/lib.rs:12-220` | Define an explicit write-ownership contract in `specs/plan_i1.md` |
| 7. Channel adapters should stay thin and protocol-oriented | Baseline aligned, inconsistent overall | `crates/channel-telegram/src/lib.rs:43-107` is a thin normalizer; `crates/gateway/src/lib.rs:985-997` consumes it cleanly; but webchat normalization lives directly in `crates/gateway/src/http.rs:289-333` | Extract a consistent interaction-entry boundary after the main structural splits land |

## Crate-by-Crate Debt Register

| Crate | Status | Evidence | Remaining debt | Remediation action |
| --- | --- | --- | --- | --- |
| `cli` | needs deepening | `cli/src/bootstrap.rs:33-160` is a clean composition root; `cli/src/main.rs:575-1258`, `cli/src/main.rs:1314-1409`, and `cli/src/main.rs:1938-2235` are still oversized | `main.rs` still owns command dispatch, remote/local branching, node serving, and large amounts of rendering | Move command families and node service loops into dedicated modules in `specs/plan_i1.md` |
| `crates/config` | mature | `crates/config/src/lib.rs:12-24` is now a true facade over `doctor`, `load`, `redaction`, `types`, and `validation` | Keep schema/config concerns from absorbing extension runtime semantics | Monitor only; no new split needed right now |
| `crates/provider` | mature | `crates/provider/src/lib.rs:1-23` is a small facade over capabilities, profile, errors, types, and vendors | The crate is broad by product scope, but its boundary is clear and vendor work is isolated | Keep API stable and evolve vendor adapters inside the existing boundary |
| `crates/runtime` | needs deepening | `crates/runtime/src/lib.rs:240-313`, `crates/runtime/src/lib.rs:316-520`, `crates/runtime/src/lib.rs:1126-1335`, `crates/runtime/src/lib.rs:1487-2012` | Real orchestration exists, but session loading, memory compression, provider loop, workflow glue, and tool/node execution all live in one implementation body | Split runtime by phase and writer ownership in `specs/plan_i1.md` |
| `crates/gateway` | needs deepening | `crates/gateway/src/lib.rs:523-538`, `crates/gateway/src/lib.rs:592-940`, `crates/gateway/src/lib.rs:1135-1241`, `crates/gateway/src/lib.rs:1429-1505` | Gateway semantics are correct, but too much state and too many command surfaces are aggregated into one file and one handle type | Split gateway internals by runs, sessions, audit/replay, ingress, and capability services in `specs/plan_i1.md` |
| `crates/tool-core` | mature | `crates/tool-core/src/lib.rs:1-18` is a small boundary over builtin, metadata, policy, registry, sources, and types | The main risk is accidental re-coupling of builtin behavior and policy during future additions | Monitor only; keep builtin tools and metadata separate |
| `crates/skill-core` | mature | `crates/skill-core/src/lib.rs:1-19` keeps manifest, native, registry, metadata, and types separate | Main remaining work is evolutionary, not structural | Monitor only; keep registry independent from concrete skill implementations |
| `crates/workflow` | mature | `crates/workflow/src/lib.rs:8-39` and `crates/workflow/src/lib.rs:182-323` keep workflow types, registry, observer, and runner together inside one focused crate | Runtime still owns a lot of workflow tracing glue, but the workflow crate boundary itself is clear | Leave crate as-is; reduce runtime-side glue in `specs/plan_i1.md` |
| `crates/memory` | aligned but concentrated | `crates/memory/src/lib.rs:43-116` owns store contracts and `crates/memory/src/lib.rs:143-198` owns summarization/compression helpers | File storage and summarization are clear, but concurrency and ownership still depend on higher layers | Clarify write ownership via `specs/plan_i1.md`; no crate split yet |
| `crates/session-core` | aligned but concentrated | `crates/session-core/src/lib.rs:184-280` shows gateway, run, channel, memory, and reference metadata all living on `SessionRecord` | The model is explicit, but many layers can mutate it; that creates drift risk without a writer contract | Define which layer writes which fields in `specs/plan_i1.md` |
| `crates/inspect` | aligned but concentrated | `crates/inspect/src/lib.rs:12-220` defines the trace contract for tools, capability jobs, ingress, provider attempts, failures, and lifecycle | The schema is correct, but it is growing with every feature and needs stricter writer ownership and version discipline | Keep schema centralized; formalize writer ownership in `specs/plan_i1.md` |
| `crates/mcp-core` | aligned | `crates/mcp-core/src/lib.rs:278-352` keeps MCP lifecycle and tool registration inside one clear boundary | Current lifecycle is still local stdio MCP only; reconnect and richer operational policy are future work | Keep boundary stable; expand behavior later without leaking MCP transport upward |
| `crates/sdk` | mature | `crates/sdk/src/lib.rs:15-207` is a thin HTTP/SSE client over protocol DTOs | No structural debt beyond continuing to mirror the protocol surface cleanly | Monitor only; no split needed |
| `crates/control-protocol` | mature | `crates/control-protocol/src/lib.rs:10-220` is a DTO/event contract crate with no transport or CLI logic | The main long-term risk is versioning, not ownership | Monitor only; keep it serialization-focused |
| `crates/node-protocol` | aligned but concentrated | `crates/node-protocol/src/lib.rs:251-439` cleanly owns node registration, selection, dispatch, and result transport | The node contract is explicit, but the current headless serve loop still lives in `cli/src/main.rs:1314-1409` instead of a dedicated non-CLI service boundary | Extract node serving out of CLI in `specs/plan_i1.md` while keeping this protocol crate stable |
| `crates/extension-core` | needs deepening | `crates/extension-core/src/lib.rs:257-365` loads extension sets, while `crates/extension-core/src/lib.rs:580-713` also hardcodes builtin tool and skill construction | The crate is both extension loader and builtin catalog assembler; that blurs the intended configuration/extension boundary | Separate builtin catalog assembly from extension load policy in `specs/plan_i1.md` |
| `crates/scheduler-core` | mature baseline | `crates/scheduler-core/src/lib.rs:10-110` contains only cron registration and storage | It is intentionally narrow and currently does not carry orchestration policy | Monitor only; keep it storage-focused |
| `crates/channel-telegram` | mature baseline | `crates/channel-telegram/src/lib.rs:43-107` normalizes Telegram payloads into Mosaic ingress facts | The adapter is thin and correct, but overall interaction-entry coverage is incomplete because only Telegram has a dedicated channel crate today | Keep adapter thin; normalize webchat and future channels the same way |

## High-Priority Findings

### 1. `cli/src/main.rs` is still too large for a composition root

What is already good:

- `cli/src/bootstrap.rs:33-160` is the right pattern: assemble shared components, return composed handles, avoid duplicating reusable logic.

What is still off:

- `cli/src/main.rs:575-613` keeps all top-level dispatch in one file.
- `cli/src/main.rs:650-1258` keeps most command-path behavior in the same file.
- `cli/src/main.rs:1314-1409` still hosts the local headless node service loop.
- `cli/src/main.rs:1938-2235` still contains a large amount of inspect and gateway presentation code.

Why this matters:

- AGENTS says `cli/` is the first delivery path and composition root, not the long-term home for stable shared operator behavior.
- The problem is now less about missing crates and more about internal command decomposition.

### 2. Gateway semantics are correct, but the crate is too concentrated

What is already good:

- `GatewayHandle` really is the control-plane center.
- The crate owns routing, audit, replay, run registry, sessions, ingress, and HTTP/SSE exposure in the right architectural layer.

What is still off:

- `crates/gateway/src/lib.rs:523-538` shows one state object holding many distinct stores and coordinators.
- `crates/gateway/src/lib.rs:1135-1241` shows run submission, session sync, audit, event broadcast, and runtime spawn all tightly coupled in one path.
- `crates/gateway/src/lib.rs:1429-1505` shows runtime events immediately mutating run records, session state, metrics, audit, and broadcast output from one sink.

Why this matters:

- AGENTS explicitly warns that Gateway should coordinate without becoming a dumping ground.
- Today the semantics are still correct, but the implementation shape makes future changes risky.

### 3. Runtime is doing real orchestration, but too many phases live together

What is already good:

- `crates/runtime/src/lib.rs:240-313` proves Runtime is choosing profiles and branching across assistant, skill, and workflow flows.
- `crates/runtime/src/lib.rs:316-520` proves Runtime is not a thin provider wrapper.

What is still off:

- `crates/runtime/src/lib.rs:1126-1335` mixes cross-session reference resolution, compression, memory read/write, and session persistence.
- `crates/runtime/src/lib.rs:1487-2012` mixes capability guardrails, timeout/retry, node routing, trace construction, and failure shaping.

Why this matters:

- AGENTS requires Runtime orchestration to stay separate from tool implementation, but it does not require all orchestration phases to stay in one file.
- The current concentration makes cancellation, interruption, and future sub-agent work harder to isolate.

### 4. Extension loading is still coupled to builtin catalog assembly

What is already good:

- Extension policy, validation, and reload exist and are real.
- The crate is correctly reused by bootstrap and Gateway reload.

What is still off:

- `crates/extension-core/src/lib.rs:257-365` loads extension sets and MCP registrations.
- `crates/extension-core/src/lib.rs:580-653` also decides which builtin tools exist and how they are instantiated.
- `crates/extension-core/src/lib.rs:669-713` also decides builtin skill registration.

Why this matters:

- AGENTS positions configuration/extensions as describing behavior on top of a stable core.
- The current design makes builtin catalog evolution and extension loading policy the same problem.

### 5. Fact flow exists, but writer ownership is implicit

Current write path:

- Gateway queues and updates run/session state at `crates/gateway/src/lib.rs:1160-1168` and `crates/gateway/src/lib.rs:1395-1415`.
- Runtime records references and memory state at `crates/runtime/src/lib.rs:1147-1173` and `crates/runtime/src/lib.rs:1266-1335`.
- Session state becomes the durable merged view at `crates/session-core/src/lib.rs:184-280`.
- Inspect stores the parallel trace contract at `crates/inspect/src/lib.rs:12-220`.

Why this matters:

- The architecture already has the right pieces.
- The next failure mode is not missing data. It is conflicting or drifting ownership over the same facts.

## Mature Crates

These crates already match AGENTS.md well enough that they should be treated as stable boundaries, not reopened casually:

- `crates/config`
- `crates/provider`
- `crates/tool-core`
- `crates/skill-core`
- `crates/workflow`
- `crates/sdk`
- `crates/control-protocol`
- `crates/scheduler-core`
- `crates/channel-telegram`

## Crates That Are Correct but Still Concentrated

- `crates/memory`
- `crates/session-core`
- `crates/inspect`
- `crates/mcp-core`
- `crates/node-protocol`

These do not need immediate crate splits. They do need stricter ownership rules around how higher layers use them.

## Remediation Priority Order

1. Decompose `cli/src/main.rs` so the CLI is again mostly a composition root and command dispatcher.
2. Split Gateway internals by run supervision, session/routing sync, audit/replay, ingress, and HTTP.
3. Split Runtime internals by run phase and define explicit writer ownership for session, memory, and trace facts.
4. Separate builtin catalog assembly from extension loading and policy validation.
5. Normalize interaction-entry handling so webchat follows the same adapter pattern as Telegram.

## Follow-On Plan Split

Existing follow-on plan:

- `specs/plan_h5.md` should continue to own docs, examples, and the operator golden path. It is still valid, but it does not remove the structural debts listed here.

New remediation plan:

- `specs/plan_i1.md` is the direct code remediation follow-up for this audit. It should own CLI decomposition, Gateway/runtime internal splits, explicit fact ownership, extension-core cleanup, and interaction-entry consistency.

## Recommendation

Treat this audit as the baseline for all non-TUI follow-up work.

The core judgment is simple:

- non-TUI Mosaic is no longer shallow
- the remaining risks are implementation concentration and ownership ambiguity
- the next round should deepen boundaries, not add more top-level features first
