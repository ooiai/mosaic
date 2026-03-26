# Extension Examples

- `time-and-summary.yaml`: extension manifest with one manifest skill and one workflow

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
