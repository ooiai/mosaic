# Azure OpenAI (OpenAI-compatible) Setup

This guide documents the reliable setup path for Azure OpenAI with Mosaic CLI.

## 1) Environment Variables

```bash
export AZURE_OPENAI_BASE_URL="https://<resource>.openai.azure.com/openai/v1"
export AZURE_OPENAI_API_KEY="<your-api-key>"
```

Notes:

- `AZURE_OPENAI_BASE_URL` can be either:
  - `https://<resource>.openai.azure.com/openai/v1`
  - `https://<resource>.openai.azure.com/openai`
- Mosaic provider normalizes `/v1` to avoid duplicate `/v1/v1`.
- Do not pass the literal string `AZURE_OPENAI_BASE_URL`; pass the env value.

## 2) Setup Profile

```bash
mosaic --project-state --profile az-openai setup \
  --base-url "$AZURE_OPENAI_BASE_URL" \
  --api-key-env AZURE_OPENAI_API_KEY \
  --model "gpt-5.2"
```

Important:

- Flag name is `--api-key-env` (kebab-case), not `--api_key_env`.

## 3) Validate Connectivity

```bash
mosaic --project-state --profile az-openai --json models list
mosaic --project-state --profile az-openai --json ask "hello"
```

## 4) Common Errors

### `404 Not Found` or `Resource not found`

Check:

- base URL host is correct (`<resource>.openai.azure.com`)
- base URL path is `/openai` or `/openai/v1`
- no duplicated `/v1/v1`
- model value matches your Azure deployment/model naming

### `command not found: mosaic`

Install and expose Cargo bin:

```bash
cd cli
cargo install --path crates/mosaic-cli --force
echo 'export PATH="$HOME/.cargo/bin:$PATH"' >> ~/.zshrc
source ~/.zshrc
```
