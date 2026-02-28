use std::cmp::Ordering;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::time::Duration;

use mosaic_core::error::{MosaicError, Result};
use mosaic_core::state::{StateMode, StatePaths};
use reqwest::StatusCode;
use serde_json::{Value, json};

use super::runtime_context::resolve_state_paths;
use super::utils::print_json;
use super::{Cli, UpdateArgs};

const UPDATE_SOURCE_ENV: &str = "MOSAIC_UPDATE_SOURCE";
const UPDATE_LATEST_ENV: &str = "MOSAIC_UPDATE_LATEST";

#[derive(Debug, Clone)]
struct StateCleanupSummary {
    removed_dirs: Vec<String>,
    removed_files: Vec<String>,
}

pub(super) async fn handle_update(cli: &Cli, args: UpdateArgs) -> Result<()> {
    let current_version = env!("CARGO_PKG_VERSION").to_string();

    if !args.check {
        if cli.json {
            print_json(&json!({
                "ok": true,
                "checked": false,
                "current_version": current_version,
                "update_available": false,
            }));
        } else {
            println!("current version: {current_version}");
            println!("update check: skipped (pass --check to query latest)");
        }
        return Ok(());
    }

    let source = resolve_update_source(args.source.clone())?;
    let latest_version = fetch_latest_version(&source, args.timeout_ms).await?;
    let update_available = is_update_available(&current_version, &latest_version);

    if cli.json {
        print_json(&json!({
            "ok": true,
            "checked": true,
            "source": source,
            "current_version": current_version,
            "latest_version": latest_version,
            "update_available": update_available,
            "up_to_date": !update_available,
        }));
    } else {
        println!("current version: {current_version}");
        println!("latest version: {latest_version}");
        println!("source: {source}");
        if update_available {
            println!("update available: yes");
        } else {
            println!("update available: no");
        }
    }
    Ok(())
}

pub(super) fn handle_reset(cli: &Cli) -> Result<()> {
    require_yes(cli, "reset")?;
    let paths = resolve_state_paths(cli.project_state)?;
    let summary = reset_state(&paths)?;

    if cli.json {
        print_json(&json!({
            "ok": true,
            "mode": mode_name(paths.mode),
            "removed_dirs": summary.removed_dirs,
            "removed_files": summary.removed_files,
            "state_root": paths.root_dir.display().to_string(),
            "reinitialized": true,
        }));
    } else {
        println!("state reset complete (mode: {})", mode_name(paths.mode));
        println!("removed directories: {}", summary.removed_dirs.len());
        println!("removed files: {}", summary.removed_files.len());
        println!("state root: {}", paths.root_dir.display());
    }
    Ok(())
}

pub(super) fn handle_uninstall(cli: &Cli) -> Result<()> {
    require_yes(cli, "uninstall")?;
    let paths = resolve_state_paths(cli.project_state)?;
    let targets = uninstall_targets(&paths)?;
    let mut removed = Vec::new();
    for target in targets {
        if target.exists() {
            std::fs::remove_dir_all(&target)?;
            removed.push(target.display().to_string());
        }
    }

    if cli.json {
        print_json(&json!({
            "ok": true,
            "mode": mode_name(paths.mode),
            "removed_dirs": removed,
            "note": "binary uninstall is manual (cargo uninstall mosaic-cli)",
        }));
    } else {
        println!("uninstall complete (mode: {})", mode_name(paths.mode));
        if removed.is_empty() {
            println!("no state directories found");
        } else {
            for path in &removed {
                println!("removed: {path}");
            }
        }
        println!("binary uninstall (optional): cargo uninstall mosaic-cli");
    }
    Ok(())
}

fn mode_name(mode: StateMode) -> &'static str {
    match mode {
        StateMode::Xdg => "xdg",
        StateMode::Project => "project",
    }
}

fn require_yes(cli: &Cli, action: &str) -> Result<()> {
    if cli.yes {
        return Ok(());
    }
    Err(MosaicError::ApprovalRequired(format!(
        "{action} is destructive; rerun with --yes to confirm"
    )))
}

fn resolve_update_source(arg_source: Option<String>) -> Result<String> {
    if let Some(source) = arg_source {
        let value = source.trim().to_string();
        if value.is_empty() {
            return Err(MosaicError::Validation(
                "--source cannot be empty".to_string(),
            ));
        }
        return Ok(value);
    }
    if let Some(source) = std::env::var_os(UPDATE_SOURCE_ENV) {
        let value = source.to_string_lossy().trim().to_string();
        if !value.is_empty() {
            return Ok(value);
        }
    }
    Err(MosaicError::Validation(format!(
        "update source is not configured. pass --source or set {UPDATE_SOURCE_ENV}"
    )))
}

async fn fetch_latest_version(source: &str, timeout_ms: u64) -> Result<String> {
    if source.starts_with("mock://") {
        return parse_latest_version(source.trim_start_matches("mock://"));
    }
    if let Some(value) = std::env::var_os(UPDATE_LATEST_ENV) {
        let latest = value.to_string_lossy().trim().to_string();
        if !latest.is_empty() {
            return Ok(latest);
        }
    }

    let client = reqwest::Client::builder()
        .timeout(Duration::from_millis(timeout_ms))
        .build()
        .map_err(|err| MosaicError::Network(format!("failed to build update client: {err}")))?;
    let response = client
        .get(source)
        .header("accept", "application/json, text/plain")
        .header(
            "user-agent",
            format!("mosaic-cli/{}", env!("CARGO_PKG_VERSION")),
        )
        .send()
        .await
        .map_err(|err| MosaicError::Network(format!("update request failed: {err}")))?;
    let status = response.status();
    let body = response
        .text()
        .await
        .map_err(|err| MosaicError::Network(format!("failed to read update response: {err}")))?;
    if status != StatusCode::OK {
        return Err(MosaicError::Network(format!(
            "update request failed ({}): {}",
            status,
            truncate_for_error(&body)
        )));
    }
    parse_latest_version(&body)
}

