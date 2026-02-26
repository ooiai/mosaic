use chrono::Utc;
use serde_json::{Value, json};

use mosaic_channels::{
    AddChannelInput, ChannelRepository, ChannelSendOptions, ChannelTemplateDefaults,
    RotateTokenEnvInput, UpdateChannelInput, channels_events_dir, channels_file_path,
    format_channel_for_output,
};
use mosaic_core::error::{MosaicError, Result};

use super::{
    ChannelsArgs, ChannelsCommand, Cli, parse_json_input, print_json, resolve_state_paths,
    save_json_file,
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
        ChannelsCommand::Logs { channel, tail } => {
            let events = repository.logs(channel.as_deref(), tail)?;
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "events": events,
                    "channel": channel,
                }));
            } else if events.is_empty() {
                println!("No channel events found.");
            } else {
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
