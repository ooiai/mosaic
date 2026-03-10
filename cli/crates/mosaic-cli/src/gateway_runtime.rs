use std::net::{TcpStream, ToSocketAddrs};
use std::process::{Command, Stdio};
use std::time::Duration;

use chrono::Utc;
use serde_json::{Value, json};
use tiny_http::{Method, Response, Server};

use mosaic_core::error::MosaicError;
use mosaic_gateway::{GatewayClient, GatewayRequest, HttpGatewayClient};

use crate::utils::{load_json_file_opt, save_state_json_file};

use super::{
    Cli, DEFAULT_PROFILE, GatewayRuntimeStatus, GatewayServiceState, GatewayStartResult,
    GatewayState, GatewayStopResult, Result,
};

pub(super) fn upsert_gateway_service(
    service_path: &std::path::Path,
    host: Option<String>,
    port: Option<u16>,
    installed: bool,
) -> Result<GatewayServiceState> {
    let existing: Option<GatewayServiceState> = load_json_file_opt(service_path)?;
    let now = Utc::now();
    let service = GatewayServiceState {
        installed,
        host: host
            .or_else(|| existing.as_ref().map(|item| item.host.clone()))
            .unwrap_or_else(|| "127.0.0.1".to_string()),
        port: port
            .or_else(|| existing.as_ref().map(|item| item.port))
            .unwrap_or(8787),
        installed_at: existing
            .as_ref()
            .map(|item| item.installed_at)
            .unwrap_or(now),
        updated_at: now,
    };
    save_state_json_file(service_path, &service, "gateway service state")?;
    Ok(service)
}

pub(super) fn resolve_gateway_start_target(
    service_path: &std::path::Path,
    host: Option<String>,
    port: Option<u16>,
    default_host: &str,
    default_port: u16,
) -> Result<(String, u16)> {
    let service: Option<GatewayServiceState> = load_json_file_opt(service_path)?;
    let resolved_host = host
        .or_else(|| service.as_ref().map(|item| item.host.clone()))
        .unwrap_or_else(|| default_host.to_string());
    let resolved_port = port
        .or_else(|| service.as_ref().map(|item| item.port))
        .unwrap_or(default_port);
    Ok((resolved_host, resolved_port))
}

