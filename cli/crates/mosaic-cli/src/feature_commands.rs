use std::collections::BTreeSet;
use std::io::Read;
#[cfg(unix)]
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::Command as ProcessCommand;
use std::process::Stdio;
use std::thread;
use std::time::{Duration, Instant};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::json;

use mosaic_core::error::MosaicError;
use mosaic_memory::{MemoryIndexOptions, MemoryStore, memory_index_path, memory_status_path};
use mosaic_ops::{
    ApprovalDecision, ApprovalStore, SandboxProfile, SandboxStore, evaluate_approval,
    evaluate_sandbox,
};
use mosaic_plugins::{ExtensionRegistry, ExtensionSource, PluginEntry, RegistryRoots};

use super::{
    BrowserArgs, BrowserCommand, Cli, ExtensionSourceFilterArg, MemoryArgs, MemoryCommand,
    PluginHookArg, PluginsArgs, PluginsCommand, Result, SkillsArgs, SkillsCommand,
    browser_history_file_path, browser_open_visit, browser_state_file_path,
    load_browser_history_or_default, load_browser_state_or_default, print_json,
    resolve_state_paths, save_browser_history, save_browser_state,
};

const PLUGIN_STATE_VERSION: u32 = 1;
const DEFAULT_PLUGIN_HOOK_TIMEOUT_MS: u64 = 15_000;
const DEFAULT_PLUGIN_MAX_OUTPUT_BYTES: u64 = 262_144;
const MAX_PLUGIN_MAX_OUTPUT_BYTES: u64 = 16 * 1024 * 1024;
const PLUGIN_OUTPUT_READ_CHUNK_BYTES: usize = 8 * 1024;
const MAX_PLUGIN_OUTPUT_PREVIEW_CHARS: usize = 240;
const PLUGIN_CPU_WATCHDOG_MULTIPLIER: u64 = 4;
const PLUGIN_CPU_WATCHDOG_MIN_BUDGET_MS: u64 = 200;
#[cfg(any(
    target_os = "linux",
    target_os = "android",
    target_os = "freebsd",
    target_os = "dragonfly",
    target_os = "netbsd",
    target_os = "openbsd"
))]
const MIN_PLUGIN_MEMORY_RLIMIT_BYTES: u64 = 16 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PluginStateFile {
    version: u32,
    disabled_plugins: Vec<String>,
}

impl Default for PluginStateFile {
    fn default() -> Self {
        Self {
            version: PLUGIN_STATE_VERSION,
            disabled_plugins: Vec::new(),
        }
    }
}

impl PluginStateFile {
    fn is_enabled(&self, plugin_id: &str) -> bool {
        !self.disabled_plugins.iter().any(|item| item == plugin_id)
    }

    fn set_enabled(&mut self, plugin_id: &str, enabled: bool) -> bool {
        let normalized = plugin_id.trim();
        if normalized.is_empty() {
            return false;
        }
        let mut disabled = self
            .disabled_plugins
            .iter()
            .map(|item| item.trim().to_string())
            .filter(|item| !item.is_empty())
            .collect::<BTreeSet<_>>();
        let changed = if enabled {
            disabled.remove(normalized)
        } else {
            disabled.insert(normalized.to_string())
        };
        self.disabled_plugins = disabled.into_iter().collect();
        changed
    }
}

#[derive(Debug, Clone, Serialize)]
struct PluginView {
    #[serde(flatten)]
    plugin: PluginEntry,
    enabled: bool,
}

pub(super) async fn handle_browser(cli: &Cli, args: BrowserArgs) -> Result<()> {
    let paths = resolve_state_paths(cli.project_state)?;
    paths.ensure_dirs()?;
    let history_path = browser_history_file_path(&paths.data_dir);
    let state_path = browser_state_file_path(&paths.data_dir);
    let mut history = load_browser_history_or_default(&history_path)?;
    let mut state = load_browser_state_or_default(&state_path)?;
    match args.command {
        BrowserCommand::Start => {
            if !state.running {
                state.running = true;
                state.started_at = Some(Utc::now());
            }
            save_browser_state(&state_path, &state)?;
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "state": state,
                    "path": state_path.display().to_string(),
                }));
            } else {
                println!("browser runtime: running");
                if let Some(started_at) = state.started_at {
                    println!("started_at: {}", started_at.to_rfc3339());
                }
            }
        }
        BrowserCommand::Stop => {
            state.running = false;
            state.stopped_at = Some(Utc::now());
            save_browser_state(&state_path, &state)?;
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "state": state,
                    "path": state_path.display().to_string(),
                }));
            } else {
                println!("browser runtime: stopped");
                if let Some(stopped_at) = state.stopped_at {
                    println!("stopped_at: {}", stopped_at.to_rfc3339());
                }
            }
        }
        BrowserCommand::Status => {
            let latest_visit = history.iter().max_by(|lhs, rhs| lhs.ts.cmp(&rhs.ts));
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "state": state,
                    "active_visit_id": state.active_visit_id,
                    "total_visits": history.len(),
                    "latest_visit": latest_visit,
                    "state_path": state_path.display().to_string(),
                    "history_path": history_path.display().to_string(),
                }));
            } else {
                println!(
                    "browser runtime: {}",
                    if state.running { "running" } else { "stopped" }
                );
                println!("total visits: {}", history.len());
                println!(
                    "active visit: {}",
                    state.active_visit_id.as_deref().unwrap_or("-")
                );
                if let Some(visit) = latest_visit {
                    println!("latest visit: {} {}", visit.id, visit.url);
                }
            }
        }
        BrowserCommand::Open { url, timeout_ms } | BrowserCommand::Navigate { url, timeout_ms } => {
            if timeout_ms == 0 {
                return Err(MosaicError::Validation(
                    "--timeout-ms must be greater than 0".to_string(),
                ));
            }
            let visit = browser_open_visit(&url, timeout_ms).await?;
            history.push(visit.clone());
            state.active_visit_id = Some(visit.id.clone());
            save_browser_history(&history_path, &history)?;
            save_browser_state(&state_path, &state)?;
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "visit": visit,
                    "path": history_path.display().to_string(),
                    "active_visit_id": state.active_visit_id,
                }));
            } else {
                let status = visit
                    .http_status
                    .map(|value| value.to_string())
                    .unwrap_or_else(|| "-".to_string());
                println!("browser open: {}", visit.url);
                println!("visit id: {}", visit.id);
                println!("ok: {}", visit.ok);
                println!("status: {status}");
                if let Some(title) = visit.title {
                    println!("title: {title}");
                }
                if let Some(error) = visit.error {
                    println!("error: {error}");
                }
            }
        }
        BrowserCommand::History { tail } | BrowserCommand::Tabs { tail } => {
            let visits = sorted_tail_visits(history, tail);
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "visits": visits,
                    "active_visit_id": state.active_visit_id,
                    "path": history_path.display().to_string(),
                }));
            } else if visits.is_empty() {
                println!("No browser history.");
            } else {
                for visit in visits {
                    let status = visit
                        .http_status
                        .map(|value| value.to_string())
                        .unwrap_or_else(|| "-".to_string());
                    println!(
                        "{} [{}] {} status={} title={}",
                        visit.id,
                        if visit.ok { "ok" } else { "error" },
                        visit.url,
                        status,
                        visit.title.unwrap_or_else(|| "-".to_string())
                    );
                }
            }
        }
        BrowserCommand::Show { visit_id } => {
            let visit = find_visit_by_id(&history, &visit_id)?;
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "visit": visit,
                }));
            } else {
                println!("visit: {}", visit.id);
                println!("ts: {}", visit.ts.to_rfc3339());
                println!("url: {}", visit.url);
                println!("ok: {}", visit.ok);
                if let Some(status) = visit.http_status {
                    println!("status: {status}");
                }
                if let Some(content_type) = visit.content_type {
                    println!("content_type: {content_type}");
                }
                if let Some(content_length) = visit.content_length {
                    println!("content_length: {content_length}");
                }
                if let Some(title) = visit.title {
                    println!("title: {title}");
                }
                if let Some(preview) = visit.preview {
                    println!("preview: {preview}");
                }
                if let Some(error) = visit.error {
                    println!("error: {error}");
                }
            }
        }
        BrowserCommand::Focus { visit_id } => {
            let visit = find_visit_by_id(&history, &visit_id)?;
            state.active_visit_id = Some(visit.id.clone());
            save_browser_state(&state_path, &state)?;
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "active_visit_id": state.active_visit_id,
                    "visit": visit,
                    "path": state_path.display().to_string(),
                }));
            } else {
                println!("active visit set: {}", visit.id);
                println!("url: {}", visit.url);
            }
        }
        BrowserCommand::Snapshot { visit_id } => {
            let visit = resolve_snapshot_visit(
                &history,
                visit_id.as_deref(),
                state.active_visit_id.as_deref(),
            )?;
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "snapshot": {
                        "visit_id": visit.id,
                        "ts": visit.ts,
                        "url": visit.url,
                        "ok": visit.ok,
                        "http_status": visit.http_status,
                        "title": visit.title,
                        "preview": visit.preview,
                    }
                }));
            } else {
                println!("visit: {}", visit.id);
                println!("url: {}", visit.url);
                println!("ok: {}", visit.ok);
                if let Some(status) = visit.http_status {
                    println!("status: {status}");
                }
                println!("title: {}", visit.title.unwrap_or_else(|| "-".to_string()));
                println!(
                    "preview: {}",
                    visit.preview.unwrap_or_else(|| "-".to_string())
                );
            }
        }
        BrowserCommand::Screenshot { visit_id, out } => {
            let visit = resolve_snapshot_visit(
                &history,
                visit_id.as_deref(),
                state.active_visit_id.as_deref(),
            )?;
            let cwd = std::env::current_dir().map_err(|err| MosaicError::Io(err.to_string()))?;
            let output_path = match out {
                Some(path) => {
                    let candidate = PathBuf::from(path);
                    if candidate.is_absolute() {
                        candidate
                    } else {
                        cwd.join(candidate)
                    }
                }
                None => paths
                    .data_dir
                    .join("browser-screenshots")
                    .join(format!("{}.txt", visit.id)),
            };
            if let Some(parent) = output_path.parent() {
                std::fs::create_dir_all(parent).map_err(|err| {
                    MosaicError::Io(format!(
                        "failed to create screenshot directory '{}': {err}",
                        parent.display()
                    ))
                })?;
            }
            let title = visit.title.unwrap_or_else(|| "-".to_string());
            let preview = visit.preview.unwrap_or_else(|| "-".to_string());
            let status = visit
                .http_status
                .map(|value| value.to_string())
                .unwrap_or_else(|| "-".to_string());
            let payload = format!(
                "MOSAIC_BROWSER_SCREENSHOT_V1\nvisit_id={}\nts={}\nurl={}\nok={}\nstatus={}\ntitle={}\npreview={}\n",
                visit.id,
                visit.ts.to_rfc3339(),
                visit.url,
                visit.ok,
                status,
                title,
                preview
            );
            std::fs::write(&output_path, payload.as_bytes()).map_err(|err| {
                MosaicError::Io(format!(
                    "failed to write screenshot artifact '{}': {err}",
                    output_path.display()
                ))
            })?;
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "visit_id": visit.id,
                    "output": output_path.display().to_string(),
                    "bytes": payload.len(),
                }));
            } else {
                println!("visit: {}", visit.id);
                println!("output: {}", output_path.display());
            }
        }
        BrowserCommand::Close { visit_id, all } | BrowserCommand::Clear { visit_id, all } => {
            if all && visit_id.is_some() {
                return Err(MosaicError::Validation(
                    "cannot set visit_id and --all together".to_string(),
                ));
            }
            if !all && visit_id.is_none() {
                return Err(MosaicError::Validation(
                    "specify visit_id or use --all".to_string(),
                ));
            }

            let (removed, remaining) = if all {
                let removed = history.len();
                (removed, Vec::new())
            } else {
                let target = visit_id.expect("validated visit_id");
                let before = history.len();
                let remaining = history
                    .into_iter()
                    .filter(|item| item.id != target)
                    .collect::<Vec<_>>();
                (before.saturating_sub(remaining.len()), remaining)
            };
            if removed > 0 || all {
                save_browser_history(&history_path, &remaining)?;
            }
            if all {
                state.active_visit_id = None;
                save_browser_state(&state_path, &state)?;
            } else if removed > 0 {
                let current_active = state.active_visit_id.clone();
                if let Some(active_id) = current_active
                    && !remaining.iter().any(|visit| visit.id == active_id)
                {
                    state.active_visit_id = remaining.last().map(|visit| visit.id.clone());
                    save_browser_state(&state_path, &state)?;
                }
            }
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "removed": removed,
                    "remaining": remaining.len(),
                    "path": history_path.display().to_string(),
                    "active_visit_id": state.active_visit_id,
                }));
            } else {
                println!("removed visits: {removed}");
                println!("remaining visits: {}", remaining.len());
            }
        }
    }
    Ok(())
}

