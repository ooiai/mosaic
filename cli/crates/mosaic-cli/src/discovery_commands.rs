use std::collections::BTreeSet;
use std::fs::{self, File};
use std::io::BufWriter;
use std::net::ToSocketAddrs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use mosaic_core::error::{MosaicError, Result};
use qrcodegen::{QrCode, QrCodeEcc};
use serde_json::json;

use super::utils::print_json;
use super::{Cli, DnsArgs, DnsCommand, DocsArgs, QrArgs, QrCommand, QrRenderArg};

const DOC_TOPICS: [(&str, &str); 16] = [
    ("cli", "cli/README.md"),
    ("channels", "cli/docs/channels-slack.md"),
    ("gateway", "cli/docs/gateway-ops.md"),
    ("mcp", "cli/docs/mcp.md"),
    ("knowledge", "cli/docs/knowledge.md"),
    ("beta", "cli/docs/beta-release.md"),
    ("distribution", "cli/docs/distribution.md"),
    ("observability", "cli/docs/observability.md"),
    ("security", "cli/docs/security-audit.md"),
    ("approvals", "cli/docs/sandbox-approvals.md"),
    ("safety", "cli/docs/sandbox-approvals.md"),
    ("memory", "cli/docs/memory.md"),
    ("agents", "cli/docs/agents.md"),
    ("regression", "cli/docs/regression-runbook.md"),
    ("progress", "cli/docs/progress.md"),
    ("worklog", "WORKLOG.md"),
];

pub(super) fn handle_docs(cli: &Cli, args: DocsArgs) -> Result<()> {
    let topics = DOC_TOPICS
        .iter()
        .map(|(topic, url)| json!({ "topic": topic, "url": url }))
        .collect::<Vec<_>>();

    if let Some(topic) = args.topic {
        let normalized = topic.trim().to_ascii_lowercase();
        let Some((resolved_topic, url)) = DOC_TOPICS
            .iter()
            .find(|(candidate, _)| candidate == &normalized)
        else {
            return Err(MosaicError::Validation(format!(
                "unknown docs topic '{}'. run `mosaic docs` to list topics",
                topic
            )));
        };

        if cli.json {
            print_json(&json!({
                "ok": true,
                "topic": resolved_topic,
                "url": url,
            }));
        } else {
            println!("{resolved_topic}: {url}");
        }
        return Ok(());
    }

    if cli.json {
        print_json(&json!({
            "ok": true,
            "topics": topics,
        }));
    } else {
        println!("Docs topics:");
        for (topic, url) in DOC_TOPICS {
            println!("- {topic}: {url}");
        }
    }
    Ok(())
}

pub(super) fn handle_dns(cli: &Cli, args: DnsArgs) -> Result<()> {
    match args.command {
        DnsCommand::Resolve { host, port } => {
            let target = format!("{host}:{port}");
            let resolved = target.to_socket_addrs().map_err(|err| {
                MosaicError::Network(format!("failed to resolve '{}': {err}", target))
            })?;
            let addresses = resolved
                .map(|addr| addr.ip().to_string())
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect::<Vec<_>>();
            if addresses.is_empty() {
                return Err(MosaicError::Network(format!(
                    "dns resolution returned no addresses for '{}'",
                    target
                )));
            }

            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "host": host,
                    "port": port,
                    "addresses": addresses,
                }));
            } else {
                println!("host: {host}");
                println!("port: {port}");
                for address in addresses {
                    println!("- {address}");
                }
            }
        }
    }
    Ok(())
}

