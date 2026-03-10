# MCP Servers (CLI)

`mosaic mcp` provides a local control plane for MCP server registry and readiness checks.

## Commands

```bash
# list configured MCP servers
mosaic --project-state mcp list

# add a server (repeat --arg, --env, and --env-from as needed)
mosaic --project-state mcp add \
  --name local-mcp \
  --command /absolute/path/to/server \
  --arg --stdio \
  --env MCP_MODE=local \
  --env-from OPENAI_API_KEY=AZURE_OPENAI_API_KEY \
  --cwd /absolute/path/to/project

# inspect one configured server
mosaic --project-state mcp show <server_id>

# update one configured server (explicit config mutation path)
mosaic --project-state mcp update <server_id> --name docs-mcp
mosaic --project-state mcp update <server_id> --env-from OPENAI_API_KEY=AZURE_OPENAI_API_KEY --clear-cwd
mosaic --project-state mcp update <server_id> --clear-args --disable

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
mosaic --project-state mcp repair <server_id> --timeout-ms 2000 --set-env-from OPENAI_API_KEY=AZURE_OPENAI_API_KEY
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
  - `id`, `name`, `command`, `args`, `env`, `env_from`, `cwd`, `enabled`
  - `created_at`, `updated_at`, `last_check_at`, `last_check_error`
- `mcp add --env` is for non-sensitive runtime pairs only.
- `mcp add --env-from KEY=ENV_NAME` persists environment variable indirection without storing the secret value itself.
- Secret literals like `OPENAI_API_KEY=...` are rejected and must be supplied by the process environment that launches Mosaic instead of being persisted in the registry.
- `--env` and `--env-from` cannot configure the same key.

## Secret-Safe Runtime Env

Use `--env` for non-sensitive flags and `--env-from` for secrets or operator-managed runtime env:

```bash
export AZURE_OPENAI_API_KEY=...

mosaic --project-state mcp add \
  --name docs-mcp \
  --command /absolute/path/to/server \
  --arg --stdio \
  --env MCP_MODE=local \
  --env-from OPENAI_API_KEY=AZURE_OPENAI_API_KEY
```

The registry stores `OPENAI_API_KEY -> AZURE_OPENAI_API_KEY`, then resolves the real value only when Mosaic starts the MCP process or runs protocol diagnostics.

## Update vs Repair

Use `mcp update` for explicit operator-owned config changes:

- rename a server
- replace `args`
- replace `env`
- replace `env_from`
- replace/clear `cwd`
- toggle enabled state

`mcp update` uses replacement semantics for collection fields:

- `--arg` replaces the full `args` list
- `--env` replaces the full `env` map
- `--env-from` replaces the full `env_from` map
- `--clear-args`, `--clear-env`, `--clear-env-from`, and `--clear-cwd` explicitly remove those fields

`mcp repair` is different. It is diagnose-driven remediation for a detected problem set (`disabled`, missing `cwd`, broken env reference), not the canonical path for routine config edits.

## Check Semantics

`mcp check` validates:

1. Server is enabled
2. `command` is resolvable/executable
3. `cwd` exists and is a directory (when provided)
4. every `env_from` source env exists in the Mosaic process environment

`mcp diagnose` runs the same prechecks, then performs a best-effort stdio initialize probe:

1. resolves `env` + `env_from` into the child process environment
2. starts the configured server command with args/env/cwd
3. writes an MCP `initialize` request over stdio
4. waits for initialize response until `--timeout-ms`
5. returns protocol probe details (`attempted`, `handshake_ok`, `response_kind`, `stderr_preview`, `error`) plus actionable recommendations

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
4. optionally rewires env references (`--set-env-from KEY=ENV_NAME`)
5. re-runs diagnose and returns before/after deltas + remaining recommendations

`--report-out <path>` is also supported for `repair`.

Both forms return operation success on valid input and expose health details in payload/text output.

`mosaic doctor` also emits `mcp_env_refs`, which summarizes configured MCP env indirections and warns when one or more source env vars are missing.

## Error Contract

- Missing server id: `validation` (`exit_code=7`)
- Invalid `--env` format (`KEY=VALUE` required): `validation` (`exit_code=7`)
- Invalid `--env-from` format (`KEY=ENV_NAME` required): `validation` (`exit_code=7`)
- Secret-like literal in `--env`: `validation` (`exit_code=7`)
- Invalid `--env-from` env token: `validation` (`exit_code=7`)
- Invalid `mcp update` with no field change: `validation` (`exit_code=7`)
- Invalid `mcp update` conflicting clear/replace flags: `validation` (`exit_code=7`)
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
