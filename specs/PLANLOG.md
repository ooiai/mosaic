# PLANLOG

This log records the current archive status of `specs/plan_**.md`.

Rules:

- completed plans live under `specs/completed/`
- planned / not-yet-implemented plans stay in `specs/`
- `logged_at` is the date this archive was normalized
- `executed_at` is the best-known execution date from git history or user confirmation
- `status` distinguishes committed work from completed-but-uncommitted workspace state

Status meanings:

- `completed (committed)` means a representative implementation commit exists in git history
- `completed (working tree)` means the work is present in the current workspace but not yet committed
- `completed (historical)` means the plan was executed earlier, but no isolated commit could be reconstructed cleanly
- `completed (user-confirmed)` means completion came from an explicit user confirmation rather than a single recoverable commit

## Completed

| logged_at | executed_at | plan | status | location | verification | commit_ref | note |
| --- | --- | --- | --- | --- | --- | --- | --- |
| 2026-04-02 | historical | plan_a.md | completed (historical) | `specs/completed/plan_a.md` | archived baseline | mixed / not isolated | initial workspace and runtime skeleton |
| 2026-04-02 | historical | plan_a2.md | completed (historical) | `specs/completed/plan_a2.md` | archived baseline | mixed / not isolated | bootstrap and runtime follow-up |
| 2026-04-02 | historical | plan_b.md | completed (historical) | `specs/completed/plan_b.md` | archived baseline | mixed / not isolated | provider loop and tool calling baseline |
| 2026-04-02 | historical | plan_c.md | completed (historical) | `specs/completed/plan_c.md` | archived baseline | mixed / not isolated | tool call ids and read-file path |
| 2026-04-02 | user-confirmed | plan_c2.md | completed (user-confirmed) | `specs/completed/plan_c2.md` | explicit user confirmation | user confirmation | user-confirmed completed baseline |
| 2026-04-02 | 2026-03-24 | plan_c3.md | completed (committed) | `specs/completed/plan_c3.md` | code + tests | `73f5cab` | mock provider state machine and inspect refresh |
| 2026-04-02 | historical | plan_c4.md | completed (historical) | `specs/completed/plan_c4.md` | archived baseline | mixed / not isolated | durations, summaries, verbose run events |
| 2026-04-02 | 2026-03-24 | plan_c5.md | completed (committed) | `specs/completed/plan_c5.md` | code + tests | `a13a45f` | runtime event sink model |
| 2026-04-02 | 2026-03-24 | plan_c6.md | completed (committed) | `specs/completed/plan_c6.md` | code + tests | `0db84c4` | TUI event ingestion path |
| 2026-04-02 | 2026-03-24 | plan_c7.md | completed (committed) | `specs/completed/plan_c7.md` | code + tests | `bbe8b26` | composite sink and TUI injection |
| 2026-04-02 | 2026-03-24 | plan_d1.md | completed (committed) | `specs/completed/plan_d1.md` | code + tests | `037de3a` | `run --tui` and finalized runtime event flow |
| 2026-04-02 | 2026-03-24 | plan_d1-1.md | completed (committed) | `specs/completed/plan_d1-1.md` | validation commit | `f754899` | realtime flow validation |
| 2026-04-02 | 2026-03-25 | plan_d2.md | completed (committed) | `specs/completed/plan_d2.md` | code + tests | `e419d0d` | session/runtime product deepening |
| 2026-04-02 | 2026-03-25 | plan_d3.md | completed (committed) | `specs/completed/plan_d3.md` | code + tests | `e419d0d` | gateway control-plane deepening |
| 2026-04-02 | 2026-03-25 | plan_d4.md | completed (committed) | `specs/completed/plan_d4.md` | code + tests | `06deff9` | workflow and skill orchestration |
| 2026-04-02 | 2026-03-25 | plan_d5.md | completed (committed) | `specs/completed/plan_d5.md` | code + tests | `b10a60a` | MCP transport/productization |
| 2026-04-02 | 2026-03-25 | plan_e1.md | completed (committed) | `specs/completed/plan_e1.md` | code + tests | `af46b07` | external ingress and control plane |
| 2026-04-02 | 2026-03-25 | plan_e2.md | completed (committed) | `specs/completed/plan_e2.md` | code + tests | `7e6e843` | production single-node baseline |
| 2026-04-02 | 2026-03-25 | plan_e3.md | completed (committed) | `specs/completed/plan_e3.md` | code + tests | `4aaa104` | gateway/control-plane hardening |
| 2026-04-02 | 2026-03-25 | plan_e4.md | completed (committed) | `specs/completed/plan_e4.md` | code + tests | `57290fb` | workflow/runtime production hardening |
| 2026-04-02 | 2026-03-26 | plan_e5.md | completed (committed) | `specs/completed/plan_e5.md` | code + tests | `b31570b` | remote capability / MCP hardening |
| 2026-04-02 | 2026-03-26 | plan_f1.md | completed (committed) | `specs/completed/plan_f1.md` | code + tests | `eb46f88` | external control-plane hardening |
| 2026-04-02 | 2026-03-26 | plan_f2.md | completed (committed) | `specs/completed/plan_f2.md` | code + tests | `b6f5fbc` | setup/config/TUI product entry hardening |
| 2026-04-02 | 2026-03-26 | plan_g1.md | completed (committed) | `specs/completed/plan_g1.md` | docs + tests | `9b4bd02` | onboarding docs and tutorials |
| 2026-04-02 | 2026-03-26 | plan_g2.md | completed (committed) | `specs/completed/plan_g2.md` | code + tests | `109ee0c` | provider platform and tracing |
| 2026-04-02 | 2026-03-26 | plan_g3.md | completed (committed) | `specs/completed/plan_g3.md` | code + tests | `9beeb72` | TUI as primary conversation surface |
| 2026-04-02 | 2026-03-26 | plan_g4.md | completed (committed) | `specs/completed/plan_g4.md` | code + tests | `a7b67e7` | CLI productization |
| 2026-04-02 | 2026-03-26 | plan_g5.md | completed (committed) | `specs/completed/plan_g5.md` | code + tests | `dd1431a` | runtime/gateway/session hardening |
| 2026-04-02 | 2026-03-26 | plan_g6.md | completed (committed) | `specs/completed/plan_g6.md` | code + tests | `defdbdb` | production delivery hardening |
| 2026-04-02 | 2026-03-27 | plan_h1.md | completed (committed) | `specs/completed/plan_h1.md` | code + tests | `9700797` | crate-internal extraction |
| 2026-04-02 | 2026-03-27 | plan_h2.md | completed (committed) | `specs/completed/plan_h2.md` | docs + tests | `9f9cb5d` | one README per crate |
| 2026-04-02 | 2026-03-27 | plan_h3.md | completed (committed) | `specs/completed/plan_h3.md` | code + tests | `4003783` | real integration test matrix |
| 2026-04-02 | 2026-03-27 | plan_h4.md | completed (committed) | `specs/completed/plan_h4.md` | docs + tests | `89d96b5` | non-TUI AGENTS alignment audit |
| 2026-04-02 | 2026-03-27 | plan_h5.md | completed (committed) | `specs/completed/plan_h5.md` | docs + tests | `3847a14` | docs/examples/full-stack golden path |
| 2026-04-02 | 2026-03-27 | plan_i1.md | completed (committed) | `specs/completed/plan_i1.md` | code + tests | `0ee890e` | non-TUI structural remediation |
| 2026-04-02 | 2026-03-27 | plan_i2.md | completed (committed) | `specs/completed/plan_i2.md` | code + tests | `2c42515` | no-mock real-data acceptance hardening |
| 2026-04-02 | 2026-03-27 | plan_i3.md | completed (committed) | `specs/completed/plan_i3.md` | code + tests | `c8ce1e8` | residual mock-first cleanup |
| 2026-04-02 | 2026-03-28 | plan_j1.md | completed (committed) | `specs/completed/plan_j1.md` | code + tests | `ed24215` | channel interaction contract and Telegram replies |
| 2026-04-02 | 2026-03-28 | plan_j2.md | completed (committed) | `specs/completed/plan_j2.md` | code + tests | `57d0407` | config-driven channel routing |
| 2026-04-02 | 2026-03-28 | plan_j3.md | completed (committed) | `specs/completed/plan_j3.md` | docs + tests | `3c032b6` | Telegram-first real acceptance baseline |
| 2026-04-02 | 2026-03-28 | plan_j4.md | completed (committed) | `specs/completed/plan_j4.md` | code + tests | `84db9e9` | CLI operator baseline |
| 2026-04-02 | 2026-03-28 | plan_j5.md | completed (committed) | `specs/completed/plan_j5.md` | code + tests | `de1afdb` | crate real-test matrix baseline |
| 2026-04-02 | 2026-03-30 | plan_k1.md | completed (committed) | `specs/completed/plan_k1.md` | code + tests | `e67e908` | channel command discoverability patch |
| 2026-04-02 | 2026-03-30 | plan_k2.md | completed (committed) | `specs/completed/plan_k2.md` | code + tests | `f743480` | file upload and multimodal routing patch |
| 2026-04-02 | 2026-03-30 | plan_k3.md | completed (committed) | `specs/completed/plan_k3.md` | code + tests | `4d4a64c` | Telegram multi-bot tenancy patch |
| 2026-04-02 | 2026-03-30 | plan_k4.md | completed (committed) | `specs/completed/plan_k4.md` | code + tests | `4d4a64c` | bot/model/provider routing policy patch |
| 2026-04-02 | 2026-03-30 | plan_k5.md | completed (committed) | `specs/completed/plan_k5.md` | docs + tests | `bf7f10f` | docs/examples/testing refresh patch |
| 2026-04-02 | 2026-03-31 | plan_k6.md | completed (committed) | `specs/completed/plan_k6.md` | code + tests | `fbb0065` | reserved follow-up patch |
| 2026-04-02 | 2026-04-01 | plan_l1.md | completed (committed) | `specs/completed/plan_l1.md` | code + tests | `7e473e6` | markdown skill packs |
| 2026-04-02 | 2026-04-01 | plan_l2.md | completed (committed) | `specs/completed/plan_l2.md` | code + tests | `eaa4019` | workspace-local sandbox environments |
| 2026-04-02 | 2026-04-01 | plan_l3.md | completed (committed) | `specs/completed/plan_l3.md` | code + tests | `79ab672` | capability taxonomy unification |
| 2026-04-02 | 2026-04-01 | plan_l4.md | completed (committed) | `specs/completed/plan_l4.md` | code + tests | `97a0291` | provenance and reload boundaries |
| 2026-04-02 | 2026-04-01 | plan_l5.md | completed (committed) | `specs/completed/plan_l5.md` | code + tests | `91472f7` | docs / examples / release refresh |
| 2026-04-02 | 2026-04-02 | plan_l6.md | completed (working tree) | `specs/completed/plan_l6.md` | local code + tests run in workspace | uncommitted workspace | mock removed from operator-facing config |
| 2026-04-02 | 2026-04-02 | plan_l7.md | completed (working tree) | `specs/completed/plan_l7.md` | local code + tests run in workspace | uncommitted workspace | Telegram command keyboard and aliases |
| 2026-04-02 | 2026-04-02 | plan_l8.md | completed (working tree) | `specs/completed/plan_l8.md` | local code review and wiring | uncommitted workspace | outbound trace support for Telegram quick replies |
| 2026-04-02 | 2026-04-02 | plan_l9.md | completed (working tree) | `specs/completed/plan_l9.md` | local docs + tests run in workspace | uncommitted workspace | Telegram docs and acceptance refresh |
| 2026-04-02 | 2026-04-02 | plan_l10.md | completed (working tree) | `specs/completed/plan_l10.md` | local docs + targeted tests run in workspace | uncommitted workspace | Telegram documentation-coupled release surface |
| 2026-04-02 | 2026-04-02 | plan_l11.md | completed (working tree) | `specs/completed/plan_l11.md` | local code + tests run in workspace | uncommitted workspace | chat-first TUI operator surface |
| 2026-04-02 | 2026-04-02 | plan_l11-2.md | completed (working tree) | `specs/completed/plan_l11-2.md` | local code + tests + PTY input verification run in workspace | uncommitted workspace | `/mosaic` canonical TUI commands plus input/gateway contract patch; follow-up bugfix preserved composer draft across gateway session refresh so chat input now survives live TUI polling |
| 2026-04-02 | 2026-04-02 | plan_l11-1.md | completed (working tree) | `specs/completed/plan_l11-1.md` | local docs + plan alignment tests run in workspace | uncommitted workspace | TUI and Telegram contract realignment for l12-l14 |
| 2026-04-02 | 2026-04-02 | plan_l12.md | completed (working tree) | `specs/completed/plan_l12.md` | local code + tests run in workspace | uncommitted workspace | sandbox execution environment deepening |
| 2026-04-02 | 2026-04-02 | plan_l13.md | completed (working tree) | `specs/completed/plan_l13.md` | local code + tests run in workspace | uncommitted workspace | markdown skill pack execution model deepening |
| 2026-04-02 | 2026-04-02 | plan_l14.md | completed (working tree) | `specs/completed/plan_l14.md` | local code + tests run in workspace | uncommitted workspace | capability, node, and MCP real-delivery hardening |
| 2026-04-02 | 2026-04-02 | plan_l15.md | completed (working tree) | `specs/completed/plan_l15.md` | local code + docs + tests run in workspace | uncommitted workspace | Codex-style single-shell TUI rebuild |
| 2026-04-02 | 2026-04-02 | plan_l16.md | completed (working tree) | `specs/completed/plan_l16.md` | local code + docs + tests run in workspace | uncommitted workspace | bare slash TUI command system and popup rebuild |
| 2026-04-02 | 2026-04-02 | plan_l17.md | completed (working tree) | `specs/completed/plan_l17.md` | local code + docs + tests run in workspace | uncommitted workspace | transcript-native streaming and collapsed capability cards |
| 2026-04-02 | 2026-04-02 | plan_l18.md | completed (working tree) | `specs/completed/plan_l18.md` | local docs + tests run in workspace | uncommitted workspace | docs, release, and acceptance refresh for Codex-style TUI |
| 2026-04-02 | 2026-04-02 | plan_l19.md | completed (working tree) | `specs/completed/plan_l19.md` | local code + docs + tests run in workspace | uncommitted workspace | TUI shell architecture split into transcript cells, bottom pane, overlays, and modular renderers |
| 2026-04-02 | 2026-04-02 | plan_l20.md | completed (working tree) | `specs/completed/plan_l20.md` | local code + docs + tests run in workspace | uncommitted workspace | denser Codex-style shell chrome, popup polish, and busy-state UI feedback |
| 2026-04-02 | 2026-04-02 | plan_l21.md | completed (working tree) | `specs/completed/plan_l21.md` | local code + docs + tests run in workspace | uncommitted workspace | dynamic active-turn lifecycle, attached capability activity, inline detail reveal, and explicit MCP/sandbox/node progress inside one assistant cell |
| 2026-04-02 | 2026-04-02 | plan_l22.md | completed (working tree) | `specs/completed/plan_l22.md` | local docs + tests run in workspace | uncommitted workspace | Codex-style TUI parity closeout across docs, acceptance lane, release contract, and local plan metadata |
| 2026-04-03 | 2026-04-03 | plan_l24.md | completed (working tree) | `specs/completed/plan_l24.md` | local code + tests run in workspace | uncommitted workspace | closed the shell data model, retired the old bottom-pane intermediate, and moved overlay composition fully into snapshot-driven shell state |
| 2026-04-03 | 2026-04-03 | plan_l25.md | completed (working tree) | `specs/completed/plan_l25.md` | local code + tests run in workspace | uncommitted workspace | tightened shell chrome rhythm, popup grouping/windowing, bottom-pane control surface, and transcript cell hierarchy |
| 2026-04-03 | 2026-04-03 | plan_l26.md | completed (working tree) | `specs/completed/plan_l26.md` | local code + tests run in workspace | uncommitted workspace | finished live assistant turn mutation, kept retry/cancel inside the same conversation object, and pinned detail inspection to committed cell identity |
| 2026-04-03 | 2026-04-03 | plan_l27.md | completed (working tree) | `specs/completed/plan_l27.md` | local docs + tests + PTY acceptance probe run in workspace | uncommitted workspace | froze the final local PTY acceptance lane, closed the TUI/Telegram/CLI role split, and archived the staged TUI finish series |
| 2026-04-03 | 2026-04-03 | plan_m1.md | completed (committed) | `specs/completed/plan_m1.md` | code + tests | `f4cdbc3` | markdown rendering for assistant messages via pulldown-cmark |
| 2026-04-03 | 2026-04-03 | plan_m2.md | completed (committed) | `specs/completed/plan_m2.md` | code + tests | `f782cb0` | syntax highlighting for fenced code blocks in markdown renderer |
| 2026-04-03 | 2026-04-03 | plan_m3.md | completed (committed) | `specs/completed/plan_m3.md` | code + tests | `f782cb0` | width-adaptive cell rendering and auto-scroll follow flag |
| 2026-04-03 | 2026-04-03 | plan_m4.md | completed (committed) | `specs/completed/plan_m4.md` | code + tests | `f782cb0` | rich exec/tool cell with spinner animation and live output |
| 2026-04-03 | 2026-04-03 | plan_m5.md | completed (committed) | `specs/completed/plan_m5.md` | code + tests | `f782cb0` | inline diff display for file patches in detail overlay |
| 2026-04-03 | 2026-04-03 | plan_m6.md | completed (committed) | `specs/completed/plan_m6.md` | code + tests | `f4cdbc3` | status bar: git branch detection and token usage accumulation |
| 2026-04-03 | 2026-04-03 | plan_m7.md | completed (committed) | `specs/completed/plan_m7.md` | code + tests | `53164c9` | approval overlay for capability calls with y/n/Esc routing |
| 2026-04-03 | 2026-04-03 | plan_m8.md | completed (committed) | `specs/completed/plan_m8.md` | docs + tests | `53164c9` | docs update, spec assertion tests, and M-series archive |
| 2026-04-03 | 2026-04-03 | plan_n4.md | completed (committed) | `specs/completed/plan_n4.md` | code + tests | `d2d1644` | N4+N5: cursor always visible, cursor_pos sync, remove mouse capture flood |
| 2026-04-03 | 2026-04-03 | plan_n5.md | completed (committed) | `specs/completed/plan_n5.md` | code + tests | `d2d1644` | N5: cursor preservation on session switch, follow=true on push/submit |
| 2026-04-03 | 2026-04-03 | plan_n6.md | completed (committed) | `specs/completed/plan_n6.md` | code + tests | `4eef488` | N6: scroll override fix (ID-based session compare), system message filter, Copilot visual redesign |
| 2026-04-05 | 2026-04-05 | plan_o1.md | completed (committed) | `specs/completed/plan_o1.md` | code + tests | `d11accf` | O1-O3: mouse capture re-enabled, cursor hidden under overlays, enter_hint in hint line |

