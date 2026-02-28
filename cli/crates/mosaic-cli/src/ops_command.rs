use std::time::Duration;

use serde_json::{Value, json};

use mosaic_core::error::{MosaicError, Result};
use mosaic_ops::{
    ApprovalDecision, ApprovalStore, SandboxStore, SystemEventStore, UnifiedLogEntry, collect_logs,
    evaluate_approval, evaluate_sandbox, list_profiles, snapshot_presence, system_events_path,
};

use super::{
    AllowlistCommand, ApprovalsArgs, ApprovalsCommand, Cli, LogsArgs, SandboxArgs, SandboxCommand,
    SystemArgs, SystemCommand, dispatch_system_event, parse_json_input, print_json,
    resolve_state_paths,
};

pub(super) async fn handle_logs(cli: &Cli, args: LogsArgs) -> Result<()> {
    let paths = resolve_state_paths(cli.project_state)?;
    paths.ensure_dirs()?;
    let source_filter = args.source.as_deref();

    if !args.follow {
        let entries = filter_logs(collect_logs(&paths.data_dir, args.tail)?, source_filter);
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
        let entries = filter_logs(
            collect_logs(&paths.data_dir, args.tail.max(200))?,
            source_filter,
        );
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

fn filter_logs(entries: Vec<UnifiedLogEntry>, source_filter: Option<&str>) -> Vec<UnifiedLogEntry> {
    match source_filter {
        Some(source) => entries
            .into_iter()
            .filter(|entry| entry.source == source)
            .collect(),
        None => entries,
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
        SystemCommand::List { tail, name } => {
            let mut events = store.read_tail(tail)?;
            if let Some(name_filter) = name.as_deref() {
                events.retain(|event| event.name == name_filter);
            }
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "events": events,
                    "path": store.path().display().to_string(),
                }));
            } else if events.is_empty() {
                println!("No system events.");
            } else {
                for event in events {
                    println!("{} {} {}", event.ts.to_rfc3339(), event.name, event.data);
                }
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
        ApprovalsCommand::Check { command } => {
            let policy = store.load_or_default()?;
            let decision = evaluate_approval(&command, &policy);
            let (decision_name, reason, approved_by) = match decision {
                ApprovalDecision::Auto { approved_by } => ("auto", None, Some(approved_by)),
                ApprovalDecision::NeedsConfirmation { reason } => ("confirm", Some(reason), None),
                ApprovalDecision::Deny { reason } => ("deny", Some(reason), None),
            };
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "command": command,
                    "decision": decision_name,
                    "approved_by": approved_by,
                    "reason": reason,
                    "policy_mode": policy.mode,
                    "path": store.path().display().to_string(),
                }));
            } else {
                println!("command: {command}");
                println!("decision: {decision_name}");
                if let Some(approved_by) = approved_by {
                    println!("approved_by: {approved_by}");
                }
                if let Some(reason) = reason {
                    println!("reason: {reason}");
                }
                println!("approvals mode: {:?}", policy.mode);
                println!("path: {}", store.path().display());
            }
            return Ok(());
        }
        ApprovalsCommand::Get => store.load_or_default()?,
        ApprovalsCommand::Set { mode } => store.set_mode(mode.into())?,
        ApprovalsCommand::Allowlist { command } => match command {
            AllowlistCommand::List => store.load_or_default()?,
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
    match args.command {
        SandboxCommand::Get => {
            let policy = store.load_or_default()?;
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "policy": policy,
                    "path": store.path().display().to_string(),
                }));
            } else {
                println!("sandbox profile: {:?}", policy.profile);
                println!("path: {}", store.path().display());
            }
        }
        SandboxCommand::Set { profile } => {
            let policy = store.set_profile(profile.into())?;
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "policy": policy,
                    "path": store.path().display().to_string(),
                }));
            } else {
                println!("sandbox profile set: {:?}", policy.profile);
                println!("path: {}", store.path().display());
            }
        }
        SandboxCommand::Check { command } => {
            let policy = store.load_or_default()?;
            let reason = evaluate_sandbox(&command, policy.profile);
            let decision = if reason.is_some() { "deny" } else { "allow" };
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "command": command,
                    "decision": decision,
                    "reason": reason,
                    "profile": policy.profile,
                    "path": store.path().display().to_string(),
                }));
            } else {
                println!("command: {command}");
                println!("decision: {decision}");
                if let Some(reason) = reason {
                    println!("reason: {reason}");
                }
                println!("sandbox profile: {:?}", policy.profile);
                println!("path: {}", store.path().display());
            }
        }
        SandboxCommand::List => {
            let policy = store.load_or_default()?;
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
            let policy = store.load_or_default()?;
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