pub(super) async fn start_gateway_runtime(
    cli: &Cli,
    gateway_path: &std::path::Path,
    host: String,
    port: u16,
) -> Result<GatewayStartResult> {
    if let Some(existing) = load_json_file_opt::<GatewayState>(gateway_path)? {
        let alive = if gateway_test_mode() {
            existing.running
        } else {
            is_process_alive(existing.pid)
                && probe_gateway_health(&existing.host, existing.port).await
        };
        if alive {
            return Ok(GatewayStartResult {
                state: existing,
                already_running: true,
            });
        }
    }

    let pid = if gateway_test_mode() {
        0
    } else {
        spawn_gateway_process(cli, &host, port)?
    };
    if !gateway_test_mode() {
        let mut ready = false;
        for _ in 0..40 {
            if probe_gateway_health(&host, port).await {
                ready = true;
                break;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        if !ready {
            return Err(MosaicError::Network(format!(
                "gateway did not become healthy at http://{host}:{port}/health"
            )));
        }
    }
    let now = Utc::now();
    let state = GatewayState {
        running: true,
        host,
        port,
        pid,
        started_at: now,
        updated_at: now,
    };
    save_state_json_file(gateway_path, &state, "gateway runtime state")?;
    Ok(GatewayStartResult {
        state,
        already_running: false,
    })
}

pub(super) fn stop_gateway_runtime(
    gateway_path: &std::path::Path,
    required: bool,
) -> Result<GatewayStopResult> {
    let Some(state) = load_json_file_opt::<GatewayState>(gateway_path)? else {
        if required {
            return Err(MosaicError::Config(
                "gateway state file not found; not running".to_string(),
            ));
        }
        return Ok(GatewayStopResult {
            was_running: false,
            stopped: false,
            state: None,
        });
    };

    let was_alive = if gateway_test_mode() {
        state.running
    } else {
        is_process_alive(state.pid)
    };
    let stopped = if was_alive {
        if gateway_test_mode() {
            true
        } else {
            stop_process(state.pid)?
        }
    } else {
        false
    };

    let next = GatewayState {
        running: false,
        host: state.host,
        port: state.port,
        pid: state.pid,
        started_at: state.started_at,
        updated_at: Utc::now(),
    };
    save_state_json_file(gateway_path, &next, "gateway runtime state")?;
    Ok(GatewayStopResult {
        was_running: was_alive,
        stopped: stopped || !was_alive,
        state: Some(next),
    })
}

pub(super) async fn collect_gateway_runtime_status(
    gateway_path: &std::path::Path,
    gateway_service_path: &std::path::Path,
) -> Result<GatewayRuntimeStatus> {
    let state: Option<GatewayState> = load_json_file_opt(gateway_path)?;
    let service: Option<GatewayServiceState> = load_json_file_opt(gateway_service_path)?;
    let process_alive = state.as_ref().is_some_and(|value| {
        if gateway_test_mode() {
            value.running
        } else {
            is_process_alive(value.pid)
        }
    });
    let endpoint_healthy = if let Some(value) = &state {
        if gateway_test_mode() {
            value.running
        } else {
            probe_gateway_health(&value.host, value.port).await
        }
    } else if let Some(value) = &service {
        if gateway_test_mode() {
            false
        } else {
            probe_gateway_health(&value.host, value.port).await
        }
    } else {
        false
    };
    let running = match &state {
        Some(value) => {
            if gateway_test_mode() {
                value.running
            } else {
                process_alive && endpoint_healthy
            }
        }
        None => false,
    };
    let (target_host, target_port) = resolve_gateway_target(gateway_path, gateway_service_path)?;
    Ok(GatewayRuntimeStatus {
        running,
        process_alive,
        endpoint_healthy,
        state,
        service,
        target_host,
        target_port,
    })
}

#[derive(Debug, Clone)]
pub(super) struct GatewayCallDispatch {
    pub(super) request_id: String,
    pub(super) host: String,
    pub(super) port: u16,
    pub(super) result: Value,
}

pub(super) async fn dispatch_gateway_call(
    gateway_path: &std::path::Path,
    gateway_service_path: &std::path::Path,
    method: &str,
    params: Value,
) -> Result<GatewayCallDispatch> {
    if gateway_test_mode() {
        let state: GatewayState = load_json_file_opt(gateway_path)?.ok_or_else(|| {
            MosaicError::GatewayUnavailable("gateway is not running in test mode".to_string())
        })?;
        if !state.running {
            return Err(MosaicError::GatewayUnavailable(
                "gateway is not running in test mode".to_string(),
            ));
        }
        let result = match method {
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
                return Err(MosaicError::GatewayProtocol(format!(
                    "gateway test mode does not support method '{}'",
                    method
                )));
            }
        };
        return Ok(GatewayCallDispatch {
            request_id: "gateway-test-mode".to_string(),
            host: state.host,
            port: state.port,
            result,
        });
    }

    let (host, port) = resolve_gateway_target(gateway_path, gateway_service_path)?;
    let client = HttpGatewayClient::new(&host, port)?;
    let request = GatewayRequest::new(method.to_string(), Some(params));
    let request_id = request.id.clone();
    let response = client.call(request).await?;
    Ok(GatewayCallDispatch {
        request_id,
        host,
        port,
        result: response.result.unwrap_or(Value::Null),
    })
}

fn spawn_gateway_process(cli: &Cli, host: &str, port: u16) -> Result<u32> {
    let exe = std::env::current_exe().map_err(|err| {
        MosaicError::Io(format!("failed to resolve current executable path: {err}"))
    })?;
    let mut cmd = Command::new(exe);
    if !cli.debug {
        cmd.stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
    }
    if cli.project_state {
        cmd.arg("--project-state");
    }
    if cli.debug {
        cmd.arg("--debug");
    }
    if cli.profile != DEFAULT_PROFILE {
        cmd.arg("--profile").arg(&cli.profile);
    }
    cmd.arg("gateway")
        .arg("serve")
        .arg("--host")
        .arg(host)
        .arg("--port")
        .arg(port.to_string());
    let child = cmd
        .spawn()
        .map_err(|err| MosaicError::Io(format!("failed to spawn gateway process: {err}")))?;
    Ok(child.id())
}

pub(super) fn gateway_test_mode() -> bool {
    std::env::var("MOSAIC_GATEWAY_TEST_MODE").ok().as_deref() == Some("1")
}

fn is_process_alive(pid: u32) -> bool {
    #[cfg(unix)]
    {
        match Command::new("kill")
            .arg("-0")
            .arg(pid.to_string())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
        {
            Ok(status) => status.success(),
            Err(_) => false,
        }
    }
    #[cfg(windows)]
    {
        match Command::new("tasklist")
            .arg("/FI")
            .arg(format!("PID eq {}", pid))
            .output()
        {
            Ok(output) => String::from_utf8_lossy(&output.stdout).contains(&pid.to_string()),
            Err(_) => false,
        }
    }
}

fn stop_process(pid: u32) -> Result<bool> {
    #[cfg(unix)]
    {
        let status = Command::new("kill")
            .arg(pid.to_string())
            .status()
            .map_err(|err| MosaicError::Io(format!("failed to issue kill for pid {pid}: {err}")))?;
        if !status.success() {
            return Ok(false);
        }
        for _ in 0..30 {
            if !is_process_alive(pid) {
                return Ok(true);
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        Ok(false)
    }
    #[cfg(windows)]
    {
        let status = Command::new("taskkill")
            .arg("/PID")
            .arg(pid.to_string())
            .arg("/F")
            .status()
            .map_err(|err| {
                MosaicError::Io(format!("failed to issue taskkill for pid {pid}: {err}"))
            })?;
        Ok(status.success())
    }
}

async fn probe_gateway_health(host: &str, port: u16) -> bool {
    let address = format!("{host}:{port}");
    let mut addrs = match address.to_socket_addrs() {
        Ok(values) => values,
        Err(_) => return false,
    };
    let Some(first_addr) = addrs.next() else {
        return false;
    };
    if TcpStream::connect_timeout(&first_addr, Duration::from_millis(250)).is_err() {
        return false;
    }
    let url = format!("http://{host}:{port}/health");
    match reqwest::Client::builder()
        .timeout(Duration::from_secs(2))
        .build()
    {
        Ok(client) => match client.get(url).send().await {
            Ok(response) => response.status().is_success(),
            Err(_) => false,
        },
        Err(_) => false,
    }
}

pub(super) fn run_gateway_http_server(host: &str, port: u16) -> Result<()> {
    let server = Server::http(format!("{host}:{port}"))
        .map_err(|err| MosaicError::Network(format!("failed to bind gateway server: {err}")))?;
    let started_at = Utc::now();
    for mut request in server.incoming_requests() {
        let method = request.method().clone();
        let url = request.url().to_string();
        let response = match (method, url.as_str()) {
            (Method::Get, "/health") => Response::from_string(
                json!({
                    "ok": true,
                    "service": "mosaic-gateway",
                    "ts": Utc::now(),
                })
                .to_string(),
            ),
            (Method::Get, "/status") => Response::from_string(
                json!({
                    "ok": true,
                    "service": "mosaic-gateway",
                    "started_at": started_at,
                    "uptime_seconds": (Utc::now() - started_at).num_seconds(),
                })
                .to_string(),
            ),
            (Method::Get, "/discover") => Response::from_string(
                json!({
                    "ok": true,
                    "methods": ["health", "status", "echo", "nodes.run", "nodes.invoke"],
                })
                .to_string(),
            ),
            (Method::Post, "/call") => {
                let mut body = String::new();
                if request.as_reader().read_to_string(&mut body).is_err() {
                    Response::from_string(
                        json!({
                            "ok": false,
                            "error": {
                                "code": "invalid_request",
                                "message": "failed to read request body",
                            }
                        })
                        .to_string(),
                    )
                    .with_status_code(400)
                } else {
                    let parsed = serde_json::from_str::<GatewayRequest>(&body);
                    match parsed {
                        Ok(payload) => match payload.method.as_str() {
                            "health" => Response::from_string(
                                json!({
                                    "ok": true,
                                    "result": {
                                        "ok": true,
                                        "service": "mosaic-gateway",
                                        "ts": Utc::now(),
                                    }
                                })
                                .to_string(),
                            ),
                            "status" => Response::from_string(
                                json!({
                                    "ok": true,
                                    "result": {
                                        "ok": true,
                                        "service": "mosaic-gateway",
                                        "started_at": started_at,
                                        "uptime_seconds": (Utc::now() - started_at).num_seconds(),
                                    }
                                })
                                .to_string(),
                            ),
                            "echo" => Response::from_string(
                                json!({
                                    "ok": true,
                                    "result": {
                                        "ok": true,
                                        "echo": payload.params,
                                    }
                                })
                                .to_string(),
                            ),
                            "nodes.run" => Response::from_string(
                                json!({
                                    "ok": true,
                                    "result": {
                                        "ok": true,
                                        "status": "accepted",
                                        "node_id": payload.params.get("node_id").cloned().unwrap_or(Value::Null),
                                        "command": payload.params.get("command").cloned().unwrap_or(Value::Null),
                                    }
                                })
                                .to_string(),
                            ),
                            "nodes.invoke" => Response::from_string(
                                json!({
                                    "ok": true,
                                    "result": {
                                        "ok": true,
                                        "status": "accepted",
                                        "node_id": payload.params.get("node_id").cloned().unwrap_or(Value::Null),
                                        "method": payload.params.get("method").cloned().unwrap_or(Value::Null),
                                        "params": payload.params.get("params").cloned().unwrap_or(Value::Null),
                                    }
                                })
                                .to_string(),
                            ),
                            _ => Response::from_string(
                                json!({
                                    "ok": false,
                                    "error": {
                                        "code": "method_not_found",
                                        "message": format!("unknown method '{}'", payload.method),
                                    }
                                })
                                .to_string(),
                            )
                            .with_status_code(404),
                        },
                        Err(err) => Response::from_string(
                            json!({
                                "ok": false,
                                "error": {
                                    "code": "invalid_request",
                                    "message": format!("invalid JSON request: {err}"),
                                }
                            })
                            .to_string(),
                        )
                        .with_status_code(400),
                    }
                }
            }
            _ => Response::from_string(
                json!({
                    "ok": false,
                    "error": "not_found",
                })
                .to_string(),
            )
            .with_status_code(404),
        };
        let response = response.with_header(
            tiny_http::Header::from_bytes("Content-Type", "application/json").map_err(|err| {
                MosaicError::Unknown(format!("failed to create response header: {err:?}"))
            })?,
        );
        let _ = request.respond(response);
    }
    Ok(())
}

pub(super) fn resolve_gateway_target(
    gateway_path: &std::path::Path,
    gateway_service_path: &std::path::Path,
) -> Result<(String, u16)> {
    let state: Option<GatewayState> = load_json_file_opt(gateway_path)?;
    if let Some(state) = state {
        return Ok((state.host, state.port));
    }
    let service: Option<GatewayServiceState> = load_json_file_opt(gateway_service_path)?;
    if let Some(service) = service {
        return Ok((service.host, service.port));
    }
    Ok(("127.0.0.1".to_string(), 8787))
}
