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
        McpCommand::Check { server_id } => {
            let result = store.check(&server_id)?;
            if cli.json {
                print_json(&json!({
                    "ok": true,
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
        }
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
