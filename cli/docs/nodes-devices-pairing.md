# Nodes / Devices / Pairing (CLI)

This document covers the local control-plane commands for node execution, device lifecycle, and pairing approvals.

## Commands

```bash
mosaic --project-state nodes list
mosaic --project-state nodes status [node-id]
mosaic --project-state nodes diagnose [node-id] [--stale-after-minutes <minutes>] [--repair]
mosaic --project-state --json nodes diagnose [node-id] [--stale-after-minutes <minutes>] [--repair] [--report-out .mosaic/reports/nodes-diagnose.json]
# run/invoke go through gateway, start it first:
mosaic --project-state gateway start
mosaic --project-state --yes nodes run <node-id> --command "<shell-like command>"
mosaic --project-state nodes invoke <node-id> <method> [--params '<json>']

mosaic --project-state devices list
mosaic --project-state devices approve <device-id> [--name <name>]
mosaic --project-state devices reject <device-id> [--reason <text>]
mosaic --project-state devices rotate <device-id>
mosaic --project-state devices revoke <device-id> [--reason <text>]

mosaic --project-state pairing list [--status pending|approved|rejected]
mosaic --project-state pairing request --device <device-id> [--node <node-id>] [--reason <text>]
mosaic --project-state pairing approve <request-id>
mosaic --project-state pairing reject <request-id> [--reason <text>]
```

## Storage

- Nodes: `.mosaic/data/nodes.json`
- Devices: `.mosaic/data/devices.json`
- Pairing requests: `.mosaic/data/pairing-requests.json`
- Telemetry/events: `.mosaic/data/nodes-events.jsonl`

## Notes

- `nodes run` and `nodes invoke` are dispatched via gateway (`nodes.run` / `nodes.invoke` methods).
- `nodes diagnose` checks local control-plane consistency and reports:
  - stale online node heartbeat (`stale_online_node`)
  - approved pairing / device status drift (`approved_pairing_device_mismatch`)
  - pending pairing blocked by rejected/revoked device (`pending_pairing_blocked_device`)
  - orphan node/device references in pairing records (`orphan_pairing_reference`)
- `nodes diagnose --repair` applies safe local remediation:
  - stale online node -> mark node offline
  - approved pairing drift -> set device to approved
  - blocked/orphan pending pairing -> auto reject with reason
- `nodes diagnose --report-out <path>` writes the full diagnosis payload to a JSON artifact for CI, regression, or incident review.
- `nodes run` follows approvals/sandbox policy; under default confirm mode, use `--yes` for non-interactive runs.
- Pairing approval automatically marks the associated device as approved.
- Pairing rejection marks the request as `rejected` and sets the device status to `rejected` when the device exists.
- `pairing request` is useful for local/dev workflow to seed approval requests before `pairing approve` or `pairing reject`.
- `nodes status <node-id> --json` reports pairing counters: `total`, `pending`, `approved`, `rejected`.
- `nodes run`, `nodes invoke`, `devices approve|reject|rotate|revoke`, and `pairing request|approve|reject` append normalized lifecycle events to `.mosaic/data/nodes-events.jsonl`.
- `observability report/export` now includes a `nodes` slice plus summary counters such as `nodes_total`, `devices_total`, `pairings_total`, `nodes_events_count`, and `nodes_failed_events`.
