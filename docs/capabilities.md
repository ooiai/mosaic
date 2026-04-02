# Capability Taxonomy

This document is the source of truth for Mosaic capability taxonomy after `plan_l3`.

The goal is simple:

- one route vocabulary
- one source vocabulary
- one execution-target vocabulary
- one failure-origin vocabulary

That vocabulary is used by runtime traces, `mosaic inspect --verbose`, gateway run detail DTOs, incident bundles, and operator docs.

See also:

- [skills.md](./skills.md)
- [sandbox.md](./sandbox.md)
- [configuration.md](./configuration.md)

## Route Kind

`route_kind` describes the top-level route selected by runtime.

Allowed values:

- `assistant`
- `tool`
- `skill`
- `workflow`

Notes:

- `control` is still a gateway/operator command path, but it is not part of the capability taxonomy itself.
- `node` and `mcp` are not route kinds.

## Capability Source Kind

`capability_source_kind` describes where the selected capability came from.

Allowed values:

- `builtin`
- `workspace_config`
- `extension`
- `mcp`
- `native_skill`
- `manifest_skill`
- `markdown_skill_pack`

Interpretation:

- builtin tool: `route_kind=tool`, `capability_source_kind=builtin`
- MCP tool: `route_kind=tool`, `capability_source_kind=mcp`
- native skill: `route_kind=skill`, `capability_source_kind=native_skill`
- markdown skill pack: `route_kind=skill`, `capability_source_kind=markdown_skill_pack`
- extension workflow: `route_kind=workflow`, `capability_source_kind=extension`

## Execution Target

`execution_target` describes where the selected capability actually runs.

Allowed values:

- `local`
- `mcp_server`
- `node`
- `provider`
- `workflow_engine`

Interpretation:

- builtin local tool: `execution_target=local`
- MCP tool: `execution_target=mcp_server`
- node-routed tool: `execution_target=node`
- assistant response generation: `execution_target=provider`
- workflow route: `execution_target=workflow_engine`

Important boundary:

- node is an execution target, not a capability type
- MCP is a tool source and MCP server target, not a workflow/skill substitute

## Orchestration Owner

`orchestration_owner` describes which layer is coordinating the action.

Current values:

- `runtime`
- `workflow_engine`
- `gateway`

Examples:

- direct assistant/tool/skill runs are usually owned by `runtime`
- workflow steps are owned by `workflow_engine`
- gateway control commands are owned by `gateway`

## Failure Origin

`failure_origin` explains where a failed run or failed capability actually broke.

Allowed values:

- `provider`
- `runtime`
- `tool`
- `mcp`
- `node`
- `skill`
- `workflow`
- `sandbox`
- `config`
- `gateway`

Examples:

- provider auth/rate/transport failure: `failure_origin=provider`
- MCP subprocess/tool failure: `failure_origin=mcp`
- node affinity/no-eligible-node/permission failure: `failure_origin=node`
- sandbox env creation failure: `failure_origin=sandbox`
- invalid visibility/profile/policy/config mismatch: `failure_origin=config`

## Five Core Distinctions

### Builtin Tool

- `route_kind=tool`
- `capability_source_kind=builtin` or `workspace_config` or `extension`
- `execution_target=local` or `node`

### MCP Tool

- `route_kind=tool`
- `capability_source_kind=mcp`
- `execution_target=mcp_server`

### Node-Routed Capability

- still `route_kind=tool`
- not a separate capability type
- represented by `execution_target=node`

### Skill

- `route_kind=skill`
- `capability_source_kind=native_skill`, `manifest_skill`, or `markdown_skill_pack`
- usually `execution_target=local`

### Workflow

- `route_kind=workflow`
- `execution_target=workflow_engine`
- may internally invoke provider/tool/skill steps

## Where Operators See This

The taxonomy is surfaced in:

- `mosaic inspect --verbose`
- gateway run detail and incident bundles
- runtime capability traces
- tool traces and skill traces
- workflow step traces

When debugging a run, the minimum interpretation path is:

1. `route_kind`
2. `capability_source_kind`
3. `execution_target`
4. `orchestration_owner`
5. `failure_origin` if the run failed

## Related Examples

- [examples/capabilities/README.md](../examples/capabilities/README.md)
- [examples/capabilities/builtin-tool.yaml](../examples/capabilities/builtin-tool.yaml)
- [examples/capabilities/node-routed-tool.yaml](../examples/capabilities/node-routed-tool.yaml)
- [examples/capabilities/workflow.yaml](../examples/capabilities/workflow.yaml)
- [examples/mcp-filesystem.yaml](../examples/mcp-filesystem.yaml)

## What This Does Not Mean

- MCP is not "another workflow system"
- node is not "another capability kind"
- skill is not "a raw prompt snippet"
- workflow is not "just a tool with multiple steps"

These distinctions are enforced in code, traces, and operator docs to avoid drift between runtime behavior and operator understanding.
