# Compatibility

This guide documents the compatibility expectations for Mosaic delivery artifacts.

## Workspace config schema

Current workspace schema version:

- `schema_version: 1`

A release must not silently change the meaning of an existing schema field without updating the upgrade guide.

## Compatibility matrix

| Surface | Backward expectation | Notes |
| --- | --- | --- |
| `.mosaic/config.yaml` | additive changes preferred | keep `schema_version` stable unless migration rules change |
| session files | older sessions should still load | route backfill and optional run metadata should remain tolerant |
| run traces | older traces should still inspect | missing lifecycle fields must default safely |
| Gateway HTTP API | additive changes preferred | preserve existing endpoints and event envelope shape |
| Gateway SSE stream | additive events allowed | consumers should tolerate new event variants |
| node protocol store | reconnectable and tolerant | preserve declared capability semantics |
| extension manifests | version-aware | use `version_pin` when operators require a fixed pack |

## Provider compatibility

Mosaic currently supports these provider families:

- `mock`
- `openai`
- `azure`
- `anthropic`
- `ollama`
- `openai-compatible`

Provider-specific model names can change independently of Mosaic releases.

## Release rule

If a change breaks one of the surfaces above, it must ship with:

- explicit release notes
- an updated [upgrade.md](./upgrade.md)
- compatibility notes for operators
