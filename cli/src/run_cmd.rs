use std::{path::PathBuf, sync::Arc};

use anyhow::{Result, bail};
use mosaic_config::{AppConfig, LoadedMosaicConfig, load_from_file};
use mosaic_gateway::{
    GatewayCommand as GatewayControlCommand, GatewayRunRequest, GatewayRunResult,
};

use crate::{
    build_gateway_handle, ensure_loaded_config, finish_gateway_outcome, gateway_client_from_loaded,
    local_cli_ingress, remote_cli_ingress, spawn_gateway_runtime_event_forwarder,
};

pub(crate) async fn run_cmd(
    file: PathBuf,
    skill: Option<String>,
    workflow: Option<String>,
    session: Option<String>,
    profile: Option<String>,
    attach: Option<String>,
    tui: bool,
    resume: bool,
) -> Result<()> {
    let app_cfg = load_from_file(&file)?;
    let loaded = ensure_loaded_config(profile.clone())?;

    if let Some(url) = attach {
        if tui {
            bail!(
                "remote attach does not support `mosaic run --tui`; use `mosaic tui --attach {url}`"
            )
        }

        return run_cmd_remote(loaded, app_cfg, skill, workflow, session, profile, url).await;
    }

    if tui {
        return run_cmd_with_tui(loaded, app_cfg, skill, workflow, session, profile, resume).await;
    }

    let gateway = build_gateway_handle(&loaded, Some(&app_cfg))?;
    let forwarder = spawn_gateway_runtime_event_forwarder(
        gateway.subscribe(),
        Arc::new(crate::output::CliEventSink),
    );
    let outcome = gateway
        .submit_command(GatewayControlCommand::SubmitRun(GatewayRunRequest {
            system: app_cfg.agent.system,
            input: app_cfg.task.input,
            skill,
            workflow,
            session_id: session,
            profile,
            ingress: Some(local_cli_ingress(None)),
        }))?
        .wait()
        .await;
    forwarder.abort();

    finish_gateway_outcome(outcome)
}

async fn run_cmd_with_tui(
    loaded: LoadedMosaicConfig,
    app_cfg: AppConfig,
    skill: Option<String>,
    workflow: Option<String>,
    session: Option<String>,
    profile: Option<String>,
    resume: bool,
) -> Result<()> {
    let gateway = build_gateway_handle(&loaded, Some(&app_cfg))?;
    let event_buffer = mosaic_tui::build_tui_event_buffer();
    let forwarder = spawn_gateway_runtime_event_forwarder(
        gateway.subscribe(),
        mosaic_tui::build_tui_event_sink(event_buffer.clone()),
    );

    let request = GatewayRunRequest {
        system: app_cfg.agent.system,
        input: app_cfg.task.input,
        skill,
        workflow,
        session_id: session,
        profile,
        ingress: Some(local_cli_ingress(None)),
    };

    let submitted = gateway.submit_command(GatewayControlCommand::SubmitRun(request))?;
    let runtime_handle = tokio::spawn(async move { submitted.wait().await });
    let tui_handle = tokio::task::spawn_blocking(move || {
        mosaic_tui::run_until_complete_with_event_buffer(resume, event_buffer)
    });

    let runtime_outcome = runtime_handle.await?;
    forwarder.abort();
    let tui_join = tui_handle.await;

    let run_result = finish_gateway_outcome(runtime_outcome);

    match (run_result, tui_join) {
        (Err(err), _) => Err(err),
        (Ok(()), Err(err)) => Err(err.into()),
        (Ok(()), Ok(tui_result)) => {
            tui_result?;
            Ok(())
        }
    }
}

async fn run_cmd_remote(
    loaded: LoadedMosaicConfig,
    app_cfg: AppConfig,
    skill: Option<String>,
    workflow: Option<String>,
    session: Option<String>,
    profile: Option<String>,
    attach: String,
) -> Result<()> {
    let client = gateway_client_from_loaded(&loaded, attach.clone());
    let response = client
        .submit_run(GatewayRunRequest {
            system: app_cfg.agent.system,
            input: app_cfg.task.input,
            skill,
            workflow,
            session_id: session,
            profile,
            ingress: Some(remote_cli_ingress(&attach)),
        })
        .await?;

    crate::finish_remote_gateway_run(&loaded, response)
}

pub(crate) fn finish_successful_gateway_run(result: GatewayRunResult) -> Result<()> {
    println!("{}", result.output);
    println!("saved trace: {}", result.trace_path.display());
    println!("gateway_run_id: {}", result.gateway_run_id);
    println!("correlation_id: {}", result.correlation_id);
    println!("session_route: {}", result.session_route);

    let mut next_steps = Vec::new();
    if let Some(session_id) = result.trace.session_id.as_deref() {
        next_steps.push(format!("mosaic session show {}", session_id));
    }
    next_steps.push(format!("mosaic inspect {}", result.trace_path.display()));
    crate::print_next_steps(next_steps);
    Ok(())
}
