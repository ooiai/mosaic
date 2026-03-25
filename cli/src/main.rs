use std::{env, fs, path::PathBuf, sync::Arc};

use anyhow::{Context, Result, anyhow, bail};
use clap::{Parser, Subcommand};
use mosaic_config::{
    ACTIVE_PROFILE_ENV, ConfigSourceKind, DoctorStatus, LoadConfigOptions, LoadedMosaicConfig,
    ValidationLevel, doctor_mosaic_config, init_workspace_config, load_from_file,
    load_mosaic_config, redact_mosaic_config, save_mosaic_config, validate_mosaic_config,
};
use mosaic_control_protocol::{
    GatewayEvent, IngressTrace, RunResponse, SessionDetailDto, SessionSummaryDto,
};
use mosaic_gateway::{
    GatewayCommand as GatewayControlCommand, GatewayHandle, GatewayRunError, GatewayRunRequest,
    GatewayRunResult,
};
use mosaic_inspect::RunTrace;
use mosaic_provider::ProviderProfileRegistry;
use mosaic_runtime::events::RunEventSink;
use mosaic_sdk::GatewayClient;
use mosaic_session_core::SessionSummary;

mod bootstrap;
mod output;

#[derive(Debug, Parser)]
#[command(name = "mosaic")]
#[command(version, about = "Mosaic control-plane console and runtime skeleton")]
struct Cli {
    #[arg(long, global = true, help = "Start the TUI in resume mode")]
    resume: bool,
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Debug, Subcommand)]
enum Commands {
    Run {
        file: PathBuf,
        #[arg(long, conflicts_with = "workflow")]
        skill: Option<String>,
        #[arg(long, conflicts_with = "skill")]
        workflow: Option<String>,
        #[arg(long)]
        session: Option<String>,
        #[arg(long)]
        profile: Option<String>,
        #[arg(long)]
        attach: Option<String>,
        #[arg(long, help = "Show the TUI while the run executes")]
        tui: bool,
    },
    Inspect {
        file: PathBuf,
    },
    Tui {
        #[arg(long)]
        session: Option<String>,
        #[arg(long)]
        profile: Option<String>,
        #[arg(long)]
        attach: Option<String>,
    },
    Setup {
        #[command(subcommand)]
        command: SetupCommand,
    },
    Session {
        #[arg(long)]
        attach: Option<String>,
        #[command(subcommand)]
        command: SessionCommand,
    },
    Model {
        #[command(subcommand)]
        command: ModelCommand,
    },
    Gateway {
        #[command(subcommand)]
        command: GatewayCliCommand,
    },
}

#[derive(Debug, Subcommand, PartialEq, Eq)]
enum SetupCommand {
    Init {
        #[arg(long)]
        force: bool,
    },
    Validate,
    Doctor,
}

#[derive(Debug, Subcommand, PartialEq, Eq)]
enum SessionCommand {
    List,
    Show { id: String },
}

#[derive(Debug, Subcommand, PartialEq, Eq)]
enum ModelCommand {
    List,
    Use { profile: String },
}

#[derive(Debug, Subcommand, PartialEq, Eq)]
enum GatewayCliCommand {
    Serve {
        #[arg(long, help = "Start the local in-process gateway monitor")]
        local: bool,
        #[arg(long, help = "Bind the HTTP gateway protocol on host:port")]
        http: Option<String>,
    },
    Sessions,
}

#[derive(Debug, PartialEq, Eq)]
enum DispatchCommand {
    Tui {
        resume: bool,
        session: Option<String>,
        profile: Option<String>,
        attach: Option<String>,
    },
    Run {
        file: PathBuf,
        skill: Option<String>,
        workflow: Option<String>,
        session: Option<String>,
        profile: Option<String>,
        attach: Option<String>,
        tui: bool,
        resume: bool,
    },
    Inspect {
        file: PathBuf,
    },
    Setup {
        command: SetupCommand,
    },
    Session {
        attach: Option<String>,
        command: SessionCommand,
    },
    Model {
        command: ModelCommand,
    },
    Gateway {
        command: GatewayCliCommand,
    },
}

