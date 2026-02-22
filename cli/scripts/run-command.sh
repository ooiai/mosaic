mosaic --project-state --profile az-openai setup \
  --base-url "$AZURE_OPENAI_BASE_URL" \
  --api-key-env AZURE_OPENAI_API_KEY \
  --model "gpt5.2"


mosaic --project-state --profile az-openai configure --show
mosaic --project-state --profile az-openai models list
mosaic --project-state --profile az-openai ask "你好"
