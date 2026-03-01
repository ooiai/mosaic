use std::time::Duration;

use serde_json::{Value, json};

use mosaic_core::error::{MosaicError, Result};
use mosaic_ops::{
    ApprovalDecision, ApprovalStore, SandboxStore, SystemEventStore, UnifiedLogEntry, collect_logs,
    evaluate_approval, evaluate_sandbox, list_profiles, snapshot_presence, system_events_path,
};

use super::{
    AllowlistCommand, ApprovalsArgs, ApprovalsCommand, Cli, LogsArgs, SafetyArgs, SafetyCommand,
    SandboxArgs, SandboxCommand, SystemArgs, SystemCommand, dispatch_system_event,
    parse_json_input, print_json, resolve_state_paths,
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

struct ApprovalDecisionView {
    decision: &'static str,
    reason: Option<String>,
    approved_by: Option<String>,
}

struct SafetyCheckView {
    command: String,
    decision: &'static str,
    reason: Option<String>,
    approved_by: Option<String>,
    sandbox_decision: &'static str,
    sandbox_reason: Option<String>,
    approval_decision: &'static str,
    approval_reason: Option<String>,
    approval_mode: String,
    sandbox_profile: String,
}

fn approval_decision_view(decision: ApprovalDecision) -> ApprovalDecisionView {
    match decision {
        ApprovalDecision::Auto { approved_by } => ApprovalDecisionView {
            decision: "auto",
            reason: None,
            approved_by: Some(approved_by),
        },
        ApprovalDecision::NeedsConfirmation { reason } => ApprovalDecisionView {
            decision: "confirm",
            reason: Some(reason),
            approved_by: None,
        },
        ApprovalDecision::Deny { reason } => ApprovalDecisionView {
            decision: "deny",
            reason: Some(reason),
            approved_by: None,
        },
    }
}

fn evaluate_safety(
    command: &str,
    policy: &mosaic_ops::ApprovalPolicy,
    profile: mosaic_ops::SandboxProfile,
) -> SafetyCheckView {
    let sandbox_reason = evaluate_sandbox(command, profile);
    let sandbox_decision = if sandbox_reason.is_some() {
        "deny"
    } else {
        "allow"
    };
    let approval = approval_decision_view(evaluate_approval(command, policy));

    let (decision, reason) = if let Some(reason) = sandbox_reason.clone() {
        ("deny", Some(reason))
    } else {
        match approval.decision {
            "deny" => ("deny", approval.reason.clone()),
            "confirm" => ("confirm", approval.reason.clone()),
            _ => ("allow", None),
        }
    };

    SafetyCheckView {
        command: command.to_string(),
        decision,
        reason,
        approved_by: approval.approved_by.clone(),
        sandbox_decision,
        sandbox_reason,
        approval_decision: approval.decision,
        approval_reason: approval.reason,
        approval_mode: format!("{:?}", policy.mode).to_lowercase(),
        sandbox_profile: format!("{:?}", profile).to_lowercase(),
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
            let approval = approval_decision_view(evaluate_approval(&command, &policy));
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "command": command,
                    "decision": approval.decision,
                    "approved_by": approval.approved_by,
                    "reason": approval.reason,
                    "policy_mode": policy.mode,
                    "path": store.path().display().to_string(),
                }));
            } else {
                println!("command: {command}");
                println!("decision: {}", approval.decision);
                if let Some(approved_by) = approval.approved_by {
                    println!("approved_by: {approved_by}");
                }
                if let Some(reason) = approval.reason {
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

pub(super) fn handle_safety(cli: &Cli, args: SafetyArgs) -> Result<()> {
    let paths = resolve_state_paths(cli.project_state)?;
    paths.ensure_dirs()?;
    let approval_store = ApprovalStore::new(paths.approvals_policy_path.clone());
    let sandbox_store = SandboxStore::new(paths.sandbox_policy_path.clone());
    let approval_policy = approval_store.load_or_default()?;
    let sandbox_policy = sandbox_store.load_or_default()?;

    match args.command {
        SafetyCommand::Get => {
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "approvals": {
                        "policy": approval_policy,
                        "path": approval_store.path().display().to_string(),
                    },
                    "sandbox": {
                        "policy": sandbox_policy,
                        "path": sandbox_store.path().display().to_string(),
                    },
                }));
            } else {
                println!("approvals mode: {:?}", approval_policy.mode);
                if approval_policy.allowlist.is_empty() {
                    println!("approvals allowlist: <empty>");
                } else {
                    println!("approvals allowlist:");
                    for item in approval_policy.allowlist {
                        println!("- {item}");
                    }
                }
                println!("approvals path: {}", approval_store.path().display());
                println!("sandbox profile: {:?}", sandbox_policy.profile);
                println!("sandbox path: {}", sandbox_store.path().display());
            }
        }
        SafetyCommand::Check { command } => {
            let check = evaluate_safety(&command, &approval_policy, sandbox_policy.profile);
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "command": check.command,
                    "decision": check.decision,
                    "reason": check.reason,
                    "approved_by": check.approved_by,
                    "sandbox": {
                        "profile": check.sandbox_profile,
                        "decision": check.sandbox_decision,
                        "reason": check.sandbox_reason,
                    },
                    "approvals": {
                        "mode": check.approval_mode,
                        "decision": check.approval_decision,
                        "reason": check.approval_reason,
                    },
                    "paths": {
                        "approvals_policy": approval_store.path().display().to_string(),
                        "sandbox_policy": sandbox_store.path().display().to_string(),
                    }
                }));
            } else {
                println!("command: {}", check.command);
                println!("decision: {}", check.decision);
                if let Some(reason) = check.reason {
                    println!("reason: {reason}");
                }
                if let Some(approved_by) = check.approved_by {
                    println!("approved_by: {approved_by}");
                }
                println!(
                    "sandbox: {} ({})",
                    check.sandbox_profile, check.sandbox_decision
                );
                if let Some(reason) = check.sandbox_reason {
                    println!("sandbox_reason: {reason}");
                }
                println!(
                    "approvals: {} ({})",
                    check.approval_mode, check.approval_decision
                );
                if let Some(reason) = check.approval_reason {
                    println!("approval_reason: {reason}");
                }
            }
        }
        SafetyCommand::Report { command } => {
            let profile = mosaic_ops::profile_info(sandbox_policy.profile);
            let check = command
                .as_deref()
                .map(|value| evaluate_safety(value, &approval_policy, sandbox_policy.profile));
            let profiles = list_profiles();
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "approvals": {
                        "policy": approval_policy,
                        "path": approval_store.path().display().to_string(),
                    },
                    "sandbox": {
                        "policy": sandbox_policy,
                        "path": sandbox_store.path().display().to_string(),
                        "profile_info": profile,
                        "profiles": profiles,
                    },
                    "check": check.map(|value| json!({
                        "command": value.command,
                        "decision": value.decision,
                        "reason": value.reason,
                        "approved_by": value.approved_by,
                        "sandbox": {
                            "profile": value.sandbox_profile,
                            "decision": value.sandbox_decision,
                            "reason": value.sandbox_reason,
                        },
                        "approvals": {
                            "mode": value.approval_mode,
                            "decision": value.approval_decision,
                            "reason": value.approval_reason,
                        },
                    })),
                }));
            } else {
                println!("safety report");
                println!("approvals mode: {:?}", approval_policy.mode);
                println!(
                    "approvals allowlist entries: {}",
                    approval_policy.allowlist.len()
                );
                println!("sandbox profile: {:?}", sandbox_policy.profile);
                println!("sandbox description: {}", profile.description);
                if profile.blocked_examples.is_empty() {
                    println!("sandbox blocked examples: <none>");
                } else {
                    println!("sandbox blocked examples:");
                    for example in profile.blocked_examples {
                        println!("- {example}");
                    }
                }
                if let Some(check) = check {
                    println!("check.command: {}", check.command);
                    println!("check.decision: {}", check.decision);
                    if let Some(reason) = check.reason {
                        println!("check.reason: {reason}");
                    }
                    if let Some(approved_by) = check.approved_by {
                        println!("check.approved_by: {approved_by}");
                    }
                    println!(
                        "check.sandbox: {} ({})",
                        check.sandbox_profile, check.sandbox_decision
                    );
                    if let Some(reason) = check.sandbox_reason {
                        println!("check.sandbox_reason: {reason}");
                    }
                    println!(
                        "check.approvals: {} ({})",
                        check.approval_mode, check.approval_decision
                    );
                    if let Some(reason) = check.approval_reason {
                        println!("check.approval_reason: {reason}");
                    }
                }
                println!("approvals path: {}", approval_store.path().display());
                println!("sandbox path: {}", sandbox_store.path().display());
            }
        }
    }

    Ok(())
}
