use std::fs::{self, OpenOptions};
use std::io::Write as _;
use std::path::Path;
use std::time::Instant;

use chrono::Utc;
use serde::Serialize;
use serde_json::{Value, json};
use uuid::Uuid;

use mosaic_core::error::MosaicError;
use mosaic_gateway::{GatewayClient, GatewayRequest};

use super::{
    Cli, GatewayArgs, GatewayCommand, GatewayState, HttpGatewayClient, Result,
    collect_gateway_runtime_status, emit_checks, gateway_test_mode, load_json_file_opt,
    parse_json_input, print_json, resolve_gateway_start_target, resolve_gateway_target,
    resolve_state_paths, run_check, run_gateway_http_server, start_gateway_runtime,
    stop_gateway_runtime, upsert_gateway_service,
};

pub(super) async fn handle_gateway(cli: &Cli, args: GatewayArgs) -> Result<()> {
    let paths = resolve_state_paths(cli.project_state)?;
    paths.ensure_dirs()?;
    let gateway_path = paths.data_dir.join("gateway.json");
    let gateway_service_path = paths.data_dir.join("gateway-service.json");
    let gateway_events_path = paths.data_dir.join("gateway-events.jsonl");
    match args.command {
        GatewayCommand::Install { host, port } => {
            let service =
                upsert_gateway_service(&gateway_service_path, Some(host), Some(port), true)?;
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "service": service,
                    "path": gateway_service_path.display().to_string(),
                }));
            } else {
                println!("gateway service installed");
                println!("host: {}", service.host);
                println!("port: {}", service.port);
                println!("path: {}", gateway_service_path.display());
            }
        }
        GatewayCommand::Start { host, port } => {
            let (resolved_host, resolved_port) =
                resolve_gateway_start_target(&gateway_service_path, host, port, "127.0.0.1", 8787)?;
            let service = upsert_gateway_service(
                &gateway_service_path,
                Some(resolved_host.clone()),
                Some(resolved_port),
                true,
            )?;
            let start =
                start_gateway_runtime(cli, &gateway_path, resolved_host, resolved_port).await?;
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "already_running": start.already_running,
                    "gateway": start.state,
                    "service": service,
                    "path": gateway_path.display().to_string(),
                }));
            } else if start.already_running {
                println!(
                    "Gateway already running on {}:{}",
                    start.state.host, start.state.port
                );
            } else {
                println!("Gateway is running.");
                println!("host: {}", start.state.host);
                println!("port: {}", start.state.port);
                println!("pid: {}", start.state.pid);
                println!("state: {}", gateway_path.display());
            }
        }
        GatewayCommand::Restart { host, port } => {
            let (resolved_host, resolved_port) =
                resolve_gateway_start_target(&gateway_service_path, host, port, "127.0.0.1", 8787)?;
            let service = upsert_gateway_service(
                &gateway_service_path,
                Some(resolved_host.clone()),
                Some(resolved_port),
                true,
            )?;
            let stop = stop_gateway_runtime(&gateway_path, false)?;
            let start =
                start_gateway_runtime(cli, &gateway_path, resolved_host, resolved_port).await?;
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "was_running": stop.was_running,
                    "stopped": stop.stopped,
                    "gateway": start.state,
                    "service": service,
                    "path": gateway_path.display().to_string(),
                }));
            } else {
                println!(
                    "gateway restarted (previous_running={} stopped={})",
                    stop.was_running, stop.stopped
                );
                println!("host: {}", start.state.host);
                println!("port: {}", start.state.port);
                println!("pid: {}", start.state.pid);
            }
        }
        GatewayCommand::Status { deep } => {
            let status =
                collect_gateway_runtime_status(&gateway_path, &gateway_service_path).await?;
            if cli.json {
                let mut payload = json!({
                    "ok": true,
                    "running": status.running,
                    "installed": status
                        .service
                        .as_ref()
                        .map(|service| service.installed)
                        .unwrap_or(false),
                    "gateway": status.state,
                    "service": status.service,
                    "path": gateway_path.display().to_string(),
                    "service_path": gateway_service_path.display().to_string(),
                });
                if deep {
                    payload["deep"] = json!({
                        "process_alive": status.process_alive,
                        "endpoint_healthy": status.endpoint_healthy,
                        "target_host": status.target_host,
                        "target_port": status.target_port,
                        "state_file_exists": gateway_path.exists(),
                        "service_file_exists": gateway_service_path.exists(),
                    });
                }
                print_json(&payload);
            } else if let Some(state) = status.state {
                println!(
                    "gateway: running={} installed={} host={} port={} pid={} updated={}",
                    status.running,
                    status
                        .service
                        .as_ref()
                        .map(|service| service.installed)
                        .unwrap_or(false),
                    state.host,
                    state.port,
                    state.pid,
                    state.updated_at.to_rfc3339()
                );
                if deep {
                    println!("process_alive: {}", status.process_alive);
                    println!("endpoint_healthy: {}", status.endpoint_healthy);
                    println!("target: {}:{}", status.target_host, status.target_port);
                }
            } else {
                println!(
                    "gateway: not running (installed={})",
                    status
                        .service
                        .as_ref()
                        .map(|service| service.installed)
                        .unwrap_or(false)
                );
                if deep {
                    println!("process_alive: {}", status.process_alive);
                    println!("endpoint_healthy: {}", status.endpoint_healthy);
                    println!("target: {}:{}", status.target_host, status.target_port);
                }
            }
        }
        GatewayCommand::Health { verbose, repair } => {
            let mut status =
                collect_gateway_runtime_status(&gateway_path, &gateway_service_path).await?;
            let mut repair_check: Option<(bool, String)> = None;
            if repair {
                if status.endpoint_healthy {
                    repair_check = Some((
                        true,
                        "auto-repair skipped: endpoint already healthy".to_string(),
                    ));
                } else {
                    match resolve_gateway_start_target(
                        &gateway_service_path,
                        None,
                        None,
                        "127.0.0.1",
                        8787,
                    ) {
                        Ok((target_host, target_port)) => {
                            if let Err(err) = upsert_gateway_service(
                                &gateway_service_path,
                                Some(target_host.clone()),
                                Some(target_port),
                                true,
                            ) {
                                repair_check = Some((false, format!("auto-repair failed: {err}")));
                            } else {
                                match start_gateway_runtime(
                                    cli,
                                    &gateway_path,
                                    target_host.clone(),
                                    target_port,
                                )
                                .await
                                {
                                    Ok(started) => {
                                        status = collect_gateway_runtime_status(
                                            &gateway_path,
                                            &gateway_service_path,
                                        )
                                        .await?;
                                        let repaired = status.endpoint_healthy;
                                        repair_check = Some((
                                            repaired,
                                            if repaired {
                                                format!(
                                                    "auto-repair attempted on {}:{} (already_running={}) and endpoint is healthy",
                                                    target_host,
                                                    target_port,
                                                    started.already_running
                                                )
                                            } else {
                                                format!(
                                                    "auto-repair attempted on {}:{} but endpoint is still unreachable",
                                                    target_host, target_port
                                                )
                                            },
                                        ));
                                    }
                                    Err(err) => {
                                        repair_check =
                                            Some((false, format!("auto-repair failed: {err}")));
                                    }
                                }
                            }
                        }
                        Err(err) => {
                            repair_check = Some((false, format!("auto-repair failed: {err}")));
                        }
                    }
                }
            }
            let installed = status
                .service
                .as_ref()
                .map(|service| service.installed)
                .unwrap_or(false);
            let mut checks = vec![
                run_check(
                    "gateway_service_file",
                    gateway_service_path.exists(),
                    "gateway service file",
                ),
                run_check(
                    "gateway_installed",
                    installed,
                    if installed {
                        "gateway service installed"
                    } else {
                        "gateway service not installed"
                    },
                ),
                run_check(
                    "gateway_state_file",
                    gateway_path.exists(),
                    "gateway runtime state file",
                ),
                run_check(
                    "gateway_process",
                    status.process_alive,
                    if status.process_alive {
                        "gateway process is alive"
                    } else {
                        "gateway process is not alive"
                    },
                ),
                run_check(
                    "gateway_endpoint",
                    status.endpoint_healthy,
                    if status.endpoint_healthy {
                        "GET /health reachable"
                    } else {
                        "GET /health unreachable"
                    },
                ),
            ];
            if gateway_test_mode() {
                let running = status.state.as_ref().is_some_and(|value| value.running);
                checks.push(run_check(
                    "gateway_discover",
                    running,
                    if running {
                        "test mode discover surface available"
                    } else {
                        "test mode runtime not running"
                    },
                ));
                checks.push(run_check(
                    "gateway_protocol_methods",
                    running,
                    if running {
                        "required methods available: health,status"
                    } else {
                        "required methods unknown (runtime not running)"
                    },
                ));
                checks.push(run_check(
                    "gateway_call_status",
                    running,
                    if running {
                        "test mode status method callable"
                    } else {
                        "status method check skipped (runtime not running)"
                    },
                ));
                checks.push(run_check(
                    "gateway_call_health",
                    running,
                    if running {
                        "test mode health method callable"
                    } else {
                        "health method check skipped (runtime not running)"
                    },
                ));
                checks.push(run_check(
                    "gateway_status_schema_profile",
                    running,
                    if running {
                        "status schema profile: basic"
                    } else {
                        "status schema check skipped (runtime not running)"
                    },
                ));
                checks.push(run_check(
                    "gateway_health_schema_profile",
                    running,
                    if running {
                        "health schema profile: basic"
                    } else {
                        "health schema check skipped (runtime not running)"
                    },
                ));
            } else if status.endpoint_healthy {
                match HttpGatewayClient::new(&status.target_host, status.target_port) {
                    Ok(client) => {
                        let discovery_result = client.discover().await;
                        match discovery_result {
                            Ok(discovery) => {
                                let method_count = discovery.methods.len();
                                let missing_required = ["health", "status"]
                                    .iter()
                                    .filter(|required| {
                                        !discovery.methods.iter().any(|value| value == **required)
                                    })
                                    .copied()
                                    .collect::<Vec<_>>();
                                checks.push(run_check(
                                    "gateway_discover",
                                    true,
                                    format!("discover endpoint reachable ({method_count} methods)"),
                                ));
                                checks.push(run_check(
                                    "gateway_protocol_methods",
                                    missing_required.is_empty(),
                                    if missing_required.is_empty() {
                                        "required methods available: health,status".to_string()
                                    } else {
                                        format!(
                                            "missing required methods: {}",
                                            missing_required.join(",")
                                        )
                                    },
                                ));
                            }
                            Err(err) => {
                                checks.push(run_check(
                                    "gateway_discover",
                                    false,
                                    format!("discover check failed: {err}"),
                                ));
                                checks.push(run_check(
                                    "gateway_protocol_methods",
                                    false,
                                    "required methods unknown (discover failed)",
                                ));
                            }
                        }

                        match client.call(GatewayRequest::new("status", None)).await {
                            Ok(response) => {
                                let result_payload = response.result.unwrap_or(Value::Null);
                                let has_data = !result_payload.is_null();
                                let (schema_ok, schema_detail) =
                                    validate_gateway_method_schema("status", &result_payload);
                                checks.push(run_check(
                                    "gateway_call_status",
                                    true,
                                    if has_data {
                                        "status method callable"
                                    } else {
                                        "status method callable (empty payload)"
                                    },
                                ));
                                checks.push(run_check(
                                    "gateway_status_schema_profile",
                                    schema_ok,
                                    schema_detail,
                                ));
                            }
                            Err(err) => {
                                checks.push(run_check(
                                    "gateway_call_status",
                                    false,
                                    format!("status call failed: {err}"),
                                ));
                                checks.push(run_check(
                                    "gateway_status_schema_profile",
                                    false,
                                    "status schema check skipped (status call failed)",
                                ));
                            }
                        }

                        match client.call(GatewayRequest::new("health", None)).await {
                            Ok(response) => {
                                let result_payload = response.result.unwrap_or(Value::Null);
                                let (schema_ok, schema_detail) =
                                    validate_gateway_method_schema("health", &result_payload);
                                checks.push(run_check(
                                    "gateway_call_health",
                                    true,
                                    "health method callable",
                                ));
                                checks.push(run_check(
                                    "gateway_health_schema_profile",
                                    schema_ok,
                                    schema_detail,
                                ));
                            }
                            Err(err) => {
                                checks.push(run_check(
                                    "gateway_call_health",
                                    false,
                                    format!("health call failed: {err}"),
                                ));
                                checks.push(run_check(
                                    "gateway_health_schema_profile",
                                    false,
                                    "health schema check skipped (health call failed)",
                                ));
                            }
                        }
                    }
                    Err(err) => {
                        checks.push(run_check(
                            "gateway_discover",
                            false,
                            format!("gateway client init failed: {err}"),
                        ));
                        checks.push(run_check(
                            "gateway_protocol_methods",
                            false,
                            "required methods unknown (gateway client init failed)",
                        ));
                        checks.push(run_check(
                            "gateway_call_status",
                            false,
                            "status method check skipped (gateway client init failed)",
                        ));
                        checks.push(run_check(
                            "gateway_call_health",
                            false,
                            "health method check skipped (gateway client init failed)",
                        ));
                        checks.push(run_check(
                            "gateway_status_schema_profile",
                            false,
                            "status schema check skipped (gateway client init failed)",
                        ));
                        checks.push(run_check(
                            "gateway_health_schema_profile",
                            false,
                            "health schema check skipped (gateway client init failed)",
                        ));
                    }
                }
            } else {
                checks.push(run_check(
                    "gateway_discover",
                    false,
                    "discover check skipped (endpoint unreachable)",
                ));
                checks.push(run_check(
                    "gateway_protocol_methods",
                    false,
                    "required methods unknown (endpoint unreachable)",
                ));
                checks.push(run_check(
                    "gateway_call_status",
                    false,
                    "status method check skipped (endpoint unreachable)",
                ));
                checks.push(run_check(
                    "gateway_call_health",
                    false,
                    "health method check skipped (endpoint unreachable)",
                ));
                checks.push(run_check(
                    "gateway_status_schema_profile",
                    false,
                    "status schema check skipped (endpoint unreachable)",
                ));
                checks.push(run_check(
                    "gateway_health_schema_profile",
                    false,
                    "health schema check skipped (endpoint unreachable)",
                ));
            }
            if let Some(state) = status.state {
                checks.push(run_check(
                    "gateway_target",
                    true,
                    format!("{}:{} (pid={})", state.host, state.port, state.pid),
                ));
            } else {
                checks.push(run_check(
                    "gateway_target",
                    installed,
                    format!("{}:{}", status.target_host, status.target_port),
                ));
            }
            if let Some((ok, detail)) = repair_check {
                checks.push(run_check("gateway_auto_repair", ok, detail));
            }
            if verbose {
                checks.push(run_check(
                    "gateway_runtime_running",
                    status.running,
                    format!("running={}", status.running),
                ));
            }
            emit_checks(cli.json, "gateway_health", checks)?;
            if verbose && !cli.json {
                println!(
                    "target endpoint: http://{}:{}",
                    status.target_host, status.target_port
                );
            }
        }
        GatewayCommand::Call { method, params } => {
            let started = Instant::now();
            if gateway_test_mode() {
                let state: Option<GatewayState> = load_json_file_opt(&gateway_path)?;
                if !state.as_ref().is_some_and(|value| value.running) {
                    let err = MosaicError::GatewayUnavailable(
                        "gateway is not running in test mode".to_string(),
                    );
                    write_gateway_event(
                        &gateway_events_path,
                        GatewayEventInput {
                            action: "call",
                            method: Some(method.as_str()),
                            host: "127.0.0.1",
                            port: 8787,
                            mode: "test_mode",
                            success: false,
                            latency_ms: started.elapsed().as_millis(),
                            error_code: Some(err.code().to_string()),
                            error_message: Some(err.to_string()),
                        },
                    );
                    return Err(err);
                }
                let params = params
                    .as_deref()
                    .map(|value| parse_json_input(value, "gateway params"))
                    .transpose()?
                    .unwrap_or(Value::Null);
                let data = match method.as_str() {
                    "status" => json!({
                        "ok": true,
                        "service": "mosaic-gateway",
                        "test_mode": true,
                    }),
                    "health" => json!({
                        "ok": true,
                        "service": "mosaic-gateway",
                        "test_mode": true,
                    }),
                    "echo" => json!({
                        "ok": true,
                        "echo": params,
                        "test_mode": true,
                    }),
                    "nodes.run" => json!({
                        "ok": true,
                        "status": "accepted",
                        "mode": "test_mode",
                        "node_id": params.get("node_id").cloned().unwrap_or(Value::Null),
                        "command": params.get("command").cloned().unwrap_or(Value::Null),
                    }),
                    "nodes.invoke" => json!({
                        "ok": true,
                        "status": "accepted",
                        "mode": "test_mode",
                        "node_id": params.get("node_id").cloned().unwrap_or(Value::Null),
                        "method": params.get("method").cloned().unwrap_or(Value::Null),
                        "params": params.get("params").cloned().unwrap_or(Value::Null),
                    }),
                    _ => {
                        let err = MosaicError::GatewayProtocol(format!(
                            "unknown gateway method '{}' in test mode",
                            method
                        ));
                        write_gateway_event(
                            &gateway_events_path,
                            GatewayEventInput {
                                action: "call",
                                method: Some(method.as_str()),
                                host: "127.0.0.1",
                                port: 8787,
                                mode: "test_mode",
                                success: false,
                                latency_ms: started.elapsed().as_millis(),
                                error_code: Some(err.code().to_string()),
                                error_message: Some(err.to_string()),
                            },
                        );
                        return Err(err);
                    }
                };
                write_gateway_event(
                    &gateway_events_path,
                    GatewayEventInput {
                        action: "call",
                        method: Some(method.as_str()),
                        host: "127.0.0.1",
                        port: 8787,
                        mode: "test_mode",
                        success: true,
                        latency_ms: started.elapsed().as_millis(),
                        error_code: None,
                        error_message: None,
                    },
                );
                if cli.json {
                    print_json(&json!({
                        "ok": true,
                        "request_id": "gateway-test-mode",
                        "method": method,
                        "data": data,
                        "gateway": { "host": "127.0.0.1", "port": 8787 },
                    }));
                } else {
                    println!("gateway method: {method}");
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&data).unwrap_or_default()
                    );
                }
                return Ok(());
            }

            let (host, port) = resolve_gateway_target(&gateway_path, &gateway_service_path)?;
            let client = HttpGatewayClient::new(&host, port)?;
            let params = params
                .as_deref()
                .map(|value| parse_json_input(value, "gateway params"))
                .transpose()?;
            let request = GatewayRequest::new(method.clone(), params);
            let request_id = request.id.clone();
            let response = match client.call(request).await {
                Ok(value) => {
                    write_gateway_event(
                        &gateway_events_path,
                        GatewayEventInput {
                            action: "call",
                            method: Some(method.as_str()),
                            host: host.as_str(),
                            port,
                            mode: "http",
                            success: true,
                            latency_ms: started.elapsed().as_millis(),
                            error_code: None,
                            error_message: None,
                        },
                    );
                    value
                }
                Err(err) => {
                    write_gateway_event(
                        &gateway_events_path,
                        GatewayEventInput {
                            action: "call",
                            method: Some(method.as_str()),
                            host: host.as_str(),
                            port,
                            mode: "http",
                            success: false,
                            latency_ms: started.elapsed().as_millis(),
                            error_code: Some(err.code().to_string()),
                            error_message: Some(err.to_string()),
                        },
                    );
                    return Err(err);
                }
            };
            let result = response.result.unwrap_or(Value::Null);
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "request_id": request_id,
                    "method": method,
                    "data": result,
                    "gateway": {
                        "host": host,
                        "port": port,
                    }
                }));
            } else {
                println!("gateway method: {method}");
                println!("request id: {request_id}");
                println!(
                    "{}",
                    serde_json::to_string_pretty(&result).unwrap_or_else(|_| "{}".to_string())
                );
            }
        }
        GatewayCommand::Probe => {
            let started = Instant::now();
            if gateway_test_mode() {
                let state: Option<GatewayState> = load_json_file_opt(&gateway_path)?;
                if !state.as_ref().is_some_and(|value| value.running) {
                    let err = MosaicError::GatewayUnavailable(
                        "gateway is not running in test mode".to_string(),
                    );
                    write_gateway_event(
                        &gateway_events_path,
                        GatewayEventInput {
                            action: "probe",
                            method: None,
                            host: "127.0.0.1",
                            port: 8787,
                            mode: "test_mode",
                            success: false,
                            latency_ms: started.elapsed().as_millis(),
                            error_code: Some(err.code().to_string()),
                            error_message: Some(err.to_string()),
                        },
                    );
                    return Err(err);
                }
                write_gateway_event(
                    &gateway_events_path,
                    GatewayEventInput {
                        action: "probe",
                        method: None,
                        host: "127.0.0.1",
                        port: 8787,
                        mode: "test_mode",
                        success: true,
                        latency_ms: started.elapsed().as_millis(),
                        error_code: None,
                        error_message: None,
                    },
                );
                if cli.json {
                    print_json(&json!({
                        "ok": true,
                        "probe": {
                            "ok": true,
                            "endpoint": "test-mode://gateway/health",
                            "latency_ms": 0,
                            "detail": "gateway test mode",
                        },
                        "gateway": { "host": "127.0.0.1", "port": 8787 },
                    }));
                } else {
                    println!("gateway probe ok (test mode)");
                }
                return Ok(());
            }

            let (host, port) = resolve_gateway_target(&gateway_path, &gateway_service_path)?;
            let client = HttpGatewayClient::new(&host, port)?;
            let probe = match client.probe().await {
                Ok(value) => {
                    write_gateway_event(
                        &gateway_events_path,
                        GatewayEventInput {
                            action: "probe",
                            method: None,
                            host: host.as_str(),
                            port,
                            mode: "http",
                            success: true,
                            latency_ms: started.elapsed().as_millis(),
                            error_code: None,
                            error_message: None,
                        },
                    );
                    value
                }
                Err(err) => {
                    write_gateway_event(
                        &gateway_events_path,
                        GatewayEventInput {
                            action: "probe",
                            method: None,
                            host: host.as_str(),
                            port,
                            mode: "http",
                            success: false,
                            latency_ms: started.elapsed().as_millis(),
                            error_code: Some(err.code().to_string()),
                            error_message: Some(err.to_string()),
                        },
                    );
                    return Err(err);
                }
            };
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "probe": probe,
                    "gateway": {
                        "host": host,
                        "port": port,
                    }
                }));
            } else {
                println!("gateway probe ok");
                println!("endpoint: {}", probe.endpoint);
                println!("latency: {}ms", probe.latency_ms);
                println!("detail: {}", probe.detail);
            }
        }
        GatewayCommand::Discover => {
            let started = Instant::now();
            if gateway_test_mode() {
                let methods = vec!["health", "status", "echo", "nodes.run", "nodes.invoke"];
                write_gateway_event(
                    &gateway_events_path,
                    GatewayEventInput {
                        action: "discover",
                        method: None,
                        host: "127.0.0.1",
                        port: 8787,
                        mode: "test_mode",
                        success: true,
                        latency_ms: started.elapsed().as_millis(),
                        error_code: None,
                        error_message: None,
                    },
                );
                if cli.json {
                    print_json(&json!({
                        "ok": true,
                        "discovery": {
                            "ok": true,
                            "endpoint": "test-mode://gateway/discover",
                            "methods": methods,
                        },
                        "gateway": { "host": "127.0.0.1", "port": 8787 },
                    }));
                } else {
                    println!("gateway methods:");
                    println!("- health");
                    println!("- status");
                    println!("- echo");
                    println!("- nodes.run");
                    println!("- nodes.invoke");
                }
                return Ok(());
            }

            let (host, port) = resolve_gateway_target(&gateway_path, &gateway_service_path)?;
            let client = HttpGatewayClient::new(&host, port)?;
            let discovery = match client.discover().await {
                Ok(value) => {
                    write_gateway_event(
                        &gateway_events_path,
                        GatewayEventInput {
                            action: "discover",
                            method: None,
                            host: host.as_str(),
                            port,
                            mode: "http",
                            success: true,
                            latency_ms: started.elapsed().as_millis(),
                            error_code: None,
                            error_message: None,
                        },
                    );
                    value
                }
                Err(err) => {
                    write_gateway_event(
                        &gateway_events_path,
                        GatewayEventInput {
                            action: "discover",
                            method: None,
                            host: host.as_str(),
                            port,
                            mode: "http",
                            success: false,
                            latency_ms: started.elapsed().as_millis(),
                            error_code: Some(err.code().to_string()),
                            error_message: Some(err.to_string()),
                        },
                    );
                    return Err(err);
                }
            };
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "discovery": discovery,
                    "gateway": {
                        "host": host,
                        "port": port,
                    }
                }));
            } else if discovery.methods.is_empty() {
                println!("gateway methods: <none>");
            } else {
                println!("gateway methods:");
                for method in discovery.methods {
                    println!("- {method}");
                }
            }
        }
        GatewayCommand::Diagnose { method, params } => {
            let started = Instant::now();
            let requested_method = method.unwrap_or_else(|| "status".to_string());
            let parsed_params = params
                .as_deref()
                .map(|value| parse_json_input(value, "gateway diagnose params"))
                .transpose()?;

            if gateway_test_mode() {
                let state: Option<GatewayState> = load_json_file_opt(&gateway_path)?;
                if !state.as_ref().is_some_and(|value| value.running) {
                    let err = MosaicError::GatewayUnavailable(
                        "gateway is not running in test mode".to_string(),
                    );
                    write_gateway_event(
                        &gateway_events_path,
                        GatewayEventInput {
                            action: "diagnose",
                            method: Some(requested_method.as_str()),
                            host: "127.0.0.1",
                            port: 8787,
                            mode: "test_mode",
                            success: false,
                            latency_ms: started.elapsed().as_millis(),
                            error_code: Some(err.code().to_string()),
                            error_message: Some(err.to_string()),
                        },
                    );
                    return Err(err);
                }

                let steps = vec![
                    json!({
                        "name": "probe",
                        "ok": true,
                        "latency_ms": 0u128,
                        "detail": "gateway test mode probe",
                        "error_code": Value::Null,
                    }),
                    json!({
                        "name": "discover",
                        "ok": true,
                        "latency_ms": 0u128,
                        "detail": "gateway test mode discover",
                        "error_code": Value::Null,
                    }),
                    json!({
                        "name": "call",
                        "ok": true,
                        "latency_ms": 0u128,
                        "detail": format!("gateway test mode call: {}", requested_method),
                        "error_code": Value::Null,
                    }),
                ];
                write_gateway_event(
                    &gateway_events_path,
                    GatewayEventInput {
                        action: "diagnose",
                        method: Some(requested_method.as_str()),
                        host: "127.0.0.1",
                        port: 8787,
                        mode: "test_mode",
                        success: true,
                        latency_ms: started.elapsed().as_millis(),
                        error_code: None,
                        error_message: None,
                    },
                );
                if cli.json {
                    print_json(&json!({
                        "ok": true,
                        "diagnose": {
                            "target": { "host": "127.0.0.1", "port": 8787 },
                            "method": requested_method,
                            "params": parsed_params,
                            "steps": steps,
                            "summary": {
                                "total": 3,
                                "passed": 3,
                                "failed": 0,
                            }
                        }
                    }));
                } else {
                    println!("gateway diagnose (test mode)");
                    println!("target: 127.0.0.1:8787");
                    println!("method: {}", requested_method);
                    println!("steps: total=3 passed=3 failed=0");
                    println!("- probe: ok");
                    println!("- discover: ok");
                    println!("- call: ok");
                }
                return Ok(());
            }

            let (host, port) = resolve_gateway_target(&gateway_path, &gateway_service_path)?;
            let client = HttpGatewayClient::new(&host, port)?;
            let mut steps = Vec::new();
            let mut passed = 0usize;
            let mut failed = 0usize;

            let probe_started = std::time::Instant::now();
            match client.probe().await {
                Ok(probe) => {
                    passed += 1;
                    steps.push(json!({
                        "name": "probe",
                        "ok": true,
                        "latency_ms": probe_started.elapsed().as_millis(),
                        "detail": probe.detail,
                        "error_code": Value::Null,
                    }));
                }
                Err(err) => {
                    failed += 1;
                    let code = err.code().to_string();
                    steps.push(json!({
                        "name": "probe",
                        "ok": false,
                        "latency_ms": probe_started.elapsed().as_millis(),
                        "detail": err.to_string(),
                        "error_code": code,
                    }));
                }
            }

            let discover_started = std::time::Instant::now();
            match client.discover().await {
                Ok(discovery) => {
                    passed += 1;
                    steps.push(json!({
                        "name": "discover",
                        "ok": true,
                        "latency_ms": discover_started.elapsed().as_millis(),
                        "detail": format!("methods={}", discovery.methods.len()),
                        "methods": discovery.methods,
                        "error_code": Value::Null,
                    }));
                }
                Err(err) => {
                    failed += 1;
                    let code = err.code().to_string();
                    steps.push(json!({
                        "name": "discover",
                        "ok": false,
                        "latency_ms": discover_started.elapsed().as_millis(),
                        "detail": err.to_string(),
                        "error_code": code,
                    }));
                }
            }

            let call_started = std::time::Instant::now();
            let request = GatewayRequest::new(requested_method.clone(), parsed_params.clone());
            match client.call(request).await {
                Ok(response) => {
                    passed += 1;
                    steps.push(json!({
                        "name": "call",
                        "ok": true,
                        "latency_ms": call_started.elapsed().as_millis(),
                        "detail": format!("method '{}' callable", requested_method),
                        "result_present": response.result.is_some(),
                        "error_code": Value::Null,
                    }));
                }
                Err(err) => {
                    failed += 1;
                    let code = err.code().to_string();
                    steps.push(json!({
                        "name": "call",
                        "ok": false,
                        "latency_ms": call_started.elapsed().as_millis(),
                        "detail": err.to_string(),
                        "error_code": code,
                    }));
                }
            }
            let summary_error = if failed > 0 {
                Some(format!(
                    "diagnose failed steps={failed} (method={requested_method})"
                ))
            } else {
                None
            };
            write_gateway_event(
                &gateway_events_path,
                GatewayEventInput {
                    action: "diagnose",
                    method: Some(requested_method.as_str()),
                    host: host.as_str(),
                    port,
                    mode: "http",
                    success: failed == 0,
                    latency_ms: started.elapsed().as_millis(),
                    error_code: if failed > 0 {
                        Some("gateway_diagnose_failed".to_string())
                    } else {
                        None
                    },
                    error_message: summary_error,
                },
            );

            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "diagnose": {
                        "target": { "host": host, "port": port },
                        "method": requested_method,
                        "params": parsed_params,
                        "steps": steps,
                        "summary": {
                            "total": passed + failed,
                            "passed": passed,
                            "failed": failed,
                        }
                    }
                }));
            } else {
                println!("gateway diagnose");
                println!("target: {}:{}", host, port);
                println!("method: {}", requested_method);
                println!(
                    "steps: total={} passed={} failed={}",
                    passed + failed,
                    passed,
                    failed
                );
                for step in steps {
                    println!(
                        "- {}: {} ({})",
                        step["name"].as_str().unwrap_or("-"),
                        if step["ok"].as_bool().unwrap_or(false) {
                            "ok"
                        } else {
                            "failed"
                        },
                        step["detail"].as_str().unwrap_or("-")
                    );
                }
            }
        }
        GatewayCommand::Stop => {
            let stop = stop_gateway_runtime(&gateway_path, true)?;
            let next = stop.state.ok_or_else(|| {
                MosaicError::Config("gateway state file not found; not running".to_string())
            })?;
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "was_running": stop.was_running,
                    "stopped": stop.stopped,
                    "gateway": next,
                }));
            } else if stop.was_running {
                println!(
                    "Gateway {} (pid={})",
                    if stop.stopped {
                        "stopped"
                    } else {
                        "stop signal sent"
                    },
                    next.pid
                );
            } else {
                println!("Gateway process was not running.");
            }
        }
        GatewayCommand::Uninstall => {
            let stop = stop_gateway_runtime(&gateway_path, false)?;
            let removed_state_file = if gateway_path.exists() {
                std::fs::remove_file(&gateway_path)?;
                true
            } else {
                false
            };
            let removed_service_file = if gateway_service_path.exists() {
                std::fs::remove_file(&gateway_service_path)?;
                true
            } else {
                false
            };
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "was_running": stop.was_running,
                    "stopped": stop.stopped,
                    "removed_state_file": removed_state_file,
                    "removed_service_file": removed_service_file,
                }));
            } else {
                println!(
                    "gateway uninstalled (was_running={} stopped={} removed_state={} removed_service={})",
                    stop.was_running, stop.stopped, removed_state_file, removed_service_file
                );
            }
        }
        GatewayCommand::Serve { host, port } => {
            run_gateway_http_server(&host, port)?;
        }
    }
    Ok(())
}

