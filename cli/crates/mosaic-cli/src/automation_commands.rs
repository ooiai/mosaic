use std::collections::{BTreeMap, BTreeSet};

use chrono::{DateTime, Duration, Utc};
use serde_json::{Value, json};

use mosaic_core::error::MosaicError;

use super::{
    AutomationReplayReasonArg, Cli, CronArgs, CronCommand, CronEventRecord, CronJobRecord,
    HookEventRecord, HookRecord, HooksArgs, HooksCommand, Result, WebhookEventRecord,
    WebhookMethod, WebhookRecord, WebhooksArgs, WebhooksCommand, apply_cron_result,
    apply_hook_last_result, apply_webhook_last_result, cron_jobs_file_path, execute_cron_job,
    execute_hook_command, execute_webhook, generate_cron_job_id, generate_hook_id,
    generate_webhook_id, hook_execution_error, hooks_file_path, load_cron_jobs_or_default,
    load_hooks_or_default, load_webhooks_or_default, normalize_optional_secret_env,
    normalize_webhook_path, parse_json_input, print_json, read_cron_events, read_hook_events,
    read_webhook_events, resolve_state_paths, save_cron_jobs, save_hooks, save_json_file,
    save_webhooks, webhook_execution_error, webhooks_file_path,
};

fn resolve_logs_cutoff(
    since_minutes: Option<u64>,
    command_hint: &str,
) -> Result<Option<DateTime<Utc>>> {
    let Some(since_minutes) = since_minutes else {
        return Ok(None);
    };
    if since_minutes == 0 {
        return Err(MosaicError::Validation(format!(
            "{command_hint} --since-minutes must be greater than 0"
        )));
    }
    let since_i64 = i64::try_from(since_minutes).map_err(|_| {
        MosaicError::Validation(format!("{command_hint} --since-minutes value is too large"))
    })?;
    Ok(Some(Utc::now() - Duration::minutes(since_i64)))
}

fn apply_logs_cutoff<T>(
    mut events: Vec<T>,
    tail: usize,
    cutoff: Option<DateTime<Utc>>,
    ts_of: impl Fn(&T) -> DateTime<Utc>,
) -> Vec<T> {
    if let Some(cutoff) = cutoff {
        events.retain(|item| ts_of(item) >= cutoff);
    }
    if events.len() > tail {
        let keep_from = events.len() - tail;
        events = events.split_off(keep_from);
    }
    events
}

fn automation_reason_key(reason: AutomationReplayReasonArg) -> &'static str {
    match reason {
        AutomationReplayReasonArg::ApprovalRequired => "approval_required",
        AutomationReplayReasonArg::SandboxDenied => "sandbox_denied",
        AutomationReplayReasonArg::Auth => "auth",
        AutomationReplayReasonArg::Validation => "validation",
        AutomationReplayReasonArg::Tool => "tool",
        AutomationReplayReasonArg::HookFailures => "hook_failures",
        AutomationReplayReasonArg::Unknown => "unknown",
    }
}

fn is_retryable_reason(reason: &str) -> bool {
    matches!(reason, "tool" | "hook_failures")
}

fn classify_hook_replay_reason(event: &HookEventRecord) -> &'static str {
    match event.error_code.as_deref() {
        Some("approval_required") => "approval_required",
        Some("sandbox_denied") => "sandbox_denied",
        Some("auth") => "auth",
        Some("validation") => "validation",
        Some("tool") => "tool",
        Some(_) => "unknown",
        None => {
            if event.exit_code.unwrap_or_default() != 0 {
                "tool"
            } else {
                "unknown"
            }
        }
    }
}

fn classify_cron_replay_reason(event: &CronEventRecord) -> &'static str {
    if event.hooks_failed > 0 {
        return "hook_failures";
    }
    let Some(message) = event.error.as_deref() else {
        return "unknown";
    };
    let lower = message.to_lowercase();
    if lower.contains("approval required") {
        "approval_required"
    } else if lower.contains("sandbox denied") {
        "sandbox_denied"
    } else if lower.contains("auth") {
        "auth"
    } else if lower.contains("validation") {
        "validation"
    } else if lower.contains("tool") || lower.contains("exit status") {
        "tool"
    } else {
        "unknown"
    }
}

fn classify_webhook_replay_reason(event: &WebhookEventRecord) -> &'static str {
    if event.hooks_failed > 0 {
        return "hook_failures";
    }
    match event.error_code.as_deref() {
        Some("approval_required") => "approval_required",
        Some("sandbox_denied") => "sandbox_denied",
        Some("auth") => "auth",
        Some("validation") => "validation",
        Some("tool") => "tool",
        Some(_) => "unknown",
        None => "unknown",
    }
}

fn build_automation_replay_batch_plan(
    candidates: &[Value],
    batch_size: Option<usize>,
) -> Vec<Value> {
    if candidates.is_empty() {
        return Vec::new();
    }
    let effective_batch_size = batch_size.unwrap_or(candidates.len()).max(1);
    candidates
        .chunks(effective_batch_size)
        .enumerate()
        .map(|(index, chunk)| {
            let mut reasons = BTreeSet::new();
            for candidate in chunk {
                if let Some(reason) = candidate["reason"].as_str() {
                    reasons.insert(reason.to_string());
                }
            }
            json!({
                "batch_index": index + 1,
                "size": chunk.len(),
                "first_ts": chunk
                    .first()
                    .and_then(|item| item["ts"].as_str())
                    .unwrap_or("-"),
                "last_ts": chunk
                    .last()
                    .and_then(|item| item["ts"].as_str())
                    .unwrap_or("-"),
                "retryable_only": chunk.iter().all(|item| item["retryable"].as_bool().unwrap_or(false)),
                "reasons": reasons.into_iter().collect::<Vec<_>>(),
                "items": chunk
                    .iter()
                    .map(|item| {
                        json!({
                            "ts": item["ts"],
                            "reason": item["reason"],
                            "retryable": item["retryable"],
                        })
                    })
                    .collect::<Vec<_>>(),
            })
        })
        .collect()
}