pub(super) fn handle_qr(cli: &Cli, args: QrArgs) -> Result<()> {
    match args.command {
        QrCommand::Encode {
            value,
            render,
            output,
            quiet_zone,
            module_size,
        } => {
            let value = value.trim();
            if value.is_empty() {
                return Err(MosaicError::Validation(
                    "qr value cannot be empty".to_string(),
                ));
            }
            let payload = format!("mosaic://qr?value={}", url_encode(value));
            let rendered = render_qr_payload(&payload, render, output, quiet_zone, module_size)?;
            emit_qr_output(
                cli,
                "encode",
                &payload,
                rendered,
                json!({
                    "value": value,
                }),
            );
        }
        QrCommand::Pairing {
            device,
            node,
            ttl_seconds,
            render,
            output,
            quiet_zone,
            module_size,
        } => {
            let device = device.trim();
            if device.is_empty() {
                return Err(MosaicError::Validation(
                    "--device cannot be empty".to_string(),
                ));
            }
            let node = node.trim();
            if node.is_empty() {
                return Err(MosaicError::Validation(
                    "--node cannot be empty".to_string(),
                ));
            }
            let now = unix_timestamp()?;
            let expires_at = now.saturating_add(ttl_seconds);
            let payload = format!(
                "mosaic://pairing?device={}&node={}&exp={}",
                url_encode(device),
                url_encode(node),
                expires_at
            );
            let rendered = render_qr_payload(&payload, render, output, quiet_zone, module_size)?;
            emit_qr_output(
                cli,
                "pairing",
                &payload,
                rendered,
                json!({
                    "device": device,
                    "node": node,
                    "expires_at": expires_at,
                    "ttl_seconds": ttl_seconds,
                }),
            );
        }
    }
    Ok(())
}

enum RenderedQr {
    Payload,
    Ascii(String),
    Png { output: String },
}

fn emit_qr_output(
    cli: &Cli,
    kind: &str,
    payload: &str,
    rendered: RenderedQr,
    metadata: serde_json::Value,
) {
    if cli.json {
        let mut body = json!({
            "ok": true,
            "kind": kind,
            "payload": payload,
        });
        if let Some(root) = body.as_object_mut() {
            if let Some(meta) = metadata.as_object() {
                for (key, value) in meta {
                    root.insert(key.to_string(), value.clone());
                }
            }
            match &rendered {
                RenderedQr::Payload => {
                    root.insert("render".to_string(), json!("payload"));
                }
                RenderedQr::Ascii(ascii) => {
                    root.insert("render".to_string(), json!("ascii"));
                    root.insert("ascii".to_string(), json!(ascii));
                }
                RenderedQr::Png { output } => {
                    root.insert("render".to_string(), json!("png"));
                    root.insert("output".to_string(), json!(output));
                }
            }
        }
        print_json(&body);
        return;
    }

    match rendered {
        RenderedQr::Payload => println!("{payload}"),
        RenderedQr::Ascii(ascii) => println!("{ascii}"),
        RenderedQr::Png { output } => {
            println!("saved: {output}");
            println!("payload: {payload}");
        }
    }
}

fn render_qr_payload(
    payload: &str,
    render: QrRenderArg,
    output: Option<String>,
    quiet_zone: u8,
    module_size: u32,
) -> Result<RenderedQr> {
    if module_size == 0 {
        return Err(MosaicError::Validation(
            "--module-size must be greater than 0".to_string(),
        ));
    }
    if quiet_zone > 32 {
        return Err(MosaicError::Validation(
            "--quiet-zone must be less than or equal to 32".to_string(),
        ));
    }

    if matches!(render, QrRenderArg::Payload) {
        if output.is_some() {
            return Err(MosaicError::Validation(
                "--output is only supported when --render png".to_string(),
            ));
        }
        return Ok(RenderedQr::Payload);
    }

    let qr = QrCode::encode_text(payload, QrCodeEcc::Medium)
        .map_err(|err| MosaicError::Validation(format!("failed to encode qr payload: {err}")))?;

    match render {
        QrRenderArg::Payload => Ok(RenderedQr::Payload),
        QrRenderArg::Ascii => {
            if output.is_some() {
                return Err(MosaicError::Validation(
                    "--output is only supported when --render png".to_string(),
                ));
            }
            Ok(RenderedQr::Ascii(render_qr_ascii(
                &qr,
                i32::from(quiet_zone),
            )))
        }
        QrRenderArg::Png => {
            let output = output
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
                .ok_or_else(|| {
                    MosaicError::Validation("--output is required when --render png".to_string())
                })?;
            write_qr_png(&qr, Path::new(&output), quiet_zone, module_size)?;
            Ok(RenderedQr::Png { output })
        }
    }
}

