use std::fs::{self};
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use mosaic_channels::{
    ChannelRepository, ChannelSendOptions, channels_events_dir, channels_file_path,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

use mosaic_core::error::{MosaicError, Result};
use mosaic_core::privacy::{append_sanitized_jsonl, write_pretty_state_json_file};

use super::{
    Cli, TtsArgs, TtsCommand, VoicecallArgs, VoicecallCommand, print_json, resolve_state_paths,
};

const TTS_VOICES: &[&str] = &["alloy", "echo", "fable", "onyx", "nova", "shimmer"];

#[derive(Debug, Clone, Serialize, Deserialize)]
struct VoicecallState {
    active: bool,
    call_id: Option<String>,
    target: Option<String>,
    channel_id: Option<String>,
    started_at: Option<DateTime<Utc>>,
    ended_at: Option<DateTime<Utc>>,
    messages_sent: u64,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct VoicecallEvent {
    ts: DateTime<Utc>,
    call_id: Option<String>,
    direction: String,
    payload: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    channel_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    delivered_via: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    delivery_status: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    attempts: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    http_status: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    error: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    endpoint_masked: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    target_masked: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    parse_mode: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TtsEvent {
    ts: DateTime<Utc>,
    voice: String,
    format: String,
    text_preview: String,
    output: Option<String>,
    bytes_written: Option<u64>,
}

pub(super) fn handle_tts(cli: &Cli, args: TtsArgs) -> Result<()> {
    let paths = resolve_state_paths(cli.project_state)?;
    paths.ensure_dirs()?;

    match args.command {
        TtsCommand::Voices => {
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "voices": TTS_VOICES,
                }));
            } else {
                println!("tts voices:");
                for voice in TTS_VOICES {
                    println!("- {voice}");
                }
            }
        }
        TtsCommand::Speak {
            text,
            voice,
            format,
            out,
        } => {
            let text = text.trim();
            if text.is_empty() {
                return Err(MosaicError::Validation(
                    "tts text cannot be empty".to_string(),
                ));
            }
            validate_tts_voice(&voice)?;
            let format = normalize_tts_format(&format)?;

            let payload = synthesize_tts_payload(text, &voice, &format);
            let (output_path, bytes_written) = write_tts_output(out.as_deref(), &payload)?;

            let event = TtsEvent {
                ts: Utc::now(),
                voice: voice.clone(),
                format: format.clone(),
                text_preview: preview_text(text),
                output: output_path.clone(),
                bytes_written,
            };
            append_jsonl(&tts_events_path(&paths.data_dir), &event)?;

            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "voice": voice,
                    "format": format,
                    "text_preview": preview_text(text),
                    "output": output_path,
                    "bytes_written": bytes_written,
                }));
            } else {
                println!("tts synthesized (voice={}, format={})", voice, format);
                if let Some(path) = output_path {
                    println!("saved: {path}");
                }
            }
        }
        TtsCommand::Diagnose {
            voice,
            format,
            text,
            out,
            timeout_ms,
            report_out,
        } => {
            if timeout_ms == 0 {
                return Err(MosaicError::Validation(
                    "tts diagnose --timeout-ms must be greater than 0".to_string(),
                ));
            }
            let text = text.trim();
            if text.is_empty() {
                return Err(MosaicError::Validation(
                    "tts diagnose text cannot be empty".to_string(),
                ));
            }
            validate_tts_voice(&voice)?;
            let format = normalize_tts_format(&format)?;

            let started_at = Utc::now();
            let payload = synthesize_tts_payload(text, &voice, &format);
            let payload_bytes = payload.len() as u64;
            let (output_path, bytes_written) = write_tts_output(out.as_deref(), &payload)?;
            let finished_at = Utc::now();
            let duration_ms = (finished_at - started_at).num_milliseconds().max(0) as u64;

            let event = TtsEvent {
                ts: finished_at,
                voice: voice.clone(),
                format: format.clone(),
                text_preview: preview_text(text),
                output: output_path.clone(),
                bytes_written: Some(payload_bytes),
            };
            let events_path = tts_events_path(&paths.data_dir);
            append_jsonl(&events_path, &event)?;

            let output_write_path = output_path.clone();
            let checks = vec![
                json!({"name": "voice_supported", "ok": true, "voice": voice.clone()}),
                json!({"name": "format_supported", "ok": true, "format": format.clone()}),
                json!({"name": "synthesis_probe", "ok": true, "payload_bytes": payload_bytes}),
                json!({"name": "events_log_writable", "ok": true, "path": events_path.display().to_string()}),
                json!({"name": "output_write", "ok": output_write_path.is_some(), "path": output_write_path, "bytes_written": bytes_written, "skipped": out.is_none()}),
            ];

            let mut report_path = None;
            if let Some(path) = report_out {
                let report = json!({
                    "generated_at": finished_at.to_rfc3339(),
                    "command": "tts diagnose",
                    "module": "tts",
                    "result": {
                        "ok": true,
                        "voice": voice,
                        "format": format,
                        "timeout_ms": timeout_ms,
                        "duration_ms": duration_ms,
                        "text_preview": preview_text(text),
                        "payload_bytes": payload_bytes,
                        "output": output_path,
                        "bytes_written": bytes_written,
                        "checks": checks,
                    }
                });
                write_json_report(&path, &report)?;
                report_path = Some(path.display().to_string());
            }

            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "voice": voice,
                    "format": format,
                    "timeout_ms": timeout_ms,
                    "duration_ms": duration_ms,
                    "text_preview": preview_text(text),
                    "payload_bytes": payload_bytes,
                    "output": output_path,
                    "bytes_written": bytes_written,
                    "checks": checks,
                    "report_path": report_path,
                }));
            } else {
                println!("tts diagnose ok");
                println!("voice: {voice}");
                println!("format: {format}");
                println!("payload_bytes: {payload_bytes}");
                println!("duration_ms: {duration_ms}");
                if let Some(path) = output_path {
                    println!("probe_output: {path}");
                }
                if let Some(path) = report_path {
                    println!("report: {path}");
                }
            }
        }
    }
    Ok(())
}

