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

# run deep batch checks (includes protocol probe)
mosaic --project-state mcp check --all --deep --timeout-ms 2000
mosaic --project-state mcp check --all --deep --timeout-ms 2000 --report-out .mosaic/reports/mcp-check-deep.json

# run deep protocol diagnosis (stdio initialize probe)
mosaic --project-state mcp diagnose <server_id> --timeout-ms 2000
mosaic --project-state mcp diagnose <server_id> --timeout-ms 2000 --report-out .mosaic/reports/mcp-diagnose.json

# auto-remediation workflow (enable disabled servers; optional missing-cwd cleanup)
mosaic --project-state mcp repair <server_id> --timeout-ms 2000
mosaic --project-state mcp repair --all --timeout-ms 2000 --clear-missing-cwd --report-out .mosaic/reports/mcp-repair.json

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

`mcp diagnose` runs the same prechecks, then performs a best-effort stdio initialize probe:

1. starts the configured server command with args/env/cwd
2. writes an MCP `initialize` request over stdio
3. waits for initialize response until `--timeout-ms`
4. returns protocol probe details (`attempted`, `handshake_ok`, `response_kind`, `stderr_preview`, `error`) plus actionable recommendations

`mcp check <server_id>` returns one check result.

`mcp check --all` (or `mcp check`) returns batch summary:

- `checked`
- `healthy`
- `unhealthy`
- `results[]` (`server` + `check`)

`mcp check --all --deep` additionally runs protocol initialize probes in parallel and returns:

- `protocol_ok`
- `protocol_failed`
- `probe_skipped`
- `precheck_unhealthy`
- `results[]` (`server` + `check` + `protocol_probe` + merged `healthy`)

`--report-out <path>` is supported for both `check` and `diagnose` and writes the full JSON payload.

`mcp repair` executes a controlled remediation pass:

1. runs diagnose on target server(s)
2. auto-enables servers that fail only due to `disabled`
3. optionally clears invalid `cwd` (`--clear-missing-cwd`)
4. re-runs diagnose and returns before/after deltas + remaining recommendations

`--report-out <path>` is also supported for `repair`.

Both forms return operation success on valid input and expose health details in payload/text output.

## Error Contract

- Missing server id: `validation` (`exit_code=7`)
- Invalid `--env` format (`KEY=VALUE` required): `validation` (`exit_code=7`)
- Invalid `mcp diagnose --timeout-ms` (`0` or too large): `validation` (`exit_code=7`)
- Invalid `mcp check --deep --timeout-ms` (`0` or too large): `validation` (`exit_code=7`)
- Invalid `mcp repair` target (missing `<server_id>` and no `--all`): `validation` (`exit_code=7`)
- Invalid `mcp repair --timeout-ms` (`0` or too large): `validation` (`exit_code=7`)

## Regression Tests

- `cargo test -p mosaic-mcp`
- `cargo test -p mosaic-cli --test mcp_ops`
- `cargo test -p mosaic-cli --test command_surface`
- `cargo test -p mosaic-cli --test error_codes`
- `cargo test -p mosaic-cli --test json_contract_modules`