fn build_automation_replay_diagnostics<T>(
    selected: &[(T, &'static str, bool)],
    ts_of: impl Fn(&T) -> DateTime<Utc>,
) -> Value {
    let mut reason_histogram = BTreeMap::new();
    let mut retryable_candidates = 0usize;
    let mut non_retryable_candidates = 0usize;
    let mut oldest_ts: Option<DateTime<Utc>> = None;
    let mut newest_ts: Option<DateTime<Utc>> = None;

    for (item, reason, retryable) in selected {
        *reason_histogram
            .entry((*reason).to_string())
            .or_insert(0usize) += 1;
        if *retryable {
            retryable_candidates = retryable_candidates.saturating_add(1);
        } else {
            non_retryable_candidates = non_retryable_candidates.saturating_add(1);
        }
        let ts = ts_of(item);
        oldest_ts = Some(oldest_ts.map_or(ts, |current| current.min(ts)));
        newest_ts = Some(newest_ts.map_or(ts, |current| current.max(ts)));
    }

    let oldest_candidate_age_minutes = oldest_ts
        .map(|ts| Utc::now().signed_duration_since(ts).num_minutes())
        .map(|minutes| minutes.max(0));
    let suggested_strategy = if selected.is_empty() {
        "no_candidates"
    } else if non_retryable_candidates > 0 {
        "inspect_non_retryable_first"
    } else if selected.len() >= 20 {
        "apply_in_batches"
    } else {
        "safe_to_apply"
    };

    json!({
        "selected_candidates": selected.len(),
        "retryable_candidates": retryable_candidates,
        "non_retryable_candidates": non_retryable_candidates,
        "oldest_candidate_ts": oldest_ts.map(|value| value.to_rfc3339()),
        "newest_candidate_ts": newest_ts.map(|value| value.to_rfc3339()),
        "oldest_candidate_age_minutes": oldest_candidate_age_minutes,
        "reason_histogram": reason_histogram,
        "suggested_strategy": suggested_strategy,
    })
}

pub(super) fn handle_hooks(cli: &Cli, args: HooksArgs) -> Result<()> {
    let paths = resolve_state_paths(cli.project_state)?;
    paths.ensure_dirs()?;
    let hooks_path = hooks_file_path(&paths.data_dir);
    let mut hooks = load_hooks_or_default(&hooks_path)?;
    match args.command {
        HooksCommand::List { event } => {
            let event_filter = event.map(|value| value.trim().to_string());
            let filtered = hooks
                .into_iter()
                .filter(|hook| {
                    if let Some(filter) = &event_filter {
                        hook.event == *filter
                    } else {
                        true
                    }
                })
                .collect::<Vec<_>>();
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "hooks": filtered,
                    "path": hooks_path.display().to_string(),
                }));
            } else if filtered.is_empty() {
                println!("No hooks configured.");
            } else {
                for hook in filtered {
                    let status = if hook.enabled { "enabled" } else { "disabled" };
                    println!(
                        "{} [{}] {} -> {}",
                        hook.id, status, hook.event, hook.command
                    );
                }
            }
        }
        HooksCommand::Add {
            name,
            event,
            command,
            disabled,
        } => {
            let name = name.trim().to_string();
            if name.is_empty() {
                return Err(MosaicError::Validation(
                    "hook name cannot be empty".to_string(),
                ));
            }
            let event = event.trim().to_string();
            if event.is_empty() {
                return Err(MosaicError::Validation(
                    "hook event cannot be empty".to_string(),
                ));
            }
            let command = command.trim().to_string();
            if command.is_empty() {
                return Err(MosaicError::Validation(
                    "hook command cannot be empty".to_string(),
                ));
            }
            let now = Utc::now();
            let hook = HookRecord {
                id: generate_hook_id(),
                name,
                event,
                command,
                enabled: !disabled,
                created_at: now,
                updated_at: now,
                last_triggered_at: None,
                last_result: None,
            };
            hooks.push(hook.clone());
            save_hooks(&hooks_path, &hooks)?;
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "hook": hook,
                    "path": hooks_path.display().to_string(),
                }));
            } else {
                println!("hook added: {}", hook.id);
                println!("event: {}", hook.event);
                println!("enabled: {}", hook.enabled);
            }
        }
        HooksCommand::Remove { hook_id } => {
            let before = hooks.len();
            hooks.retain(|hook| hook.id != hook_id);
            let removed = hooks.len() != before;
            if removed {
                save_hooks(&hooks_path, &hooks)?;
            }
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "removed": removed,
                    "hook_id": hook_id,
                }));
            } else if removed {
                println!("removed hook {hook_id}");
            } else {
                println!("hook {hook_id} not found");
            }
        }
        HooksCommand::Enable { hook_id } => {
            let hook = hooks
                .iter_mut()
                .find(|item| item.id == hook_id)
                .ok_or_else(|| MosaicError::Validation(format!("hook '{}' not found", hook_id)))?;
            hook.enabled = true;
            hook.updated_at = Utc::now();
            let hook = hook.clone();
            save_hooks(&hooks_path, &hooks)?;
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "hook": hook,
                    "path": hooks_path.display().to_string(),
                }));
            } else {
                println!("hook enabled: {}", hook.id);
            }
        }
        HooksCommand::Disable { hook_id } => {
            let hook = hooks
                .iter_mut()
                .find(|item| item.id == hook_id)
                .ok_or_else(|| MosaicError::Validation(format!("hook '{}' not found", hook_id)))?;
            hook.enabled = false;
            hook.updated_at = Utc::now();
            let hook = hook.clone();
            save_hooks(&hooks_path, &hooks)?;
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "hook": hook,
                    "path": hooks_path.display().to_string(),
                }));
            } else {
                println!("hook disabled: {}", hook.id);
            }
        }
        HooksCommand::Run { hook_id, data } => {
            let hook = hooks
                .iter()
                .find(|item| item.id == hook_id)
                .ok_or_else(|| MosaicError::Validation(format!("hook '{}' not found", hook_id)))?
                .clone();
            let payload = data
                .as_deref()
                .map(|value| parse_json_input(value, "hook data"))
                .transpose()?
                .unwrap_or(Value::Null);
            let report = execute_hook_command(cli, &paths, &hook, "manual", payload)?;
            apply_hook_last_result(&mut hooks, &hook.id, &report);
            save_hooks(&hooks_path, &hooks)?;
            if !report.ok {
                return Err(hook_execution_error(&hook.id, &report));
            }
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "hook": hook,
                    "result": report,
                }));
            } else {
                println!("hook executed: {}", hook.id);
                println!("status: success");
                if let Some(code) = report.exit_code {
                    println!("exit_code: {code}");
                }
            }
        }
        HooksCommand::Logs {
            hook,
            tail,
            since_minutes,
            summary,
        } => {
            let cutoff = resolve_logs_cutoff(since_minutes, "hooks logs")?;
            let read_tail = if cutoff.is_some() { usize::MAX } else { tail };
            let events = apply_logs_cutoff(
                read_hook_events(&paths.data_dir, hook.as_deref(), read_tail)?,
                tail,
                cutoff,
                |item| item.ts,
            );
            let summary_payload = if summary {
                let ok = events.iter().filter(|item| item.ok).count();
                let failed = events.len().saturating_sub(ok);
                let mut trigger_counts = BTreeMap::new();
                let mut delivery_counts = BTreeMap::new();
                for event in &events {
                    *trigger_counts
                        .entry(event.trigger.clone())
                        .or_insert(0usize) += 1;
                    *delivery_counts
                        .entry(event.delivery_status.clone())
                        .or_insert(0usize) += 1;
                }
                Some(json!({
                    "total": events.len(),
                    "ok": ok,
                    "failed": failed,
                    "by_trigger": trigger_counts,
                    "by_delivery_status": delivery_counts,
                }))
            } else {
                None
            };
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "events": events,
                    "summary": summary_payload,
                }));
            } else if events.is_empty() {
                println!("No hook events found.");
            } else {
                if let Some(summary_payload) = summary_payload {
                    println!(
                        "hooks logs summary total={} ok={} failed={}",
                        summary_payload["total"].as_u64().unwrap_or(0),
                        summary_payload["ok"].as_u64().unwrap_or(0),
                        summary_payload["failed"].as_u64().unwrap_or(0)
                    );
                    if let Some(by_trigger) = summary_payload["by_trigger"].as_object() {
                        for (trigger, count) in by_trigger {
                            println!("- trigger {}: {}", trigger, count);
                        }
                    }
                }
                if summary {
                    return Ok(());
                }
                for item in events {
                    let status = if item.ok { "ok" } else { "error" };
                    let code = item
                        .exit_code
                        .map(|value| value.to_string())
                        .unwrap_or_else(|| "-".to_string());
                    println!(
                        "{} [{}] hook={} event={} trigger={} code={} msg={}",
                        item.ts.to_rfc3339(),
                        status,
                        item.hook_id,
                        item.event,
                        item.trigger,
                        code,
                        item.error.unwrap_or_default(),
                    );
                }
            }
        }
        HooksCommand::Replay {
            hook,
            tail,
            limit,
            batch_size,
            since_minutes,
            reasons,
            retryable_only,
            apply,
            max_apply,
            stop_on_error,
            report_out,
        } => {
            if limit == 0 {
                return Err(MosaicError::Validation(
                    "hooks replay --limit must be greater than 0".to_string(),
                ));
            }
            if let Some(batch_size) = batch_size
                && batch_size == 0
            {
                return Err(MosaicError::Validation(
                    "hooks replay --batch-size must be greater than 0".to_string(),
                ));
            }
            if let Some(max_apply) = max_apply
                && max_apply == 0
            {
                return Err(MosaicError::Validation(
                    "hooks replay --max-apply must be greater than 0".to_string(),
                ));
            }
            if apply && !cli.yes {
                return Err(MosaicError::ApprovalRequired(
                    "hooks replay --apply requires --yes in non-interactive mode".to_string(),
                ));
            }

            let cutoff = resolve_logs_cutoff(since_minutes, "hooks replay")?;
            let read_tail = if cutoff.is_some() { usize::MAX } else { tail };
            let reason_filters = reasons
                .iter()
                .map(|reason| automation_reason_key(*reason))
                .collect::<BTreeSet<_>>();
            let reason_filters_json = reason_filters.iter().copied().collect::<Vec<_>>();
            let report_path = report_out.as_ref().map(|path| path.display().to_string());
            let events = apply_logs_cutoff(
                read_hook_events(&paths.data_dir, hook.as_deref(), read_tail)?,
                tail,
                cutoff,
                |item| item.ts,
            );
            let failed_events = events.iter().filter(|item| !item.ok).count();
            let mut selected = Vec::<(HookEventRecord, &'static str, bool)>::new();
            for item in events.into_iter().filter(|item| !item.ok).rev() {
                let reason = classify_hook_replay_reason(&item);
                if !reason_filters.is_empty() && !reason_filters.contains(reason) {
                    continue;
                }
                let retryable = is_retryable_reason(reason);
                if retryable_only && !retryable {
                    continue;
                }
                selected.push((item, reason, retryable));
                if selected.len() >= limit {
                    break;
                }
            }
            let candidates = selected
                .iter()
                .map(|(item, reason, retryable)| {
                    let data = serde_json::to_string(&item.data).unwrap_or_else(|_| "null".into());
                    json!({
                        "ts": item.ts,
                        "hook_id": item.hook_id,
                        "hook_name": item.hook_name,
                        "event": item.event,
                        "trigger": item.trigger,
                        "error_code": item.error_code,
                        "error": item.error,
                        "delivery_status": item.delivery_status,
                        "reason": reason,
                        "retryable": retryable,
                        "suggested_command": format!("mosaic --project-state --yes hooks run {} --data '{}'", item.hook_id, data),
                        "data": item.data,
                    })
                })
                .collect::<Vec<_>>();
            let batch_plan = build_automation_replay_batch_plan(&candidates, batch_size);
            let recovery_diagnostics =
                build_automation_replay_diagnostics(&selected, |item| item.ts);
            let write_report = |payload: &Value| -> Result<()> {
                if let Some(path) = report_out.as_ref() {
                    save_json_file(path, payload)?;
                }
                Ok(())
            };

            if !apply {
                let payload = json!({
                    "ok": true,
                    "apply": false,
                    "tail": tail,
                    "limit": limit,
                    "batch_size": batch_size,
                    "max_apply": max_apply,
                    "since_minutes": since_minutes,
                    "retryable_only": retryable_only,
                    "reason_filters": reason_filters_json,
                    "report_out": report_path,
                    "failed_events": failed_events,
                    "selected_candidates": candidates.len(),
                    "batch_plan": batch_plan.clone(),
                    "recovery_diagnostics": recovery_diagnostics.clone(),
                    "candidates": candidates,
                });
                write_report(&payload)?;
                if cli.json {
                    print_json(&payload);
                } else if candidates.is_empty() {
                    println!("No failed hook events to replay.");
                } else {
                    println!(
                        "Hook replay plan selected={} failed_events={} apply=false reason_filters={}",
                        candidates.len(),
                        failed_events,
                        if reason_filters.is_empty() {
                            "-".to_string()
                        } else {
                            reason_filters.iter().copied().collect::<Vec<_>>().join(",")
                        }
                    );
                    if !batch_plan.is_empty() {
                        println!(
                            "batch plan: requested_size={} batches={}",
                            batch_size.unwrap_or(candidates.len()),
                            batch_plan.len()
                        );
                    }
                    println!(
                        "diagnostics: retryable={} non_retryable={} oldest_age_minutes={} strategy={}",
                        recovery_diagnostics["retryable_candidates"]
                            .as_u64()
                            .unwrap_or(0),
                        recovery_diagnostics["non_retryable_candidates"]
                            .as_u64()
                            .unwrap_or(0),
                        recovery_diagnostics["oldest_candidate_age_minutes"]
                            .as_i64()
                            .unwrap_or(0),
                        recovery_diagnostics["suggested_strategy"]
                            .as_str()
                            .unwrap_or("-"),
                    );
                    for item in &candidates {
                        println!(
                            "- ts={} hook={} trigger={} reason={} retryable={} error={}",
                            item["ts"].as_str().unwrap_or("-"),
                            item["hook_id"].as_str().unwrap_or("-"),
                            item["trigger"].as_str().unwrap_or("-"),
                            item["reason"].as_str().unwrap_or("-"),
                            item["retryable"].as_bool().unwrap_or(false),
                            item["error"].as_str().unwrap_or("-")
                        );
                    }
                    if let Some(path) = report_path.as_deref() {
                        println!("report: {path}");
                    }
                }
                return Ok(());
            }

            let mut attempted = 0usize;
            let mut succeeded = 0usize;
            let mut failed = 0usize;
            let mut stopped_early = false;
            let mut apply_results = Vec::new();
            let mut changed = false;
            let planned_attempts = max_apply.unwrap_or(selected.len()).min(selected.len());
            let skipped_due_to_apply_limit = selected.len().saturating_sub(planned_attempts);

            for (item, reason, retryable) in selected.iter().rev().take(planned_attempts) {
                attempted = attempted.saturating_add(1);
                let Some(hook_record) =
                    hooks.iter().find(|entry| entry.id == item.hook_id).cloned()
                else {
                    failed = failed.saturating_add(1);
                    apply_results.push(json!({
                        "ts": item.ts,
                        "hook_id": item.hook_id,
                        "ok": false,
                        "error_code": "validation",
                        "reason": reason,
                        "retryable": retryable,
                        "error": format!("hook '{}' not found", item.hook_id),
                    }));
                    if stop_on_error {
                        stopped_early = true;
                        break;
                    }
                    continue;
                };

                let report =
                    execute_hook_command(cli, &paths, &hook_record, "replay", item.data.clone())?;
                apply_hook_last_result(&mut hooks, &hook_record.id, &report);
                changed = true;
                if report.ok {
                    succeeded = succeeded.saturating_add(1);
                } else {
                    failed = failed.saturating_add(1);
                }
                apply_results.push(json!({
                    "ts": item.ts,
                    "hook_id": hook_record.id,
                    "ok": report.ok,
                    "error_code": report.error_code,
                    "reason": reason,
                    "retryable": retryable,
                    "error": report.error,
                    "exit_code": report.exit_code,
                    "duration_ms": report.duration_ms,
                }));
                if stop_on_error && !report.ok {
                    stopped_early = true;
                    break;
                }
            }
            let skipped_due_to_stop_on_error = if stopped_early {
                planned_attempts.saturating_sub(attempted)
            } else {
                0
            };

            if changed {
                save_hooks(&hooks_path, &hooks)?;
            }

            let payload = json!({
                "ok": true,
                "apply": true,
                "tail": tail,
                "limit": limit,
                "batch_size": batch_size,
                "max_apply": max_apply,
                "since_minutes": since_minutes,
                "retryable_only": retryable_only,
                "reason_filters": reason_filters_json,
                "report_out": report_path,
                "failed_events": failed_events,
                "selected_candidates": candidates.len(),
                "batch_plan": batch_plan.clone(),
                "recovery_diagnostics": recovery_diagnostics,
                "planned_attempts": planned_attempts,
                "attempted": attempted,
                "succeeded": succeeded,
                "failed": failed,
                "stopped_early": stopped_early,
                "skipped_due_to_apply_limit": skipped_due_to_apply_limit,
                "skipped_due_to_stop_on_error": skipped_due_to_stop_on_error,
                "results": apply_results,
                "candidates": candidates,
            });
            write_report(&payload)?;
            if cli.json {
                print_json(&payload);
            } else {
                println!(
                    "Hook replay apply selected={} planned={} attempted={} succeeded={} failed={} stopped_early={} skipped_apply_limit={} skipped_stop_on_error={}",
                    candidates.len(),
                    planned_attempts,
                    attempted,
                    succeeded,
                    failed,
                    stopped_early,
                    skipped_due_to_apply_limit,
                    skipped_due_to_stop_on_error
                );
                if let Some(path) = report_path.as_deref() {
                    println!("report: {path}");
                }
            }
        }
    }
    Ok(())
}