pub(super) async fn handle_voicecall(cli: &Cli, args: VoicecallArgs) -> Result<()> {
    let paths = resolve_state_paths(cli.project_state)?;
    paths.ensure_dirs()?;
    let state_path = voicecall_state_path(&paths.data_dir);
    let events_path = voicecall_events_path(&paths.data_dir);

    match args.command {
        VoicecallCommand::Start { target, channel_id } => {
            let mut state = load_voicecall_state(&state_path)?;
            if state.active {
                if cli.json {
                    print_json(&json!({
                        "ok": true,
                        "already_active": true,
                        "state": state,
                    }));
                } else {
                    println!(
                        "voicecall already active (call_id={})",
                        state.call_id.clone().unwrap_or_else(|| "-".to_string())
                    );
                }
                return Ok(());
            }

            let now = Utc::now();
            let call_id = format!("vc-{}", Uuid::new_v4().simple());
            state.active = true;
            state.call_id = Some(call_id.clone());
            state.target = normalize_optional(&target);
            state.channel_id = normalize_optional(&channel_id);
            state.started_at = Some(now);
            state.ended_at = None;
            state.updated_at = now;
            state.messages_sent = 0;
            save_voicecall_state(&state_path, &state)?;

            append_jsonl(
                &events_path,
                &VoicecallEvent {
                    ts: now,
                    call_id: Some(call_id),
                    direction: "system".to_string(),
                    payload: "voicecall started".to_string(),
                    channel_id: None,
                    delivered_via: None,
                    delivery_status: None,
                    attempts: None,
                    http_status: None,
                    error: None,
                    endpoint_masked: None,
                    target_masked: None,
                    parse_mode: None,
                },
            )?;

            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "already_active": false,
                    "state": state,
                }));
            } else {
                println!("voicecall started");
            }
        }
        VoicecallCommand::Status => {
            let state = load_voicecall_state(&state_path)?;
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "state": state,
                }));
            } else {
                println!("active: {}", state.active);
                println!("call_id: {}", state.call_id.as_deref().unwrap_or("-"));
                println!("target: {}", state.target.as_deref().unwrap_or("-"));
                println!("channel_id: {}", state.channel_id.as_deref().unwrap_or("-"));
                println!("messages_sent: {}", state.messages_sent);
                if let Some(started_at) = state.started_at {
                    println!("started_at: {}", started_at.to_rfc3339());
                }
                if let Some(ended_at) = state.ended_at {
                    println!("ended_at: {}", ended_at.to_rfc3339());
                }
                println!("updated_at: {}", state.updated_at.to_rfc3339());
            }
        }
        VoicecallCommand::Send {
            text,
            parse_mode,
            token_env,
        } => {
            let text = text.trim();
            if text.is_empty() {
                return Err(MosaicError::Validation(
                    "voicecall text cannot be empty".to_string(),
                ));
            }

            let mut state = load_voicecall_state(&state_path)?;
            if !state.active {
                return Err(MosaicError::Validation(
                    "voicecall is not active. run `mosaic voicecall start` first".to_string(),
                ));
            }

            if state.channel_id.is_none() && (parse_mode.is_some() || token_env.is_some()) {
                return Err(MosaicError::Validation(
                    "voicecall send --parse-mode/--token-env requires an active call bound to --channel-id"
                        .to_string(),
                ));
            }

            let channel_delivery = if let Some(channel_id) = state.channel_id.clone() {
                let repository = channels_repository(&paths.data_dir);
                match repository
                    .send_with_options(
                        &channel_id,
                        text,
                        token_env,
                        false,
                        ChannelSendOptions {
                            parse_mode,
                            ..ChannelSendOptions::default()
                        },
                    )
                    .await
                {
                    Ok(result) => Some(result),
                    Err(err) => {
                        append_jsonl(
                            &events_path,
                            &VoicecallEvent {
                                ts: Utc::now(),
                                call_id: state.call_id.clone(),
                                direction: "error".to_string(),
                                payload: "voicecall channel delivery failed".to_string(),
                                channel_id: Some(channel_id),
                                delivered_via: None,
                                delivery_status: Some("failed".to_string()),
                                attempts: None,
                                http_status: None,
                                error: Some(err.to_string()),
                                endpoint_masked: None,
                                target_masked: None,
                                parse_mode: None,
                            },
                        )?;
                        return Err(err);
                    }
                }
            } else {
                None
            };

            let now = Utc::now();
            state.messages_sent = state.messages_sent.saturating_add(1);
            state.updated_at = now;
            save_voicecall_state(&state_path, &state)?;

            let (direction, delivery_status) = if channel_delivery.is_some() {
                ("channel_outbound".to_string(), Some("success".to_string()))
            } else {
                ("outbound".to_string(), None)
            };

            append_jsonl(
                &events_path,
                &VoicecallEvent {
                    ts: now,
                    call_id: state.call_id.clone(),
                    direction,
                    payload: preview_text(text),
                    channel_id: channel_delivery
                        .as_ref()
                        .map(|value| value.channel_id.clone()),
                    delivered_via: channel_delivery
                        .as_ref()
                        .map(|value| value.delivered_via.clone()),
                    delivery_status,
                    attempts: channel_delivery.as_ref().map(|value| value.attempts),
                    http_status: channel_delivery
                        .as_ref()
                        .and_then(|value| value.http_status),
                    error: None,
                    endpoint_masked: channel_delivery
                        .as_ref()
                        .and_then(|value| value.endpoint_masked.clone()),
                    target_masked: channel_delivery
                        .as_ref()
                        .and_then(|value| value.target_masked.clone()),
                    parse_mode: channel_delivery
                        .as_ref()
                        .and_then(|value| value.parse_mode.clone()),
                },
            )?;

            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "call_id": state.call_id,
                    "messages_sent": state.messages_sent,
                    "text_preview": preview_text(text),
                    "channel_delivery": channel_delivery,
                }));
            } else {
                println!("voicecall message sent");
                if let Some(delivery) = channel_delivery {
                    println!("channel_id: {}", delivery.channel_id);
                    println!("delivered_via: {}", delivery.delivered_via);
                    println!("attempts: {}", delivery.attempts);
                    if let Some(status) = delivery.http_status {
                        println!("http_status: {status}");
                    }
                    if let Some(target) = delivery.target_masked {
                        println!("target: {target}");
                    }
                }
            }
        }
        VoicecallCommand::History { tail } => {
            let events = load_voicecall_events(&events_path, tail)?;
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "events": events,
                }));
            } else if events.is_empty() {
                println!("No voicecall events.");
            } else {
                for event in events {
                    let mut details = Vec::new();
                    if let Some(channel_id) = &event.channel_id {
                        details.push(format!("channel={channel_id}"));
                    }
                    if let Some(delivered_via) = &event.delivered_via {
                        details.push(format!("via={delivered_via}"));
                    }
                    if let Some(delivery_status) = &event.delivery_status {
                        details.push(format!("status={delivery_status}"));
                    }
                    if let Some(attempts) = event.attempts {
                        details.push(format!("attempts={attempts}"));
                    }
                    if let Some(http_status) = event.http_status {
                        details.push(format!("http={http_status}"));
                    }
                    if let Some(error) = &event.error {
                        details.push(format!("error={error}"));
                    }
                    println!(
                        "{} [{}] {}{}",
                        event.ts.to_rfc3339(),
                        event.direction,
                        event.payload,
                        if details.is_empty() {
                            String::new()
                        } else {
                            format!(" ({})", details.join(", "))
                        }
                    );
                }
            }
        }
        VoicecallCommand::Stop => {
            let mut state = load_voicecall_state(&state_path)?;
            if !state.active {
                if cli.json {
                    print_json(&json!({
                        "ok": true,
                        "stopped": false,
                        "state": state,
                    }));
                } else {
                    println!("voicecall already stopped");
                }
                return Ok(());
            }

            let now = Utc::now();
            state.active = false;
            state.ended_at = Some(now);
            state.updated_at = now;
            save_voicecall_state(&state_path, &state)?;

            append_jsonl(
                &events_path,
                &VoicecallEvent {
                    ts: now,
                    call_id: state.call_id.clone(),
                    direction: "system".to_string(),
                    payload: "voicecall stopped".to_string(),
                    channel_id: None,
                    delivered_via: None,
                    delivery_status: None,
                    attempts: None,
                    http_status: None,
                    error: None,
                    endpoint_masked: None,
                    target_masked: None,
                    parse_mode: None,
                },
            )?;

            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "stopped": true,
                    "state": state,
                }));
            } else {
                println!("voicecall stopped");
            }
        }
    }

    Ok(())
}

