# Provider Examples

These files are workspace-config snippets for `.mosaic/config.yaml`.

- `openai.yaml`: direct OpenAI over the OpenAI API
- `ollama.yaml`: local Ollama over its OpenAI-compatible endpoint
- `anthropic.yaml`: Anthropic through an OpenAI-compatible bridge or proxy

After copying a profile block into `.mosaic/config.yaml`, run:

```bash
mosaic setup validate
mosaic setup doctor
mosaic model list
```
