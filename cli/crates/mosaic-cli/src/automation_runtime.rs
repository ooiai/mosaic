use chrono::{DateTime, Duration as ChronoDuration, Utc};
use serde_json::{Value, json};

use mosaic_core::config::RunGuardMode;
use mosaic_core::error::MosaicError;
use mosaic_core::state::StatePaths;
use mosaic_memory::{
    MemoryCleanupPolicy, MemoryCleanupPolicyStore, MemoryPruneOptions, memory_cleanup_policy_path,
    prune_memory_namespaces,
};
use mosaic_ops::{
    ApprovalStore, RuntimePolicy, SandboxStore, SystemEventStore, system_events_path,
};
use mosaic_tools::{RunCommandOutput, ToolContext, ToolExecutor};

use super::{
    Cli, CronEventRecord, CronExecutionReport, CronJobRecord, CronLastResult, HookEventRecord,
    HookExecutionReport, HookLastResult, HookRecord, Result, SystemEventDispatch,
    WebhookEventRecord, WebhookExecutionReport, WebhookLastResult, WebhookRecord, cron_events_dir,
    cron_events_file_path, hook_events_dir, hook_events_file_path, hooks_file_path,
    load_hooks_or_default, preview_text, save_hooks, webhook_events_dir, webhook_events_file_path,
};

const MEMORY_CLEANUP_EVENT: &str = "mosaic.memory.cleanup";
const MEMORY_CLEANUP_EVENT_ALIAS: &str = "memory.cleanup";

pub(super) fn dispatch_system_event(
    cli: &Cli,
    paths: &StatePaths,
    event_name: &str,
    data: Value,
) -> Result<SystemEventDispatch> {
    let store = SystemEventStore::new(system_events_path(&paths.data_dir));
    let event = store.append_event(event_name, data.clone())?;
    run_builtin_system_event_handlers(paths, event_name, &data)?;
    let hook_reports = run_hooks_for_system_event(cli, paths, event_name, data)?;
    Ok(SystemEventDispatch {
        event,
        hook_reports,
    })
}

fn run_builtin_system_event_handlers(
    paths: &StatePaths,
    event_name: &str,
    data: &Value,
) -> Result<()> {
    match event_name {
        MEMORY_CLEANUP_EVENT | MEMORY_CLEANUP_EVENT_ALIAS => {
            execute_memory_cleanup_event(paths, event_name, data)
        }
        _ => Ok(()),
    }
}

fn execute_memory_cleanup_event(paths: &StatePaths, event_name: &str, data: &Value) -> Result<()> {
    let dry_run = parse_optional_event_bool(event_name, data, "dry_run")?.unwrap_or(false);
    let force = parse_optional_event_bool(event_name, data, "force")?.unwrap_or(false);

    let store = MemoryCleanupPolicyStore::new(memory_cleanup_policy_path(&paths.policy_dir));
    let policy = store.load_or_default()?;
    if !policy.enabled {
        return Ok(());
    }
    if !force
        && let Some(wait_minutes) = memory_policy_remaining_wait_minutes(&policy, Utc::now())
        && wait_minutes > 0
    {
        return Ok(());
    }

    let prune = prune_memory_namespaces(
        &paths.data_dir,
        MemoryPruneOptions {
            max_namespaces: policy.max_namespaces,
            max_age_hours: policy.max_age_hours,
            max_documents_per_namespace: policy.max_documents_per_namespace,
            dry_run,
        },
    )?;
    if !dry_run {
        let _ = store.mark_run(prune.removed_count)?;
    }
    Ok(())
}

fn parse_optional_event_bool(event_name: &str, data: &Value, key: &str) -> Result<Option<bool>> {
    match data {
        Value::Null => Ok(None),
        Value::Object(map) => match map.get(key) {
            None => Ok(None),
            Some(Value::Bool(value)) => Ok(Some(*value)),
            Some(_) => Err(MosaicError::Validation(format!(
                "system event '{event_name}' field '{key}' must be a boolean"
            ))),
        },
        _ => Err(MosaicError::Validation(format!(
            "system event '{event_name}' data must be an object or null"
        ))),
    }
}

fn memory_policy_remaining_wait_minutes(
    policy: &MemoryCleanupPolicy,
    now: DateTime<Utc>,
) -> Option<u64> {
    let interval_minutes = policy.min_interval_minutes?;
    let last_run_at = policy.last_run_at?;
    let interval = i64::try_from(interval_minutes).ok()?;
    let elapsed = now.signed_duration_since(last_run_at).num_minutes();
    if elapsed >= interval {
        return None;
    }
    let remaining = if elapsed <= 0 {
        interval
    } else {
        interval - elapsed
    };
    u64::try_from(remaining).ok()
}