impl Cli {
    fn dispatch(self) -> DispatchCommand {
        match self.command {
            None => DispatchCommand::Tui {
                resume: self.resume,
                session: None,
                profile: None,
                attach: None,
            },
            Some(Commands::Tui {
                session,
                profile,
                attach,
            }) => DispatchCommand::Tui {
                resume: self.resume,
                session,
                profile,
                attach,
            },
            Some(Commands::Run {
                file,
                skill,
                workflow,
                session,
                profile,
                attach,
                tui,
            }) => DispatchCommand::Run {
                file,
                skill,
                workflow,
                session,
                profile,
                attach,
                tui,
                resume: self.resume,
            },
            Some(Commands::Inspect { file }) => DispatchCommand::Inspect { file },
            Some(Commands::Setup { command }) => DispatchCommand::Setup { command },
            Some(Commands::Session { attach, command }) => {
                DispatchCommand::Session { attach, command }
            }
            Some(Commands::Model { command }) => DispatchCommand::Model { command },
            Some(Commands::Gateway { command }) => DispatchCommand::Gateway { command },
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    match Cli::parse().dispatch() {
        DispatchCommand::Tui {
            resume,
            session,
            profile,
            attach,
        } => tui_cmd(resume, session, profile, attach).await,
        DispatchCommand::Run {
            file,
            skill,
            workflow,
            session,
            profile,
            attach,
            tui,
            resume,
        } => run_cmd(file, skill, workflow, session, profile, attach, tui, resume).await,
        DispatchCommand::Inspect { file } => inspect_cmd(file),
        DispatchCommand::Setup { command } => setup_cmd(command),
        DispatchCommand::Session { attach, command } => session_cmd(attach, command).await,
        DispatchCommand::Model { command } => model_cmd(command),
        DispatchCommand::Gateway { command } => gateway_cmd(command).await,
    }
}

async fn tui_cmd(
    resume: bool,
    session: Option<String>,
    profile: Option<String>,
    attach: Option<String>,
) -> Result<()> {
    let loaded = ensure_loaded_config(profile.clone())?;
    let available_profiles = loaded
        .config
        .profiles
        .iter()
        .map(|(name, profile)| mosaic_tui::app::ProfileOption {
            name: name.clone(),
            model: profile.model.clone(),
            provider_type: profile.provider_type.clone(),
        })
        .collect();
    let (gateway, session_id, active_profile, active_model) = if let Some(url) = attach {
        let client = GatewayClient::new(url);
        let session_id = resolve_remote_tui_session_id(&client, session.as_deref()).await?;
        let session_detail = client.get_session(&session_id).await?;
        let active_profile = profile
            .or_else(|| {
                session_detail
                    .as_ref()
                    .map(|detail| detail.provider_profile.clone())
            })
            .unwrap_or_else(|| loaded.config.active_profile.clone());
        let active_model = session_detail
            .as_ref()
            .map(|detail| detail.model.clone())
            .or_else(|| {
                loaded
                    .config
                    .profiles
                    .get(&active_profile)
                    .map(|profile| profile.model.clone())
            })
            .unwrap_or_else(|| active_profile.clone());

        (
            mosaic_tui::InteractiveGateway::Remote(client),
            session_id,
            active_profile,
            active_model,
        )
    } else {
        let gateway = build_gateway_handle(&loaded, None)?;
        let session_id = resolve_tui_session_id(&gateway, session.as_deref())?;
        let active_profile = profile.unwrap_or_else(|| loaded.config.active_profile.clone());
        let active_model = loaded
            .config
            .profiles
            .get(&active_profile)
            .ok_or_else(|| anyhow!("unknown provider profile: {}", active_profile))?
            .model
            .clone();

        (
            mosaic_tui::InteractiveGateway::Local(gateway),
            session_id,
            active_profile,
            active_model,
        )
    };

    let context = mosaic_tui::InteractiveSessionContext {
        gateway,
        runtime_handle: tokio::runtime::Handle::current(),
        event_buffer: mosaic_tui::build_tui_event_buffer(),
        session_id,
        system: None,
        active_profile,
        active_model,
        available_profiles,
    };

    tokio::task::spawn_blocking(move || mosaic_tui::run_interactive_session(resume, context))
        .await??;

    Ok(())
}

async fn run_cmd(
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
    let forwarder =
        spawn_gateway_runtime_event_forwarder(gateway.subscribe(), Arc::new(output::CliEventSink));
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
    app_cfg: mosaic_config::AppConfig,
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
    app_cfg: mosaic_config::AppConfig,
    skill: Option<String>,
    workflow: Option<String>,
    session: Option<String>,
    profile: Option<String>,
    attach: String,
) -> Result<()> {
    let client = GatewayClient::new(attach.clone());
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

    finish_remote_gateway_run(&loaded, response)
}

fn finish_gateway_outcome(
    outcome: std::result::Result<GatewayRunResult, GatewayRunError>,
) -> Result<()> {
    match outcome {
        Ok(result) => finish_successful_gateway_run(result),
        Err(err) => finish_failed_gateway_run(err),
    }
}

fn finish_successful_gateway_run(result: GatewayRunResult) -> Result<()> {
    println!("{}", result.output);
    println!("saved trace: {}", result.trace_path.display());
    println!("gateway_run_id: {}", result.gateway_run_id);
    println!("correlation_id: {}", result.correlation_id);
    println!("session_route: {}", result.session_route);
    Ok(())
}

fn finish_failed_gateway_run(err: GatewayRunError) -> Result<()> {
    let gateway_run_id = err.gateway_run_id().to_owned();
    let correlation_id = err.correlation_id().to_owned();
    let session_route = err.session_route().to_owned();
    let (source, _trace, path) = err.into_parts();

    if !path.as_os_str().is_empty() {
        println!("saved trace: {}", path.display());
    }
    println!("gateway_run_id: {}", gateway_run_id);
    println!("correlation_id: {}", correlation_id);
    println!("session_route: {}", session_route);

    Err(source)
}

fn setup_cmd(command: SetupCommand) -> Result<()> {
    match command {
        SetupCommand::Init { force } => {
            let cwd = env::current_dir()?;
            let path = init_workspace_config(&cwd, force)?;
            println!("initialized workspace config: {}", path.display());
            Ok(())
        }
        SetupCommand::Validate => {
            let loaded = load_config()?;
            let report = validate_mosaic_config(&loaded.config);
            print_config_summary(&loaded);
            print_validation_report(&report);

            if report.has_errors() {
                bail!("configuration validation failed")
            }

            println!("validation: ok");
            Ok(())
        }
        SetupCommand::Doctor => {
            let loaded = load_config()?;
            let doctor = doctor_mosaic_config(&loaded.config, &current_dir()?);
            print_config_summary(&loaded);
            print_validation_report(&doctor.validation);
            println!("doctor checks:");
            for check in &doctor.checks {
                println!(
                    "  [{}] {}",
                    doctor_status_label(&check.status),
                    check.message
                );
            }

            if doctor.has_errors() {
                bail!("configuration doctor found errors")
            }

            println!("doctor: ok");
            Ok(())
        }
    }
}

async fn session_cmd(attach: Option<String>, command: SessionCommand) -> Result<()> {
    if let Some(url) = attach {
        let client = GatewayClient::new(url);

        return match command {
            SessionCommand::List => print_remote_session_list(&client.list_sessions().await?),
            SessionCommand::Show { id } => {
                let session = client
                    .get_session(&id)
                    .await?
                    .ok_or_else(|| anyhow!("session not found: {}", id))?;
                print_remote_session_detail(&session)
            }
        };
    }

    let loaded = ensure_loaded_config(None)?;
    let gateway = build_gateway_handle(&loaded, None)?;

    match command {
        SessionCommand::List => print_session_list(&gateway.list_sessions()?),
        SessionCommand::Show { id } => {
            let session = gateway
                .load_session(&id)?
                .ok_or_else(|| anyhow!("session not found: {}", id))?;

            println!("id: {}", session.id);
            println!("title: {}", session.title);
            println!("created_at: {}", session.created_at);
            println!("updated_at: {}", session.updated_at);
            println!("provider_profile: {}", session.provider_profile);
            println!("provider_type: {}", session.provider_type);
            println!("model: {}", session.model);
            println!("last_run_id: {:?}", session.last_run_id);
            println!("session_route: {}", session.gateway.route);
            println!(
                "last_gateway_run_id: {:?}",
                session.gateway.last_gateway_run_id
            );
            println!(
                "last_correlation_id: {:?}",
                session.gateway.last_correlation_id
            );
            println!("message_count: {}", session.transcript.len());

            if !session.transcript.is_empty() {
                println!("\ntranscript:");
                for (idx, message) in session.transcript.iter().enumerate() {
                    println!(
                        "[{}] {} {} {:?}",
                        idx + 1,
                        transcript_role_label(&message.role),
                        message.created_at,
                        message.tool_call_id
                    );
                    println!("  {}", truncate_for_cli(&message.content, 400));
                }
            }

            Ok(())
        }
    }
}

async fn gateway_cmd(command: GatewayCliCommand) -> Result<()> {
    let loaded = ensure_loaded_config(None)?;
    let gateway = build_gateway_handle(&loaded, None)?;

    match command {
        GatewayCliCommand::Sessions => print_session_list(&gateway.list_sessions()?),
        GatewayCliCommand::Serve { local, http } => {
            if let Some(bind) = http {
                let addr: std::net::SocketAddr = bind.parse()?;
                let session_count = gateway.list_sessions()?.len();
                println!("http gateway ready");
                println!("active_profile: {}", loaded.config.active_profile);
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
                                    "[gateway] run={} corr={} session={} route={} event={}",
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
    }
}

fn model_cmd(command: ModelCommand) -> Result<()> {
    match command {
        ModelCommand::List => {
            let loaded = ensure_loaded_config(None)?;
            let registry = ProviderProfileRegistry::from_config(&loaded.config)?;

            for profile in registry.list() {
                let marker = if profile.name == loaded.config.active_profile {
                    '*'
                } else {
                    ' '
                };
                println!(
                    "{} {} | type={} | model={} | family={} | tools={} | sessions={} | api_key_env={:?} | api_key_present={}",
                    marker,
                    profile.name,
                    profile.provider_type,
                    profile.model,
                    profile.capabilities.family,
                    profile.capabilities.supports_tools,
                    profile.capabilities.supports_sessions,
                    profile.api_key_env,
                    profile.api_key_present(),
                );
            }

            Ok(())
        }
        ModelCommand::Use { profile } => {
            let mut loaded = ensure_loaded_config(None)?;

            if !loaded.config.profiles.contains_key(&profile) {
                bail!("unknown provider profile: {}", profile);
            }

            loaded.config.active_profile = profile.clone();
            save_mosaic_config(&loaded.workspace_config_path, &loaded.config)?;

            println!(
                "active profile set to {} in {}",
                profile,
                loaded.workspace_config_path.display()
            );

            if env::var(ACTIVE_PROFILE_ENV).is_ok() {
                println!(
                    "note: {} is currently set and will still override the workspace profile at runtime",
                    ACTIVE_PROFILE_ENV
                );
            }

            Ok(())
        }
    }
}

fn inspect_cmd(file: PathBuf) -> Result<()> {
    let content = fs::read_to_string(file)?;
    let trace: RunTrace = serde_json::from_str(&content)?;
    let summary = trace.summary();

    println!("run_id: {}", trace.run_id);
    println!("gateway_run_id: {:?}", trace.gateway_run_id);
    println!("correlation_id: {:?}", trace.correlation_id);
    println!("session_id: {:?}", trace.session_id);
    println!("session_route: {:?}", trace.session_route);
    println!("ingress: {:?}", trace.ingress);
    println!("workflow_name: {:?}", trace.workflow_name);
    println!("status: {}", summary.status);
    println!("started_at: {}", trace.started_at);
    println!("finished_at: {:?}", trace.finished_at);
    println!("duration_ms: {:?}", summary.duration_ms);
    println!("input: {}", trace.input);
    println!("output: {:?}", trace.output);
    println!("error: {:?}", trace.error);

    if let Some(profile) = &trace.effective_profile {
        println!("\neffective_profile:");
        println!("  profile: {}", profile.profile);
        println!("  provider_type: {}", profile.provider_type);
        println!("  model: {}", profile.model);
        println!("  api_key_env: {:?}", profile.api_key_env);
        println!("  api_key_present: {}", profile.api_key_present);
    }

    if let Some(ingress) = &trace.ingress {
        println!("\ningress:");
        println!("  kind: {}", ingress.kind);
        println!("  channel: {:?}", ingress.channel);
        println!("  source: {:?}", ingress.source);
        println!("  remote_addr: {:?}", ingress.remote_addr);
        println!("  display_name: {:?}", ingress.display_name);
        println!("  gateway_url: {:?}", ingress.gateway_url);
    }

    println!("\nsummary:");
    println!("  tool_calls: {}", summary.tool_calls);
    println!("  skill_calls: {}", summary.skill_calls);
    println!("  workflow_steps: {}", trace.step_traces.len());

    if let Ok(loaded) = load_config() {
        let redacted = redact_mosaic_config(&loaded.config);
        println!("\ncurrent_workspace_config:");
        println!("  active_profile: {}", redacted.active_profile);
        println!(
            "  session_store_root_dir: {}",
            redacted.session_store_root_dir
        );
        println!("  inspect_runs_dir: {}", redacted.inspect_runs_dir);
        println!("  profiles:");
        for profile in redacted.profiles {
            println!(
                "    - {} | type={} | model={} | api_key_env={:?} | api_key_present={}",
                profile.name,
                profile.provider_type,
                profile.model,
                profile.api_key_env,
                profile.api_key_present
            );
        }
    }

    if !trace.tool_calls.is_empty() {
        println!("\n== tool calls ==");

        for (idx, call) in trace.tool_calls.iter().enumerate() {
            println!("[{}]", idx + 1);
            println!("  call_id: {:?}", call.call_id);
            println!("  name: {}", call.name);
            println!("  source: {}", call.source.label());
            if let Some(server_name) = call.source.server_name() {
                println!("  server_name: {}", server_name);
            }
            if let Some(remote_tool_name) = call.source.remote_tool_name() {
                println!("  remote_tool_name: {}", remote_tool_name);
            }
            println!("  started_at: {}", call.started_at);
            println!("  finished_at: {:?}", call.finished_at);
            println!("  duration_ms: {:?}", call.duration_ms());
            println!("  input: {}", serde_json::to_string_pretty(&call.input)?);

            match &call.output {
                Some(output) => println!("  output_preview: {}", truncate_for_cli(output, 240)),
                None => println!("  output_preview: <none>"),
            }
        }
    }

    if !trace.skill_calls.is_empty() {
        println!("\n== skill calls ==");

        for (idx, call) in trace.skill_calls.iter().enumerate() {
            println!("[{}]", idx + 1);
            println!("  name: {}", call.name);
            println!("  started_at: {}", call.started_at);
            println!("  finished_at: {:?}", call.finished_at);
            println!("  duration_ms: {:?}", call.duration_ms());
            println!("  input: {}", serde_json::to_string_pretty(&call.input)?);

            match &call.output {
                Some(output) => println!("  output_preview: {}", truncate_for_cli(output, 240)),
                None => println!("  output_preview: <none>"),
            }
        }
    }

    if !trace.step_traces.is_empty() {
        println!("\n== workflow steps ==");

        for (idx, step) in trace.step_traces.iter().enumerate() {
            println!("[{}]", idx + 1);
            println!("  name: {}", step.name);
            println!("  kind: {}", step.kind);
            println!("  status: {}", step.status());
            println!("  started_at: {}", step.started_at);
            println!("  finished_at: {:?}", step.finished_at);
            println!("  duration_ms: {:?}", step.duration_ms());
            println!("  input_preview: {}", truncate_for_cli(&step.input, 240));

            match &step.output {
                Some(output) => println!("  output_preview: {}", truncate_for_cli(output, 240)),
                None => println!("  output_preview: <none>"),
            }

            match &step.error {
                Some(error) => println!("  error: {}", truncate_for_cli(error, 240)),
                None => println!("  error: <none>"),
            }
        }
    }

    Ok(())
}

fn load_config() -> Result<LoadedMosaicConfig> {
    load_mosaic_config(&LoadConfigOptions::default())
}

fn resolve_tui_session_id(gateway: &GatewayHandle, requested: Option<&str>) -> Result<String> {
    if let Some(session_id) = requested {
        return Ok(session_id.to_owned());
    }

    Ok(gateway
        .list_sessions()?
        .into_iter()
        .next()
        .map(|session| session.id)
        .unwrap_or_else(|| "default".to_owned()))
}

async fn resolve_remote_tui_session_id(
    client: &GatewayClient,
    requested: Option<&str>,
) -> Result<String> {
    if let Some(session_id) = requested {
        return Ok(session_id.to_owned());
    }

    Ok(client
        .list_sessions()
        .await?
        .into_iter()
        .next()
        .map(|session| session.id)
        .unwrap_or_else(|| "default".to_owned()))
}

fn build_gateway_handle(
    loaded: &LoadedMosaicConfig,
    app_cfg: Option<&mosaic_config::AppConfig>,
) -> Result<GatewayHandle> {
    bootstrap::build_local_gateway(tokio::runtime::Handle::current(), &loaded.config, app_cfg)
}

fn print_session_list(sessions: &[SessionSummary]) -> Result<()> {
    if sessions.is_empty() {
        println!("no sessions found");
        return Ok(());
    }

    for session in sessions {
        println!(
            "{} | {} | profile={} | model={} | route={} | messages={} | updated_at={} | gateway_run={} | correlation={}",
            session.id,
            session.title,
            session.provider_profile,
            session.model,
            session.session_route,
            session.message_count,
            session.updated_at,
            session.last_gateway_run_id.as_deref().unwrap_or("-"),
            session.last_correlation_id.as_deref().unwrap_or("-"),
        );
        if let Some(preview) = session.last_message_preview.as_deref() {
            println!("  last: {}", preview);
        }
    }

    Ok(())
}

fn print_remote_session_list(sessions: &[SessionSummaryDto]) -> Result<()> {
    if sessions.is_empty() {
        println!("no sessions found");
        return Ok(());
    }

    for session in sessions {
        println!(
            "{} | {} | profile={} | model={} | route={} | messages={} | updated_at={} | gateway_run={} | correlation={}",
            session.id,
            session.title,
            session.provider_profile,
            session.model,
            session.session_route,
            session.message_count,
            session.updated_at,
            session.last_gateway_run_id.as_deref().unwrap_or("-"),
            session.last_correlation_id.as_deref().unwrap_or("-"),
        );
        if let Some(preview) = session.last_message_preview.as_deref() {
            println!("  last: {}", preview);
        }
    }

    Ok(())
}

fn print_remote_session_detail(session: &SessionDetailDto) -> Result<()> {
    println!("id: {}", session.id);
    println!("title: {}", session.title);
    println!("created_at: {}", session.created_at);
    println!("updated_at: {}", session.updated_at);
    println!("provider_profile: {}", session.provider_profile);
    println!("provider_type: {}", session.provider_type);
    println!("model: {}", session.model);
    println!("last_run_id: {:?}", session.last_run_id);
    println!("session_route: {}", session.gateway.route);
    println!(
        "last_gateway_run_id: {:?}",
        session.gateway.last_gateway_run_id
    );
    println!(
        "last_correlation_id: {:?}",
        session.gateway.last_correlation_id
    );
    println!("message_count: {}", session.transcript.len());

    if !session.transcript.is_empty() {
        println!("\ntranscript:");
        for (idx, message) in session.transcript.iter().enumerate() {
            println!(
                "[{}] {} {} {:?}",
                idx + 1,
                remote_transcript_role_label(&message.role),
                message.created_at,
                message.tool_call_id
            );
            println!("  {}", truncate_for_cli(&message.content, 400));
        }
    }

    Ok(())
}

fn finish_remote_gateway_run(loaded: &LoadedMosaicConfig, result: RunResponse) -> Result<()> {
    let trace_path = save_remote_trace(loaded, &result.trace)?;
    println!("{}", result.output);
    println!("saved trace: {}", trace_path.display());
    println!("gateway_run_id: {}", result.gateway_run_id);
    println!("correlation_id: {}", result.correlation_id);
    println!("session_route: {}", result.session_route);
    Ok(())
}

fn save_remote_trace(loaded: &LoadedMosaicConfig, trace: &RunTrace) -> Result<PathBuf> {
    let dir = resolve_workspace_relative_path(&loaded.config.inspect.runs_dir)?;
    trace.save_to_dir(dir)
}

fn resolve_workspace_relative_path(path: &str) -> Result<PathBuf> {
    let path = PathBuf::from(path);
    if path.is_absolute() {
        return Ok(path);
    }

    Ok(env::current_dir()?.join(path))
}

fn local_cli_ingress(gateway_url: Option<String>) -> IngressTrace {
    IngressTrace {
        kind: "local_cli".to_owned(),
        channel: Some("cli".to_owned()),
        source: Some("mosaic-cli".to_owned()),
        remote_addr: None,
        display_name: None,
        gateway_url,
    }
}

fn remote_cli_ingress(gateway_url: &str) -> IngressTrace {
    IngressTrace {
        kind: "remote_operator".to_owned(),
        channel: Some("cli".to_owned()),
        source: Some("mosaic-cli".to_owned()),
        remote_addr: None,
        display_name: None,
        gateway_url: Some(gateway_url.to_owned()),
    }
}

fn spawn_gateway_runtime_event_forwarder(
    mut receiver: tokio::sync::broadcast::Receiver<mosaic_gateway::GatewayEventEnvelope>,
    sink: Arc<dyn RunEventSink>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        loop {
            let Ok(envelope) = receiver.recv().await else {
                break;
            };

            if let GatewayEvent::Runtime(event) = envelope.event {
                sink.emit(event);
            }
        }
    })
}

fn gateway_event_label(event: &GatewayEvent) -> String {
    match event {
        GatewayEvent::RunSubmitted { profile, .. } => format!("run_submitted profile={profile}"),
        GatewayEvent::Runtime(_) => "runtime_event".to_owned(),
        GatewayEvent::SessionUpdated { summary } => {
            format!(
                "session_updated id={} messages={}",
                summary.id, summary.message_count
            )
        }
        GatewayEvent::RunCompleted { output_preview } => {
            format!(
                "run_completed preview={}",
                truncate_for_cli(output_preview, 80)
            )
        }
        GatewayEvent::RunFailed { error } => {
            format!("run_failed error={}", truncate_for_cli(error, 80))
        }
    }
}

fn ensure_loaded_config(active_profile_override: Option<String>) -> Result<LoadedMosaicConfig> {
    let mut options = LoadConfigOptions::default();
    options.overrides.active_profile = active_profile_override;

    let loaded = load_mosaic_config(&options)?;
    let report = validate_mosaic_config(&loaded.config);
    if report.has_errors() {
        bail!(
            "configuration is invalid:\n{}",
            render_validation_issues(&report)
        );
    }

    Ok(loaded)
}

fn current_dir() -> Result<PathBuf> {
    env::current_dir().context("failed to resolve current directory")
}

fn print_config_summary(loaded: &LoadedMosaicConfig) {
    println!("config sources:");
    for source in &loaded.sources {
        println!(
            "  [{}] {}",
            config_source_label(&source.kind),
            source.detail
        );
    }

    let redacted = redact_mosaic_config(&loaded.config);
    println!("\nconfig summary:");
    println!("  schema_version: {}", redacted.schema_version);
    println!("  active_profile: {}", redacted.active_profile);
    println!(
        "  session_store_root_dir: {}",
        redacted.session_store_root_dir
    );
    println!("  inspect_runs_dir: {}", redacted.inspect_runs_dir);
    println!("  profiles:");
    for profile in redacted.profiles {
        println!(
            "    - {} | type={} | model={} | api_key_env={:?} | api_key_present={}",
            profile.name,
            profile.provider_type,
            profile.model,
            profile.api_key_env,
            profile.api_key_present
        );
    }
}

fn print_validation_report(report: &mosaic_config::ValidationReport) {
    if report.issues.is_empty() {
        println!("\nvalidation issues: none");
        return;
    }

    println!("\nvalidation issues:");
    for issue in &report.issues {
        println!(
            "  [{}] {}: {}",
            validation_level_label(&issue.level),
            issue.field,
            issue.message
        );
    }
}

fn render_validation_issues(report: &mosaic_config::ValidationReport) -> String {
    report
        .issues
        .iter()
        .map(|issue| {
            format!(
                "[{}] {}: {}",
                validation_level_label(&issue.level),
                issue.field,
                issue.message
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn config_source_label(kind: &ConfigSourceKind) -> &'static str {
    match kind {
        ConfigSourceKind::Defaults => "defaults",
        ConfigSourceKind::User => "user",
        ConfigSourceKind::Workspace => "workspace",
        ConfigSourceKind::Env => "env",
        ConfigSourceKind::Cli => "cli",
    }
}

fn validation_level_label(level: &ValidationLevel) -> &'static str {
    match level {
        ValidationLevel::Error => "error",
        ValidationLevel::Warning => "warning",
    }
}

fn doctor_status_label(status: &DoctorStatus) -> &'static str {
    match status {
        DoctorStatus::Ok => "ok",
        DoctorStatus::Warning => "warning",
        DoctorStatus::Error => "error",
    }
}

fn transcript_role_label(role: &mosaic_session_core::TranscriptRole) -> &'static str {
    match role {
        mosaic_session_core::TranscriptRole::System => "system",
        mosaic_session_core::TranscriptRole::User => "user",
        mosaic_session_core::TranscriptRole::Assistant => "assistant",
        mosaic_session_core::TranscriptRole::Tool => "tool",
    }
}

fn remote_transcript_role_label(role: &mosaic_control_protocol::TranscriptRoleDto) -> &'static str {
    match role {
        mosaic_control_protocol::TranscriptRoleDto::System => "system",
        mosaic_control_protocol::TranscriptRoleDto::User => "user",
        mosaic_control_protocol::TranscriptRoleDto::Assistant => "assistant",
        mosaic_control_protocol::TranscriptRoleDto::Tool => "tool",
    }
}

fn truncate_for_cli(value: &str, limit: usize) -> String {
    if value.chars().count() <= limit {
        return value.to_string();
    }

    let truncated: String = value.chars().take(limit).collect();
    format!("{truncated}...")
}

#[cfg(test)]
mod tests {
    use clap::Parser;
    use mosaic_tool_core::ToolSource;

