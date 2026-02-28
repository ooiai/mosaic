use std::fs::{self, OpenOptions};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use clap::CommandFactory;
use clap_complete::{Shell, generate};
use mosaic_agents::{AgentStore, agent_routes_path, agents_file_path};
use mosaic_channels::{ChannelRepository, channels_events_dir, channels_file_path};
use mosaic_core::config::ConfigManager;
use mosaic_core::error::{MosaicError, Result};
use mosaic_core::session::SessionStore;
use mosaic_core::state::StateMode;
use mosaic_memory::{MemoryStore, memory_index_path, memory_status_path};
use mosaic_ops::{ApprovalStore, SandboxStore, snapshot_presence};
use serde_json::json;

use super::runtime_context::resolve_state_paths;
use super::utils::{load_json_file_opt, print_json};
use super::{
    Cli, CompletionArgs, CompletionCommand, CompletionShellArg, DirectoryArgs, GatewayServiceState,
    GatewayState,
};

pub(super) fn handle_completion(cli: &Cli, args: CompletionArgs) -> Result<()> {
    match args.command {
        CompletionCommand::Shell { shell } => {
            let mut command = Cli::command();
            let mut output = Vec::new();
            generate(completion_shell(shell), &mut command, "mosaic", &mut output);
            let script = String::from_utf8(output)
                .map_err(|err| MosaicError::Unknown(format!("invalid completion output: {err}")))?;
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "shell": completion_shell_name(shell),
                    "script": script,
                }));
            } else {
                print!("{script}");
            }
            Ok(())
        }
        CompletionCommand::Install { shell, dir } => {
            let mut command = Cli::command();
            let mut output = Vec::new();
            generate(completion_shell(shell), &mut command, "mosaic", &mut output);

            let target_dir = dir.unwrap_or(default_install_dir(shell)?);
            fs::create_dir_all(&target_dir)?;
            let path = target_dir.join(completion_file_name(shell));
            fs::write(&path, output)?;

            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "shell": completion_shell_name(shell),
                    "path": path.display().to_string(),
                }));
            } else {
                println!(
                    "installed {} completion: {}",
                    completion_shell_name(shell),
                    path.display()
                );
            }
            Ok(())
        }
    }
}

pub(super) fn handle_directory(cli: &Cli, args: DirectoryArgs) -> Result<()> {
    let paths = resolve_state_paths(cli.project_state)?;
    if args.ensure {
        paths.ensure_dirs()?;
    }
    let mode = match paths.mode {
        StateMode::Xdg => "xdg",
        StateMode::Project => "project",
    };
    let checks = json!({
        "root_dir": path_check(&paths.root_dir, true, args.check_writable),
        "config_path": path_check(&paths.config_path, false, args.check_writable),
        "models_path": path_check(&paths.models_path, false, args.check_writable),
        "data_dir": path_check(&paths.data_dir, true, args.check_writable),
        "policy_dir": path_check(&paths.policy_dir, true, args.check_writable),
        "approvals_policy_path": path_check(&paths.approvals_policy_path, false, args.check_writable),
        "sandbox_policy_path": path_check(&paths.sandbox_policy_path, false, args.check_writable),
        "system_events_path": path_check(&paths.system_events_path, false, args.check_writable),
        "sessions_dir": path_check(&paths.sessions_dir, true, args.check_writable),
        "audit_dir": path_check(&paths.audit_dir, true, args.check_writable),
        "audit_log_path": path_check(&paths.audit_log_path, false, args.check_writable),
    });

    if cli.json {
        print_json(&json!({
            "ok": true,
            "mode": mode,
            "ensured": args.ensure,
            "paths": {
                "root_dir": paths.root_dir.display().to_string(),
                "config_path": paths.config_path.display().to_string(),
                "models_path": paths.models_path.display().to_string(),
                "data_dir": paths.data_dir.display().to_string(),
                "policy_dir": paths.policy_dir.display().to_string(),
                "approvals_policy_path": paths.approvals_policy_path.display().to_string(),
                "sandbox_policy_path": paths.sandbox_policy_path.display().to_string(),
                "system_events_path": paths.system_events_path.display().to_string(),
                "sessions_dir": paths.sessions_dir.display().to_string(),
                "audit_dir": paths.audit_dir.display().to_string(),
                "audit_log_path": paths.audit_log_path.display().to_string(),
            },
            "checks": checks,
        }));
    } else {
        println!("mode: {mode}");
        println!("ensured: {}", args.ensure);
        println!("root: {}", paths.root_dir.display());
        println!("config: {}", paths.config_path.display());
        println!("models: {}", paths.models_path.display());
        println!("data: {}", paths.data_dir.display());
        println!("policy: {}", paths.policy_dir.display());
        println!(
            "approvals policy: {}",
            paths.approvals_policy_path.display()
        );
        println!("sandbox policy: {}", paths.sandbox_policy_path.display());
        println!("system events: {}", paths.system_events_path.display());
        println!("sessions: {}", paths.sessions_dir.display());
        println!("audit dir: {}", paths.audit_dir.display());
        println!("audit log: {}", paths.audit_log_path.display());
        if args.check_writable {
            println!("writable checks:");
            print_check_line("root_dir", &checks);
            print_check_line("config_path", &checks);
            print_check_line("models_path", &checks);
            print_check_line("data_dir", &checks);
            print_check_line("policy_dir", &checks);
            print_check_line("approvals_policy_path", &checks);
            print_check_line("sandbox_policy_path", &checks);
            print_check_line("system_events_path", &checks);
            print_check_line("sessions_dir", &checks);
            print_check_line("audit_dir", &checks);
            print_check_line("audit_log_path", &checks);
        }
    }
    Ok(())
}

