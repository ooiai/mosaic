use std::path::PathBuf;

use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;

use mosaic_core::error::{MosaicError, Result};
use mosaic_core::privacy::write_pretty_state_json_file;
use mosaic_core::state::StatePaths;

pub(super) fn extract_html_title(body: &str) -> Option<String> {
    let lower = body.to_lowercase();
    let title_start = lower.find("<title")?;
    let open_end_rel = lower[title_start..].find('>')?;
    let content_start = title_start + open_end_rel + 1;
    let close_rel = lower[content_start..].find("</title>")?;
    let content_end = content_start + close_rel;
    let title = body[content_start..content_end].trim();
    if title.is_empty() {
        None
    } else {
        Some(title.to_string())
    }
}

pub(super) fn preview_text(value: &str, max_len: usize) -> Option<String> {
    let text = value.trim();
    if text.is_empty() {
        return None;
    }
    if text.len() <= max_len {
        return Some(text.to_string());
    }
    let mut clipped = text.chars().take(max_len).collect::<String>();
    clipped.push_str("...");
    Some(clipped)
}

pub(super) fn parse_json_input(raw: &str, field_name: &str) -> Result<Value> {
    serde_json::from_str(raw).map_err(|err| {
        MosaicError::Validation(format!(
            "{field_name} must be valid JSON, parse error: {err}"
        ))
    })
}

pub(super) fn resolve_baseline_path(
    paths: &StatePaths,
    cwd: &std::path::Path,
    raw: Option<String>,
) -> PathBuf {
    raw.map_or_else(
        || paths.root_dir.join("security").join("baseline.toml"),
        |value| {
            let path = PathBuf::from(value);
            if path.is_absolute() {
                path
            } else {
                cwd.join(path)
            }
        },
    )
}

pub(super) fn resolve_output_path(cwd: &std::path::Path, raw: &str) -> PathBuf {
    let path = PathBuf::from(raw);
    if path.is_absolute() {
        path
    } else {
        cwd.join(path)
    }
}

pub(super) fn normalize_non_empty_list(
    values: Vec<String>,
    field_name: &str,
) -> Result<Vec<String>> {
    let mut normalized = values
        .into_iter()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();
    if normalized.len()
        != normalized
            .iter()
            .collect::<std::collections::HashSet<_>>()
            .len()
    {
        normalized.sort();
        normalized.dedup();
    }
    for value in &normalized {
        if value.trim().is_empty() {
            return Err(MosaicError::Validation(format!(
                "{field_name} entry cannot be empty"
            )));
        }
    }
    Ok(normalized)
}

pub(super) fn remove_matching(target: &mut Vec<String>, values: &[String]) -> usize {
    let before = target.len();
    target.retain(|item| !values.contains(item));
    before.saturating_sub(target.len())
}

pub(super) fn load_json_file_opt<T>(path: &std::path::Path) -> Result<Option<T>>
where
    T: DeserializeOwned,
{
    if !path.exists() {
        return Ok(None);
    }
    let raw = std::fs::read_to_string(path)?;
    let parsed = serde_json::from_str::<T>(&raw).map_err(|err| {
        MosaicError::Validation(format!("invalid JSON {}: {err}", path.display()))
    })?;
    Ok(Some(parsed))
}

pub(super) fn save_json_file<T>(path: &std::path::Path, value: &T) -> Result<()>
where
    T: Serialize,
{
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let raw = serde_json::to_string_pretty(value).map_err(|err| {
        MosaicError::Validation(format!(
            "failed to serialize JSON {}: {err}",
            path.display()
        ))
    })?;
    std::fs::write(path, raw)?;
    Ok(())
}

pub(super) fn save_state_json_file<T>(
    path: &std::path::Path,
    value: &T,
    context: &str,
) -> Result<()>
where
    T: Serialize,
{
    write_pretty_state_json_file(path, value, context)
}

pub(super) fn print_json(value: &Value) {
    let rendered = serde_json::to_string_pretty(value).unwrap_or_else(|_| "{}".to_string());
    println!("{rendered}");
}

pub(super) fn print_json_line(value: &Value) {
    let rendered = serde_json::to_string(value).unwrap_or_else(|_| "{}".to_string());
    println!("{rendered}");
}

pub(super) fn binary_in_path(name: &str) -> bool {
    if PathBuf::from(name).is_absolute() {
        return PathBuf::from(name).exists();
    }
    std::env::var_os("PATH")
        .and_then(|paths| {
            std::env::split_paths(&paths).find_map(|dir| {
                let full = dir.join(name);
                if full.exists() { Some(full) } else { None }
            })
        })
        .is_some()
}