    use super::{Cli, DispatchCommand, ModelCommand, SessionCommand, SetupCommand};

    #[test]
    fn defaults_to_tui_when_no_subcommand_is_present() {
        let cli = Cli::parse_from(["mosaic"]);

        assert_eq!(
            cli.dispatch(),
            DispatchCommand::Tui {
                resume: false,
                session: None,
                profile: None,
                attach: None,
            }
        );
    }

    #[test]
    fn accepts_resume_flag_without_forcing_a_subcommand() {
        let cli = Cli::parse_from(["mosaic", "--resume"]);

        assert_eq!(
            cli.dispatch(),
            DispatchCommand::Tui {
                resume: true,
                session: None,
                profile: None,
                attach: None,
            }
        );
    }

    #[test]
    fn parses_tui_subcommand_with_resume_flag() {
        let cli = Cli::parse_from(["mosaic", "tui", "--resume", "--session", "demo"]);

        assert_eq!(
            cli.dispatch(),
            DispatchCommand::Tui {
                resume: true,
                session: Some("demo".to_owned()),
                profile: None,
                attach: None,
            }
        );
    }

    #[test]
    fn parses_tui_subcommand_with_remote_attach() {
        let cli = Cli::parse_from([
            "mosaic",
            "tui",
            "--attach",
            "http://127.0.0.1:8080",
            "--session",
            "remote-demo",
        ]);

        assert_eq!(
            cli.dispatch(),
            DispatchCommand::Tui {
                resume: false,
                session: Some("remote-demo".to_owned()),
                profile: None,
                attach: Some("http://127.0.0.1:8080".to_owned()),
            }
        );
    }

