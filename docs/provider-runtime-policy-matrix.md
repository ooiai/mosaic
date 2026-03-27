# Provider and Runtime Policy Matrix

These are the operator-visible knobs that affect real request behavior in Mosaic.

The rule for i2 is simple:

- vendor protocol details stay inside `mosaic-provider`
- request stability, retry, compatibility, and loop ceilings must be visible in config and trace output

## Provider Transport Policy

| Config key | Scope | Default | Applies to | Visible in |
| --- | --- | --- | --- | --- |
| `provider_defaults.timeout_ms` | workspace default | provider-type specific | all profiles without a profile override | `mosaic config show`, `mosaic inspect --verbose` |
| `profiles.<name>.transport.timeout_ms` | profile override | unset | one provider profile | config show, inspect effective profile |
| `provider_defaults.max_retries` | workspace default | provider-type specific | all profiles without a profile override | config show, inspect effective profile |
| `profiles.<name>.transport.max_retries` | profile override | unset | one provider profile | config show, inspect effective profile |
| `provider_defaults.retry_backoff_ms` | workspace default | `150` ms for networked providers, `0` for `mock` | all profiles without a profile override | config show, inspect effective profile |
| `profiles.<name>.transport.retry_backoff_ms` | profile override | unset | one provider profile | config show, inspect effective profile |
| `profiles.<name>.transport.custom_headers` | profile override | empty | provider-specific compatibility headers | config show, inspect effective profile |

## Provider Vendor Policy

| Config key | Scope | Default | Applies to | Notes |
| --- | --- | --- | --- | --- |
| `profiles.<name>.vendor.azure_api_version` | profile override | `2024-10-21` | `azure` only | Controls Azure request query string |
| `profiles.<name>.vendor.anthropic_version` | profile override | `2023-06-01` | `anthropic` only | Controls `anthropic-version` header |
| `profiles.<name>.vendor.allow_custom_headers` | profile override | `false` | all providers | Must be enabled before `custom_headers` are accepted |

Reserved headers such as `authorization` and `api-key` cannot be overridden through `custom_headers`.

## Runtime Policy

| Config key | Scope | Default | Meaning | Visible in |
| --- | --- | --- | --- | --- |
| `runtime.max_provider_round_trips` | workspace | `8` | maximum assistant provider/tool loop rounds before the run stops | `mosaic config show`, `mosaic inspect` |
| `runtime.max_workflow_provider_round_trips` | workspace | `8` | maximum provider/tool loop rounds inside workflow prompt steps | config show, inspect |
| `runtime.continue_after_tool_error` | workspace | `false` | when `true`, tool failures are fed back into the conversation as tool output and the loop may continue | config show, inspect |

## Provider-Type Defaults

| Provider type | Default timeout | Default retries | Default backoff |
| --- | --- | --- | --- |
| `mock` | `0` | `0` | `0` |
| `openai` | `45000` ms | `2` | `150` ms |
| `azure` | `45000` ms | `2` | `150` ms |
| `openai-compatible` | `45000` ms | `2` | `150` ms |
| `anthropic` | `60000` ms | `2` | `150` ms |
| `ollama` | `90000` ms | `1` | `150` ms |

These values are defaults, not hard-coded operator ceilings. Use profile-level overrides when a provider requires different behavior.

## Operator Checklist

After changing any policy:

```bash
mosaic setup validate
mosaic config show
mosaic model list
mosaic inspect .mosaic/runs/<run-id>.json --verbose
```

You should be able to answer:

- which timeout and retry policy the run actually used
- whether the provider carried a vendor API version override
- whether the runtime stopped because of provider retries or because it hit a runtime loop ceiling
