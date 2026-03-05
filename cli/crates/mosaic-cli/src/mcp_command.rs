use std::collections::BTreeMap;

use serde_json::json;

use mosaic_core::error::{MosaicError, Result};
use mosaic_mcp::{AddMcpServerInput, McpStore, mcp_servers_file_path};

use super::{Cli, McpArgs, McpCommand, print_json, resolve_state_paths};

pub(super) fn handle_mcp(cli: &Cli, args: McpArgs) -> Result<()> {
    let paths = resolve_state_paths(cli.project_state)?;
    paths.ensure_dirs()?;
    let store = McpStore::new(mcp_servers_file_path(&paths.data_dir));
    store.ensure_dirs()?;

    match args.command {
        McpCommand::List => {
            let servers = store.list()?;
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "servers": servers,
                    "path": store.path().display().to_string(),
                }));
            } else if servers.is_empty() {
                println!("No MCP servers configured.");
            } else {
                println!("mcp servers: {}", servers.len());
                for server in servers {
                    println!(
                        "- {} ({}) command={} enabled={} cwd={}",
                        server.id,
                        server.name,
                        server.command,
                        server.enabled,
                        server.cwd.unwrap_or_else(|| "-".to_string())
                    );
                }
            }
        }
        McpCommand::Add {
            name,
            command,
            args,
            env,
            cwd,
            disabled,
        } => {
            let created = store.add(AddMcpServerInput {
                id: None,
                name,
                command,
                args,
                env: parse_env_entries(env)?,
                cwd,
                enabled: !disabled,
            })?;
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "server": created,
                    "path": store.path().display().to_string(),
                }));
            } else {
                println!("Added MCP server {} ({})", created.id, created.name);
            }
        }
        McpCommand::Show { server_id } => handle_show(cli, &store, &server_id)?,
        McpCommand::Check { server_id, all } => handle_check(cli, &store, server_id, all)?,
        McpCommand::Enable { server_id } => {
            let server = store.set_enabled(&server_id, true)?;
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "server": server,
                }));
            } else {
                println!("enabled mcp server {}", server.id);
            }
        }
        McpCommand::Disable { server_id } => {
            let server = store.set_enabled(&server_id, false)?;
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "server": server,
                }));
            } else {
                println!("disabled mcp server {}", server.id);
            }
        }
        McpCommand::Remove { server_id } => {
            let removed = store.remove(&server_id)?;
            if !removed {
                return Err(MosaicError::Validation(format!(
                    "mcp server '{server_id}' not found"
                )));
            }
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "removed": true,
                    "server_id": server_id,
                }));
            } else {
                println!("removed mcp server {}", server_id);
            }
        }
    }
    Ok(())
}

fn handle_show(cli: &Cli, store: &McpStore, server_id: &str) -> Result<()> {
    let server = store
        .get(server_id)?
        .ok_or_else(|| MosaicError::Validation(format!("mcp server '{server_id}' not found")))?;
    if cli.json {
        print_json(&json!({
            "ok": true,
            "server": server,
            "path": store.path().display().to_string(),
        }));
        return Ok(());
    }

    println!("id: {}", server.id);
    println!("name: {}", server.name);
    println!("command: {}", server.command);
    if server.args.is_empty() {
        println!("args: <none>");
    } else {
        println!("args: {}", server.args.join(" "));
    }
    if server.env.is_empty() {
        println!("env: <none>");
    } else {
        println!("env:");
        for (key, value) in server.env {
            println!("- {key}={value}");
        }
    }
    println!("cwd: {}", server.cwd.unwrap_or_else(|| "-".to_string()));
    println!("enabled: {}", server.enabled);
    println!(
        "last_check_at: {}",
        server
            .last_check_at
            .map(|value| value.to_rfc3339())
            .unwrap_or_else(|| "-".to_string())
    );
    println!(
        "last_check_error: {}",
        server.last_check_error.unwrap_or_else(|| "-".to_string())
    );
    Ok(())
}

fn handle_check(cli: &Cli, store: &McpStore, server_id: Option<String>, all: bool) -> Result<()> {
    if all || server_id.is_none() {
        let checks = store.check_all()?;
        let healthy_count = checks.iter().filter(|item| item.check.healthy).count();
        if cli.json {
            print_json(&json!({
                "ok": true,
                "all": true,
                "checked": checks.len(),
                "healthy": healthy_count,
                "unhealthy": checks.len().saturating_sub(healthy_count),
                "results": checks,
            }));
        } else if checks.is_empty() {
            println!("No MCP servers configured.");
        } else {
            println!(
                "checked={} healthy={} unhealthy={}",
                checks.len(),
                healthy_count,
                checks.len().saturating_sub(healthy_count)
            );
            for item in checks {
                println!(
                    "- {} healthy={} issues={}",
                    item.server.id,
                    item.check.healthy,
                    if item.check.issues.is_empty() {
                        "<none>".to_string()
                    } else {
                        item.check.issues.join("; ")
                    }
                );
            }
        }
        return Ok(());
    }

    let server_id = server_id.expect("checked above");
    let result = store.check(&server_id)?;
    if cli.json {
        print_json(&json!({
            "ok": true,
            "all": false,
            "healthy": result.check.healthy,
            "server": result.server,
            "check": result.check,
        }));
    } else {
        println!("server: {}", result.server.id);
        println!("healthy: {}", result.check.healthy);
        if let Some(path) = result.check.executable_resolved {
            println!("executable: {}", path);
        }
        if result.check.issues.is_empty() {
            println!("issues: <none>");
        } else {
            println!("issues:");
            for issue in result.check.issues {
                println!("- {}", issue);
            }
        }
    }
    Ok(())
}

fn parse_env_entries(entries: Vec<String>) -> Result<BTreeMap<String, String>> {
    let mut env = BTreeMap::new();
    for entry in entries {
        let Some((key, value)) = entry.split_once('=') else {
            return Err(MosaicError::Validation(format!(
                "invalid --env value '{entry}', expected KEY=VALUE"
            )));
        };
        let key = key.trim();
        if key.is_empty() {
            return Err(MosaicError::Validation(
                "environment key cannot be empty".to_string(),
            ));
        }
        if key
            .chars()
            .any(|ch| !(ch.is_ascii_alphanumeric() || ch == '_'))
        {
            return Err(MosaicError::Validation(format!(
                "invalid environment key '{key}'"
            )));
        }
        env.insert(key.to_string(), value.to_string());
    }
    Ok(env)
}
