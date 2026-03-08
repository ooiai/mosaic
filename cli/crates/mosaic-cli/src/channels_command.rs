use chrono::{Duration, Utc};
use serde_json::{Value, json};
use std::collections::BTreeMap;

use mosaic_channels::{
    AddChannelInput, ChannelRepository, ChannelSendOptions, ChannelTemplateDefaults,
    RotateTokenEnvInput, UpdateChannelInput, channels_events_dir, channels_file_path,
    format_channel_for_output,
};
use mosaic_core::error::{MosaicError, Result};

use super::{
    ChannelsArgs, ChannelsCommand, Cli, ReplayReasonArg, parse_json_input, print_json,
    resolve_state_paths, save_json_file,
};

pub(super) async fn handle_channels(cli: &Cli, args: ChannelsArgs) -> Result<()> {
    let paths = resolve_state_paths(cli.project_state)?;
    paths.ensure_dirs()?;
    let channels_path = channels_file_path(&paths.data_dir);
    let channel_events_dir = channels_events_dir(&paths.data_dir);
    let repository = ChannelRepository::new(channels_path.clone(), channel_events_dir);
    match args.command {
        ChannelsCommand::List => {
            let channels = repository.list()?;
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "channels": channels,
                    "path": channels_path.display().to_string(),
                }));
            } else if channels.is_empty() {
                println!("No channels configured.");
            } else {
                for channel in channels {
                    println!(
                        "{} name={} kind={} endpoint={} target={} defaults={} last_login={} last_send={} last_error={}",
                        channel.id,
                        channel.name,
                        channel.kind,
                        channel.endpoint_masked.unwrap_or_else(|| "-".to_string()),
                        channel.target_masked.unwrap_or_else(|| "-".to_string()),
                        channel.has_template_defaults,
                        channel
                            .last_login_at
                            .map(|v| v.to_rfc3339())
                            .unwrap_or_else(|| "-".to_string()),
                        channel
                            .last_send_at
                            .map(|v| v.to_rfc3339())
                            .unwrap_or_else(|| "-".to_string()),
                        channel.last_error.unwrap_or_else(|| "-".to_string())
                    );
                }
            }
        }
        ChannelsCommand::Status => {
            let status = repository.status()?;
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "status": status,
                }));
            } else {
                println!("channels total: {}", status.total_channels);
                println!("channels healthy: {}", status.healthy_channels);
                println!("channels with errors: {}", status.channels_with_errors);
                if let Some(last_send_at) = status.last_send_at {
                    println!("last send at: {}", last_send_at.to_rfc3339());
                }
                if !status.kinds.is_empty() {
                    println!("kinds:");
                    for (kind, count) in status.kinds {
                        println!("- {kind}: {count}");
                    }
                }
            }
        }
        ChannelsCommand::Add {
            name,
            kind,
            endpoint,
            chat_id,
            token_env,
            default_parse_mode,
            default_title,
            default_block,
            default_metadata,
        } => {
            let default_metadata = default_metadata
                .map(|value| parse_json_input(&value, "channels add default metadata"))
                .transpose()?;
            let entry = repository.add(AddChannelInput {
                name,
                kind,
                endpoint,
                target: chat_id,
                token_env,
                template_defaults: ChannelTemplateDefaults {
                    parse_mode: default_parse_mode,
                    title: default_title,
                    blocks: default_block,
                    metadata: default_metadata,
                },
            })?;
            let rendered = format_channel_for_output(&entry);
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "channel": rendered,
                    "path": channels_path.display().to_string(),
                }));
            } else {
                println!("Channel added: {}", rendered.id);
            }
        }
        ChannelsCommand::Update {
            channel_id,
            name,
            endpoint,
            chat_id,
            token_env,
            clear_token_env,
            default_parse_mode,
            default_title,
            default_block,
            default_metadata,
            clear_defaults,
        } => {
            let default_metadata = default_metadata
                .map(|value| parse_json_input(&value, "channels update default metadata"))
                .transpose()?;
            let template_defaults = if default_parse_mode.is_some()
                || default_title.is_some()
                || !default_block.is_empty()
                || default_metadata.is_some()
            {
                Some(ChannelTemplateDefaults {
                    parse_mode: default_parse_mode,
                    title: default_title,
                    blocks: default_block,
                    metadata: default_metadata,
                })
            } else {
                None
            };
            let updated = repository.update(
                &channel_id,
                UpdateChannelInput {
                    name,
                    endpoint,
                    target: chat_id,
                    token_env,
                    clear_token_env,
                    template_defaults,
                    clear_template_defaults: clear_defaults,
                },
            )?;
            let rendered = format_channel_for_output(&updated);
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "channel": rendered,
                }));
            } else {
                println!("Channel updated: {}", rendered.id);
            }
        }
        ChannelsCommand::Login {
            channel_id,
            token_env,
        } => {
            let login = repository.login(&channel_id, token_env.as_deref())?;
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "channel_id": channel_id,
                    "token_env": login.token_env,
                    "token_present": login.token_present,
                    "channel": format_channel_for_output(&login.channel),
                }));
            } else {
                println!("Channel login recorded for {channel_id}");
                println!(
                    "token env {} {}",
                    login.token_env,
                    if login.token_present {
                        "found"
                    } else {
                        "not found"
                    }
                );
            }
        }
        ChannelsCommand::Send {
            channel_id,
            text,
            parse_mode,
            title,
            block,
            metadata,
            idempotency_key,
            token_env,
        } => {
            let metadata = metadata
                .map(|value| parse_json_input(&value, "channels send metadata"))
                .transpose()?;
            let result = repository
                .send_with_options(
                    &channel_id,
                    &text,
                    token_env,
                    false,
                    ChannelSendOptions {
                        parse_mode,
                        title,
                        blocks: block,
                        idempotency_key,
                        metadata,
                    },
                )
                .await?;
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "channel_id": result.channel_id,
                    "kind": result.kind,
                    "delivered_via": result.delivered_via,
                    "attempts": result.attempts,
                    "http_status": result.http_status,
                    "endpoint_masked": result.endpoint_masked,
                    "target_masked": result.target_masked,
                    "parse_mode": result.parse_mode,
                    "idempotency_key": result.idempotency_key,
                    "deduplicated": result.deduplicated,
                    "rate_limited_ms": result.rate_limited_ms,
                    "event_path": result.event_path,
                }));
            } else {
                println!("Message sent via {}", result.delivered_via);
                if let Some(endpoint) = result.endpoint_masked {
                    println!("endpoint: {endpoint}");
                }
                if let Some(target) = result.target_masked {
                    println!("target: {target}");
                }
                if let Some(parse_mode) = result.parse_mode {
                    println!("parse_mode: {parse_mode}");
                }
                if let Some(key) = result.idempotency_key {
                    println!("idempotency_key: {key}");
                }
                if result.deduplicated {
                    println!("deduplicated: true");
                }
                if let Some(waited) = result.rate_limited_ms {
                    println!("rate_limited_ms: {waited}");
                }
            }
        }
        ChannelsCommand::Logs {
            channel,
            tail,
            summary,
        } => {
            let events = repository.logs(channel.as_deref(), tail)?;
            if cli.json {
                let payload = if summary {
                    json!({
                        "ok": true,
                        "events": events,
                        "channel": channel,
                        "summary": summarize_channel_events(&events),
                    })
                } else {
                    json!({
                        "ok": true,
                        "events": events,
                        "channel": channel,
                    })
                };
                print_json(&payload);
            } else if events.is_empty() {
                println!("No channel events found.");
            } else {
                if summary {
                    let summary = summarize_channel_events(&events);
                    println!(
                        "events total={} success={} failed={} retryable_failed={} non_retryable_failed={} probes={} deduplicated={}",
                        summary["total_events"].as_u64().unwrap_or(0),
                        summary["success_events"].as_u64().unwrap_or(0),
                        summary["failed_events"].as_u64().unwrap_or(0),
                        summary["retryable_failed_events"].as_u64().unwrap_or(0),
                        summary["non_retryable_failed_events"].as_u64().unwrap_or(0),
                        summary["probe_events"].as_u64().unwrap_or(0),
                        summary["deduplicated_events"].as_u64().unwrap_or(0)
                    );
                    if let Some(reasons) = summary["failure_reasons"].as_object()
                        && !reasons.is_empty()
                    {
                        println!("failure reasons:");
                        for (reason, count) in reasons {
                            println!("- {}: {}", reason, count.as_u64().unwrap_or(0));
                        }
                    }
                    if let Some(channels) = summary["channels"].as_array() {
                        println!("channels:");
                        for item in channels {
                            println!(
                                "- {} total={} success={} failed={} retryable_failed={} last_error={}",
                                item["channel_id"].as_str().unwrap_or("-"),
                                item["total_events"].as_u64().unwrap_or(0),
                                item["success_events"].as_u64().unwrap_or(0),
                                item["failed_events"].as_u64().unwrap_or(0),
                                item["retryable_failed_events"].as_u64().unwrap_or(0),
                                item["last_error"].as_str().unwrap_or("-"),
                            );
                        }
                    }
                    if let Some(replay_candidates) = summary["replay_candidates"].as_array()
                        && !replay_candidates.is_empty()
                    {
                        println!("replay candidates:");
                        for item in replay_candidates {
                            println!(
                                "- {} retryable={} reason={} action={} command={}",
                                item["channel_id"].as_str().unwrap_or("-"),
                                item["retryable"].as_bool().unwrap_or(false),
                                item["last_failed_reason"].as_str().unwrap_or("-"),
                                item["suggested_action"].as_str().unwrap_or("-"),
                                item["suggested_command"].as_str().unwrap_or("-"),
                            );
                        }
                    }
                }
                for event in events {
                    println!(
                        "{} channel={} kind={} status={} attempt={} http={} parse_mode={} idempotency_key={} deduplicated={} rate_limited_ms={} error={} preview={}",
                        event.ts.to_rfc3339(),
                        event.channel_id,
                        event.kind,
                        event.delivery_status,
                        event.attempt,
                        event
                            .http_status
                            .map(|value| value.to_string())
                            .unwrap_or_else(|| "-".to_string()),
                        event.parse_mode.unwrap_or_else(|| "-".to_string()),
                        event.idempotency_key.unwrap_or_else(|| "-".to_string()),
                        event.deduplicated,
                        event
                            .rate_limited_ms
                            .map(|value| value.to_string())
                            .unwrap_or_else(|| "-".to_string()),
                        event.error.unwrap_or_else(|| "-".to_string()),
                        event.text_preview
                    );
                }
            }
        }
        ChannelsCommand::Replay {
            channel_id,
            tail,
            since_minutes,
            limit,
            batch_size,
            min_attempt,
            http_statuses,
            include_non_retryable,
            reasons,
            apply,
            max_apply,
            require_full_payload,
            stop_on_error,
            report_out,
            token_env,
        } => {
            if limit == 0 {
                return Err(MosaicError::Validation(
                    "channels replay --limit must be greater than 0".to_string(),
                ));
            }
            if let Some(batch_size) = batch_size
                && batch_size == 0
            {
                return Err(MosaicError::Validation(
                    "channels replay --batch-size must be greater than 0".to_string(),
                ));
            }
            if let Some(min_attempt) = min_attempt
                && min_attempt == 0
            {
                return Err(MosaicError::Validation(
                    "channels replay --min-attempt must be greater than 0".to_string(),
                ));
            }
            if let Some(invalid_status) = http_statuses
                .iter()
                .find(|status| !(**status >= 100 && **status <= 599))
            {
                return Err(MosaicError::Validation(format!(
                    "channels replay --http-status must be in 100..=599 (got {invalid_status})"
                )));
            }
            if let Some(max_apply) = max_apply
                && max_apply == 0
            {
                return Err(MosaicError::Validation(
                    "channels replay --max-apply must be greater than 0".to_string(),
                ));
            }
            let channel_exists = repository.list()?.iter().any(|item| item.id == channel_id);
            if !channel_exists {
                return Err(MosaicError::Config(format!(
                    "channel '{channel_id}' not found"
                )));
            }
            let events = repository.logs(Some(&channel_id), tail)?;
            let since_cutoff = since_minutes
                .map(|minutes| {
                    i64::try_from(minutes).map_err(|_| {
                        MosaicError::Validation(
                            "channels replay --since-minutes value is too large".to_string(),
                        )
                    })
                })
                .transpose()?
                .map(Duration::minutes)
                .map(|duration| Utc::now() - duration);
            let report_out_path = report_out.as_ref().map(|path| path.display().to_string());
            let total_failed_events = events
                .iter()
                .filter(|event| event.delivery_status == "failed")
                .count();
            let failed_events_in_window = events
                .iter()
                .filter(|event| event.delivery_status == "failed")
                .filter(|event| {
                    since_cutoff
                        .as_ref()
                        .is_none_or(|cutoff| event.ts >= *cutoff)
                })
                .count();
            #[derive(Clone)]
            struct ReplayCandidate {
                ts: chrono::DateTime<chrono::Utc>,
                channel_id: String,
                kind: String,
                attempt: usize,
                http_status: Option<u16>,
                error: Option<String>,
                text_preview: String,
                parse_mode: Option<String>,
                idempotency_key: Option<String>,
                replay_text: String,
                replay_source: String,
                full_payload_available: bool,
                retryable: bool,
                reason: String,
                suggested_action: String,
                suggested_command: String,
            }

            let mut replay_candidates = Vec::<ReplayCandidate>::new();
            let reason_filters = reasons
                .iter()
                .map(|reason| replay_reason_key(*reason))
                .collect::<std::collections::BTreeSet<_>>();
            let http_status_filters = http_statuses
                .iter()
                .copied()
                .collect::<std::collections::BTreeSet<_>>();
            let reason_filters_json = reason_filters.iter().copied().collect::<Vec<_>>();
            let http_status_filters_json = http_status_filters.iter().copied().collect::<Vec<_>>();
            let write_report = |payload: &Value| -> Result<()> {
                if let Some(path) = report_out.as_ref() {
                    save_json_file(path, payload)?;
                }
                Ok(())
            };
            for event in events.iter().rev() {
                if event.delivery_status != "failed" {
                    continue;
                }
                if since_cutoff
                    .as_ref()
                    .is_some_and(|cutoff| event.ts < *cutoff)
                {
                    continue;
                }
                if min_attempt.is_some_and(|minimum| event.attempt < minimum) {
                    continue;
                }
                if !http_status_filters.is_empty()
                    && event
                        .http_status
                        .is_none_or(|status| !http_status_filters.contains(&status))
                {
                    continue;
                }
                let (reason, retryable, suggested_action) = classify_failure(event);
                if !include_non_retryable && !retryable {
                    continue;
                }
                if !reason_filters.is_empty() && !reason_filters.contains(reason.as_str()) {
                    continue;
                }
                let replay_payload = event.replay_payload.as_ref();
                let replay_text = replay_payload
                    .map(|payload| payload.text.clone())
                    .unwrap_or_else(|| event.text_preview.clone());
                let replay_source = if replay_payload.is_some() {
                    "full_payload"
                } else {
                    "text_preview_fallback"
                };
                let replay_parse_mode = replay_payload
                    .and_then(|payload| payload.parse_mode.clone())
                    .or_else(|| event.parse_mode.clone());
                let replay_idempotency_key = replay_payload
                    .and_then(|payload| payload.idempotency_key.clone())
                    .or_else(|| event.idempotency_key.clone());
                replay_candidates.push(ReplayCandidate {
                    ts: event.ts,
                    channel_id: event.channel_id.clone(),
                    kind: event.kind.clone(),
                    attempt: event.attempt,
                    http_status: event.http_status,
                    error: event.error.clone(),
                    text_preview: event.text_preview.clone(),
                    parse_mode: replay_parse_mode.clone(),
                    idempotency_key: replay_idempotency_key.clone(),
                    replay_text,
                    replay_source: replay_source.to_string(),
                    full_payload_available: replay_payload.is_some(),
                    retryable,
                    reason,
                    suggested_action,
                    suggested_command: build_replay_command_template(
                        &event.channel_id,
                        replay_parse_mode.as_deref(),
                        replay_idempotency_key.as_deref(),
                    ),
                });
                if replay_candidates.len() >= limit {
                    break;
                }
            }
            let replay_candidates_json = replay_candidates
                .iter()
                .map(|item| {
                    json!({
                        "ts": item.ts.to_rfc3339(),
                        "channel_id": item.channel_id,
                        "kind": item.kind,
                        "attempt": item.attempt,
                        "http_status": item.http_status,
                        "error": item.error,
                        "text_preview": item.text_preview,
                        "parse_mode": item.parse_mode,
                        "idempotency_key": item.idempotency_key,
                        "replay_source": item.replay_source,
                        "full_payload_available": item.full_payload_available,
                        "retryable": item.retryable,
                        "reason": item.reason,
                        "suggested_action": item.suggested_action,
                        "suggested_command": item.suggested_command,
                    })
                })
                .collect::<Vec<_>>();
            let batch_plan = build_replay_batch_plan(&replay_candidates_json, batch_size);

            if apply && replay_candidates.is_empty() {
                let payload = json!({
                    "ok": true,
                    "channel_id": channel_id,
                    "tail": tail,
                    "since_minutes": since_minutes,
                    "limit": limit,
                    "batch_size": batch_size,
                    "max_apply": max_apply,
                    "include_non_retryable": include_non_retryable,
                    "min_attempt": min_attempt,
                    "http_status_filters": http_status_filters_json,
                    "reason_filters": reason_filters_json,
                    "apply": true,
                    "token_env": token_env,
                    "report_out": report_out_path,
                    "total_failed_events": total_failed_events,
                    "failed_events_in_window": failed_events_in_window,
                    "selected_candidates": 0,
                    "batch_plan": batch_plan.clone(),
                    "applied": {
                        "planned": 0,
                        "attempted": 0,
                        "succeeded": 0,
                        "failed": 0,
                        "fallback_count": 0,
                        "stopped_on_error": false,
                        "skipped_due_to_apply_limit": 0,
                        "skipped_due_to_stop_on_error": 0,
                        "results": [],
                    },
                    "replay_candidates": replay_candidates_json.clone(),
                });
                write_report(&payload)?;
                if cli.json {
                    print_json(&payload);
                } else {
                    println!(
                        "No replay candidates found for {} (failed_events={} tail={}).",
                        channel_id, total_failed_events, tail
                    );
                }
                return Ok(());
            }

            if apply {
                validate_replay_apply_readiness(&repository, &channel_id, token_env.as_deref())?;
                let mut applied_results = Vec::new();
                let mut succeeded = 0usize;
                let mut failed = 0usize;
                let mut stopped_early = false;
                let planned_attempts = max_apply
                    .unwrap_or(replay_candidates.len())
                    .min(replay_candidates.len());
                let skipped_due_to_apply_limit =
                    replay_candidates.len().saturating_sub(planned_attempts);
                let fallback_count = replay_candidates
                    .iter()
                    .filter(|item| !item.full_payload_available)
                    .count();
                if require_full_payload && fallback_count > 0 {
                    return Err(MosaicError::Validation(format!(
                        "channels replay --require-full-payload blocked apply: {fallback_count} candidate(s) only contain text_preview fallback"
                    )));
                }
                for candidate in replay_candidates.iter().rev().take(planned_attempts) {
                    let send_result = repository
                        .send_with_options(
                            &channel_id,
                            &candidate.replay_text,
                            token_env.clone(),
                            false,
                            ChannelSendOptions {
                                parse_mode: candidate.parse_mode.clone(),
                                title: None,
                                blocks: Vec::new(),
                                idempotency_key: candidate.idempotency_key.clone(),
                                metadata: None,
                            },
                        )
                        .await;
                    match send_result {
                        Ok(result) => {
                            succeeded += 1;
                            applied_results.push(json!({
                                "ok": true,
                                "ts": candidate.ts.to_rfc3339(),
                                "reason": candidate.reason,
                                "retryable": candidate.retryable,
                                "replay_source": candidate.replay_source,
                                "channel_id": result.channel_id,
                                "attempts": result.attempts,
                                "http_status": result.http_status,
                                "event_path": result.event_path,
                            }));
                        }
                        Err(err) => {
                            failed += 1;
                            applied_results.push(json!({
                                "ok": false,
                                "ts": candidate.ts.to_rfc3339(),
                                "reason": candidate.reason,
                                "retryable": candidate.retryable,
                                "replay_source": candidate.replay_source,
                                "error": {
                                    "code": err.code(),
                                    "message": err.to_string(),
                                    "exit_code": err.exit_code(),
                                },
                            }));
                            if stop_on_error {
                                stopped_early = true;
                                break;
                            }
                        }
                    }
                }
                let attempted = succeeded + failed;
                let skipped_due_to_stop_on_error = if stopped_early {
                    planned_attempts.saturating_sub(attempted)
                } else {
                    0
                };
                let payload = if fallback_count > 0 {
                    json!({
                        "ok": true,
                    "channel_id": channel_id,
                    "tail": tail,
                    "since_minutes": since_minutes,
                    "limit": limit,
                    "batch_size": batch_size,
                    "max_apply": max_apply,
                    "include_non_retryable": include_non_retryable,
                    "min_attempt": min_attempt,
                    "http_status_filters": http_status_filters_json,
                        "reason_filters": reason_filters_json,
                        "apply": true,
                        "token_env": token_env,
                        "report_out": report_out_path,
                        "total_failed_events": total_failed_events,
                        "failed_events_in_window": failed_events_in_window,
                        "selected_candidates": replay_candidates_json.len(),
                        "batch_plan": batch_plan.clone(),
                        "applied": {
                            "planned": planned_attempts,
                            "attempted": attempted,
                            "succeeded": succeeded,
                            "failed": failed,
                            "fallback_count": fallback_count,
                            "stopped_on_error": stopped_early,
                            "skipped_due_to_apply_limit": skipped_due_to_apply_limit,
                            "skipped_due_to_stop_on_error": skipped_due_to_stop_on_error,
                            "results": applied_results.clone(),
                        },
                        "replay_candidates": replay_candidates_json.clone(),
                        "warning": "apply mode fell back to text_preview for candidates without stored full payload",
                    })
                } else {
                    json!({
                        "ok": true,
                        "channel_id": channel_id,
                        "tail": tail,
                        "since_minutes": since_minutes,
                        "limit": limit,
                        "batch_size": batch_size,
                        "max_apply": max_apply,
                        "include_non_retryable": include_non_retryable,
                        "min_attempt": min_attempt,
                        "http_status_filters": http_status_filters_json,
                        "reason_filters": reason_filters_json,
                        "apply": true,
                        "token_env": token_env,
                        "report_out": report_out_path,
                        "total_failed_events": total_failed_events,
                        "failed_events_in_window": failed_events_in_window,
                        "selected_candidates": replay_candidates_json.len(),
                        "batch_plan": batch_plan.clone(),
                        "applied": {
                            "planned": planned_attempts,
                            "attempted": attempted,
                            "succeeded": succeeded,
                            "failed": failed,
                            "fallback_count": fallback_count,
                            "stopped_on_error": stopped_early,
                            "skipped_due_to_apply_limit": skipped_due_to_apply_limit,
                            "skipped_due_to_stop_on_error": skipped_due_to_stop_on_error,
                            "results": applied_results.clone(),
                        },
                        "replay_candidates": replay_candidates_json.clone(),
                        "warning": Value::Null,
                    })
                };
                write_report(&payload)?;
                if cli.json {
                    print_json(&payload);
                } else {
                    println!(
                        "Replay apply channel={} planned={} attempted={} succeeded={} failed={} fallback_count={} stop_on_error={} stopped_early={} skipped_apply_limit={} skipped_stop_on_error={}",
                        channel_id,
                        planned_attempts,
                        attempted,
                        succeeded,
                        failed,
                        fallback_count,
                        stop_on_error,
                        stopped_early,
                        skipped_due_to_apply_limit,
                        skipped_due_to_stop_on_error
                    );
                    if fallback_count > 0 {
                        println!(
                            "warning: replay apply fell back to text_preview for {fallback_count} candidate(s) without stored full payload."
                        );
                    }
                    for item in applied_results {
                        if item["ok"].as_bool().unwrap_or(false) {
                            println!(
                                "- ok ts={} reason={} replay_source={} http={} event_path={}",
                                item["ts"].as_str().unwrap_or("-"),
                                item["reason"].as_str().unwrap_or("-"),
                                item["replay_source"].as_str().unwrap_or("-"),
                                item["http_status"]
                                    .as_u64()
                                    .map(|value| value.to_string())
                                    .unwrap_or_else(|| "-".to_string()),
                                item["event_path"].as_str().unwrap_or("-")
                            );
                        } else {
                            println!(
                                "- failed ts={} reason={} replay_source={} error={}",
                                item["ts"].as_str().unwrap_or("-"),
                                item["reason"].as_str().unwrap_or("-"),
                                item["replay_source"].as_str().unwrap_or("-"),
                                item["error"]["message"].as_str().unwrap_or("-"),
                            );
                        }
                    }
                }
                return Ok(());
            }
            let payload = json!({
                "ok": true,
                "channel_id": channel_id,
                "tail": tail,
                "since_minutes": since_minutes,
                "limit": limit,
                "batch_size": batch_size,
                "max_apply": max_apply,
                "include_non_retryable": include_non_retryable,
                "min_attempt": min_attempt,
                "http_status_filters": http_status_filters_json,
                "reason_filters": reason_filters_json,
                "apply": false,
                "token_env": token_env,
                "report_out": report_out_path,
                "total_failed_events": total_failed_events,
                "failed_events_in_window": failed_events_in_window,
                "selected_candidates": replay_candidates_json.len(),
                "batch_plan": batch_plan.clone(),
                "replay_candidates": replay_candidates_json.clone(),
            });
            write_report(&payload)?;
            if cli.json {
                print_json(&payload);
            } else if replay_candidates_json.is_empty() {
                println!(
                    "No replay candidates found for {} (failed_events={} tail={}).",
                    channel_id, total_failed_events, tail
                );
            } else {
                println!(
                    "Replay candidates channel={} selected={} failed_events={} tail={} include_non_retryable={} min_attempt={} apply=false",
                    channel_id,
                    replay_candidates_json.len(),
                    total_failed_events,
                    tail,
                    include_non_retryable,
                    min_attempt
                        .map(|value| value.to_string())
                        .unwrap_or_else(|| "-".to_string())
                );
                if !reason_filters.is_empty() {
                    println!(
                        "reason filters: {}",
                        reason_filters.into_iter().collect::<Vec<_>>().join(",")
                    );
                }
                if !http_status_filters.is_empty() {
                    println!(
                        "http status filters: {}",
                        http_status_filters
                            .iter()
                            .map(|status| status.to_string())
                            .collect::<Vec<_>>()
                            .join(",")
                    );
                }
                if !batch_plan.is_empty() {
                    println!(
                        "batch plan: requested_size={} batches={}",
                        batch_size.unwrap_or(replay_candidates_json.len()),
                        batch_plan.len()
                    );
                    for batch in &batch_plan {
                        println!(
                            "- batch#{} size={} from={} to={} reasons={}",
                            batch["batch_index"].as_u64().unwrap_or(0),
                            batch["size"].as_u64().unwrap_or(0),
                            batch["first_ts"].as_str().unwrap_or("-"),
                            batch["last_ts"].as_str().unwrap_or("-"),
                            batch["reasons"]
                                .as_array()
                                .map(|values| {
                                    values
                                        .iter()
                                        .filter_map(|value| value.as_str())
                                        .collect::<Vec<_>>()
                                        .join(",")
                                })
                                .unwrap_or_else(|| "-".to_string()),
                        );
                    }
                }
                for item in replay_candidates_json {
                    println!(
                        "- {} retryable={} reason={} source={} action={} command={} preview={}",
                        item["ts"].as_str().unwrap_or("-"),
                        item["retryable"].as_bool().unwrap_or(false),
                        item["reason"].as_str().unwrap_or("-"),
                        item["replay_source"].as_str().unwrap_or("-"),
                        item["suggested_action"].as_str().unwrap_or("-"),
                        item["suggested_command"].as_str().unwrap_or("-"),
                        item["text_preview"].as_str().unwrap_or("-"),
                    );
                }
            }
        }
        ChannelsCommand::Capabilities { channel, target } => {
            let capabilities = repository.capabilities(channel.as_deref(), target.as_deref())?;
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "capabilities": capabilities,
                }));
            } else if capabilities.is_empty() {
                println!("No channel capabilities resolved.");
            } else {
                for capability in capabilities {
                    println!(
                        "{} aliases={} endpoint={} token_env={} probe={} bearer_token={} parse_mode={} template={} idempotency={} rate_limit_report={}",
                        capability.kind,
                        if capability.aliases.is_empty() {
                            "-".to_string()
                        } else {
                            capability.aliases.join(",")
                        },
                        capability.supports_endpoint,
                        capability.supports_token_env,
                        capability.supports_test_probe,
                        capability.supports_bearer_token,
                        capability.supports_parse_mode,
                        capability.supports_message_template,
                        capability.supports_idempotency_key,
                        capability.supports_rate_limit_report
                    );
                    if let Some(diagnostics) = capability.diagnostics {
                        println!(
                            "  target={} name={} ready_for_send={} endpoint_configured={} target_configured={} token_env={} token_present={}",
                            diagnostics.channel_id,
                            diagnostics.channel_name,
                            diagnostics.ready_for_send,
                            diagnostics.endpoint_configured,
                            diagnostics.target_configured,
                            diagnostics.token_env.unwrap_or_else(|| "-".to_string()),
                            diagnostics
                                .token_present
                                .map(|value| value.to_string())
                                .unwrap_or_else(|| "-".to_string())
                        );
                        if !diagnostics.issues.is_empty() {
                            println!("  issues: {}", diagnostics.issues.join(" | "));
                        }
                    }
                }
            }
        }
        ChannelsCommand::Resolve { channel, query } => {
            let query = query.join(" ");
            let entries = repository.resolve(&channel, &query)?;
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "entries": entries,
                    "channel": channel,
                    "query": query,
                }));
            } else if entries.is_empty() {
                println!("No channels resolved.");
            } else {
                for entry in entries {
                    println!(
                        "{} name={} kind={} endpoint={} target={} last_send={} last_error={}",
                        entry.id,
                        entry.name,
                        entry.kind,
                        entry.endpoint_masked.unwrap_or_else(|| "-".to_string()),
                        entry.target_masked.unwrap_or_else(|| "-".to_string()),
                        entry
                            .last_send_at
                            .map(|value| value.to_rfc3339())
                            .unwrap_or_else(|| "-".to_string()),
                        entry.last_error.unwrap_or_else(|| "-".to_string())
                    );
                }
            }
        }
        ChannelsCommand::Export { out } => {
            let file = repository.export_channels()?;
            let payload = json!({
                "schema": "mosaic.channels.export.v1",
                "exported_at": Utc::now(),
                "channels_file": file,
            });
            if let Some(path) = out {
                if let Some(parent) = path.parent().filter(|value| !value.as_os_str().is_empty()) {
                    std::fs::create_dir_all(parent)?;
                }
                let rendered = serde_json::to_string_pretty(&payload).map_err(|err| {
                    MosaicError::Validation(format!("failed to encode channels export JSON: {err}"))
                })?;
                std::fs::write(&path, rendered)?;
                if cli.json {
                    print_json(&json!({
                        "ok": true,
                        "path": path.display().to_string(),
                        "channels": payload["channels_file"]["channels"].as_array().map_or(0usize, |v| v.len()),
                    }));
                } else {
                    println!(
                        "Exported {} channels to {}",
                        payload["channels_file"]["channels"]
                            .as_array()
                            .map_or(0usize, |items| items.len()),
                        path.display()
                    );
                }
            } else if cli.json {
                print_json(&json!({
                    "ok": true,
                    "export": payload,
                }));
            } else {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&payload).map_err(|err| {
                        MosaicError::Validation(format!(
                            "failed to render channels export JSON: {err}"
                        ))
                    })?
                );
            }
        }
        ChannelsCommand::Import {
            file,
            replace,
            strict,
            dry_run,
            report_out,
        } => {
            let raw = std::fs::read_to_string(&file).map_err(|err| {
                MosaicError::Config(format!(
                    "failed to read channels import file {}: {err}",
                    file.display()
                ))
            })?;
            let value = serde_json::from_str::<Value>(&raw).map_err(|err| {
                MosaicError::Validation(format!(
                    "invalid channels import JSON {}: {err}",
                    file.display()
                ))
            })?;
            let import_value = value
                .as_object()
                .and_then(|obj| obj.get("channels_file"))
                .cloned()
                .unwrap_or(value);
            let import_result =
                repository.import_channels_json(import_value, replace, strict, dry_run);
            let report_path = if let Some(path) = report_out.as_ref() {
                let report = match &import_result {
                    Ok(summary) => json!({
                        "schema": "mosaic.channels.import-report.v1",
                        "generated_at": Utc::now(),
                        "request": {
                            "file": file.display().to_string(),
                            "replace": replace,
                            "strict": strict,
                            "dry_run": dry_run,
                        },
                        "result": {
                            "ok": true,
                            "summary": summary,
                        }
                    }),
                    Err(err) => json!({
                        "schema": "mosaic.channels.import-report.v1",
                        "generated_at": Utc::now(),
                        "request": {
                            "file": file.display().to_string(),
                            "replace": replace,
                            "strict": strict,
                            "dry_run": dry_run,
                        },
                        "result": {
                            "ok": false,
                            "error": {
                                "code": err.code(),
                                "message": err.to_string(),
                                "exit_code": err.exit_code(),
                            },
                        }
                    }),
                };
                save_json_file(path, &report)?;
                Some(path.display().to_string())
            } else {
                None
            };
            let summary = import_result?;
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "file": file.display().to_string(),
                    "summary": summary,
                    "report_path": report_path,
                }));
            } else {
                println!(
                    "Import {}from {}: total={} imported={} updated={} skipped={} replace={} strict={}",
                    if summary.dry_run { "(dry-run) " } else { "" },
                    file.display(),
                    summary.total,
                    summary.imported,
                    summary.updated,
                    summary.skipped,
                    summary.replace,
                    summary.strict
                );
                if let Some(path) = report_path {
                    println!("report: {path}");
                }
            }
        }
        ChannelsCommand::RotateTokenEnv {
            channel,
            all,
            kind,
            from_token_env,
            to,
            dry_run,
            report_out,
        } => {
            let summary = repository.rotate_token_env(RotateTokenEnvInput {
                channel_id: channel,
                all,
                kind,
                from_token_env,
                to_token_env: to,
                dry_run,
            })?;
            let report_path = if let Some(path) = report_out {
                if let Some(parent) = path.parent().filter(|value| !value.as_os_str().is_empty()) {
                    std::fs::create_dir_all(parent)?;
                }
                let report = json!({
                    "schema": "mosaic.channels.token-rotation-report.v1",
                    "generated_at": Utc::now(),
                    "summary": summary.clone(),
                });
                let rendered = serde_json::to_string_pretty(&report).map_err(|err| {
                    MosaicError::Validation(format!(
                        "failed to encode token rotation report JSON: {err}"
                    ))
                })?;
                std::fs::write(&path, rendered)?;
                Some(path.display().to_string())
            } else {
                None
            };
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "summary": summary,
                    "report_path": report_path,
                }));
            } else {
                println!(
                    "Token env rotation {}complete: total={} updated={} skipped_already_set={} skipped_unsupported={} skipped_from_mismatch={} source={} target={}",
                    if summary.dry_run { "(dry-run) " } else { "" },
                    summary.total,
                    summary.updated,
                    summary.skipped_already_set,
                    summary.skipped_unsupported,
                    summary.skipped_from_mismatch,
                    summary.from_token_env.as_deref().unwrap_or("*"),
                    summary.to_token_env
                );
                if let Some(path) = report_path {
                    println!("report: {path}");
                }
            }
        }
        ChannelsCommand::Remove { channel_id } => {
            let removed = repository.remove(&channel_id)?;
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "removed": format_channel_for_output(&removed),
                }));
            } else {
                println!("Removed channel {}", removed.id);
            }
        }
        ChannelsCommand::Logout { channel_id } => {
            let logged_out = repository.logout(&channel_id)?;
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "channel": format_channel_for_output(&logged_out),
                }));
            } else {
                println!("Cleared token env for channel {}", logged_out.id);
            }
        }
        ChannelsCommand::Test {
            channel_id,
            token_env,
        } => {
            let probe_text = "mosaic channel connectivity probe";
            let result = repository
                .send(&channel_id, probe_text, token_env, true)
                .await?;
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "channel_id": result.channel_id,
                    "kind": result.kind,
                    "probe": result.probe,
                    "attempts": result.attempts,
                    "http_status": result.http_status,
                    "endpoint_masked": result.endpoint_masked,
                    "target_masked": result.target_masked,
                    "parse_mode": result.parse_mode,
                    "idempotency_key": result.idempotency_key,
                    "deduplicated": result.deduplicated,
                    "rate_limited_ms": result.rate_limited_ms,
                    "event_path": result.event_path,
                }));
            } else {
                println!("Channel test passed for {}", result.channel_id);
                println!("attempts: {}", result.attempts);
                if let Some(endpoint) = result.endpoint_masked {
                    println!("endpoint: {endpoint}");
                }
                if let Some(target) = result.target_masked {
                    println!("target: {target}");
                }
                if let Some(waited) = result.rate_limited_ms {
                    println!("rate_limited_ms: {waited}");
                }
            }
        }
    }
    Ok(())
}