    #[test]
    fn parses_run_subcommand() {
        let cli = Cli::parse_from([
            "mosaic",
            "run",
            "examples/basic-agent.yaml",
            "--skill",
            "summarize",
            "--session",
            "demo",
            "--profile",
            "gpt-5.4-mini",
        ]);

        assert_eq!(
            cli.dispatch(),
            DispatchCommand::Run {
                file: "examples/basic-agent.yaml".into(),
                skill: Some("summarize".to_owned()),
                workflow: None,
                session: Some("demo".to_owned()),
                profile: Some("gpt-5.4-mini".to_owned()),
                attach: None,
                tui: false,
                resume: false,
            }
        );
    }

    #[test]
    fn parses_run_subcommand_with_remote_attach() {
        let cli = Cli::parse_from([
            "mosaic",
            "run",
            "examples/basic-agent.yaml",
            "--attach",
            "http://127.0.0.1:8080",
        ]);

        assert_eq!(
            cli.dispatch(),
            DispatchCommand::Run {
                file: "examples/basic-agent.yaml".into(),
                skill: None,
                workflow: None,
                session: None,
                profile: None,
                attach: Some("http://127.0.0.1:8080".to_owned()),
                tui: false,
                resume: false,
            }
        );
    }

    #[test]
    fn parses_run_subcommand_with_tui() {
        let cli = Cli::parse_from([
            "mosaic",
            "--resume",
            "run",
            "examples/time-now-agent.yaml",
            "--tui",
        ]);

        assert_eq!(
            cli.dispatch(),
            DispatchCommand::Run {
                file: "examples/time-now-agent.yaml".into(),
                skill: None,
                workflow: None,
                session: None,
                profile: None,
                attach: None,
                tui: true,
                resume: true,
            }
        );
    }

