# Sandbox Examples

These examples focus on workspace-local execution environments and sandbox bindings.

Use them together with [docs/sandbox.md](../../docs/sandbox.md).

## Python-Oriented Binding

- [python-markdown-skill-pack.yaml](./python-markdown-skill-pack.yaml): markdown skill pack bound to a Python sandbox env

## Node-Oriented Binding

- [node-manifest-skill.yaml](./node-manifest-skill.yaml): manifest skill bound to a Node sandbox env

## Inspection

After loading one of these examples, inspect the resulting env state:

```bash
mosaic sandbox list
mosaic sandbox inspect <env-id>
```

Then confirm the same env identity in the run trace:

```bash
mosaic inspect .mosaic/runs/<run-id>.json --verbose
```