pub(super) fn handle_cron(cli: &Cli, args: CronArgs) -> Result<()> {
    let paths = resolve_state_paths(cli.project_state)?;
    paths.ensure_dirs()?;
    let jobs_path = cron_jobs_file_path(&paths.data_dir);
    let mut jobs = load_cron_jobs_or_default(&jobs_path)?;
    match args.command {
        CronCommand::List { event } => {
            let event_filter = event.map(|value| value.trim().to_string());
            let filtered = jobs
                .into_iter()
                .filter(|job| {
                    if let Some(filter) = &event_filter {
                        job.event == *filter
                    } else {
                        true
                    }
                })
                .collect::<Vec<_>>();
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "jobs": filtered,
                    "path": jobs_path.display().to_string(),
                }));
            } else if filtered.is_empty() {
                println!("No cron jobs configured.");
            } else {
                for job in filtered {
                    let status = if job.enabled { "enabled" } else { "disabled" };
                    println!(
                        "{} [{}] every={}s event={} next={} name={}",
                        job.id,
                        status,
                        job.every_seconds,
                        job.event,
                        job.next_run_at.to_rfc3339(),
                        job.name
                    );
                }
            }
        }
        CronCommand::Add {
            name,
            event,
            every,
            data,
            disabled,
        } => {
            if every == 0 {
                return Err(MosaicError::Validation(
                    "--every must be greater than 0 seconds".to_string(),
                ));
            }
            let name = name.trim().to_string();
            if name.is_empty() {
                return Err(MosaicError::Validation(
                    "cron job name cannot be empty".to_string(),
                ));
            }
            let event = event.trim().to_string();
            if event.is_empty() {
                return Err(MosaicError::Validation(
                    "cron event cannot be empty".to_string(),
                ));
            }
            let data = data
                .as_deref()
                .map(|value| parse_json_input(value, "cron data"))
                .transpose()?
                .unwrap_or(Value::Null);
            let now = Utc::now();
            let job = CronJobRecord {
                id: generate_cron_job_id(),
                name,
                event,
                every_seconds: every,
                data,
                enabled: !disabled,
                created_at: now,
                updated_at: now,
                last_run_at: None,
                next_run_at: now,
                run_count: 0,
                last_result: None,
            };
            jobs.push(job.clone());
            save_cron_jobs(&jobs_path, &jobs)?;
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "job": job,
                    "path": jobs_path.display().to_string(),
                }));
            } else {
                println!("cron job added: {}", job.id);
                println!("event: {}", job.event);
                println!("every: {}s", job.every_seconds);
                println!("enabled: {}", job.enabled);
            }
        }
        CronCommand::Remove { job_id } => {
            let before = jobs.len();
            jobs.retain(|job| job.id != job_id);
            let removed = jobs.len() != before;
            if removed {
                save_cron_jobs(&jobs_path, &jobs)?;
            }
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "removed": removed,
                    "job_id": job_id,
                }));
            } else if removed {
                println!("removed cron job {job_id}");
            } else {
                println!("cron job {job_id} not found");
            }
        }
        CronCommand::Enable { job_id } => {
            let job = jobs
                .iter_mut()
                .find(|item| item.id == job_id)
                .ok_or_else(|| {
                    MosaicError::Validation(format!("cron job '{}' not found", job_id))
                })?;
            job.enabled = true;
            job.updated_at = Utc::now();
            let job = job.clone();
            save_cron_jobs(&jobs_path, &jobs)?;
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "job": job,
                    "path": jobs_path.display().to_string(),
                }));
            } else {
                println!("cron job enabled: {}", job.id);
            }
        }
        CronCommand::Disable { job_id } => {
            let job = jobs
                .iter_mut()
                .find(|item| item.id == job_id)
                .ok_or_else(|| {
                    MosaicError::Validation(format!("cron job '{}' not found", job_id))
                })?;
            job.enabled = false;
            job.updated_at = Utc::now();
            let job = job.clone();
            save_cron_jobs(&jobs_path, &jobs)?;
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "job": job,
                    "path": jobs_path.display().to_string(),
                }));
            } else {
                println!("cron job disabled: {}", job.id);
            }
        }
        CronCommand::Run { job_id, data } => {
            let index = jobs
                .iter()
                .position(|item| item.id == job_id)
                .ok_or_else(|| {
                    MosaicError::Validation(format!("cron job '{}' not found", job_id))
                })?;
            let payload_override = data
                .as_deref()
                .map(|value| parse_json_input(value, "cron data"))
                .transpose()?;
            let snapshot = jobs[index].clone();
            let report = execute_cron_job(cli, &paths, &snapshot, "manual", payload_override)?;
            apply_cron_result(&mut jobs[index], &report, Utc::now())?;
            save_cron_jobs(&jobs_path, &jobs)?;
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "job": jobs[index].clone(),
                    "result": report,
                }));
            } else {
                println!("cron job executed: {}", jobs[index].id);
                println!("ok: {}", report.ok);
                if let Some(error) = report.error {
                    println!("error: {error}");
                }
            }
        }
        CronCommand::Tick { limit } => {
            if limit == 0 {
                return Err(MosaicError::Validation(
                    "--limit must be greater than 0".to_string(),
                ));
            }
            let now = Utc::now();
            let mut due = jobs
                .iter()
                .enumerate()
                .filter(|(_, job)| job.enabled && job.next_run_at <= now)
                .map(|(index, _)| index)
                .collect::<Vec<_>>();
            due.sort_by_key(|index| jobs[*index].next_run_at);
            let mut reports = Vec::new();
            for index in due.into_iter().take(limit) {
                let snapshot = jobs[index].clone();
                let report = execute_cron_job(cli, &paths, &snapshot, "tick", None)?;
                apply_cron_result(&mut jobs[index], &report, Utc::now())?;
                reports.push(report);
            }
            if !reports.is_empty() {
                save_cron_jobs(&jobs_path, &jobs)?;
            }
            let ok_count = reports.iter().filter(|item| item.ok).count();
            let failed_count = reports.len().saturating_sub(ok_count);
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "triggered": reports.len(),
                    "ok_count": ok_count,
                    "failed_count": failed_count,
                    "results": reports,
                }));
            } else {
                println!("cron tick triggered: {}", reports.len());
                println!("ok: {ok_count}");
                println!("failed: {failed_count}");
            }
        }
        CronCommand::Logs {
            job,
            tail,
            since_minutes,
            summary,
        } => {
            let cutoff = resolve_logs_cutoff(since_minutes, "cron logs")?;
            let read_tail = if cutoff.is_some() { usize::MAX } else { tail };
            let events = apply_logs_cutoff(
                read_cron_events(&paths.data_dir, job.as_deref(), read_tail)?,
                tail,
                cutoff,
                |item| item.ts,
            );
            let summary_payload = if summary {
                let ok = events.iter().filter(|item| item.ok).count();
                let failed = events.len().saturating_sub(ok);
                let hooks_triggered = events
                    .iter()
                    .map(|item| item.hooks_triggered)
                    .sum::<usize>();
                let hooks_ok = events.iter().map(|item| item.hooks_ok).sum::<usize>();
                let hooks_failed = events.iter().map(|item| item.hooks_failed).sum::<usize>();
                let mut trigger_counts = BTreeMap::new();
                for event in &events {
                    *trigger_counts
                        .entry(event.trigger.clone())
                        .or_insert(0usize) += 1;
                }
                Some(json!({
                    "total": events.len(),
                    "ok": ok,
                    "failed": failed,
                    "hooks_triggered": hooks_triggered,
                    "hooks_ok": hooks_ok,
                    "hooks_failed": hooks_failed,
                    "by_trigger": trigger_counts,
                }))
            } else {
                None
            };
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "events": events,
                    "summary": summary_payload,
                }));
            } else if events.is_empty() {
                println!("No cron events found.");
            } else {
                if let Some(summary_payload) = summary_payload {
                    println!(
                        "cron logs summary total={} ok={} failed={} hooks={}/{}",
                        summary_payload["total"].as_u64().unwrap_or(0),
                        summary_payload["ok"].as_u64().unwrap_or(0),
                        summary_payload["failed"].as_u64().unwrap_or(0),
                        summary_payload["hooks_ok"].as_u64().unwrap_or(0),
                        summary_payload["hooks_triggered"].as_u64().unwrap_or(0)
                    );
                    if let Some(by_trigger) = summary_payload["by_trigger"].as_object() {
                        for (trigger, count) in by_trigger {
                            println!("- trigger {}: {}", trigger, count);
                        }
                    }
                }
                if summary {
                    return Ok(());
                }
                for item in events {
                    let status = if item.ok { "ok" } else { "error" };
                    println!(
                        "{} [{}] job={} trigger={} event={} hooks={}/{} error={}",
                        item.ts.to_rfc3339(),
                        status,
                        item.job_id,
                        item.trigger,
                        item.event,
                        item.hooks_ok,
                        item.hooks_triggered,
                        item.error.unwrap_or_default(),
                    );
                }
            }
        }
        CronCommand::Replay {
            job,
            tail,
            limit,
            batch_size,
            since_minutes,
            reasons,
            retryable_only,
            apply,
            max_apply,
            stop_on_error,
            report_out,
        } => {
            if limit == 0 {
                return Err(MosaicError::Validation(
                    "cron replay --limit must be greater than 0".to_string(),
                ));
            }
            if let Some(batch_size) = batch_size
                && batch_size == 0
            {
                return Err(MosaicError::Validation(
                    "cron replay --batch-size must be greater than 0".to_string(),
                ));
            }
            if let Some(max_apply) = max_apply
                && max_apply == 0
            {
                return Err(MosaicError::Validation(
                    "cron replay --max-apply must be greater than 0".to_string(),
                ));
            }
            if apply && !cli.yes {
                return Err(MosaicError::ApprovalRequired(
                    "cron replay --apply requires --yes in non-interactive mode".to_string(),
                ));
            }

            let cutoff = resolve_logs_cutoff(since_minutes, "cron replay")?;
            let read_tail = if cutoff.is_some() { usize::MAX } else { tail };
            let reason_filters = reasons
                .iter()
                .map(|reason| automation_reason_key(*reason))
                .collect::<BTreeSet<_>>();
            let reason_filters_json = reason_filters.iter().copied().collect::<Vec<_>>();
            let report_path = report_out.as_ref().map(|path| path.display().to_string());
            let events = apply_logs_cutoff(
                read_cron_events(&paths.data_dir, job.as_deref(), read_tail)?,
                tail,
                cutoff,
                |item| item.ts,
            );
            let failed_events = events.iter().filter(|item| !item.ok).count();
            let mut selected = Vec::<(CronEventRecord, &'static str, bool)>::new();
            for item in events.into_iter().filter(|item| !item.ok).rev() {
                let reason = classify_cron_replay_reason(&item);
                if !reason_filters.is_empty() && !reason_filters.contains(reason) {
                    continue;
                }
                let retryable = is_retryable_reason(reason);
                if retryable_only && !retryable {
                    continue;
                }
                selected.push((item, reason, retryable));
                if selected.len() >= limit {
                    break;
                }
            }
            let candidates = selected
                .iter()
                .map(|(item, reason, retryable)| {
                    let data = serde_json::to_string(&item.data).unwrap_or_else(|_| "null".into());
                    json!({
                        "ts": item.ts,
                        "job_id": item.job_id,
                        "job_name": item.job_name,
                        "event": item.event,
                        "trigger": item.trigger,
                        "error": item.error,
                        "hooks_failed": item.hooks_failed,
                        "reason": reason,
                        "retryable": retryable,
                        "suggested_command": format!("mosaic --project-state --yes cron run {} --data '{}'", item.job_id, data),
                        "data": item.data,
                    })
                })
                .collect::<Vec<_>>();
            let batch_plan = build_automation_replay_batch_plan(&candidates, batch_size);
            let recovery_diagnostics =
                build_automation_replay_diagnostics(&selected, |item| item.ts);
            let write_report = |payload: &Value| -> Result<()> {
                if let Some(path) = report_out.as_ref() {
                    save_json_file(path, payload)?;
                }
                Ok(())
            };

            if !apply {
                let payload = json!({
                    "ok": true,
                    "apply": false,
                    "tail": tail,
                    "limit": limit,
                    "batch_size": batch_size,
                    "max_apply": max_apply,
                    "since_minutes": since_minutes,
                    "retryable_only": retryable_only,
                    "reason_filters": reason_filters_json,
                    "report_out": report_path,
                    "failed_events": failed_events,
                    "selected_candidates": candidates.len(),
                    "batch_plan": batch_plan.clone(),
                    "recovery_diagnostics": recovery_diagnostics.clone(),
                    "candidates": candidates,
                });
                write_report(&payload)?;
                if cli.json {
                    print_json(&payload);
                } else if candidates.is_empty() {
                    println!("No failed cron events to replay.");
                } else {
                    println!(
                        "Cron replay plan selected={} failed_events={} apply=false reason_filters={}",
                        candidates.len(),
                        failed_events,
                        if reason_filters.is_empty() {
                            "-".to_string()
                        } else {
                            reason_filters.iter().copied().collect::<Vec<_>>().join(",")
                        }
                    );
                    if !batch_plan.is_empty() {
                        println!(
                            "batch plan: requested_size={} batches={}",
                            batch_size.unwrap_or(candidates.len()),
                            batch_plan.len()
                        );
                    }
                    println!(
                        "diagnostics: retryable={} non_retryable={} oldest_age_minutes={} strategy={}",
                        recovery_diagnostics["retryable_candidates"]
                            .as_u64()
                            .unwrap_or(0),
                        recovery_diagnostics["non_retryable_candidates"]
                            .as_u64()
                            .unwrap_or(0),
                        recovery_diagnostics["oldest_candidate_age_minutes"]
                            .as_i64()
                            .unwrap_or(0),
                        recovery_diagnostics["suggested_strategy"]
                            .as_str()
                            .unwrap_or("-"),
                    );
                    for item in &candidates {
                        println!(
                            "- ts={} job={} trigger={} reason={} retryable={} error={}",
                            item["ts"].as_str().unwrap_or("-"),
                            item["job_id"].as_str().unwrap_or("-"),
                            item["trigger"].as_str().unwrap_or("-"),
                            item["reason"].as_str().unwrap_or("-"),
                            item["retryable"].as_bool().unwrap_or(false),
                            item["error"].as_str().unwrap_or("-")
                        );
                    }
                    if let Some(path) = report_path.as_deref() {
                        println!("report: {path}");
                    }
                }
                return Ok(());
            }

            let mut attempted = 0usize;
            let mut succeeded = 0usize;
            let mut failed = 0usize;
            let mut stopped_early = false;
            let mut apply_results = Vec::new();
            let mut changed = false;
            let planned_attempts = max_apply.unwrap_or(selected.len()).min(selected.len());
            let skipped_due_to_apply_limit = selected.len().saturating_sub(planned_attempts);

            for (item, reason, retryable) in selected.iter().rev().take(planned_attempts) {
                attempted = attempted.saturating_add(1);
                let Some(index) = jobs.iter().position(|entry| entry.id == item.job_id) else {
                    failed = failed.saturating_add(1);
                    apply_results.push(json!({
                        "ts": item.ts,
                        "job_id": item.job_id,
                        "ok": false,
                        "error_code": "validation",
                        "reason": reason,
                        "retryable": retryable,
                        "error": format!("cron job '{}' not found", item.job_id),
                    }));
                    if stop_on_error {
                        stopped_early = true;
                        break;
                    }
                    continue;
                };

                let snapshot = jobs[index].clone();
                let report =
                    execute_cron_job(cli, &paths, &snapshot, "replay", Some(item.data.clone()))?;
                apply_cron_result(&mut jobs[index], &report, Utc::now())?;
                changed = true;
                if report.ok {
                    succeeded = succeeded.saturating_add(1);
                } else {
                    failed = failed.saturating_add(1);
                }
                apply_results.push(json!({
                    "ts": item.ts,
                    "job_id": snapshot.id,
                    "ok": report.ok,
                    "hooks_triggered": report.hooks_triggered,
                    "hooks_ok": report.hooks_ok,
                    "hooks_failed": report.hooks_failed,
                    "reason": reason,
                    "retryable": retryable,
                    "error": report.error,
                }));
                if stop_on_error && !report.ok {
                    stopped_early = true;
                    break;
                }
            }
            let skipped_due_to_stop_on_error = if stopped_early {
                planned_attempts.saturating_sub(attempted)
            } else {
                0
            };

            if changed {
                save_cron_jobs(&jobs_path, &jobs)?;
            }

            let payload = json!({
                "ok": true,
                "apply": true,
                "tail": tail,
                "limit": limit,
                "batch_size": batch_size,
                "max_apply": max_apply,
                "since_minutes": since_minutes,
                "retryable_only": retryable_only,
                "reason_filters": reason_filters_json,
                "report_out": report_path,
                "failed_events": failed_events,
                "selected_candidates": candidates.len(),
                "batch_plan": batch_plan.clone(),
                "recovery_diagnostics": recovery_diagnostics,
                "planned_attempts": planned_attempts,
                "attempted": attempted,
                "succeeded": succeeded,
                "failed": failed,
                "stopped_early": stopped_early,
                "skipped_due_to_apply_limit": skipped_due_to_apply_limit,
                "skipped_due_to_stop_on_error": skipped_due_to_stop_on_error,
                "results": apply_results,
                "candidates": candidates,
            });
            write_report(&payload)?;
            if cli.json {
                print_json(&payload);
            } else {
                println!(
                    "Cron replay apply selected={} planned={} attempted={} succeeded={} failed={} stopped_early={} skipped_apply_limit={} skipped_stop_on_error={}",
                    candidates.len(),
                    planned_attempts,
                    attempted,
                    succeeded,
                    failed,
                    stopped_early,
                    skipped_due_to_apply_limit,
                    skipped_due_to_stop_on_error
                );
                if let Some(path) = report_path.as_deref() {
                    println!("report: {path}");
                }
            }
        }
    }
    Ok(())
}

