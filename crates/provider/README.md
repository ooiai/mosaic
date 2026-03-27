# mosaic-provider

`mosaic-provider` turns configured provider profiles into callable LLM transports, scheduling decisions, and normalized provider failures.

## Positioning

This crate is the provider boundary for Mosaic. It keeps vendor-specific HTTP behavior, profile scheduling, tool visibility, and provider error translation out of `cli/`, `gateway`, and `runtime`.

## Architecture Layer

Agent Runtime Layer.

## Responsibilities

- Define the shared provider contract through `LlmProvider`, `Message`, `ToolDefinition`, and `CompletionResponse`.
- Build concrete providers for `mock`, `openai`, `azure`, `anthropic`, `ollama`, and `openai-compatible`.
- Hold provider profile state and scheduling policy in `ProviderProfileRegistry`.
- Translate transport and upstream failures into `ProviderError` and `ProviderErrorKind`.
- Expose tool visibility helpers that let the runtime pass only healthy, authorized tools to a model.

## Out of Scope

- Session persistence, memory compression, and transcript ownership.
- Tool execution and capability routing.
- HTTP ingress, audit logging, and control-plane authorization.
- TUI or CLI presentation.

## Public Boundary

- Traits and types: `LlmProvider`, `Message`, `Role`, `ToolCall`, `ToolDefinition`, `CompletionResponse`, `ProviderCompletion`.
- Profile and scheduling: `ProviderProfile`, `ProviderProfileRegistry`, `SchedulingRequest`, `SchedulingIntent`, `ScheduledProfile`, `ModelCapabilities`.
- Error surface: `ProviderError`, `ProviderErrorKind`, `public_error_message`.
- Vendor entrypoints: `MockProvider`, `OpenAiProvider`, `AzureProvider`, `AnthropicProvider`, `OllamaProvider`, `OpenAiCompatibleProvider`, `build_provider_from_profile`.

## Why This Is In `crates/`

Provider behavior is shared by `mosaic-runtime`, `mosaic-gateway`, CLI model/setup flows, and tests. It is a stable reusable boundary, not a one-command concern, so leaving it in `cli/` would duplicate provider logic and vendor policy.

## Relationships

- Upstream crates: `mosaic-config` provides profile configuration; `mosaic-tool-core` provides tool metadata used for tool exposure policy.
- Downstream crates: `mosaic-runtime` schedules and calls providers; `mosaic-gateway` injects the registry into long-running control-plane state; `cli` exposes model and setup surfaces on top of this crate.
- Runtime/control-plane coupling: `cli` should explain provider state, `gateway` should wire provider state, and `runtime` should be the only place that orchestrates turns. This crate should not absorb runtime planning or gateway routing.

## Minimal Use

```rust
use mosaic_config::MosaicConfig;
use mosaic_provider::ProviderProfileRegistry;

let config = MosaicConfig::default();
let registry = ProviderProfileRegistry::from_config(&config)?;
let provider = registry.build_provider(None)?;
let metadata = provider.metadata();
```

## Testing

```bash
cargo test -p mosaic-provider
```

The current test suite covers scheduling policy, structured errors, and vendor request formatting with local HTTP fixtures.

## Current Limitations

- Provider calls are request/response only; token streaming is not first-class yet.
- Vendor coverage is focused on the currently supported providers and local fixture tests.
- Provider policy is model-profile centric; it does not yet expose richer cost or latency telemetry.

## Roadmap

- Add streaming-friendly provider contracts and trace metadata.
- Split vendor fixtures into clearer protocol/live test layers.
- Expand provider capability metadata so runtime scheduling can reason about more than context window and tool support.
