# Sandbox + Approvals Policies

This document covers V2 policy controls used by `run_cmd`.

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
mosaic --project-state approvals set confirm
mosaic --project-state approvals set allowlist
mosaic --project-state approvals allowlist add "cargo test"
mosaic --project-state approvals allowlist remove "cargo test"
```

## Sandbox Profiles

- `restricted`: blocks network/system-impacting commands (`curl`, `ssh`, `docker`, `sudo`, ...)
- `standard`: normal developer mode (still subject to guard + approvals)
- `elevated`: least restrictive

Commands:

```bash
mosaic --project-state sandbox list
mosaic --project-state sandbox explain --profile restricted
```

## Runtime Order

`run_cmd` is processed in this order:

1. Sandbox policy
2. Approval policy
3. Existing tool guard (`confirm_dangerous` / `all_confirm` / `unrestricted`)
4. Command execution and audit log write

## Error Codes

- `approval_required`
- `sandbox_denied`