fn sorted_tail_visits(
    mut visits: Vec<super::BrowserVisitRecord>,
    tail: usize,
) -> Vec<super::BrowserVisitRecord> {
    visits.sort_by(|lhs, rhs| lhs.ts.cmp(&rhs.ts));
    if visits.len() > tail {
        let keep_from = visits.len() - tail;
        visits.split_off(keep_from)
    } else {
        visits
    }
}

fn find_visit_by_id(
    history: &[super::BrowserVisitRecord],
    visit_id: &str,
) -> Result<super::BrowserVisitRecord> {
    history
        .iter()
        .find(|item| item.id == visit_id)
        .cloned()
        .ok_or_else(|| MosaicError::Validation(format!("visit '{}' not found", visit_id)))
}

fn resolve_snapshot_visit(
    history: &[super::BrowserVisitRecord],
    visit_id: Option<&str>,
    active_visit_id: Option<&str>,
) -> Result<super::BrowserVisitRecord> {
    if history.is_empty() {
        return Err(MosaicError::Validation(
            "no browser visits available; run `mosaic browser open --url <...>` first".to_string(),
        ));
    }
    if let Some(visit_id) = visit_id {
        return find_visit_by_id(history, visit_id);
    }
    if let Some(active) = active_visit_id
        && let Ok(found) = find_visit_by_id(history, active)
    {
        return Ok(found);
    }
    history
        .iter()
        .max_by(|lhs, rhs| lhs.ts.cmp(&rhs.ts))
        .cloned()
        .ok_or_else(|| MosaicError::Validation("no browser visits available".to_string()))
}

pub(super) fn handle_memory(cli: &Cli, args: MemoryArgs) -> Result<()> {
    let paths = resolve_state_paths(cli.project_state)?;
    paths.ensure_dirs()?;
    let store = MemoryStore::new(
        memory_index_path(&paths.data_dir),
        memory_status_path(&paths.data_dir),
    );

    match args.command {
        MemoryCommand::Index {
            path,
            max_files,
            max_file_size,
            max_content_bytes,
        } => {
            let cwd = std::env::current_dir().map_err(|err| MosaicError::Io(err.to_string()))?;
            let root = {
                let raw = PathBuf::from(path);
                if raw.is_absolute() {
                    raw
                } else {
                    cwd.join(raw)
                }
            };
            let result = store.index(MemoryIndexOptions {
                root,
                max_files,
                max_file_size,
                max_content_bytes,
            })?;
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "index": result,
                }));
            } else {
                println!("memory indexed documents: {}", result.indexed_documents);
                println!("memory skipped files: {}", result.skipped_files);
                println!("index path: {}", result.index_path);
            }
        }
        MemoryCommand::Search { query, limit } => {
            let result = store.search(&query, Some(limit))?;
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "result": result,
                }));
            } else if result.hits.is_empty() {
                println!("No memory hits.");
            } else {
                println!(
                    "memory search hits: {} (showing {})",
                    result.total_hits,
                    result.hits.len()
                );
                for hit in result.hits {
                    println!("{} score={} {}", hit.path, hit.score, hit.snippet);
                }
            }
        }
        MemoryCommand::Status => {
            let status = store.status()?;
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "status": status,
                }));
            } else {
                println!("indexed documents: {}", status.indexed_documents);
                println!(
                    "last indexed at: {}",
                    status
                        .last_indexed_at
                        .map(|value| value.to_rfc3339())
                        .unwrap_or_else(|| "-".to_string())
                );
                println!("index path: {}", status.index_path);
            }
        }
        MemoryCommand::Clear => {
            let cleared = store.clear()?;
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "cleared": cleared,
                }));
            } else {
                println!(
                    "memory clear: index_removed={} status_removed={}",
                    cleared.removed_index, cleared.removed_status
                );
                println!("index path: {}", cleared.index_path);
                println!("status path: {}", cleared.status_path);
            }
        }
    }
    Ok(())
}

