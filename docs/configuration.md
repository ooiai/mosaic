# Configuration

Mosaic reads configuration from YAML and merges multiple sources into one runtime view.

This guide focuses on the operator-visible config knobs, especially the Telegram bot registry and attachment routing added in k1-k4.

For the capability taxonomy that config ultimately feeds into, see [capabilities.md](./capabilities.md). In particular:

- config can change `capability_source_kind`
- config and policy can affect `route_kind`
- config can change `execution_target` indirectly through MCP registration, node routing, and workflow composition
- config failures should surface as `failure_origin=config` instead of being confused with runtime/provider failures

## File Locations

Workspace config:

```text
.mosaic/config.yaml
```

Optional user config:

```text
~/.config/mosaic/config.yaml
```

## Merge Order

Later sources override earlier ones.

1. built-in defaults
2. user config
3. workspace config
4. environment overrides
5. CLI overrides

Current environment override support:

- `MOSAIC_ACTIVE_PROFILE`

## Minimal Workspace Config

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

## Top-Level Fields

### `schema_version`

- default: `1`
- purpose: config schema compatibility marker

### `active_profile`

- default: `gpt-5.4-mini`
- purpose: selects the provider profile used by the runtime unless overridden by session, ingress, bot policy, or CLI

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
- `attachments.mode`: provider-level attachment default
- `attachments.multimodal_profile`: profile to switch to for provider-native attachments
- `attachments.specialized_processor_profile`: profile to switch to for specialized processor attachment work
- `attachments.allowed_attachment_kinds`: optional kind allowlist
- `attachments.max_attachment_size_mb`: optional per-profile size limit

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

### `sandbox`

`sandbox` defines the workspace-local execution environment baseline under `.mosaic/sandbox`.

Example:

```yaml
sandbox:
  base_dir: .mosaic/sandbox
  python:
    strategy: venv
  node:
    strategy: npm
  cleanup:
    run_workdirs_after_hours: 24
    attachments_after_hours: 24
```

Fields:

- `base_dir`: workspace-local sandbox root
- `python.strategy`: `venv`, `uv`, or `disabled`
- `node.strategy`: `npm`, `pnpm`, `layout_only`, or `disabled`
- `cleanup.run_workdirs_after_hours`: how long run workdirs under `.mosaic/sandbox/work/runs` are retained
- `cleanup.attachments_after_hours`: how long sandbox-managed attachment workdirs are retained

Capability bindings may also declare a sandbox env:

```yaml
tools:
  - type: builtin
    name: exec_command
    sandbox:
      kind: shell
      env_name: exec-command
      scope: capability
      dependency_spec:
        - sh
```

The runtime uses these bindings to resolve per-capability env identity while still allocating a per-run workdir for each run.

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
  telegram_secret_token_env: MOSAIC_TELEGRAM_SECRET_TOKEN
```

All fields are optional in `local` mode. In `production`, `operator_token_env` is required by validation.

For single-bot Telegram workspaces, `auth.telegram_secret_token_env` is usually enough. For multi-bot Telegram workspaces, each bot can override its own webhook secret env under `telegram.bots.<name>.webhook_secret_token_env`.

### `telegram`

`telegram.bots` is the bot registry for bot-aware workspaces.

Single-bot baseline:

```yaml
telegram:
  bots:
    primary:
      bot_token_env: MOSAIC_TELEGRAM_BOT_TOKEN
      webhook_secret_token_env: MOSAIC_TELEGRAM_SECRET_TOKEN
      route_key: primary
      webhook_path: /ingress/telegram/primary
      default_profile: openai
      allowed_tools:
        - read_file
      allowed_skills:
        - summarize_notes
      allowed_workflows:
        - summarize_operator_note
```

Multi-bot baseline:

```yaml
telegram:
  bots:
    ops:
      bot_token_env: MOSAIC_TELEGRAM_OPS_BOT_TOKEN
      webhook_secret_token_env: MOSAIC_TELEGRAM_OPS_SECRET_TOKEN
      route_key: ops
      webhook_path: /ingress/telegram/ops
      default_profile: openai
      allowed_tools:
        - read_file
    media:
      bot_token_env: MOSAIC_TELEGRAM_MEDIA_BOT_TOKEN
      webhook_secret_token_env: MOSAIC_TELEGRAM_MEDIA_SECRET_TOKEN
      route_key: media
      webhook_path: /ingress/telegram/media
      default_profile: openai-vision
      allowed_skills:
        - summarize_notes
      allowed_workflows:
        - summarize_operator_note
