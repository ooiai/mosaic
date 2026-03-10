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
}