fn run_hooks_for_system_event(
    cli: &Cli,
    paths: &StatePaths,
    event_name: &str,
    data: Value,
) -> Result<Vec<HookExecutionReport>> {
    let hooks_path = hooks_file_path(&paths.data_dir);
    let mut hooks = load_hooks_or_default(&hooks_path)?;
    let mut reports = Vec::new();
    let mut changed = false;

    for index in 0..hooks.len() {
        if !hooks[index].enabled || hooks[index].event != event_name {
            continue;
        }
        let hook = hooks[index].clone();
        let report = execute_hook_command(cli, paths, &hook, "system_event", data.clone())?;
        apply_hook_last_result(&mut hooks, &hook.id, &report);
        reports.push(report);
        changed = true;
    }

    if changed {
        save_hooks(&hooks_path, &hooks)?;
    }
    Ok(reports)
}

pub(super) fn execute_hook_command(
    cli: &Cli,
    paths: &StatePaths,
    hook: &HookRecord,
    trigger: &str,
    payload: Value,
) -> Result<HookExecutionReport> {
    let cwd = std::env::current_dir().map_err(|err| MosaicError::Io(err.to_string()))?;
    let approval_store = ApprovalStore::new(paths.approvals_policy_path.clone());
    let sandbox_store = SandboxStore::new(paths.sandbox_policy_path.clone());
    let runtime_policy = RuntimePolicy {
        approval: approval_store.load_or_default()?,
        sandbox: sandbox_store.load_or_default()?,
    };
    let executor = ToolExecutor::new(RunGuardMode::ConfirmDangerous, Some(runtime_policy));
    let context = ToolContext {
        cwd,
        yes: cli.yes,
        interactive: false,
    };
    let execution = executor.execute(
        "run_cmd",
        json!({
            "command": hook.command.clone(),
        }),
        &context,
    );

    let report = match execution {
        Ok(value) => {
            let output: RunCommandOutput = serde_json::from_value(value)?;
            let ok = output.exit_code == 0;
            HookExecutionReport {
                hook_id: hook.id.clone(),
                hook_name: hook.name.clone(),
                event: hook.event.clone(),
                trigger: trigger.to_string(),
                command: output.command,
                ok,
                exit_code: Some(output.exit_code),
                duration_ms: Some(u64::try_from(output.duration_ms).unwrap_or(u64::MAX)),
                approved_by: Some(output.approved_by),
                error_code: if ok { None } else { Some("tool".to_string()) },
                error: if ok {
                    None
                } else {
                    Some(format!("command exited with status {}", output.exit_code))
                },
                stdout: Some(output.stdout),
                stderr: Some(output.stderr),
            }
        }
        Err(err) => HookExecutionReport {
            hook_id: hook.id.clone(),
            hook_name: hook.name.clone(),
            event: hook.event.clone(),
            trigger: trigger.to_string(),
            command: hook.command.clone(),
            ok: false,
            exit_code: None,
            duration_ms: None,
            approved_by: None,
            error_code: Some(err.code().to_string()),
            error: Some(err.to_string()),
            stdout: None,
            stderr: None,
        },
    };

    append_hook_event(&paths.data_dir, hook, trigger, &report, payload)?;
    Ok(report)
}

fn append_hook_event(
    data_dir: &std::path::Path,
    hook: &HookRecord,
    trigger: &str,
    report: &HookExecutionReport,
    payload: Value,
) -> Result<()> {
    let event = HookEventRecord {
        ts: Utc::now(),
        hook_id: hook.id.clone(),
        hook_name: hook.name.clone(),
        event: hook.event.clone(),
        trigger: trigger.to_string(),
        command: hook.command.clone(),
        delivery_status: if report.ok {
            "success".to_string()
        } else {
            "failed".to_string()
        },
        ok: report.ok,
        exit_code: report.exit_code,
        duration_ms: report.duration_ms,
        approved_by: report.approved_by.clone(),
        error_code: report.error_code.clone(),
        error: report.error.clone(),
        stdout_preview: report
            .stdout
            .as_deref()
            .and_then(|value| preview_text(value, 240)),
        stderr_preview: report
            .stderr
            .as_deref()
            .and_then(|value| preview_text(value, 240)),
        data: payload,
    };
    let path = hook_events_file_path(data_dir, &hook.id);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let encoded = serde_json::to_string(&event).map_err(|err| {
        MosaicError::Validation(format!(
            "failed to encode hook event {}: {err}",
            path.display()
        ))
    })?;
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)?;
    use std::io::Write as _;
    file.write_all(encoded.as_bytes())?;
    file.write_all(b"\n")?;
    Ok(())
}

