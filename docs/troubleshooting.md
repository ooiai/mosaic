# Troubleshooting

This page collects the first operator checks to run when Mosaic does not behave the way you expect.

## Start here

```bash
mosaic setup validate
mosaic setup doctor
mosaic gateway status
```

## `configuration validation failed`

What it usually means:

- `active_profile` does not exist in `profiles`
- an `openai-compatible` profile is missing `api_key_env`
- `deployment.profile` is invalid

What to do next:

- inspect `.mosaic/config.yaml`
- compare it with [docs/configuration.md](./configuration.md)
- copy a known-good provider example from `examples/providers/`
- rerun `mosaic setup validate`

## `doctor found errors`

Typical causes:

- missing provider credential environment variable
- bad session or run directory permissions
- production auth requirements not configured

What to do next:

- read the reported check line carefully
- export the missing environment variable
- rerun `mosaic setup doctor`

## `unsupported provider type`

Current valid runtime provider types are:

- `mock`
- `openai`
- `azure`
- `anthropic`
- `ollama`
- `openai-compatible`

Compare with:

- [docs/providers.md](./providers.md)
- [examples/providers/README.md](../examples/providers/README.md)

## OpenAI key missing

If your profile uses:

```yaml
api_key_env: OPENAI_API_KEY
```

then export it before running Mosaic:

```bash
export OPENAI_API_KEY=your_api_key_here
```

## Ollama config validates but requests fail

Check:

- Ollama is running
- the OpenAI-compatible endpoint is reachable at `http://127.0.0.1:11434/v1`
- the model name exists locally
- `OLLAMA_API_KEY` is set if validation expects it

## TUI starts but no real model output appears

Check:

- `mosaic model list`
- `mosaic setup validate`
- `mosaic setup doctor`
- `mosaic gateway status`

If you are still on `mock`, the TUI is working but you are not talking to a real provider yet.

## `session not found`

List sessions first:

```bash
mosaic session list
```

Then open the exact ID:

```bash
mosaic session show <session-id>
```

## `inspect` cannot find the trace

List the run directory:

```bash
ls .mosaic/runs
```

Then inspect the exact file:

```bash
mosaic inspect .mosaic/runs/<run-id>.json
```

## Remote attach fails or returns auth errors

Check:

- the server is running with `mosaic gateway serve --http ...`
- the URL is correct
- `MOSAIC_OPERATOR_TOKEN` is set if operator auth is enabled
- ingress shared secrets are configured when needed

For a known-good HTTP Gateway + Telegram path, compare against:

- [docs/full-stack.md](./full-stack.md)
- [examples/full-stack/openai-telegram.config.yaml](../examples/full-stack/openai-telegram.config.yaml)
- [examples/channels/telegram-update.json](../examples/channels/telegram-update.json)

## Need a compact incident package

Use:

```bash
mosaic gateway incident <run-id>
```

Then inspect the saved JSON under `.mosaic/audit/incidents/`.

## Gateway starts but ingress requests fail

Check:

- `mosaic adapter status`
- `mosaic gateway status`
- the correct ingress path, for example `/ingress/webchat` or `/ingress/telegram`
- the correct shared-secret header when `auth.webchat_shared_secret_env` or `auth.telegram_secret_token_env` is configured

Known-good payloads:

- [examples/channels/webchat-message.json](../examples/channels/webchat-message.json)
- [examples/channels/telegram-update.json](../examples/channels/telegram-update.json)
