# Configuration

Mosaic reads configuration from YAML and merges multiple sources into one runtime view.

## File locations

Workspace config:

```text
.mosaic/config.yaml
```

Optional user config:

```text
~/.config/mosaic/config.yaml
```

## Merge order

Later sources override earlier ones.

1. built-in defaults
2. user config
3. workspace config
4. environment overrides
5. CLI overrides

Current environment override support:

- `MOSAIC_ACTIVE_PROFILE`

## Minimal workspace config

```yaml
schema_version: 1
active_profile: openai
profiles:
  openai:
    type: openai
    model: gpt-5.4-mini
    base_url: https://api.openai.com/v1
    api_key_env: OPENAI_API_KEY
```

## Top-level fields

### `schema_version`

- default: `1`
- purpose: config schema compatibility marker

### `active_profile`

- default: `gpt-5.4-mini`
- purpose: selects the provider profile used by the runtime unless overridden by session, ingress, or CLI

### `profiles`

- default: built-in first-class provider profiles plus the explicit dev-only `mock` profile
- purpose: provider profile registry used by `run`, `tui`, `gateway`, and scheduling logic

Supported `type` values:

- `mock`
- `openai`
- `azure`
- `anthropic`
- `ollama`
- `openai-compatible`

`mock` remains available for dev/test paths, but product onboarding should prefer one of the real first-class provider profiles above.

Profile fields:

- `type`: provider implementation
- `model`: model or deployment name
- `base_url`: endpoint root when required or overridden
- `api_key_env`: environment variable containing the credential
- `transport.timeout_ms`: per-profile request timeout override
- `transport.max_retries`: per-profile provider retry override
- `transport.retry_backoff_ms`: per-profile retry backoff override
- `transport.custom_headers`: optional provider-specific custom headers
- `vendor.azure_api_version`: Azure API version override
- `vendor.anthropic_version`: Anthropic version header override
- `vendor.allow_custom_headers`: must be `true` before custom headers are accepted

### `provider_defaults`

```yaml
provider_defaults:
  timeout_ms: 45000
  max_retries: 2
  retry_backoff_ms: 250
```

This block carries workspace-level defaults for provider transport behavior.

### `runtime`

```yaml
runtime:
  max_provider_round_trips: 8
  max_workflow_provider_round_trips: 8
  continue_after_tool_error: false
```

This block controls runtime loop ceilings and whether tool failures may be fed back into the conversation and retried.

### `deployment`

```yaml
deployment:
  profile: local
  workspace_name: default
```

- `profile` valid values: `local`, `staging`, `production`
- `workspace_name` default: `default`

### `auth`

```yaml
auth:
  operator_token_env: MOSAIC_OPERATOR_TOKEN
  webchat_shared_secret_env: MOSAIC_WEBCHAT_SHARED_SECRET
  telegram_secret_token_env: MOSAIC_TELEGRAM_SECRET
```

All fields are optional in `local` mode. In `production`, `operator_token_env` is required by validation.

### `session_store`

```yaml
session_store:
  root_dir: .mosaic/sessions
```

### `inspect`

```yaml
inspect:
  runs_dir: .mosaic/runs
```

### `audit`

```yaml
audit:
  root_dir: .mosaic/audit
  retention_days: 14
  event_replay_window: 256
  redact_inputs: true
```

### `observability`

```yaml
observability:
  enable_metrics: true
  enable_readiness: true
  slow_consumer_lag_threshold: 32
```

### `extensions`

```yaml
extensions:
  manifests:
    - path: .mosaic/extensions/time-and-summary.yaml
      version_pin: 0.1.0
```

### `policies`

```yaml
policies:
  allow_exec: true
  allow_webhook: true
  allow_cron: true
  allow_mcp: true
  hot_reload_enabled: true
```

These flags gate high-privilege surfaces and extension behavior.

## Commands that show effective config

```bash
mosaic setup validate
mosaic setup doctor
mosaic config show
mosaic model list
```

For effective per-run policy values:

```bash
mosaic inspect .mosaic/runs/<run-id>.json --verbose
```

## Related docs

- [providers.md](./providers.md)
- [provider-runtime-policy-matrix.md](./provider-runtime-policy-matrix.md)
- [real-vs-mock-acceptance.md](./real-vs-mock-acceptance.md)
- [examples/providers/openai.yaml](../examples/providers/openai.yaml)
- [examples/full-stack/openai-webchat.config.yaml](../examples/full-stack/openai-webchat.config.yaml)
