use std::collections::BTreeMap;
use std::path::PathBuf;

use serde_json::{Value, json};

use mosaic_core::error::{MosaicError, Result};
use mosaic_core::privacy::validate_value_for_state_persistence;
use mosaic_mcp::{
    AddMcpServerInput, McpDiagnoseOptions, McpServerDiagnoseResult, McpStore, UpdateMcpServerInput,
    mcp_servers_file_path,
};

use super::{Cli, McpArgs, McpCommand, print_json, resolve_state_paths, save_json_file};

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
                        "- {} ({}) command={} enabled={} cwd={} env_refs={}",
                        server.id,
                        server.name,
                        server.command,
                        server.enabled,
                        server.cwd.unwrap_or_else(|| "-".to_string()),
                        server.env_from.len()
                    );
                }
            }
        }
        McpCommand::Add {
            name,
            command,
            args,
            env,
            env_from,
            cwd,
            disabled,
        } => {
            let created = store.add(AddMcpServerInput {
                id: None,
                name,
                command,
                args,
                env: parse_env_entries(env)?,
                env_from: parse_env_from_entries(env_from)?,
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
        McpCommand::Update {
            server_id,
            name,
            command,
            args,
            clear_args,
            env,
            clear_env,
            env_from,
            clear_env_from,
            cwd,
            clear_cwd,
            enable,
            disable,
        } => {
            let result = store.update(
                &server_id,
                UpdateMcpServerInput {
                    name,
                    command,
                    args: (!args.is_empty()).then_some(args),
                    clear_args,
                    env: if env.is_empty() {
                        None
                    } else {
                        Some(parse_env_entries(env)?)
                    },
                    clear_env,
                    env_from: if env_from.is_empty() {
                        None
                    } else {
                        Some(parse_env_from_entries(env_from)?)
                    },
                    clear_env_from,
                    cwd,
                    clear_cwd,
                    enabled: if enable {
                        Some(true)
                    } else if disable {
                        Some(false)
                    } else {
                        None
                    },
                },
            )?;
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "changed": result.changed,
                    "server": result.server,
                    "path": store.path().display().to_string(),
                }));
            } else {
                println!(
                    "updated mcp server {} changed={}",
                    result.server.id, result.changed
                );
            }
        }
        McpCommand::Show { server_id } => handle_show(cli, &store, &server_id)?,
        McpCommand::Check {
            server_id,
            all,
            deep,
            timeout_ms,
            report_out,
        } => handle_check(cli, &store, server_id, all, deep, timeout_ms, report_out)?,
        McpCommand::Diagnose {
            server_id,
            timeout_ms,
            report_out,
        } => handle_diagnose(cli, &store, &server_id, timeout_ms, report_out)?,
        McpCommand::Repair {
            server_id,
            all,
            timeout_ms,
            clear_missing_cwd,
            set_env_from,
            report_out,
        } => handle_repair(
            cli,
            &store,
            server_id,
            all,
            timeout_ms,
            clear_missing_cwd,
            parse_env_from_entries(set_env_from)?,
            report_out,
        )?,
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