fn parse_latest_version(raw: &str) -> Result<String> {
    let text = raw.trim();
    if text.is_empty() {
        return Err(MosaicError::Validation(
            "latest version payload is empty".to_string(),
        ));
    }

    if let Ok(value) = serde_json::from_str::<Value>(text)
        && let Some(result) = latest_from_json(&value)
    {
        return Ok(result);
    }

    Ok(text.to_string())
}

fn latest_from_json(value: &Value) -> Option<String> {
    match value {
        Value::String(v) => Some(v.trim().to_string()),
        Value::Object(map) => {
            for key in ["latest", "version", "tag_name"] {
                let Some(Value::String(v)) = map.get(key) else {
                    continue;
                };
                let latest = v.trim();
                if !latest.is_empty() {
                    return Some(latest.to_string());
                }
            }
            None
        }
        _ => None,
    }
}

fn truncate_for_error(text: &str) -> String {
    let trimmed = text.trim();
    if trimmed.chars().count() <= 200 {
        trimmed.to_string()
    } else {
        let mut out = trimmed.chars().take(200).collect::<String>();
        out.push_str("...");
        out
    }
}

fn normalize_version(value: &str) -> String {
    value.trim().trim_start_matches('v').to_string()
}

fn is_update_available(current: &str, latest: &str) -> bool {
    match (
        parse_numeric_version(current),
        parse_numeric_version(latest),
    ) {
        (Some(current_parts), Some(latest_parts)) => {
            compare_numeric_versions(&latest_parts, &current_parts) == Ordering::Greater
        }
        _ => normalize_version(current) != normalize_version(latest),
    }
}

fn parse_numeric_version(value: &str) -> Option<Vec<u64>> {
    let normalized = normalize_version(value);
    if normalized.is_empty() {
        return None;
    }
    let mut parts = Vec::new();
    for chunk in normalized.split('.') {
        let digits = chunk
            .chars()
            .take_while(|ch| ch.is_ascii_digit())
            .collect::<String>();
        if digits.is_empty() {
            return None;
        }
        let parsed = digits.parse::<u64>().ok()?;
        parts.push(parsed);
    }
    if parts.is_empty() { None } else { Some(parts) }
}

fn compare_numeric_versions(left: &[u64], right: &[u64]) -> Ordering {
    let len = left.len().max(right.len());
    for idx in 0..len {
        let lhs = left.get(idx).copied().unwrap_or(0);
        let rhs = right.get(idx).copied().unwrap_or(0);
        match lhs.cmp(&rhs) {
            Ordering::Equal => continue,
            order => return order,
        }
    }
    Ordering::Equal
}

fn reset_state(paths: &StatePaths) -> Result<StateCleanupSummary> {
    let mut removed_dirs = Vec::new();
    let mut removed_files = Vec::new();

    let files = [
        &paths.config_path,
        &paths.models_path,
        &paths.system_events_path,
        &paths.audit_log_path,
        &paths.approvals_policy_path,
        &paths.sandbox_policy_path,
    ];
    for file in files {
        if file.exists() {
            std::fs::remove_file(file)?;
            removed_files.push(file.display().to_string());
        }
    }

    let dirs = [
        &paths.sessions_dir,
        &paths.audit_dir,
        &paths.policy_dir,
        &paths.data_dir,
    ];
    for dir in dirs {
        if dir.exists() {
            std::fs::remove_dir_all(dir)?;
            removed_dirs.push(dir.display().to_string());
        }
    }

    paths.ensure_dirs()?;
    Ok(StateCleanupSummary {
        removed_dirs,
        removed_files,
    })
}

fn uninstall_targets(paths: &StatePaths) -> Result<Vec<PathBuf>> {
    let mut targets = BTreeSet::new();
    match paths.mode {
        StateMode::Project => {
            ensure_safe_uninstall_path(paths.mode, &paths.root_dir)?;
            targets.insert(paths.root_dir.clone());
        }
        StateMode::Xdg => {
            ensure_safe_uninstall_path(paths.mode, &paths.root_dir)?;
            ensure_safe_uninstall_path(paths.mode, &paths.data_dir)?;
            targets.insert(paths.root_dir.clone());
            targets.insert(paths.data_dir.clone());
        }
    }
    Ok(targets.into_iter().collect())
}

fn ensure_safe_uninstall_path(mode: StateMode, path: &Path) -> Result<()> {
    let name = path
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| {
            MosaicError::Validation(format!(
                "refusing to uninstall invalid state path: {}",
                path.display()
            ))
        })?;
    let safe = match mode {
        StateMode::Project => name == ".mosaic",
        StateMode::Xdg => name == "mosaic",
    };
    if safe {
        Ok(())
    } else {
        Err(MosaicError::Validation(format!(
            "refusing to uninstall non-mosaic path: {}",
            path.display()
        )))
    }
}
