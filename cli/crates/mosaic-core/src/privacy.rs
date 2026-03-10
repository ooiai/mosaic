use std::fs::OpenOptions;
use std::io::Write as _;
use std::path::Path;

use regex::Regex;
use serde::Serialize;
use serde_json::Value;

use crate::error::{MosaicError, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SanitizationSummary {
    pub redacted_fields: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatePersistenceIssueKind {
    PrivateKeyMaterial,
    SensitiveFieldLiteral,
    SecretLikeLiteral,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatePersistenceIssue {
    pub path: String,
    pub kind: StatePersistenceIssueKind,
}

pub fn sanitize_value_for_persistence(
    value: &mut Value,
    context: &str,
) -> Result<SanitizationSummary> {
    let mut summary = SanitizationSummary::default();
    sanitize_value_recursive(value, context, &mut summary)?;
    Ok(summary)
}

pub fn sanitize_text_for_persistence(text: &str, context: &str) -> Result<String> {
    if let Some(reason) = redline_reason(text) {
        return Err(MosaicError::Tool(format!(
            "blocked {context}: detected {reason}"
        )));
    }
    Ok(redact_secret_text(text))
}

pub fn encode_sanitized_json<T: Serialize>(record: &T, context: &str) -> Result<String> {
    let mut encoded = serde_json::to_value(record).map_err(|err| {
        MosaicError::Validation(format!("failed to encode {context} JSON value: {err}"))
    })?;
    let _ = sanitize_value_for_persistence(&mut encoded, context)?;
    serde_json::to_string(&encoded)
        .map_err(|err| MosaicError::Validation(format!("failed to encode {context} JSON: {err}")))
}

pub fn append_sanitized_jsonl<T: Serialize>(path: &Path, record: &T, context: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    let line = encode_sanitized_json(record, context)?;
    file.write_all(line.as_bytes())?;
    file.write_all(b"\n")?;
    Ok(())
}

pub fn render_sanitized_jsonl<'a, I, T>(records: I, context: &str) -> Result<String>
where
    I: IntoIterator<Item = &'a T>,
    T: Serialize + 'a,
{
    let mut rendered = String::new();
    for record in records {
        let line = encode_sanitized_json(record, context)?;
        rendered.push_str(&line);
        rendered.push('\n');
    }
    Ok(rendered)
}

pub fn validate_value_for_state_persistence(value: &Value, context: &str) -> Result<()> {
    if let Some(issue) = inspect_value_for_state_persistence(value)
        .into_iter()
        .next()
    {
        return Err(MosaicError::Validation(format_state_persistence_issue(
            context, &issue,
        )));
    }
    Ok(())
}

pub fn inspect_value_for_state_persistence(value: &Value) -> Vec<StatePersistenceIssue> {
    let mut issues = Vec::new();
    inspect_state_value_recursive(value, "$", None, false, &mut issues);
    issues
}

pub fn write_pretty_state_json_file<T: Serialize>(
    path: &Path,
    value: &T,
    context: &str,
) -> Result<()> {
    let encoded = serde_json::to_value(value).map_err(|err| {
        MosaicError::Validation(format!("failed to encode {context} JSON value: {err}"))
    })?;
    validate_value_for_state_persistence(&encoded, context)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let raw = serde_json::to_string_pretty(value).map_err(|err| {
        MosaicError::Validation(format!("failed to encode {context} JSON: {err}"))
    })?;
    std::fs::write(path, raw)?;
    Ok(())
}

pub fn write_pretty_state_toml_file<T: Serialize>(
    path: &Path,
    value: &T,
    context: &str,
) -> Result<()> {
    let encoded = serde_json::to_value(value).map_err(|err| {
        MosaicError::Validation(format!("failed to encode {context} TOML value: {err}"))
    })?;
    validate_value_for_state_persistence(&encoded, context)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let raw = toml::to_string_pretty(value).map_err(|err| {
        MosaicError::Validation(format!("failed to encode {context} TOML: {err}"))
    })?;
    std::fs::write(path, raw)?;
    Ok(())
}

fn sanitize_value_recursive(
    value: &mut Value,
    context: &str,
    summary: &mut SanitizationSummary,
) -> Result<()> {
    match value {
        Value::String(raw) => {
            let sanitized = sanitize_text_for_persistence(raw, context)?;
            if sanitized != *raw {
                *raw = sanitized;
                summary.redacted_fields += 1;
            }
        }
        Value::Array(items) => {
            for item in items {
                sanitize_value_recursive(item, context, summary)?;
            }
        }
        Value::Object(map) => {
            for (key, nested) in map.iter_mut() {
                if looks_like_secret_key(key)
                    && let Value::String(secret_like) = nested
                    && !secret_like.is_empty()
                {
                    let redacted = sanitize_text_for_persistence(secret_like, context)?;
                    if redacted != "[REDACTED]" {
                        *secret_like = "[REDACTED]".to_string();
                        summary.redacted_fields += 1;
                        continue;
                    }
                }
                sanitize_value_recursive(nested, context, summary)?;
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) => {}
    }
    Ok(())
}

fn inspect_state_value_recursive(
    value: &Value,
    path: &str,
    current_key: Option<&str>,
    allow_env_reference_value: bool,
    issues: &mut Vec<StatePersistenceIssue>,
) {
    match value {
        Value::String(raw) => {
            inspect_state_string(raw, path, current_key, allow_env_reference_value, issues)
        }
        Value::Array(items) => {
            for (index, item) in items.iter().enumerate() {
                let next_path = format!("{path}[{index}]");
                inspect_state_value_recursive(
                    item,
                    &next_path,
                    current_key,
                    allow_env_reference_value,
                    issues,
                );
            }
        }
        Value::Object(map) => {
            for (key, nested) in map {
                let next_path = if path == "$" {
                    format!("$.{key}")
                } else {
                    format!("{path}.{key}")
                };
                let next_allow_env_reference_value = allow_env_reference_value || key == "env_from";
                inspect_state_value_recursive(
                    nested,
                    &next_path,
                    Some(key),
                    next_allow_env_reference_value,
                    issues,
                );
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) => {}
    }
}

fn inspect_state_string(
    raw: &str,
    path: &str,
    current_key: Option<&str>,
    allow_env_reference_value: bool,
    issues: &mut Vec<StatePersistenceIssue>,
) {
    if let Some(reason) = redline_reason(raw) {
        let kind = match reason {
            "private key material" => StatePersistenceIssueKind::PrivateKeyMaterial,
            _ => StatePersistenceIssueKind::SecretLikeLiteral,
        };
        issues.push(StatePersistenceIssue {
            path: path.to_string(),
            kind,
        });
        return;
    }

    if let Some(key) = current_key
        && !allow_env_reference_value
        && looks_like_state_secret_key(key)
        && !raw.trim().is_empty()
    {
        issues.push(StatePersistenceIssue {
            path: path.to_string(),
            kind: StatePersistenceIssueKind::SensitiveFieldLiteral,
        });
        return;
    }

    let redacted = redact_secret_text(raw);
    if redacted != raw {
        issues.push(StatePersistenceIssue {
            path: path.to_string(),
            kind: StatePersistenceIssueKind::SecretLikeLiteral,
        });
    }
}

fn format_state_persistence_issue(context: &str, issue: &StatePersistenceIssue) -> String {
    match issue.kind {
        StatePersistenceIssueKind::PrivateKeyMaterial => {
            format!(
                "blocked {context}: detected private key material at {}",
                issue.path
            )
        }
        StatePersistenceIssueKind::SensitiveFieldLiteral => format!(
            "blocked {context}: secret-like value stored in sensitive state field {}",
            issue.path
        ),
        StatePersistenceIssueKind::SecretLikeLiteral => {
            format!(
                "blocked {context}: detected secret-like value at {}",
                issue.path
            )
        }
    }
}

fn looks_like_secret_key(key: &str) -> bool {
    let normalized = key.trim().to_ascii_lowercase();
    if normalized.is_empty() {
        return false;
    }
    [
        "api_key",
        "apikey",
        "secret",
        "password",
        "passwd",
        "access_token",
        "refresh_token",
        "bearer_token",
        "authorization",
        "auth_header",
        "cookie",
    ]
    .iter()
    .any(|marker| normalized == *marker || normalized.contains(marker))
        || normalized == "token"
        || normalized.ends_with("_token")
}

fn looks_like_state_secret_key(key: &str) -> bool {
    let normalized = key.trim().to_ascii_lowercase();
    if normalized.is_empty() {
        return false;
    }
    if normalized.ends_with("_env")
        || normalized.ends_with("_envs")
        || normalized.ends_with("_path")
        || normalized.ends_with("_paths")
        || normalized.ends_with("_file")
        || normalized.ends_with("_files")
    {
        return false;
    }
    [
        "api_key",
        "apikey",
        "secret",
        "password",
        "passwd",
        "access_token",
        "refresh_token",
        "bearer_token",
        "authorization",
        "auth_header",
        "cookie",
    ]
    .iter()
    .any(|marker| normalized == *marker || normalized.contains(marker))
        || normalized == "token"
        || normalized.ends_with("_token")
}

fn redline_reason(text: &str) -> Option<&'static str> {
    let markers = [
        "BEGIN PRIVATE KEY",
        "BEGIN RSA PRIVATE KEY",
        "BEGIN OPENSSH PRIVATE KEY",
        "BEGIN EC PRIVATE KEY",
        "BEGIN DSA PRIVATE KEY",
    ];
    if markers.iter().any(|marker| text.contains(marker)) {
        return Some("private key material");
    }
    None
}

fn redact_secret_text(text: &str) -> String {
    if text.is_empty() {
        return String::new();
    }

    let mut output = text.to_string();
    let replacements = [
        (
            r#"(?i)\b((?:[a-z_][a-z0-9_]*(?:api[_-]?key|token|secret|password))|api[_-]?key|token|secret|password)\b\s*[:=]\s*["']?([^\s"']+)["']?"#,
            "$1=[REDACTED]",
        ),
        (r#"sk-[A-Za-z0-9_-]{20,}"#, "[REDACTED_OPENAI_KEY]"),
        (r#"AKIA[0-9A-Z]{16}"#, "[REDACTED_AWS_ACCESS_KEY]"),
        (r#"xox[baprs]-[A-Za-z0-9-]{10,}"#, "[REDACTED_SLACK_TOKEN]"),
    ];

    for (pattern, replacement) in replacements {
        if let Ok(regex) = Regex::new(pattern) {
            output = regex.replace_all(&output, replacement).into_owned();
        }
    }

    if let Ok(bearer) = Regex::new(r#"(?i)bearer\s+[A-Za-z0-9._-]{12,}"#) {
        output = bearer
            .replace_all(&output, "Bearer [REDACTED]")
            .into_owned();
    }

    output
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use tempfile::tempdir;

    use super::*;

    #[test]
    fn sanitization_redacts_secret_like_fields_and_text() {
        let mut value = json!({
            "access_token": "abcd1234",
            "nested": {
                "line": "api_key = sk-test-secret-value-1234567890"
            }
        });

        let summary = sanitize_value_for_persistence(&mut value, "test persistence").unwrap();
        assert!(summary.redacted_fields >= 2);
        assert_eq!(value["access_token"], "[REDACTED]");
        assert!(
            value["nested"]["line"]
                .as_str()
                .unwrap()
                .contains("api_key=[REDACTED]")
        );
    }

    #[test]
    fn sanitization_blocks_private_key_material() {
        let mut value = json!({
            "payload": "-----BEGIN OPENSSH PRIVATE KEY-----"
        });
        let err = sanitize_value_for_persistence(&mut value, "session event").unwrap_err();
        assert!(err.to_string().contains("private key material"));
    }

    #[test]
    fn append_sanitized_jsonl_redacts_before_write() {
        let temp = tempdir().expect("tempdir");
        let path = temp.path().join("events.jsonl");
        append_sanitized_jsonl(
            &path,
            &json!({"text": "token=sk-live-secret-12345678901234567890"}),
            "test event",
        )
        .expect("append");
        let raw = std::fs::read_to_string(path).expect("read");
        assert!(raw.contains("token=[REDACTED]"));
    }

    #[test]
    fn render_sanitized_jsonl_blocks_private_key_material() {
        let err = render_sanitized_jsonl(
            [&json!({"payload": "-----BEGIN PRIVATE KEY-----"})],
            "history entry",
        )
        .expect_err("should block");
        assert!(err.to_string().contains("private key material"));
    }

    #[test]
    fn state_validation_blocks_secret_like_key_values() {
        let value = json!({
            "access_token": "plain-secret-value"
        });
        let err = validate_value_for_state_persistence(&value, "gateway state").unwrap_err();
        assert!(err.to_string().contains("sensitive state field"));
    }

    #[test]
    fn state_validation_allows_env_reference_fields() {
        let value = json!({
            "token_env": "MOSAIC_TELEGRAM_BOT_TOKEN",
            "api_key_env": "OPENAI_API_KEY"
        });
        validate_value_for_state_persistence(&value, "channel state").expect("allowed");
    }

    #[test]
    fn state_validation_allows_env_from_secret_key_targets() {
        let value = json!({
            "env_from": {
                "OPENAI_API_KEY": "AZURE_OPENAI_API_KEY",
                "ANTHROPIC_API_KEY": "ANTHROPIC_API_KEY"
            }
        });
        validate_value_for_state_persistence(&value, "mcp servers state").expect("allowed");
    }

    #[test]
    fn write_pretty_state_json_file_blocks_secret_literal_in_generic_field() {
        let temp = tempdir().expect("tempdir");
        let path = temp.path().join("state.json");
        let err = write_pretty_state_json_file(
            &path,
            &json!({"command": "echo sk-live-secret-12345678901234567890"}),
            "hook state",
        )
        .expect_err("should block");
        assert!(err.to_string().contains("secret-like value"));
    }

    #[test]
    fn inspect_state_persistence_reports_issue_paths_and_kinds() {
        let issues = inspect_value_for_state_persistence(&json!({
            "token_env": "OPENAI_API_KEY",
            "provider": {
                "api_key": "sk-live-secret-12345678901234567890"
            },
            "pem": "-----BEGIN OPENSSH PRIVATE KEY-----"
        }));
        assert_eq!(issues.len(), 2);
        assert!(issues.iter().any(|issue| {
            issue.path == "$.provider.api_key"
                && issue.kind == StatePersistenceIssueKind::SensitiveFieldLiteral
        }));
        assert!(issues.iter().any(|issue| {
            issue.path == "$.pem" && issue.kind == StatePersistenceIssueKind::PrivateKeyMaterial
        }));
    }
}