fn path_check(path: &Path, is_dir: bool, check_writable: bool) -> serde_json::Value {
    let exists = path.exists();
    let mut body = json!({
        "exists": exists,
    });
    if check_writable {
        let writable = if is_dir {
            probe_dir_writable(path)
        } else {
            probe_file_writable(path)
        };
        if let Some(root) = body.as_object_mut() {
            root.insert("writable".to_string(), json!(writable));
        }
    }
    body
}

fn probe_dir_writable(path: &Path) -> bool {
    if !path.exists() || !path.is_dir() {
        return false;
    }
    let probe = path.join(format!(
        ".mosaic-write-check-{}-{}",
        std::process::id(),
        unique_suffix()
    ));
    match OpenOptions::new().write(true).create_new(true).open(&probe) {
        Ok(_) => {
            let _ = fs::remove_file(&probe);
            true
        }
        Err(_) => false,
    }
}

fn probe_file_writable(path: &Path) -> bool {
    if path.exists() {
        return OpenOptions::new().append(true).open(path).is_ok();
    }
    match path.parent() {
        Some(parent) => probe_dir_writable(parent),
        None => false,
    }
}

fn unique_suffix() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_nanos())
        .unwrap_or_default()
}

fn print_check_line(key: &str, checks: &serde_json::Value) {
    let exists = checks[key]["exists"].as_bool().unwrap_or(false);
    let writable = checks[key]["writable"].as_bool().unwrap_or(false);
    println!("- {key}: exists={exists} writable={writable}");
}