fn handle_repair(
    cli: &Cli,
    store: &McpStore,
    server_id: Option<String>,
    all: bool,
    timeout_ms: u64,
    clear_missing_cwd: bool,
    set_env_from: BTreeMap<String, String>,
    report_out: Option<String>,
) -> Result<()> {
    validate_timeout_ms(timeout_ms)?;
    if !all && server_id.is_none() {
        return Err(MosaicError::Validation(
            "mcp repair requires <server_id> or --all".to_string(),
        ));
    }
    let report_out_path = report_out.map(PathBuf::from);

    let targets = if all {
        store.diagnose_all(McpDiagnoseOptions { timeout_ms })?
    } else {
        let server_id = server_id.expect("validated above");
        vec![store.diagnose(&server_id, McpDiagnoseOptions { timeout_ms })?]
    };

    let mut changed = 0usize;
    let mut results: Vec<Value> = Vec::with_capacity(targets.len());
    for target in targets {
        let server_id = target.server.id.clone();
        let mut actions = Vec::new();
        let before_env_from = target.server.env_from.clone();

        if has_disabled_issue(&target.check.issues) {
            store.set_enabled(&server_id, true)?;
            actions.push("enabled_server".to_string());
        }
        if clear_missing_cwd && has_missing_cwd_issue(&target.check.issues) {
            store.set_cwd(&server_id, None)?;
            actions.push("cleared_missing_cwd".to_string());
        }
        if !set_env_from.is_empty() {
            let updated = store.merge_env_from(&server_id, &set_env_from)?;
            if updated.env_from != before_env_from {
                actions.push("updated_env_from".to_string());
            }
        }

        let after = store.diagnose(&server_id, McpDiagnoseOptions { timeout_ms })?;
        let changed_entry = !actions.is_empty();
        if changed_entry {
            changed = changed.saturating_add(1);
        }

        results.push(json!({
            "server_id": server_id,
            "changed": changed_entry,
            "actions": actions,
            "before": {
                "healthy": target.healthy,
                "check_healthy": target.check.healthy,
                "protocol_handshake_ok": target.protocol_probe.handshake_ok,
                "issues": target.check.issues,
                "env_from": before_env_from,
            },
            "after": {
                "healthy": after.healthy,
                "check_healthy": after.check.healthy,
                "protocol_handshake_ok": after.protocol_probe.handshake_ok,
                "issues": after.check.issues,
                "env_from": after.server.env_from.clone(),
            },
            "remaining_recommendations": recommend_actions(&after),
        }));
    }

    let payload = json!({
        "ok": true,
        "all": all,
        "timeout_ms": timeout_ms,
        "clear_missing_cwd": clear_missing_cwd,
        "set_env_from": set_env_from,
        "checked": results.len(),
        "changed": changed,
        "unchanged": results.len().saturating_sub(changed),
        "results": results,
        "report_out": report_out_path.as_ref().map(|path| path.display().to_string()),
    });
    if let Some(path) = report_out_path.as_ref() {
        save_json_file(path, &payload)?;
    }

    if cli.json {
        print_json(&payload);
        return Ok(());
    }

    println!(
        "checked={} changed={} unchanged={}",
        payload["checked"].as_u64().unwrap_or(0),
        payload["changed"].as_u64().unwrap_or(0),
        payload["unchanged"].as_u64().unwrap_or(0),
    );
    if let Some(path) = report_out_path.as_ref() {
        println!("report: {}", path.display());
    }
    Ok(())
}

fn handle_diagnose(
    cli: &Cli,
    store: &McpStore,
    server_id: &str,
    timeout_ms: u64,
    report_out: Option<String>,
) -> Result<()> {
    validate_timeout_ms(timeout_ms)?;

    let result = store.diagnose(server_id, McpDiagnoseOptions { timeout_ms })?;
    let recommendations = recommend_actions(&result);
    let report_out_path = report_out.map(PathBuf::from);
    let payload = json!({
        "ok": true,
        "server": result.server,
        "check": result.check,
        "protocol_probe": result.protocol_probe,
        "healthy": result.healthy,
        "recommendations": recommendations,
        "timeout_ms": timeout_ms,
        "report_out": report_out_path.as_ref().map(|path| path.display().to_string()),
    });
    if let Some(path) = report_out_path.as_ref() {
        save_json_file(path, &payload)?;
    }
    if cli.json {
        print_json(&payload);
        return Ok(());
    }

    println!(
        "server: {}",
        payload["server"]["id"].as_str().unwrap_or("-")
    );
    println!("healthy: {}", payload["healthy"].as_bool().unwrap_or(false));
    println!(
        "check_healthy: {}",
        payload["check"]["healthy"].as_bool().unwrap_or(false)
    );
    if payload["check"]["issues"]
        .as_array()
        .is_none_or(|issues| issues.is_empty())
    {
        println!("check_issues: <none>");
    } else {
        println!("check_issues:");
        for issue in payload["check"]["issues"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|entry| entry.as_str())
        {
            println!("- {issue}");
        }
    }

    println!(
        "protocol_probe_attempted: {}",
        payload["protocol_probe"]["attempted"]
            .as_bool()
            .unwrap_or(false)
    );
    println!(
        "protocol_probe_timeout_ms: {}",
        payload["protocol_probe"]["timeout_ms"]
            .as_u64()
            .unwrap_or(0)
    );
    println!(
        "protocol_probe_duration_ms: {}",
        payload["protocol_probe"]["duration_ms"]
            .as_u64()
            .unwrap_or(0)
    );
    println!(
        "protocol_probe_handshake_ok: {}",
        payload["protocol_probe"]["handshake_ok"]
            .as_bool()
            .unwrap_or(false)
    );
    println!(
        "protocol_probe_response_kind: {}",
        payload["protocol_probe"]["response_kind"]
            .as_str()
            .unwrap_or("-")
    );
    println!(
        "protocol_probe_error: {}",
        payload["protocol_probe"]["error"].as_str().unwrap_or("-")
    );
    println!(
        "protocol_probe_response_preview: {}",
        payload["protocol_probe"]["response_preview"]
            .as_str()
            .unwrap_or("-")
    );
    println!(
        "protocol_probe_stderr_preview: {}",
        payload["protocol_probe"]["stderr_preview"]
            .as_str()
            .unwrap_or("-")
    );

    if payload["recommendations"]
        .as_array()
        .is_none_or(|items| items.is_empty())
    {
        println!("recommendations: <none>");
    } else {
        println!("recommendations:");
        for recommendation in payload["recommendations"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|entry| entry.as_str())
        {
            println!("- {recommendation}");
        }
    }
    if let Some(path) = report_out_path.as_ref() {
        println!("report_out: {}", path.display());
    }
    Ok(())
}