pub(super) fn handle_webhooks(cli: &Cli, args: WebhooksArgs) -> Result<()> {
    let paths = resolve_state_paths(cli.project_state)?;
    paths.ensure_dirs()?;
    let webhooks_path = webhooks_file_path(&paths.data_dir);
    let mut webhooks = load_webhooks_or_default(&webhooks_path)?;
    match args.command {
        WebhooksCommand::List { event } => {
            let event_filter = event.map(|value| value.trim().to_string());
            let filtered = webhooks
                .into_iter()
                .filter(|webhook| {
                    if let Some(filter) = &event_filter {
                        webhook.event == *filter
                    } else {
                        true
                    }
                })
                .collect::<Vec<_>>();
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "webhooks": filtered,
                    "path": webhooks_path.display().to_string(),
                }));
            } else if filtered.is_empty() {
                println!("No webhooks configured.");
            } else {
                for webhook in filtered {
                    let status = if webhook.enabled {
                        "enabled"
                    } else {
                        "disabled"
                    };
                    println!(
                        "{} [{}] {} {} -> {} ({})",
                        webhook.id,
                        status,
                        webhook.method,
                        webhook.path,
                        webhook.event,
                        webhook.name
                    );
                }
            }
        }
        WebhooksCommand::Add {
            name,
            event,
            path,
            method,
            secret_env,
            disabled,
        } => {
            let name = name.trim().to_string();
            if name.is_empty() {
                return Err(MosaicError::Validation(
                    "webhook name cannot be empty".to_string(),
                ));
            }
            let event = event.trim().to_string();
            if event.is_empty() {
                return Err(MosaicError::Validation(
                    "webhook event cannot be empty".to_string(),
                ));
            }
            let path = normalize_webhook_path(&path)?;
            let secret_env = normalize_optional_secret_env(secret_env)?;
            let now = Utc::now();
            let webhook = WebhookRecord {
                id: generate_webhook_id(),
                name,
                event,
                path,
                method: method.into(),
                secret_env,
                enabled: !disabled,
                created_at: now,
                updated_at: now,
                last_triggered_at: None,
                last_result: None,
            };
            webhooks.push(webhook.clone());
            save_webhooks(&webhooks_path, &webhooks)?;
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "webhook": webhook,
                    "path": webhooks_path.display().to_string(),
                }));
            } else {
                println!("webhook added: {}", webhook.id);
                println!("route: {} {}", webhook.method, webhook.path);
                println!("event: {}", webhook.event);
            }
        }
        WebhooksCommand::Remove { webhook_id } => {
            let before = webhooks.len();
            webhooks.retain(|item| item.id != webhook_id);
            let removed = webhooks.len() != before;
            if removed {
                save_webhooks(&webhooks_path, &webhooks)?;
            }
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "removed": removed,
                    "webhook_id": webhook_id,
                }));
            } else if removed {
                println!("removed webhook {webhook_id}");
            } else {
                println!("webhook {webhook_id} not found");
            }
        }
        WebhooksCommand::Enable { webhook_id } => {
            let webhook = webhooks
                .iter_mut()
                .find(|item| item.id == webhook_id)
                .ok_or_else(|| {
                    MosaicError::Validation(format!("webhook '{}' not found", webhook_id))
                })?;
            webhook.enabled = true;
            webhook.updated_at = Utc::now();
            let webhook = webhook.clone();
            save_webhooks(&webhooks_path, &webhooks)?;
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "webhook": webhook,
                    "path": webhooks_path.display().to_string(),
                }));
            } else {
                println!("webhook enabled: {}", webhook.id);
            }
        }
        WebhooksCommand::Disable { webhook_id } => {
            let webhook = webhooks
                .iter_mut()
                .find(|item| item.id == webhook_id)
                .ok_or_else(|| {
                    MosaicError::Validation(format!("webhook '{}' not found", webhook_id))
                })?;
            webhook.enabled = false;
            webhook.updated_at = Utc::now();
            let webhook = webhook.clone();
            save_webhooks(&webhooks_path, &webhooks)?;
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "webhook": webhook,
                    "path": webhooks_path.display().to_string(),
                }));
            } else {
                println!("webhook disabled: {}", webhook.id);
            }
        }
        WebhooksCommand::Trigger {
            webhook_id,
            data,
            secret,
        } => {
            let index = webhooks
                .iter()
                .position(|item| item.id == webhook_id)
                .ok_or_else(|| {
                    MosaicError::Validation(format!("webhook '{}' not found", webhook_id))
                })?;
            let payload = data
                .as_deref()
                .map(|value| parse_json_input(value, "webhook data"))
                .transpose()?
                .unwrap_or(Value::Null);
            let snapshot = webhooks[index].clone();
            let report =
                execute_webhook(cli, &paths, &snapshot, "manual", payload, secret.as_deref())?;
            apply_webhook_last_result(&mut webhooks[index], &report);
            save_webhooks(&webhooks_path, &webhooks)?;
            if !report.ok {
                return Err(webhook_execution_error(&snapshot.id, &report));
            }
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "webhook": webhooks[index].clone(),
                    "result": report,
                }));
            } else {
                println!("webhook triggered: {}", snapshot.id);
                println!("event: {}", snapshot.event);
            }
        }
        WebhooksCommand::Resolve {
            path,
            method,
            data,
            secret,
        } => {
            let path = normalize_webhook_path(&path)?;
            let method: WebhookMethod = method.into();
            let index = webhooks
                .iter()
                .position(|item| item.enabled && item.path == path && item.method == method)
                .ok_or_else(|| {
                    MosaicError::Validation(format!(
                        "no enabled webhook matched {} {}",
                        method, path
                    ))
                })?;
            let payload = data
                .as_deref()
                .map(|value| parse_json_input(value, "webhook data"))
                .transpose()?
                .unwrap_or(Value::Null);
            let snapshot = webhooks[index].clone();
            let report = execute_webhook(
                cli,
                &paths,
                &snapshot,
                "resolve",
                payload,
                secret.as_deref(),
            )?;
            apply_webhook_last_result(&mut webhooks[index], &report);
            save_webhooks(&webhooks_path, &webhooks)?;
            if !report.ok {
                return Err(webhook_execution_error(&snapshot.id, &report));
            }
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "webhook": webhooks[index].clone(),
                    "result": report,
                }));
            } else {
                println!("webhook resolved: {}", snapshot.id);
                println!("route: {} {}", snapshot.method, snapshot.path);
                println!("event: {}", snapshot.event);
            }
        }
        WebhooksCommand::Logs {
            webhook,
            tail,
            since_minutes,
            summary,
        } => {
            let cutoff = resolve_logs_cutoff(since_minutes, "webhooks logs")?;
            let read_tail = if cutoff.is_some() { usize::MAX } else { tail };
            let events = apply_logs_cutoff(
                read_webhook_events(&paths.data_dir, webhook.as_deref(), read_tail)?,
                tail,
                cutoff,
                |item| item.ts,
            );
            let summary_payload = if summary {
                let ok = events.iter().filter(|item| item.ok).count();
                let failed = events.len().saturating_sub(ok);
                let hooks_triggered = events
                    .iter()
                    .map(|item| item.hooks_triggered)
                    .sum::<usize>();
                let hooks_ok = events.iter().map(|item| item.hooks_ok).sum::<usize>();
                let hooks_failed = events.iter().map(|item| item.hooks_failed).sum::<usize>();
                let mut trigger_counts = BTreeMap::new();
                let mut method_counts = BTreeMap::new();
                for event in &events {
                    *trigger_counts
                        .entry(event.trigger.clone())
                        .or_insert(0usize) += 1;
                    *method_counts
                        .entry(event.method.to_string())
                        .or_insert(0usize) += 1;
                }
                Some(json!({
                    "total": events.len(),
                    "ok": ok,
                    "failed": failed,
                    "hooks_triggered": hooks_triggered,
                    "hooks_ok": hooks_ok,
                    "hooks_failed": hooks_failed,
                    "by_trigger": trigger_counts,
                    "by_method": method_counts,
                }))
            } else {
                None
            };
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "events": events,
                    "summary": summary_payload,
                }));
            } else if events.is_empty() {
                println!("No webhook events found.");
            } else {
                if let Some(summary_payload) = summary_payload {
                    println!(
                        "webhooks logs summary total={} ok={} failed={} hooks={}/{}",
                        summary_payload["total"].as_u64().unwrap_or(0),
                        summary_payload["ok"].as_u64().unwrap_or(0),
                        summary_payload["failed"].as_u64().unwrap_or(0),
                        summary_payload["hooks_ok"].as_u64().unwrap_or(0),
                        summary_payload["hooks_triggered"].as_u64().unwrap_or(0)
                    );
                    if let Some(by_trigger) = summary_payload["by_trigger"].as_object() {
                        for (trigger, count) in by_trigger {
                            println!("- trigger {}: {}", trigger, count);
                        }
                    }
                }
                if summary {
                    return Ok(());
                }
                for event in events {
                    let status = if event.ok { "ok" } else { "error" };
                    println!(
                        "{} [{}] webhook={} trigger={} route={} {} event={} hooks={}/{} error={}",
                        event.ts.to_rfc3339(),
                        status,
                        event.webhook_id,
                        event.trigger,
                        event.method,
                        event.path,
                        event.event,
                        event.hooks_ok,
                        event.hooks_triggered,
                        event.error.unwrap_or_default(),
                    );
                }
            }
        }
        WebhooksCommand::Replay {
            webhook,
            tail,
            limit,
            batch_size,
            since_minutes,
            reasons,
            retryable_only,
            secret,
            apply,
            max_apply,
            stop_on_error,
            report_out,
        } => {
            if limit == 0 {
                return Err(MosaicError::Validation(
                    "webhooks replay --limit must be greater than 0".to_string(),
                ));
            }
            if let Some(batch_size) = batch_size
                && batch_size == 0
            {
                return Err(MosaicError::Validation(
                    "webhooks replay --batch-size must be greater than 0".to_string(),
                ));
            }
            if let Some(max_apply) = max_apply
                && max_apply == 0
            {
                return Err(MosaicError::Validation(
                    "webhooks replay --max-apply must be greater than 0".to_string(),
                ));
            }
            if apply && !cli.yes {
                return Err(MosaicError::ApprovalRequired(
                    "webhooks replay --apply requires --yes in non-interactive mode".to_string(),
                ));
            }

            let cutoff = resolve_logs_cutoff(since_minutes, "webhooks replay")?;
            let read_tail = if cutoff.is_some() { usize::MAX } else { tail };
            let reason_filters = reasons
                .iter()
                .map(|reason| automation_reason_key(*reason))
                .collect::<BTreeSet<_>>();
            let reason_filters_json = reason_filters.iter().copied().collect::<Vec<_>>();
            let report_path = report_out.as_ref().map(|path| path.display().to_string());
            let events = apply_logs_cutoff(
                read_webhook_events(&paths.data_dir, webhook.as_deref(), read_tail)?,
                tail,
                cutoff,
                |item| item.ts,
            );
            let failed_events = events.iter().filter(|item| !item.ok).count();
            let mut selected = Vec::<(WebhookEventRecord, &'static str, bool)>::new();
            for item in events.into_iter().filter(|item| !item.ok).rev() {
                let reason = classify_webhook_replay_reason(&item);
                if !reason_filters.is_empty() && !reason_filters.contains(reason) {
                    continue;
                }
                let retryable = is_retryable_reason(reason);
                if retryable_only && !retryable {
                    continue;
                }
                selected.push((item, reason, retryable));
                if selected.len() >= limit {
                    break;
                }
            }
            let candidates = selected
                .iter()
                .map(|(item, reason, retryable)| {
                    let data = serde_json::to_string(&item.data).unwrap_or_else(|_| "null".into());
                    let secret_env = webhooks
                        .iter()
                        .find(|entry| entry.id == item.webhook_id)
                        .and_then(|entry| entry.secret_env.clone());
                    let requires_secret = secret_env.is_some();
                    let suggested_secret = secret_env
                        .map(|env| format!(" --secret \"${{{env}}}\""))
                        .unwrap_or_default();
                    json!({
                        "ts": item.ts,
                        "webhook_id": item.webhook_id,
                        "webhook_name": item.webhook_name,
                        "event": item.event,
                        "trigger": item.trigger,
                        "path": item.path,
                        "method": item.method.to_string(),
                        "error_code": item.error_code,
                        "error": item.error,
                        "reason": reason,
                        "retryable": retryable,
                        "requires_secret": requires_secret,
                        "suggested_command": format!("mosaic --project-state --yes webhooks trigger {} --data '{}'{}", item.webhook_id, data, suggested_secret),
                        "data": item.data,
                    })
                })
                .collect::<Vec<_>>();
            let batch_plan = build_automation_replay_batch_plan(&candidates, batch_size);
            let recovery_diagnostics =
                build_automation_replay_diagnostics(&selected, |item| item.ts);
            let write_report = |payload: &Value| -> Result<()> {
                if let Some(path) = report_out.as_ref() {
                    save_json_file(path, payload)?;
                }
                Ok(())
            };

            if !apply {
                let payload = json!({
                    "ok": true,
                    "apply": false,
                    "tail": tail,
                    "limit": limit,
                    "batch_size": batch_size,
                    "max_apply": max_apply,
                    "since_minutes": since_minutes,
                    "retryable_only": retryable_only,
                    "reason_filters": reason_filters_json,
                    "report_out": report_path,
                    "failed_events": failed_events,
                    "selected_candidates": candidates.len(),
                    "batch_plan": batch_plan.clone(),
                    "recovery_diagnostics": recovery_diagnostics.clone(),
                    "candidates": candidates,
                });
                write_report(&payload)?;
                if cli.json {
                    print_json(&payload);
                } else if candidates.is_empty() {
                    println!("No failed webhook events to replay.");
                } else {
                    println!(
                        "Webhook replay plan selected={} failed_events={} apply=false reason_filters={}",
                        candidates.len(),
                        failed_events,
                        if reason_filters.is_empty() {
                            "-".to_string()
                        } else {
                            reason_filters.iter().copied().collect::<Vec<_>>().join(",")
                        }
                    );
                    if !batch_plan.is_empty() {
                        println!(
                            "batch plan: requested_size={} batches={}",
                            batch_size.unwrap_or(candidates.len()),
                            batch_plan.len()
                        );
                    }
                    println!(
                        "diagnostics: retryable={} non_retryable={} oldest_age_minutes={} strategy={}",
                        recovery_diagnostics["retryable_candidates"]
                            .as_u64()
                            .unwrap_or(0),
                        recovery_diagnostics["non_retryable_candidates"]
                            .as_u64()
                            .unwrap_or(0),
                        recovery_diagnostics["oldest_candidate_age_minutes"]
                            .as_i64()
                            .unwrap_or(0),
                        recovery_diagnostics["suggested_strategy"]
                            .as_str()
                            .unwrap_or("-"),
                    );
                    for item in &candidates {
                        println!(
                            "- ts={} webhook={} trigger={} reason={} retryable={} error={} requires_secret={}",
                            item["ts"].as_str().unwrap_or("-"),
                            item["webhook_id"].as_str().unwrap_or("-"),
                            item["trigger"].as_str().unwrap_or("-"),
                            item["reason"].as_str().unwrap_or("-"),
                            item["retryable"].as_bool().unwrap_or(false),
                            item["error"].as_str().unwrap_or("-"),
                            item["requires_secret"].as_bool().unwrap_or(false)
                        );
                    }
                    if let Some(path) = report_path.as_deref() {
                        println!("report: {path}");
                    }
                }
                return Ok(());
            }

            let mut attempted = 0usize;
            let mut succeeded = 0usize;
            let mut failed = 0usize;
            let mut stopped_early = false;
            let mut apply_results = Vec::new();
            let mut changed = false;
            let planned_attempts = max_apply.unwrap_or(selected.len()).min(selected.len());
            let skipped_due_to_apply_limit = selected.len().saturating_sub(planned_attempts);

            for (item, reason, retryable) in selected.iter().rev().take(planned_attempts) {
                attempted = attempted.saturating_add(1);
                let Some(index) = webhooks
                    .iter()
                    .position(|entry| entry.id == item.webhook_id)
                else {
                    failed = failed.saturating_add(1);
                    apply_results.push(json!({
                        "ts": item.ts,
                        "webhook_id": item.webhook_id,
                        "ok": false,
                        "error_code": "validation",
                        "reason": reason,
                        "retryable": retryable,
                        "error": format!("webhook '{}' not found", item.webhook_id),
                    }));
                    if stop_on_error {
                        stopped_early = true;
                        break;
                    }
                    continue;
                };

                let snapshot = webhooks[index].clone();
                let resolved_secret = if let Some(value) = secret.clone() {
                    Some(value)
                } else if let Some(secret_env) = snapshot.secret_env.clone() {
                    match std::env::var(&secret_env) {
                        Ok(value) if !value.is_empty() => Some(value),
                        Ok(_) => {
                            failed = failed.saturating_add(1);
                            apply_results.push(json!({
                                "ts": item.ts,
                                "webhook_id": snapshot.id,
                                "ok": false,
                                "error_code": "auth",
                                "reason": "auth",
                                "retryable": false,
                                "error": format!("webhook '{}' secret env '{}' is empty", snapshot.id, secret_env),
                            }));
                            if stop_on_error {
                                stopped_early = true;
                                break;
                            }
                            continue;
                        }
                        Err(_) => {
                            failed = failed.saturating_add(1);
                            apply_results.push(json!({
                                "ts": item.ts,
                                "webhook_id": snapshot.id,
                                "ok": false,
                                "error_code": "auth",
                                "reason": "auth",
                                "retryable": false,
                                "error": format!("webhook '{}' secret env '{}' is not set", snapshot.id, secret_env),
                            }));
                            if stop_on_error {
                                stopped_early = true;
                                break;
                            }
                            continue;
                        }
                    }
                } else {
                    None
                };

                let report = execute_webhook(
                    cli,
                    &paths,
                    &snapshot,
                    "replay",
                    item.data.clone(),
                    resolved_secret.as_deref(),
                )?;
                apply_webhook_last_result(&mut webhooks[index], &report);
                changed = true;
                if report.ok {
                    succeeded = succeeded.saturating_add(1);
                } else {
                    failed = failed.saturating_add(1);
                }
                apply_results.push(json!({
                    "ts": item.ts,
                    "webhook_id": snapshot.id,
                    "ok": report.ok,
                    "hooks_triggered": report.hooks_triggered,
                    "hooks_ok": report.hooks_ok,
                    "hooks_failed": report.hooks_failed,
                    "error_code": report.error_code,
                    "reason": reason,
                    "retryable": retryable,
                    "error": report.error,
                }));
                if stop_on_error && !report.ok {
                    stopped_early = true;
                    break;
                }
            }
            let skipped_due_to_stop_on_error = if stopped_early {
                planned_attempts.saturating_sub(attempted)
            } else {
                0
            };

            if changed {
                save_webhooks(&webhooks_path, &webhooks)?;
            }

            let payload = json!({
                "ok": true,
                "apply": true,
                "tail": tail,
                "limit": limit,
                "batch_size": batch_size,
                "max_apply": max_apply,
                "since_minutes": since_minutes,
                "retryable_only": retryable_only,
                "reason_filters": reason_filters_json,
                "report_out": report_path,
                "failed_events": failed_events,
                "selected_candidates": candidates.len(),
                "batch_plan": batch_plan.clone(),
                "recovery_diagnostics": recovery_diagnostics,
                "planned_attempts": planned_attempts,
                "attempted": attempted,
                "succeeded": succeeded,
                "failed": failed,
                "stopped_early": stopped_early,
                "skipped_due_to_apply_limit": skipped_due_to_apply_limit,
                "skipped_due_to_stop_on_error": skipped_due_to_stop_on_error,
                "results": apply_results,
                "candidates": candidates,
            });
            write_report(&payload)?;
            if cli.json {
                print_json(&payload);
            } else {
                println!(
                    "Webhook replay apply selected={} planned={} attempted={} succeeded={} failed={} stopped_early={} skipped_apply_limit={} skipped_stop_on_error={}",
                    candidates.len(),
                    planned_attempts,
                    attempted,
                    succeeded,
                    failed,
                    stopped_early,
                    skipped_due_to_apply_limit,
                    skipped_due_to_stop_on_error
                );
                if let Some(path) = report_path.as_deref() {
                    println!("report: {path}");
                }
            }
        }
    }
    Ok(())
}