pub(super) fn handle_plugins(cli: &Cli, args: PluginsArgs) -> Result<()> {
    let paths = resolve_state_paths(cli.project_state)?;
    paths.ensure_dirs()?;
    let registry = ExtensionRegistry::new(RegistryRoots::from_state_root(paths.root_dir.clone()));
    let plugin_state_path = paths.data_dir.join("plugins-state.json");
    let mut plugin_state = load_plugin_state(&plugin_state_path)?;

    match args.command {
        PluginsCommand::List { source } => {
            let requested_source = parse_extension_source_filter(source);
            let plugins = registry
                .list_plugins()?
                .into_iter()
                .filter(|entry| source_matches(requested_source, entry.source))
                .collect::<Vec<_>>();
            let plugin_views = plugin_views_with_state(plugins, &plugin_state);
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "source_filter": extension_source_filter_name(source),
                    "count": plugin_views.len(),
                    "plugins": plugin_views,
                }));
            } else if plugin_views.is_empty() {
                println!("No plugins found.");
            } else {
                println!("plugins: {}", plugin_views.len());
                println!("source filter: {}", extension_source_filter_name(source));
                for plugin in plugin_views {
                    println!(
                        "- {} ({}) source={:?} enabled={} version={} manifest_valid={}",
                        plugin.plugin.id,
                        plugin.plugin.name,
                        plugin.plugin.source,
                        plugin.enabled,
                        plugin.plugin.version.unwrap_or_else(|| "-".to_string()),
                        plugin.plugin.manifest_valid
                    );
                }
            }
        }
        PluginsCommand::Info { plugin_id } => {
            let plugin = registry.plugin_info(&plugin_id)?;
            let plugin_view = PluginView {
                enabled: plugin_state.is_enabled(&plugin.id),
                plugin,
            };
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "plugin": plugin_view,
                }));
            } else {
                println!("id: {}", plugin_view.plugin.id);
                println!("name: {}", plugin_view.plugin.name);
                println!("source: {:?}", plugin_view.plugin.source);
                println!("enabled: {}", plugin_view.enabled);
                println!(
                    "version: {}",
                    plugin_view
                        .plugin
                        .version
                        .unwrap_or_else(|| "-".to_string())
                );
                println!(
                    "description: {}",
                    plugin_view
                        .plugin
                        .description
                        .unwrap_or_else(|| "-".to_string())
                );
                println!("path: {}", plugin_view.plugin.path);
                println!("manifest path: {}", plugin_view.plugin.manifest_path);
                println!("manifest valid: {}", plugin_view.plugin.manifest_valid);
                if let Some(error) = plugin_view.plugin.manifest_error {
                    println!("manifest error: {error}");
                }
            }
        }
        PluginsCommand::Check { plugin_id } => {
            let report = registry.check_plugins(plugin_id.as_deref())?;
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "report": report,
                }));
            } else {
                println!(
                    "plugin checks: checked={} failed={} ok={}",
                    report.checked, report.failed, report.ok
                );
                for result in report.results {
                    println!(
                        "- {} source={:?} ok={}",
                        result.id, result.source, result.ok
                    );
                    for check in result.checks {
                        let status = if check.ok { "OK" } else { "WARN" };
                        println!("  [{status}] {}: {}", check.name, check.detail);
                    }
                }
            }
        }
        PluginsCommand::Install { path, force } => {
            let cwd = std::env::current_dir().map_err(|err| MosaicError::Io(err.to_string()))?;
            let source = {
                let value = PathBuf::from(path);
                if value.is_absolute() {
                    value
                } else {
                    cwd.join(value)
                }
            };
            let outcome = registry.install_plugin_from_path(&source, force)?;
            let state_changed = plugin_state.set_enabled(&outcome.id, true);
            if state_changed {
                save_plugin_state(&plugin_state_path, &plugin_state)?;
            }
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "installed": outcome,
                    "enabled": true,
                    "state_changed": state_changed,
                    "state_path": plugin_state_path.display().to_string(),
                }));
            } else {
                println!(
                    "Installed plugin {} -> {}",
                    outcome.id, outcome.installed_path
                );
                if outcome.replaced {
                    println!("replaced existing plugin package");
                }
                if state_changed {
                    println!("plugin enabled in state: {}", plugin_state_path.display());
                }
            }
        }
        PluginsCommand::Enable { plugin_id } => {
            let plugin = registry.plugin_info(&plugin_id)?;
            let changed = plugin_state.set_enabled(&plugin.id, true);
            if changed {
                save_plugin_state(&plugin_state_path, &plugin_state)?;
            }
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "plugin_id": plugin.id,
                    "enabled": true,
                    "changed": changed,
                    "state_path": plugin_state_path.display().to_string(),
                }));
            } else {
                println!("plugin: {}", plugin.id);
                println!("enabled: true");
                println!("changed: {changed}");
                println!("state: {}", plugin_state_path.display());
            }
        }
        PluginsCommand::Disable { plugin_id } => {
            let plugin = registry.plugin_info(&plugin_id)?;
            let changed = plugin_state.set_enabled(&plugin.id, false);
            if changed {
                save_plugin_state(&plugin_state_path, &plugin_state)?;
            }
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "plugin_id": plugin.id,
                    "enabled": false,
                    "changed": changed,
                    "state_path": plugin_state_path.display().to_string(),
                }));
            } else {
                println!("plugin: {}", plugin.id);
                println!("enabled: false");
                println!("changed: {changed}");
                println!("state: {}", plugin_state_path.display());
            }
        }
        PluginsCommand::Doctor => {
            let plugins = registry.list_plugins()?;
            let report = registry.check_plugins(None)?;
            let enabled_plugins_list = plugins
                .iter()
                .filter(|plugin| plugin_state.is_enabled(&plugin.id))
                .cloned()
                .collect::<Vec<_>>();
            let disabled_plugins = plugins
                .iter()
                .filter(|plugin| !plugin_state.is_enabled(&plugin.id))
                .map(|plugin| plugin.id.clone())
                .collect::<Vec<_>>();
            let enabled_plugins = plugins.len().saturating_sub(disabled_plugins.len());
            let runtime_missing_run_hooks = enabled_plugins_list
                .iter()
                .filter_map(|plugin| missing_runtime_hook(plugin, PluginRuntimeHook::Run))
                .collect::<Vec<_>>();
            let runtime_missing_doctor_hooks = enabled_plugins_list
                .iter()
                .filter_map(|plugin| missing_runtime_hook(plugin, PluginRuntimeHook::Doctor))
                .collect::<Vec<_>>();
            let runtime_missing_run_count = runtime_missing_run_hooks.len();
            let runtime_missing_doctor_count = runtime_missing_doctor_hooks.len();
            let installed_plugin_ids = plugins
                .into_iter()
                .map(|plugin| plugin.id)
                .collect::<BTreeSet<_>>();
            let stale_disabled_ids = plugin_state
                .disabled_plugins
                .iter()
                .filter(|plugin_id| !installed_plugin_ids.contains(*plugin_id))
                .cloned()
                .collect::<Vec<_>>();

            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "doctor": {
                        "plugins_total": report.checked,
                        "enabled_plugins": enabled_plugins,
                        "disabled_plugins": disabled_plugins.len(),
                        "disabled_plugin_ids": disabled_plugins,
                        "stale_disabled_ids": stale_disabled_ids,
                        "runtime_missing_run_hooks": runtime_missing_run_hooks,
                        "runtime_missing_doctor_hooks": runtime_missing_doctor_hooks,
                        "runtime_runnable_plugins": enabled_plugins_list
                            .len()
                            .saturating_sub(runtime_missing_run_count),
                        "state_path": plugin_state_path.display().to_string(),
                        "health_ok": report.ok
                            && stale_disabled_ids.is_empty()
                            && runtime_missing_run_count == 0,
                    },
                    "report": report,
                }));
            } else {
                println!("plugins total: {}", report.checked);
                println!("enabled plugins: {enabled_plugins}");
                println!("disabled plugins: {}", disabled_plugins.len());
                println!("state path: {}", plugin_state_path.display());
                if !disabled_plugins.is_empty() {
                    println!("disabled ids: {}", disabled_plugins.join(", "));
                }
                if !stale_disabled_ids.is_empty() {
                    println!("stale disabled ids: {}", stale_disabled_ids.join(", "));
                }
                if !runtime_missing_run_hooks.is_empty() {
                    println!(
                        "runtime missing run hooks: {}",
                        runtime_missing_run_hooks.join(", ")
                    );
                }
                if !runtime_missing_doctor_hooks.is_empty() {
                    println!(
                        "runtime missing doctor hooks: {}",
                        runtime_missing_doctor_hooks.join(", ")
                    );
                }
                println!(
                    "runtime missing run hooks count: {}",
                    runtime_missing_run_count
                );
                println!(
                    "runtime missing doctor hooks count: {}",
                    runtime_missing_doctor_count
                );
                println!(
                    "doctor health: {}",
                    if report.ok && stale_disabled_ids.is_empty() && runtime_missing_run_count == 0
                    {
                        "ok"
                    } else {
                        "warn"
                    }
                );
            }
        }
        PluginsCommand::Run {
            plugin_id,
            hook,
            timeout_ms,
            args,
        } => {
            let plugin = registry.plugin_info(&plugin_id)?;
            if !plugin_state.is_enabled(&plugin.id) {
                return Err(MosaicError::Validation(format!(
                    "plugin '{}' is disabled. run `mosaic plugins enable {}` first",
                    plugin.id, plugin.id
                )));
            }
            let runtime_hook = PluginRuntimeHook::from_cli(hook);
            let hook_path = resolve_plugin_hook_path(&plugin, runtime_hook)?;
            let hook_display = runtime_hook.as_str();
            let resolved_timeout_ms = resolve_plugin_timeout_ms(&plugin, timeout_ms)?;
            let max_output_bytes = resolve_plugin_max_output_bytes(&plugin)?;
            let resource_limits = resolve_plugin_resource_limits(&plugin)?;
            let cpu_watchdog_ms = resolve_plugin_cpu_watchdog_ms(&plugin)?;
            validate_plugin_resource_limits_platform(&plugin, resource_limits)?;
            let sandbox_store = SandboxStore::new(paths.sandbox_policy_path.clone());
            let default_profile = sandbox_store.load_or_default()?.profile;
            let sandbox_profile = resolve_plugin_sandbox_profile(&plugin, default_profile)?;
            validate_plugin_hook_sandbox(&plugin, &hook_path, sandbox_profile)?;
            let command = build_plugin_hook_command(&hook_path, &args)?;
            if let Some(reason) = evaluate_sandbox(&command.rendered, sandbox_profile) {
                return Err(MosaicError::SandboxDenied(reason));
            }
            let approval_store = ApprovalStore::new(paths.approvals_policy_path.clone());
            let approval_policy = approval_store.load_or_default()?;
            let approved_by = match evaluate_approval(&command.rendered, &approval_policy) {
                ApprovalDecision::Auto { approved_by } => approved_by,
                ApprovalDecision::NeedsConfirmation { reason } => {
                    if cli.yes {
                        "flag_yes".to_string()
                    } else {
                        return Err(MosaicError::ApprovalRequired(format!(
                            "{reason}. rerun with --yes"
                        )));
                    }
                }
                ApprovalDecision::Deny { reason } => {
                    return Err(MosaicError::ApprovalRequired(reason));
                }
            };

            let started = Instant::now();
            let execution = run_plugin_hook(
                &plugin,
                command,
                resolved_timeout_ms,
                max_output_bytes,
                resource_limits,
                cpu_watchdog_ms,
            )?;
            let duration_ms = started.elapsed().as_millis();
            let exit_code = execution.exit_code;
            let evaluated_resource_limit_error = if execution.resource_limit_error.is_none() {
                evaluate_plugin_resource_limits(
                    &plugin,
                    resource_limits,
                    execution.resources.as_ref(),
                )?
            } else {
                None
            };
            let resource_limit_error = execution
                .resource_limit_error
                .clone()
                .or(evaluated_resource_limit_error);
            let run_ok = exit_code == 0 && !execution.timed_out && resource_limit_error.is_none();
            let now = Utc::now();
            let event_log_path = append_plugin_event(
                &paths.data_dir,
                &plugin.id,
                &PluginRuntimeEvent {
                    ts: now,
                    plugin_id: plugin.id.clone(),
                    hook: hook_display.to_string(),
                    hook_path: hook_path.display().to_string(),
                    command: execution.command.program.clone(),
                    args: execution.command.args.clone(),
                    timeout_ms: resolved_timeout_ms,
                    output_limit_bytes: execution.output_limit_bytes,
                    timed_out: execution.timed_out,
                    sandbox_profile: sandbox_profile_label(sandbox_profile).to_string(),
                    approved_by: approved_by.clone(),
                    resource_limits: resource_limits.as_option(),
                    resource_metrics: execution.resources.clone(),
                    resource_limit_error: resource_limit_error.clone(),
                    resource_rlimits_applied: execution.resource_rlimits_applied,
                    stdout_bytes: execution.stdout.bytes,
                    stderr_bytes: execution.stderr.bytes,
                    stdout_truncated: execution.stdout.truncated,
                    stderr_truncated: execution.stderr.truncated,
                    exit_code,
                    duration_ms,
                    ok: run_ok,
                    stdout_preview: text_preview(
                        &execution.stdout.text,
                        MAX_PLUGIN_OUTPUT_PREVIEW_CHARS,
                    ),
                    stderr_preview: text_preview(
                        &execution.stderr.text,
                        MAX_PLUGIN_OUTPUT_PREVIEW_CHARS,
                    ),
                    error: if execution.timed_out {
                        Some(format!(
                            "execution timed out after {}ms",
                            resolved_timeout_ms
                        ))
                    } else if let Some(reason) = resource_limit_error.clone() {
                        Some(reason)
                    } else if exit_code != 0 {
                        Some(format!("hook exited with code {exit_code}"))
                    } else {
                        None
                    },
                },
            )?;
            if cli.json && run_ok {
                print_json(&json!({
                    "ok": true,
                    "plugin_id": plugin.id,
                    "hook": hook_display,
                    "hook_path": hook_path.display().to_string(),
                    "command": {
                        "program": execution.command.program.clone(),
                        "args": execution.command.args.clone(),
                        "rendered": execution.command.rendered.clone(),
                    },
                    "timeout_ms": resolved_timeout_ms,
                    "output_limit_bytes": execution.output_limit_bytes,
                    "timed_out": execution.timed_out,
                    "sandbox_profile": sandbox_profile_label(sandbox_profile),
                    "approved_by": approved_by,
                    "resource_limits": resource_limits.as_option(),
                    "resource_metrics": execution.resources.clone(),
                    "resource_rlimits_applied": execution.resource_rlimits_applied,
                    "stdout_bytes": execution.stdout.bytes,
                    "stderr_bytes": execution.stderr.bytes,
                    "stdout_truncated": execution.stdout.truncated,
                    "stderr_truncated": execution.stderr.truncated,
                    "exit_code": exit_code,
                    "duration_ms": duration_ms,
                    "stdout": execution.stdout.text.clone(),
                    "stderr": execution.stderr.text.clone(),
                    "event_log_path": event_log_path.display().to_string(),
                }));
            } else if !cli.json {
                println!("plugin: {}", plugin.id);
                println!("hook: {hook_display}");
                println!("hook_path: {}", hook_path.display());
                println!("timeout_ms: {resolved_timeout_ms}");
                println!(
                    "sandbox_profile: {}",
                    sandbox_profile_label(sandbox_profile)
                );
                println!("approved_by: {approved_by}");
                println!("timed_out: {}", execution.timed_out);
                println!("exit_code: {exit_code}");
                println!("duration_ms: {duration_ms}");
                println!("output_limit_bytes: {}", execution.output_limit_bytes);
                println!(
                    "resource_rlimits_applied: {}",
                    execution.resource_rlimits_applied
                );
                println!(
                    "stdout_bytes: {} (truncated={})",
                    execution.stdout.bytes, execution.stdout.truncated
                );
                println!(
                    "stderr_bytes: {} (truncated={})",
                    execution.stderr.bytes, execution.stderr.truncated
                );
                if let Some(limits) = resource_limits.as_option() {
                    if let Some(max_cpu_ms) = limits.max_cpu_ms {
                        println!("limit.max_cpu_ms: {max_cpu_ms}");
                    }
                    if let Some(max_rss_kb) = limits.max_rss_kb {
                        println!("limit.max_rss_kb: {max_rss_kb}");
                    }
                }
                if let Some(metrics) = execution.resources.as_ref() {
                    if let Some(cpu_user_ms) = metrics.cpu_user_ms {
                        println!("metrics.cpu_user_ms: {cpu_user_ms}");
                    }
                    if let Some(cpu_system_ms) = metrics.cpu_system_ms {
                        println!("metrics.cpu_system_ms: {cpu_system_ms}");
                    }
                    if let Some(cpu_total_ms) = metrics.cpu_total_ms {
                        println!("metrics.cpu_total_ms: {cpu_total_ms}");
                    }
                    if let Some(max_rss_kb) = metrics.max_rss_kb {
                        println!("metrics.max_rss_kb: {max_rss_kb}");
                    }
                }
                if let Some(reason) = resource_limit_error.as_ref() {
                    println!("resource_limit_error: {reason}");
                }
                println!("event_log: {}", event_log_path.display());
                if !execution.stdout.text.trim().is_empty() {
                    println!("stdout:");
                    print!("{}", execution.stdout.text);
                }
                if !execution.stderr.text.trim().is_empty() {
                    println!("stderr:");
                    eprint!("{}", execution.stderr.text);
                }
            }
            if execution.timed_out {
                return Err(MosaicError::Tool(format!(
                    "plugin '{}' hook '{}' timed out after {}ms",
                    plugin.id, hook_display, resolved_timeout_ms
                )));
            }
            if let Some(reason) = resource_limit_error {
                return Err(MosaicError::Tool(reason));
            }
            if exit_code != 0 {
                return Err(MosaicError::Tool(format!(
                    "plugin '{}' hook '{}' failed with exit code {}",
                    plugin.id, hook_display, exit_code
                )));
            }
        }
        PluginsCommand::Remove { plugin_id } => {
            let removed = registry.remove_project_plugin(&plugin_id)?;
            let state_changed = if removed {
                let changed = plugin_state.set_enabled(&plugin_id, true);
                if changed {
                    save_plugin_state(&plugin_state_path, &plugin_state)?;
                }
                changed
            } else {
                false
            };
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "removed": removed,
                    "plugin_id": plugin_id,
                    "state_changed": state_changed,
                }));
            } else if removed {
                println!("Removed plugin {plugin_id}");
                if state_changed {
                    println!("cleared plugin state entry");
                }
            } else {
                println!("Plugin {plugin_id} not found in project scope.");
            }
        }
    }
    Ok(())
}

