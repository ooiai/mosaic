use chrono::Utc;
use serde_json::json;

use mosaic_core::error::MosaicError;

use super::{
    Cli, DeviceRecord, DeviceStatus, DevicesArgs, DevicesCommand, NodeRuntimeStatus, PairingArgs,
    PairingCommand, PairingRequestRecord, PairingStatus, Result, devices_file_path,
    generate_pairing_request_id, load_devices_or_default, load_nodes_or_default,
    load_pairing_requests_or_default, next_pairing_seq, nodes_file_path,
    pairing_requests_file_path, print_json, resolve_state_paths, save_devices, save_nodes,
    save_pairing_requests,
};

pub(super) fn handle_devices(cli: &Cli, args: DevicesArgs) -> Result<()> {
    let paths = resolve_state_paths(cli.project_state)?;
    paths.ensure_dirs()?;
    let devices_path = devices_file_path(&paths.data_dir);
    let mut devices = load_devices_or_default(&devices_path)?;

    match args.command {
        DevicesCommand::List => {
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "devices": devices,
                    "path": devices_path.display().to_string(),
                }));
            } else if devices.is_empty() {
                println!("No devices found.");
            } else {
                for device in devices {
                    println!(
                        "{} name={} status={:?} token_v={} last_seen={} last_error={}",
                        device.id,
                        device.name,
                        device.status,
                        device.token_version,
                        device.last_seen_at.to_rfc3339(),
                        device.last_error.unwrap_or_else(|| "-".to_string())
                    );
                }
            }
        }
        DevicesCommand::Approve { device_id, name } => {
            let now = Utc::now();
            let fingerprint = format!("fp-{}-{}", now.timestamp_millis(), next_pairing_seq());
            let device =
                if let Some(existing) = devices.iter_mut().find(|item| item.id == device_id) {
                    existing.status = DeviceStatus::Approved;
                    if let Some(name) = name {
                        existing.name = name.trim().to_string();
                    }
                    existing.updated_at = now;
                    existing.last_seen_at = now;
                    existing.last_error = None;
                    existing.clone()
                } else {
                    let device = DeviceRecord {
                        id: device_id.clone(),
                        name: name.unwrap_or_else(|| device_id.clone()),
                        fingerprint,
                        status: DeviceStatus::Approved,
                        token_version: 1,
                        last_seen_at: now,
                        updated_at: now,
                        last_error: None,
                    };
                    devices.push(device.clone());
                    device
                };
            save_devices(&devices_path, &devices)?;
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "device": device,
                }));
            } else {
                println!("device approved: {}", device.id);
            }
        }
        DevicesCommand::Reject { device_id, reason } => {
            let now = Utc::now();
            let device = devices
                .iter_mut()
                .find(|item| item.id == device_id)
                .ok_or_else(|| {
                    MosaicError::Validation(format!("device '{}' not found", device_id))
                })?;
            device.status = DeviceStatus::Rejected;
            device.last_error = reason.clone();
            device.updated_at = now;
            let device = device.clone();
            save_devices(&devices_path, &devices)?;
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "device": device,
                }));
            } else {
                println!("device rejected: {}", device.id);
            }
        }
        DevicesCommand::Rotate { device_id } => {
            let now = Utc::now();
            let device = devices
                .iter_mut()
                .find(|item| item.id == device_id)
                .ok_or_else(|| {
                    MosaicError::Validation(format!("device '{}' not found", device_id))
                })?;
            if device.status != DeviceStatus::Approved {
                return Err(MosaicError::Validation(format!(
                    "device '{}' must be approved before rotate",
                    device_id
                )));
            }
            device.token_version = device.token_version.saturating_add(1);
            device.updated_at = now;
            device.last_seen_at = now;
            let device = device.clone();
            save_devices(&devices_path, &devices)?;
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "device": device,
                }));
            } else {
                println!(
                    "device rotated: {} token_v={}",
                    device.id, device.token_version
                );
            }
        }
        DevicesCommand::Revoke { device_id, reason } => {
            let now = Utc::now();
            let device = devices
                .iter_mut()
                .find(|item| item.id == device_id)
                .ok_or_else(|| {
                    MosaicError::Validation(format!("device '{}' not found", device_id))
                })?;
            device.status = DeviceStatus::Revoked;
            device.last_error = reason.clone();
            device.updated_at = now;
            let device = device.clone();
            save_devices(&devices_path, &devices)?;
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "device": device,
                }));
            } else {
                println!("device revoked: {}", device.id);
            }
        }
    }
    Ok(())
}