pub(super) fn apply_hook_last_result(
    hooks: &mut [HookRecord],
    hook_id: &str,
    report: &HookExecutionReport,
) {
    if let Some(hook) = hooks.iter_mut().find(|item| item.id == hook_id) {
        let now = Utc::now();
        hook.updated_at = now;
        hook.last_triggered_at = Some(now);
        hook.last_result = Some(HookLastResult {
            ok: report.ok,
            exit_code: report.exit_code,
            duration_ms: report.duration_ms,
            approved_by: report.approved_by.clone(),
            error_code: report.error_code.clone(),
            error: report.error.clone(),
        });
    }
}

pub(super) fn hook_execution_error(hook_id: &str, report: &HookExecutionReport) -> MosaicError {
    let message = report
        .error
        .clone()
        .unwrap_or_else(|| format!("hook '{}' execution failed", hook_id));
    match report.error_code.as_deref() {
        Some("config") => MosaicError::Config(message),
        Some("auth") => MosaicError::Auth(message),
        Some("network") => MosaicError::Network(message),
        Some("io") => MosaicError::Io(message),
        Some("validation") => MosaicError::Validation(message),
        Some("gateway_unavailable") => MosaicError::GatewayUnavailable(message),
        Some("gateway_protocol") => MosaicError::GatewayProtocol(message),
        Some("channel_unsupported") => MosaicError::ChannelUnsupported(message),
        Some("approval_required") => MosaicError::ApprovalRequired(message),
        Some("sandbox_denied") => MosaicError::SandboxDenied(message),
        _ => {
            if let Some(code) = report.exit_code {
                MosaicError::Tool(format!("hook '{}' exited with status {}", hook_id, code))
            } else {
                MosaicError::Tool(message)
            }
        }
    }
}

pub(super) fn read_hook_events(
    data_dir: &std::path::Path,
    hook_id: Option<&str>,
    tail: usize,
) -> Result<Vec<HookEventRecord>> {
    let mut events = Vec::new();
    if let Some(hook_id) = hook_id {
        let path = hook_events_file_path(data_dir, hook_id);
        load_hook_events_file(&path, &mut events)?;
    } else {
        let dir = hook_events_dir(data_dir);
        if dir.exists() {
            for entry in std::fs::read_dir(&dir)? {
                let path = entry?.path();
                if path.extension().and_then(|value| value.to_str()) != Some("jsonl") {
                    continue;
                }
                load_hook_events_file(&path, &mut events)?;
            }
        }
    }
    events.sort_by(|lhs, rhs| lhs.ts.cmp(&rhs.ts));
    if events.len() > tail {
        let keep_from = events.len() - tail;
        events = events.split_off(keep_from);
    }
    Ok(events)
}

fn load_hook_events_file(path: &std::path::Path, events: &mut Vec<HookEventRecord>) -> Result<()> {
    if !path.exists() {
        return Ok(());
    }
    let raw = std::fs::read_to_string(path)?;
    for line in raw.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let event = serde_json::from_str::<HookEventRecord>(line).map_err(|err| {
            MosaicError::Validation(format!(
                "invalid hook event format {}: {err}",
                path.display()
            ))
        })?;
        events.push(event);
    }
    Ok(())
}

