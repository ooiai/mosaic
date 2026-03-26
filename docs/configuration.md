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
    type: openai-compatible
    model: gpt-5.4-mini
    base_url: https://api.openai.com/v1
    api_key_env: OPENAI_API_KEY
```

## Top-level fields

### `schema_version`

- default: `1`
- required: no
- purpose: config schema compatibility marker

### `active_profile`

- default: `mock`
- required: yes, in practice
- purpose: selects the provider profile used by the runtime unless overridden by session or CLI

### `profiles`

- default: built-in `gpt-5.4`, `gpt-5.4-mini`, and `mock`
- required: yes if you want a real provider
- purpose: provider profile registry used by `run`, `tui`, `gateway`, and scheduling logic

Profile fields:

- `type`: current valid values are `mock` and `openai-compatible`
- `model`: model name sent to the provider
- `base_url`: endpoint root for `openai-compatible`
- `api_key_env`: environment variable containing the credential

### `deployment`

```yaml
deployment:
  profile: local
  workspace_name: default
```

- `profile` default: `local`
- valid values today: `local`, `staging`, `production`
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

- default: `.mosaic/sessions`
- purpose: session and transcript persistence

### `inspect`

```yaml
inspect:
  runs_dir: .mosaic/runs
```

- default: `.mosaic/runs`
- purpose: saved trace output directory

### `audit`

```yaml
audit:
  root_dir: .mosaic/audit
  retention_days: 14
  event_replay_window: 256
  redact_inputs: true
```

- `root_dir` default: `.mosaic/audit`
- `retention_days` default: `14`
- `event_replay_window` default: `256`
- `redact_inputs` default: `true`

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
      enabled: true
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
mosaic model list
```

## Example files

- [examples/providers/openai.yaml](../examples/providers/openai.yaml)
- [examples/providers/ollama.yaml](../examples/providers/ollama.yaml)
- [examples/providers/anthropic.yaml](../examples/providers/anthropic.yaml)