fn validate_gateway_method_schema(method: &str, payload: &Value) -> (bool, String) {
    let Some(obj) = payload.as_object() else {
        return (
            false,
            format!("{method} schema profile failed: result is not an object"),
        );
    };
    match obj.get("ok") {
        Some(Value::Bool(_)) => {}
        _ => {
            return (
                false,
                format!("{method} schema profile failed: missing boolean field 'ok'"),
            );
        }
    }
    if method == "status" {
        let has_status_signal = ["service", "status", "gateway", "runtime"]
            .iter()
            .any(|key| obj.contains_key(*key));
        if !has_status_signal {
            return (
                false,
                "status schema profile failed: missing status payload key (service/status/gateway/runtime)"
                    .to_string(),
            );
        }
    }
    (true, format!("{method} schema profile: basic"))
}

#[derive(Debug)]
struct GatewayEventInput<'a> {
    action: &'a str,
    method: Option<&'a str>,
    host: &'a str,
    port: u16,
    mode: &'a str,
    success: bool,
    latency_ms: u128,
    error_code: Option<String>,
    error_message: Option<String>,
}

#[derive(Debug, Serialize)]
struct GatewayEventRecord {
    id: String,
    ts: String,
    action: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    method: Option<String>,
    target_host: String,
    target_port: u16,
    mode: String,
    success: bool,
    latency_ms: u128,
    #[serde(skip_serializing_if = "Option::is_none")]
    error_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error_message: Option<String>,
}

fn write_gateway_event(path: &Path, input: GatewayEventInput<'_>) {
    let record = GatewayEventRecord {
        id: Uuid::new_v4().to_string(),
        ts: Utc::now().to_rfc3339(),
        action: input.action.to_string(),
        method: input.method.map(ToString::to_string),
        target_host: input.host.to_string(),
        target_port: input.port,
        mode: input.mode.to_string(),
        success: input.success,
        latency_ms: input.latency_ms,
        error_code: input.error_code,
        error_message: input.error_message,
    };
    let _ = append_jsonl(path, &record);
}

fn append_jsonl<T: Serialize>(path: &Path, record: &T) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    let line = serde_json::to_string(record)
        .map_err(|err| MosaicError::Validation(format!("failed to encode gateway event: {err}")))?;
    file.write_all(line.as_bytes())?;
    file.write_all(b"\n")?;
    Ok(())
}