## Reference / Umbrella

| logged_at | executed_at | plan | status | location | verification | commit_ref | note |
| --- | --- | --- | --- | --- | --- | --- | --- |
| 2026-04-02 | 2026-04-03 | plan_l23.md | umbrella / reference | `specs/plan_l23.md` | umbrella/reference parity plan with staged execution completed through `plan_l24.md` to `plan_l27.md` | n/a | completion-grade Codex TUI parity umbrella retained as a reference spec after the staged finish series closed |

## Planned / Pending

| logged_at | executed_at | plan | status | location | verification | commit_ref | note |
| --- | --- | --- | --- | --- | --- | --- | --- |
| 2026-04-05 | 2026-04-05 | plan_p1.md | completed (committed) | `specs/completed/plan_p1.md` | code + tests | `3e8cb94` | P1-A: scroll clamp prevents infinite scroll; P1-B: transcript_len reset prevents duplicate message on DB sync |
| 2026-04-05 | 2026-04-05 | plan_p2.md | completed (committed) | `specs/completed/plan_p2.md` | code + tests | `8ba0108` | Q1: TranscriptState::new() pins scroll to bottom on load; Q2: sync redesign preserves locally-submitted user messages and deduplicates when DB catches up |
| 2026-04-05 | 2026-04-05 | plan_q.md | completed (committed) | n/a (session plan) | code + tests | `58c1648` | Q1: remove redundant title label for UserMessage/AssistantMessage cells; Q2: remove dump_render debug test |
| 2026-04-06 | 2026-04-06 | session plan (TUI bug fixes) | completed (committed) | n/a (session plan) | code + 5 regression tests | `ec58ef7` | Bug#1: double failure cells fixed via run_id dedup in finalize_active_turn; Bug#2: paste truncation fixed (InputEvent::Paste + insert_text); Bug#3: mouse scroll routes to active overlay; Bug#4: gateway RunFailed uses correct gateway_run_id |
