# AGENTS.md

## Purpose

This document defines how contributors and coding agents should understand, extend, and modify **Mosaic**.

Mosaic must be understood as a **self-hosted AI assistant control plane**, not as a single chat frontend.

It is a long-running, multi-channel, stateful, routable, executable, extensible, and governable Agent system. Its job is not only to answer questions, but to coordinate always-on Agent behavior across channels, sessions, tools, and devices.

---

## System Identity

Mosaic connects five major concerns through a long-running **Gateway**:

- external messaging channels such as WhatsApp, Telegram, Slack, Discord, and WebChat
- internal state such as sessions, memory, routing, permissions, and event streams
- the Agent runtime such as primary agents, sub-agents, model scheduling, and context compression
- operator surfaces such as Web, CLI, and desktop control interfaces
- execution capabilities such as browser, canvas, exec, pdf, image, cron, webhook, and device-node actions

Because of that, Mosaic must never be treated as a thin "chat UI + LLM API" system.

---

## Architecture Layers and Responsibilities

| Layer                             | Representative Components                                             | Primary Responsibilities                                                                            | Design Meaning                                   |
| --------------------------------- | --------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------- | ------------------------------------------------ |
| Interaction Entry Layer           | WhatsApp / Telegram / Slack / Discord / WebChat                       | Accept messages, normalize external payloads, ingest private/group/thread context                   | Put the assistant where users already are        |
| Control Plane Layer               | Gateway, WS protocol, routing, session management, config, Control UI | Unified ingress, auth, session mapping, event broadcast, commands, observability                    | The Gateway is the system hub                    |
| Agent Runtime Layer               | Pi agent, sub-agents, `sessions_*` tools, model selection/switching   | Runtime orchestration, context compression, collaboration, cross-session communication              | Model calls become orchestrated runtime behavior |
| Capability Execution Layer        | exec / browser / canvas / cron / pdf / image / webhook / file tools   | Execute real actions, access environments, trigger automations, read/generate content               | This defines the system ceiling and risk surface |
| Device Node Layer                 | macOS node, iOS node, Android node, headless node                     | Expose device-local abilities such as command execution, notifications, camera, recording, location | Extend the Agent into real devices               |
| Configuration and Extension Layer | `mosaic.json`, skills, plugins, workspace                             | Policies, workspaces, tool strategy, extension packs, hot reload                                    | Enable long-term product evolution               |

---

## Repository Architecture

The repository is a **Cargo Workspace**.

This is not only a packaging decision. It is the main mechanism for keeping the project modular, testable, and evolvable.

### Repository Map

| Path           | Role                              | Rule                                                  |
| -------------- | --------------------------------- | ----------------------------------------------------- |
| `cli/`         | CLI-facing application code       | Default place to start one-step feature work          |
| `crates/`      | Reusable independent Cargo crates | Put shared, stable, testable logic here               |
| workspace root | Shared workspace configuration    | Central dependency policy, linting, build consistency |

### How to Interpret the Workspace

- `cli/` is the **composition root**
- `crates/` is the **reusable module layer**
- the workspace root is the **consistency boundary**

### Default Rule

When a new task is implemented in one step, **start in `cli/` by default**.

Only move logic into `crates/` when one or more of the following becomes true:

- the logic is reused by multiple commands or flows
- the logic is stable enough to deserve a public boundary
- the logic is cross-cutting and not specific to one CLI path
- the logic needs isolated tests and a clear module contract

### Recommended Project Engineering Layout

Use the following project structure as the default repository shape:

```text
mosaic/
├── Cargo.toml              # workspace root, members, [workspace.dependencies]
├── Makefile                # install / build / clean / check entrypoints
├── README.md
├── cli/
│   ├── Cargo.toml          # CLI application dependencies
│   └── src/
│       └── main.rs         # install target and CLI entrypoint
└── crates/
    ├── runtime/            # agent runtime core
    ├── provider/           # LLM providers (OpenAI / Claude / ...)
    ├── tools/              # tool system
    ├── skills/             # reusable capability modules
    ├── memory/             # memory / vector / kv
    ├── orchestrator/       # multi-agent / workflow
    ├── sdk/                # external SDK
    ├── tui/
    │   ├── Cargo.toml      # terminal UI crate dependencies
    │   └── src/
    └── gateway/
        ├── Cargo.toml      # gateway crate dependencies
        └── src/
```

### Repository Structure Rules

#### Workspace Root

The root `Cargo.toml` owns:

- workspace members
- `workspace.dependencies`
- shared version policy
- shared lint/build consistency

Do not scatter shared dependency policy across many crates unless there is a crate-specific reason.

#### `cli/`

`cli/src/main.rs` should be the main installation target and CLI entrypoint.

It should own:

