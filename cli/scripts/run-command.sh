## Install and check mosaic cli

cargo install --path /Users/jerrychir/Desktop/dev/coding/ooiai/mosaic/cli/crates/mosaic-cli --force
mosaic --version
mosaic --project-state --profile az-openai models list

## Check if the environment variable is set and has a reasonable length
echo "$AZURE_OPENAI_API_KEY" | wc -c



## Configure mosaic with Azure OpenAI settings
mosaic --project-state --profile az-openai setup \
  --base-url "$AZURE_OPENAI_BASE_URL" \
  --api-key-env AZURE_OPENAI_API_KEY \
  --model "gpt-5.2"
-- or
mosaic --project-state --profile az-openai configure \
    --base-url "$AZURE_OPENAI_BASE_URL" \
    --api-key-env AZURE_OPENAI_API_KEY \
    --model "gpt-5.2"


mosaic --project-state --profile az-openai configure --show
mosaic --project-state --profile az-openai models list
mosaic --project-state --profile az-openai ask "你好"


## other profiles
mosaic --project-state --profile az-openai configure \
  --base-url "$AZURE_OPENAI_BASE_URL" \
  --api_key_env AZURE_OPENAI_API_KEY \
  --model <gpt_deployment_name>

mosaic --project-state --profile az-deepseek configure \
  --base-url "$AZURE_OPENAI_BASE_URL" \
  --api_key_env AZURE_OPENAI_API_KEY \
  --model <deepseek_deployment_name>

mosaic --project-state --profile az-kimi configure \
  --base-url "$AZURE_OPENAI_BASE_URL" \
  --api_key_env AZURE_OPENAI_API_KEY \
  --model <kimi_deployment_name>
