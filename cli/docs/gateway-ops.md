# Gateway Ops (CLI)

This document covers the V2 gateway operations commands.

## Commands

```bash
mosaic --project-state gateway install --host 127.0.0.1 --port 8787
mosaic --project-state gateway start
mosaic --project-state gateway restart --port 8788
mosaic --project-state gateway status --deep
mosaic --project-state gateway health --verbose
mosaic --project-state gateway probe
mosaic --project-state gateway discover
mosaic --project-state gateway call <method> --params '<json>'
mosaic --project-state gateway stop
mosaic --project-state gateway uninstall
```

Notes:

- `gateway start` keeps backward compatibility with `gateway run` alias.
- `gateway install` stores desired host/port in `.mosaic/data/gateway-service.json`.
- `gateway status --deep` includes process/endpoint diagnostics and resolved target.
- `gateway health --verbose` includes protocol checks:
  - `gateway_discover`
  - `gateway_protocol_methods` (required `health,status`)
  - `gateway_call_status`

## `gateway call`

- Request shape:
  - `GatewayRequest { id, method, params }`
- Response shape:
  - Success: `{ ok: true, data: ... }`
  - Failure: `{ ok: false, error: { code, message, exit_code } }`

Examples:

```bash
mosaic --project-state --json gateway call status
mosaic --project-state --json gateway call echo --params '{"text":"hello"}'
```

## `gateway discover`

Returns methods exposed by current gateway runtime.

```json
{
  "ok": true,
  "discovery": {
    "ok": true,
    "endpoint": "http://127.0.0.1:8787/discover",
    "methods": ["health", "status", "echo"]
  }
}
```

## Troubleshooting

- `gateway_unavailable`:
  - Gateway process is down or endpoint is unreachable.
- `gateway_protocol`:
  - Gateway returned invalid JSON or incompatible response structure.
