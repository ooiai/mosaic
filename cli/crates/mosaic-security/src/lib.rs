use std::collections::HashSet;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use regex::Regex;
use serde::{Deserialize, Serialize};
use walkdir::WalkDir;

use mosaic_core::error::{MosaicError, Result};

const DEFAULT_MAX_FILES: usize = 800;
const DEFAULT_MAX_FILE_SIZE: usize = 256 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SecuritySeverity {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityFinding {
    pub id: String,
    pub severity: SecuritySeverity,
    pub category: String,
    pub title: String,
    pub detail: String,
    pub path: String,
    pub line: Option<usize>,
    pub suggestion: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityAuditSummary {
    pub ok: bool,
    pub findings: usize,
    pub high: usize,
    pub medium: usize,
    pub low: usize,
    pub scanned_files: usize,
    pub skipped_files: usize,
    pub generated_at: DateTime<Utc>,
    pub root: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityAuditReport {
    pub summary: SecurityAuditSummary,
    pub findings: Vec<SecurityFinding>,
}

#[derive(Debug, Clone)]
pub struct SecurityAuditOptions {
    pub root: PathBuf,
    pub deep: bool,
    pub max_files: usize,
    pub max_file_size: usize,
}

impl Default for SecurityAuditOptions {
    fn default() -> Self {
        Self {
            root: PathBuf::from("."),
            deep: false,
            max_files: DEFAULT_MAX_FILES,
            max_file_size: DEFAULT_MAX_FILE_SIZE,
        }
    }
}

#[derive(Debug, Default, Clone)]
pub struct SecurityAuditor;

impl SecurityAuditor {
    pub fn new() -> Self {
        Self
    }

    pub fn audit(&self, options: SecurityAuditOptions) -> Result<SecurityAuditReport> {
        let root = canonicalize_root(&options.root)?;
        let rules = Rules::new(options.deep)?;

        let mut findings = Vec::new();
        let mut finding_keys = HashSet::new();
        let mut scanned_files = 0usize;
        let mut skipped_files = 0usize;

        for entry in WalkDir::new(&root).into_iter().flatten() {
            if scanned_files >= options.max_files {
                break;
            }
            let path = entry.path();
            if path.is_dir() {
                continue;
            }
            if should_skip(path) {
                skipped_files += 1;
                continue;
            }

            let metadata = match std::fs::metadata(path) {
                Ok(metadata) => metadata,
                Err(_) => {
                    skipped_files += 1;
                    continue;
                }
            };
            if metadata.len() as usize > options.max_file_size {
                skipped_files += 1;
                continue;
            }

            let content = match std::fs::read_to_string(path) {
                Ok(value) => value,
                Err(_) => {
                    skipped_files += 1;
                    continue;
                }
            };
            scanned_files += 1;
            let relative = relative_path(&root, path);
            scan_content(
                &rules,
                &relative,
                &content,
                &mut findings,
                &mut finding_keys,
            );
        }

        let high = findings
            .iter()
            .filter(|finding| finding.severity == SecuritySeverity::High)
            .count();
        let medium = findings
            .iter()
            .filter(|finding| finding.severity == SecuritySeverity::Medium)
            .count();
        let low = findings
            .iter()
            .filter(|finding| finding.severity == SecuritySeverity::Low)
            .count();

        let summary = SecurityAuditSummary {
            ok: high == 0,
            findings: findings.len(),
            high,
            medium,
            low,
            scanned_files,
            skipped_files,
            generated_at: Utc::now(),
            root: root.display().to_string(),
        };

        Ok(SecurityAuditReport { summary, findings })
    }
}

#[derive(Debug)]
struct Rules {
    hardcoded_secret: Regex,
    aws_access_key: Regex,
    insecure_http: Regex,
    curl_pipe_shell: Regex,
    wildcard_cors: Regex,
    javascript_eval: Regex,
}

impl Rules {
    fn new(_deep: bool) -> Result<Self> {
        Ok(Self {
            hardcoded_secret: Regex::new(
                r#"(?i)\b(api[_-]?key|secret|token|password)\b[^\n]{0,48}[:=][^\n]{0,8}["'][^"'\s]{12,}["']"#,
            )
            .map_err(|err| MosaicError::Validation(format!("invalid regex hardcoded_secret: {err}")))?,
            aws_access_key: Regex::new(r"\bAKIA[0-9A-Z]{16}\b")
                .map_err(|err| MosaicError::Validation(format!("invalid regex aws_access_key: {err}")))?,
            insecure_http: Regex::new(r#"http://[^\s"'<>]+"#)
                .map_err(|err| MosaicError::Validation(format!("invalid regex insecure_http: {err}")))?,
            curl_pipe_shell: Regex::new(r"(?i)curl\s+[^\n|]+\|\s*(sh|bash)")
                .map_err(|err| MosaicError::Validation(format!("invalid regex curl_pipe_shell: {err}")))?,
            wildcard_cors: Regex::new(r#"(?i)access-control-allow-origin\s*[:=]\s*["']\*["']"#)
                .map_err(|err| MosaicError::Validation(format!("invalid regex wildcard_cors: {err}")))?,
            javascript_eval: Regex::new(r"\beval\s*\(")
                .map_err(|err| MosaicError::Validation(format!("invalid regex javascript_eval: {err}")))?,
        })
    }
}

fn scan_content(
    rules: &Rules,
    path: &str,
    content: &str,
    findings: &mut Vec<SecurityFinding>,
    keys: &mut HashSet<String>,
) {
    if content.contains("BEGIN PRIVATE KEY") || content.contains("BEGIN RSA PRIVATE KEY") {
        push_finding(
            findings,
            keys,
            SecurityFinding {
                id: format!("sec_{}", uuid::Uuid::new_v4()),
                severity: SecuritySeverity::High,
                category: "credential_exposure".to_string(),
                title: "Private key material detected".to_string(),
                detail: "File contains PEM private key markers.".to_string(),
                path: path.to_string(),
                line: line_of(content, "BEGIN").or(Some(1)),
                suggestion: Some(
                    "Move private keys to a secure secret store and rotate immediately."
                        .to_string(),
                ),
            },
        );
    }

    for (line_number, line) in content.lines().enumerate() {
        if rules.hardcoded_secret.is_match(line) {
            push_finding(
                findings,
                keys,
                SecurityFinding {
                    id: format!("sec_{}", uuid::Uuid::new_v4()),
                    severity: SecuritySeverity::High,
                    category: "credential_exposure".to_string(),
                    title: "Potential hardcoded secret".to_string(),
                    detail: "Detected secret-like assignment with quoted literal value."
                        .to_string(),
                    path: path.to_string(),
                    line: Some(line_number + 1),
                    suggestion: Some(
                        "Replace literals with environment variables or secret manager references."
                            .to_string(),
                    ),
                },
            );
        }

        if rules.aws_access_key.is_match(line) {
            push_finding(
                findings,
                keys,
                SecurityFinding {
                    id: format!("sec_{}", uuid::Uuid::new_v4()),
                    severity: SecuritySeverity::High,
                    category: "credential_exposure".to_string(),
                    title: "AWS access key pattern detected".to_string(),
                    detail: "Line matches AKIA-style access key format.".to_string(),
                    path: path.to_string(),
                    line: Some(line_number + 1),
                    suggestion: Some(
                        "Revoke this key and replace with scoped runtime credentials.".to_string(),
                    ),
                },
            );
        }

        if rules.curl_pipe_shell.is_match(line) {
            push_finding(
                findings,
                keys,
                SecurityFinding {
                    id: format!("sec_{}", uuid::Uuid::new_v4()),
                    severity: SecuritySeverity::Medium,
                    category: "supply_chain".to_string(),
                    title: "curl pipe to shell detected".to_string(),
                    detail: "Direct execution of remote script was detected.".to_string(),
                    path: path.to_string(),
                    line: Some(line_number + 1),
                    suggestion: Some(
                        "Download script, pin checksum/signature, and review before execution."
                            .to_string(),
                    ),
                },
            );
        }

        if rules.insecure_http.is_match(line) {
            push_finding(
                findings,
                keys,
                SecurityFinding {
                    id: format!("sec_{}", uuid::Uuid::new_v4()),
                    severity: SecuritySeverity::Low,
                    category: "transport_security".to_string(),
                    title: "Insecure HTTP endpoint detected".to_string(),
                    detail: "Plain HTTP URL found; consider TLS-protected HTTPS.".to_string(),
                    path: path.to_string(),
                    line: Some(line_number + 1),
                    suggestion: Some("Migrate to HTTPS where possible.".to_string()),
                },
            );
        }

        if rules.wildcard_cors.is_match(line) {
            push_finding(
                findings,
                keys,
                SecurityFinding {
                    id: format!("sec_{}", uuid::Uuid::new_v4()),
                    severity: SecuritySeverity::Medium,
                    category: "cors".to_string(),
                    title: "Wildcard CORS policy detected".to_string(),
                    detail: "Access-Control-Allow-Origin is configured as '*'".to_string(),
                    path: path.to_string(),
                    line: Some(line_number + 1),
                    suggestion: Some(
                        "Use explicit trusted origins instead of wildcard.".to_string(),
                    ),
                },
            );
        }

        if rules.javascript_eval.is_match(line) {
            push_finding(
                findings,
                keys,
                SecurityFinding {
                    id: format!("sec_{}", uuid::Uuid::new_v4()),
                    severity: SecuritySeverity::Medium,
                    category: "code_injection".to_string(),
                    title: "eval() usage detected".to_string(),
                    detail: "Dynamic code execution increases injection risk.".to_string(),
                    path: path.to_string(),
                    line: Some(line_number + 1),
                    suggestion: Some(
                        "Replace eval() with safer parsing/execution strategy.".to_string(),
                    ),
                },
            );
        }
    }
}

fn push_finding(
    findings: &mut Vec<SecurityFinding>,
    keys: &mut HashSet<String>,
    finding: SecurityFinding,
) {
    let key = format!(
        "{}:{}:{}:{}",
        finding.path,
        finding.line.unwrap_or_default(),
        finding.category,
        finding.title
    );
    if keys.insert(key) {
        findings.push(finding);
    }
}

fn line_of(content: &str, needle: &str) -> Option<usize> {
    content
        .lines()
        .position(|line| line.contains(needle))
        .map(|idx| idx + 1)
}

fn canonicalize_root(root: &Path) -> Result<PathBuf> {
    if root.exists() {
        return root.canonicalize().map_err(|err| {
            MosaicError::Io(format!(
                "failed to resolve security root {}: {err}",
                root.display()
            ))
        });
    }
    Err(MosaicError::Validation(format!(
        "security root path does not exist: {}",
        root.display()
    )))
}

fn relative_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .ok()
        .map(|value| value.to_string_lossy().to_string())
        .unwrap_or_else(|| path.to_string_lossy().to_string())
}

fn should_skip(path: &Path) -> bool {
    let value = path.to_string_lossy();
    value.contains("/.git/")
        || value.contains("/target/")
        || value.contains("/node_modules/")
        || value.contains("/.pnpm-store/")
        || value.contains("/.mosaic/")
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::*;

    #[test]
    fn audit_detects_private_key_and_secret() {
        let temp = tempdir().expect("tempdir");
        std::fs::write(
            temp.path().join("secrets.txt"),
            "-----BEGIN PRIVATE KEY-----\napi_key = \"sk-test-secret-value-123456\"\n",
        )
        .expect("write file");

        let auditor = SecurityAuditor::new();
        let report = auditor
            .audit(SecurityAuditOptions {
                root: temp.path().to_path_buf(),
                ..SecurityAuditOptions::default()
            })
            .expect("audit report");

        assert!(report.summary.findings >= 2);
        assert!(report.summary.high >= 1);
        assert!(!report.summary.ok);
    }

    #[test]
    fn audit_detects_curl_pipe_and_http() {
        let temp = tempdir().expect("tempdir");
        std::fs::write(
            temp.path().join("install.sh"),
            "curl https://example.com/install.sh | sh\nBASE_URL=http://example.com",
        )
        .expect("write script");

        let auditor = SecurityAuditor::new();
        let report = auditor
            .audit(SecurityAuditOptions {
                root: temp.path().to_path_buf(),
                deep: true,
                ..SecurityAuditOptions::default()
            })
            .expect("audit report");

        assert!(report.summary.findings >= 2);
        assert!(
            report
                .findings
                .iter()
                .any(|finding| finding.category == "supply_chain")
        );
        assert!(
            report
                .findings
                .iter()
                .any(|finding| finding.category == "transport_security")
        );
    }

    #[test]
    fn audit_on_clean_files_has_zero_findings() {
        let temp = tempdir().expect("tempdir");
        std::fs::write(
            temp.path().join("main.rs"),
            "fn main() { println!(\"ok\"); }",
        )
        .expect("write file");

        let auditor = SecurityAuditor::new();
        let report = auditor
            .audit(SecurityAuditOptions {
                root: temp.path().to_path_buf(),
                ..SecurityAuditOptions::default()
            })
            .expect("audit report");

        assert_eq!(report.summary.findings, 0);
        assert!(report.summary.ok);
    }
}
