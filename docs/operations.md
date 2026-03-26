# Operations

This guide covers the day-2 operating model for a long-lived Mosaic deployment.

## Core directories

Mosaic keeps its primary state under `.mosaic/`.

Important paths:

- `.mosaic/config.yaml`: workspace config
- `.mosaic/sessions/`: persisted session state and transcripts
- `.mosaic/runs/`: inspectable run traces
- `.mosaic/audit/`: audit events and incident bundles
- `.mosaic/extensions/`: extension manifests
- `.mosaic/memory/`: session memory snapshots
- `.mosaic/nodes/`: node registrations and affinity
- `.mosaic/cron/`: cron registrations

## Logging

Mosaic writes operator-facing results to stdout and internal logs to stderr.

Recommended production pattern:

- let `systemd` or your supervisor collect stderr
- use journal rotation or your platform log retention policy
- keep trace, audit, and session data on disk for operator workflows

If you need richer logs during diagnosis:

```bash
mosaic --log-level info gateway status
mosaic --log-level debug inspect .mosaic/runs/<run-id>.json
```

## Health and readiness

Local control plane checks:

```bash
mosaic gateway status
mosaic gateway audit --limit 20
mosaic gateway replay --limit 20
```

Remote attach checks:

```bash
mosaic gateway --attach http://127.0.0.1:8080 status
mosaic adapter status
```

## Backup

At minimum, back up:

- `.mosaic/config.yaml`
- `.mosaic/sessions/`
- `.mosaic/runs/`
- `.mosaic/audit/`
- `.mosaic/memory/`
- `.mosaic/nodes/`
- `.mosaic/cron/`
- your service env file, for example `/etc/mosaic/mosaic.env`

A simple filesystem backup is enough because Mosaic currently stores this state as local files.

## Restore

1. Stop the service.
2. Restore `.mosaic/` and the env file.
3. Start the service again.
4. Verify:

```bash
mosaic session list
mosaic gateway status
mosaic inspect .mosaic/runs/<run-id>.json
```

## Incident response

Export an incident bundle for any run:

```bash
mosaic gateway incident <run-id>
```

Useful follow-up commands:

```bash
mosaic gateway audit --limit 50
mosaic gateway replay --limit 50
mosaic session show <session-id>
mosaic inspect .mosaic/runs/<run-id>.json --verbose
```

## Release smoke

Before and after a deployment, run:

```bash
make smoke
```

For a full delivery gate, run:

```bash
make release-check
```

Continue with [upgrade.md](./upgrade.md) for version changes and rollback planning.