pub(super) fn execute_cron_job(
    cli: &Cli,
    paths: &StatePaths,
    job: &CronJobRecord,
    trigger: &str,
    payload_override: Option<Value>,
) -> Result<CronExecutionReport> {
    let payload = payload_override.unwrap_or_else(|| job.data.clone());
    let dispatch = dispatch_system_event(cli, paths, &job.event, payload.clone());
    let report = match dispatch {
        Ok(dispatch) => {
            let hooks_triggered = dispatch.hook_reports.len();
            let hooks_ok = dispatch.hook_reports.iter().filter(|item| item.ok).count();
            let hooks_failed = hooks_triggered.saturating_sub(hooks_ok);
            let error = if hooks_failed > 0 {
                Some(format!("{hooks_failed} hook execution(s) failed"))
            } else {
                None
            };
            CronExecutionReport {
                job_id: job.id.clone(),
                job_name: job.name.clone(),
                trigger: trigger.to_string(),
                event: job.event.clone(),
                ok: hooks_failed == 0,
                hooks_triggered,
                hooks_ok,
                hooks_failed,
                error,
                system_event: Some(dispatch.event),
            }
        }
        Err(err) => CronExecutionReport {
            job_id: job.id.clone(),
            job_name: job.name.clone(),
            trigger: trigger.to_string(),
            event: job.event.clone(),
            ok: false,
            hooks_triggered: 0,
            hooks_ok: 0,
            hooks_failed: 0,
            error: Some(err.to_string()),
            system_event: None,
        },
    };
    append_cron_event(&paths.data_dir, job, trigger, payload, &report)?;
    Ok(report)
}

fn append_cron_event(
    data_dir: &std::path::Path,
    job: &CronJobRecord,
    trigger: &str,
    payload: Value,
    report: &CronExecutionReport,
) -> Result<()> {
    let event = CronEventRecord {
        ts: Utc::now(),
        job_id: job.id.clone(),
        job_name: job.name.clone(),
        trigger: trigger.to_string(),
        event: job.event.clone(),
        data: payload,
        ok: report.ok,
        hooks_triggered: report.hooks_triggered,
        hooks_ok: report.hooks_ok,
        hooks_failed: report.hooks_failed,
        error: report.error.clone(),
    };
    let path = cron_events_file_path(data_dir, &job.id);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let encoded = serde_json::to_string(&event).map_err(|err| {
        MosaicError::Validation(format!(
            "failed to encode cron event {}: {err}",
            path.display()
        ))
    })?;
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)?;
    use std::io::Write as _;
    file.write_all(encoded.as_bytes())?;
    file.write_all(b"\n")?;
    Ok(())
}

pub(super) fn apply_cron_result(
    job: &mut CronJobRecord,
    report: &CronExecutionReport,
    ran_at: DateTime<Utc>,
) -> Result<()> {
    job.updated_at = ran_at;
    job.last_run_at = Some(ran_at);
    job.run_count = job.run_count.saturating_add(1);
    job.next_run_at = next_cron_run_at(ran_at, job.every_seconds)?;
    job.last_result = Some(CronLastResult {
        ok: report.ok,
        hooks_triggered: report.hooks_triggered,
        hooks_ok: report.hooks_ok,
        hooks_failed: report.hooks_failed,
        error: report.error.clone(),
    });
    Ok(())
}

fn next_cron_run_at(from: DateTime<Utc>, every_seconds: u64) -> Result<DateTime<Utc>> {
    if every_seconds == 0 {
        return Err(MosaicError::Validation(
            "cron interval must be greater than 0 seconds".to_string(),
        ));
    }
    let every_seconds = i64::try_from(every_seconds).map_err(|_| {
        MosaicError::Validation("cron interval is too large to schedule".to_string())
    })?;
    Ok(from + ChronoDuration::seconds(every_seconds))
}

pub(super) fn read_cron_events(
    data_dir: &std::path::Path,
    job_id: Option<&str>,
    tail: usize,
) -> Result<Vec<CronEventRecord>> {
    let mut events = Vec::new();
    if let Some(job_id) = job_id {
        let path = cron_events_file_path(data_dir, job_id);
        load_cron_events_file(&path, &mut events)?;
    } else {
        let dir = cron_events_dir(data_dir);
        if dir.exists() {
            for entry in std::fs::read_dir(&dir)? {
                let path = entry?.path();
                if path.extension().and_then(|value| value.to_str()) != Some("jsonl") {
                    continue;
                }
                load_cron_events_file(&path, &mut events)?;
            }
        }
    }
    events.sort_by(|lhs, rhs| lhs.ts.cmp(&rhs.ts));
    if events.len() > tail {
        let keep_from = events.len() - tail;
        events = events.split_off(keep_from);
    }
    Ok(events)
}

fn load_cron_events_file(path: &std::path::Path, events: &mut Vec<CronEventRecord>) -> Result<()> {
    if !path.exists() {
        return Ok(());
    }
    let raw = std::fs::read_to_string(path)?;
    for line in raw.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let event = serde_json::from_str::<CronEventRecord>(line).map_err(|err| {
            MosaicError::Validation(format!(
                "invalid cron event format {}: {err}",
                path.display()
            ))
        })?;
        events.push(event);
    }
    Ok(())
}