pub(super) fn handle_dashboard(cli: &Cli) -> Result<()> {
    let paths = resolve_state_paths(cli.project_state)?;
    paths.ensure_dirs()?;
    let state_mode = match paths.mode {
        StateMode::Xdg => "xdg",
        StateMode::Project => "project",
    };

    let manager = ConfigManager::new(paths.config_path.clone());
    let session_store = SessionStore::new(paths.sessions_dir.clone());
    let agent_store = AgentStore::new(
        agents_file_path(&paths.data_dir),
        agent_routes_path(&paths.data_dir),
    );
    let channels_repo = ChannelRepository::new(
        channels_file_path(&paths.data_dir),
        channels_events_dir(&paths.data_dir),
    );
    let approval_store = ApprovalStore::new(paths.approvals_policy_path.clone());
    let sandbox_store = SandboxStore::new(paths.sandbox_policy_path.clone());
    let memory_store = MemoryStore::new(
        memory_index_path(&paths.data_dir),
        memory_status_path(&paths.data_dir),
    );

    let mut warnings = Vec::new();

    let sessions = collect_optional("sessions", &mut warnings, || session_store.list_sessions())
        .unwrap_or_default();
    let latest_session = sessions.first().map(|entry| entry.session_id.clone());
    let latest_session_updated = sessions.first().and_then(|entry| entry.last_updated);

    let agents =
        collect_optional("agents", &mut warnings, || agent_store.list()).unwrap_or_default();
    let default_agent_id = collect_optional("agent_routes", &mut warnings, || {
        agent_store
            .load_routes()
            .map(|routes| routes.default_agent_id)
    })
    .flatten();

    let channels_status = collect_optional("channels", &mut warnings, || channels_repo.status());

    let gateway_state_path = paths.data_dir.join("gateway.json");
    let gateway_service_path = paths.data_dir.join("gateway-service.json");
    let gateway_state = collect_optional("gateway_state", &mut warnings, || {
        load_json_file_opt::<GatewayState>(&gateway_state_path)
    })
    .flatten();
    let gateway_service = collect_optional("gateway_service", &mut warnings, || {
        load_json_file_opt::<GatewayServiceState>(&gateway_service_path)
    })
    .flatten();

    let (gateway_host, gateway_port) = match (&gateway_state, &gateway_service) {
        (Some(state), _) => (state.host.clone(), state.port),
        (None, Some(service)) => (service.host.clone(), service.port),
        (None, None) => ("127.0.0.1".to_string(), 8787),
    };

    let approval_policy = collect_optional("approvals_policy", &mut warnings, || {
        approval_store.load_or_default()
    });
    let sandbox_policy = collect_optional("sandbox_policy", &mut warnings, || {
        sandbox_store.load_or_default()
    });
    let memory_status = collect_optional("memory_status", &mut warnings, || memory_store.status());

    let mut configured = false;
    let mut profile_name: Option<String> = None;
    let mut provider_kind: Option<String> = None;
    let mut model: Option<String> = None;

    if manager.exists() {
        match manager
            .load()
            .and_then(|cfg| cfg.resolve_profile(Some(&cli.profile)))
        {
            Ok(resolved) => {
                configured = true;
                profile_name = Some(resolved.profile_name);
                provider_kind =
                    Some(format!("{:?}", resolved.profile.provider.kind).to_lowercase());
                model = Some(resolved.profile.provider.model);
            }
            Err(err) => {
                warnings.push(format!("config: {err}"));
            }
        }
    }

    let cwd = std::env::current_dir()?;
    let presence = snapshot_presence(&cwd);

    if cli.json {
        print_json(&json!({
            "ok": true,
            "configured": configured,
            "profile": profile_name,
            "state_mode": state_mode,
            "latest_session": latest_session,
            "agents_count": agents.len(),
            "default_agent_id": default_agent_id,
            "dashboard": {
                "config": {
                    "configured": configured,
                    "profile": profile_name,
                    "provider_kind": provider_kind,
                    "model": model,
                    "path": manager.path().display().to_string(),
                },
                "sessions": {
                    "total": sessions.len(),
                    "latest_id": latest_session,
                    "latest_updated_at": latest_session_updated,
                },
                "agents": {
                    "total": agents.len(),
                    "default_agent_id": default_agent_id,
                },
                "channels": {
                    "total": channels_status.as_ref().map(|status| status.total_channels),
                    "healthy": channels_status.as_ref().map(|status| status.healthy_channels),
                    "with_errors": channels_status.as_ref().map(|status| status.channels_with_errors),
                    "kinds": channels_status.as_ref().map(|status| status.kinds.clone()),
                    "last_send_at": channels_status.as_ref().and_then(|status| status.last_send_at),
                },
                "gateway": {
                    "running": gateway_state.as_ref().is_some_and(|state| state.running),
                    "host": gateway_host,
                    "port": gateway_port,
                    "pid": gateway_state.as_ref().map(|state| state.pid),
                    "updated_at": gateway_state.as_ref().map(|state| state.updated_at),
                    "state_file_exists": gateway_state_path.exists(),
                    "service_file_exists": gateway_service_path.exists(),
                    "service_installed": gateway_service.as_ref().map(|service| service.installed).unwrap_or(false),
                },
                "policy": {
                    "approval_mode": approval_policy.as_ref().map(|policy| format!("{:?}", policy.mode).to_lowercase()),
                    "approval_allowlist_size": approval_policy.as_ref().map(|policy| policy.allowlist.len()),
                    "sandbox_profile": sandbox_policy.as_ref().map(|policy| format!("{:?}", policy.profile).to_lowercase()),
                },
                "memory": {
                    "indexed_documents": memory_status.as_ref().map(|status| status.indexed_documents),
                    "last_indexed_at": memory_status.as_ref().and_then(|status| status.last_indexed_at),
                    "index_path": memory_status.as_ref().map(|status| status.index_path.clone()),
                },
                "presence": presence,
            },
            "warnings": warnings,
        }));
    } else {
        println!("dashboard");
        println!("- configured: {configured}");
        println!(
            "- profile: {}",
            profile_name.unwrap_or_else(|| "<none>".to_string())
        );
        if let Some(value) = &model {
            println!("- model: {value}");
        }
        println!("- state mode: {state_mode}");
        println!("- sessions: {}", sessions.len());
        if let Some(latest) = latest_session {
            println!("- latest session: {latest}");
        }
        println!("- agents: {}", agents.len());
        if let Some(default_agent_id) = default_agent_id {
            println!("- default agent: {default_agent_id}");
        }
        if let Some(status) = channels_status {
            println!(
                "- channels: total={} healthy={} with_errors={}",
                status.total_channels, status.healthy_channels, status.channels_with_errors
            );
        } else {
            println!("- channels: unavailable");
        }
        println!(
            "- gateway: running={} target={}:{}",
            gateway_state.as_ref().is_some_and(|state| state.running),
            gateway_host,
            gateway_port
        );
        if let Some(policy) = approval_policy {
            println!(
                "- approvals: {:?} (allowlist={})",
                policy.mode,
                policy.allowlist.len()
            );
        }
        if let Some(policy) = sandbox_policy {
            println!("- sandbox: {:?}", policy.profile);
        }
        if let Some(status) = memory_status {
            println!("- memory indexed documents: {}", status.indexed_documents);
        }
        println!("- host: {}", presence.hostname);
        if !warnings.is_empty() {
            println!("warnings:");
            for warning in warnings {
                println!("- {warning}");
            }
        }
    }

    Ok(())
}