fn render_qr_ascii(qr: &QrCode, quiet_zone: i32) -> String {
    let size = qr.size();
    let mut lines = Vec::new();
    for y in -quiet_zone..(size + quiet_zone) {
        let mut line = String::new();
        for x in -quiet_zone..(size + quiet_zone) {
            if qr.get_module(x, y) {
                line.push_str("██");
            } else {
                line.push_str("  ");
            }
        }
        lines.push(line);
    }
    lines.join("\n")
}

fn write_qr_png(qr: &QrCode, output: &Path, quiet_zone: u8, module_size: u32) -> Result<()> {
    let border = i32::from(quiet_zone);
    let total_modules = qr.size() + border * 2;
    let total_modules_u32 = u32::try_from(total_modules).map_err(|_| {
        MosaicError::Validation("qr dimensions are invalid for png rendering".to_string())
    })?;
    let side = total_modules_u32.checked_mul(module_size).ok_or_else(|| {
        MosaicError::Validation("png dimensions overflow from qr settings".to_string())
    })?;
    let pixels_len = side
        .checked_mul(side)
        .and_then(|px| usize::try_from(px).ok())
        .ok_or_else(|| MosaicError::Validation("png buffer size overflow".to_string()))?;

    let mut pixels = vec![255_u8; pixels_len];
    for y_mod in 0..total_modules_u32 {
        for x_mod in 0..total_modules_u32 {
            let qr_x = x_mod as i32 - border;
            let qr_y = y_mod as i32 - border;
            if !qr.get_module(qr_x, qr_y) {
                continue;
            }
            let x_start = x_mod * module_size;
            let y_start = y_mod * module_size;
            for dy in 0..module_size {
                for dx in 0..module_size {
                    let px = x_start + dx;
                    let py = y_start + dy;
                    let idx = (py * side + px) as usize;
                    pixels[idx] = 0;
                }
            }
        }
    }

    if let Some(parent) = output.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent).map_err(|err| {
            MosaicError::Io(format!(
                "failed to create parent directory '{}': {err}",
                parent.display()
            ))
        })?;
    }

    let file = File::create(output).map_err(|err| {
        MosaicError::Io(format!(
            "failed to create png output '{}': {err}",
            output.display()
        ))
    })?;
    let writer = BufWriter::new(file);
    let mut encoder = png::Encoder::new(writer, side, side);
    encoder.set_color(png::ColorType::Grayscale);
    encoder.set_depth(png::BitDepth::Eight);
    let mut png_writer = encoder.write_header().map_err(|err| {
        MosaicError::Io(format!(
            "failed to write png header '{}': {err}",
            output.display()
        ))
    })?;
    png_writer.write_image_data(&pixels).map_err(|err| {
        MosaicError::Io(format!(
            "failed to write png data '{}': {err}",
            output.display()
        ))
    })?;

    Ok(())
}

fn unix_timestamp() -> Result<u64> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|err| MosaicError::Unknown(format!("system clock before unix epoch: {err}")))?;
    Ok(now.as_secs())
}

fn url_encode(value: &str) -> String {
    let mut encoded = String::with_capacity(value.len());
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
            encoded.push(byte as char);
        } else {
            encoded.push('%');
            encoded.push(hex_digit(byte >> 4));
            encoded.push(hex_digit(byte & 0x0f));
        }
    }
    encoded
}

fn hex_digit(value: u8) -> char {
    match value {
        0..=9 => (b'0' + value) as char,
        10..=15 => (b'A' + (value - 10)) as char,
        _ => '0',
    }
}
