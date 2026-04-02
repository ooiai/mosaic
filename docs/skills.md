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
- [examples/extensions/markdown-skill-pack.yaml](../examples/extensions/markdown-skill-pack.yaml)

## Skill and Sandbox

Skills do not bypass sandbox policy.

The relevant rules are:

- a skill may declare a sandbox binding
- runtime resolves that binding into a workspace-local sandbox env
- dependency and env state should live under `.mosaic/sandbox/`

This matters most for:

- markdown skill packs with helper scripts
- manifest skills that rely on external runtimes
- future specialized processors

The sandbox details live in [sandbox.md](./sandbox.md).

## Skills in Telegram Lanes

Telegram currently carries the strongest real interactive acceptance lane while TUI remains incomplete.

If a Telegram bot exposes explicit skill execution such as `/mosaic skill ...`, the following must stay aligned in the same change set:

- [telegram-step-by-step.md](./telegram-step-by-step.md)
- [telegram-real-e2e.md](./telegram-real-e2e.md)
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
- whether it used a sandbox env

Use:

- `mosaic inspect .mosaic/runs/<run-id>.json --verbose`
- `mosaic gateway incident <run-id>`

Expected trace fields include:

- `route_kind=skill`
- `capability_source_kind=native_skill|manifest_skill|markdown_skill_pack`
- `source_name`
- `source_path`
- `source_version`

## Limitations Today

- markdown skill packs are template-style in v1
- manifest skill execution is intentionally sequential
- native skill inventory is still small
- richer dependency-aware execution belongs to the sandbox environment model, not to the skill registry itself

## Quick Start

1. Start with [examples/skills/README.md](../examples/skills/README.md).
2. Register a manifest or markdown skill through config or an extension manifest.
3. Run `mosaic setup validate`.
4. Trigger the skill through CLI, channel command, or workflow.
5. Confirm provenance with `mosaic inspect --verbose`.
