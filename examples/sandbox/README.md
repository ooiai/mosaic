# Sandbox Examples

These examples focus on workspace-local execution environments and sandbox bindings.

Use them together with [docs/sandbox.md](../../docs/sandbox.md).

Current operator flow:

1. bind a tool or skill to a sandbox env
2. validate the workspace config
3. inspect sandbox lifecycle from CLI or TUI
4. rebuild the env if it drifts or the dependency fingerprint changes

## Python-Oriented Binding

- [python-markdown-skill-pack.yaml](./python-markdown-skill-pack.yaml): markdown skill pack bound to a Python sandbox env

## Node-Oriented Binding

- [node-manifest-skill.yaml](./node-manifest-skill.yaml): manifest skill bound to a Node sandbox env

## Inspection

After loading one of these examples, inspect the resulting env state:

```bash
mosaic sandbox list
mosaic sandbox inspect <env-id>
mosaic sandbox rebuild <env-id>
```

From the chat-first TUI, the equivalent local operator commands are:

- `/sandbox status`
- `/sandbox inspect <env-id>`
- `/sandbox rebuild <env-id>`
- `/sandbox clean`

Then confirm the same env identity in the run trace:

```bash
mosaic inspect .mosaic/runs/<run-id>.json --verbose
```