pub(super) fn handle_pairing(cli: &Cli, args: PairingArgs) -> Result<()> {
    let paths = resolve_state_paths(cli.project_state)?;
    paths.ensure_dirs()?;
    let pairings_path = pairing_requests_file_path(&paths.data_dir);
    let devices_path = devices_file_path(&paths.data_dir);
    let nodes_path = nodes_file_path(&paths.data_dir);
    let mut pairings = load_pairing_requests_or_default(&pairings_path)?;
    let mut devices = load_devices_or_default(&devices_path)?;
    let mut nodes = load_nodes_or_default(&nodes_path)?;

    match args.command {
        PairingCommand::List { status } => {
            let filtered = if let Some(status) = status {
                let status: PairingStatus = status.into();
                pairings
                    .into_iter()
                    .filter(|item| item.status == status)
                    .collect::<Vec<_>>()
            } else {
                pairings
            };
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "requests": filtered,
                    "path": pairings_path.display().to_string(),
                }));
            } else if filtered.is_empty() {
                println!("No pairing requests.");
            } else {
                for request in filtered {
                    println!(
                        "{} device={} node={} status={:?} requested={}",
                        request.id,
                        request.device_id,
                        request.node_id,
                        request.status,
                        request.requested_at.to_rfc3339()
                    );
                }
            }
        }
        PairingCommand::Approve { request_id } => {
            let now = Utc::now();
            let request = pairings
                .iter_mut()
                .find(|item| item.id == request_id)
                .ok_or_else(|| {
                    MosaicError::Validation(format!("pairing request '{}' not found", request_id))
                })?;
            if request.status != PairingStatus::Pending {
                return Err(MosaicError::Validation(format!(
                    "pairing request '{}' is not pending",
                    request_id
                )));
            }
            request.status = PairingStatus::Approved;
            request.updated_at = now;
            let request_device_id = request.device_id.clone();
            let request_node_id = request.node_id.clone();

            let device = if let Some(device) =
                devices.iter_mut().find(|item| item.id == request_device_id)
            {
                device.status = DeviceStatus::Approved;
                device.updated_at = now;
                device.last_seen_at = now;
                device.last_error = None;
                device.clone()
            } else {
                let device = DeviceRecord {
                    id: request_device_id.clone(),
                    name: request_device_id.clone(),
                    fingerprint: format!("fp-{}-{}", now.timestamp_millis(), next_pairing_seq()),
                    status: DeviceStatus::Approved,
                    token_version: 1,
                    last_seen_at: now,
                    updated_at: now,
                    last_error: None,
                };
                devices.push(device.clone());
                device
            };

            let node = nodes
                .iter_mut()
                .find(|item| item.id == request_node_id)
                .ok_or_else(|| {
                    MosaicError::Validation(format!("node '{}' not found", request_node_id))
                })?;
            node.status = NodeRuntimeStatus::Online;
            node.last_seen_at = now;
            node.updated_at = now;
            let request = request.clone();

            save_pairing_requests(&pairings_path, &pairings)?;
            save_devices(&devices_path, &devices)?;
            save_nodes(&nodes_path, &nodes)?;
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "request": request,
                    "device": device,
                }));
            } else {
                println!("pairing approved: {}", request.id);
                println!("device: {}", device.id);
            }
        }
        PairingCommand::Reject { request_id, reason } => {
            let now = Utc::now();
            let request = pairings
                .iter_mut()
                .find(|item| item.id == request_id)
                .ok_or_else(|| {
                    MosaicError::Validation(format!("pairing request '{}' not found", request_id))
                })?;
            if request.status != PairingStatus::Pending {
                return Err(MosaicError::Validation(format!(
                    "pairing request '{}' is not pending",
                    request_id
                )));
            }
            request.status = PairingStatus::Rejected;
            request.updated_at = now;
            request.reason = reason.and_then(|raw| {
                let trimmed = raw.trim();
                if trimmed.is_empty() {
                    None
                } else {
                    Some(trimmed.to_string())
                }
            });
            let request = request.clone();

            if let Some(device) = devices.iter_mut().find(|item| item.id == request.device_id) {
                device.status = DeviceStatus::Rejected;
                device.last_error = request.reason.clone();
                device.updated_at = now;
            }
            save_pairing_requests(&pairings_path, &pairings)?;
            save_devices(&devices_path, &devices)?;
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "request": request,
                }));
            } else {
                println!("pairing rejected: {}", request.id);
            }
        }
        PairingCommand::Request {
            device,
            node,
            reason,
        } => {
            let now = Utc::now();
            if !nodes.iter().any(|item| item.id == node) {
                return Err(MosaicError::Validation(format!(
                    "node '{}' not found",
                    node
                )));
            }
            if !devices.iter().any(|item| item.id == device) {
                devices.push(DeviceRecord {
                    id: device.clone(),
                    name: device.clone(),
                    fingerprint: format!("fp-{}-{}", now.timestamp_millis(), next_pairing_seq()),
                    status: DeviceStatus::Pending,
                    token_version: 1,
                    last_seen_at: now,
                    updated_at: now,
                    last_error: None,
                });
                save_devices(&devices_path, &devices)?;
            }
            let request = PairingRequestRecord {
                id: generate_pairing_request_id(),
                device_id: device,
                node_id: node,
                status: PairingStatus::Pending,
                reason,
                requested_at: now,
                updated_at: now,
            };
            pairings.push(request.clone());
            save_pairing_requests(&pairings_path, &pairings)?;
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "request": request,
                }));
            } else {
                println!("pairing request created: {}", request.id);
            }
        }
    }
    Ok(())
}