```

Per-bot fields:

- `bot_token_env`: env var that holds the Telegram Bot API token
- `webhook_secret_token_env`: env var used to validate inbound webhook requests for that bot
- `route_key`: short bot route used in `/ingress/telegram/<route>`
- `webhook_path`: optional explicit path; if omitted, it defaults from the route key
- `default_profile`: profile selected for that bot when the incoming message does not override it
- `allowed_tools`: explicit tool allowlist for that bot
- `allowed_skills`: explicit skill allowlist for that bot
- `allowed_workflows`: explicit workflow allowlist for that bot
- `attachments`: per-bot attachment route override

### `attachments`

This block has two parts:

1. policy: download and cache rules
2. routing: where attachments should go after normalization

Example:

```yaml
attachments:
  policy:
    enabled: true
    cache_dir: .mosaic/cache/attachments
    max_size_bytes: 26214400
    download_timeout_ms: 15000
    allowed_mime_types:
      - image/jpeg
      - image/png
      - application/pdf
    cleanup_after_hours: 24
  routing:
    default:
      mode: disabled
    channel_overrides:
      telegram:
        mode: provider_native
        multimodal_profile: openai-vision
        allowed_attachment_kinds:
          - image
          - document
        max_attachment_size_mb: 25
    bot_overrides:
      legacy-bot-route:
        mode: specialized_processor
        processor: summarize_notes
        specialized_processor_profile: openai
        allowed_attachment_kinds:
          - document
        max_attachment_size_mb: 10
```

Routing fields:

- `mode`: `provider_native`, `specialized_processor`, or `disabled`
- `processor`: named processor to use when `mode: specialized_processor`
- `multimodal_profile`: profile to select when a provider-native multimodal run is required
- `specialized_processor_profile`: profile to select when running the specialized processor
- `allowed_attachment_kinds`: attachment kind allowlist using `image`, `document`, `audio`, `video`, `other`
- `max_attachment_size_mb`: optional route-specific size limit

Preferred new pattern:

- use `telegram.bots.<name>.attachments` for new per-bot policy
- keep `attachments.routing.channel_overrides` for shared channel-wide defaults
- treat `attachments.routing.bot_overrides` as a compatibility layer when you must key by route name

### `tools`, `skills`, `workflows`

These arrays override or register capability exposure from workspace config.

Relevant channel fields:

- `visibility`
- `invocation_mode`
- `allowed_channels`
- `required_policy`
- `accepts_attachments`

That means a capability can be:

- visible in TUI but hidden from Telegram
- explicit-only in `/mosaic`
- attachment-aware only when the manifest or workspace config enables it

### `extensions`

```yaml
extensions:
  manifests:
    - path: .mosaic/extensions/telegram-e2e.yaml
      version_pin: 0.1.0
```

This is the normal way to add manifest skills and workflows such as the attachment-aware Telegram examples.

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

## Commands That Show Effective Config

```bash
mosaic setup validate
mosaic setup doctor
mosaic config show
mosaic config sources
mosaic model list
mosaic adapter status
```

For effective per-run policy values:

```bash
mosaic inspect .mosaic/runs/<run-id>.json --verbose
```

Look for:

- `effective_profile`
- provider multimodal capability summary
- attachment route and selected profile
- bot identity and policy scope

## Example Files

- [examples/full-stack/openai-telegram-single-bot.config.yaml](../examples/full-stack/openai-telegram-single-bot.config.yaml)
- [examples/full-stack/openai-telegram-multi-bot.config.yaml](../examples/full-stack/openai-telegram-multi-bot.config.yaml)
- [examples/full-stack/openai-telegram-multimodal.config.yaml](../examples/full-stack/openai-telegram-multimodal.config.yaml)
- [examples/full-stack/openai-telegram-bot-split.config.yaml](../examples/full-stack/openai-telegram-bot-split.config.yaml)
- [examples/channels/telegram-photo-update.json](../examples/channels/telegram-photo-update.json)
- [examples/channels/telegram-document-update.json](../examples/channels/telegram-document-update.json)
- [examples/extensions/telegram-e2e.yaml](../examples/extensions/telegram-e2e.yaml)

## Related Docs

- [providers.md](./providers.md)
- [channels.md](./channels.md)
- [telegram-step-by-step.md](./telegram-step-by-step.md)
- [telegram-real-e2e.md](./telegram-real-e2e.md)
- [provider-runtime-policy-matrix.md](./provider-runtime-policy-matrix.md)
- [real-vs-mock-acceptance.md](./real-vs-mock-acceptance.md)