pub(super) fn handle_skills(cli: &Cli, args: SkillsArgs) -> Result<()> {
    let paths = resolve_state_paths(cli.project_state)?;
    paths.ensure_dirs()?;
    let registry = ExtensionRegistry::new(RegistryRoots::from_state_root(paths.root_dir.clone()));

    match args.command {
        SkillsCommand::List { source } => {
            let requested_source = parse_extension_source_filter(source);
            let skills = registry
                .list_skills()?
                .into_iter()
                .filter(|entry| source_matches(requested_source, entry.source))
                .collect::<Vec<_>>();
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "source_filter": extension_source_filter_name(source),
                    "count": skills.len(),
                    "skills": skills,
                }));
            } else if skills.is_empty() {
                println!("No skills found.");
            } else {
                println!("skills: {}", skills.len());
                println!("source filter: {}", extension_source_filter_name(source));
                for skill in skills {
                    println!("- {} ({}) source={:?}", skill.id, skill.title, skill.source);
                }
            }
        }
        SkillsCommand::Info { skill_id } => {
            let skill = registry.skill_info(&skill_id)?;
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "skill": skill,
                }));
            } else {
                println!("id: {}", skill.id);
                println!("title: {}", skill.title);
                println!(
                    "description: {}",
                    skill.description.unwrap_or_else(|| "-".to_string())
                );
                println!("source: {:?}", skill.source);
                println!("path: {}", skill.path);
                println!("skill file: {}", skill.skill_file);
            }
        }
        SkillsCommand::Check { skill_id } => {
            let report = registry.check_skills(skill_id.as_deref())?;
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "report": report,
                }));
            } else {
                println!(
                    "skill checks: checked={} failed={} ok={}",
                    report.checked, report.failed, report.ok
                );
                for result in report.results {
                    println!(
                        "- {} source={:?} ok={}",
                        result.id, result.source, result.ok
                    );
                    for check in result.checks {
                        let status = if check.ok { "OK" } else { "WARN" };
                        println!("  [{status}] {}: {}", check.name, check.detail);
                    }
                }
            }
        }
        SkillsCommand::Install { path, force } => {
            let cwd = std::env::current_dir().map_err(|err| MosaicError::Io(err.to_string()))?;
            let source = {
                let value = PathBuf::from(path);
                if value.is_absolute() {
                    value
                } else {
                    cwd.join(value)
                }
            };
            let outcome = registry.install_skill_from_path(&source, force)?;
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "installed": outcome,
                }));
            } else {
                println!(
                    "Installed skill {} -> {}",
                    outcome.id, outcome.installed_path
                );
                if outcome.replaced {
                    println!("replaced existing skill package");
                }
            }
        }
        SkillsCommand::Remove { skill_id } => {
            let removed = registry.remove_project_skill(&skill_id)?;
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "removed": removed,
                    "skill_id": skill_id,
                }));
            } else if removed {
                println!("Removed skill {skill_id}");
            } else {
                println!("Skill {skill_id} not found in project scope.");
            }
        }
    }
    Ok(())
}

