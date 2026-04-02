# Skills

Mosaic skills are reusable capability units that live above tools and below workflows.

This document is the operator-facing source of truth for the skill system after `plan_l1` and `plan_l5`.

See also:

- [capabilities.md](./capabilities.md)
- [sandbox.md](./sandbox.md)
- [configuration.md](./configuration.md)
- [examples/skills/README.md](../examples/skills/README.md)

## Skill Types

Mosaic currently supports three skill sources.

### Native Skill

Native skills are implemented in Rust and registered directly by code.

Examples:

- builtin `summarize`

Properties:

- best for first-party stable behaviors
- versioned with the workspace build
- no external pack layout required

### Manifest Skill

Manifest skills are declarative skills described in YAML.

They define:

- metadata
- optional tool dependencies
- one or more sequential steps

Properties:

- easy to ship in extension manifests
- good for small reusable flows
- conservative execution model

### Markdown Skill Pack

Markdown skill packs are directory-based skill packages centered on `SKILL.md`.

They may include:

- `SKILL.md`
- frontmatter metadata
- `templates/`
- `references/`
- `scripts/`

Properties:

- best for reusable prompt- and template-oriented skill packs
- easier to author outside Rust code
- intended to feel closer to external skill ecosystems
- pack-relative and sandbox-aware by design

## Skill vs Workflow

This distinction is important:

- a `skill` is one reusable capability unit
- a `workflow` is a multi-step orchestration unit

A workflow may call skills, tools, and prompt steps.

A skill is not:

- an MCP adapter
- a node route
- a channel command
- a workflow graph

In capability taxonomy terms:

- skill -> `route_kind=skill`
- workflow -> `route_kind=workflow`

## Skill vs Tool

Tools are execution primitives.

Skills are reusable capability units that may use tools internally or may only structure prompt behavior.

Typical differences:

- tool: direct side-effect or retrieval action
- skill: reusable problem-solving unit

Examples:

- `read_file` is a tool
- `summarize_notes` is a skill

## How Skills Enter Mosaic

Skills may be introduced through:

- builtin registration
- workspace config
- extension manifests
- markdown skill pack references

The operator-facing composition path is:

1. define or reference the skill
2. validate with `mosaic setup validate` or `mosaic extension validate`
3. expose it through config, extension policy, channel policy, or bot visibility
4. invoke it through runtime, workflow, or channel command

## Config and Extension Wiring

Workspace or extension examples:

```yaml
skills:
  - type: builtin
    name: summarize
```

```yaml
skills:
  - type: manifest
    name: summarize_notes
    steps:
      - kind: echo
        name: draft
        input: "{{input}}"
```

```yaml
skills:
  - type: markdown_pack
    name: operator_note
    path: ./examples/skills/operator-note
```

Related examples:

- [examples/skills/native-skill.yaml](../examples/skills/native-skill.yaml)
- [examples/skills/manifest-skill.yaml](../examples/skills/manifest-skill.yaml)
- [examples/skills/operator-note/SKILL.md](../examples/skills/operator-note/SKILL.md)
- [examples/skills/operator-note/templates/note.md](../examples/skills/operator-note/templates/note.md)
- [examples/skills/operator-note/references/escalation.md](../examples/skills/operator-note/references/escalation.md)
- [examples/skills/operator-note/scripts/annotate.py](../examples/skills/operator-note/scripts/annotate.py)
- [examples/extensions/markdown-skill-pack.yaml](../examples/extensions/markdown-skill-pack.yaml)

The `operator_note` pack now demonstrates the full markdown-pack execution path:

- `SKILL.md` frontmatter
- `templates/`
- `references/`
- `scripts/`
- attachment-aware rendering
- sandbox-backed helper script execution

## Skill and Sandbox

Skills do not bypass sandbox policy.

The relevant rules are:

- a skill may declare a sandbox binding
- runtime resolves that binding into a workspace-local sandbox env
- dependency and env state should live under `.mosaic/sandbox/`
- helper scripts execute inside that selected sandbox env, not in the global Python or Node environment

This matters most for:

- markdown skill packs with helper scripts
- manifest skills that rely on external runtimes
- future specialized processors

The sandbox details live in [sandbox.md](./sandbox.md).

Local operators should now be able to verify skill env readiness from both CLI and TUI:

- `mosaic sandbox status`
- `/sandbox status`
- `/sandbox inspect <env>`

## Skills in Telegram Lanes

Telegram currently carries the strongest real external interactive acceptance lane.
TUI is the primary local chat-first operator surface for skill discovery and local execution proof.

If a Telegram bot exposes explicit skill execution such as `/mosaic skill ...`, the following must stay aligned in the same change set:

- [telegram-step-by-step.md](./telegram-step-by-step.md)
- [telegram-real-e2e.md](./telegram-real-e2e.md)
- [tui.md](./tui.md) when local skill discovery or inline execution visibility changes
- Telegram examples and extension manifests

This applies to:

- manifest skills
- markdown skill packs
- attachment-aware skills

If a Telegram-visible skill depends on helper scripts or a dedicated env, the Telegram docs should also mention the sandbox checks from [sandbox.md](./sandbox.md).

## How Operators See Skill Provenance

Operators should be able to answer:

- which skill ran
- where it came from
- whether it was native, manifest, or markdown
- whether it used templates, references, scripts, and a sandbox env

Use:

- `mosaic inspect .mosaic/runs/<run-id>.json --verbose`
- `mosaic gateway incident <run-id>`

Expected trace fields include:

- `route_kind=skill`
- `capability_source_kind=native_skill|manifest_skill|markdown_skill_pack`
- `source_name`
- `source_path`
- `source_version`
- markdown-pack usage such as `template`, `references`, `script`, `script_runtime`, and attachment summary

## Limitations Today

- markdown skill packs now support templates, references, and helper scripts, but remain intentionally pack-relative and sandbox-scoped
- manifest skill execution is intentionally sequential
- native skill inventory is still small
- package dependency resolution still belongs to the sandbox environment model, not to the skill registry itself

## Quick Start

1. Start with [examples/skills/README.md](../examples/skills/README.md).
2. Register a manifest or markdown skill through config or an extension manifest.
3. Run `mosaic setup validate`.
4. In the TUI, type `/skill op<Tab>` to complete the markdown pack name, or invoke it through CLI, channel command, or workflow.
5. Confirm provenance with `mosaic inspect --verbose`.
