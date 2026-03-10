use std::path::PathBuf;
use std::{cmp::Ordering, collections::BTreeMap};

use serde_json::json;

use mosaic_core::error::{MosaicError, Result};
use mosaic_core::state::StatePaths;
use mosaic_security::{
    SecurityAuditOptions, SecurityAuditor, SecurityBaselineConfig, apply_baseline,
    refresh_report_metadata, report_to_sarif,
};

use super::{
    Cli, SecurityArgs, SecurityBaselineCommand, SecurityCommand, SecuritySeverityArg,
    normalize_non_empty_list, print_json, remove_matching, resolve_baseline_path,
    resolve_output_path, resolve_state_paths,
};

pub(super) fn handle_security(cli: &Cli, args: SecurityArgs) -> Result<()> {
    let paths = resolve_state_paths(cli.project_state)?;
    paths.ensure_dirs()?;
    let auditor = SecurityAuditor::new();
    let cwd = std::env::current_dir().map_err(|err| MosaicError::Io(err.to_string()))?;

    match args.command {
        SecurityCommand::Audit {
            path,
            deep,
            max_files,
            max_file_size,
            baseline,
            no_baseline,
            update_baseline,
            sarif,
            sarif_output,
            min_severity,
            categories,
            top,
        } => {
            if no_baseline && update_baseline {
                return Err(MosaicError::Validation(
                    "--no-baseline and --update-baseline cannot be used together".to_string(),
                ));
            }
            if top == Some(0) {
                return Err(MosaicError::Validation(
                    "--top must be greater than 0".to_string(),
                ));
            }
            let categories = normalize_non_empty_list(categories, "category")?;
            let root = {
                let raw = PathBuf::from(path);
                if raw.is_absolute() {
                    raw
                } else {
                    cwd.join(raw)
                }
            };
            let mut report = auditor.audit(SecurityAuditOptions {
                root,
                deep,
                max_files,
                max_file_size,
            })?;
            let baseline_path = resolve_baseline_path(&paths, &cwd, baseline);
            let baseline_path_display = baseline_path.display().to_string();
            let mut baseline_added = 0usize;
            let mut baseline_enabled = false;
            let mut sarif_output_path = None;

            if !no_baseline || update_baseline {
                let mut baseline_config =
                    SecurityBaselineConfig::load_optional(&baseline_path)?.unwrap_or_default();
                if !no_baseline {
                    baseline_enabled = true;
                    let applied = apply_baseline(report, &baseline_config);
                    report = applied.report;
                    report.summary.baseline_path = Some(baseline_path_display.clone());
                }
                if update_baseline {
                    baseline_enabled = true;
                    baseline_added = baseline_config.add_findings(&report.findings);
                    baseline_config.save_to_path(&baseline_path)?;
                    if !report.findings.is_empty() {
                        report.summary.ignored += report.findings.len();
                        report.findings.clear();
                        report.summary.findings = 0;
                        report.summary.high = 0;
                        report.summary.medium = 0;
                        report.summary.low = 0;
                        report.summary.ok = true;
                    }
                    report.summary.baseline_path = Some(baseline_path_display.clone());
                }
            }
            let (report, filtered_out) =
                apply_audit_filters(report, min_severity, &categories, top);

            let sarif_value = if sarif || sarif_output.is_some() {
                Some(report_to_sarif(&report))
            } else {
                None
            };
            if let Some(raw_path) = sarif_output {
                let output_path = resolve_output_path(&cwd, &raw_path);
                if let Some(parent) = output_path.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                let encoded = serde_json::to_string_pretty(
                    sarif_value
                        .as_ref()
                        .ok_or_else(|| MosaicError::Unknown("sarif value missing".to_string()))?,
                )
                .map_err(|err| {
                    MosaicError::Validation(format!("failed to encode sarif JSON: {err}"))
                })?;
                std::fs::write(&output_path, encoded)?;
                sarif_output_path = Some(output_path.display().to_string());
            }

            if sarif {
                print_json(
                    sarif_value
                        .as_ref()
                        .ok_or_else(|| MosaicError::Unknown("sarif value missing".to_string()))?,
                );
                return Ok(());
            }

            if cli.json {
                let dimensions = audit_dimensions(&report);
                print_json(&json!({
                    "ok": true,
                    "report": report,
                    "filters": {
                        "min_severity": min_severity.map(security_severity_name),
                        "categories": categories,
                        "top": top,
                        "filtered_out": filtered_out,
                    },
                    "dimensions": dimensions,
                    "baseline": {
                        "enabled": baseline_enabled,
                        "updated": update_baseline,
                        "added": baseline_added,
                        "path": if baseline_enabled {
                            Some(baseline_path_display.clone())
                        } else {
                            None
                        },
                    },
                    "sarif_output": sarif_output_path,
                }));
            } else {
                println!(
                    "security audit summary: findings={} high={} medium={} low={} ignored={} scanned={} skipped={}",
                    report.summary.findings,
                    report.summary.high,
                    report.summary.medium,
                    report.summary.low,
                    report.summary.ignored,
                    report.summary.scanned_files,
                    report.summary.skipped_files
                );
                println!(
                    "risk: score={} level={:?}",
                    report.risk.score, report.risk.level
                );
                if min_severity.is_some() || !categories.is_empty() || top.is_some() {
                    println!(
                        "filters: min_severity={} categories={} top={} filtered_out={}",
                        min_severity.map(security_severity_name).unwrap_or("-"),
                        if categories.is_empty() {
                            "-".to_string()
                        } else {
                            categories.join(",")
                        },
                        top.map(|value| value.to_string())
                            .unwrap_or_else(|| "-".to_string()),
                        filtered_out
                    );
                }
                let dimensions = audit_dimensions(&report);
                println!(
                    "dimensions: categories={} high={} medium={} low={}",
                    dimensions["categories"]
                        .as_object()
                        .map(|map| map.len())
                        .unwrap_or(0),
                    dimensions["severities"]["high"].as_u64().unwrap_or(0),
                    dimensions["severities"]["medium"].as_u64().unwrap_or(0),
                    dimensions["severities"]["low"].as_u64().unwrap_or(0),
                );
                for recommendation in &report.risk.recommendations {
                    println!("recommendation: {recommendation}");
                }
                if baseline_enabled {
                    println!("baseline: {baseline_path_display}");
                }
                if update_baseline {
                    println!("baseline updated: added {baseline_added} fingerprints");
                }
                if let Some(sarif_output_path) = sarif_output_path {
                    println!("sarif: {sarif_output_path}");
                }
                if report.findings.is_empty() {
                    println!("No security findings.");
                } else {
                    for finding in report.findings {
                        println!(
                            "[{:?}] {}:{} {} ({})",
                            finding.severity,
                            finding.path,
                            finding
                                .line
                                .map(|value| value.to_string())
                                .unwrap_or_else(|| "-".to_string()),
                            finding.title,
                            finding.category
                        );
                        if let Some(suggestion) = finding.suggestion {
                            println!("  suggestion: {suggestion}");
                        }
                    }
                }
            }
        }
        SecurityCommand::Baseline { command } => {
            handle_security_baseline(cli, &paths, &cwd, command)?;
        }
    }
    Ok(())
}

