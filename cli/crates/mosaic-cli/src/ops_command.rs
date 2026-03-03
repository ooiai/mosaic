use std::collections::{BTreeMap, BTreeSet};
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use mosaic_channels::{ChannelRepository, channels_events_dir, channels_file_path};
use mosaic_core::audit::CommandAudit;
use mosaic_core::error::{MosaicError, Result};
use mosaic_ops::{
    ApprovalDecision, ApprovalStore, SandboxStore, SystemEventStore, UnifiedLogEntry, collect_logs,
    evaluate_approval, evaluate_sandbox, list_profiles, snapshot_presence, system_events_path,
};

use super::{
    AllowlistCommand, ApprovalsArgs, ApprovalsCommand, Cli, LogsArgs, ObservabilityArgs,
    ObservabilityCommand, SafetyArgs, SafetyCommand, SandboxArgs, SandboxCommand, SystemArgs,
    SystemCommand, collect_gateway_runtime_status, dispatch_system_event, parse_json_input,
    print_json, resolve_state_paths, save_json_file,
};

pub(super) async fn handle_logs(cli: &Cli, args: LogsArgs) -> Result<()> {
    let paths = resolve_state_paths(cli.project_state)?;
    paths.ensure_dirs()?;
    let source_filter = args.source.as_deref();

    if !args.follow {
        let entries = filter_logs(collect_logs(&paths.data_dir, args.tail)?, source_filter);
        if cli.json {
            print_json(&json!({
                "ok": true,
                "logs": entries,
            }));
        } else if entries.is_empty() {
            println!("No logs found.");
        } else {
            for entry in entries {
                println!(
                    "{} [{}] {}",
                    entry
                        .ts
                        .map(|value| value.to_rfc3339())
                        .unwrap_or_else(|| "-".to_string()),
                    entry.source,
                    entry.payload
                );
            }
        }
        return Ok(());
    }

    let mut printed = 0usize;
    loop {
        let entries = filter_logs(
            collect_logs(&paths.data_dir, args.tail.max(200))?,
            source_filter,
        );
        if entries.len() > printed {
            for entry in entries.iter().skip(printed) {
                println!(
                    "{} [{}] {}",
                    entry
                        .ts
                        .map(|value| value.to_rfc3339())
                        .unwrap_or_else(|| "-".to_string()),
                    entry.source,
                    entry.payload
                );
            }
            printed = entries.len();
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
}

fn filter_logs(entries: Vec<UnifiedLogEntry>, source_filter: Option<&str>) -> Vec<UnifiedLogEntry> {
    match source_filter {
        Some(source) => entries
            .into_iter()
            .filter(|entry| entry.source == source)
            .collect(),
        None => entries,
    }
}

struct ObservabilityReportOptions {
    tail: usize,
    source: Option<String>,
    event_tail: usize,
    audit_tail: usize,
    compare_window: usize,
    event_name: Option<String>,
    include_doctor: bool,
    plugin_soak_report: Option<std::path::PathBuf>,
}

pub(super) async fn handle_observability(cli: &Cli, args: ObservabilityArgs) -> Result<()> {
    let (report, export_path) = match args.command {
        ObservabilityCommand::Report {
            tail,
            source,
            event_tail,
            audit_tail,
            compare_window,
            event_name,
            no_doctor,
            plugin_soak_report,
        } => (
            build_observability_report(
                cli,
                ObservabilityReportOptions {
                    tail,
                    source,
                    event_tail,
                    audit_tail,
                    compare_window,
                    event_name,
                    include_doctor: !no_doctor,
                    plugin_soak_report,
                },
            )
            .await?,
            None,
        ),
        ObservabilityCommand::Export {
            out,
            tail,
            source,
            event_tail,
            audit_tail,
            compare_window,
            event_name,
            no_doctor,
            plugin_soak_report,
        } => (
            build_observability_report(
                cli,
                ObservabilityReportOptions {
                    tail,
                    source,
                    event_tail,
                    audit_tail,
                    compare_window,
                    event_name,
                    include_doctor: !no_doctor,
                    plugin_soak_report,
                },
            )
            .await?,
            Some(out),
        ),
    };

    if let Some(path) = export_path.as_ref() {
        save_json_file(path, &report)?;
    }

    if cli.json {
        print_json(&json!({
            "ok": true,
            "report": report,
            "export_path": export_path.map(|value| value.display().to_string()),
        }));
    } else {
        println!("observability report");
        println!(
            "generated_at: {}",
            report["generated_at"].as_str().unwrap_or("-")
        );
        println!("profile: {}", report["profile"].as_str().unwrap_or("-"));
        println!(
            "logs_count: {}",
            report["summary"]["logs_count"].as_u64().unwrap_or(0)
        );
        println!(
            "system_events_count: {}",
            report["summary"]["system_events_count"]
                .as_u64()
                .unwrap_or(0)
        );
        println!(
            "gateway_running: {}",
            report["summary"]["gateway_running"]
                .as_bool()
                .unwrap_or(false)
        );
        println!(
            "gateway_endpoint_healthy: {}",
            report["summary"]["gateway_endpoint_healthy"]
                .as_bool()
                .unwrap_or(false)
        );
        println!(
            "channels_total: {}",
            report["summary"]["channels_total"].as_u64().unwrap_or(0)
        );
        println!(
            "channel_events_count: {}",
            report["summary"]["channel_events_count"]
                .as_u64()
                .unwrap_or(0)
        );
        println!(
            "channel_failed_events: {}",
            report["summary"]["channel_failed_events"]
                .as_u64()
                .unwrap_or(0)
        );
        println!(
            "alerts_total: {}",
            report["summary"]["alerts_total"].as_u64().unwrap_or(0)
        );
        println!(
            "alerts_warning: {}",
            report["summary"]["alerts_warning"].as_u64().unwrap_or(0)
        );
        println!(
            "alerts_critical: {}",
            report["summary"]["alerts_critical"].as_u64().unwrap_or(0)
        );
        println!(
            "alerts_suppressed: {}",
            report["summary"]["alerts_suppressed"].as_u64().unwrap_or(0)
        );
        println!(
            "slo_gateway_met: {}",
            report["summary"]["slo_gateway_met"]
                .as_bool()
                .unwrap_or(false)
        );
        println!(
            "slo_channels_met: {}",
            report["summary"]["slo_channels_met"]
                .as_bool()
                .unwrap_or(false)
        );
        println!(
            "slo_history_count: {}",
            report["summary"]["slo_history_count"].as_u64().unwrap_or(0)
        );
        println!(
            "slo_gateway_unmet_streak: {}",
            report["summary"]["slo_gateway_unmet_streak"]
                .as_u64()
                .unwrap_or(0)
        );
        println!(
            "slo_channels_unmet_streak: {}",
            report["summary"]["slo_channels_unmet_streak"]
                .as_u64()
                .unwrap_or(0)
        );
        println!(
            "slo_incident_hints: {}",
            report["summary"]["slo_incident_hints"]
                .as_u64()
                .unwrap_or(0)
        );
        println!(
            "safety_compare_available: {}",
            report["summary"]["safety_compare_available"]
                .as_bool()
                .unwrap_or(false)
        );
        if report["filters"]["compare_window"].as_u64().unwrap_or(0) > 0 {
            println!(
                "safety_failed_delta: {}",
                report["summary"]["safety_failed_delta"]
                    .as_i64()
                    .unwrap_or(0)
            );
            println!(
                "safety_failure_rate_delta: {:.4}",
                report["summary"]["safety_failure_rate_delta"]
                    .as_f64()
                    .unwrap_or(0.0)
            );
        }
        println!(
            "doctor_included: {}",
            report["summary"]["doctor_included"]
                .as_bool()
                .unwrap_or(false)
        );
        println!(
            "plugin_soak_available: {}",
            report["summary"]["plugin_soak_available"]
                .as_bool()
                .unwrap_or(false)
        );
        println!(
            "plugin_soak_history_count: {}",
            report["summary"]["plugin_soak_history_count"]
                .as_u64()
                .unwrap_or(0)
        );
        println!(
            "plugin_soak_history_pruned: {}",
            report["summary"]["plugin_soak_history_pruned"]
                .as_u64()
                .unwrap_or(0)
        );
        println!(
            "plugin_soak_delta_available: {}",
            report["summary"]["plugin_soak_delta_available"]
                .as_bool()
                .unwrap_or(false)
        );
        println!(
            "plugin_soak_incident_hints: {}",
            report["summary"]["plugin_soak_incident_hints"]
                .as_u64()
                .unwrap_or(0)
        );
        if report["summary"]["plugin_soak_available"]
            .as_bool()
            .unwrap_or(false)
        {
            println!(
                "plugin_soak_status: {}",
                report["summary"]["plugin_soak_status"]
                    .as_str()
                    .unwrap_or("unknown")
            );
            println!(
                "plugin_soak_completion_ratio: {:.4}",
                report["plugin_soak"]["trend"]["completion_ratio"]
                    .as_f64()
                    .unwrap_or(0.0)
            );
            if report["summary"]["plugin_soak_delta_available"]
                .as_bool()
                .unwrap_or(false)
            {
                println!(
                    "plugin_soak_completion_ratio_delta: {:.4}",
                    report["summary"]["plugin_soak_completion_ratio_delta"]
                        .as_f64()
                        .unwrap_or(0.0)
                );
            }
            println!(
                "plugin_soak_completion_unmet_streak: {}",
                report["summary"]["plugin_soak_completion_unmet_streak"]
                    .as_u64()
                    .unwrap_or(0)
            );
            println!(
                "plugin_soak_status_unmet_streak: {}",
                report["summary"]["plugin_soak_status_unmet_streak"]
                    .as_u64()
                    .unwrap_or(0)
            );
        }
        if report["summary"]["doctor_included"]
            .as_bool()
            .unwrap_or(false)
        {
            println!(
                "doctor_ok: {}",
                report["summary"]["doctor_ok"].as_u64().unwrap_or(0)
            );
            println!(
                "doctor_warn: {}",
                report["summary"]["doctor_warn"].as_u64().unwrap_or(0)
            );
        }
        if let Some(path) = export_path.as_ref() {
            println!("export_path: {}", path.display());
        }
    }

    Ok(())
}

async fn build_observability_report(
    cli: &Cli,
    options: ObservabilityReportOptions,
) -> Result<Value> {
    let paths = resolve_state_paths(cli.project_state)?;
    paths.ensure_dirs()?;

    let source_filter = options.source.as_deref();
    let logs = filter_logs(collect_logs(&paths.data_dir, options.tail)?, source_filter);

    let events_store = SystemEventStore::new(system_events_path(&paths.data_dir));
    let mut system_events = events_store.read_tail(options.event_tail)?;
    if let Some(name_filter) = options.event_name.as_deref() {
        system_events.retain(|event| event.name == name_filter);
    }

    let approvals_store = ApprovalStore::new(paths.approvals_policy_path.clone());
    let approvals_policy = approvals_store.load_or_default()?;
    let sandbox_store = SandboxStore::new(paths.sandbox_policy_path.clone());
    let sandbox_policy = sandbox_store.load_or_default()?;
    let safety_audit = build_safety_audit(
        &paths.audit_log_path,
        options.audit_tail,
        options.compare_window,
        &approvals_policy,
        sandbox_policy.profile,
    )?;
    let mut plugin_soak =
        build_plugin_soak_report(options.plugin_soak_report.as_deref(), &paths.root_dir);
    let plugin_soak_history = build_plugin_soak_history(&plugin_soak, &paths.data_dir);
    if let Value::Object(object) = &mut plugin_soak {
        object.insert("history".to_string(), plugin_soak_history.clone());
    }
    let gateway = build_gateway_observability(
        &paths.data_dir.join("gateway.json"),
        &paths.data_dir.join("gateway-service.json"),
    )
    .await;
    let channels = build_channels_observability(&paths.data_dir, options.tail);
    let alerts = build_observability_alerts(&gateway, &channels, &safety_audit, &plugin_soak);
    let mut slo = build_observability_slo(&gateway, &channels);
    let slo_history = build_observability_slo_history(&slo, &alerts, &paths.data_dir);
    if let Value::Object(object) = &mut slo {
        object.insert("history".to_string(), slo_history.clone());
    }

    let (doctor_checks, doctor_ok, doctor_warn) = if options.include_doctor {
        let checks = super::diagnostics_command::collect_doctor_checks(cli).await?;
        let ok = checks
            .iter()
            .filter(|check| check.get("status").and_then(Value::as_str) == Some("ok"))
            .count();
        let warn = checks.len().saturating_sub(ok);
        (Some(checks), ok, warn)
    } else {
        (None, 0, 0)
    };

    Ok(json!({
        "generated_at": chrono::Utc::now().to_rfc3339(),
        "profile": cli.profile,
        "state_mode": format!("{:?}", paths.mode).to_lowercase(),
        "filters": {
            "logs_tail": options.tail,
            "logs_source": options.source,
            "events_tail": options.event_tail,
            "audit_tail": options.audit_tail,
            "compare_window": options.compare_window,
            "event_name": options.event_name,
            "include_doctor": options.include_doctor,
            "plugin_soak_report": options
                .plugin_soak_report
                .as_ref()
                .map(|path| path.display().to_string()),
        },
        "paths": {
            "data_dir": paths.data_dir.display().to_string(),
            "system_events": events_store.path().display().to_string(),
            "approvals_policy": approvals_store.path().display().to_string(),
            "sandbox_policy": sandbox_store.path().display().to_string(),
            "gateway_state": paths.data_dir.join("gateway.json").display().to_string(),
            "gateway_service": paths.data_dir.join("gateway-service.json").display().to_string(),
            "channels_file": channels_file_path(&paths.data_dir).display().to_string(),
            "channel_events_dir": channels_events_dir(&paths.data_dir).display().to_string(),
        },
        "summary": {
            "logs_count": logs.len(),
            "system_events_count": system_events.len(),
            "gateway_running": gateway["running"],
            "gateway_endpoint_healthy": gateway["endpoint_healthy"],
            "channels_total": channels["summary"]["total_channels"],
            "channels_with_errors": channels["summary"]["channels_with_errors"],
            "channel_events_count": channels["summary"]["event_count"],
            "channel_failed_events": channels["summary"]["failed_events"],
            "alerts_total": alerts["total"],
            "alerts_warning": alerts["warning"],
            "alerts_critical": alerts["critical"],
            "alerts_suppressed": alerts["suppressed"]["count"],
            "safety_audit_entries": safety_audit["summary"]["total_entries"],
            "safety_audit_parse_errors": safety_audit["summary"]["parse_errors"],
            "safety_compare_available": safety_audit["comparison"]["available"],
            "safety_failed_delta": safety_audit["comparison"]["delta"]["failed"],
            "safety_failure_rate_delta": safety_audit["comparison"]["delta"]["failure_rate"],
            "slo_gateway_met": slo["gateway"]["met"],
            "slo_channels_met": slo["channels"]["met"],
            "slo_history_count": slo_history["sample_count"],
            "slo_gateway_ratio_delta": slo_history["current_vs_previous"]["delta"]["gateway_ratio"],
            "slo_channels_ratio_delta": slo_history["current_vs_previous"]["delta"]["channels_ratio"],
            "slo_gateway_unmet_streak": slo_history["streaks"]["gateway_unmet"],
            "slo_channels_unmet_streak": slo_history["streaks"]["channels_unmet"],
            "slo_incident_hints": slo_history["incident_hints_count"],
            "doctor_included": options.include_doctor,
            "doctor_ok": doctor_ok,
            "doctor_warn": doctor_warn,
            "plugin_soak_available": plugin_soak["available"],
            "plugin_soak_status": plugin_soak["status"],
            "plugin_soak_history_count": plugin_soak_history["sample_count"],
            "plugin_soak_history_pruned": plugin_soak_history["retention"]["pruned"],
            "plugin_soak_delta_available": plugin_soak_history["current_vs_previous"]["available"],
            "plugin_soak_completion_ratio_delta": plugin_soak_history["current_vs_previous"]["delta"]["completion_ratio"],
            "plugin_soak_incident_hints": plugin_soak_history["incident_hints_count"],
            "plugin_soak_completion_unmet_streak": plugin_soak_history["streaks"]["completion_unmet"],
            "plugin_soak_status_unmet_streak": plugin_soak_history["streaks"]["status_not_ok"],
        },
        "policy": {
            "approvals_mode": approvals_policy.mode,
            "approvals_allowlist_size": approvals_policy.allowlist.len(),
            "sandbox_profile": sandbox_policy.profile,
        },
        "logs": logs,
        "system_events": system_events,
        "gateway": gateway,
        "channels": channels,
        "alerts": alerts,
        "slo": slo,
        "safety_audit": safety_audit,
        "plugin_soak": plugin_soak,
        "doctor": {
            "checks": doctor_checks,
        },
    }))
}

fn error_to_json(err: &MosaicError) -> Value {
    json!({
        "code": err.code(),
        "message": err.to_string(),
        "exit_code": err.exit_code(),
    })
}

async fn build_gateway_observability(gateway_path: &Path, gateway_service_path: &Path) -> Value {
    match collect_gateway_runtime_status(gateway_path, gateway_service_path).await {
        Ok(status) => json!({
            "ok": true,
            "running": status.running,
            "process_alive": status.process_alive,
            "endpoint_healthy": status.endpoint_healthy,
            "target": {
                "host": status.target_host,
                "port": status.target_port,
            },
            "state": status.state,
            "service": status.service,
            "paths": {
                "state_file": gateway_path.display().to_string(),
                "service_file": gateway_service_path.display().to_string(),
            },
            "error": Value::Null,
        }),
        Err(err) => json!({
            "ok": false,
            "running": false,
            "process_alive": false,
            "endpoint_healthy": false,
            "target": Value::Null,
            "state": Value::Null,
            "service": Value::Null,
            "paths": {
                "state_file": gateway_path.display().to_string(),
                "service_file": gateway_service_path.display().to_string(),
            },
            "error": error_to_json(&err),
        }),
    }
}

fn build_channels_observability(data_dir: &Path, tail: usize) -> Value {
    let channels_path = channels_file_path(data_dir);
    let events_dir = channels_events_dir(data_dir);
    let repository = ChannelRepository::new(channels_path.clone(), events_dir.clone());

    let (status, status_error) = match repository.status() {
        Ok(value) => (Some(value), None),
        Err(err) => (None, Some(error_to_json(&err))),
    };
    let (events, events_error) = match repository.logs(None, tail) {
        Ok(value) => (Some(value), None),
        Err(err) => (None, Some(error_to_json(&err))),
    };

    let mut delivery_status_counts = BTreeMap::<String, usize>::new();
    let mut event_kind_counts = BTreeMap::<String, usize>::new();
    let mut http_status_counts = BTreeMap::<String, usize>::new();
    let mut failed_events = 0usize;
    let mut deduplicated_events = 0usize;
    let mut probe_events = 0usize;
    let mut max_attempt = 0usize;
    let mut latest_event_ts = None::<String>;

    let recent_events = events
        .as_ref()
        .map(|items| {
            for event in items {
                *delivery_status_counts
                    .entry(event.delivery_status.clone())
                    .or_default() += 1;
                *event_kind_counts.entry(event.kind.clone()).or_default() += 1;
                if let Some(http_status) = event.http_status {
                    *http_status_counts
                        .entry(http_status.to_string())
                        .or_default() += 1;
                }
                if event.delivery_status == "failed" {
                    failed_events += 1;
                }
                if event.deduplicated {
                    deduplicated_events += 1;
                }
                if event.kind == "test_probe" {
                    probe_events += 1;
                }
                max_attempt = max_attempt.max(event.attempt);
                latest_event_ts = Some(event.ts.to_rfc3339());
            }
            items
                .iter()
                .rev()
                .take(10)
                .map(|event| {
                    json!({
                        "ts": event.ts,
                        "channel_id": event.channel_id,
                        "kind": event.kind,
                        "delivery_status": event.delivery_status,
                        "attempt": event.attempt,
                        "http_status": event.http_status,
                        "error": event.error,
                        "rate_limited_ms": event.rate_limited_ms,
                        "deduplicated": event.deduplicated,
                    })
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    json!({
        "ok": status_error.is_none() && events_error.is_none(),
        "tail": tail,
        "paths": {
            "channels_file": channels_path.display().to_string(),
            "events_dir": events_dir.display().to_string(),
        },
        "status": status,
        "summary": {
            "total_channels": status.as_ref().map_or(0usize, |value| value.total_channels),
            "healthy_channels": status.as_ref().map_or(0usize, |value| value.healthy_channels),
            "channels_with_errors": status.as_ref().map_or(0usize, |value| value.channels_with_errors),
            "event_count": events.as_ref().map_or(0usize, |value| value.len()),
            "failed_events": failed_events,
            "deduplicated_events": deduplicated_events,
            "probe_events": probe_events,
            "max_attempt": max_attempt,
            "latest_event_ts": latest_event_ts,
            "delivery_status": delivery_status_counts,
            "event_kinds": event_kind_counts,
            "http_status": http_status_counts,
        },
        "recent_events": recent_events,
        "errors": {
            "status": status_error,
            "events": events_error,
        },
    })
}

fn build_observability_alerts(
    gateway: &Value,
    channels: &Value,
    safety_audit: &Value,
    plugin_soak: &Value,
) -> Value {
    let channel_failure_warn_threshold =
        read_alert_threshold("MOSAIC_OBS_ALERT_CHANNEL_FAILURE_WARN", 0.1);
    let channel_failure_critical_threshold = read_alert_threshold(
        "MOSAIC_OBS_ALERT_CHANNEL_FAILURE_CRITICAL",
        channel_failure_warn_threshold.max(0.5),
    )
    .max(channel_failure_warn_threshold);
    let safety_failure_warn_threshold =
        read_alert_threshold("MOSAIC_OBS_ALERT_SAFETY_FAILURE_WARN", 0.25);
    let safety_failure_critical_threshold = read_alert_threshold(
        "MOSAIC_OBS_ALERT_SAFETY_FAILURE_CRITICAL",
        safety_failure_warn_threshold.max(0.5),
    )
    .max(safety_failure_warn_threshold);
    let plugin_completion_ratio_min =
        read_alert_threshold("MOSAIC_OBS_ALERT_PLUGIN_COMPLETION_MIN", 1.0).clamp(0.0, 1.0);
    let min_severity = read_alert_min_severity();
    let suppress_ids = read_suppressed_alert_ids();

    let mut items = Vec::<Value>::new();

    let gateway_running = gateway
        .get("running")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let gateway_endpoint_healthy = gateway
        .get("endpoint_healthy")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    if !gateway_running {
        items.push(json!({
            "id": "gateway.not_running",
            "severity": "warning",
            "message": "gateway runtime is not running",
            "metric": "gateway_running",
            "value": gateway_running,
            "threshold": true,
        }));
    } else if !gateway_endpoint_healthy {
        items.push(json!({
            "id": "gateway.endpoint_unhealthy",
            "severity": "warning",
            "message": "gateway process is running but health endpoint is not healthy",
            "metric": "gateway_endpoint_healthy",
            "value": gateway_endpoint_healthy,
            "threshold": true,
        }));
    }

    let channel_event_count = channels["summary"]["event_count"].as_u64().unwrap_or(0);
    let channel_failed_events = channels["summary"]["failed_events"].as_u64().unwrap_or(0);
    if channel_event_count > 0 && channel_failed_events > 0 {
        let failure_rate = (channel_failed_events as f64) / (channel_event_count as f64);
        let severity = if failure_rate >= channel_failure_critical_threshold {
            "critical"
        } else {
            "warning"
        };
        if failure_rate >= channel_failure_warn_threshold {
            items.push(json!({
                "id": "channels.delivery_failures",
                "severity": severity,
                "message": format!("channel delivery failures detected in recent tail (failed={channel_failed_events} / total={channel_event_count})"),
                "metric": "channel_failure_rate",
                "value": failure_rate,
                "threshold": {
                    "warning": channel_failure_warn_threshold,
                    "critical": channel_failure_critical_threshold,
                },
            }));
        }
    }

    let safety_total = safety_audit["summary"]["total_entries"]
        .as_u64()
        .unwrap_or(0);
    let safety_failed = safety_audit["summary"]["failed"].as_u64().unwrap_or(0);
    if safety_total > 0 {
        let failure_rate = (safety_failed as f64) / (safety_total as f64);
        if failure_rate >= safety_failure_warn_threshold {
            let severity = if failure_rate >= safety_failure_critical_threshold {
                "critical"
            } else {
                "warning"
            };
            items.push(json!({
                "id": "safety.failure_rate_high",
                "severity": severity,
                "message": format!("safety audit failure rate is elevated (failed={safety_failed} / total={safety_total})"),
                "metric": "safety_failure_rate",
                "value": failure_rate,
                "threshold": {
                    "warning": safety_failure_warn_threshold,
                    "critical": safety_failure_critical_threshold,
                },
            }));
        }
    }

    let plugin_soak_available = plugin_soak
        .get("available")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    if plugin_soak_available {
        let plugin_soak_status = plugin_soak
            .get("status")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        if plugin_soak_status != "ok" {
            items.push(json!({
                "id": "plugin_soak.status_warn",
                "severity": "warning",
                "message": format!("plugin soak status is {plugin_soak_status}"),
                "metric": "plugin_soak_status",
                "value": plugin_soak_status,
                "threshold": "ok",
            }));
        }
        let completion_ratio = plugin_soak["trend"]["completion_ratio"]
            .as_f64()
            .unwrap_or(0.0);
        if completion_ratio < plugin_completion_ratio_min {
            items.push(json!({
                "id": "plugin_soak.incomplete",
                "severity": "warning",
                "message": format!("plugin soak completion ratio is below expected (ratio={completion_ratio:.4})"),
                "metric": "plugin_soak_completion_ratio",
                "value": completion_ratio,
                "threshold": plugin_completion_ratio_min,
            }));
        }
    }

    let mut visible_items = Vec::<Value>::new();
    let mut suppressed_items = Vec::<Value>::new();
    let mut suppressed_by_id = 0usize;
    let mut suppressed_by_severity = 0usize;
    for mut item in items {
        let id = item
            .get("id")
            .and_then(Value::as_str)
            .map(str::to_string)
            .unwrap_or_default();
        let severity_value = item
            .get("severity")
            .and_then(Value::as_str)
            .unwrap_or("warning");
        let suppress_reason = if !id.is_empty() && suppress_ids.contains(&id) {
            suppressed_by_id += 1;
            Some("id_match")
        } else if alert_severity_rank(severity_value) < alert_severity_rank(min_severity.as_str()) {
            suppressed_by_severity += 1;
            Some("min_severity")
        } else {
            None
        };

        if let Some(reason) = suppress_reason {
            if let Value::Object(object) = &mut item {
                object.insert(
                    "suppressed_reason".to_string(),
                    Value::String(reason.to_string()),
                );
            }
            suppressed_items.push(item);
        } else {
            visible_items.push(item);
        }
    }

    let critical = visible_items
        .iter()
        .filter(|item| item.get("severity").and_then(Value::as_str) == Some("critical"))
        .count();
    let warning = visible_items
        .iter()
        .filter(|item| item.get("severity").and_then(Value::as_str) == Some("warning"))
        .count();
    let suppress_id_list = suppress_ids.into_iter().collect::<Vec<_>>();

    json!({
        "total": visible_items.len(),
        "warning": warning,
        "critical": critical,
        "min_severity": min_severity.as_str(),
        "suppress_ids": suppress_id_list,
        "suppressed": {
            "count": suppressed_items.len(),
            "by_reason": {
                "id_match": suppressed_by_id,
                "min_severity": suppressed_by_severity,
            },
            "items": suppressed_items,
        },
        "thresholds": {
            "channel_failure_warn": channel_failure_warn_threshold,
            "channel_failure_critical": channel_failure_critical_threshold,
            "safety_failure_warn": safety_failure_warn_threshold,
            "safety_failure_critical": safety_failure_critical_threshold,
            "plugin_completion_min": plugin_completion_ratio_min,
        },
        "items": visible_items,
    })
}

fn read_alert_threshold(env_key: &str, default_value: f64) -> f64 {
    std::env::var(env_key)
        .ok()
        .and_then(|raw| raw.trim().parse::<f64>().ok())
        .filter(|value| value.is_finite() && *value >= 0.0)
        .unwrap_or(default_value)
}

#[derive(Debug, Clone, Copy)]
enum AlertSeverity {
    Warning,
    Critical,
}

impl AlertSeverity {
    fn as_str(self) -> &'static str {
        match self {
            Self::Warning => "warning",
            Self::Critical => "critical",
        }
    }
}

fn alert_severity_rank(value: &str) -> u8 {
    match value {
        "critical" => 2,
        "warning" => 1,
        _ => 0,
    }
}

fn read_alert_min_severity() -> AlertSeverity {
    match std::env::var("MOSAIC_OBS_ALERT_MIN_SEVERITY")
        .ok()
        .map(|raw| raw.trim().to_ascii_lowercase())
        .as_deref()
    {
        Some("critical") => AlertSeverity::Critical,
        _ => AlertSeverity::Warning,
    }
}

fn read_suppressed_alert_ids() -> BTreeSet<String> {
    std::env::var("MOSAIC_OBS_ALERT_SUPPRESS_IDS")
        .ok()
        .map(|raw| {
            raw.split(',')
                .map(|value| value.trim())
                .filter(|value| !value.is_empty())
                .map(str::to_string)
                .collect::<BTreeSet<_>>()
        })
        .unwrap_or_default()
}

fn build_observability_slo(gateway: &Value, channels: &Value) -> Value {
    let window = read_positive_usize("MOSAIC_OBS_SLO_WINDOW", 20);
    let gateway_target = read_ratio_threshold("MOSAIC_OBS_SLO_GATEWAY_TARGET", 0.99);
    let channels_target = read_ratio_threshold("MOSAIC_OBS_SLO_CHANNELS_TARGET", 0.99);

    let gateway_healthy = gateway
        .get("running")
        .and_then(Value::as_bool)
        .unwrap_or(false)
        && gateway
            .get("endpoint_healthy")
            .and_then(Value::as_bool)
            .unwrap_or(false);
    let gateway_samples = 1usize;
    let gateway_healthy_samples = if gateway_healthy { 1usize } else { 0usize };
    let gateway_ratio = (gateway_healthy_samples as f64) / (gateway_samples as f64);
    let gateway_met = gateway_ratio >= gateway_target;

    let channels_total = channels["summary"]["event_count"].as_u64().unwrap_or(0) as usize;
    let channels_failed = channels["summary"]["failed_events"].as_u64().unwrap_or(0) as usize;
    let channels_success = channels_total.saturating_sub(channels_failed);
    let channels_available = channels_total > 0;
    let channels_samples = channels_total.min(window);
    let channels_ratio = if channels_total == 0 {
        1.0
    } else {
        (channels_success as f64) / (channels_total as f64)
    };
    let channels_met = if channels_available {
        channels_ratio >= channels_target
    } else {
        true
    };

    json!({
        "window": window,
        "gateway": {
            "target": gateway_target,
            "samples": gateway_samples,
            "healthy_samples": gateway_healthy_samples,
            "ratio": gateway_ratio,
            "met": gateway_met,
        },
        "channels": {
            "target": channels_target,
            "available": channels_available,
            "samples": channels_samples,
            "successful_samples": channels_success,
            "ratio": channels_ratio,
            "met": channels_met,
        },
    })
}

fn read_positive_usize(env_key: &str, default_value: usize) -> usize {
    std::env::var(env_key)
        .ok()
        .and_then(|raw| raw.trim().parse::<usize>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(default_value)
}

fn read_ratio_threshold(env_key: &str, default_value: f64) -> f64 {
    read_alert_threshold(env_key, default_value).clamp(0.0, 1.0)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ObservabilitySloHistoryEntry {
    ts: String,
    gateway_ratio: f64,
    gateway_met: bool,
    channels_ratio: f64,
    channels_met: bool,
    channels_available: bool,
    alert_ids: Vec<String>,
}

fn slo_history_path(data_dir: &Path) -> PathBuf {
    data_dir
        .join("reports")
        .join("observability-slo-history.jsonl")
}

fn read_slo_history(
    history_path: &Path,
) -> (Vec<ObservabilitySloHistoryEntry>, usize, Option<String>) {
    if !history_path.exists() {
        return (Vec::new(), 0, None);
    }
    let raw = match std::fs::read_to_string(history_path) {
        Ok(value) => value,
        Err(err) => {
            return (
                Vec::new(),
                0,
                Some(format!(
                    "failed to read slo history '{}': {err}",
                    history_path.display()
                )),
            );
        }
    };
    let mut entries = Vec::new();
    let mut parse_errors = 0usize;
    for line in raw.lines() {
        if line.trim().is_empty() {
            continue;
        }
        match serde_json::from_str::<ObservabilitySloHistoryEntry>(line) {
            Ok(entry) => entries.push(entry),
            Err(_) => parse_errors += 1,
        }
    }
    (entries, parse_errors, None)
}

fn append_slo_history_entry(
    history_path: &Path,
    entry: &ObservabilitySloHistoryEntry,
) -> std::result::Result<(), String> {
    if let Some(parent) = history_path.parent() {
        std::fs::create_dir_all(parent).map_err(|err| {
            format!(
                "failed to create slo history directory '{}': {err}",
                parent.display()
            )
        })?;
    }
    let mut rendered = serde_json::to_string(entry)
        .map_err(|err| format!("failed to serialize slo history entry: {err}"))?;
    rendered.push('\n');
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(history_path)
        .map_err(|err| {
            format!(
                "failed to open slo history file '{}': {err}",
                history_path.display()
            )
        })?;
    file.write_all(rendered.as_bytes()).map_err(|err| {
        format!(
            "failed to write slo history file '{}': {err}",
            history_path.display()
        )
    })?;
    Ok(())
}

fn rewrite_slo_history_entries(
    history_path: &Path,
    entries: &[ObservabilitySloHistoryEntry],
) -> std::result::Result<(), String> {
    if let Some(parent) = history_path.parent() {
        std::fs::create_dir_all(parent).map_err(|err| {
            format!(
                "failed to create slo history directory '{}': {err}",
                parent.display()
            )
        })?;
    }
    let mut rendered = String::new();
    for entry in entries {
        let line = serde_json::to_string(entry)
            .map_err(|err| format!("failed to serialize slo history entry: {err}"))?;
        rendered.push_str(&line);
        rendered.push('\n');
    }
    std::fs::write(history_path, rendered).map_err(|err| {
        format!(
            "failed to rewrite slo history file '{}': {err}",
            history_path.display()
        )
    })?;
    Ok(())
}

fn slo_history_entry_from_report(
    slo: &Value,
    alerts: &Value,
) -> std::result::Result<ObservabilitySloHistoryEntry, String> {
    let gateway = slo
        .get("gateway")
        .ok_or_else(|| "missing slo.gateway".to_string())?;
    let channels = slo
        .get("channels")
        .ok_or_else(|| "missing slo.channels".to_string())?;
    let alert_ids = alerts
        .get("items")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.get("id").and_then(Value::as_str).map(str::to_string))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    let extract_f64 = |container: &Value, key: &str| -> std::result::Result<f64, String> {
        container
            .get(key)
            .and_then(Value::as_f64)
            .ok_or_else(|| format!("missing slo field '{key}'"))
    };
    let extract_bool = |container: &Value, key: &str| -> std::result::Result<bool, String> {
        container
            .get(key)
            .and_then(Value::as_bool)
            .ok_or_else(|| format!("missing slo field '{key}'"))
    };

    Ok(ObservabilitySloHistoryEntry {
        ts: chrono::Utc::now().to_rfc3339(),
        gateway_ratio: extract_f64(gateway, "ratio")?,
        gateway_met: extract_bool(gateway, "met")?,
        channels_ratio: extract_f64(channels, "ratio")?,
        channels_met: extract_bool(channels, "met")?,
        channels_available: extract_bool(channels, "available")?,
        alert_ids,
    })
}

fn slo_history_entry_json(entry: &ObservabilitySloHistoryEntry) -> Value {
    json!({
        "ts": entry.ts,
        "gateway_ratio": entry.gateway_ratio,
        "gateway_met": entry.gateway_met,
        "channels_ratio": entry.channels_ratio,
        "channels_met": entry.channels_met,
        "channels_available": entry.channels_available,
        "alert_ids": entry.alert_ids,
    })
}

fn slo_history_delta(
    current: &ObservabilitySloHistoryEntry,
    previous: &ObservabilitySloHistoryEntry,
) -> Value {
    json!({
        "gateway_ratio": current.gateway_ratio - previous.gateway_ratio,
        "channels_ratio": current.channels_ratio - previous.channels_ratio,
        "gateway_met_changed": current.gateway_met != previous.gateway_met,
        "channels_met_changed": current.channels_met != previous.channels_met,
    })
}

fn empty_slo_history_delta() -> Value {
    json!({
        "gateway_ratio": 0.0,
        "channels_ratio": 0.0,
        "gateway_met_changed": false,
        "channels_met_changed": false,
    })
}

fn count_gateway_unmet_streak(entries: &[ObservabilitySloHistoryEntry]) -> usize {
    entries
        .iter()
        .rev()
        .take_while(|entry| !entry.gateway_met)
        .count()
}

fn count_channels_unmet_streak(entries: &[ObservabilitySloHistoryEntry]) -> usize {
    entries
        .iter()
        .rev()
        .take_while(|entry| entry.channels_available && !entry.channels_met)
        .count()
}

fn count_repeated_alert_streak(entries: &[ObservabilitySloHistoryEntry], alert_id: &str) -> usize {
    entries
        .iter()
        .rev()
        .take_while(|entry| entry.alert_ids.iter().any(|id| id == alert_id))
        .count()
}

fn build_slo_incident_hints(
    entries: &[ObservabilitySloHistoryEntry],
    current: Option<&ObservabilitySloHistoryEntry>,
    incident_window: usize,
    repeat_threshold: usize,
) -> Vec<Value> {
    let mut hints = Vec::<Value>::new();
    let gateway_unmet_streak = count_gateway_unmet_streak(entries);
    if gateway_unmet_streak >= incident_window {
        hints.push(json!({
            "id": "slo.gateway_unmet_streak",
            "severity": "warning",
            "message": format!("gateway SLO has been unmet for {gateway_unmet_streak} consecutive samples"),
            "streak": gateway_unmet_streak,
            "threshold": incident_window,
        }));
    }

    let channels_unmet_streak = count_channels_unmet_streak(entries);
    if channels_unmet_streak >= incident_window {
        hints.push(json!({
            "id": "slo.channels_unmet_streak",
            "severity": "warning",
            "message": format!("channels SLO has been unmet for {channels_unmet_streak} consecutive samples"),
            "streak": channels_unmet_streak,
            "threshold": incident_window,
        }));
    }

    if let Some(current_entry) = current {
        for alert_id in &current_entry.alert_ids {
            let repeat_count = count_repeated_alert_streak(entries, alert_id);
            if repeat_count >= repeat_threshold {
                hints.push(json!({
                    "id": "alerts.repeated",
                    "severity": "info",
                    "message": format!("alert '{alert_id}' repeated for {repeat_count} consecutive samples"),
                    "alert_id": alert_id,
                    "streak": repeat_count,
                    "threshold": repeat_threshold,
                }));
            }
        }
    }

    hints
}

fn build_observability_slo_history(slo: &Value, alerts: &Value, data_dir: &Path) -> Value {
    let history_path = slo_history_path(data_dir);
    let max_samples = read_positive_usize("MOSAIC_OBS_SLO_HISTORY_MAX_SAMPLES", 500);
    let incident_window = read_positive_usize("MOSAIC_OBS_SLO_INCIDENT_WINDOW", 5);
    let repeat_threshold = read_positive_usize("MOSAIC_OBS_ALERT_REPEAT_HINT_THRESHOLD", 3);
    let (mut entries, parse_errors, read_error) = read_slo_history(&history_path);
    let mut appended = false;
    let mut write_error = None::<String>;
    let mut pruned = 0usize;

    let current_entry = match slo_history_entry_from_report(slo, alerts) {
        Ok(entry) => {
            if let Err(err) = append_slo_history_entry(&history_path, &entry) {
                write_error = Some(err);
            } else {
                appended = true;
                entries.push(entry.clone());
            }
            Some(entry)
        }
        Err(err) => {
            write_error = Some(err);
            None
        }
    };

    let current_vs_previous = if let Some(current) = current_entry.as_ref() {
        let previous = if appended {
            if entries.len() >= 2 {
                entries.get(entries.len() - 2).cloned()
            } else {
                None
            }
        } else {
            entries.last().cloned()
        };

        if let Some(previous) = previous {
            json!({
                "available": true,
                "current_ts": current.ts,
                "previous_ts": previous.ts,
                "delta": slo_history_delta(current, &previous),
            })
        } else {
            json!({
                "available": false,
                "current_ts": current.ts,
                "previous_ts": null,
                "delta": empty_slo_history_delta(),
            })
        }
    } else {
        json!({
            "available": false,
            "current_ts": null,
            "previous_ts": null,
            "delta": empty_slo_history_delta(),
        })
    };

    if entries.len() > max_samples {
        pruned = entries.len() - max_samples;
        entries.drain(0..pruned);
        if let Err(err) = rewrite_slo_history_entries(&history_path, &entries)
            && write_error.is_none()
        {
            write_error = Some(err);
        }
    }

    let incident_hints = build_slo_incident_hints(
        &entries,
        current_entry.as_ref(),
        incident_window,
        repeat_threshold,
    );
    let gateway_unmet_streak = count_gateway_unmet_streak(&entries);
    let channels_unmet_streak = count_channels_unmet_streak(&entries);
    let latest = entries.last().cloned();

    json!({
        "path": history_path.display().to_string(),
        "sample_count": entries.len(),
        "retention": {
            "max_samples": max_samples,
            "pruned": pruned,
        },
        "parse_errors": parse_errors,
        "appended": appended,
        "read_error": read_error,
        "write_error": write_error,
        "latest": latest.as_ref().map(slo_history_entry_json),
        "current_run": current_entry.as_ref().map(slo_history_entry_json),
        "current_vs_previous": current_vs_previous,
        "streaks": {
            "gateway_unmet": gateway_unmet_streak,
            "channels_unmet": channels_unmet_streak,
        },
        "incident_window": incident_window,
        "alert_repeat_threshold": repeat_threshold,
        "incident_hints_count": incident_hints.len(),
        "incident_hints": incident_hints,
    })
}

#[derive(Default)]
struct PluginSoakRawMetrics {
    iterations: Option<u64>,
    ok_runs: Option<u64>,
    cpu_failures: Option<u64>,
    rss_failures: Option<u64>,
    event_lines_ok: Option<u64>,
    event_lines_cpuwatch: Option<u64>,
    event_lines_rss: Option<u64>,
    workspace: Option<String>,
}

struct ParsedPluginSoakReport {
    iterations: u64,
    ok_runs: u64,
    cpu_failures: u64,
    rss_failures: u64,
    event_lines_ok: u64,
    event_lines_cpuwatch: u64,
    event_lines_rss: u64,
    workspace: Option<String>,
}

fn build_plugin_soak_report(explicit_path: Option<&Path>, state_root: &Path) -> Value {
    let path = resolve_plugin_soak_report_path(explicit_path, state_root);
    let source = if explicit_path.is_some() {
        "explicit"
    } else {
        "auto"
    };
    let Some(path) = path else {
        return json!({
            "available": false,
            "status": "missing",
            "source": source,
            "path": explicit_path.map(|value| value.display().to_string()),
            "error": "plugin soak report not found",
        });
    };
    let raw = match std::fs::read_to_string(&path) {
        Ok(value) => value,
        Err(err) => {
            return json!({
                "available": false,
                "status": "error",
                "source": source,
                "path": path.display().to_string(),
                "error": format!("failed to read plugin soak report: {err}"),
            });
        }
    };
    let parsed = match parse_plugin_soak_report(&raw) {
        Ok(value) => value,
        Err(err) => {
            let raw_tail = raw
                .lines()
                .rev()
                .take(5)
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .collect::<Vec<_>>();
            return json!({
                "available": false,
                "status": "error",
                "source": source,
                "path": path.display().to_string(),
                "error": err,
                "raw_tail": raw_tail,
            });
        }
    };

    let expected_total_runs = parsed.iterations.saturating_mul(3);
    let recorded_total_runs = parsed
        .ok_runs
        .saturating_add(parsed.cpu_failures)
        .saturating_add(parsed.rss_failures);
    let completion_ratio = if expected_total_runs == 0 {
        0.0
    } else {
        (recorded_total_runs as f64) / (expected_total_runs as f64)
    };
    let ok_success_rate = if parsed.iterations == 0 {
        0.0
    } else {
        (parsed.ok_runs as f64) / (parsed.iterations as f64)
    };
    let cpu_failure_rate = if parsed.iterations == 0 {
        0.0
    } else {
        (parsed.cpu_failures as f64) / (parsed.iterations as f64)
    };
    let rss_failure_rate = if parsed.iterations == 0 {
        0.0
    } else {
        (parsed.rss_failures as f64) / (parsed.iterations as f64)
    };
    let event_ok_drift = (parsed.event_lines_ok as i64) - (parsed.ok_runs as i64);
    let event_cpuwatch_drift = (parsed.event_lines_cpuwatch as i64) - (parsed.cpu_failures as i64);
    let event_rss_drift = (parsed.event_lines_rss as i64) - (parsed.rss_failures as i64);

    let mut warnings = Vec::<String>::new();
    if recorded_total_runs != expected_total_runs {
        warnings.push(format!(
            "recorded_total_runs={} differs from expected_total_runs={}",
            recorded_total_runs, expected_total_runs
        ));
    }
    if parsed.ok_runs != parsed.iterations {
        warnings.push(format!(
            "ok_runs={} differs from iterations={}",
            parsed.ok_runs, parsed.iterations
        ));
    }
    if parsed.cpu_failures != parsed.iterations {
        warnings.push(format!(
            "cpu_failures={} differs from iterations={}",
            parsed.cpu_failures, parsed.iterations
        ));
    }
    if parsed.rss_failures != parsed.iterations {
        warnings.push(format!(
            "rss_failures={} differs from iterations={}",
            parsed.rss_failures, parsed.iterations
        ));
    }
    if event_ok_drift != 0 {
        warnings.push(format!("event_lines.ok drift={event_ok_drift}"));
    }
    if event_cpuwatch_drift != 0 {
        warnings.push(format!("event_lines.cpuwatch drift={event_cpuwatch_drift}"));
    }
    if event_rss_drift != 0 {
        warnings.push(format!("event_lines.rss drift={event_rss_drift}"));
    }
    let status = if warnings.is_empty() { "ok" } else { "warn" };

    json!({
        "available": true,
        "status": status,
        "source": source,
        "path": path.display().to_string(),
        "summary": {
            "iterations": parsed.iterations,
            "ok_runs": parsed.ok_runs,
            "cpu_failures": parsed.cpu_failures,
            "rss_failures": parsed.rss_failures,
            "event_lines": {
                "ok": parsed.event_lines_ok,
                "cpuwatch": parsed.event_lines_cpuwatch,
                "rss": parsed.event_lines_rss,
            },
            "workspace": parsed.workspace,
        },
        "rates": {
            "ok_success_rate": ok_success_rate,
            "cpu_failure_rate": cpu_failure_rate,
            "rss_failure_rate": rss_failure_rate,
        },
        "trend": {
            "expected_total_runs": expected_total_runs,
            "recorded_total_runs": recorded_total_runs,
            "completion_ratio": completion_ratio,
            "event_line_drift": {
                "ok": event_ok_drift,
                "cpuwatch": event_cpuwatch_drift,
                "rss": event_rss_drift,
            },
        },
        "warnings": warnings,
    })
}

fn resolve_plugin_soak_report_path(
    explicit_path: Option<&Path>,
    state_root: &Path,
) -> Option<PathBuf> {
    if let Some(path) = explicit_path {
        return Some(path.to_path_buf());
    }

    let mut candidates = Vec::<PathBuf>::new();
    if let Ok(cwd) = std::env::current_dir() {
        candidates.push(cwd.join("cli/reports/plugin-soak-latest.log"));
        candidates.push(cwd.join("reports/plugin-soak-latest.log"));
    }
    if let Some(project_dir) = state_root.parent() {
        candidates.push(project_dir.join("cli/reports/plugin-soak-latest.log"));
        candidates.push(project_dir.join("reports/plugin-soak-latest.log"));
    }
    candidates.push(state_root.join("reports/plugin-soak-latest.log"));

    candidates.into_iter().find(|path| path.is_file())
}

fn parse_plugin_soak_report(raw: &str) -> std::result::Result<ParsedPluginSoakReport, String> {
    let mut metrics = PluginSoakRawMetrics::default();
    for line in raw.lines() {
        metrics.iterations = metrics
            .iterations
            .or_else(|| extract_u64_from_line(line, "iterations"));
        metrics.ok_runs = metrics
            .ok_runs
            .or_else(|| extract_u64_from_line(line, "ok_runs"));
        metrics.cpu_failures = metrics
            .cpu_failures
            .or_else(|| extract_u64_from_line(line, "cpu_failures"));
        metrics.rss_failures = metrics
            .rss_failures
            .or_else(|| extract_u64_from_line(line, "rss_failures"));
        metrics.event_lines_ok = metrics
            .event_lines_ok
            .or_else(|| extract_u64_from_line(line, "event_lines.ok"));
        metrics.event_lines_cpuwatch = metrics
            .event_lines_cpuwatch
            .or_else(|| extract_u64_from_line(line, "event_lines.cpuwatch"));
        metrics.event_lines_rss = metrics
            .event_lines_rss
            .or_else(|| extract_u64_from_line(line, "event_lines.rss"));
        metrics.workspace = metrics
            .workspace
            .or_else(|| extract_string_from_line(line, "workspace"));
    }

    let iterations = metrics
        .iterations
        .ok_or_else(|| "missing iterations in plugin soak report".to_string())?;
    let ok_runs = metrics
        .ok_runs
        .ok_or_else(|| "missing ok_runs in plugin soak report".to_string())?;
    let cpu_failures = metrics
        .cpu_failures
        .ok_or_else(|| "missing cpu_failures in plugin soak report".to_string())?;
    let rss_failures = metrics
        .rss_failures
        .ok_or_else(|| "missing rss_failures in plugin soak report".to_string())?;
    let event_lines_ok = metrics
        .event_lines_ok
        .ok_or_else(|| "missing event_lines.ok in plugin soak report".to_string())?;
    let event_lines_cpuwatch = metrics
        .event_lines_cpuwatch
        .ok_or_else(|| "missing event_lines.cpuwatch in plugin soak report".to_string())?;
    let event_lines_rss = metrics
        .event_lines_rss
        .ok_or_else(|| "missing event_lines.rss in plugin soak report".to_string())?;

    Ok(ParsedPluginSoakReport {
        iterations,
        ok_runs,
        cpu_failures,
        rss_failures,
        event_lines_ok,
        event_lines_cpuwatch,
        event_lines_rss,
        workspace: metrics.workspace,
    })
}

fn extract_u64_from_line(line: &str, key: &str) -> Option<u64> {
    let needle = format!("{key}=");
    let index = line.find(&needle)?;
    let suffix = line[index + needle.len()..].trim_start();
    let digits = suffix
        .chars()
        .take_while(|value| value.is_ascii_digit())
        .collect::<String>();
    if digits.is_empty() {
        return None;
    }
    digits.parse::<u64>().ok()
}

fn extract_string_from_line(line: &str, key: &str) -> Option<String> {
    let needle = format!("{key}=");
    let index = line.find(&needle)?;
    let value = line[index + needle.len()..].trim();
    if value.is_empty() {
        None
    } else {
        Some(value.to_string())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PluginSoakHistoryEntry {
    ts: String,
    status: String,
    source: String,
    report_path: String,
    iterations: u64,
    ok_runs: u64,
    cpu_failures: u64,
    rss_failures: u64,
    completion_ratio: f64,
    ok_success_rate: f64,
    cpu_failure_rate: f64,
    rss_failure_rate: f64,
    event_line_drift_ok: i64,
    event_line_drift_cpuwatch: i64,
    event_line_drift_rss: i64,
    warnings_count: usize,
}

fn plugin_soak_history_path(data_dir: &Path) -> PathBuf {
    data_dir.join("reports").join("plugin-soak-history.jsonl")
}

fn read_plugin_soak_history(
    history_path: &Path,
) -> (Vec<PluginSoakHistoryEntry>, usize, Option<String>) {
    if !history_path.exists() {
        return (Vec::new(), 0, None);
    }
    let raw = match std::fs::read_to_string(history_path) {
        Ok(value) => value,
        Err(err) => {
            return (
                Vec::new(),
                0,
                Some(format!(
                    "failed to read plugin soak history '{}': {err}",
                    history_path.display()
                )),
            );
        }
    };
    let mut entries = Vec::new();
    let mut parse_errors = 0usize;
    for line in raw.lines() {
        if line.trim().is_empty() {
            continue;
        }
        match serde_json::from_str::<PluginSoakHistoryEntry>(line) {
            Ok(entry) => entries.push(entry),
            Err(_) => parse_errors += 1,
        }
    }
    (entries, parse_errors, None)
}

fn append_plugin_soak_history_entry(
    history_path: &Path,
    entry: &PluginSoakHistoryEntry,
) -> std::result::Result<(), String> {
    if let Some(parent) = history_path.parent() {
        std::fs::create_dir_all(parent).map_err(|err| {
            format!(
                "failed to create plugin soak history directory '{}': {err}",
                parent.display()
            )
        })?;
    }
    let mut rendered = serde_json::to_string(entry)
        .map_err(|err| format!("failed to serialize plugin soak history entry: {err}"))?;
    rendered.push('\n');
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(history_path)
        .map_err(|err| {
            format!(
                "failed to open plugin soak history file '{}': {err}",
                history_path.display()
            )
        })?;
    file.write_all(rendered.as_bytes()).map_err(|err| {
        format!(
            "failed to write plugin soak history file '{}': {err}",
            history_path.display()
        )
    })?;
    Ok(())
}

fn rewrite_plugin_soak_history_entries(
    history_path: &Path,
    entries: &[PluginSoakHistoryEntry],
) -> std::result::Result<(), String> {
    if let Some(parent) = history_path.parent() {
        std::fs::create_dir_all(parent).map_err(|err| {
            format!(
                "failed to create plugin soak history directory '{}': {err}",
                parent.display()
            )
        })?;
    }
    let mut rendered = String::new();
    for entry in entries {
        let line = serde_json::to_string(entry)
            .map_err(|err| format!("failed to serialize plugin soak history entry: {err}"))?;
        rendered.push_str(&line);
        rendered.push('\n');
    }
    std::fs::write(history_path, rendered).map_err(|err| {
        format!(
            "failed to rewrite plugin soak history file '{}': {err}",
            history_path.display()
        )
    })?;
    Ok(())
}

fn plugin_soak_history_entry_from_report(
    plugin_soak: &Value,
) -> std::result::Result<PluginSoakHistoryEntry, String> {
    let available = plugin_soak
        .get("available")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    if !available {
        return Err("plugin soak report is not available for history recording".to_string());
    }

    let status = plugin_soak
        .get("status")
        .and_then(Value::as_str)
        .ok_or_else(|| "missing plugin soak status".to_string())?
        .to_string();
    let source = plugin_soak
        .get("source")
        .and_then(Value::as_str)
        .ok_or_else(|| "missing plugin soak source".to_string())?
        .to_string();
    let report_path = plugin_soak
        .get("path")
        .and_then(Value::as_str)
        .ok_or_else(|| "missing plugin soak path".to_string())?
        .to_string();
    let summary = plugin_soak
        .get("summary")
        .ok_or_else(|| "missing plugin soak summary".to_string())?;
    let rates = plugin_soak
        .get("rates")
        .ok_or_else(|| "missing plugin soak rates".to_string())?;
    let trend = plugin_soak
        .get("trend")
        .ok_or_else(|| "missing plugin soak trend".to_string())?;
    let drift = trend
        .get("event_line_drift")
        .ok_or_else(|| "missing plugin soak trend.event_line_drift".to_string())?;
    let warnings_count = plugin_soak
        .get("warnings")
        .and_then(Value::as_array)
        .map_or(0usize, |values| values.len());

    let extract_u64 = |container: &Value, key: &str| -> std::result::Result<u64, String> {
        container
            .get(key)
            .and_then(Value::as_u64)
            .ok_or_else(|| format!("missing plugin soak field '{key}'"))
    };
    let extract_i64 = |container: &Value, key: &str| -> std::result::Result<i64, String> {
        container
            .get(key)
            .and_then(Value::as_i64)
            .ok_or_else(|| format!("missing plugin soak field '{key}'"))
    };
    let extract_f64 = |container: &Value, key: &str| -> std::result::Result<f64, String> {
        container
            .get(key)
            .and_then(Value::as_f64)
            .ok_or_else(|| format!("missing plugin soak field '{key}'"))
    };

    Ok(PluginSoakHistoryEntry {
        ts: chrono::Utc::now().to_rfc3339(),
        status,
        source,
        report_path,
        iterations: extract_u64(summary, "iterations")?,
        ok_runs: extract_u64(summary, "ok_runs")?,
        cpu_failures: extract_u64(summary, "cpu_failures")?,
        rss_failures: extract_u64(summary, "rss_failures")?,
        completion_ratio: extract_f64(trend, "completion_ratio")?,
        ok_success_rate: extract_f64(rates, "ok_success_rate")?,
        cpu_failure_rate: extract_f64(rates, "cpu_failure_rate")?,
        rss_failure_rate: extract_f64(rates, "rss_failure_rate")?,
        event_line_drift_ok: extract_i64(drift, "ok")?,
        event_line_drift_cpuwatch: extract_i64(drift, "cpuwatch")?,
        event_line_drift_rss: extract_i64(drift, "rss")?,
        warnings_count,
    })
}

fn plugin_soak_history_entry_json(entry: &PluginSoakHistoryEntry) -> Value {
    json!({
        "ts": entry.ts,
        "status": entry.status,
        "source": entry.source,
        "report_path": entry.report_path,
        "iterations": entry.iterations,
        "ok_runs": entry.ok_runs,
        "cpu_failures": entry.cpu_failures,
        "rss_failures": entry.rss_failures,
        "completion_ratio": entry.completion_ratio,
        "ok_success_rate": entry.ok_success_rate,
        "cpu_failure_rate": entry.cpu_failure_rate,
        "rss_failure_rate": entry.rss_failure_rate,
        "event_line_drift": {
            "ok": entry.event_line_drift_ok,
            "cpuwatch": entry.event_line_drift_cpuwatch,
            "rss": entry.event_line_drift_rss,
        },
        "warnings_count": entry.warnings_count,
    })
}

fn plugin_soak_history_delta(
    current: &PluginSoakHistoryEntry,
    previous: &PluginSoakHistoryEntry,
) -> Value {
    json!({
        "iterations": (current.iterations as i64) - (previous.iterations as i64),
        "ok_runs": (current.ok_runs as i64) - (previous.ok_runs as i64),
        "cpu_failures": (current.cpu_failures as i64) - (previous.cpu_failures as i64),
        "rss_failures": (current.rss_failures as i64) - (previous.rss_failures as i64),
        "completion_ratio": current.completion_ratio - previous.completion_ratio,
        "ok_success_rate": current.ok_success_rate - previous.ok_success_rate,
        "cpu_failure_rate": current.cpu_failure_rate - previous.cpu_failure_rate,
        "rss_failure_rate": current.rss_failure_rate - previous.rss_failure_rate,
        "event_line_drift": {
            "ok": current.event_line_drift_ok - previous.event_line_drift_ok,
            "cpuwatch": current.event_line_drift_cpuwatch - previous.event_line_drift_cpuwatch,
            "rss": current.event_line_drift_rss - previous.event_line_drift_rss,
        },
        "warnings_count": (current.warnings_count as i64) - (previous.warnings_count as i64),
    })
}

fn empty_plugin_soak_history_delta() -> Value {
    json!({
        "iterations": 0,
        "ok_runs": 0,
        "cpu_failures": 0,
        "rss_failures": 0,
        "completion_ratio": 0.0,
        "ok_success_rate": 0.0,
        "cpu_failure_rate": 0.0,
        "rss_failure_rate": 0.0,
        "event_line_drift": {
            "ok": 0,
            "cpuwatch": 0,
            "rss": 0,
        },
        "warnings_count": 0,
    })
}

fn plugin_soak_total_drift(entry: &PluginSoakHistoryEntry) -> i64 {
    entry.event_line_drift_ok.abs()
        + entry.event_line_drift_cpuwatch.abs()
        + entry.event_line_drift_rss.abs()
}

fn count_plugin_soak_completion_unmet_streak(
    entries: &[PluginSoakHistoryEntry],
    completion_target: f64,
) -> usize {
    entries
        .iter()
        .rev()
        .take_while(|entry| entry.completion_ratio < completion_target)
        .count()
}

fn count_plugin_soak_status_unmet_streak(entries: &[PluginSoakHistoryEntry]) -> usize {
    entries
        .iter()
        .rev()
        .take_while(|entry| entry.status != "ok")
        .count()
}

struct PluginSoakIncidentHintConfig {
    completion_target: f64,
    completion_drop_warn: f64,
    drift_abs_threshold: i64,
    incident_window: usize,
    repeat_threshold: usize,
}

fn build_plugin_soak_incident_hints(
    entries: &[PluginSoakHistoryEntry],
    current: Option<&PluginSoakHistoryEntry>,
    previous: Option<&PluginSoakHistoryEntry>,
    config: &PluginSoakIncidentHintConfig,
) -> Vec<Value> {
    let mut hints = Vec::<Value>::new();
    let completion_unmet_streak =
        count_plugin_soak_completion_unmet_streak(entries, config.completion_target);
    if completion_unmet_streak >= config.repeat_threshold {
        hints.push(json!({
            "id": "plugin_soak.completion_unmet_streak",
            "severity": "warning",
            "message": format!(
                "plugin soak completion ratio stayed below target for {completion_unmet_streak} runs"
            ),
            "completion_target": config.completion_target,
            "streak": completion_unmet_streak,
        }));
    }

    let status_unmet_streak = count_plugin_soak_status_unmet_streak(entries);
    if status_unmet_streak >= config.repeat_threshold {
        hints.push(json!({
            "id": "plugin_soak.status_unmet_streak",
            "severity": "warning",
            "message": format!("plugin soak status stayed non-ok for {status_unmet_streak} runs"),
            "streak": status_unmet_streak,
        }));
    }

    let recent_entries = entries
        .iter()
        .rev()
        .take(config.incident_window)
        .collect::<Vec<_>>();
    let recent_drift_count = recent_entries
        .iter()
        .filter(|entry| plugin_soak_total_drift(entry) >= config.drift_abs_threshold)
        .count();
    if recent_drift_count >= config.repeat_threshold {
        hints.push(json!({
            "id": "plugin_soak.drift_repeated",
            "severity": "warning",
            "message": format!(
                "plugin soak drift exceeded threshold in {recent_drift_count}/{} recent runs",
                config.incident_window
            ),
            "window": config.incident_window,
            "repeat_threshold": config.repeat_threshold,
            "drift_abs_threshold": config.drift_abs_threshold,
            "repeated": recent_drift_count,
        }));
    }

    let recent_warning_count = recent_entries
        .iter()
        .filter(|entry| entry.warnings_count > 0)
        .count();
    if recent_warning_count >= config.repeat_threshold {
        hints.push(json!({
            "id": "plugin_soak.warnings_repeated",
            "severity": "warning",
            "message": format!(
                "plugin soak report emitted warnings in {recent_warning_count}/{} recent runs",
                config.incident_window
            ),
            "window": config.incident_window,
            "repeat_threshold": config.repeat_threshold,
            "repeated": recent_warning_count,
        }));
    }

    if let (Some(current), Some(previous)) = (current, previous) {
        let completion_ratio_delta = current.completion_ratio - previous.completion_ratio;
        if completion_ratio_delta <= -config.completion_drop_warn {
            hints.push(json!({
                "id": "plugin_soak.completion_regression",
                "severity": "warning",
                "message": format!(
                    "plugin soak completion ratio dropped by {:.4} between consecutive runs",
                    completion_ratio_delta.abs()
                ),
                "drop": completion_ratio_delta,
                "threshold": -config.completion_drop_warn,
            }));
        }
    }

    hints
}

fn build_plugin_soak_history(plugin_soak: &Value, data_dir: &Path) -> Value {
    let history_path = plugin_soak_history_path(data_dir);
    let max_samples = read_positive_usize("MOSAIC_OBS_PLUGIN_SOAK_HISTORY_MAX_SAMPLES", 200);
    let completion_target = read_ratio_threshold("MOSAIC_OBS_ALERT_PLUGIN_COMPLETION_MIN", 1.0);
    let completion_drop_warn =
        read_ratio_threshold("MOSAIC_OBS_PLUGIN_SOAK_COMPLETION_DROP_WARN", 0.02);
    let incident_window = read_positive_usize("MOSAIC_OBS_PLUGIN_SOAK_INCIDENT_WINDOW", 5);
    let repeat_threshold = read_positive_usize("MOSAIC_OBS_PLUGIN_SOAK_REPEAT_HINT_THRESHOLD", 3);
    let drift_abs_threshold =
        read_positive_usize("MOSAIC_OBS_PLUGIN_SOAK_DRIFT_ABS_THRESHOLD", 1) as i64;
    let incident_hint_config = PluginSoakIncidentHintConfig {
        completion_target,
        completion_drop_warn,
        drift_abs_threshold,
        incident_window,
        repeat_threshold,
    };
    let (mut entries, parse_errors, read_error) = read_plugin_soak_history(&history_path);
    let mut appended = false;
    let mut write_error = None::<String>;
    let mut pruned = 0usize;

    let current_entry = if plugin_soak
        .get("available")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        match plugin_soak_history_entry_from_report(plugin_soak) {
            Ok(entry) => {
                if let Err(err) = append_plugin_soak_history_entry(&history_path, &entry) {
                    write_error = Some(err);
                } else {
                    appended = true;
                    entries.push(entry.clone());
                }
                Some(entry)
            }
            Err(err) => {
                write_error = Some(err);
                None
            }
        }
    } else {
        None
    };
    let previous_for_current = if current_entry.is_some() {
        if appended {
            if entries.len() >= 2 {
                entries.get(entries.len() - 2).cloned()
            } else {
                None
            }
        } else {
            entries.last().cloned()
        }
    } else {
        None
    };
    let current_vs_previous = if let Some(current) = current_entry.as_ref() {
        if let Some(previous) = previous_for_current.as_ref() {
            json!({
                "available": true,
                "current_ts": current.ts,
                "previous_ts": previous.ts,
                "delta": plugin_soak_history_delta(current, previous),
            })
        } else {
            json!({
                "available": false,
                "current_ts": current.ts,
                "previous_ts": null,
                "delta": empty_plugin_soak_history_delta(),
            })
        }
    } else {
        json!({
            "available": false,
            "current_ts": null,
            "previous_ts": null,
            "delta": empty_plugin_soak_history_delta(),
        })
    };

    if entries.len() > max_samples {
        pruned = entries.len() - max_samples;
        entries.drain(0..pruned);
        if let Err(err) = rewrite_plugin_soak_history_entries(&history_path, &entries)
            && write_error.is_none()
        {
            write_error = Some(err);
        }
    }

    let latest = entries.last().cloned();
    let completion_unmet_streak =
        count_plugin_soak_completion_unmet_streak(&entries, completion_target);
    let status_unmet_streak = count_plugin_soak_status_unmet_streak(&entries);
    let recent_drift_count = entries
        .iter()
        .rev()
        .take(incident_window)
        .filter(|entry| plugin_soak_total_drift(entry) >= drift_abs_threshold)
        .count();
    let incident_hints = build_plugin_soak_incident_hints(
        &entries,
        current_entry.as_ref(),
        previous_for_current.as_ref(),
        &incident_hint_config,
    );

    json!({
        "path": history_path.display().to_string(),
        "sample_count": entries.len(),
        "retention": {
            "max_samples": max_samples,
            "pruned": pruned,
        },
        "parse_errors": parse_errors,
        "appended": appended,
        "read_error": read_error,
        "write_error": write_error,
        "latest": latest.as_ref().map(plugin_soak_history_entry_json),
        "current_run": current_entry.as_ref().map(plugin_soak_history_entry_json),
        "current_vs_previous": current_vs_previous,
        "window": {
            "size": incident_window,
            "repeat_threshold": repeat_threshold,
            "completion_target": completion_target,
            "drift_abs_threshold": drift_abs_threshold,
            "recent_drift_count": recent_drift_count,
        },
        "streaks": {
            "completion_unmet": completion_unmet_streak,
            "status_not_ok": status_unmet_streak,
        },
        "incident_hints_count": incident_hints.len(),
        "incident_hints": incident_hints,
    })
}

fn read_command_audit_entries(path: &std::path::Path) -> Result<(Vec<CommandAudit>, usize)> {
    if !path.exists() {
        return Ok((Vec::new(), 0));
    }

    let raw = std::fs::read_to_string(path)?;
    let mut entries = Vec::new();
    let mut parse_errors = 0usize;
    for line in raw.lines() {
        if line.trim().is_empty() {
            continue;
        }
        match serde_json::from_str::<CommandAudit>(line) {
            Ok(entry) => entries.push(entry),
            Err(_) => parse_errors += 1,
        }
    }

    Ok((entries, parse_errors))
}

#[derive(Default)]
struct AuditWindowSummary {
    total_entries: usize,
    succeeded: usize,
    failed: usize,
    average_duration_ms: f64,
    blocked_if_restricted: usize,
    approved_by_counts: BTreeMap<String, usize>,
    current_decision_counts: BTreeMap<String, usize>,
    top_commands: Vec<Value>,
}

fn summarize_audit_window(
    entries: &[CommandAudit],
    approval_policy: &mosaic_ops::ApprovalPolicy,
    sandbox_profile: mosaic_ops::SandboxProfile,
) -> AuditWindowSummary {
    let total_entries = entries.len();
    let succeeded = entries.iter().filter(|entry| entry.exit_code == 0).count();
    let failed = total_entries.saturating_sub(succeeded);
    let mut total_duration_ms = 0u128;
    let mut approved_by_counts = BTreeMap::<String, usize>::new();
    let mut command_prefix_counts = BTreeMap::<String, usize>::new();
    let mut current_decision_counts = BTreeMap::<String, usize>::new();
    let mut blocked_if_restricted = 0usize;

    for entry in entries {
        total_duration_ms = total_duration_ms.saturating_add(entry.duration_ms);
        *approved_by_counts
            .entry(entry.approved_by.clone())
            .or_default() += 1;
        let prefix = entry
            .command
            .split_whitespace()
            .next()
            .unwrap_or("<empty>")
            .to_string();
        *command_prefix_counts.entry(prefix).or_default() += 1;

        let evaluated = evaluate_safety(&entry.command, approval_policy, sandbox_profile);
        *current_decision_counts
            .entry(evaluated.decision.to_string())
            .or_default() += 1;

        if evaluate_sandbox(&entry.command, mosaic_ops::SandboxProfile::Restricted).is_some() {
            blocked_if_restricted += 1;
        }
    }

    let mut top_commands = command_prefix_counts
        .into_iter()
        .map(|(command, count)| json!({ "command": command, "count": count }))
        .collect::<Vec<_>>();
    top_commands.sort_by(|lhs, rhs| {
        rhs["count"]
            .as_u64()
            .unwrap_or(0)
            .cmp(&lhs["count"].as_u64().unwrap_or(0))
            .then_with(|| {
                lhs["command"]
                    .as_str()
                    .unwrap_or("")
                    .cmp(rhs["command"].as_str().unwrap_or(""))
            })
    });
    if top_commands.len() > 5 {
        top_commands.truncate(5);
    }

    let average_duration_ms = if total_entries == 0 {
        0.0
    } else {
        (total_duration_ms as f64) / (total_entries as f64)
    };

    AuditWindowSummary {
        total_entries,
        succeeded,
        failed,
        average_duration_ms,
        blocked_if_restricted,
        approved_by_counts,
        current_decision_counts,
        top_commands,
    }
}

fn build_audit_comparison(
    current: &AuditWindowSummary,
    previous: &AuditWindowSummary,
    compare_window: usize,
    available: bool,
) -> Value {
    let to_rate = |failed: usize, total: usize| -> f64 {
        if total == 0 {
            0.0
        } else {
            (failed as f64) / (total as f64)
        }
    };
    let current_failure_rate = to_rate(current.failed, current.total_entries);
    let previous_failure_rate = to_rate(previous.failed, previous.total_entries);

    json!({
        "enabled": compare_window > 0,
        "available": available,
        "window": compare_window,
        "current": {
            "total_entries": current.total_entries,
            "failed": current.failed,
            "failure_rate": current_failure_rate,
            "average_duration_ms": current.average_duration_ms,
            "blocked_if_restricted": current.blocked_if_restricted,
        },
        "previous": {
            "total_entries": previous.total_entries,
            "failed": previous.failed,
            "failure_rate": previous_failure_rate,
            "average_duration_ms": previous.average_duration_ms,
            "blocked_if_restricted": previous.blocked_if_restricted,
        },
        "delta": {
            "total_entries": (current.total_entries as i64) - (previous.total_entries as i64),
            "failed": (current.failed as i64) - (previous.failed as i64),
            "failure_rate": current_failure_rate - previous_failure_rate,
            "average_duration_ms": current.average_duration_ms - previous.average_duration_ms,
            "blocked_if_restricted": (current.blocked_if_restricted as i64) - (previous.blocked_if_restricted as i64),
        },
    })
}

fn build_safety_audit(
    audit_log_path: &std::path::Path,
    tail: usize,
    compare_window: usize,
    approval_policy: &mosaic_ops::ApprovalPolicy,
    sandbox_profile: mosaic_ops::SandboxProfile,
) -> Result<Value> {
    let (entries, parse_errors) = read_command_audit_entries(audit_log_path)?;
    let current_start = if tail == 0 {
        entries.len()
    } else {
        entries.len().saturating_sub(tail)
    };
    let current_entries = entries[current_start..].to_vec();
    let previous_entries = if compare_window == 0 || current_start == 0 {
        Vec::new()
    } else {
        let start = current_start.saturating_sub(compare_window);
        entries[start..current_start].to_vec()
    };

    let current_summary =
        summarize_audit_window(&current_entries, approval_policy, sandbox_profile);
    let previous_summary =
        summarize_audit_window(&previous_entries, approval_policy, sandbox_profile);
    let comparison = build_audit_comparison(
        &current_summary,
        &previous_summary,
        compare_window,
        compare_window > 0 && !previous_entries.is_empty(),
    );

    let recent = current_entries
        .iter()
        .rev()
        .take(10)
        .map(|entry| {
            let evaluated = evaluate_safety(&entry.command, approval_policy, sandbox_profile);
            json!({
                "id": entry.id,
                "ts": entry.ts,
                "session_id": entry.session_id,
                "command": entry.command,
                "approved_by": entry.approved_by,
                "exit_code": entry.exit_code,
                "duration_ms": entry.duration_ms,
                "current_policy_decision": evaluated.decision,
                "current_policy_reason": evaluated.reason,
            })
        })
        .collect::<Vec<_>>();

    Ok(json!({
        "path": audit_log_path.display().to_string(),
        "tail": tail,
        "compare_window": compare_window,
        "summary": {
            "total_entries": current_summary.total_entries,
            "parse_errors": parse_errors,
            "succeeded": current_summary.succeeded,
            "failed": current_summary.failed,
            "average_duration_ms": current_summary.average_duration_ms,
            "blocked_if_restricted": current_summary.blocked_if_restricted,
            "approved_by": current_summary.approved_by_counts,
            "current_policy_decisions": current_summary.current_decision_counts,
            "top_commands": current_summary.top_commands,
        },
        "comparison": comparison,
        "recent_commands": recent,
    }))
}

struct ApprovalDecisionView {
    decision: &'static str,
    reason: Option<String>,
    approved_by: Option<String>,
}

struct SafetyCheckView {
    command: String,
    decision: &'static str,
    reason: Option<String>,
    approved_by: Option<String>,
    sandbox_decision: &'static str,
    sandbox_reason: Option<String>,
    approval_decision: &'static str,
    approval_reason: Option<String>,
    approval_mode: String,
    sandbox_profile: String,
}

fn approval_decision_view(decision: ApprovalDecision) -> ApprovalDecisionView {
    match decision {
        ApprovalDecision::Auto { approved_by } => ApprovalDecisionView {
            decision: "auto",
            reason: None,
            approved_by: Some(approved_by),
        },
        ApprovalDecision::NeedsConfirmation { reason } => ApprovalDecisionView {
            decision: "confirm",
            reason: Some(reason),
            approved_by: None,
        },
        ApprovalDecision::Deny { reason } => ApprovalDecisionView {
            decision: "deny",
            reason: Some(reason),
            approved_by: None,
        },
    }
}

fn evaluate_safety(
    command: &str,
    policy: &mosaic_ops::ApprovalPolicy,
    profile: mosaic_ops::SandboxProfile,
) -> SafetyCheckView {
    let sandbox_reason = evaluate_sandbox(command, profile);
    let sandbox_decision = if sandbox_reason.is_some() {
        "deny"
    } else {
        "allow"
    };
    let approval = approval_decision_view(evaluate_approval(command, policy));

    let (decision, reason) = if let Some(reason) = sandbox_reason.clone() {
        ("deny", Some(reason))
    } else {
        match approval.decision {
            "deny" => ("deny", approval.reason.clone()),
            "confirm" => ("confirm", approval.reason.clone()),
            _ => ("allow", None),
        }
    };

    SafetyCheckView {
        command: command.to_string(),
        decision,
        reason,
        approved_by: approval.approved_by.clone(),
        sandbox_decision,
        sandbox_reason,
        approval_decision: approval.decision,
        approval_reason: approval.reason,
        approval_mode: format!("{:?}", policy.mode).to_lowercase(),
        sandbox_profile: format!("{:?}", profile).to_lowercase(),
    }
}

pub(super) fn handle_system(cli: &Cli, args: SystemArgs) -> Result<()> {
    let paths = resolve_state_paths(cli.project_state)?;
    paths.ensure_dirs()?;
    let store = SystemEventStore::new(system_events_path(&paths.data_dir));
    match args.command {
        SystemCommand::Event { name, data } => {
            let data = data
                .as_deref()
                .map(|value| parse_json_input(value, "system event data"))
                .transpose()?
                .unwrap_or(Value::Null);
            let dispatch = dispatch_system_event(cli, &paths, &name, data)?;
            let event = dispatch.event;
            let hook_reports = dispatch.hook_reports;
            let hooks_ok = hook_reports.iter().filter(|item| item.ok).count();
            let hooks_failed = hook_reports.len().saturating_sub(hooks_ok);
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "event": event,
                    "path": store.path().display().to_string(),
                    "hooks": {
                        "triggered": hook_reports.len(),
                        "ok": hooks_ok,
                        "failed": hooks_failed,
                        "results": hook_reports,
                    }
                }));
            } else {
                println!("event recorded: {}", event.name);
                println!("path: {}", store.path().display());
                if hook_reports.is_empty() {
                    println!("hooks triggered: 0");
                } else {
                    println!("hooks triggered: {}", hook_reports.len());
                    println!("hooks ok: {hooks_ok}");
                    println!("hooks failed: {hooks_failed}");
                }
            }
        }
        SystemCommand::Presence => {
            let cwd = std::env::current_dir().map_err(|err| MosaicError::Io(err.to_string()))?;
            let presence = snapshot_presence(&cwd);
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "presence": presence,
                }));
            } else {
                println!("presence:");
                println!("hostname: {}", presence.hostname);
                println!("pid: {}", presence.pid);
                println!("cwd: {}", presence.cwd);
                println!("ts: {}", presence.ts.to_rfc3339());
            }
        }
        SystemCommand::List { tail, name } => {
            let mut events = store.read_tail(tail)?;
            if let Some(name_filter) = name.as_deref() {
                events.retain(|event| event.name == name_filter);
            }
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "events": events,
                    "path": store.path().display().to_string(),
                }));
            } else if events.is_empty() {
                println!("No system events.");
            } else {
                for event in events {
                    println!("{} {} {}", event.ts.to_rfc3339(), event.name, event.data);
                }
            }
        }
    }
    Ok(())
}

