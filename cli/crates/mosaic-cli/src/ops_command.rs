use std::time::Duration;

use serde_json::{Value, json};

use mosaic_core::error::{MosaicError, Result};
use mosaic_ops::{
    ApprovalStore, SandboxStore, SystemEventStore, collect_logs, list_profiles, snapshot_presence,
    system_events_path,
};

use super::{
    AllowlistCommand, ApprovalsArgs, ApprovalsCommand, Cli, LogsArgs, SandboxArgs, SandboxCommand,
    SystemArgs, SystemCommand, dispatch_system_event, parse_json_input, print_json,
    resolve_state_paths,
};

pub(super) async fn handle_logs(cli: &Cli, args: LogsArgs) -> Result<()> {
    let paths = resolve_state_paths(cli.project_state)?;
    paths.ensure_dirs()?;

    if !args.follow {
        let entries = collect_logs(&paths.data_dir, args.tail)?;
        if cli.json {
            print_json(&json!({
                "ok": true,
                "logs": entries,
            }));
        } else if entries.is_empty() {
            println!("No logs found.");
        } else {
            for entry in entries {
                println!(
                    "{} [{}] {}",
                    entry
                        .ts
                        .map(|value| value.to_rfc3339())
                        .unwrap_or_else(|| "-".to_string()),
                    entry.source,
                    entry.payload
                );
            }
        }
        return Ok(());
    }

    let mut printed = 0usize;
    loop {
        let entries = collect_logs(&paths.data_dir, args.tail.max(200))?;
        if entries.len() > printed {
            for entry in entries.iter().skip(printed) {
                println!(
                    "{} [{}] {}",
                    entry
                        .ts
                        .map(|value| value.to_rfc3339())
                        .unwrap_or_else(|| "-".to_string()),
                    entry.source,
                    entry.payload
                );
            }
            printed = entries.len();
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
}

pub(super) fn handle_system(cli: &Cli, args: SystemArgs) -> Result<()> {
    let paths = resolve_state_paths(cli.project_state)?;
    paths.ensure_dirs()?;
    let store = SystemEventStore::new(system_events_path(&paths.data_dir));
    match args.command {
        SystemCommand::Event { name, data } => {
            let data = data
                .as_deref()
                .map(|value| parse_json_input(value, "system event data"))
                .transpose()?
                .unwrap_or(Value::Null);
            let dispatch = dispatch_system_event(cli, &paths, &name, data)?;
            let event = dispatch.event;
            let hook_reports = dispatch.hook_reports;
            let hooks_ok = hook_reports.iter().filter(|item| item.ok).count();
            let hooks_failed = hook_reports.len().saturating_sub(hooks_ok);
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "event": event,
                    "path": store.path().display().to_string(),
                    "hooks": {
                        "triggered": hook_reports.len(),
                        "ok": hooks_ok,
                        "failed": hooks_failed,
                        "results": hook_reports,
                    }
                }));
            } else {
                println!("event recorded: {}", event.name);
                println!("path: {}", store.path().display());
                if hook_reports.is_empty() {
                    println!("hooks triggered: 0");
                } else {
                    println!("hooks triggered: {}", hook_reports.len());
                    println!("hooks ok: {hooks_ok}");
                    println!("hooks failed: {hooks_failed}");
                }
            }
        }
        SystemCommand::Presence => {
            let cwd = std::env::current_dir().map_err(|err| MosaicError::Io(err.to_string()))?;
            let presence = snapshot_presence(&cwd);
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "presence": presence,
                }));
            } else {
                println!("presence:");
                println!("hostname: {}", presence.hostname);
                println!("pid: {}", presence.pid);
                println!("cwd: {}", presence.cwd);
                println!("ts: {}", presence.ts.to_rfc3339());
            }
        }
    }
    Ok(())
}

pub(super) fn handle_approvals(cli: &Cli, args: ApprovalsArgs) -> Result<()> {
    let paths = resolve_state_paths(cli.project_state)?;
    paths.ensure_dirs()?;
    let store = ApprovalStore::new(paths.approvals_policy_path.clone());
    let policy = match args.command {
        ApprovalsCommand::Get => store.load_or_default()?,
        ApprovalsCommand::Set { mode } => store.set_mode(mode.into())?,
        ApprovalsCommand::Allowlist { command } => match command {
            AllowlistCommand::Add { prefix } => store.add_allowlist(&prefix)?,
            AllowlistCommand::Remove { prefix } => store.remove_allowlist(&prefix)?,
        },
    };

    if cli.json {
        print_json(&json!({
            "ok": true,
            "policy": policy,
            "path": store.path().display().to_string(),
        }));
    } else {
        println!("approvals mode: {:?}", policy.mode);
        if policy.allowlist.is_empty() {
            println!("allowlist: <empty>");
        } else {
            println!("allowlist:");
            for item in policy.allowlist {
                println!("- {item}");
            }
        }
        println!("path: {}", store.path().display());
    }
    Ok(())
}

pub(super) fn handle_sandbox(cli: &Cli, args: SandboxArgs) -> Result<()> {
    let paths = resolve_state_paths(cli.project_state)?;
    paths.ensure_dirs()?;
    let store = SandboxStore::new(paths.sandbox_policy_path.clone());
    let policy = store.load_or_default()?;
    match args.command {
        SandboxCommand::List => {
            let profiles = list_profiles();
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "current": policy.profile,
                    "profiles": profiles,
                    "path": store.path().display().to_string(),
                }));
            } else {
                println!("current sandbox profile: {:?}", policy.profile);
                for profile in profiles {
                    println!("- {:?}: {}", profile.profile, profile.description);
                }
            }
        }
        SandboxCommand::Explain { profile } => {
            let profile = profile.map(Into::into).unwrap_or(policy.profile);
            let info = mosaic_ops::profile_info(profile);
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "profile": info,
                    "path": store.path().display().to_string(),
                }));
            } else {
                println!("sandbox profile: {:?}", info.profile);
                println!("{}", info.description);
                if info.blocked_examples.is_empty() {
                    println!("blocked examples: <none>");
                } else {
                    println!("blocked examples:");
                    for example in info.blocked_examples {
                        println!("- {example}");
                    }
                }
            }
        }
    }
    Ok(())
}
