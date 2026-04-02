# Sandbox

Mosaic sandboxing is not only a path allowlist.

It is the combination of:

- execution policy sandbox
- execution environment sandbox

This document is the operator-facing source of truth after `plan_l2` and `plan_l5`.

See also:

- [capabilities.md](./capabilities.md)
- [skills.md](./skills.md)
- [configuration.md](./configuration.md)
- [examples/sandbox/README.md](../examples/sandbox/README.md)

## Two Layers

### Execution Policy Sandbox

This layer controls whether a capability is allowed to run and under which constraints.

Typical concerns:

- capability allow or deny
- path restrictions
- attachment restrictions
- retry, timeout, interruption
- risk classification
- audit and inspect visibility

### Execution Environment Sandbox

This layer controls where a capability's runtime environment lives.

Typical concerns:

- workspace-local env directories
- Python env isolation
- Node env isolation
- run workdirs
- cached dependencies
- rebuild, clean, and inspect lifecycle
- helper-script execution for markdown skill packs

This is what prevents skill or tool dependencies from polluting the host machine or other workspaces.

## Workspace Layout

Mosaic manages sandbox state under `.mosaic/sandbox/`.

Current layout:

```text
.mosaic/sandbox/
├── python/
│   ├── envs/
│   └── cache/
├── node/
│   ├── envs/
│   └── cache/
├── shell/
│   └── envs/
├── processors/
├── work/
│   └── runs/
└── attachments/
```

The exact directory set is owned by `mosaic-sandbox-core`.

## Python and Node Isolation

Current v1 direction:

- Python envs live under `.mosaic/sandbox/python/envs/...`
- Node envs live under `.mosaic/sandbox/node/envs/...`
- per-run workdirs live under `.mosaic/sandbox/work/runs/...`

The goal is:

- dependencies stay workspace-local
- skill and tool envs do not write into global Python or Node package locations by default
- multiple workspaces do not share execution env state unless explicitly designed to
- markdown skill pack helpers run inside the selected sandbox env instead of the host Python/Node installation

## Config Model

Workspace-level sandbox settings:

```yaml
sandbox:
  base_dir: .mosaic/sandbox
  python:
    strategy: venv
    install:
      enabled: true
      timeout_ms: 120000
      retry_limit: 0
      allowed_sources:
        - registry
        - file
  node:
    strategy: npm
    install:
      enabled: true
      timeout_ms: 120000
      retry_limit: 0
      allowed_sources:
        - registry
        - file
  cleanup:
    run_workdirs_after_hours: 24
    attachments_after_hours: 24
```

Capability-level binding:

```yaml
skills:
  - type: markdown_pack
    name: operator_note
    path: ./examples/skills/operator-note
    sandbox:
      kind: python
      env_name: operator-note-pack
      scope: capability
      dependency_spec:
        - jinja2
```

Key ideas:

- workspace config defines the default sandbox layout and strategies
- install policy controls whether dependency installs are allowed, how long they may run, how many retries are attempted, and whether `registry` and/or `file` sources are accepted
- capability bindings define which env identity a tool or skill should use
- runtime allocates a run workdir even when a capability env is reused

## Lifecycle States

Sandbox env records now expose explicit lifecycle states:

- `preparing`
- `ready`
- `drifted`
- `failed`
- `rebuild_required`

Operators should be able to tell whether an env was newly prepared, safely reused, drifted on disk, blocked by install policy, or failed during create/install/health-check.

## CLI Commands

Use the sandbox commands to inspect and manage workspace-local environments:

```bash
mosaic sandbox status
mosaic sandbox list
mosaic sandbox inspect <env-id>
mosaic sandbox rebuild <env-id>
mosaic sandbox clean
```

These commands should be the first stop when diagnosing capability env issues.

## Sandbox vs Node

Node and sandbox are not the same thing.

- sandbox answers: where is the env and what is allowed here?
- node answers: which external execution target is running the capability?

A node-routed tool may still need sandbox-aware policy, but node execution is expressed as:

- `execution_target=node`

It is not a separate sandbox type.

## Sandbox vs MCP

MCP is also not the same thing as sandbox.

- MCP is a tool integration protocol and subprocess or server boundary
- sandbox is the execution policy and environment boundary around capability execution

In taxonomy terms:

- MCP tool -> `route_kind=tool`, `capability_source_kind=mcp`, `execution_target=mcp_server`
- sandbox failures -> `failure_origin=sandbox`

## Sandbox in Telegram Lanes

Telegram is currently the strongest real external interactive GUI acceptance lane.
TUI is the primary local chat-first operator surface for sandbox status and inline failure diagnosis.

When a Telegram-exposed skill, tool, or attachment processor depends on sandbox readiness:

- update [telegram-step-by-step.md](./telegram-step-by-step.md)
- update [telegram-real-e2e.md](./telegram-real-e2e.md)
- update the matching Telegram examples

When the local operator sandbox flow changes, also update:

- [tui.md](./tui.md)
- [testing.md](./testing.md)
- [release.md](./release.md)

The Telegram operator path should explicitly call out:

- `mosaic sandbox status`
- `mosaic sandbox list`
- any required env identity or rebuild step

The local TUI path should expose the same lifecycle with:

- `/sandbox status`
- `/sandbox inspect <env>`
- `/sandbox rebuild <env>`
- `/sandbox clean`

## Diagnosing Failures

When a capability run fails, the operator should be able to answer:

- was the failure caused by provider, tool, MCP, node, or sandbox?
- which sandbox env was selected?
- was the env created successfully?
- was the capability blocked by policy or by env setup?

Use:

- `mosaic inspect .mosaic/runs/<run-id>.json --verbose`
- `mosaic gateway incident <run-id>`
- `mosaic sandbox inspect <env-id>`

Look for:

- `failure_origin=sandbox`
- `sandbox_scope`
- `sandbox_run`
- tool or skill sandbox metadata

## Limitations Today

- env creation is workspace-local, not cluster or distributed
- install policy is explicit but still intentionally conservative
- richer long-term drift detection, lockfile ownership, and distributed env reuse can still deepen

## Quick Start

1. Start with [examples/sandbox/README.md](../examples/sandbox/README.md).
2. Define a sandbox binding on a tool or skill.
3. Run `mosaic setup validate`.
4. Create or inspect the env with `mosaic sandbox list` and `mosaic sandbox inspect <env-id>`.
5. Confirm the same env identity appears in `mosaic inspect --verbose`.