fn recommend_actions(result: &McpServerDiagnoseResult) -> Vec<String> {
    let mut actions = Vec::new();
    if result
        .check
        .issues
        .iter()
        .any(|issue| issue.contains("disabled"))
    {
        actions.push(format!(
            "enable server with `mosaic mcp enable {}`",
            result.server.id
        ));
    }
    if result
        .check
        .issues
        .iter()
        .any(|issue| issue.contains("not found in PATH"))
    {
        actions.push(
            "set --command to an absolute executable path or ensure command is in PATH".to_string(),
        );
    }
    if result
        .check
        .issues
        .iter()
        .any(|issue| issue.contains("cwd"))
    {
        actions.push("set a valid --cwd path for the MCP server".to_string());
    }
    for env_ref in result
        .check
        .env_refs
        .iter()
        .filter(|env_ref| !env_ref.present)
    {
        actions.push(format!(
            "export {} before launching Mosaic or run `mosaic mcp repair {} --set-env-from {}=<ENV_NAME>`",
            env_ref.source, result.server.id, env_ref.key
        ));
    }
    if result.protocol_probe.attempted && !result.protocol_probe.handshake_ok {
        actions.push(
            "verify server supports MCP stdio initialize handshake and check server startup logs"
                .to_string(),
        );
        if result
            .protocol_probe
            .error
            .as_deref()
            .unwrap_or_default()
            .contains("timed out")
        {
            actions.push(
                "increase timeout via `mosaic mcp diagnose <id> --timeout-ms <ms>`".to_string(),
            );
        }
    }
    actions
}

fn has_disabled_issue(issues: &[String]) -> bool {
    issues.iter().any(|issue| issue.contains("disabled"))
}