fn validate_tts_voice(voice: &str) -> Result<()> {
    if TTS_VOICES.iter().any(|candidate| candidate == &voice) {
        Ok(())
    } else {
        Err(MosaicError::Validation(format!(
            "unsupported tts voice '{}'. available: {}",
            voice,
            TTS_VOICES.join(", ")
        )))
    }
}

fn normalize_tts_format(format: &str) -> Result<String> {
    let format = format.trim().to_ascii_lowercase();
    match format.as_str() {
        "wav" | "mp3" | "txt" => Ok(format),
        _ => Err(MosaicError::Validation(format!(
            "unsupported tts format '{}'. expected wav|mp3|txt",
            format
        ))),
    }
}

fn synthesize_tts_payload(text: &str, voice: &str, format: &str) -> String {
    format!("mosaic-tts-mock\nvoice={voice}\nformat={format}\ntext={text}\n")
}

fn write_tts_output(out: Option<&str>, payload: &str) -> Result<(Option<String>, Option<u64>)> {
    if let Some(out) = out {
        let out_path = PathBuf::from(out);
        if let Some(parent) = out_path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&out_path, payload.as_bytes())?;
        Ok((
            Some(out_path.display().to_string()),
            Some(payload.len() as u64),
        ))
    } else {
        Ok((None, None))
    }
}

