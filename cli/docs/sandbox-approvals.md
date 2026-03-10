# Sandbox + Approvals Policies

This document covers V2 policy controls used by `run_cmd`.
`plugins run` also consumes sandbox policy (or plugin runtime override) before executing hook scripts.

## Policy Files

- Approvals: `.mosaic/policy/approvals.toml`
- Sandbox: `.mosaic/policy/sandbox.toml`

In XDG mode, files are stored under the Mosaic config root.

## Approval Modes

- `deny`: block command execution
- `confirm`: require confirmation for command execution
- `allowlist`: auto-approve only allowlisted command prefixes

Commands:

```bash
mosaic --project-state approvals get
mosaic --project-state approvals check --command "cargo test --workspace"
mosaic --project-state approvals set confirm
mosaic --project-state approvals set allowlist
mosaic --project-state approvals allowlist add "cargo test"
mosaic --project-state approvals allowlist list
mosaic --project-state approvals allowlist remove "cargo test"
```

## Sandbox Profiles

- `restricted`: blocks network/system-impacting commands (`curl`, `ssh`, `docker`, `sudo`, ...)
- `standard`: normal developer mode (still subject to guard + approvals)
- `elevated`: least restrictive

Commands:

```bash
mosaic --project-state sandbox get
mosaic --project-state sandbox set restricted
mosaic --project-state sandbox check --command "curl https://example.com"
mosaic --project-state sandbox list
mosaic --project-state sandbox explain --profile restricted
```

## Runtime Order

`run_cmd` is processed in this order:

1. Sandbox policy
2. Approval policy
3. Existing tool guard (`confirm_dangerous` / `all_confirm` / `unrestricted`)
4. Command execution and audit log write

## Private Data Guard (New)

`mosaic-tools` now enforces privacy guardrails for local sensitive data.

Default behavior:

1. `read_file` / `write_file`
   - blocks critical sensitive files:
     - shell profiles/histories (`.zshrc`, `.bashrc`, `.zsh_history`, ...)
     - key material (`.ssh/*`, `.gnupg/*`, `*.pem`, `id_rsa`, ...)
     - cloud cluster credentials (`.aws/credentials`, `.kube/config`)
   - treats `.env` / `.env.*` as confidential:
     - requires explicit `--yes` (otherwise `approval_required`)
2. `search_text`
   - automatically skips sensitive files unless explicitly allowed
   - in non-`--yes` mode, confidential files are excluded from results
3. `run_cmd`
   - blocks direct command access to sensitive local credential/profile paths
   - requires confirmation for `env` / `printenv` style dumps
4. Tool output redaction
   - redacts secret-like payloads in `read_file`, `write_file`, `search_text`, and `run_cmd` outputs
   - examples: `*_TOKEN`, `*_API_KEY`, `password=...`, OpenAI-like keys, AWS key markers, bearer tokens
5. Persistence guard (session/audit/events)
   - before writing `sessions/*.jsonl`, `audit/commands.jsonl`, system events, channel events, gateway/realtime events, hook/webhook/cron events, plugin runtime events, observability history files, and knowledge evaluate history:
     - secret-like values are redacted
     - private key markers (`BEGIN ... PRIVATE KEY`) trigger hard block (write rejected)

Override env switches (for explicit audited admin workflows):

- `MOSAIC_ALLOW_SENSITIVE_FILES=1`
- `MOSAIC_ALLOW_SENSITIVE_COMMANDS=1`
- `MOSAIC_DISABLE_SECRET_REDACTION=1`

`plugins run` is processed in this order:

1. Resolve timeout (`--timeout-ms` > plugin `[runtime].timeout_ms` > default `15000ms`)
2. Resolve output cap (`[runtime].max_output_bytes` > default `262144` bytes per stream)
3. Resolve runtime resource limits (`[runtime].max_cpu_ms`, `[runtime].max_rss_kb`)
4. Resolve sandbox profile (plugin `[runtime].sandbox_profile` > global sandbox profile)
5. Restricted-shell preflight checks on hook script lines
6. Evaluate approvals policy (`auto|confirm|deny`), `confirm` requires `--yes` in non-interactive flow
7. Hook execution (`max_cpu_ms` applies proactive unix `RLIMIT_CPU` pre-exec; supported unix targets apply proactive memory rlimits for safe `max_rss_kb` thresholds: `RLIMIT_AS` on linux/android and `RLIMIT_DATA` on BSD targets, while other unix targets fall back to post-run checks) + cpu wall-time watchdog fallback (`[runtime].cpu_watchdog_ms` override or derived from `max_cpu_ms` when tighter than global timeout; used as non-unix CPU-limit fallback) + runtime metrics capture (`cpu_user_ms/cpu_system_ms/cpu_total_ms/max_rss_kb`) + output truncation guardrails
8. Resource-limit evaluation (`max_rss_kb` post-run metrics checks require unix-like metrics support; non-unix rejects `max_rss_kb` during validation) + event write (`.mosaic/data/plugin-events/<plugin_id>.jsonl`)

## Safety Command Surface

`safety` provides a single entry point that merges sandbox + approvals decisions for one command.

```bash
mosaic --project-state safety get
mosaic --project-state safety check --command "cargo test --workspace"
mosaic --project-state safety report --command "curl https://example.com" --audit-tail 100 --compare-window 100
```

- `safety get`: current approvals+sandbox policies and paths.
- `safety check`: effective decision (`allow|confirm|deny`) for one command.
- `safety report`: profile descriptions plus optional merged decision result, audit summary, and window diff comparison (`--compare-window`) from `.mosaic/data/audit/commands.jsonl`.

## Error Codes

- `approval_required`
- `sandbox_denied`