fn collect_optional<T, F>(label: &str, warnings: &mut Vec<String>, action: F) -> Option<T>
where
    F: FnOnce() -> Result<T>,
{
    match action() {
        Ok(value) => Some(value),
        Err(err) => {
            warnings.push(format!("{label}: {err}"));
            None
        }
    }
}

fn completion_shell(shell: CompletionShellArg) -> Shell {
    match shell {
        CompletionShellArg::Bash => Shell::Bash,
        CompletionShellArg::Zsh => Shell::Zsh,
        CompletionShellArg::Fish => Shell::Fish,
        CompletionShellArg::PowerShell => Shell::PowerShell,
        CompletionShellArg::Elvish => Shell::Elvish,
    }
}

fn completion_shell_name(shell: CompletionShellArg) -> &'static str {
    match shell {
        CompletionShellArg::Bash => "bash",
        CompletionShellArg::Zsh => "zsh",
        CompletionShellArg::Fish => "fish",
        CompletionShellArg::PowerShell => "powershell",
        CompletionShellArg::Elvish => "elvish",
    }
}

fn completion_file_name(shell: CompletionShellArg) -> &'static str {
    match shell {
        CompletionShellArg::Bash => "mosaic",
        CompletionShellArg::Zsh => "_mosaic",
        CompletionShellArg::Fish => "mosaic.fish",
        CompletionShellArg::PowerShell => "mosaic.ps1",
        CompletionShellArg::Elvish => "mosaic.elv",
    }
}

fn default_install_dir(shell: CompletionShellArg) -> Result<PathBuf> {
    let home = std::env::var_os("HOME").ok_or_else(|| {
        MosaicError::Config("HOME is not set; pass --dir for completion install".to_string())
    })?;
    let home = PathBuf::from(home);
    let path = match shell {
        CompletionShellArg::Bash => home.join(".local/share/bash-completion/completions"),
        CompletionShellArg::Zsh => home.join(".zfunc"),
        CompletionShellArg::Fish => home.join(".config/fish/completions"),
        CompletionShellArg::PowerShell => home.join("Documents/PowerShell/Completions"),
        CompletionShellArg::Elvish => home.join(".config/elvish/lib"),
    };
    Ok(path)
}