fn summarize_channel_events(events: &[mosaic_channels::ChannelLogEntry]) -> Value {
    #[derive(Default)]
    struct PerChannel {
        total: u64,
        success: u64,
        failed: u64,
        retryable_failed: u64,
        last_error: Option<String>,
        last_ts: Option<chrono::DateTime<chrono::Utc>>,
        latest_failed: Option<FailedEvent>,
    }

    #[derive(Clone)]
    struct FailedEvent {
        ts: chrono::DateTime<chrono::Utc>,
        http_status: Option<u16>,
        error: Option<String>,
        preview: String,
        replay_source: String,
        full_payload_available: bool,
        parse_mode: Option<String>,
        idempotency_key: Option<String>,
        reason: String,
        retryable: bool,
        suggested_action: String,
    }

    let mut total = 0u64;
    let mut success = 0u64;
    let mut failed = 0u64;
    let mut retryable_failed = 0u64;
    let mut probes = 0u64;
    let mut deduplicated = 0u64;
    let mut failure_reasons: BTreeMap<String, u64> = BTreeMap::new();
    let mut by_channel: BTreeMap<String, PerChannel> = BTreeMap::new();

    for event in events {
        total += 1;
        if event.kind == "test_probe" {
            probes += 1;
        }
        if event.deduplicated {
            deduplicated += 1;
        }
        let is_success =
            event.delivery_status == "success" || event.delivery_status == "deduplicated";
        if is_success {
            success += 1;
        }
        if event.delivery_status == "failed" {
            failed += 1;
        }

        let channel = by_channel.entry(event.channel_id.clone()).or_default();
        channel.total += 1;
        if is_success {
            channel.success += 1;
        }
        if event.delivery_status == "failed" {
            let (reason, is_retryable, suggested_action) = classify_failure(event);
            let replay_payload = event.replay_payload.as_ref();
            let replay_source = if replay_payload.is_some() {
                "full_payload"
            } else {
                "text_preview_fallback"
            };
            let replay_parse_mode = replay_payload
                .and_then(|payload| payload.parse_mode.clone())
                .or_else(|| event.parse_mode.clone());
            let replay_idempotency_key = replay_payload
                .and_then(|payload| payload.idempotency_key.clone())
                .or_else(|| event.idempotency_key.clone());
            channel.failed += 1;
            if is_retryable {
                channel.retryable_failed += 1;
                retryable_failed += 1;
            }
            *failure_reasons.entry(reason.clone()).or_default() += 1;
            channel.last_error = event.error.clone();
            let failed_event = FailedEvent {
                ts: event.ts,
                http_status: event.http_status,
                error: event.error.clone(),
                preview: event.text_preview.clone(),
                replay_source: replay_source.to_string(),
                full_payload_available: replay_payload.is_some(),
                parse_mode: replay_parse_mode,
                idempotency_key: replay_idempotency_key,
                reason,
                retryable: is_retryable,
                suggested_action,
            };
            let should_replace_latest = channel
                .latest_failed
                .as_ref()
                .is_none_or(|existing| failed_event.ts >= existing.ts);
            if should_replace_latest {
                channel.latest_failed = Some(failed_event);
            }
        }
        channel.last_ts = Some(event.ts);
    }

    let mut replay_candidates = Vec::new();
    let channels = by_channel
        .into_iter()
        .map(|(channel_id, mut item)| {
            let latest_failed = item.latest_failed.take();
            if let Some(failed_event) = latest_failed.as_ref() {
                replay_candidates.push(json!({
                    "channel_id": channel_id,
                    "last_failed_at": failed_event.ts.to_rfc3339(),
                    "last_failed_http_status": failed_event.http_status,
                    "last_failed_error": failed_event.error,
                    "last_failed_reason": failed_event.reason,
                    "retryable": failed_event.retryable,
                    "replay_source": failed_event.replay_source,
                    "full_payload_available": failed_event.full_payload_available,
                    "suggested_action": failed_event.suggested_action,
                    "suggested_command": build_replay_command_template(
                        &channel_id,
                        failed_event.parse_mode.as_deref(),
                        failed_event.idempotency_key.as_deref(),
                    ),
                    "last_failed_preview": failed_event.preview,
                }));
            }
            json!({
                "channel_id": channel_id,
                "total_events": item.total,
                "success_events": item.success,
                "failed_events": item.failed,
                "retryable_failed_events": item.retryable_failed,
                "last_event_at": item.last_ts.map(|value| value.to_rfc3339()),
                "last_error": item.last_error,
            })
        })
        .collect::<Vec<_>>();
    replay_candidates.sort_by(|lhs, rhs| {
        let lhs_key = lhs
            .get("last_failed_at")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let rhs_key = rhs
            .get("last_failed_at")
            .and_then(Value::as_str)
            .unwrap_or_default();
        rhs_key.cmp(lhs_key)
    });
    let non_retryable_failed = failed.saturating_sub(retryable_failed);

    json!({
        "total_events": total,
        "success_events": success,
        "failed_events": failed,
        "retryable_failed_events": retryable_failed,
        "non_retryable_failed_events": non_retryable_failed,
        "probe_events": probes,
        "deduplicated_events": deduplicated,
        "failure_reasons": failure_reasons,
        "replay_candidates": replay_candidates,
        "channels": channels,
    })
}

