use std::path::PathBuf;

use serde_json::json;

use mosaic_core::error::MosaicError;
use mosaic_memory::{MemoryIndexOptions, MemoryStore, memory_index_path, memory_status_path};
use mosaic_plugins::{ExtensionRegistry, RegistryRoots};

use super::{
    BrowserArgs, BrowserCommand, Cli, MemoryArgs, MemoryCommand, PluginsArgs, PluginsCommand,
    Result, SkillsArgs, SkillsCommand, browser_history_file_path, browser_open_visit,
    load_browser_history_or_default, print_json, resolve_state_paths, save_browser_history,
};

pub(super) async fn handle_browser(cli: &Cli, args: BrowserArgs) -> Result<()> {
    let paths = resolve_state_paths(cli.project_state)?;
    paths.ensure_dirs()?;
    let history_path = browser_history_file_path(&paths.data_dir);
    let mut history = load_browser_history_or_default(&history_path)?;
    match args.command {
        BrowserCommand::Open { url, timeout_ms } => {
            if timeout_ms == 0 {
                return Err(MosaicError::Validation(
                    "--timeout-ms must be greater than 0".to_string(),
                ));
            }
            let visit = browser_open_visit(&url, timeout_ms).await?;
            history.push(visit.clone());
            save_browser_history(&history_path, &history)?;
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "visit": visit,
                    "path": history_path.display().to_string(),
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
        BrowserCommand::History { tail } => {
            let mut visits = history;
            visits.sort_by(|lhs, rhs| lhs.ts.cmp(&rhs.ts));
            if visits.len() > tail {
                let keep_from = visits.len() - tail;
                visits = visits.split_off(keep_from);
            }
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "visits": visits,
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
            let visit = history
                .into_iter()
                .find(|item| item.id == visit_id)
                .ok_or_else(|| {
                    MosaicError::Validation(format!("visit '{}' not found", visit_id))
                })?;
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
        BrowserCommand::Clear { visit_id, all } => {
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
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "removed": removed,
                    "remaining": remaining.len(),
                    "path": history_path.display().to_string(),
                }));
            } else {
                println!("removed visits: {removed}");
                println!("remaining visits: {}", remaining.len());
            }
        }
    }
    Ok(())
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
    }
    Ok(())
}

pub(super) fn handle_plugins(cli: &Cli, args: PluginsArgs) -> Result<()> {
    let paths = resolve_state_paths(cli.project_state)?;
    paths.ensure_dirs()?;
    let registry = ExtensionRegistry::new(RegistryRoots::from_state_root(paths.root_dir.clone()));

    match args.command {
        PluginsCommand::List => {
            let plugins = registry.list_plugins()?;
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "count": plugins.len(),
                    "plugins": plugins,
                }));
            } else if plugins.is_empty() {
                println!("No plugins found.");
            } else {
                println!("plugins: {}", plugins.len());
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
        SkillsCommand::List => {
            let skills = registry.list_skills()?;
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "count": skills.len(),
                    "skills": skills,
                }));
            } else if skills.is_empty() {
                println!("No skills found.");
            } else {
                println!("skills: {}", skills.len());
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
