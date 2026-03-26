# Security

This guide describes the default security posture and operational boundaries for Mosaic.

## Secret management

Do not place raw provider keys or operator tokens directly in `.mosaic/config.yaml`.

Use env var indirection instead:

- `profiles.<name>.api_key_env`
- `auth.operator_token_env`
- `auth.webchat_shared_secret_env`
- `auth.telegram_secret_token_env`

Start from [`.env.example`](../.env.example) and move the real values into a file that only the service user can read.

## Operator auth

Production deployments should set:

```yaml
auth:
  operator_token_env: MOSAIC_OPERATOR_TOKEN
```

This protects operator-facing HTTP control-plane commands.

## Channel ingress auth

Use shared secrets for channel adapters where supported:

```yaml
auth:
  webchat_shared_secret_env: MOSAIC_WEBCHAT_SHARED_SECRET
  telegram_secret_token_env: MOSAIC_TELEGRAM_SECRET_TOKEN
```

## Provider credentials

Provider profiles should reference the minimum required credential for that provider.

Examples:

- `OPENAI_API_KEY`
- `AZURE_OPENAI_API_KEY`
- `ANTHROPIC_API_KEY`

Rotate provider credentials outside Mosaic and restart the service if your process manager does not automatically refresh env files.

## Capability, node, and extension boundaries

High-risk actions should stay explicit.

Current control points include:

- capability risk metadata and permission scopes
- node capability declarations and affinity
- extension policy gates for `exec`, `webhook`, `cron`, `mcp`, and hot reload
- provider-side tool visibility filtering based on authorized capabilities

Recommended production defaults:

- disable `allow_exec` unless you truly need local command execution
- keep `hot_reload_enabled` off unless operators actively manage extension rollout
- review node registrations before attaching them to production sessions

## Audit and redaction

Keep `audit.redact_inputs: true` in production.

This protects operator inputs and ingress payloads in audit output while still preserving:

- run ids
- session ids
- correlation ids
- summarized failure context

Use these commands during review:

```bash
mosaic config show
mosaic gateway audit --limit 20
mosaic gateway incident <run-id>
```

## Upgrade discipline

Before upgrades:

- export an incident bundle for any active investigation
- back up `.mosaic/` and the env file
- review [compatibility.md](./compatibility.md)
- follow [upgrade.md](./upgrade.md)
