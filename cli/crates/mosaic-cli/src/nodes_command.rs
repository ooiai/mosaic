use chrono::Utc;
use serde_json::{Value, json};

use mosaic_core::error::MosaicError;
use mosaic_ops::{
    ApprovalDecision, ApprovalStore, SandboxStore, evaluate_approval, evaluate_sandbox,
};

use super::{
    Cli, DeviceStatus, NodeRuntimeStatus, NodesArgs, NodesCommand, PairingStatus, Result,
    devices_file_path, dispatch_gateway_call, load_devices_or_default, load_nodes_or_default,
    load_pairing_requests_or_default, next_pairing_seq, nodes_file_path,
    pairing_requests_file_path, parse_json_input, print_json, resolve_state_paths, save_nodes,
};

pub(super) async fn handle_nodes(cli: &Cli, args: NodesArgs) -> Result<()> {
    let paths = resolve_state_paths(cli.project_state)?;
    paths.ensure_dirs()?;
    let nodes_path = nodes_file_path(&paths.data_dir);
    let devices_path = devices_file_path(&paths.data_dir);
    let pairings_path = pairing_requests_file_path(&paths.data_dir);
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
                if cli.json {
                    print_json(&json!({
                        "ok": true,
                        "node": node,
                        "pairings": {
                            "total": node_pairings.len(),
                            "pending": node_pairings
                                .iter()
                                .filter(|item| item.status == PairingStatus::Pending)
                                .count(),
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
                        "pairings: total={} pending={}",
                        node_pairings.len(),
                        node_pairings
                            .iter()
                            .filter(|item| item.status == PairingStatus::Pending)
                            .count()
                    );
                }
            } else if cli.json {
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
                    "pending_pairings": pairings
                        .iter()
                        .filter(|item| item.status == PairingStatus::Pending)
                        .count(),
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
            let gateway = dispatch_gateway_call(
                &gateway_path,
                &gateway_service_path,
                "nodes.run",
                json!({
                    "node_id": node_id.clone(),
                    "command": command.clone(),
                    "approved_by": approved_by.clone(),
                }),
            )
            .await?;
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
            let gateway = dispatch_gateway_call(
                &gateway_path,
                &gateway_service_path,
                "nodes.invoke",
                json!({
                    "node_id": node_id.clone(),
                    "method": method.clone(),
                    "params": parsed_params.clone(),
                }),
            )
            .await?;

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
