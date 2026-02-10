use std::path::Path;

use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_json::Value;

use mosaic_core::error::Result;

#[derive(Debug, Clone, Serialize)]
pub struct UnifiedLogEntry {
    pub source: String,
    pub ts: Option<DateTime<Utc>>,
    pub payload: Value,
}

pub fn collect_logs(data_dir: &Path, tail: usize) -> Result<Vec<UnifiedLogEntry>> {
    let mut entries = Vec::new();

    load_jsonl_file(
        &mut entries,
        &data_dir.join("system-events.jsonl"),
        "system",
    )?;
    load_jsonl_file(
        &mut entries,
        &data_dir.join("audit/commands.jsonl"),
        "audit",
    )?;

    let channel_events_dir = data_dir.join("channel-events");
    if channel_events_dir.exists() {
        for entry in std::fs::read_dir(&channel_events_dir)? {
            let path = entry?.path();
            if path.extension().and_then(|value| value.to_str()) != Some("jsonl") {
                continue;
            }
            let source = format!(
                "channel:{}",
                path.file_stem()
                    .and_then(|value| value.to_str())
                    .unwrap_or("unknown")
            );
            load_jsonl_file(&mut entries, &path, &source)?;
        }
    }

    entries.sort_by(|lhs, rhs| lhs.ts.cmp(&rhs.ts));
    if entries.len() > tail {
        let keep_from = entries.len() - tail;
        entries = entries.split_off(keep_from);
    }
    Ok(entries)
}

fn load_jsonl_file(entries: &mut Vec<UnifiedLogEntry>, path: &Path, source: &str) -> Result<()> {
    if !path.exists() {
        return Ok(());
    }
    let raw = std::fs::read_to_string(path)?;
    for line in raw.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let payload =
            serde_json::from_str::<Value>(line).unwrap_or_else(|_| Value::String(line.to_string()));
        let ts = payload.get("ts").and_then(Value::as_str).and_then(parse_ts);
        entries.push(UnifiedLogEntry {
            source: source.to_string(),
            ts,
            payload,
        });
    }
    Ok(())
}

fn parse_ts(value: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|parsed| parsed.with_timezone(&Utc))
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::*;

    #[test]
    fn collect_logs_handles_missing_files() {
        let temp = tempdir().expect("tempdir");
        let logs = collect_logs(temp.path(), 50).expect("collect logs");
        assert!(logs.is_empty());
    }
}