fn has_missing_cwd_issue(issues: &[String]) -> bool {
    issues
        .iter()
        .any(|issue| issue.contains("cwd") && issue.contains("does not exist"))
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
    if server.env_from.is_empty() {
        println!("env_from: <none>");
    } else {
        println!("env_from:");
        for (key, source) in server.env_from {
            println!("- {key} <- {source}");
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

fn handle_check(
    cli: &Cli,
    store: &McpStore,
    server_id: Option<String>,
    all: bool,
    deep: bool,
    timeout_ms: u64,
    report_out: Option<String>,
) -> Result<()> {
    if deep {
        validate_timeout_ms(timeout_ms)?;
    }
    let report_out_path = report_out.map(PathBuf::from);

    if all || server_id.is_none() {
        if deep {
            let results = store.diagnose_all(McpDiagnoseOptions { timeout_ms })?;
            let healthy_count = results.iter().filter(|item| item.healthy).count();
            let protocol_ok = results
                .iter()
                .filter(|item| item.protocol_probe.handshake_ok)
                .count();
            let protocol_failed = results
                .iter()
                .filter(|item| item.protocol_probe.attempted && !item.protocol_probe.handshake_ok)
                .count();
            let probe_skipped = results
                .iter()
                .filter(|item| !item.protocol_probe.attempted)
                .count();
            let precheck_unhealthy = results.iter().filter(|item| !item.check.healthy).count();
            let payload = json!({
                "ok": true,
                "all": true,
                "deep": true,
                "timeout_ms": timeout_ms,
                "checked": results.len(),
                "healthy": healthy_count,
                "unhealthy": results.len().saturating_sub(healthy_count),
                "precheck_unhealthy": precheck_unhealthy,
                "protocol_ok": protocol_ok,
                "protocol_failed": protocol_failed,
                "probe_skipped": probe_skipped,
                "results": results,
                "report_out": report_out_path.as_ref().map(|path| path.display().to_string()),
            });
            if let Some(path) = report_out_path.as_ref() {
                save_json_file(path, &payload)?;
            }
            if cli.json {
                print_json(&payload);
            } else if payload["checked"].as_u64().unwrap_or(0) == 0 {
                println!("No MCP servers configured.");
            } else {
                println!(
                    "checked={} healthy={} unhealthy={} protocol_ok={} protocol_failed={} probe_skipped={}",
                    payload["checked"].as_u64().unwrap_or(0),
                    payload["healthy"].as_u64().unwrap_or(0),
                    payload["unhealthy"].as_u64().unwrap_or(0),
                    payload["protocol_ok"].as_u64().unwrap_or(0),
                    payload["protocol_failed"].as_u64().unwrap_or(0),
                    payload["probe_skipped"].as_u64().unwrap_or(0),
                );
                if let Some(path) = report_out_path.as_ref() {
                    println!("report: {}", path.display());
                }
            }
            return Ok(());
        }

        let checks = store.check_all()?;
        let healthy_count = checks.iter().filter(|item| item.check.healthy).count();
        let payload = json!({
            "ok": true,
            "all": true,
            "deep": false,
            "checked": checks.len(),
            "healthy": healthy_count,
            "unhealthy": checks.len().saturating_sub(healthy_count),
            "results": checks,
            "report_out": report_out_path.as_ref().map(|path| path.display().to_string()),
        });
        if let Some(path) = report_out_path.as_ref() {
            save_json_file(path, &payload)?;
        }
        if cli.json {
            print_json(&payload);
        } else if payload["checked"].as_u64().unwrap_or(0) == 0 {
            println!("No MCP servers configured.");
        } else {
            println!(
                "checked={} healthy={} unhealthy={}",
                payload["checked"].as_u64().unwrap_or(0),
                payload["healthy"].as_u64().unwrap_or(0),
                payload["unhealthy"].as_u64().unwrap_or(0)
            );
            if let Some(path) = report_out_path.as_ref() {
                println!("report: {}", path.display());
            }
        }
        return Ok(());
    }

    let server_id = server_id.expect("checked above");
    if deep {
        let result = store.diagnose(&server_id, McpDiagnoseOptions { timeout_ms })?;
        let recommendations = recommend_actions(&result);
        let payload = json!({
            "ok": true,
            "all": false,
            "deep": true,
            "timeout_ms": timeout_ms,
            "healthy": result.healthy,
            "server": result.server,
            "check": result.check,
            "protocol_probe": result.protocol_probe,
            "recommendations": recommendations,
            "report_out": report_out_path.as_ref().map(|path| path.display().to_string()),
        });
        if let Some(path) = report_out_path.as_ref() {
            save_json_file(path, &payload)?;
        }
        if cli.json {
            print_json(&payload);
        } else {
            println!(
                "server: {}",
                payload["server"]["id"].as_str().unwrap_or("-")
            );
            println!("healthy: {}", payload["healthy"].as_bool().unwrap_or(false));
            println!(
                "protocol_probe: attempted={} handshake_ok={} kind={} error={}",
                payload["protocol_probe"]["attempted"]
                    .as_bool()
                    .unwrap_or(false),
                payload["protocol_probe"]["handshake_ok"]
                    .as_bool()
                    .unwrap_or(false),
                payload["protocol_probe"]["response_kind"]
                    .as_str()
                    .unwrap_or("-"),
                payload["protocol_probe"]["error"].as_str().unwrap_or("-"),
            );
            if let Some(path) = report_out_path.as_ref() {
                println!("report: {}", path.display());
            }
        }
        return Ok(());
    }

    let result = store.check(&server_id)?;
    let payload = json!({
        "ok": true,
        "all": false,
        "deep": false,
        "healthy": result.check.healthy,
        "server": result.server,
        "check": result.check,
        "report_out": report_out_path.as_ref().map(|path| path.display().to_string()),
    });
    if let Some(path) = report_out_path.as_ref() {
        save_json_file(path, &payload)?;
    }
    if cli.json {
        print_json(&payload);
    } else {
        println!(
            "server: {}",
            payload["server"]["id"].as_str().unwrap_or(&server_id)
        );
        println!("healthy: {}", payload["healthy"].as_bool().unwrap_or(false));
        if let Some(path) = payload["check"]["executable_resolved"].as_str() {
            println!("executable: {}", path);
        }
        let issues = payload["check"]["issues"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        if issues.is_empty() {
            println!("issues: <none>");
        } else {
            println!("issues:");
            for issue in issues {
                println!("- {}", issue.as_str().unwrap_or("-"));
            }
        }
        if let Some(path) = report_out_path.as_ref() {
            println!("report: {}", path.display());
        }
    }
    Ok(())
}

fn validate_timeout_ms(timeout_ms: u64) -> Result<()> {
    if timeout_ms == 0 {
        return Err(MosaicError::Validation(
            "timeout_ms must be greater than 0".to_string(),
        ));
    }
    if timeout_ms > 120_000 {
        return Err(MosaicError::Validation(
            "timeout_ms must be less than or equal to 120000".to_string(),
        ));
    }
    Ok(())
}

fn parse_env_entries(entries: Vec<String>) -> Result<BTreeMap<String, String>> {
    let mut env = BTreeMap::new();
    for entry in entries {
        let (key, value) = parse_env_mapping_entry(&entry, "--env", "KEY=VALUE")?;
        env.insert(key.to_string(), value.to_string());
    }
    let encoded = serde_json::to_value(&env).map_err(|err| {
        MosaicError::Validation(format!("failed to encode mcp env entries: {err}"))
    })?;
    if validate_value_for_state_persistence(&encoded, "mcp server env state").is_err() {
        return Err(MosaicError::Validation(
            "mcp add --env cannot persist secret-like literal values; inject secrets in the process environment that launches Mosaic and keep only non-sensitive env pairs in the registry".to_string(),
        ));
    }
    Ok(env)
}

fn parse_env_from_entries(entries: Vec<String>) -> Result<BTreeMap<String, String>> {
    let mut env_from = BTreeMap::new();
    for entry in entries {
        let (key, source) = parse_env_mapping_entry(&entry, "--env-from", "KEY=ENV_NAME")?;
        validate_env_token(source, "environment source")?;
        env_from.insert(key.to_string(), source.to_string());
    }
    Ok(env_from)
}

fn parse_env_mapping_entry<'a>(
    entry: &'a str,
    flag: &str,
    format_hint: &str,
) -> Result<(&'a str, &'a str)> {
    let Some((key, value)) = entry.split_once('=') else {
        return Err(MosaicError::Validation(format!(
            "invalid {flag} value '{entry}', expected {format_hint}"
        )));
    };
    let key = key.trim();
    validate_env_token(key, "environment key")?;
    Ok((key, value.trim()))
}

fn validate_env_token(value: &str, label: &str) -> Result<()> {
    if value.is_empty() {
        return Err(MosaicError::Validation(format!("{label} cannot be empty")));
    }
    if value
        .chars()
        .any(|ch| !(ch.is_ascii_alphanumeric() || ch == '_'))
    {
        return Err(MosaicError::Validation(format!(
            "invalid {label} '{value}'"
        )));
    }
    Ok(())
}