    #[test]
    fn parses_run_subcommand_with_workflow() {
        let cli = Cli::parse_from([
            "mosaic",
            "run",
            "examples/research-skill.yaml",
            "--workflow",
            "research_brief",
        ]);

        assert_eq!(
            cli.dispatch(),
            DispatchCommand::Run {
                file: "examples/research-skill.yaml".into(),
                skill: None,
                workflow: Some("research_brief".to_owned()),
                session: None,
                profile: None,
                attach: None,
                tui: false,
                resume: false,
            }
        );
    }

    #[test]
    fn parses_inspect_subcommand() {
        let cli = Cli::parse_from(["mosaic", "inspect", ".mosaic/runs/demo.json"]);

        assert_eq!(
            cli.dispatch(),
            DispatchCommand::Inspect {
                file: ".mosaic/runs/demo.json".into(),
            }
        );
    }

    #[test]
    fn parses_setup_subcommands() {
        let cli = Cli::parse_from(["mosaic", "setup", "init", "--force"]);
        assert_eq!(
            cli.dispatch(),
            DispatchCommand::Setup {
                command: SetupCommand::Init { force: true },
            }
        );

        let cli = Cli::parse_from(["mosaic", "setup", "doctor"]);
        assert_eq!(
            cli.dispatch(),
            DispatchCommand::Setup {
                command: SetupCommand::Doctor,
            }
        );
    }

