use std::fs;

use anyhow::{Result, anyhow, bail};

use crate::{
    GatewayCliCommand, build_gateway_handle, ensure_loaded_config, finish_gateway_outcome,
    gateway_client_from_loaded, gateway_event_label, print_gateway_audit_events,
    print_gateway_incident_bundle, print_gateway_replay_window, print_gateway_status,
    print_remote_session_list, print_run_detail, print_run_list, save_incident_bundle,
};

pub(crate) async fn gateway_cmd(attach: Option<String>, command: GatewayCliCommand) -> Result<()> {
    let loaded = ensure_loaded_config(None)?;

    if let Some(url) = attach {
        let client = gateway_client_from_loaded(&loaded, url);
        return match command {
            GatewayCliCommand::Serve { .. } => {
                bail!("remote attach does not support `mosaic gateway serve`")
            }
            GatewayCliCommand::Sessions => {
                print_remote_session_list(&client.list_sessions().await?)
            }
            GatewayCliCommand::Runs => print_run_list(&client.list_runs().await?),
            GatewayCliCommand::ShowRun { id } => {
                let run = client
                    .get_run(&id)
                    .await?
                    .ok_or_else(|| anyhow!("run not found: {}", id))?;
                print_run_detail(&run)
            }
            GatewayCliCommand::Cancel { id } => print_run_detail(&client.cancel_run(&id).await?),
            GatewayCliCommand::Retry { id } => {
                let result = client.retry_run(&id).await?;
                crate::finish_remote_gateway_run(&loaded, result)
            }
            GatewayCliCommand::Status => {
                let health = client.health().await?;
                let readiness = client.readiness().await?;
                let metrics = client.metrics().await?;
                print_gateway_status(&health, &readiness, &metrics)
            }
            GatewayCliCommand::Audit { limit } => {
                print_gateway_audit_events(&client.audit_events(limit).await?)
            }
            GatewayCliCommand::Replay { limit } => {
                print_gateway_replay_window(&client.replay_window(limit).await?)
            }
            GatewayCliCommand::Incident { id, out } => {
                let bundle = client.incident_bundle(&id).await?;
                let path = save_incident_bundle(&loaded, &bundle, out)?;
                print_gateway_incident_bundle(&bundle, &path)
            }
        };
    }

    let gateway = build_gateway_handle(&loaded, None)?;
    match command {
        GatewayCliCommand::Sessions => crate::print_session_list(&gateway.list_sessions()?),
        GatewayCliCommand::Runs => print_run_list(&gateway.list_runs()?),
        GatewayCliCommand::ShowRun { id } => {
            let run = gateway
                .load_run(&id)?
                .ok_or_else(|| anyhow!("run not found: {}", id))?;
            print_run_detail(&run)
        }
        GatewayCliCommand::Cancel { id } => print_run_detail(&gateway.cancel_run(&id)?),
        GatewayCliCommand::Retry { id } => {
            let result = gateway.retry_run(&id)?;
            finish_gateway_outcome(result.wait().await)
        }
        GatewayCliCommand::Status => {
            let health = gateway.health();
            let readiness = gateway.readiness();
            let metrics = gateway.metrics();
            print_gateway_status(&health, &readiness, &metrics)
        }
        GatewayCliCommand::Audit { limit } => {
            print_gateway_audit_events(&gateway.audit_events(limit))
        }
        GatewayCliCommand::Replay { limit } => {
            print_gateway_replay_window(&gateway.replay_window(limit))
        }
        GatewayCliCommand::Incident { id, out } => {
            let (bundle, saved_path) = gateway.incident_bundle(&id)?;
            let path = if let Some(out) = out {
                fs::write(&out, serde_json::to_string_pretty(&bundle)?)?;
                out
            } else {
                saved_path
            };
            print_gateway_incident_bundle(&bundle, &path)
        }
        GatewayCliCommand::Serve { local, http } => {
            serve_gateway(&loaded, gateway, local, http).await
        }
    }
}

async fn serve_gateway(
    loaded: &mosaic_config::LoadedMosaicConfig,
    gateway: mosaic_gateway::GatewayHandle,
    local: bool,
    http: Option<String>,
) -> Result<()> {
    if let Some(bind) = http {
        let addr: std::net::SocketAddr = bind.parse()?;
        let session_count = gateway.list_sessions()?.len();
        println!("http gateway ready");
        println!("active_profile: {}", loaded.config.active_profile);
        println!("deployment_profile: {}", loaded.config.deployment.profile);
        println!("auth_mode: {}", gateway.auth_mode());
        println!("sessions: {}", session_count);
        println!("listen: {}", addr);
        println!("press Ctrl-C to stop");

        mosaic_gateway::serve_http_with_shutdown(gateway, addr, async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await?;
        println!("http gateway stopped");
        return Ok(());
    }

    if !local {
        bail!("use `mosaic gateway serve --local` or `mosaic gateway serve --http <addr>`");
    }

    let session_count = gateway.list_sessions()?.len();
    println!("local gateway ready");
    println!("active_profile: {}", loaded.config.active_profile);
    println!("deployment_profile: {}", loaded.config.deployment.profile);
    println!("auth_mode: {}", gateway.auth_mode());
    println!("sessions: {}", session_count);
    println!("press Ctrl-C to stop");

    let mut receiver = gateway.subscribe();
    loop {
        tokio::select! {
            signal = tokio::signal::ctrl_c() => {
                signal?;
                println!("local gateway stopped");
                break;
            }
            event = receiver.recv() => {
                match event {
                    Ok(envelope) => {
                        println!(
                            "[gateway] run={} corr={} session={} route={} event= {}",
                            envelope.gateway_run_id,
                            envelope.correlation_id,
                            envelope.session_id.as_deref().unwrap_or("<none>"),
                            envelope.session_route,
                            gateway_event_label(&envelope.event),
                        );
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(skipped)) => {
                        println!("[gateway] lagged {} events", skipped);
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                }
            }
        }
    }

    Ok(())
}
