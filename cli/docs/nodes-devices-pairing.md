# Nodes / Devices / Pairing (CLI)

This document covers the local control-plane commands for node execution, device lifecycle, and pairing approvals.

## Commands

```bash
mosaic --project-state nodes list
mosaic --project-state nodes status [node-id]
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
```

## Storage

- Nodes: `.mosaic/data/nodes.json`
- Devices: `.mosaic/data/devices.json`
- Pairing requests: `.mosaic/data/pairing-requests.json`

## Notes

- `nodes run` and `nodes invoke` are dispatched via gateway (`nodes.run` / `nodes.invoke` methods).
- `nodes run` follows approvals/sandbox policy; under default confirm mode, use `--yes` for non-interactive runs.
- Pairing approval automatically marks the associated device as approved.
- `pairing request` is useful for local/dev workflow to seed approval requests before `pairing approve`.