    #[test]
    fn parses_session_and_model_subcommands() {
        let cli = Cli::parse_from(["mosaic", "session", "show", "demo"]);
        assert_eq!(
            cli.dispatch(),
            DispatchCommand::Session {
                attach: None,
                command: SessionCommand::Show {
                    id: "demo".to_owned(),
                },
            }
        );

        let cli = Cli::parse_from(["mosaic", "model", "use", "gpt-5.4-mini"]);
        assert_eq!(
            cli.dispatch(),
            DispatchCommand::Model {
                command: ModelCommand::Use {
                    profile: "gpt-5.4-mini".to_owned(),
                },
            }
        );
    }

    #[test]
    fn parses_session_subcommand_with_remote_attach() {
        let cli = Cli::parse_from([
            "mosaic",
            "session",
            "--attach",
            "http://127.0.0.1:8080",
            "list",
        ]);

        assert_eq!(
            cli.dispatch(),
            DispatchCommand::Session {
                attach: Some("http://127.0.0.1:8080".to_owned()),
                command: SessionCommand::List,
            }
        );
    }

    #[test]
    fn parses_gateway_serve_http_subcommand() {
        let cli = Cli::parse_from(["mosaic", "gateway", "serve", "--http", "127.0.0.1:8080"]);

        assert_eq!(
            cli.dispatch(),
            DispatchCommand::Gateway {
                command: super::GatewayCliCommand::Serve {
                    local: false,
                    http: Some("127.0.0.1:8080".to_owned()),
                },
            }
        );
    }

    #[test]
    fn truncate_for_cli_keeps_short_strings() {
        assert_eq!(super::truncate_for_cli("hello", 10), "hello");
    }

    #[test]
    fn truncate_for_cli_shortens_long_strings() {
        assert_eq!(
            super::truncate_for_cli("abcdefghijklmnopqrstuvwxyz", 5),
            "abcde..."
        );
    }

    #[test]
    fn builtin_tool_source_has_no_remote_details() {
        let source = ToolSource::Builtin;

        assert_eq!(source.label(), "builtin");
        assert_eq!(source.server_name(), None);
        assert_eq!(source.remote_tool_name(), None);
    }

    #[test]
    fn mcp_tool_source_exposes_server_and_remote_tool_names() {
        let source = ToolSource::Mcp {
            server: "filesystem".to_owned(),
            remote_tool: "read_file".to_owned(),
        };

        assert_eq!(source.label(), "mcp");
        assert_eq!(source.server_name(), Some("filesystem"));
        assert_eq!(source.remote_tool_name(), Some("read_file"));
    }
}
