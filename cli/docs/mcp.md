# MCP Servers (CLI)

`mosaic mcp` provides a local control plane for MCP server registry and readiness checks.

## Commands

```bash
# list configured MCP servers
mosaic --project-state mcp list

# add a server (repeat --arg and --env as needed)
mosaic --project-state mcp add \
  --name local-mcp \
  --command /absolute/path/to/server \
  --arg --stdio \
  --env MCP_TOKEN=example \
  --cwd /absolute/path/to/project

# inspect one configured server
mosaic --project-state mcp show <server_id>

# run readiness check for one server (does not start it)
mosaic --project-state mcp check <server_id>

# run readiness checks for all servers
mosaic --project-state mcp check --all
# (same as omitting server id)
mosaic --project-state mcp check

# toggle availability in policy/runtime
mosaic --project-state mcp disable <server_id>
mosaic --project-state mcp enable <server_id>

# remove server from registry
mosaic --project-state mcp remove <server_id>
```

## Storage

- Registry file: `.mosaic/data/mcp-servers.json` (or XDG data dir in default mode)
- Each server tracks:
  - `id`, `name`, `command`, `args`, `env`, `cwd`, `enabled`
  - `created_at`, `updated_at`, `last_check_at`, `last_check_error`

## Check Semantics

`mcp check` validates:

1. Server is enabled
2. `command` is resolvable/executable
3. `cwd` exists and is a directory (when provided)

`mcp check <server_id>` returns one check result.

`mcp check --all` (or `mcp check`) returns batch summary:

- `checked`
- `healthy`
- `unhealthy`
- `results[]` (`server` + `check`)

Both forms return operation success on valid input and expose health details in payload/text output.

## Error Contract

- Missing server id: `validation` (`exit_code=7`)
- Invalid `--env` format (`KEY=VALUE` required): `validation` (`exit_code=7`)

## Regression Tests

- `cargo test -p mosaic-mcp`
- `cargo test -p mosaic-cli --test mcp_ops`
- `cargo test -p mosaic-cli --test command_surface`
- `cargo test -p mosaic-cli --test error_codes`
- `cargo test -p mosaic-cli --test json_contract_modules`
