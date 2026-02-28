use std::path::PathBuf;

use chrono::Utc;
use serde_json::json;

use mosaic_core::error::MosaicError;
use mosaic_memory::{MemoryIndexOptions, MemoryStore, memory_index_path, memory_status_path};
use mosaic_plugins::{ExtensionRegistry, ExtensionSource, RegistryRoots};

use super::{
    BrowserArgs, BrowserCommand, Cli, ExtensionSourceFilterArg, MemoryArgs, MemoryCommand,
    PluginsArgs, PluginsCommand, Result, SkillsArgs, SkillsCommand, browser_history_file_path,
    browser_open_visit, browser_state_file_path, load_browser_history_or_default,
    load_browser_state_or_default, print_json, resolve_state_paths, save_browser_history,
    save_browser_state,
};

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

    match args.command {
        PluginsCommand::List { source } => {
            let requested_source = parse_extension_source_filter(source);
            let plugins = registry
                .list_plugins()?
                .into_iter()
                .filter(|entry| source_matches(requested_source, entry.source))
                .collect::<Vec<_>>();
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "source_filter": extension_source_filter_name(source),
                    "count": plugins.len(),
                    "plugins": plugins,
                }));
            } else if plugins.is_empty() {
                println!("No plugins found.");
            } else {
                println!("plugins: {}", plugins.len());
                println!("source filter: {}", extension_source_filter_name(source));
                for plugin in plugins {
                    println!(
                        "- {} ({}) source={:?} version={} manifest_valid={}",
                        plugin.id,
                        plugin.name,
                        plugin.source,
                        plugin.version.unwrap_or_else(|| "-".to_string()),
                        plugin.manifest_valid
                    );
                }
            }
        }
        PluginsCommand::Info { plugin_id } => {
            let plugin = registry.plugin_info(&plugin_id)?;
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "plugin": plugin,
                }));
            } else {
                println!("id: {}", plugin.id);
                println!("name: {}", plugin.name);
                println!("source: {:?}", plugin.source);
                println!(
                    "version: {}",
                    plugin.version.unwrap_or_else(|| "-".to_string())
                );
                println!(
                    "description: {}",
                    plugin.description.unwrap_or_else(|| "-".to_string())
                );
                println!("path: {}", plugin.path);
                println!("manifest path: {}", plugin.manifest_path);
                println!("manifest valid: {}", plugin.manifest_valid);
                if let Some(error) = plugin.manifest_error {
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
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "installed": outcome,
                }));
            } else {
                println!(
                    "Installed plugin {} -> {}",
                    outcome.id, outcome.installed_path
                );
                if outcome.replaced {
                    println!("replaced existing plugin package");
                }
            }
        }
        PluginsCommand::Remove { plugin_id } => {
            let removed = registry.remove_project_plugin(&plugin_id)?;
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "removed": removed,
                    "plugin_id": plugin_id,
                }));
            } else if removed {
                println!("Removed plugin {plugin_id}");
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
