# Session, Inspect, and Incident Flow

This guide explains how to verify that one ingress message became durable operator state.

Use it after:

- `mosaic run ...`
- `mosaic tui`
- Gateway HTTP ingress such as webchat or Telegram
- the full-stack example in [full-stack.md](./full-stack.md)

Related examples:

- [examples/full-stack/README.md](../examples/full-stack/README.md)
- [examples/channels/telegram-update.json](../examples/channels/telegram-update.json)

For the live Telegram bot runbook, continue with [telegram-real-e2e.md](./telegram-real-e2e.md).

## 1. Confirm the session

List sessions first:

```bash
mosaic session list
```

Show the session created by the example Telegram payload:

```bash
mosaic session show telegram--100123-99
```

Look for:

- `session_route`
- `channel: Some("telegram")`
- `thread_id`
- transcript entries
- `last_gateway_run_id`
- `memory_summary`

## 2. Confirm the saved trace

Inspect the latest trace:

```bash
mosaic inspect .mosaic/runs/<run-id>.json
```

Verbose inspection:

```bash
mosaic inspect .mosaic/runs/<run-id>.json --verbose
```

Look for:

- `effective_profile`
- `ingress`
- `tool_calls`
- `capability_invocations`
- `governance`
- `provider_attempts`

## 3. Confirm audit and replay

If the Gateway is running over HTTP:

```bash
mosaic gateway --attach http://127.0.0.1:18080 audit --limit 20
mosaic gateway --attach http://127.0.0.1:18080 replay --limit 20
```

Otherwise use the local workspace view:

```bash
mosaic gateway audit --limit 20
mosaic gateway replay --limit 20
```

## 4. Export one incident bundle

```bash
mosaic gateway incident <run-id>
```

Or against the HTTP Gateway:

```bash
mosaic gateway --attach http://127.0.0.1:18080 incident <run-id>
```

The default save path is:

```text
.mosaic/audit/incidents/<run-id>.json
```

## 5. Compare the facts

For one run, these facts should agree:

- the session transcript
- the trace `run_id`, `gateway_run_id`, and ingress metadata
- the Gateway audit log
- the incident bundle

If they do not agree, continue with [troubleshooting.md](./troubleshooting.md).