- process startup
- argument parsing and command dispatch
- top-level wiring
- runtime/bootstrap composition
- first-step feature integration

It should not become the permanent home for reusable UI internals, protocol types, or stable shared logic.

#### `crates/tui/`

`crates/tui/` is the terminal UI crate.

It should own:

- TUI rendering
- terminal event loop abstractions
- terminal view state
- keyboard interaction behavior
- reusable terminal components

The CLI should launch and compose it, but the UI implementation itself should live in `crates/tui/` once the boundary is clear.

#### `crates/gateway/`

`crates/gateway/` is the Gateway crate.

It should own:

- ingress coordination
- routing and session mapping
- WS/event protocol handling
- control-plane dispatch
- runtime-facing coordination boundaries

Do not hide Gateway semantics inside generic infrastructure crates.

### Makefile Conventions

The repository root should expose a `Makefile` as the standard developer entrypoint for common CLI workflows.

At minimum, define these targets:

- `install` — install the CLI from `cli/`
- `build` — build the CLI
- `clean` — clean build artifacts
- `check` — run workspace checks

Recommended command mapping:

```make
install:
	cargo install --path cli

build:
	cargo build -p cli

clean:
	cargo clean

check:
	cargo check --workspace
```

### Makefile Rules

- prefer invoking the workspace from the repository root
- keep target names stable and obvious
- `install`, `build`, `clean`, and `check` should work without requiring developers to remember crate paths
- do not hide critical build logic in undocumented shell scripts when a root `Makefile` target is sufficient
- when new essential workflows appear, expose them consistently through the root `Makefile`

---

## `cli/` Rules

Treat `cli/` as:

- the main local entrypoint
- the assembly layer for real workflows
- the safest place for incremental delivery
- the place where multiple crates are wired together into runnable behavior

Use `cli/` for:

- command entrypoints
- flow composition
- temporary first implementations
- integration-oriented orchestration
- developer/operator-facing command behavior

Do not use `cli/` for:

- long-term shared business logic
- reusable protocols that belong to a crate
- duplicated implementations across commands
- infrastructure helpers that multiple modules need

---

## `crates/` Rules

Each crate under `crates/` should have:

- one clear responsibility
- explicit public interfaces
- minimal coupling
- isolated tests where practical
- no hidden application assembly concerns

Good candidates for `crates/` include:

- protocol types
- routing abstractions
- adapters
- runtime helpers
- domain modules
- infrastructure modules
- execution abstractions
- reusable session/state components

Avoid creating crates too early when code is still changing rapidly.

Avoid keeping logic in `cli/` once it is clearly shared, repeated, or architecturally important.

---

## `neocrates` Positioning

`neocrates` should be treated as a **foundational infrastructure facade crate**, not as a place to hide Mosaic-specific business behavior.

Use it for low-level shared capabilities such as:

- web utilities and middleware
- AWS integrations
- Diesel helpers and pooling
- Redis helpers
- cryptography helpers
- authentication helpers
- structured logging

Do not use it for:

- Gateway semantics
- session ownership rules
- channel-specific orchestration
- Agent runtime policies
- Mosaic product semantics

### Boundary Rule

`neocrates` provides capabilities.

Mosaic crates define product meaning.

Keep those two concerns separate.

---

## Implementation Decision Tree

Before writing code, answer these in order:

1. **Where does the event enter?**
   - channel adapter
   - control surface
   - scheduled trigger
   - webhook
   - device node callback

2. **Who routes it?**
   - Gateway
   - session router
   - runtime dispatcher

3. **Who owns the state?**
   - session
   - agent
   - node
   - workflow/task

4. **What executes it?**
   - agent runtime only
   - tool layer
   - device node
   - external automation

5. **Does it change public behavior?**
   - protocol
   - API
   - session semantics
   - node contract
   - configuration format

6. **Where should the code live?**
   - `cli/` for first-path composition
   - `crates/` for reusable logic
   - `neocrates` only for generic infrastructure capability

7. **How is it controlled?**
   - auth
   - permission boundary
   - logging/audit
   - observability
   - retry/timeout/interruption

If these questions are not answered, the implementation boundary is probably still unclear.

---

## Layer-by-Layer Rules

### 1. Interaction Entry Layer

Responsibilities:

- adapt external channel payloads
- normalize identities and message structures
- preserve group/private/thread/reply context

Rules:

- keep adapters thin
- normalize before entering the core system
- do not embed business orchestration into channel adapters
- do not bind a channel directly to a specific model or tool strategy

### 2. Control Plane Layer

Responsibilities:

- ingress and authentication
- session mapping and state synchronization
- routing and event dispatch
- control commands
- observability and traceability

Rules:

