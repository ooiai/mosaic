use chrono::Utc;
use serde_json::{Value, json};

use mosaic_core::error::MosaicError;

use super::{
    Cli, CronArgs, CronCommand, CronJobRecord, HookRecord, HooksArgs, HooksCommand, Result,
    WebhookMethod, WebhookRecord, WebhooksArgs, WebhooksCommand, apply_cron_result,
    apply_hook_last_result, apply_webhook_last_result, cron_jobs_file_path, execute_cron_job,
    execute_hook_command, execute_webhook, generate_cron_job_id, generate_hook_id,
    generate_webhook_id, hook_execution_error, hooks_file_path, load_cron_jobs_or_default,
    load_hooks_or_default, load_webhooks_or_default, normalize_optional_secret_env,
    normalize_webhook_path, parse_json_input, print_json, read_cron_events, read_hook_events,
    read_webhook_events, resolve_state_paths, save_cron_jobs, save_hooks, save_webhooks,
    webhook_execution_error, webhooks_file_path,
};

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
        HooksCommand::Logs { hook, tail } => {
            let events = read_hook_events(&paths.data_dir, hook.as_deref(), tail)?;
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "events": events,
                }));
            } else if events.is_empty() {
                println!("No hook events found.");
            } else {
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
        CronCommand::Logs { job, tail } => {
            let events = read_cron_events(&paths.data_dir, job.as_deref(), tail)?;
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "events": events,
                }));
            } else if events.is_empty() {
                println!("No cron events found.");
            } else {
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
        WebhooksCommand::Logs { webhook, tail } => {
            let events = read_webhook_events(&paths.data_dir, webhook.as_deref(), tail)?;
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "events": events,
                }));
            } else if events.is_empty() {
                println!("No webhook events found.");
            } else {
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
    }
    Ok(())
}
