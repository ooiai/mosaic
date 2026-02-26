use std::fs;
use std::path::PathBuf;

use clap::CommandFactory;
use clap_complete::{Shell, generate};
use mosaic_core::error::{MosaicError, Result};
use mosaic_core::state::StateMode;
use serde_json::json;

use super::runtime_context::resolve_state_paths;
use super::utils::print_json;
use super::{Cli, CompletionArgs, CompletionCommand, CompletionShellArg};

pub(super) fn handle_completion(cli: &Cli, args: CompletionArgs) -> Result<()> {
    match args.command {
        CompletionCommand::Shell { shell } => {
            let mut command = Cli::command();
            let mut output = Vec::new();
            generate(
                completion_shell(shell),
                &mut command,
                "mosaic",
                &mut output,
            );
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
            generate(
                completion_shell(shell),
                &mut command,
                "mosaic",
                &mut output,
            );

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

pub(super) fn handle_directory(cli: &Cli) -> Result<()> {
    let paths = resolve_state_paths(cli.project_state)?;
    let mode = match paths.mode {
        StateMode::Xdg => "xdg",
        StateMode::Project => "project",
    };

    if cli.json {
        print_json(&json!({
            "ok": true,
            "mode": mode,
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
            }
        }));
    } else {
        println!("mode: {mode}");
        println!("root: {}", paths.root_dir.display());
        println!("config: {}", paths.config_path.display());
        println!("models: {}", paths.models_path.display());
        println!("data: {}", paths.data_dir.display());
        println!("policy: {}", paths.policy_dir.display());
        println!("approvals policy: {}", paths.approvals_policy_path.display());
        println!("sandbox policy: {}", paths.sandbox_policy_path.display());
        println!("system events: {}", paths.system_events_path.display());
        println!("sessions: {}", paths.sessions_dir.display());
        println!("audit dir: {}", paths.audit_dir.display());
        println!("audit log: {}", paths.audit_log_path.display());
    }
    Ok(())
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