fn apply_audit_filters(
    mut report: mosaic_security::SecurityAuditReport,
    min_severity: Option<SecuritySeverityArg>,
    categories: &[String],
    top: Option<usize>,
) -> (mosaic_security::SecurityAuditReport, usize) {
    let before = report.findings.len();
    if !categories.is_empty() {
        report.findings.retain(|finding| {
            categories
                .iter()
                .any(|category| category.eq_ignore_ascii_case(&finding.category))
        });
    }
    if let Some(min_severity) = min_severity {
        report.findings.retain(|finding| {
            security_severity_rank_finding(finding.severity)
                >= security_severity_rank_arg(min_severity)
        });
    }
    report.findings.sort_by(|lhs, rhs| {
        security_severity_rank_finding(rhs.severity)
            .cmp(&security_severity_rank_finding(lhs.severity))
            .then_with(|| lhs.path.cmp(&rhs.path))
            .then_with(|| match (lhs.line, rhs.line) {
                (Some(a), Some(b)) => a.cmp(&b),
                (Some(_), None) => Ordering::Less,
                (None, Some(_)) => Ordering::Greater,
                (None, None) => Ordering::Equal,
            })
    });
    if let Some(limit) = top
        && report.findings.len() > limit
    {
        report.findings.truncate(limit);
    }
    update_audit_summary(&mut report);
    let filtered_out = before.saturating_sub(report.findings.len());
    (report, filtered_out)
}

fn update_audit_summary(report: &mut mosaic_security::SecurityAuditReport) {
    refresh_report_metadata(report);
}

fn audit_dimensions(report: &mosaic_security::SecurityAuditReport) -> serde_json::Value {
    let mut categories = BTreeMap::new();
    for finding in &report.findings {
        *categories.entry(finding.category.clone()).or_insert(0usize) += 1;
    }
    json!({
        "categories": categories,
        "severities": {
            "high": report.summary.high,
            "medium": report.summary.medium,
            "low": report.summary.low,
        }
    })
}

fn security_severity_rank_arg(value: SecuritySeverityArg) -> u8 {
    match value {
        SecuritySeverityArg::Low => 1,
        SecuritySeverityArg::Medium => 2,
        SecuritySeverityArg::High => 3,
    }
}

fn security_severity_rank_finding(value: mosaic_security::SecuritySeverity) -> u8 {
    match value {
        mosaic_security::SecuritySeverity::Low => 1,
        mosaic_security::SecuritySeverity::Medium => 2,
        mosaic_security::SecuritySeverity::High => 3,
    }
}

fn security_severity_name(value: SecuritySeverityArg) -> &'static str {
    match value {
        SecuritySeverityArg::Low => "low",
        SecuritySeverityArg::Medium => "medium",
        SecuritySeverityArg::High => "high",
    }
}