fn parse_extension_source_filter(source: ExtensionSourceFilterArg) -> Option<ExtensionSource> {
    match source {
        ExtensionSourceFilterArg::All => None,
        ExtensionSourceFilterArg::Project => Some(ExtensionSource::Project),
        ExtensionSourceFilterArg::CodexHome => Some(ExtensionSource::CodexHome),
        ExtensionSourceFilterArg::UserHome => Some(ExtensionSource::UserHome),
    }
}

fn extension_source_filter_name(source: ExtensionSourceFilterArg) -> &'static str {
    match source {
        ExtensionSourceFilterArg::All => "all",
        ExtensionSourceFilterArg::Project => "project",
        ExtensionSourceFilterArg::CodexHome => "codex_home",
        ExtensionSourceFilterArg::UserHome => "user_home",
    }
}

fn source_matches(requested: Option<ExtensionSource>, actual: ExtensionSource) -> bool {
    match requested {
        Some(expected) => expected == actual,
        None => true,
    }
}

fn plugin_views_with_state(
    plugins: Vec<PluginEntry>,
    plugin_state: &PluginStateFile,
) -> Vec<PluginView> {
    plugins
        .into_iter()
        .map(|plugin| PluginView {
            enabled: plugin_state.is_enabled(&plugin.id),
            plugin,
        })
        .collect()
}

fn load_plugin_state(path: &Path) -> Result<PluginStateFile> {
    if !path.exists() {
        return Ok(PluginStateFile::default());
    }
    let raw = std::fs::read_to_string(path).map_err(|err| {
        MosaicError::Io(format!(
            "failed to read plugin state file '{}': {err}",
            path.display()
        ))
    })?;
    let mut state: PluginStateFile = serde_json::from_str(&raw).map_err(|err| {
        MosaicError::Validation(format!(
            "failed to parse plugin state file '{}': {err}",
            path.display()
        ))
    })?;
    if state.version == 0 {
        state.version = PLUGIN_STATE_VERSION;
    }
    state.disabled_plugins = state
        .disabled_plugins
        .iter()
        .map(|item| item.trim().to_string())
        .filter(|item| !item.is_empty())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    Ok(state)
}

fn save_plugin_state(path: &Path, state: &PluginStateFile) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|err| {
            MosaicError::Io(format!(
                "failed to create plugin state directory '{}': {err}",
                parent.display()
            ))
        })?;
    }
    let rendered = serde_json::to_string_pretty(state)
        .map_err(|err| MosaicError::Io(format!("failed to serialize plugin state: {err}")))?;
    std::fs::write(path, rendered).map_err(|err| {
        MosaicError::Io(format!(
            "failed to write plugin state file '{}': {err}",
            path.display()
        ))
    })?;
    Ok(())
}

#[derive(Clone, Copy)]
enum PluginRuntimeHook {
    Run,
    Doctor,
}

impl PluginRuntimeHook {
    fn from_cli(value: PluginHookArg) -> Self {
        match value {
            PluginHookArg::Run => Self::Run,
            PluginHookArg::Doctor => Self::Doctor,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Run => "run",
            Self::Doctor => "doctor",
        }
    }
}

fn missing_runtime_hook(plugin: &PluginEntry, hook: PluginRuntimeHook) -> Option<String> {
    resolve_plugin_hook_path(plugin, hook)
        .err()
        .map(|_| plugin.id.clone())
}

fn resolve_plugin_hook_path(plugin: &PluginEntry, hook: PluginRuntimeHook) -> Result<PathBuf> {
    let plugin_root = PathBuf::from(&plugin.path);
    let manifest_candidate = plugin.runtime.as_ref().and_then(|runtime| match hook {
        PluginRuntimeHook::Run => runtime.run.as_deref(),
        PluginRuntimeHook::Doctor => runtime.doctor.as_deref(),
    });

    if let Some(path) = manifest_candidate {
        let candidate = plugin_root.join(path);
        if candidate.is_file() {
            return Ok(candidate);
        }
        return Err(MosaicError::Validation(format!(
            "plugin '{}' runtime {} hook not found at {}",
            plugin.id,
            hook.as_str(),
            candidate.display()
        )));
    }

    let candidates = [
        plugin_root.join("hooks").join(hook.as_str()),
        plugin_root
            .join("hooks")
            .join(format!("{}.sh", hook.as_str())),
        plugin_root
            .join("hooks")
            .join(format!("{}.py", hook.as_str())),
    ];
    for candidate in candidates {
        if candidate.is_file() {
            return Ok(candidate);
        }
    }
    Err(MosaicError::Validation(format!(
        "plugin '{}' has no {} hook. add [runtime].{} in plugin.toml or create hooks/{}.sh",
        plugin.id,
        hook.as_str(),
        hook.as_str(),
        hook.as_str()
    )))
}

#[derive(Debug, Clone)]
struct PluginHookCommand {
    program: String,
    args: Vec<String>,
    rendered: String,
}

struct PluginHookExecution {
    command: PluginHookCommand,
    exit_code: i32,
    stdout: CapturedOutput,
    stderr: CapturedOutput,
    timed_out: bool,
    output_limit_bytes: u64,
    resource_limit_error: Option<String>,
    resource_rlimits_applied: bool,
    resources: Option<PluginResourceMetrics>,
}

#[derive(Debug, Clone)]
struct CapturedOutput {
    text: String,
    bytes: u64,
    truncated: bool,
}

#[derive(Debug, Clone, Copy, Serialize)]
struct PluginResourceLimits {
    #[serde(skip_serializing_if = "Option::is_none")]
    max_cpu_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_rss_kb: Option<u64>,
}

impl PluginResourceLimits {
    fn is_configured(self) -> bool {
        self.max_cpu_ms.is_some() || self.max_rss_kb.is_some()
    }

    fn as_option(self) -> Option<Self> {
        if self.is_configured() {
            Some(self)
        } else {
            None
        }
    }
}

#[derive(Debug, Clone, Serialize)]
struct PluginResourceMetrics {
    #[serde(skip_serializing_if = "Option::is_none")]
    cpu_user_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cpu_system_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cpu_total_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_rss_kb: Option<u64>,
}

#[derive(Debug, Clone, Copy)]
struct ChildCpuUsage {
    user_micros: u128,
    system_micros: u128,
    max_rss_kb: u64,
}

#[derive(Debug, Clone, Serialize)]
struct PluginRuntimeEvent {
    ts: DateTime<Utc>,
    plugin_id: String,
    hook: String,
    hook_path: String,
    command: String,
    args: Vec<String>,
    timeout_ms: u64,
    output_limit_bytes: u64,
    timed_out: bool,
    sandbox_profile: String,
    approved_by: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    resource_limits: Option<PluginResourceLimits>,
    #[serde(skip_serializing_if = "Option::is_none")]
    resource_metrics: Option<PluginResourceMetrics>,
    #[serde(skip_serializing_if = "Option::is_none")]
    resource_limit_error: Option<String>,
    resource_rlimits_applied: bool,
    stdout_bytes: u64,
    stderr_bytes: u64,
    stdout_truncated: bool,
    stderr_truncated: bool,
    exit_code: i32,
    duration_ms: u128,
    ok: bool,
    stdout_preview: String,
    stderr_preview: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

fn build_plugin_hook_command(hook_path: &Path, args: &[String]) -> Result<PluginHookCommand> {
    let extension = hook_path
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| value.to_ascii_lowercase());

    let (program, mut command_args): (String, Vec<String>) = match extension.as_deref() {
        Some("sh") | Some("bash") => ("sh".to_string(), vec![hook_path.display().to_string()]),
        Some("py") => (
            std::env::var("PYTHON").unwrap_or_else(|_| "python3".to_string()),
            vec![hook_path.display().to_string()],
        ),
        _ => (hook_path.display().to_string(), Vec::new()),
    };
    command_args.extend(args.iter().cloned());
    let rendered = render_shell_command(&program, &command_args);

