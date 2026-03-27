# Residual Mock-First Audit

This audit records the i3 rule change:

> `mock` is allowed as an explicit dev/test lane, but it is no longer allowed to define Mosaic's default onboarding, default config, or default operator narrative.

## Product Rules

| Surface | Allowed default | `mock` status | Rule |
| --- | --- | --- | --- |
| `mosaic setup init` | real-provider-first profile | explicit opt-in only | use `--dev-mock` or `--profile mock` |
| `.mosaic/config.yaml` generated template | real provider active profile | present but inactive | `mock` may exist in `profiles`, not as silent default |
| `docs/getting-started.md` / `docs/providers.md` / `docs/full-stack.md` | real provider path | dev-only appendix | never present `mock` as onboarding default |
| top-level runnable examples | real-provider-first | optional dev-only lane | examples may still run against mock if the operator explicitly chose the dev lane |
| release acceptance | real provider only | forbidden | mock output is not release evidence |
| focused unit or transport tests | neutral or explicit test-double naming | allowed | use `mock` only when the test is actually about mock behavior |

## Residual Table

| Area | i3 action | Current rule after i3 |
| --- | --- | --- |
| `crates/config` | `default_active_profile()` changed to `gpt-5.4-mini`; `setup init` gained `--profile` and `--dev-mock` | product defaults are real-provider-first |
| `cli` | `setup init`, `model list`, `config show`, `config sources`, and `setup doctor` now expose active profile usage and onboarding state | operators can see `ready`, `pending-provider-credentials`, `pending-provider-configuration`, or `dev-mock` |
| `crates/provider` | provider profiles now expose usage classification | `mock` is labeled `dev-only-mock`; `openai-compatible` is labeled `compatibility` |
| docs and examples | main walkthroughs now start from real providers; mock lanes are marked `dev-only` | documentation no longer sends first-time operators into mock implicitly |
| `scripts/` | local smoke scripts now call `mosaic setup init --dev-mock` explicitly | offline smoke lanes remain available without redefining product defaults |
| `sdk` / `control-protocol` / `session-core` / `inspect` / `gateway` fixtures | generic profile identities were renamed to neutral or real-looking names where the test was not about mock semantics | mock-specific names remain only in mock-specific tests or explicit mock configs |

## Crate Remediation List

### Must stay real-provider-first

- `crates/config`
- `cli`
- `crates/provider`
- `docs/`
- `examples/`

### May keep explicit dev-only mock paths

- `crates/provider/tests/integration_mock_provider.rs`
- `examples/full-stack/mock-telegram.config.yaml`
- `scripts/test-full-stack-example.sh mock`
- focused gateway or SDK tests that intentionally inject `MockProvider`

### Must not regress

- `mosaic setup init` silently generating `active_profile: mock`
- docs saying the generated config starts on `mock`
- top-level example payloads defaulting to `profile: mock`
- public fixture identities using `mock` when the test is not actually about mock behavior

## Follow-Up Guardrails

- If a future change reintroduces `mock` as the generated active profile, that is a product regression, not a harmless docs tweak.
- If a new docs path needs a no-secret local smoke flow, it must say `dev-only` and use `mosaic setup init --dev-mock`.
- If a test depends on a mock provider for isolation, keep it explicit in the fixture or helper name so it does not look like the production default.