fn classify_failure(event: &mosaic_channels::ChannelLogEntry) -> (String, bool, String) {
    let error = event.error.as_deref().unwrap_or_default().to_lowercase();
    if event.http_status == Some(429) || error.contains("rate limited") {
        return (
            "rate_limited".to_string(),
            true,
            "wait and retry; tune sender rate or batching".to_string(),
        );
    }
    if matches!(event.http_status, Some(500..=599)) || error.contains("server error status") {
        return (
            "upstream_5xx".to_string(),
            true,
            "retry send; upstream provider failure is usually transient".to_string(),
        );
    }
    if error.contains("timed out") || error.contains("timeout") {
        return (
            "timeout".to_string(),
            true,
            "retry send and verify network stability/egress".to_string(),
        );
    }
    if matches!(event.http_status, Some(401 | 403))
        || error.contains("token")
        || error.contains("auth")
        || error.contains("credential")
    {
        return (
            "auth".to_string(),
            false,
            "fix token env/credentials before retry".to_string(),
        );
    }
    if event.http_status == Some(404) || error.contains("not found") {
        return (
            "target_not_found".to_string(),
            false,
            "verify endpoint/chat-id configuration before retry".to_string(),
        );
    }
    if matches!(event.http_status, Some(400..=499)) {
        return (
            "client_4xx".to_string(),
            false,
            "inspect payload/channel config and correct before retry".to_string(),
        );
    }
    (
        "unknown".to_string(),
        false,
        "inspect channel logs and provider details before retry".to_string(),
    )
}