    Ok(PluginHookCommand {
        program,
        args: command_args,
        rendered,
    })
}

fn run_plugin_hook(
    plugin: &PluginEntry,
    command: PluginHookCommand,
    timeout_ms: u64,
    max_output_bytes: u64,
    resource_limits: PluginResourceLimits,
    cpu_watchdog_ms: Option<u64>,
) -> Result<PluginHookExecution> {
    if timeout_ms == 0 {
        return Err(MosaicError::Validation(
            "--timeout-ms must be greater than 0".to_string(),
        ));
    }
    if max_output_bytes == 0 {
        return Err(MosaicError::Validation(
            "plugin runtime max_output_bytes must be greater than 0".to_string(),
        ));
    }

    let stream_limit = usize::try_from(max_output_bytes).unwrap_or(usize::MAX);

    let mut process = ProcessCommand::new(&command.program);
    process
        .args(&command.args)
        .current_dir(&plugin.path)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let resource_rlimits_applied =
        apply_plugin_resource_rlimits(&mut process, resource_limits, &plugin.id)?;
    let mut child = process.spawn().map_err(|err| {
        MosaicError::Tool(format!(
            "failed to execute plugin '{}' hook command '{}': {err}",
            plugin.id, command.rendered
        ))
    })?;

    let stdout_reader = child
        .stdout
        .take()
        .ok_or_else(|| MosaicError::Io("failed to capture plugin hook stdout".to_string()))?;
    let stderr_reader = child
        .stderr
        .take()
        .ok_or_else(|| MosaicError::Io("failed to capture plugin hook stderr".to_string()))?;
    let stdout_handle = spawn_output_reader(stdout_reader, stream_limit);
    let stderr_handle = spawn_output_reader(stderr_reader, stream_limit);

    let timeout = Duration::from_millis(timeout_ms);
    let cpu_watchdog_budget_ms =
        resolve_plugin_cpu_watchdog_budget_ms(timeout_ms, resource_limits, cpu_watchdog_ms);
    let cpu_watchdog_budget = cpu_watchdog_budget_ms.map(Duration::from_millis);
    let poll_interval = Duration::from_millis(25);
    let started = Instant::now();
    let cpu_started = snapshot_child_cpu_usage();
    let mut timed_out = false;
    let mut resource_limit_error = None;
    let exit_status = loop {
        match child.try_wait().map_err(|err| {
            MosaicError::Tool(format!(
                "failed waiting on plugin '{}' hook process: {err}",
                plugin.id
            ))
        })? {
            Some(status) => break status,
            None => {
                if started.elapsed() >= timeout {
                    timed_out = true;
                    child.kill().map_err(|err| {
                        MosaicError::Tool(format!(
                            "failed to terminate timed-out plugin '{}' hook: {err}",
                            plugin.id
                        ))
                    })?;
                    let status = child.wait().map_err(|err| {
                        MosaicError::Tool(format!(
                            "failed to wait after killing plugin '{}' hook: {err}",
                            plugin.id
                        ))
                    })?;
                    break status;
                }
                if let Some(cpu_watchdog_budget) = cpu_watchdog_budget {
                    let elapsed = started.elapsed();
                    if elapsed >= cpu_watchdog_budget {
                        let elapsed_ms = elapsed.as_millis().min(u128::from(u64::MAX)) as u64;
                        child.kill().map_err(|err| {
                            MosaicError::Tool(format!(
                                "failed to terminate cpu-watchdog plugin '{}' hook: {err}",
                                plugin.id
                            ))
                        })?;
                        let status = child.wait().map_err(|err| {
                            MosaicError::Tool(format!(
                                "failed to wait after cpu-watchdog kill for plugin '{}' hook: {err}",
                                plugin.id
                            ))
                        })?;
                        let max_cpu_ms = resource_limits.max_cpu_ms.unwrap_or_default();
                        let budget_ms = cpu_watchdog_budget_ms.unwrap_or_default();
                        resource_limit_error = Some(format!(
                            "plugin '{}' hook resource watchdog exceeded: elapsed_wall_ms={}ms > budget_ms={} (derived from max_cpu_ms={}ms)",
                            plugin.id, elapsed_ms, budget_ms, max_cpu_ms
                        ));
                        break status;
                    }
                }
                thread::sleep(poll_interval);
            }
        }
    };

    let stdout = join_output_reader(stdout_handle, "stdout")?;
    let stderr = join_output_reader(stderr_handle, "stderr")?;
    let fallback_timeout_code = if timed_out { 124 } else { -1 };
    let resources = plugin_resource_metrics(cpu_started, snapshot_child_cpu_usage());

    Ok(PluginHookExecution {
        command,
        exit_code: exit_status.code().unwrap_or(fallback_timeout_code),
        stdout,
        stderr,
        timed_out,
        output_limit_bytes: max_output_bytes,
        resource_limit_error,
        resource_rlimits_applied,
        resources,
    })
}

fn plugin_resource_metrics(
    cpu_started: Option<ChildCpuUsage>,
    cpu_finished: Option<ChildCpuUsage>,
) -> Option<PluginResourceMetrics> {
    let usage = cpu_started.zip(cpu_finished).map(|(start, end)| {
        let user_ms = micros_to_ms(end.user_micros.saturating_sub(start.user_micros));
        let system_ms = micros_to_ms(end.system_micros.saturating_sub(start.system_micros));
        let max_rss_kb = end.max_rss_kb;
        (
            user_ms,
            system_ms,
            user_ms.saturating_add(system_ms),
            max_rss_kb,
        )
    });
    let cpu_user_ms = usage.map(|values| values.0);
    let cpu_system_ms = usage.map(|values| values.1);
    let cpu_total_ms = usage.map(|values| values.2);
    let max_rss_kb = usage.map(|values| values.3);
    if cpu_total_ms.is_none() && max_rss_kb.is_none() {
        return None;
    }
    Some(PluginResourceMetrics {
        cpu_user_ms,
        cpu_system_ms,
        cpu_total_ms,
        max_rss_kb,
    })
}

fn micros_to_ms(value: u128) -> u64 {
    let rounded = value.saturating_add(999) / 1_000;
    rounded.min(u64::MAX as u128) as u64
}

#[cfg(unix)]
fn snapshot_child_cpu_usage() -> Option<ChildCpuUsage> {
    let mut usage = std::mem::MaybeUninit::<libc::rusage>::uninit();
    let result = unsafe { libc::getrusage(libc::RUSAGE_CHILDREN, usage.as_mut_ptr()) };
    if result != 0 {
        return None;
    }
    let usage = unsafe { usage.assume_init() };
    Some(ChildCpuUsage {
        user_micros: timeval_to_micros(usage.ru_utime),
        system_micros: timeval_to_micros(usage.ru_stime),
        max_rss_kb: normalize_max_rss_kb(usage.ru_maxrss),
    })
}

#[cfg(not(unix))]
fn snapshot_child_cpu_usage() -> Option<ChildCpuUsage> {
    None
}

#[cfg(unix)]
fn timeval_to_micros(time: libc::timeval) -> u128 {
    let seconds = if time.tv_sec < 0 {
        0
    } else {
        time.tv_sec as u128
    };
    let micros = if time.tv_usec < 0 {
        0
    } else {
        time.tv_usec as u128
    };
    seconds.saturating_mul(1_000_000).saturating_add(micros)
}

#[cfg(unix)]
fn normalize_max_rss_kb(value: libc::c_long) -> u64 {
    let raw = u64::try_from(value).unwrap_or_default();
    #[cfg(target_os = "macos")]
    {
        raw.saturating_add(1023) / 1024
    }
    #[cfg(not(target_os = "macos"))]
    {
        raw
    }
}

fn spawn_output_reader<R>(
    mut reader: R,
    max_output_bytes: usize,
) -> thread::JoinHandle<CapturedOutput>
where
    R: Read + Send + 'static,
{
    thread::spawn(move || {
        let mut captured = Vec::new();
        let mut total_bytes = 0u64;
        let mut truncated = false;
        let mut chunk = [0u8; PLUGIN_OUTPUT_READ_CHUNK_BYTES];
        loop {
            match reader.read(&mut chunk) {
                Ok(0) => break,
                Ok(read_size) => {
                    total_bytes = total_bytes.saturating_add(read_size as u64);
                    if captured.len() < max_output_bytes {
                        let remaining = max_output_bytes.saturating_sub(captured.len());
                        let take = remaining.min(read_size);
                        captured.extend_from_slice(&chunk[..take]);
                        if take < read_size {
                            truncated = true;
                        }
                    } else {
                        truncated = true;
                    }
                }
                Err(_) => break,
            }
        }
        if total_bytes > max_output_bytes as u64 {
            truncated = true;
        }
        CapturedOutput {
            text: String::from_utf8_lossy(&captured).to_string(),
            bytes: total_bytes,
            truncated,
        }
    })
}

fn join_output_reader(
    handle: thread::JoinHandle<CapturedOutput>,
    stream_name: &str,
) -> Result<CapturedOutput> {
    handle.join().map_err(|_| {
        MosaicError::Tool(format!(
            "failed to collect plugin hook {stream_name} output thread"
        ))
    })
}

fn text_preview(text: &str, limit: usize) -> String {
    if text.chars().count() <= limit {
        return text.to_string();
    }
    let truncated = text.chars().take(limit).collect::<String>();
    format!("{truncated}...")
}

