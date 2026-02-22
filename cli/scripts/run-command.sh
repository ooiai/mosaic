cargo install --path /Users/jerrychir/Desktop/dev/coding/ooiai/mosaic/cli/crates/mosaic-cli --force
mosaic --version
mosaic --project-state --profile az-openai models list

echo "$AZURE_OPENAI_API_KEY" | wc -c

mosaic --project-state --profile az-openai configure \
    --base-url "https://smartapi1.openai.azure.com/openai" \
    --api-key-env AZURE_OPENAI_API_KEY \
    --model "gpt-5.2"


mosaic --project-state --profile az-openai configure --show
mosaic --project-state --profile az-openai models list
mosaic --project-state --profile az-openai ask "你好"