fn build_replay_batch_plan(replay_candidates: &[Value], batch_size: Option<usize>) -> Vec<Value> {
    if replay_candidates.is_empty() {
        return Vec::new();
    }
    let effective_batch_size = batch_size.unwrap_or(replay_candidates.len()).max(1);
    replay_candidates
        .chunks(effective_batch_size)
        .enumerate()
        .map(|(index, chunk)| {
            let mut reasons = std::collections::BTreeSet::new();
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
                "reasons": reasons.into_iter().collect::<Vec<_>>(),
                "items": chunk
                    .iter()
                    .map(|item| {
                        json!({
                            "ts": item["ts"],
                            "reason": item["reason"],
                            "retryable": item["retryable"],
                            "replay_source": item["replay_source"],
                            "attempt": item["attempt"],
                            "http_status": item["http_status"],
                        })
                    })
                    .collect::<Vec<_>>(),
            })
        })
        .collect()
}

fn validate_replay_apply_readiness(
    repository: &ChannelRepository,
    channel_id: &str,
    token_env_override: Option<&str>,
) -> Result<()> {
    let token_env_override = token_env_override.map(str::trim);
    if token_env_override.is_some_and(str::is_empty) {
        return Err(MosaicError::Validation(
            "channels replay --token-env cannot be empty".to_string(),
        ));
    }
    if let Some(env) = token_env_override
        && std::env::var(env).is_err()
    {
        return Err(MosaicError::Auth(format!(
            "channels replay --apply requires environment variable {env}"
        )));
    }

    let capabilities = repository.capabilities(None, Some(channel_id))?;
    let Some(diagnostics) = capabilities.into_iter().find_map(|item| item.diagnostics) else {
        return Ok(());
    };

    let mut blocking_issues = diagnostics.issues.clone();
    if token_env_override.is_some() {
        blocking_issues.retain(|issue| !is_token_env_issue(issue));
    }
    if blocking_issues.is_empty() {
        return Ok(());
    }

    let issue_summary = blocking_issues.join("; ");
    let diagnostics_hint =
        format!("run `mosaic channels capabilities --target {channel_id}` to inspect diagnostics");
    if blocking_issues
        .iter()
        .all(|issue| is_token_env_issue(issue))
    {
        return Err(MosaicError::Auth(format!(
            "channels replay --apply blocked: {issue_summary}; pass --token-env <ENV> with a set env var or {diagnostics_hint}"
        )));
    }
    Err(MosaicError::Validation(format!(
        "channels replay --apply blocked: {issue_summary}; {diagnostics_hint}"
    )))
}

fn is_token_env_issue(issue: &str) -> bool {
    issue.contains("token env")
}

fn replay_reason_key(reason: ReplayReasonArg) -> &'static str {
    match reason {
        ReplayReasonArg::RateLimited => "rate_limited",
        ReplayReasonArg::Upstream5xx => "upstream_5xx",
        ReplayReasonArg::Timeout => "timeout",
        ReplayReasonArg::Auth => "auth",
        ReplayReasonArg::TargetNotFound => "target_not_found",
        ReplayReasonArg::Client4xx => "client_4xx",
        ReplayReasonArg::Unknown => "unknown",
    }
}

fn build_replay_command_template(
    channel_id: &str,
    parse_mode: Option<&str>,
    idempotency_key: Option<&str>,
) -> String {
    let mut command =
        format!("mosaic --project-state channels send {channel_id} --text \"<message>\"");
    if let Some(mode) = parse_mode.filter(|value| !value.trim().is_empty()) {
        command.push_str(&format!(" --parse-mode {}", mode.trim()));
    }
    if let Some(key) = idempotency_key.filter(|value| !value.trim().is_empty()) {
        command.push_str(&format!(" --idempotency-key {}", key.trim()));
    }
    command
}
