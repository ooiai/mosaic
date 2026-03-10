use std::collections::{HashMap, HashSet};

use chrono::{Duration, Utc};
use serde_json::{Value, json};

use mosaic_core::error::MosaicError;
use mosaic_ops::{
    ApprovalDecision, ApprovalStore, SandboxStore, evaluate_approval, evaluate_sandbox,
};

use super::{
    Cli, DeviceStatus, NodeRuntimeStatus, NodeTelemetryEventInput, NodesArgs, NodesCommand,
    PairingStatus, Result, devices_file_path, dispatch_gateway_call, load_devices_or_default,
    load_nodes_or_default, load_pairing_requests_or_default, next_pairing_seq,
    nodes_events_file_path, nodes_file_path, pairing_requests_file_path, parse_json_input,
    print_json, resolve_state_paths, save_devices, save_json_file, save_nodes,
    save_pairing_requests, write_nodes_event,
};

pub(super) async fn handle_nodes(cli: &Cli, args: NodesArgs) -> Result<()> {
    let paths = resolve_state_paths(cli.project_state)?;
    paths.ensure_dirs()?;
    let nodes_path = nodes_file_path(&paths.data_dir);
    let devices_path = devices_file_path(&paths.data_dir);
    let pairings_path = pairing_requests_file_path(&paths.data_dir);
    let events_path = nodes_events_file_path(&paths.data_dir);
    let mut nodes = load_nodes_or_default(&nodes_path)?;

    match args.command {
        NodesCommand::List => {
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "nodes": nodes,
                    "path": nodes_path.display().to_string(),
                }));
            } else {
                for node in nodes {
                    println!(
                        "{} name={} status={:?} capabilities={} last_seen={}",
                        node.id,
                        node.name,
                        node.status,
                        if node.capabilities.is_empty() {
                            "-".to_string()
                        } else {
                            node.capabilities.join(",")
                        },
                        node.last_seen_at.to_rfc3339()
                    );
                }
            }
        }
        NodesCommand::Status { node_id } => {
            let devices = load_devices_or_default(&devices_path)?;
            let pairings = load_pairing_requests_or_default(&pairings_path)?;
            if let Some(node_id) = node_id {
                let node = nodes
                    .iter()
                    .find(|item| item.id == node_id)
                    .ok_or_else(|| {
                        MosaicError::Validation(format!("node '{}' not found", node_id))
                    })?;
                let node_pairings = pairings
                    .iter()
                    .filter(|item| item.node_id == node.id)
                    .collect::<Vec<_>>();
                let pending_pairings = node_pairings
                    .iter()
                    .filter(|item| item.status == PairingStatus::Pending)
                    .count();
                let approved_pairings = node_pairings
                    .iter()
                    .filter(|item| item.status == PairingStatus::Approved)
                    .count();
                let rejected_pairings = node_pairings
                    .iter()
                    .filter(|item| item.status == PairingStatus::Rejected)
                    .count();
                if cli.json {
                    print_json(&json!({
                        "ok": true,
                        "node": node,
                        "pairings": {
                            "total": node_pairings.len(),
                            "pending": pending_pairings,
                            "approved": approved_pairings,
                            "rejected": rejected_pairings,
                        },
                        "approved_devices": devices
                            .iter()
                            .filter(|item| item.status == DeviceStatus::Approved)
                            .count(),
                    }));
                } else {
                    println!("node: {} ({})", node.id, node.name);
                    println!("status: {:?}", node.status);
                    println!(
                        "capabilities: {}",
                        if node.capabilities.is_empty() {
                            "-".to_string()
                        } else {
                            node.capabilities.join(",")
                        }
                    );
                    println!("last seen: {}", node.last_seen_at.to_rfc3339());
                    println!(
                        "pairings: total={} pending={} approved={} rejected={}",
                        node_pairings.len(),
                        pending_pairings,
                        approved_pairings,
                        rejected_pairings
                    );
                }
            } else if cli.json {
                let total_pairings = pairings.len();
                let pending_pairings = pairings
                    .iter()
                    .filter(|item| item.status == PairingStatus::Pending)
                    .count();
                let approved_pairings = pairings
                    .iter()
                    .filter(|item| item.status == PairingStatus::Approved)
                    .count();
                let rejected_pairings = pairings
                    .iter()
                    .filter(|item| item.status == PairingStatus::Rejected)
                    .count();
                let summary = json!({
                    "total": nodes.len(),
                    "online": nodes
                        .iter()
                        .filter(|item| item.status == NodeRuntimeStatus::Online)
                        .count(),
                    "approved_devices": devices
                        .iter()
                        .filter(|item| item.status == DeviceStatus::Approved)
                        .count(),
                    "total_pairings": total_pairings,
                    "pending_pairings": pending_pairings,
                    "approved_pairings": approved_pairings,
                    "rejected_pairings": rejected_pairings,
                });
                print_json(&json!({
                    "ok": true,
                    "summary": summary,
                    "nodes": nodes,
                }));
            } else {
                println!("nodes total: {}", nodes.len());
                println!(
                    "online: {}",
                    nodes
                        .iter()
                        .filter(|item| item.status == NodeRuntimeStatus::Online)
                        .count()
                );
            }
        }
        NodesCommand::Diagnose {
            node_id,
            stale_after_minutes,
            repair,
            report_out,
        } => {
            if stale_after_minutes == 0 {
                return Err(MosaicError::Validation(
                    "nodes diagnose --stale-after-minutes must be greater than 0".to_string(),
                ));
            }
            if let Some(node_id) = node_id.as_deref()
                && !nodes.iter().any(|item| item.id == node_id)
            {
                return Err(MosaicError::Validation(format!(
                    "node '{}' not found",
                    node_id
                )));
            }
            let mut devices = load_devices_or_default(&devices_path)?;
            let mut pairings = load_pairing_requests_or_default(&pairings_path)?;

            let stale_after_i64 = i64::try_from(stale_after_minutes).map_err(|_| {
                MosaicError::Validation(
                    "nodes diagnose --stale-after-minutes value is too large".to_string(),
                )
            })?;
            let now = Utc::now();
            let cutoff = now - Duration::minutes(stale_after_i64);
            let node_scope = node_id.clone();

            let mut issues = Vec::new();
            let mut actions = Vec::new();
            let mut stale_online_nodes = 0usize;
            let mut orphan_pairings = 0usize;
            let mut approved_pairing_device_mismatch = 0usize;
            let mut pending_pairing_blocked_device = 0usize;
            let mut nodes_checked = 0usize;
            let mut pairings_checked = 0usize;
            let mut nodes_changed = false;
            let mut devices_changed = false;
            let mut pairings_changed = false;

            let node_ids = nodes
                .iter()
                .map(|item| item.id.clone())
                .collect::<HashSet<_>>();
            let device_index = devices
                .iter()
                .enumerate()
                .map(|(idx, item)| (item.id.clone(), idx))
                .collect::<HashMap<_, _>>();
            let device_ids = device_index.keys().cloned().collect::<HashSet<_>>();

            for node in &mut nodes {
                if node_scope.as_deref().is_some_and(|scope| node.id != scope) {
                    continue;
                }
                nodes_checked += 1;
                let is_stale_online =
                    node.status == NodeRuntimeStatus::Online && node.last_seen_at < cutoff;
                if is_stale_online {
                    stale_online_nodes += 1;
                    issues.push(json!({
                        "kind": "stale_online_node",
                        "severity": "warn",
                        "node_id": node.id,
                        "status": node.status,
                        "last_seen_at": node.last_seen_at.to_rfc3339(),
                        "cutoff": cutoff.to_rfc3339(),
                        "detail": "node is online but heartbeat is stale beyond stale-after-minutes",
                        "repairable": true,
                    }));
                    if repair {
                        node.status = NodeRuntimeStatus::Offline;
                        node.updated_at = now;
                        nodes_changed = true;
                        actions.push(json!({
                            "kind": "mark_node_offline",
                            "ok": true,
                            "target": node.id,
                            "detail": "node status updated to offline due to stale heartbeat",
                        }));
                    }
                }
            }

            for pairing in &mut pairings {
                if node_scope
                    .as_deref()
                    .is_some_and(|scope| pairing.node_id != scope)
                {
                    continue;
                }
                pairings_checked += 1;
                let missing_node = !node_ids.contains(&pairing.node_id);
                let missing_device = !device_ids.contains(&pairing.device_id);
                if missing_node || missing_device {
                    orphan_pairings += 1;
                    issues.push(json!({
                        "kind": "orphan_pairing_reference",
                        "severity": "warn",
                        "request_id": pairing.id,
                        "node_id": pairing.node_id,
                        "device_id": pairing.device_id,
                        "pairing_status": pairing.status,
                        "missing_node": missing_node,
                        "missing_device": missing_device,
                        "detail": "pairing references missing node/device",
                        "repairable": pairing.status == PairingStatus::Pending,
                    }));
                    if repair && pairing.status == PairingStatus::Pending {
                        pairing.status = PairingStatus::Rejected;
                        pairing.reason = Some(
                            "auto-rejected: pairing references missing node/device".to_string(),
                        );
                        pairing.updated_at = now;
                        pairings_changed = true;
                        actions.push(json!({
                            "kind": "reject_orphan_pairing",
                            "ok": true,
                            "target": pairing.id,
                            "detail": "pending orphan pairing auto-rejected",
                        }));
                    }
                    continue;
                }

                if pairing.status == PairingStatus::Approved {
                    if let Some(idx) = device_index.get(&pairing.device_id).copied() {
                        let device = &mut devices[idx];
                        if device.status != DeviceStatus::Approved {
                            approved_pairing_device_mismatch += 1;
                            issues.push(json!({
                                "kind": "approved_pairing_device_mismatch",
                                "severity": "warn",
                                "request_id": pairing.id,
                                "node_id": pairing.node_id,
                                "device_id": pairing.device_id,
                                "device_status": device.status,
                                "detail": "pairing is approved but device is not approved",
                                "repairable": true,
                            }));
                            if repair {
                                device.status = DeviceStatus::Approved;
                                device.updated_at = now;
                                device.last_seen_at = now;
                                device.last_error = None;
                                devices_changed = true;
                                actions.push(json!({
                                    "kind": "repair_device_status",
                                    "ok": true,
                                    "target": device.id,
                                    "detail": "device status set to approved to match approved pairing",
                                }));
                            }
                        }
                    }
                } else if pairing.status == PairingStatus::Pending
                    && let Some(idx) = device_index.get(&pairing.device_id).copied()
                {
                    let device_status = devices[idx].status.clone();
                    if matches!(
                        device_status,
                        DeviceStatus::Rejected | DeviceStatus::Revoked
                    ) {
                        pending_pairing_blocked_device += 1;
                        issues.push(json!({
                            "kind": "pending_pairing_blocked_device",
                            "severity": "warn",
                            "request_id": pairing.id,
                            "node_id": pairing.node_id,
                            "device_id": pairing.device_id,
                            "device_status": device_status,
                            "detail": "pairing is pending but device status blocks approval",
                            "repairable": true,
                        }));
                        if repair {
                            pairing.status = PairingStatus::Rejected;
                            pairing.reason = Some(
                                "auto-rejected: pairing blocked by rejected/revoked device"
                                    .to_string(),
                            );
                            pairing.updated_at = now;
                            pairings_changed = true;
                            actions.push(json!({
                                "kind": "reject_blocked_pairing",
                                "ok": true,
                                "target": pairing.id,
                                "detail": "pending pairing auto-rejected due to blocked device status",
                            }));
                        }
                    }
                }
            }

            if nodes_changed {
                save_nodes(&nodes_path, &nodes)?;
            }
            if devices_changed {
                save_devices(&devices_path, &devices)?;
            }
            if pairings_changed {
                save_pairing_requests(&pairings_path, &pairings)?;
            }

            let issues_total = issues.len();
            let actions_applied = actions.len();
            let summary = json!({
                "nodes_checked": nodes_checked,
                "devices_checked": devices.len(),
                "pairings_checked": pairings_checked,
                "issues_total": issues_total,
                "stale_online_nodes": stale_online_nodes,
                "orphan_pairings": orphan_pairings,
                "approved_pairing_device_mismatch": approved_pairing_device_mismatch,
                "pending_pairing_blocked_device": pending_pairing_blocked_device,
                "actions_applied": actions_applied,
                "saved_nodes": nodes_changed,
                "saved_devices": devices_changed,
                "saved_pairings": pairings_changed,
            });

            let payload = json!({
                "ok": true,
                "node_scope": node_scope,
                "stale_after_minutes": stale_after_minutes,
                "cutoff": cutoff.to_rfc3339(),
                "repair": repair,
                "summary": summary,
                "issues": issues,
                "actions": actions,
                "report_out": report_out.as_ref().map(|path| path.to_string()),
                "paths": {
                    "nodes": nodes_path.display().to_string(),
                    "devices": devices_path.display().to_string(),
                    "pairings": pairings_path.display().to_string(),
                    "events": events_path.display().to_string(),
                },
            });
            if let Some(path) = report_out.as_deref() {
                save_json_file(std::path::Path::new(path), &payload)?;
            }
            write_nodes_event(
                &events_path,
                NodeTelemetryEventInput {
                    scope: "nodes",
                    action: "diagnose",
                    target_type: if node_scope.is_some() {
                        "node"
                    } else {
                        "scope"
                    },
                    target_id: node_scope.clone().unwrap_or_else(|| "all".to_string()),
                    success: issues_total == 0,
                    detail: format!(
                        "nodes diagnose issues={} actions_applied={} stale_after={}m repair={}",
                        issues_total, actions_applied, stale_after_minutes, repair
                    ),
                    node_id: node_scope.clone(),
                    device_id: None,
                    pairing_id: None,
                    repair: Some(repair),
                    issues_total: Some(issues_total),
                    actions_applied: Some(actions_applied),
                },
            );

            if cli.json {
                print_json(&payload);
            } else {
                println!(
                    "nodes diagnose scope={} stale_after={}m repair={}",
                    node_scope.as_deref().unwrap_or("all"),
                    stale_after_minutes,
                    repair
                );
                println!(
                    "issues={} stale_online_nodes={} orphan_pairings={} approved_pairing_device_mismatch={} pending_pairing_blocked_device={} actions_applied={}",
                    issues_total,
                    stale_online_nodes,
                    orphan_pairings,
                    approved_pairing_device_mismatch,
                    pending_pairing_blocked_device,
                    actions_applied
                );
                for issue in issues {
                    println!(
                        "- issue kind={} target={} detail={}",
                        issue["kind"].as_str().unwrap_or("-"),
                        issue["request_id"]
                            .as_str()
                            .or_else(|| issue["node_id"].as_str())
                            .or_else(|| issue["device_id"].as_str())
                            .unwrap_or("-"),
                        issue["detail"].as_str().unwrap_or("-")
                    );
                }
                if repair {
                    for action in actions {
                        println!(
                            "- action kind={} target={} ok={} detail={}",
                            action["kind"].as_str().unwrap_or("-"),
                            action["target"].as_str().unwrap_or("-"),
                            action["ok"].as_bool().unwrap_or(false),
                            action["detail"].as_str().unwrap_or("-")
                        );
                    }
                }
                if let Some(path) = report_out.as_deref() {
                    println!("report: {path}");
                }
            }
        }
        NodesCommand::Run { node_id, command } => {
            let now = Utc::now();
            let run_id = format!("run-{}-{}", now.timestamp_millis(), next_pairing_seq());
            let node = nodes
                .iter_mut()
                .find(|item| item.id == node_id)
                .ok_or_else(|| MosaicError::Validation(format!("node '{}' not found", node_id)))?;
            node.last_seen_at = now;
            node.updated_at = now;
            let node_id = node.id.clone();
            save_nodes(&nodes_path, &nodes)?;

            let approval_store = ApprovalStore::new(paths.approvals_policy_path.clone());
            let sandbox_store = SandboxStore::new(paths.sandbox_policy_path.clone());
            let approval_policy = approval_store.load_or_default()?;
            let sandbox_policy = sandbox_store.load_or_default()?;
            if let Some(reason) = evaluate_sandbox(&command, sandbox_policy.profile) {
                return Err(MosaicError::SandboxDenied(reason));
            }
            let approved_by = match evaluate_approval(&command, &approval_policy) {
                ApprovalDecision::Auto { approved_by } => approved_by,
                ApprovalDecision::NeedsConfirmation { reason } => {
                    if cli.yes {
                        "flag_yes".to_string()
                    } else {
                        return Err(MosaicError::ApprovalRequired(format!(
                            "{reason}. rerun with --yes"
                        )));
                    }
                }
                ApprovalDecision::Deny { reason } => {
                    return Err(MosaicError::ApprovalRequired(reason));
                }
            };
            let gateway_path = paths.data_dir.join("gateway.json");
            let gateway_service_path = paths.data_dir.join("gateway-service.json");
            let gateway = match dispatch_gateway_call(
                &gateway_path,
                &gateway_service_path,
                "nodes.run",
                json!({
                    "node_id": node_id.clone(),
                    "command": command.clone(),
                    "approved_by": approved_by.clone(),
                }),
            )
            .await
            {
                Ok(value) => value,
                Err(err) => {
                    write_nodes_event(
                        &events_path,
                        NodeTelemetryEventInput {
                            scope: "nodes",
                            action: "run",
                            target_type: "node",
                            target_id: node_id.clone(),
                            success: false,
                            detail: err.to_string(),
                            node_id: Some(node_id.clone()),
                            device_id: None,
                            pairing_id: None,
                            repair: None,
                            issues_total: None,
                            actions_applied: None,
                        },
                    );
                    return Err(err);
                }
            };
            let accepted = gateway
                .result
                .get("ok")
                .and_then(Value::as_bool)
                .unwrap_or(true);
            let status = gateway
                .result
                .get("status")
                .and_then(Value::as_str)
                .unwrap_or(if accepted { "accepted" } else { "failed" });
            write_nodes_event(
                &events_path,
                NodeTelemetryEventInput {
                    scope: "nodes",
                    action: "run",
                    target_type: "node",
                    target_id: node_id.clone(),
                    success: accepted,
                    detail: format!(
                        "nodes.run status={} gateway={} request_id={}",
                        status, gateway.host, gateway.request_id
                    ),
                    node_id: Some(node_id.clone()),
                    device_id: None,
                    pairing_id: None,
                    repair: None,
                    issues_total: None,
                    actions_applied: None,
                },
            );

            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "run_id": run_id,
                    "node_id": node_id.clone(),
                    "accepted": accepted,
                    "status": status,
                    "gateway": {
                        "host": gateway.host,
                        "port": gateway.port,
                        "request_id": gateway.request_id,
                    },
                    "result": gateway.result,
                }));
            } else {
                println!("run submitted");
                println!("run id: {run_id}");
                println!("node: {}", node_id);
                println!("status: {status}");
                println!(
                    "gateway: {}:{} request_id={}",
                    gateway.host, gateway.port, gateway.request_id
                );
            }
        }
        NodesCommand::Invoke {
            node_id,
            method,
            params,
        } => {
            let now = Utc::now();
            let invoke_id = format!("invoke-{}-{}", now.timestamp_millis(), next_pairing_seq());
            let node = nodes
                .iter_mut()
                .find(|item| item.id == node_id)
                .ok_or_else(|| MosaicError::Validation(format!("node '{}' not found", node_id)))?;
            node.last_seen_at = now;
            node.updated_at = now;
            let node_id = node.id.clone();
            save_nodes(&nodes_path, &nodes)?;

            let parsed_params = params
                .as_deref()
                .map(|value| parse_json_input(value, "invoke params"))
                .transpose()?
                .unwrap_or(Value::Null);
            let gateway_path = paths.data_dir.join("gateway.json");
            let gateway_service_path = paths.data_dir.join("gateway-service.json");
            let gateway = match dispatch_gateway_call(
                &gateway_path,
                &gateway_service_path,
                "nodes.invoke",
                json!({
                    "node_id": node_id.clone(),
                    "method": method.clone(),
                    "params": parsed_params.clone(),
                }),
            )
            .await
            {
                Ok(value) => value,
                Err(err) => {
                    write_nodes_event(
                        &events_path,
                        NodeTelemetryEventInput {
                            scope: "nodes",
                            action: "invoke",
                            target_type: "node",
                            target_id: node_id.clone(),
                            success: false,
                            detail: err.to_string(),
                            node_id: Some(node_id.clone()),
                            device_id: None,
                            pairing_id: None,
                            repair: None,
                            issues_total: None,
                            actions_applied: None,
                        },
                    );
                    return Err(err);
                }
            };
            let invoke_ok = gateway
                .result
                .get("ok")
                .and_then(Value::as_bool)
                .unwrap_or(true);
            write_nodes_event(
                &events_path,
                NodeTelemetryEventInput {
                    scope: "nodes",
                    action: "invoke",
                    target_type: "node",
                    target_id: node_id.clone(),
                    success: invoke_ok,
                    detail: format!(
                        "nodes.invoke method={} gateway={} request_id={}",
                        method, gateway.host, gateway.request_id
                    ),
                    node_id: Some(node_id.clone()),
                    device_id: None,
                    pairing_id: None,
                    repair: None,
                    issues_total: None,
                    actions_applied: None,
                },
            );

            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "invoke_id": invoke_id,
                    "node_id": node_id.clone(),
                    "method": method.clone(),
                    "gateway": {
                        "host": gateway.host,
                        "port": gateway.port,
                        "request_id": gateway.request_id,
                    },
                    "result": gateway.result,
                }));
            } else {
                println!("invoke accepted");
                println!("invoke id: {invoke_id}");
                println!("node: {}", node_id);
                println!("method: {method}");
                println!(
                    "gateway: {}:{} request_id={}",
                    gateway.host, gateway.port, gateway.request_id
                );
            }
        }
    }
    Ok(())
}