pub(super) fn normalize_webhook_path(path: &str) -> Result<String> {
    let path = path.trim();
    if path.is_empty() {
        return Err(MosaicError::Validation(
            "webhook path cannot be empty".to_string(),
        ));
    }
    if path.contains(' ') || path.contains('\t') || path.contains('\n') || path.contains('\r') {
        return Err(MosaicError::Validation(
            "webhook path cannot contain whitespace".to_string(),
        ));
    }
    if path.starts_with('/') {
        Ok(path.to_string())
    } else {
        Ok(format!("/{path}"))
    }
}

pub(super) fn normalize_optional_secret_env(secret_env: Option<String>) -> Result<Option<String>> {
    match secret_env {
        None => Ok(None),
        Some(value) => {
            let trimmed = value.trim();
            if trimmed.is_empty() {
                return Err(MosaicError::Validation(
                    "webhook secret env cannot be empty".to_string(),
                ));
            }
            if !is_valid_env_name(trimmed) {
                return Err(MosaicError::Validation(format!(
                    "invalid webhook secret env '{}'",
                    trimmed
                )));
            }
            Ok(Some(trimmed.to_string()))
        }
    }
}

fn is_valid_env_name(name: &str) -> bool {
    let mut chars = name.chars();
    match chars.next() {
        Some(first) if first == '_' || first.is_ascii_alphabetic() => {}
        _ => return false,
    }
    chars.all(|item| item == '_' || item.is_ascii_alphanumeric())
}

fn verify_webhook_secret(webhook: &WebhookRecord, provided_secret: Option<&str>) -> Result<()> {
    let Some(secret_env) = &webhook.secret_env else {
        return Ok(());
    };
    let expected = std::env::var(secret_env).map_err(|_| {
        MosaicError::Auth(format!(
            "webhook '{}' requires secret env '{}' to be set",
            webhook.id, secret_env
        ))
    })?;
    if expected.is_empty() {
        return Err(MosaicError::Auth(format!(
            "webhook '{}' secret env '{}' is empty",
            webhook.id, secret_env
        )));
    }
    let Some(provided_secret) = provided_secret else {
        return Err(MosaicError::Auth(format!(
            "webhook '{}' requires --secret to trigger",
            webhook.id
        )));
    };
    if provided_secret != expected {
        return Err(MosaicError::Auth(format!(
            "webhook '{}' secret mismatch",
            webhook.id
        )));
    }
    Ok(())
}

pub(super) fn execute_webhook(
    cli: &Cli,
    paths: &StatePaths,
    webhook: &WebhookRecord,
    trigger: &str,
    payload: Value,
    provided_secret: Option<&str>,
) -> Result<WebhookExecutionReport> {
    let dispatch = match verify_webhook_secret(webhook, provided_secret) {
        Ok(()) => dispatch_system_event(cli, paths, &webhook.event, payload.clone()),
        Err(err) => Err(err),
    };
    let report = match dispatch {
        Ok(dispatch) => {
            let hooks_triggered = dispatch.hook_reports.len();
            let hooks_ok = dispatch.hook_reports.iter().filter(|item| item.ok).count();
            let hooks_failed = hooks_triggered.saturating_sub(hooks_ok);
            let error = if hooks_failed > 0 {
                Some(format!("{hooks_failed} hook execution(s) failed"))
            } else {
                None
            };
            WebhookExecutionReport {
                webhook_id: webhook.id.clone(),
                webhook_name: webhook.name.clone(),
                trigger: trigger.to_string(),
                event: webhook.event.clone(),
                path: webhook.path.clone(),
                method: webhook.method.clone(),
                ok: hooks_failed == 0,
                hooks_triggered,
                hooks_ok,
                hooks_failed,
                error_code: if hooks_failed == 0 {
                    None
                } else {
                    Some("tool".to_string())
                },
                error,
                system_event: Some(dispatch.event),
            }
        }
        Err(err) => WebhookExecutionReport {
            webhook_id: webhook.id.clone(),
            webhook_name: webhook.name.clone(),
            trigger: trigger.to_string(),
            event: webhook.event.clone(),
            path: webhook.path.clone(),
            method: webhook.method.clone(),
            ok: false,
            hooks_triggered: 0,
            hooks_ok: 0,
            hooks_failed: 0,
            error_code: Some(err.code().to_string()),
            error: Some(err.to_string()),
            system_event: None,
        },
    };
    append_webhook_event(&paths.data_dir, webhook, trigger, payload, &report)?;
    Ok(report)
}

