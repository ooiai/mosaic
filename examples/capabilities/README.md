# Capability Examples

These examples map directly to Mosaic capability taxonomy.

Use them together with [docs/capabilities.md](../../docs/capabilities.md).

## Builtin Tool

- [builtin-tool.yaml](./builtin-tool.yaml): exposes builtin tools through an extension manifest

## MCP Tool

- [../mcp-filesystem.yaml](../mcp-filesystem.yaml): real MCP registration example

## Node-Routed Capability

- [node-routed-tool.yaml](./node-routed-tool.yaml): example tool metadata that is sandbox-aware and ready for node-preferred routing

## Workflow

- [workflow.yaml](./workflow.yaml): workflow example that invokes a manifest skill step

## How To Read Them

- builtin tool remains `route_kind=tool`
- MCP tool remains `route_kind=tool` with `capability_source_kind=mcp`
- node routing changes `execution_target` to `node`
- workflow is `route_kind=workflow`

Inspect a real run to see these distinctions:

```bash
mosaic inspect .mosaic/runs/<run-id>.json --verbose
```

For the local operator lane, you can also verify the same distinctions from the chat-first TUI:

```text
/mosaic adapter status
/mosaic node list
/mosaic tool read_file README.md
/mosaic inspect last
```

The inline inspect card should show the capability proof summary for builtin tools, MCP tools, node-routed tools, skills, and workflows.
