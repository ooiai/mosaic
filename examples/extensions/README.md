# Extension Examples

- `time-and-summary.yaml`: extension manifest with one manifest skill and one workflow
- `telegram-e2e.yaml`: Telegram-first acceptance manifest with explicit skill and workflow routing

Reference it from `.mosaic/config.yaml` and validate:

```yaml
extensions:
  manifests:
    - path: examples/extensions/time-and-summary.yaml
      version_pin: 0.1.0
      enabled: true
```

```bash
mosaic extension validate
mosaic extension list
```

For the live Telegram acceptance lane, copy:

- `examples/full-stack/openai-telegram-e2e.config.yaml` to `.mosaic/config.yaml`
- `examples/extensions/telegram-e2e.yaml` to `.mosaic/extensions/telegram-e2e.yaml`

Then follow [docs/telegram-real-e2e.md](../../docs/telegram-real-e2e.md).