fn render_shell_command(program: &str, args: &[String]) -> String {
    let mut parts = Vec::with_capacity(args.len() + 1);
    parts.push(shell_escape(program));
    parts.extend(args.iter().map(|arg| shell_escape(arg)));
    parts.join(" ")
}

fn shell_escape(value: &str) -> String {
    if value
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.' | '/' | ':'))
    {
        value.to_string()
    } else {
        format!("'{}'", value.replace('\'', "'\"'\"'"))
    }
}

fn resolve_plugin_timeout_ms(plugin: &PluginEntry, cli_timeout_ms: Option<u64>) -> Result<u64> {
    let timeout_ms = cli_timeout_ms
        .or_else(|| {
            plugin
                .runtime
                .as_ref()
                .and_then(|runtime| runtime.timeout_ms)
        })
        .unwrap_or(DEFAULT_PLUGIN_HOOK_TIMEOUT_MS);
    if timeout_ms == 0 {
        return Err(MosaicError::Validation(
            "plugin runtime timeout_ms must be greater than 0".to_string(),
        ));
    }
    Ok(timeout_ms)
}

fn resolve_plugin_max_output_bytes(plugin: &PluginEntry) -> Result<u64> {
    let max_output_bytes = plugin
        .runtime
        .as_ref()
        .and_then(|runtime| runtime.max_output_bytes)
        .unwrap_or(DEFAULT_PLUGIN_MAX_OUTPUT_BYTES);
    if max_output_bytes == 0 {
        return Err(MosaicError::Validation(format!(
            "plugin '{}' runtime max_output_bytes must be greater than 0",
            plugin.id
        )));
    }
    if max_output_bytes > MAX_PLUGIN_MAX_OUTPUT_BYTES {
        return Err(MosaicError::Validation(format!(
            "plugin '{}' runtime max_output_bytes must be <= {}",
            plugin.id, MAX_PLUGIN_MAX_OUTPUT_BYTES
        )));
    }
    Ok(max_output_bytes)
}

fn resolve_plugin_resource_limits(plugin: &PluginEntry) -> Result<PluginResourceLimits> {
    let max_cpu_ms = plugin
        .runtime
        .as_ref()
        .and_then(|runtime| runtime.max_cpu_ms);
    let max_rss_kb = plugin
        .runtime
        .as_ref()
        .and_then(|runtime| runtime.max_rss_kb);
    if matches!(max_cpu_ms, Some(0)) {
        return Err(MosaicError::Validation(format!(
            "plugin '{}' runtime max_cpu_ms must be greater than 0",
            plugin.id
        )));
    }
    if matches!(max_rss_kb, Some(0)) {
        return Err(MosaicError::Validation(format!(
            "plugin '{}' runtime max_rss_kb must be greater than 0",
            plugin.id
        )));
    }
    Ok(PluginResourceLimits {
        max_cpu_ms,
        max_rss_kb,
    })
}

fn validate_plugin_resource_limits_platform(
    plugin: &PluginEntry,
    limits: PluginResourceLimits,
) -> Result<()> {
    #[cfg(not(unix))]
    {
        if limits.max_rss_kb.is_some() {
            return Err(MosaicError::Validation(format!(
                "plugin '{}' runtime max_rss_kb requires unix-like metrics support",
                plugin.id
            )));
        }
    }
    #[cfg(unix)]
    {
        let _ = plugin;
        let _ = limits;
    }
    Ok(())
}

fn resolve_plugin_cpu_watchdog_ms(plugin: &PluginEntry) -> Result<Option<u64>> {
    let cpu_watchdog_ms = plugin
        .runtime
        .as_ref()
        .and_then(|runtime| runtime.cpu_watchdog_ms);
    if matches!(cpu_watchdog_ms, Some(0)) {
        return Err(MosaicError::Validation(format!(
            "plugin '{}' runtime cpu_watchdog_ms must be greater than 0",
            plugin.id
        )));
    }
    Ok(cpu_watchdog_ms)
}

fn resolve_plugin_cpu_watchdog_budget_ms(
    timeout_ms: u64,
    limits: PluginResourceLimits,
    cpu_watchdog_ms: Option<u64>,
) -> Option<u64> {
    if let Some(cpu_watchdog_ms) = cpu_watchdog_ms {
        let budget_ms = cpu_watchdog_ms.min(timeout_ms);
        return (budget_ms < timeout_ms).then_some(budget_ms);
    }
    let max_cpu_ms = limits.max_cpu_ms?;
    let derived_budget = max_cpu_ms
        .saturating_mul(PLUGIN_CPU_WATCHDOG_MULTIPLIER)
        .max(PLUGIN_CPU_WATCHDOG_MIN_BUDGET_MS);
    let budget_ms = derived_budget.min(timeout_ms);
    (budget_ms < timeout_ms).then_some(budget_ms)
}

#[cfg(unix)]
fn apply_plugin_resource_rlimits(
    process: &mut ProcessCommand,
    limits: PluginResourceLimits,
    plugin_id: &str,
) -> Result<bool> {
    if !limits.is_configured() {
        return Ok(false);
    }
    let cpu_seconds = limits
        .max_cpu_ms
        .map(|value| {
            let rounded = value.saturating_add(999) / 1_000;
            let rounded = rounded.max(1);
            libc::rlim_t::try_from(rounded).map_err(|_| {
                MosaicError::Validation(format!(
                    "plugin '{}' runtime max_cpu_ms is too large for this platform",
                    plugin_id
                ))
            })
        })
        .transpose()?;
    #[cfg(any(
        target_os = "linux",
        target_os = "android",
        target_os = "freebsd",
        target_os = "dragonfly",
        target_os = "netbsd",
        target_os = "openbsd"
    ))]
    let memory_limit = limits
        .max_rss_kb
        .map(|value| {
            value
                .checked_mul(1024)
                .ok_or_else(|| {
                    MosaicError::Validation(format!(
                        "plugin '{}' runtime max_rss_kb is too large for this platform",
                        plugin_id
                    ))
                })
                .and_then(|bytes| {
                    if bytes < MIN_PLUGIN_MEMORY_RLIMIT_BYTES {
                        return Ok(None);
                    }
                    libc::rlim_t::try_from(bytes).map(Some).map_err(|_| {
                        MosaicError::Validation(format!(
                            "plugin '{}' runtime max_rss_kb is too large for this platform",
                            plugin_id
                        ))
                    })
                })
        })
        .transpose()?
        .flatten();
    #[cfg(not(any(
        target_os = "linux",
        target_os = "android",
        target_os = "freebsd",
        target_os = "dragonfly",
        target_os = "netbsd",
        target_os = "openbsd"
    )))]
    let memory_limit: Option<libc::rlim_t> = None;
    // Keep tiny memory ceilings as post-run checks only: aggressive rlimit values can block the
    // hook shell from starting at all.
    if cpu_seconds.is_none() && memory_limit.is_none() {
        return Ok(false);
    }
    unsafe {
        process.pre_exec(move || {
            if let Some(cpu_limit) = cpu_seconds {
                let limit = libc::rlimit {
                    rlim_cur: cpu_limit,
                    rlim_max: cpu_limit,
                };
                if libc::setrlimit(libc::RLIMIT_CPU, &limit as *const libc::rlimit) != 0 {
                    return Err(std::io::Error::last_os_error());
                }
            }
            #[cfg(any(
                target_os = "linux",
                target_os = "android",
                target_os = "freebsd",
                target_os = "dragonfly",
                target_os = "netbsd",
                target_os = "openbsd"
            ))]
            if let Some(memory_limit) = memory_limit {
                #[cfg(any(target_os = "linux", target_os = "android"))]
                {
                    let limit = libc::rlimit {
                        rlim_cur: memory_limit,
                        rlim_max: memory_limit,
                    };
                    if libc::setrlimit(libc::RLIMIT_AS, &limit as *const libc::rlimit) != 0 {
                        return Err(std::io::Error::last_os_error());
                    }
                }
                #[cfg(any(
                    target_os = "freebsd",
                    target_os = "dragonfly",
                    target_os = "netbsd",
                    target_os = "openbsd"
                ))]
                {
                    let limit = libc::rlimit {
                        rlim_cur: memory_limit,
                        rlim_max: memory_limit,
                    };
                    if libc::setrlimit(libc::RLIMIT_DATA, &limit as *const libc::rlimit) != 0 {
                        return Err(std::io::Error::last_os_error());
                    }
                }
            }
            #[cfg(not(any(
                target_os = "linux",
                target_os = "android",
                target_os = "freebsd",
                target_os = "dragonfly",
                target_os = "netbsd",
                target_os = "openbsd"
            )))]
            let _ = memory_limit;
            Ok(())
        });
    }
    Ok(true)
}

#[cfg(not(unix))]
fn apply_plugin_resource_rlimits(
    _process: &mut ProcessCommand,
    limits: PluginResourceLimits,
    plugin_id: &str,
) -> Result<bool> {
    if limits.max_rss_kb.is_some() {
        return Err(MosaicError::Validation(format!(
            "plugin '{}' runtime max_rss_kb requires unix-like metrics support",
            plugin_id
        )));
    }
    Ok(false)
}