pub(super) fn handle_approvals(cli: &Cli, args: ApprovalsArgs) -> Result<()> {
    let paths = resolve_state_paths(cli.project_state)?;
    paths.ensure_dirs()?;
    let store = ApprovalStore::new(paths.approvals_policy_path.clone());
    let policy = match args.command {
        ApprovalsCommand::Check { command } => {
            let policy = store.load_or_default()?;
            let approval = approval_decision_view(evaluate_approval(&command, &policy));
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "command": command,
                    "decision": approval.decision,
                    "approved_by": approval.approved_by,
                    "reason": approval.reason,
                    "policy_mode": policy.mode,
                    "path": store.path().display().to_string(),
                }));
            } else {
                println!("command: {command}");
                println!("decision: {}", approval.decision);
                if let Some(approved_by) = approval.approved_by {
                    println!("approved_by: {approved_by}");
                }
                if let Some(reason) = approval.reason {
                    println!("reason: {reason}");
                }
                println!("approvals mode: {:?}", policy.mode);
                println!("path: {}", store.path().display());
            }
            return Ok(());
        }
        ApprovalsCommand::Get => store.load_or_default()?,
        ApprovalsCommand::Set { mode } => store.set_mode(mode.into())?,
        ApprovalsCommand::Allowlist { command } => match command {
            AllowlistCommand::List => store.load_or_default()?,
            AllowlistCommand::Add { prefix } => store.add_allowlist(&prefix)?,
            AllowlistCommand::Remove { prefix } => store.remove_allowlist(&prefix)?,
        },
    };

    if cli.json {
        print_json(&json!({
            "ok": true,
            "policy": policy,
            "path": store.path().display().to_string(),
        }));
    } else {
        println!("approvals mode: {:?}", policy.mode);
        if policy.allowlist.is_empty() {
            println!("allowlist: <empty>");
        } else {
            println!("allowlist:");
            for item in policy.allowlist {
                println!("- {item}");
            }
        }
        println!("path: {}", store.path().display());
    }
    Ok(())
}

