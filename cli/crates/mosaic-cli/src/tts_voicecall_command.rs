use std::fs::{self, OpenOptions};
use std::io::Write as _;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

use mosaic_core::error::{MosaicError, Result};

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

            let mut bytes_written = None;
            let mut output_path = None;
            if let Some(out) = out.as_deref() {
                let out_path = PathBuf::from(out);
                if let Some(parent) = out_path.parent() {
                    fs::create_dir_all(parent)?;
                }
                let payload = synthesize_tts_payload(text, &voice, &format);
                fs::write(&out_path, payload.as_bytes())?;
                bytes_written = Some(payload.len() as u64);
                output_path = Some(out_path.display().to_string());
            }

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
    }
    Ok(())
}

pub(super) fn handle_voicecall(cli: &Cli, args: VoicecallArgs) -> Result<()> {
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
                println!(
                    "call_id: {}",
                    state.call_id.unwrap_or_else(|| "-".to_string())
                );
            }
        }
        VoicecallCommand::Send { text } => {
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

            let now = Utc::now();
            state.messages_sent = state.messages_sent.saturating_add(1);
            state.updated_at = now;
            save_voicecall_state(&state_path, &state)?;

            append_jsonl(
                &events_path,
                &VoicecallEvent {
                    ts: now,
                    call_id: state.call_id.clone(),
                    direction: "outbound".to_string(),
                    payload: preview_text(text),
                },
            )?;

            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "call_id": state.call_id,
                    "messages_sent": state.messages_sent,
                    "text_preview": preview_text(text),
                }));
            } else {
                println!("voicecall message sent");
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
                    println!(
                        "{} [{}] {}",
                        event.ts.to_rfc3339(),
                        event.direction,
                        event.payload
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
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    let line = serde_json::to_string(record)
        .map_err(|err| MosaicError::Validation(format!("failed to encode jsonl record: {err}")))?;
    file.write_all(line.as_bytes())?;
    file.write_all(b"\n")?;
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
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let payload = serde_json::to_string_pretty(state).map_err(|err| {
        MosaicError::Validation(format!("failed to encode voicecall state: {err}"))
    })?;
    fs::write(path, payload)?;
    Ok(())
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