fn evaluate_plugin_resource_limits(
    plugin: &PluginEntry,
    limits: PluginResourceLimits,
    metrics: Option<&PluginResourceMetrics>,
) -> Result<Option<String>> {
    if !limits.is_configured() {
        return Ok(None);
    }
    if !plugin_resource_metrics_supported() {
        if limits.max_rss_kb.is_some() {
            return Err(MosaicError::Validation(format!(
                "plugin '{}' runtime max_rss_kb requires unix-like metrics support",
                plugin.id
            )));
        }
        // On non-unix platforms, cpu budgets can still be enforced by the wall-time watchdog.
        return Ok(None);
    }
    let metrics = metrics.ok_or_else(|| {
        MosaicError::Tool(format!(
            "plugin '{}' configured runtime resource limits but usage metrics are unavailable",
            plugin.id
        ))
    })?;
    if let Some(max_cpu_ms) = limits.max_cpu_ms {
        let cpu_total_ms = metrics.cpu_total_ms.ok_or_else(|| {
            MosaicError::Tool(format!(
                "plugin '{}' configured max_cpu_ms but cpu metrics are unavailable",
                plugin.id
            ))
        })?;
        if cpu_total_ms > max_cpu_ms {
            return Ok(Some(format!(
                "plugin '{}' hook resource limit exceeded: cpu_total_ms={}ms > max_cpu_ms={}ms",
                plugin.id, cpu_total_ms, max_cpu_ms
            )));
        }
    }
    if let Some(max_rss_kb) = limits.max_rss_kb {
        let observed_rss_kb = metrics.max_rss_kb.ok_or_else(|| {
            MosaicError::Tool(format!(
                "plugin '{}' configured max_rss_kb but rss metrics are unavailable",
                plugin.id
            ))
        })?;
        if observed_rss_kb > max_rss_kb {
            return Ok(Some(format!(
                "plugin '{}' hook resource limit exceeded: max_rss_kb={} > limit={}",
                plugin.id, observed_rss_kb, max_rss_kb
            )));
        }
    }
    Ok(None)
}

#[cfg(unix)]
fn plugin_resource_metrics_supported() -> bool {
    true
}

#[cfg(not(unix))]
fn plugin_resource_metrics_supported() -> bool {
    false
}

fn resolve_plugin_sandbox_profile(
    plugin: &PluginEntry,
    default_profile: SandboxProfile,
) -> Result<SandboxProfile> {
    let Some(value) = plugin
        .runtime
        .as_ref()
        .and_then(|runtime| runtime.sandbox_profile.as_deref())
    else {
        return Ok(default_profile);
    };
    match value {
        "restricted" => Ok(SandboxProfile::Restricted),
        "standard" => Ok(SandboxProfile::Standard),
        "elevated" => Ok(SandboxProfile::Elevated),
        _ => Err(MosaicError::Validation(format!(
            "plugin '{}' has invalid runtime sandbox_profile '{}'",
            plugin.id, value
        ))),
    }
}

fn sandbox_profile_label(profile: SandboxProfile) -> &'static str {
    match profile {
        SandboxProfile::Restricted => "restricted",
        SandboxProfile::Standard => "standard",
        SandboxProfile::Elevated => "elevated",
    }
}

fn validate_plugin_hook_sandbox(
    plugin: &PluginEntry,
    hook_path: &Path,
    profile: SandboxProfile,
) -> Result<()> {
    if profile != SandboxProfile::Restricted {
        return Ok(());
    }
    let extension = hook_path
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| value.to_ascii_lowercase());
    let is_shell_script = matches!(extension.as_deref(), Some("sh") | Some("bash"))
        || (extension.is_none()
            && hook_path
                .parent()
                .and_then(|parent| parent.file_name())
                .and_then(|value| value.to_str())
                == Some("hooks"));
    if !is_shell_script {
        return Ok(());
    }

    let raw = std::fs::read_to_string(hook_path).map_err(|err| {
        MosaicError::Io(format!(
            "failed to read plugin hook '{}': {err}",
            hook_path.display()
        ))
    })?;
    for (index, line) in raw.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if let Some(reason) = evaluate_sandbox(trimmed, profile) {
            return Err(MosaicError::SandboxDenied(format!(
                "plugin '{}' hook '{}' line {} blocked: {}",
                plugin.id,
                hook_path.display(),
                index + 1,
                reason
            )));
        }
    }
    Ok(())
}

fn append_plugin_event(
    data_dir: &Path,
    plugin_id: &str,
    event: &PluginRuntimeEvent,
) -> Result<PathBuf> {
    let path = data_dir
        .join("plugin-events")
        .join(format!("{plugin_id}.jsonl"));
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|err| {
            MosaicError::Io(format!(
                "failed to create plugin event directory '{}': {err}",
                parent.display()
            ))
        })?;
    }
    let mut serialized = serde_json::to_string(event)
        .map_err(|err| MosaicError::Io(format!("failed to serialize plugin event: {err}")))?;
    serialized.push('\n');
    use std::io::Write as _;
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|err| {
            MosaicError::Io(format!(
                "failed to open plugin event file '{}': {err}",
                path.display()
            ))
        })?;
    file.write_all(serialized.as_bytes()).map_err(|err| {
        MosaicError::Io(format!(
            "failed to write plugin event file '{}': {err}",
            path.display()
        ))
    })?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use mosaic_plugins::PluginRuntimeConfig;

    fn test_plugin_entry(runtime: Option<PluginRuntimeConfig>) -> PluginEntry {
        PluginEntry {
            id: "demo".to_string(),
            name: "Demo".to_string(),
            version: Some("0.1.0".to_string()),
            description: None,
            source: ExtensionSource::Project,
            path: ".".to_string(),
            manifest_path: "./plugin.toml".to_string(),
            manifest_exists: true,
            manifest_valid: true,
            manifest_error: None,
            runtime,
        }
    }

    #[test]
    fn resolve_plugin_cpu_watchdog_budget_prefers_runtime_override() {
        let limits = PluginResourceLimits {
            max_cpu_ms: Some(50),
            max_rss_kb: None,
        };
        let derived_budget = resolve_plugin_cpu_watchdog_budget_ms(5_000, limits, None);
        assert_eq!(derived_budget, Some(200));
        let override_budget = resolve_plugin_cpu_watchdog_budget_ms(5_000, limits, Some(1_250));
        assert_eq!(override_budget, Some(1_250));
    }

    #[test]
    #[cfg(unix)]
    fn validate_plugin_resource_limits_platform_accepts_rss_on_unix() {
        let plugin = test_plugin_entry(Some(PluginRuntimeConfig {
            run: Some("hooks/run.sh".to_string()),
            doctor: None,
            timeout_ms: None,
            sandbox_profile: None,
            max_cpu_ms: None,
            max_rss_kb: Some(1024),
            max_output_bytes: None,
            cpu_watchdog_ms: None,
        }));
        let limits = PluginResourceLimits {
            max_cpu_ms: None,
            max_rss_kb: Some(1024),
        };
        assert!(validate_plugin_resource_limits_platform(&plugin, limits).is_ok());
    }

    #[test]
    #[cfg(not(unix))]
    fn validate_plugin_resource_limits_platform_rejects_rss_on_non_unix() {
        let plugin = test_plugin_entry(Some(PluginRuntimeConfig {
            run: Some("hooks/run.sh".to_string()),
            doctor: None,
            timeout_ms: None,
            sandbox_profile: None,
            max_cpu_ms: None,
            max_rss_kb: Some(1024),
            max_output_bytes: None,
            cpu_watchdog_ms: None,
        }));
        let limits = PluginResourceLimits {
            max_cpu_ms: None,
            max_rss_kb: Some(1024),
        };
        let err = validate_plugin_resource_limits_platform(&plugin, limits).expect_err("error");
        assert!(
            err.to_string()
                .contains("runtime max_rss_kb requires unix-like metrics support")
        );
    }

    #[test]
    #[cfg(not(unix))]
    fn evaluate_plugin_resource_limits_allows_cpu_only_without_metrics_on_non_unix() {
        let plugin = test_plugin_entry(Some(PluginRuntimeConfig {
            run: Some("hooks/run.sh".to_string()),
            doctor: None,
            timeout_ms: None,
            sandbox_profile: None,
            max_cpu_ms: Some(100),
            max_rss_kb: None,
            max_output_bytes: None,
            cpu_watchdog_ms: None,
        }));
        let limits = PluginResourceLimits {
            max_cpu_ms: Some(100),
            max_rss_kb: None,
        };
        let evaluated = evaluate_plugin_resource_limits(&plugin, limits, None).expect("ok");
        assert!(evaluated.is_none());
    }

    #[test]
    #[cfg(not(unix))]
    fn evaluate_plugin_resource_limits_rejects_rss_without_metrics_on_non_unix() {
        let plugin = test_plugin_entry(Some(PluginRuntimeConfig {
            run: Some("hooks/run.sh".to_string()),
            doctor: None,
            timeout_ms: None,
            sandbox_profile: None,
            max_cpu_ms: None,
            max_rss_kb: Some(1024),
            max_output_bytes: None,
            cpu_watchdog_ms: None,
        }));
        let limits = PluginResourceLimits {
            max_cpu_ms: None,
            max_rss_kb: Some(1024),
        };
        let err = evaluate_plugin_resource_limits(&plugin, limits, None).expect_err("error");
        assert!(
            err.to_string()
                .contains("runtime max_rss_kb requires unix-like metrics support")
        );
    }
}