pub(super) fn handle_sandbox(cli: &Cli, args: SandboxArgs) -> Result<()> {
    let paths = resolve_state_paths(cli.project_state)?;
    paths.ensure_dirs()?;
    let store = SandboxStore::new(paths.sandbox_policy_path.clone());
    match args.command {
        SandboxCommand::Get => {
            let policy = store.load_or_default()?;
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "policy": policy,
                    "path": store.path().display().to_string(),
                }));
            } else {
                println!("sandbox profile: {:?}", policy.profile);
                println!("path: {}", store.path().display());
            }
        }
        SandboxCommand::Set { profile } => {
            let policy = store.set_profile(profile.into())?;
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "policy": policy,
                    "path": store.path().display().to_string(),
                }));
            } else {
                println!("sandbox profile set: {:?}", policy.profile);
                println!("path: {}", store.path().display());
            }
        }
        SandboxCommand::Check { command } => {
            let policy = store.load_or_default()?;
            let reason = evaluate_sandbox(&command, policy.profile);
            let decision = if reason.is_some() { "deny" } else { "allow" };
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "command": command,
                    "decision": decision,
                    "reason": reason,
                    "profile": policy.profile,
                    "path": store.path().display().to_string(),
                }));
            } else {
                println!("command: {command}");
                println!("decision: {decision}");
                if let Some(reason) = reason {
                    println!("reason: {reason}");
                }
                println!("sandbox profile: {:?}", policy.profile);
                println!("path: {}", store.path().display());
            }
        }
        SandboxCommand::List => {
            let policy = store.load_or_default()?;
            let profiles = list_profiles();
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "current": policy.profile,
                    "profiles": profiles,
                    "path": store.path().display().to_string(),
                }));
            } else {
                println!("current sandbox profile: {:?}", policy.profile);
                for profile in profiles {
                    println!("- {:?}: {}", profile.profile, profile.description);
                }
            }
        }
        SandboxCommand::Explain { profile } => {
            let policy = store.load_or_default()?;
            let profile = profile.map(Into::into).unwrap_or(policy.profile);
            let info = mosaic_ops::profile_info(profile);
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "profile": info,
                    "path": store.path().display().to_string(),
                }));
            } else {
                println!("sandbox profile: {:?}", info.profile);
                println!("{}", info.description);
                if info.blocked_examples.is_empty() {
                    println!("blocked examples: <none>");
                } else {
                    println!("blocked examples:");
                    for example in info.blocked_examples {
                        println!("- {example}");
                    }
                }
            }
        }
    }
    Ok(())
}