fn handle_security_baseline(
    cli: &Cli,
    paths: &StatePaths,
    cwd: &std::path::Path,
    command: SecurityBaselineCommand,
) -> Result<()> {
    match command {
        SecurityBaselineCommand::Show { path } => {
            let baseline_path = resolve_baseline_path(paths, cwd, path);
            let exists = baseline_path.exists();
            let baseline = SecurityBaselineConfig::load_optional(&baseline_path)?
                .unwrap_or_else(SecurityBaselineConfig::default);
            let stats = json!({
                "fingerprints": baseline.ignored_fingerprints.len(),
                "categories": baseline.ignored_categories.len(),
                "paths": baseline.ignored_paths.len(),
            });
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "baseline": baseline,
                    "path": baseline_path.display().to_string(),
                    "exists": exists,
                    "stats": stats,
                }));
            } else {
                println!("baseline path: {}", baseline_path.display());
                println!("exists: {}", exists);
                println!(
                    "entries: fingerprints={} categories={} paths={}",
                    baseline.ignored_fingerprints.len(),
                    baseline.ignored_categories.len(),
                    baseline.ignored_paths.len()
                );
            }
        }
        SecurityBaselineCommand::Add {
            path,
            fingerprints,
            categories,
            match_paths,
        } => {
            let baseline_path = resolve_baseline_path(paths, cwd, path);
            let mut baseline = SecurityBaselineConfig::load_optional(&baseline_path)?
                .unwrap_or_else(SecurityBaselineConfig::default);
            let fingerprints = normalize_non_empty_list(fingerprints, "fingerprint")?;
            let categories = normalize_non_empty_list(categories, "category")?;
            let match_paths = normalize_non_empty_list(match_paths, "match-path")?;
            if fingerprints.is_empty() && categories.is_empty() && match_paths.is_empty() {
                return Err(MosaicError::Validation(
                    "baseline add requires at least one of --fingerprint/--category/--match-path"
                        .to_string(),
                ));
            }

            let mut added = 0usize;
            for value in fingerprints {
                if !baseline.ignored_fingerprints.contains(&value) {
                    baseline.ignored_fingerprints.push(value);
                    added += 1;
                }
            }
            for value in categories {
                if !baseline.ignored_categories.contains(&value) {
                    baseline.ignored_categories.push(value);
                    added += 1;
                }
            }
            for value in match_paths {
                if !baseline.ignored_paths.contains(&value) {
                    baseline.ignored_paths.push(value);
                    added += 1;
                }
            }
            baseline.save_to_path(&baseline_path)?;

            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "added": added,
                    "path": baseline_path.display().to_string(),
                    "baseline": baseline,
                }));
            } else {
                println!("baseline updated: added {added} entries");
                println!("path: {}", baseline_path.display());
            }
        }
        SecurityBaselineCommand::Remove {
            path,
            fingerprints,
            categories,
            match_paths,
        } => {
            let baseline_path = resolve_baseline_path(paths, cwd, path);
            let exists = baseline_path.exists();
            let mut baseline = SecurityBaselineConfig::load_optional(&baseline_path)?
                .unwrap_or_else(SecurityBaselineConfig::default);
            let fingerprints = normalize_non_empty_list(fingerprints, "fingerprint")?;
            let categories = normalize_non_empty_list(categories, "category")?;
            let match_paths = normalize_non_empty_list(match_paths, "match-path")?;
            if fingerprints.is_empty() && categories.is_empty() && match_paths.is_empty() {
                return Err(MosaicError::Validation(
                    "baseline remove requires at least one of --fingerprint/--category/--match-path"
                        .to_string(),
                ));
            }

            let mut removed = 0usize;
            removed += remove_matching(&mut baseline.ignored_fingerprints, &fingerprints);
            removed += remove_matching(&mut baseline.ignored_categories, &categories);
            removed += remove_matching(&mut baseline.ignored_paths, &match_paths);
            if exists {
                baseline.save_to_path(&baseline_path)?;
            }

            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "removed": removed,
                    "path": baseline_path.display().to_string(),
                    "exists": exists,
                    "baseline": baseline,
                }));
            } else {
                println!("baseline updated: removed {removed} entries");
                println!("path: {}", baseline_path.display());
            }
        }
        SecurityBaselineCommand::Clear { path } => {
            let baseline_path = resolve_baseline_path(paths, cwd, path);
            let old = SecurityBaselineConfig::load_optional(&baseline_path)?
                .unwrap_or_else(SecurityBaselineConfig::default);
            let removed = old.ignored_fingerprints.len()
                + old.ignored_categories.len()
                + old.ignored_paths.len();
            let baseline = SecurityBaselineConfig::default();
            baseline.save_to_path(&baseline_path)?;
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "cleared": removed,
                    "path": baseline_path.display().to_string(),
                    "baseline": baseline,
                }));
            } else {
                println!("baseline cleared: removed {removed} entries");
                println!("path: {}", baseline_path.display());
            }
        }
    }
    Ok(())
}