- the Gateway coordinates; it should not become a God Object
- keep state transitions explicit and observable
- do not bury complex tool logic inside the Gateway
- preserve protocol compatibility when changing routing paths

### 3. Agent Runtime Layer

Responsibilities:

- agent and sub-agent coordination
- model selection, switching, and fallback
- context compression and memory read/write
- intent-to-action planning

Rules:

- do not reduce the runtime to one LLM call per request
- keep orchestration separate from tool implementation
- keep sub-agents focused and replaceable
- keep session tools about runtime/session concerns, not unrelated UI/device concerns

### 4. Capability Execution Layer

Responsibilities:

- execute commands, browser actions, canvas tasks, cron jobs, document processing, image/PDF work, and similar actions

Rules:

- every high-privilege action must have explicit permission boundaries
- keep tool contracts stable
- support timeout, retry, interruption, and failure reporting for side effects
- do not leak one tool implementation's internals into upper layers

### 5. Device Node Layer

Responsibilities:

- expose device-local capabilities
- act as the remote execution proxy inside real user environments

Rules:

- capabilities must be declared explicitly
- do not allow hidden privilege escalation
- keep node-control-plane protocols stable and reconnectable
- treat disconnect/reconnect/state drift as first-class concerns

### 6. Configuration and Extension Layer

Responsibilities:

- policies, workspaces, plugins, skills, toggles, hot-reloadable extension behavior

Rules:

- configuration must describe behavior, not replace architecture
- plugin and skill contracts must stay stable and version-aware
- hot reload must account for rollback, safety, and compatibility

---

## Contributor Defaults

Use these defaults unless there is a strong reason not to.

### Start Here

- start first-pass feature work in `cli/`
- reuse existing flows before inventing new ones
- extract repeated semantics into a crate
- keep protocols and contracts compatible unless the change explicitly requires a break

### Do Not Do These Casually

- do not casually change public interfaces
- do not casually change WS/event/session protocols
- do not casually rewrite Gateway coordination paths
- do not casually move unstable code into shared crates too early
- do not casually duplicate the same behavior in multiple places
- do not casually bypass authorization, logging, or audit trails

### When Repeated Logic Appears

Extract repeated code into one of:

- shared methods
- reusable modules
- shared protocol types
- shared components
- dedicated crates
- common execution abstractions

Prefer one semantic implementation over many slightly different copies.

---

## What New Work Should Usually Look Like

### Adding a New Channel

Do:

- build an adapter
- normalize events into the internal model
- reuse existing session/routing behavior

Do not:

- put Gateway logic into the adapter
- bind the adapter directly to one specific model/tool chain

### Adding a New Tool

Do:

- define a stable tool interface
- preserve permission boundaries
- add timeout/retry/interruption behavior
- wire it through existing orchestration layers

Do not:

- let one implementation detail leak upward
- bypass audit or authorization

### Adding a New Device Capability

Do:

- define the capability explicitly
- handle offline/reconnect/version mismatch
- report state and failures clearly

Do not:

- assume the node is always online
- assume device capabilities are uniform

### Adding a New Runtime Behavior

Do:

- design from session ownership and routing first
- keep orchestration inside the runtime layer
- extract stable abstractions when shared

Do not:

- solve it only with prompt changes
- mix runtime policy with low-level infrastructure helpers

---

## Change Safety Checklist

Before merging a change, verify:

- existing APIs still behave compatibly unless intentionally changed
- existing channels still work
- session semantics are not unintentionally broken
- authorization and audit are preserved
- tool side effects are observable and interruptible
- repeated logic has not been copied unnecessarily
- shared logic is in `crates/` when it is truly reusable
- unstable logic has not been over-extracted too early
- `neocrates` has not become a dumping ground for Mosaic-specific semantics

---

## Testing Expectations

Prefer the smallest test scope that proves the change safely.

### For `cli/`

Prefer:

- integration-oriented tests
- command-path regression tests
- workflow assembly verification

### For `crates/`

Prefer:

- isolated unit tests
- focused module tests
- stable contract tests

### General Rule

When fixing a bug:

- first preserve existing behavior with a regression test when practical
- then add only focused coverage for the new edge case
- do not introduce broad noisy tests that make future iteration harder

---

## Architectural Summary

Mosaic is closer to an **Agent OS / Agent control plane** than to a chat application.

That means:

- channels are entrypoints, not the center
- the Gateway is a coordinator, not a dumping ground
- the runtime is orchestration, not just prompt calling
- tools and nodes define both capability and risk
- configuration and extensions must sit on top of a stable core
- the Cargo Workspace is part of the architecture
- `cli/` is the default first delivery path
- `crates/` is the long-term reusable module layer

Every meaningful change should make the system more:

- modular
- explicit
- testable
- observable
- permission-aware
- reusable
- governable
