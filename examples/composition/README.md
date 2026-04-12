# Capability Composition Examples

These examples show how builtin capabilities, extension manifests, markdown skill packs, and MCP registrations combine into one workspace.

Use them with:

- [docs/configuration.md](../../docs/configuration.md)
- [docs/capabilities.md](../../docs/capabilities.md)
- [docs/skills.md](../../docs/skills.md)
- [docs/sandbox.md](../../docs/sandbox.md)

## Combined Workspace Example

- [openai-capability-composition.config.yaml](./openai-capability-composition.config.yaml): one workspace config that combines provider profiles, extension manifests, markdown skill packs, MCP registrations, and sandbox defaults

Validate it with:

```bash
mosaic setup validate
mosaic config show
mosaic extension validate
```