fn preview_text(text: &str) -> String {
    const MAX_PREVIEW: usize = 120;
    let mut chars = text.chars();
    let preview: String = chars.by_ref().take(MAX_PREVIEW).collect();
    if chars.next().is_some() {
        format!("{preview}...")
    } else {
        preview
    }
}

fn normalize_optional(value: &Option<String>) -> Option<String> {
    value
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

fn append_jsonl<T: Serialize>(path: &Path, record: &T) -> Result<()> {
    append_sanitized_jsonl(path, record, "realtime event persistence")
        .map_err(|err| err.with_context(format!("failed to append event {}", path.display())))
}

fn write_json_report(path: &Path, payload: &serde_json::Value) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let encoded = serde_json::to_string_pretty(payload)
        .map_err(|err| MosaicError::Validation(format!("failed to encode report JSON: {err}")))?;
    fs::write(path, encoded)?;
    Ok(())
}

fn voicecall_state_path(data_dir: &Path) -> PathBuf {
    data_dir.join("voicecall-state.json")
}

fn voicecall_events_path(data_dir: &Path) -> PathBuf {
    data_dir.join("voicecall-events.jsonl")
}

fn tts_events_path(data_dir: &Path) -> PathBuf {
    data_dir.join("tts-events.jsonl")
}

fn channels_repository(data_dir: &Path) -> ChannelRepository {
    ChannelRepository::new(channels_file_path(data_dir), channels_events_dir(data_dir))
}

fn load_voicecall_state(path: &Path) -> Result<VoicecallState> {
    if !path.exists() {
        return Ok(default_voicecall_state());
    }
    let raw = fs::read_to_string(path)?;
    serde_json::from_str::<VoicecallState>(&raw).map_err(|err| {
        MosaicError::Validation(format!(
            "invalid voicecall state JSON {}: {err}",
            path.display()
        ))
    })
}

fn save_voicecall_state(path: &Path, state: &VoicecallState) -> Result<()> {
    write_pretty_state_json_file(path, state, "voicecall state")
}

fn default_voicecall_state() -> VoicecallState {
    VoicecallState {
        active: false,
        call_id: None,
        target: None,
        channel_id: None,
        started_at: None,
        ended_at: None,
        messages_sent: 0,
        updated_at: Utc::now(),
    }
}

fn load_voicecall_events(path: &Path, tail: usize) -> Result<Vec<VoicecallEvent>> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let content = fs::read_to_string(path)?;
    let mut events = Vec::new();
    for (idx, line) in content.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let event = serde_json::from_str::<VoicecallEvent>(line).map_err(|err| {
            MosaicError::Validation(format!(
                "invalid voicecall event at {} line {}: {err}",
                path.display(),
                idx + 1
            ))
        })?;
        events.push(event);
    }
    if events.len() > tail {
        events = events.split_off(events.len() - tail);
    }
    Ok(events)
}