pub(super) fn handle_safety(cli: &Cli, args: SafetyArgs) -> Result<()> {
    let paths = resolve_state_paths(cli.project_state)?;
    paths.ensure_dirs()?;
    let approval_store = ApprovalStore::new(paths.approvals_policy_path.clone());
    let sandbox_store = SandboxStore::new(paths.sandbox_policy_path.clone());
    let approval_policy = approval_store.load_or_default()?;
    let sandbox_policy = sandbox_store.load_or_default()?;

    match args.command {
        SafetyCommand::Get => {
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "approvals": {
                        "policy": approval_policy,
                        "path": approval_store.path().display().to_string(),
                    },
                    "sandbox": {
                        "policy": sandbox_policy,
                        "path": sandbox_store.path().display().to_string(),
                    },
                }));
            } else {
                println!("approvals mode: {:?}", approval_policy.mode);
                if approval_policy.allowlist.is_empty() {
                    println!("approvals allowlist: <empty>");
                } else {
                    println!("approvals allowlist:");
                    for item in approval_policy.allowlist {
                        println!("- {item}");
                    }
                }
                println!("approvals path: {}", approval_store.path().display());
                println!("sandbox profile: {:?}", sandbox_policy.profile);
                println!("sandbox path: {}", sandbox_store.path().display());
            }
        }
        SafetyCommand::Check { command } => {
            let check = evaluate_safety(&command, &approval_policy, sandbox_policy.profile);
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "command": check.command,
                    "decision": check.decision,
                    "reason": check.reason,
                    "approved_by": check.approved_by,
                    "sandbox": {
                        "profile": check.sandbox_profile,
                        "decision": check.sandbox_decision,
                        "reason": check.sandbox_reason,
                    },
                    "approvals": {
                        "mode": check.approval_mode,
                        "decision": check.approval_decision,
                        "reason": check.approval_reason,
                    },
                    "paths": {
                        "approvals_policy": approval_store.path().display().to_string(),
                        "sandbox_policy": sandbox_store.path().display().to_string(),
                    }
                }));
            } else {
                println!("command: {}", check.command);
                println!("decision: {}", check.decision);
                if let Some(reason) = check.reason {
                    println!("reason: {reason}");
                }
                if let Some(approved_by) = check.approved_by {
                    println!("approved_by: {approved_by}");
                }
                println!(
                    "sandbox: {} ({})",
                    check.sandbox_profile, check.sandbox_decision
                );
                if let Some(reason) = check.sandbox_reason {
                    println!("sandbox_reason: {reason}");
                }
                println!(
                    "approvals: {} ({})",
                    check.approval_mode, check.approval_decision
                );
                if let Some(reason) = check.approval_reason {
                    println!("approval_reason: {reason}");
                }
            }
        }
        SafetyCommand::Report {
            command,
            audit_tail,
            compare_window,
        } => {
            let profile = mosaic_ops::profile_info(sandbox_policy.profile);
            let check = command
                .as_deref()
                .map(|value| evaluate_safety(value, &approval_policy, sandbox_policy.profile));
            let audit = build_safety_audit(
                &paths.audit_log_path,
                audit_tail,
                compare_window,
                &approval_policy,
                sandbox_policy.profile,
            )?;
            let profiles = list_profiles();
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "approvals": {
                        "policy": approval_policy,
                        "path": approval_store.path().display().to_string(),
                    },
                    "sandbox": {
                        "policy": sandbox_policy,
                        "path": sandbox_store.path().display().to_string(),
                        "profile_info": profile,
                        "profiles": profiles,
                    },
                    "audit": audit,
                    "check": check.map(|value| json!({
                        "command": value.command,
                        "decision": value.decision,
                        "reason": value.reason,
                        "approved_by": value.approved_by,
                        "sandbox": {
                            "profile": value.sandbox_profile,
                            "decision": value.sandbox_decision,
                            "reason": value.sandbox_reason,
                        },
                        "approvals": {
                            "mode": value.approval_mode,
                            "decision": value.approval_decision,
                            "reason": value.approval_reason,
                        },
                    })),
                }));
            } else {
                println!("safety report");
                println!("approvals mode: {:?}", approval_policy.mode);
                println!(
                    "approvals allowlist entries: {}",
                    approval_policy.allowlist.len()
                );
                println!("sandbox profile: {:?}", sandbox_policy.profile);
                println!("sandbox description: {}", profile.description);
                println!(
                    "audit entries (tail={}): {}",
                    audit_tail,
                    audit["summary"]["total_entries"].as_u64().unwrap_or(0)
                );
                println!(
                    "audit parse errors: {}",
                    audit["summary"]["parse_errors"].as_u64().unwrap_or(0)
                );
                println!(
                    "audit failed commands: {}",
                    audit["summary"]["failed"].as_u64().unwrap_or(0)
                );
                if compare_window > 0 {
                    println!(
                        "audit compare available: {}",
                        audit["comparison"]["available"].as_bool().unwrap_or(false)
                    );
                    println!(
                        "audit failed delta: {}",
                        audit["comparison"]["delta"]["failed"].as_i64().unwrap_or(0)
                    );
                    println!(
                        "audit failure-rate delta: {:.4}",
                        audit["comparison"]["delta"]["failure_rate"]
                            .as_f64()
                            .unwrap_or(0.0)
                    );
                }
                println!(
                    "audit blocked_if_restricted: {}",
                    audit["summary"]["blocked_if_restricted"]
                        .as_u64()
                        .unwrap_or(0)
                );
                if profile.blocked_examples.is_empty() {
                    println!("sandbox blocked examples: <none>");
                } else {
                    println!("sandbox blocked examples:");
                    for example in profile.blocked_examples {
                        println!("- {example}");
                    }
                }
                if let Some(check) = check {
                    println!("check.command: {}", check.command);
                    println!("check.decision: {}", check.decision);
                    if let Some(reason) = check.reason {
                        println!("check.reason: {reason}");
                    }
                    if let Some(approved_by) = check.approved_by {
                        println!("check.approved_by: {approved_by}");
                    }
                    println!(
                        "check.sandbox: {} ({})",
                        check.sandbox_profile, check.sandbox_decision
                    );
                    if let Some(reason) = check.sandbox_reason {
                        println!("check.sandbox_reason: {reason}");
                    }
                    println!(
                        "check.approvals: {} ({})",
                        check.approval_mode, check.approval_decision
                    );
                    if let Some(reason) = check.approval_reason {
                        println!("check.approval_reason: {reason}");
                    }
                }
                println!("approvals path: {}", approval_store.path().display());
                println!("sandbox path: {}", sandbox_store.path().display());
            }
        }
    }

    Ok(())
}