fn append_webhook_event(
    data_dir: &std::path::Path,
    webhook: &WebhookRecord,
    trigger: &str,
    payload: Value,
    report: &WebhookExecutionReport,
) -> Result<()> {
    let event = WebhookEventRecord {
        ts: Utc::now(),
        webhook_id: webhook.id.clone(),
        webhook_name: webhook.name.clone(),
        trigger: trigger.to_string(),
        event: webhook.event.clone(),
        path: webhook.path.clone(),
        method: webhook.method.clone(),
        data: payload,
        ok: report.ok,
        hooks_triggered: report.hooks_triggered,
        hooks_ok: report.hooks_ok,
        hooks_failed: report.hooks_failed,
        error_code: report.error_code.clone(),
        error: report.error.clone(),
    };
    let path = webhook_events_file_path(data_dir, &webhook.id);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let encoded = serde_json::to_string(&event).map_err(|err| {
        MosaicError::Validation(format!(
            "failed to encode webhook event {}: {err}",
            path.display()
        ))
    })?;
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)?;
    use std::io::Write as _;
    file.write_all(encoded.as_bytes())?;
    file.write_all(b"\n")?;
    Ok(())
}

pub(super) fn apply_webhook_last_result(
    webhook: &mut WebhookRecord,
    report: &WebhookExecutionReport,
) {
    let now = Utc::now();
    webhook.updated_at = now;
    webhook.last_triggered_at = Some(now);
    webhook.last_result = Some(WebhookLastResult {
        ok: report.ok,
        hooks_triggered: report.hooks_triggered,
        hooks_ok: report.hooks_ok,
        hooks_failed: report.hooks_failed,
        error_code: report.error_code.clone(),
        error: report.error.clone(),
    });
}

pub(super) fn webhook_execution_error(
    webhook_id: &str,
    report: &WebhookExecutionReport,
) -> MosaicError {
    let message = report
        .error
        .clone()
        .unwrap_or_else(|| format!("webhook '{}' execution failed", webhook_id));
    match report.error_code.as_deref() {
        Some("config") => MosaicError::Config(message),
        Some("auth") => MosaicError::Auth(message),
        Some("network") => MosaicError::Network(message),
        Some("io") => MosaicError::Io(message),
        Some("validation") => MosaicError::Validation(message),
        Some("gateway_unavailable") => MosaicError::GatewayUnavailable(message),
        Some("gateway_protocol") => MosaicError::GatewayProtocol(message),
        Some("channel_unsupported") => MosaicError::ChannelUnsupported(message),
        Some("approval_required") => MosaicError::ApprovalRequired(message),
        Some("sandbox_denied") => MosaicError::SandboxDenied(message),
        _ => MosaicError::Tool(message),
    }
}

pub(super) fn read_webhook_events(
    data_dir: &std::path::Path,
    webhook_id: Option<&str>,
    tail: usize,
) -> Result<Vec<WebhookEventRecord>> {
    let mut events = Vec::new();
    if let Some(webhook_id) = webhook_id {
        let path = webhook_events_file_path(data_dir, webhook_id);
        load_webhook_events_file(&path, &mut events)?;
    } else {
        let dir = webhook_events_dir(data_dir);
        if dir.exists() {
            for entry in std::fs::read_dir(&dir)? {
                let path = entry?.path();
                if path.extension().and_then(|value| value.to_str()) != Some("jsonl") {
                    continue;
                }
                load_webhook_events_file(&path, &mut events)?;
            }
        }
    }
    events.sort_by(|lhs, rhs| lhs.ts.cmp(&rhs.ts));
    if events.len() > tail {
        let keep_from = events.len() - tail;
        events = events.split_off(keep_from);
    }
    Ok(events)
}

fn load_webhook_events_file(
    path: &std::path::Path,
    events: &mut Vec<WebhookEventRecord>,
) -> Result<()> {
    if !path.exists() {
        return Ok(());
    }
    let raw = std::fs::read_to_string(path)?;
    for line in raw.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let event = serde_json::from_str::<WebhookEventRecord>(line).map_err(|err| {
            MosaicError::Validation(format!(
                "invalid webhook event format {}: {err}",
                path.display()
            ))
        })?;
        events.push(event);
    }
    Ok(())
}
