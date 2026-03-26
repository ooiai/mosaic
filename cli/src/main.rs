use std::{
    collections::BTreeMap,
    env, fs, io,
    path::{Path, PathBuf},
    sync::Arc,
};

use anyhow::{Context, Result, anyhow, bail};
use chrono::Utc;
use clap::{Parser, Subcommand, ValueEnum};
use mosaic_config::{
    ACTIVE_PROFILE_ENV, ConfigSourceKind, DoctorStatus, LoadConfigOptions, LoadedMosaicConfig,
    PolicyConfig, ValidationLevel, doctor_mosaic_config, init_workspace_config, load_from_file,
    load_mosaic_config, redact_mosaic_config, save_mosaic_config, validate_mosaic_config,
};
use mosaic_control_protocol::{
    AdapterStatusDto, CapabilityJobDto, CronRegistrationDto, CronRegistrationRequest,
    ExecJobRequest, GatewayAuditEventDto, GatewayEvent, HealthResponse, IncidentBundleDto,
    IngressTrace, MetricsResponse, ReadinessResponse, ReplayWindowResponse, RunResponse,
    SessionDetailDto, SessionSummaryDto, WebhookJobRequest,
};
use mosaic_extension_core::{ExtensionStatus, ExtensionValidationReport, validate_extension_set};
use mosaic_gateway::{
    GatewayCommand as GatewayControlCommand, GatewayHandle, GatewayRunError, GatewayRunRequest,
    GatewayRunResult,
};
use mosaic_inspect::RunTrace;
use mosaic_memory::{FileMemoryStore, MemoryStore};
use mosaic_node_protocol::{
    DEFAULT_STALE_AFTER_SECS, FileNodeStore, NodeCapabilityDeclaration, NodeCommandDispatch,
    NodeCommandResultEnvelope, NodeRegistration,
};
use mosaic_provider::ProviderProfileRegistry;
use mosaic_runtime::events::RunEventSink;
use mosaic_sdk::GatewayClient;
use mosaic_session_core::SessionSummary;
use mosaic_tool_core::{ExecTool, ReadFileTool, Tool};
use tracing_subscriber::EnvFilter;

mod bootstrap;
mod output;

const CLI_ABOUT: &str = "Self-hosted AI assistant control plane for sessions, the TUI, Gateway routing, and trace inspection.";
const CLI_AFTER_HELP: &str = "Quick start:
  mosaic setup init
  mosaic setup validate
  mosaic setup doctor
  mosaic model list
  mosaic tui

Docs:
  docs/getting-started.md
  docs/configuration.md
  docs/cli.md

Examples:
  examples/providers/openai.yaml
  examples/workflows/research-brief.yaml";
const SETUP_AFTER_HELP: &str = "Examples:
  mosaic setup init
  mosaic setup validate
  mosaic setup doctor";
const GATEWAY_AFTER_HELP: &str = "Examples:
  mosaic gateway status
  mosaic gateway serve --local
  mosaic gateway serve --http 127.0.0.1:8080
  mosaic gateway incident <run-id>";

#[derive(Debug, Clone, Copy, ValueEnum, PartialEq, Eq)]
enum LogFormat {
    Plain,
    Json,
}

#[derive(Debug, Parser)]
#[command(name = "mosaic")]
#[command(version, about = CLI_ABOUT, after_help = CLI_AFTER_HELP)]
struct Cli {
    #[arg(
        long,
        global = true,
        default_value = "warn",
        help = "Set internal log verbosity for stderr output"
    )]
    log_level: String,
    #[arg(long, global = true, value_enum, default_value_t = LogFormat::Plain, help = "Set internal log output format")]
    log_format: LogFormat,
    #[arg(long, global = true, help = "Start the TUI in resume browser mode")]
    resume: bool,
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Run an app YAML file through the agent runtime.
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
        #[arg(long, help = "Show the TUI while the run executes the file-backed run")]
        tui: bool,
    },
    /// Inspect one saved run trace JSON file.
    Inspect { file: PathBuf },
    /// Start the interactive terminal UI.
    Tui {
        #[arg(long)]
        session: Option<String>,
        #[arg(long)]
        profile: Option<String>,
        #[arg(long)]
        attach: Option<String>,
    },
    /// Initialize and validate workspace configuration.
    Setup {
        #[command(subcommand)]
        command: SetupCommand,
    },
    /// List sessions or inspect one saved session transcript.
    Session {
        #[arg(long)]
        attach: Option<String>,
        #[command(subcommand)]
        command: SessionCommand,
    },
    /// List configured provider profiles or switch the active one.
    Model {
        #[command(subcommand)]
        command: ModelCommand,
    },
    /// Inspect or serve the control-plane Gateway.
    Gateway {
        #[arg(long)]
        attach: Option<String>,
        #[command(subcommand)]
        command: GatewayCliCommand,
    },
    /// Inspect channel adapter readiness.
    Adapter {
        #[arg(long)]
        attach: Option<String>,
        #[command(subcommand)]
        command: AdapterCommand,
    },
    /// Manage local or remote device nodes.
    Node {
        #[command(subcommand)]
        command: NodeCliCommand,
    },
    /// Inspect or trigger high-privilege capability surfaces.
    Capability {
        #[arg(long)]
        attach: Option<String>,
        #[command(subcommand)]
        command: CapabilityCommand,
    },
    /// Register and trigger scheduled runs.
    Cron {
        #[arg(long)]
        attach: Option<String>,
        #[command(subcommand)]
        command: CronCommand,
    },
    /// Inspect saved memory summaries and references.
    Memory {
        #[command(subcommand)]
        command: MemoryCommand,
    },
    /// Validate and reload extension manifests.
    Extension {
        #[command(subcommand)]
        command: ExtensionCommand,
    },
}

#[derive(Debug, Subcommand, PartialEq, Eq)]
#[command(after_help = SETUP_AFTER_HELP)]
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
#[command(after_help = GATEWAY_AFTER_HELP)]
enum GatewayCliCommand {
    Serve {
        #[arg(long, help = "Start the local in-process Gateway event monitor")]
        local: bool,
        #[arg(long, help = "Bind the HTTP Gateway control plane on host:port")]
        http: Option<String>,
    },
    Sessions,
    Status,
    Audit {
        #[arg(long, default_value_t = 50)]
        limit: usize,
    },
    Replay {
        #[arg(long, default_value_t = 50)]
        limit: usize,
    },
    Incident {
        id: String,
        #[arg(long)]
        out: Option<PathBuf>,
    },
}

#[derive(Debug, Subcommand, PartialEq, Eq)]
enum AdapterCommand {
    Status,
    Doctor,
}

#[derive(Debug, Subcommand, PartialEq, Eq)]
enum NodeCliCommand {
    Serve {
        #[arg(long)]
        id: Option<String>,
        #[arg(long)]
        label: Option<String>,
    },
    List,
    Attach {
        node_id: String,
        #[arg(long)]
        session: Option<String>,
    },
    Capabilities {
        node_id: Option<String>,
    },
}

#[derive(Debug, Subcommand, PartialEq, Eq)]
enum CapabilityCommand {
    Doctor,
    Jobs,
    Exec {
        #[command(subcommand)]
        command: ExecCapabilityCommand,
    },
    Webhook {
        #[command(subcommand)]
        command: WebhookCapabilityCommand,
    },
}

#[derive(Debug, Subcommand, PartialEq, Eq)]
enum ExecCapabilityCommand {
    Guardrails,
    Run {
        command: String,
        #[arg(long = "arg")]
        args: Vec<String>,
        #[arg(long)]
        cwd: Option<String>,
        #[arg(long)]
        session: Option<String>,
    },
}

#[derive(Debug, Subcommand, PartialEq, Eq)]
enum WebhookCapabilityCommand {
    Test {
        url: String,
        #[arg(long)]
        method: Option<String>,
        #[arg(long)]
        body: Option<String>,
        #[arg(long = "header")]
        headers: Vec<String>,
        #[arg(long)]
        session: Option<String>,
    },
}

#[derive(Debug, Subcommand, PartialEq, Eq)]
enum CronCommand {
    List,
    Register {
        id: String,
        schedule: String,
        input: String,
        #[arg(long)]
        session: Option<String>,
        #[arg(long)]
        profile: Option<String>,
        #[arg(long)]
        skill: Option<String>,
        #[arg(long)]
        workflow: Option<String>,
    },
    Trigger {
        id: String,
    },
}

#[derive(Debug, Subcommand, PartialEq, Eq)]
enum MemoryCommand {
    List,
    Show {
        session: String,
    },
    Search {
        query: String,
        #[arg(long)]
        tag: Option<String>,
    },
}

#[derive(Debug, Subcommand, PartialEq, Eq)]
enum ExtensionCommand {
    List,
    Validate,
    Reload,
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
        attach: Option<String>,
        command: GatewayCliCommand,
    },
    Adapter {
        attach: Option<String>,
        command: AdapterCommand,
    },
    Node {
        command: NodeCliCommand,
    },
    Capability {
        attach: Option<String>,
        command: CapabilityCommand,
    },
    Cron {
        attach: Option<String>,
        command: CronCommand,
    },
    Memory {
        command: MemoryCommand,
    },
    Extension {
        command: ExtensionCommand,
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
            Some(Commands::Gateway { attach, command }) => {
                DispatchCommand::Gateway { attach, command }
            }
            Some(Commands::Adapter { attach, command }) => {
                DispatchCommand::Adapter { attach, command }
            }
            Some(Commands::Node { command }) => DispatchCommand::Node { command },
            Some(Commands::Capability { attach, command }) => {
                DispatchCommand::Capability { attach, command }
            }
            Some(Commands::Cron { attach, command }) => DispatchCommand::Cron { attach, command },
            Some(Commands::Memory { command }) => DispatchCommand::Memory { command },
            Some(Commands::Extension { command }) => DispatchCommand::Extension { command },
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    init_tracing(&cli.log_level, cli.log_format)?;

    match cli.dispatch() {
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
        DispatchCommand::Gateway { attach, command } => gateway_cmd(attach, command).await,
        DispatchCommand::Adapter { attach, command } => adapter_cmd(attach, command).await,
        DispatchCommand::Node { command } => node_cmd(command).await,
        DispatchCommand::Capability { attach, command } => capability_cmd(attach, command).await,
        DispatchCommand::Cron { attach, command } => cron_cmd(attach, command).await,
        DispatchCommand::Memory { command } => memory_cmd(command),
        DispatchCommand::Extension { command } => extension_cmd(command),
    }
}

fn init_tracing(level: &str, format: LogFormat) -> Result<()> {
    let filter = env::var("MOSAIC_LOG")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .or_else(|| {
            env::var("RUST_LOG")
                .ok()
                .filter(|value| !value.trim().is_empty())
        })
        .unwrap_or_else(|| level.to_owned());
    let env_filter = EnvFilter::builder().parse_lossy(filter);
    let builder = tracing_subscriber::fmt()
        .with_env_filter(env_filter)
        .with_writer(io::stderr)
        .with_target(false);

    let result = match format {
        LogFormat::Plain => builder.try_init(),
        LogFormat::Json => builder.json().try_init(),
    };

    match result {
        Ok(()) => Ok(()),
        Err(error) => {
            let message = error.to_string();
            if message.contains("global default trace dispatcher has already been set") {
                Ok(())
            } else {
                Err(anyhow!("failed to initialize tracing: {}", message))
            }
        }
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
    let (extension_summary, extension_policy_summary, extension_errors) =
        tui_extension_status(&loaded)?;
    let (gateway, session_id, active_profile, active_model) = if let Some(url) = attach {
        let client = gateway_client_from_loaded(&loaded, url);
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
        extension_summary,
        extension_policy_summary,
        extension_errors,
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

    let mut next_steps = Vec::new();
    if let Some(session_id) = result.trace.session_id.as_deref() {
        next_steps.push(format!("mosaic session show {}", session_id));
    }
    next_steps.push(format!("mosaic inspect {}", result.trace_path.display()));
    print_next_steps(next_steps);
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

    if !path.as_os_str().is_empty() {
        print_next_steps([format!("mosaic inspect {}", path.display())]);
    }

    Err(source)
}

fn print_next_steps<I, S>(steps: I)
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let collected = steps
        .into_iter()
        .map(|step| step.as_ref().to_owned())
        .collect::<Vec<_>>();
    if collected.is_empty() {
        return;
    }

    println!("next:");
    for step in collected {
        println!("  {}", step);
    }
}

fn setup_cmd(command: SetupCommand) -> Result<()> {
    match command {
        SetupCommand::Init { force } => {
            let cwd = env::current_dir()?;
            let path = init_workspace_config(&cwd, force)?;
            println!("initialized workspace config: {}", path.display());
            print_next_steps([
                "mosaic setup validate",
                "mosaic setup doctor",
                "mosaic model list",
                "mosaic tui",
            ]);
            Ok(())
        }
        SetupCommand::Validate => {
            let loaded = load_config()?;
            let report = validate_mosaic_config(&loaded.config);
            print_config_summary(&loaded);
            print_validation_report(&report);

            if report.has_errors() {
                bail!(
                    "configuration validation failed
next: run `mosaic setup doctor` and compare `.mosaic/config.yaml` with `docs/configuration.md`"
                )
            }

            println!("validation: ok");
            print_next_steps(["mosaic setup doctor", "mosaic model list", "mosaic tui"]);
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
                bail!(
                    "configuration doctor found errors
next: fix the reported checks, then rerun `mosaic setup doctor`"
                )
            }

            println!("doctor: ok");
            print_next_steps(["mosaic model list", "mosaic tui", "mosaic gateway status"]);
            Ok(())
        }
    }
}

async fn session_cmd(attach: Option<String>, command: SessionCommand) -> Result<()> {
    if let Some(url) = attach {
        let loaded = ensure_loaded_config(None)?;
        let client = gateway_client_from_loaded(&loaded, url);

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
            println!("channel: {:?}", session.channel_context.channel);
            println!("actor_id: {:?}", session.channel_context.actor_id);
            println!("actor_name: {:?}", session.channel_context.actor_name);
            println!("thread_id: {:?}", session.channel_context.thread_id);
            println!("thread_title: {:?}", session.channel_context.thread_title);
            println!("reply_target: {:?}", session.channel_context.reply_target);
            println!(
                "last_gateway_run_id: {:?}",
                session.gateway.last_gateway_run_id
            );
            println!(
                "last_correlation_id: {:?}",
                session.gateway.last_correlation_id
            );
            println!("message_count: {}", session.transcript.len());
            println!("memory_summary: {:?}", session.memory.latest_summary);
            println!(
                "compressed_context: {:?}",
                session.memory.compressed_context
            );
            println!("memory_entry_count: {}", session.memory.memory_entry_count);
            println!("compression_count: {}", session.memory.compression_count);
            println!("reference_count: {}", session.references.len());

            if !session.references.is_empty() {
                println!("\nreferences:");
                for reference in &session.references {
                    println!(
                        "- {} | reason={} | created_at={}",
                        reference.session_id, reference.reason, reference.created_at
                    );
                }
            }

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

async fn gateway_cmd(attach: Option<String>, command: GatewayCliCommand) -> Result<()> {
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
        GatewayCliCommand::Sessions => print_session_list(&gateway.list_sessions()?),
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
    }
}

async fn adapter_cmd(attach: Option<String>, command: AdapterCommand) -> Result<()> {
    let adapters = if let Some(url) = attach {
        let loaded = ensure_loaded_config(None)?;
        gateway_client_from_loaded(&loaded, url)
            .list_adapters()
            .await?
    } else {
        let loaded = ensure_loaded_config(None)?;
        let gateway = build_gateway_handle(&loaded, None)?;
        gateway.list_adapter_statuses()
    };

    match command {
        AdapterCommand::Status => print_adapter_statuses(&adapters),
        AdapterCommand::Doctor => print_adapter_doctor(&adapters),
    }
}

async fn node_cmd(command: NodeCliCommand) -> Result<()> {
    match command {
        NodeCliCommand::Serve { id, label } => serve_local_node(id, label).await,
        NodeCliCommand::List => {
            let loaded = ensure_loaded_config(None)?;
            let gateway = build_gateway_handle(&loaded, None)?;
            print_node_list(&gateway.list_nodes()?)
        }
        NodeCliCommand::Attach { node_id, session } => {
            let loaded = ensure_loaded_config(None)?;
            let gateway = build_gateway_handle(&loaded, None)?;
            gateway.attach_node(&node_id, session.as_deref())?;
            match session {
                Some(session) => println!("attached node {} to session {}", node_id, session),
                None => println!("attached node {} as the default node route", node_id),
            }
            Ok(())
        }
        NodeCliCommand::Capabilities { node_id } => {
            let loaded = ensure_loaded_config(None)?;
            let gateway = build_gateway_handle(&loaded, None)?;
            match node_id {
                Some(node_id) => {
                    print_node_capabilities(&node_id, &gateway.node_capabilities(&node_id)?)
                }
                None => {
                    let nodes = gateway.list_nodes()?;
                    if nodes.is_empty() {
                        println!("no nodes found");
                        return Ok(());
                    }
                    for node in nodes {
                        print_node_capabilities(&node.node_id, &node.capabilities)?;
                    }
                    Ok(())
                }
            }
        }
    }
}

async fn serve_local_node(id: Option<String>, label: Option<String>) -> Result<()> {
    let node_store = FileNodeStore::new(resolve_workspace_relative_path(".mosaic/nodes")?);
    let node_id = id.unwrap_or_else(|| "local-headless".to_owned());
    let label = label.unwrap_or_else(|| "Local Headless Node".to_owned());
    let (read_file_tool, exec_tool) = build_headless_node_tools()?;
    let capabilities = vec![
        tool_node_capability(&read_file_tool),
        tool_node_capability(&exec_tool),
    ];
    let registration = NodeRegistration::new(
        node_id.clone(),
        label.clone(),
        "file-bus",
        "headless",
        capabilities,
    );
    node_store.register_node(&registration)?;
    let _ = node_store.heartbeat(&node_id)?;

    println!("headless node ready");
    println!("node_id: {}", node_id);
    println!("label: {}", label);
    println!("transport: file-bus");
    println!("node_store: {}", node_store.root().display());
    println!(
        "capabilities: {:?}",
        registration
            .capabilities
            .iter()
            .map(|cap| cap.name.as_str())
            .collect::<Vec<_>>()
    );
    println!("press Ctrl-C to stop");

    loop {
        tokio::select! {
            signal = tokio::signal::ctrl_c() => {
                signal?;
                node_store.disconnect_node(&node_id, "operator_shutdown")?;
                println!("headless node stopped");
                break;
            }
            _ = tokio::time::sleep(std::time::Duration::from_millis(200)) => {
                let _ = node_store.heartbeat(&node_id)?;
                for dispatch in node_store.pending_commands(&node_id)? {
                    execute_headless_node_dispatch(&node_store, &dispatch, &read_file_tool, &exec_tool).await?;
                }
            }
        }
    }

    Ok(())
}

fn build_headless_node_tools() -> Result<(ReadFileTool, ExecTool)> {
    let allowed_root = env::current_dir()?;
    Ok((
        ReadFileTool::new_with_allowed_roots(vec![allowed_root.clone()]),
        ExecTool::new(vec![allowed_root]),
    ))
}

fn tool_node_capability(tool: &dyn Tool) -> NodeCapabilityDeclaration {
    let metadata = tool.metadata();
    NodeCapabilityDeclaration {
        name: metadata
            .capability
            .node
            .capability
            .clone()
            .unwrap_or_else(|| metadata.name.clone()),
        kind: metadata.capability.kind.clone(),
        permission_scopes: metadata.capability.permission_scopes.clone(),
        risk: metadata.capability.risk.clone(),
    }
}

async fn execute_headless_node_dispatch(
    node_store: &FileNodeStore,
    dispatch: &NodeCommandDispatch,
    read_file_tool: &ReadFileTool,
    exec_tool: &ExecTool,
) -> Result<()> {
    let result = match dispatch.capability.as_str() {
        "read_file" => read_file_tool.call(dispatch.input.clone()).await,
        "exec_command" => exec_tool.call(dispatch.input.clone()).await,
        capability => Err(anyhow!("unsupported node capability: {}", capability)),
    };

    let envelope = match result {
        Ok(result) => NodeCommandResultEnvelope::success(dispatch, result),
        Err(err) => NodeCommandResultEnvelope::failure(dispatch, "failed", err.to_string(), None),
    };
    node_store.complete_command(&envelope)
}

fn print_node_list(nodes: &[NodeRegistration]) -> Result<()> {
    if nodes.is_empty() {
        println!("no nodes found");
        return Ok(());
    }

    for node in nodes {
        let health = node.health(Utc::now(), DEFAULT_STALE_AFTER_SECS);
        println!(
            "{} | health={} | transport={} | platform={} | capabilities={} | last_heartbeat_at={}",
            node.node_id,
            health.label(),
            node.transport,
            node.platform,
            node.capabilities
                .iter()
                .map(|cap| cap.name.as_str())
                .collect::<Vec<_>>()
                .join(","),
            node.last_heartbeat_at,
        );
        if let Some(reason) = node.last_disconnect_reason.as_deref() {
            println!("  disconnect_reason: {}", reason);
        }
    }

    Ok(())
}

fn print_node_capabilities(
    node_id: &str,
    capabilities: &[NodeCapabilityDeclaration],
) -> Result<()> {
    println!("node: {}", node_id);
    if capabilities.is_empty() {
        println!("  capabilities: none");
        return Ok(());
    }

    for capability in capabilities {
        println!(
            "  - {} | kind={} | risk={} | scopes={:?}",
            capability.name,
            capability.kind.label(),
            capability.risk.label(),
            capability
                .permission_scopes
                .iter()
                .map(|scope| scope.label())
                .collect::<Vec<_>>(),
        );
    }

    Ok(())
}

async fn capability_cmd(attach: Option<String>, command: CapabilityCommand) -> Result<()> {
    let loaded = ensure_loaded_config(None)?;

    match command {
        CapabilityCommand::Doctor => {
            if attach.is_some() {
                bail!("remote capability doctor is not supported");
            }
            let components = bootstrap::build_gateway_components(&loaded.config, None)?;
            println!("capability doctor:");
            println!("workspace_root: {}", env::current_dir()?.display());
            for tool in components.tools.iter() {
                let meta = tool.metadata();
                println!(
                    "{} | kind={} | risk={} | scopes={:?} | timeout_ms={} | retry_limit={} | authorized={} | healthy={} | source={}",
                    meta.name,
                    meta.capability.kind.label(),
                    meta.capability.risk.label(),
                    meta.capability
                        .permission_scopes
                        .iter()
                        .map(|scope| scope.label())
                        .collect::<Vec<_>>(),
                    meta.capability.execution.timeout_ms,
                    meta.capability.execution.retry_limit,
                    meta.capability.authorized,
                    meta.capability.healthy,
                    meta.source.label(),
                );
            }
            Ok(())
        }
        CapabilityCommand::Jobs => {
            if let Some(url) = attach {
                let client = gateway_client_from_loaded(&loaded, url);
                return print_capability_jobs(&client.list_capability_jobs().await?);
            }
            let gateway = build_gateway_handle(&loaded, None)?;
            print_capability_jobs(&gateway.list_capability_jobs())
        }
        CapabilityCommand::Exec { command } => match command {
            ExecCapabilityCommand::Guardrails => {
                if attach.is_some() {
                    bail!("remote exec guardrails are not supported");
                }
                println!("exec guardrails:");
                println!("  allowed_root: {}", env::current_dir()?.display());
                println!("  permission_scope: local_exec");
                println!("  timeout_policy: tool metadata controlled");
                Ok(())
            }
            ExecCapabilityCommand::Run {
                command,
                args,
                cwd,
                session,
            } => {
                if let Some(url) = attach {
                    let client = gateway_client_from_loaded(&loaded, url);
                    let job = client
                        .run_exec_job(ExecJobRequest {
                            session_id: session,
                            command,
                            args,
                            cwd,
                        })
                        .await?;
                    return print_capability_job(&job);
                }

                let gateway = build_gateway_handle(&loaded, None)?;
                let job = gateway
                    .run_exec_job(ExecJobRequest {
                        session_id: session,
                        command,
                        args,
                        cwd,
                    })
                    .await?;
                print_capability_job(&job)
            }
        },
        CapabilityCommand::Webhook { command } => match command {
            WebhookCapabilityCommand::Test {
                url,
                method,
                body,
                headers,
                session,
            } => {
                let request = WebhookJobRequest {
                    session_id: session,
                    url,
                    method,
                    body,
                    headers: parse_header_args(&headers)?,
                };
                if let Some(url) = attach {
                    let client = gateway_client_from_loaded(&loaded, url);
                    let job = client.run_webhook_job(request).await?;
                    return print_capability_job(&job);
                }

                let gateway = build_gateway_handle(&loaded, None)?;
                let job = gateway.run_webhook_job(request).await?;
                print_capability_job(&job)
            }
        },
    }
}

async fn cron_cmd(attach: Option<String>, command: CronCommand) -> Result<()> {
    let loaded = ensure_loaded_config(None)?;

    match command {
        CronCommand::List => {
            if let Some(url) = attach {
                let client = gateway_client_from_loaded(&loaded, url);
                return print_cron_registrations(&client.list_cron_registrations().await?);
            }
            let gateway = build_gateway_handle(&loaded, None)?;
            let registrations = gateway
                .list_cron_registrations()?
                .iter()
                .map(mosaic_gateway::cron_registration_dto)
                .collect::<Vec<_>>();
            print_cron_registrations(&registrations)
        }
        CronCommand::Register {
            id,
            schedule,
            input,
            session,
            profile,
            skill,
            workflow,
        } => {
            let request = CronRegistrationRequest {
                id,
                schedule,
                input,
                session_id: session,
                profile,
                skill,
                workflow,
            };
            if let Some(url) = attach {
                let client = gateway_client_from_loaded(&loaded, url);
                let registration = client.register_cron(request).await?;
                return print_cron_registrations(&[registration]);
            }
            let gateway = build_gateway_handle(&loaded, None)?;
            let registration = gateway.register_cron(request)?;
            print_cron_registrations(&[mosaic_gateway::cron_registration_dto(&registration)])
        }
        CronCommand::Trigger { id } => {
            if let Some(url) = attach {
                let client = gateway_client_from_loaded(&loaded, url);
                let response = client.trigger_cron(&id).await?;
                return finish_remote_gateway_run(&loaded, response);
            }
            let gateway = build_gateway_handle(&loaded, None)?;
            let result = gateway.trigger_cron(&id).await?;
            finish_successful_gateway_run(result)
        }
    }
}

fn print_capability_jobs(jobs: &[CapabilityJobDto]) -> Result<()> {
    if jobs.is_empty() {
        println!("no capability jobs found");
        return Ok(());
    }

    for job in jobs {
        print_capability_job(job)?;
    }

    Ok(())
}

fn print_capability_job(job: &CapabilityJobDto) -> Result<()> {
    println!(
        "{} | name={} | kind={} | risk={} | status={} | session={} | started_at={} | finished_at={:?}",
        job.id,
        job.name,
        job.kind,
        job.risk,
        job.status,
        job.session_id.as_deref().unwrap_or("-"),
        job.started_at,
        job.finished_at,
    );
    println!("  scopes: {:?}", job.permission_scopes);
    if let Some(summary) = job.summary.as_deref() {
        println!("  summary: {}", truncate_for_cli(summary, 240));
    }
    if let Some(target) = job.target.as_deref() {
        println!("  target: {}", target);
    }
    if let Some(error) = job.error.as_deref() {
        println!("  error: {}", truncate_for_cli(error, 240));
    }
    Ok(())
}

fn print_cron_registrations(registrations: &[CronRegistrationDto]) -> Result<()> {
    if registrations.is_empty() {
        println!("no cron registrations found");
        return Ok(());
    }

    for registration in registrations {
        println!(
            "{} | schedule={} | session={} | profile={:?} | skill={:?} | workflow={:?} | last_triggered_at={:?}",
            registration.id,
            registration.schedule,
            registration.session_id.as_deref().unwrap_or("-"),
            registration.profile,
            registration.skill,
            registration.workflow,
            registration.last_triggered_at,
        );
        println!("  input: {}", truncate_for_cli(&registration.input, 240));
    }

    Ok(())
}

fn parse_header_args(headers: &[String]) -> Result<BTreeMap<String, String>> {
    let mut parsed = BTreeMap::new();
    for header in headers {
        let Some((name, value)) = header.split_once('=') else {
            bail!("invalid header '{}'; expected KEY=VALUE", header);
        };
        parsed.insert(name.trim().to_owned(), value.trim().to_owned());
    }
    Ok(parsed)
}

fn memory_cmd(command: MemoryCommand) -> Result<()> {
    let _loaded = ensure_loaded_config(None)?;
    let store = local_memory_store()?;

    match command {
        MemoryCommand::List => {
            let sessions = store.list_sessions()?;
            if sessions.is_empty() {
                println!("no memory sessions found");
                return Ok(());
            }

            for session in sessions {
                println!(
                    "{} | updated_at={} | related_sessions={} | entries={}",
                    session.session_id,
                    session.updated_at,
                    session.related_sessions.len(),
                    session.entries.len(),
                );
                if let Some(summary) = session.summary.as_deref() {
                    println!("  summary: {}", truncate_for_cli(summary, 200));
                }
                if let Some(compressed) = session.compressed_context.as_deref() {
                    println!("  compressed: {}", truncate_for_cli(compressed, 200));
                }
            }

            Ok(())
        }
        MemoryCommand::Show { session } => {
            let record = store
                .load_session(&session)?
                .ok_or_else(|| anyhow!("memory session not found: {}", session))?;
            println!("session_id: {}", record.session_id);
            println!("updated_at: {}", record.updated_at);
            println!("tags: {:?}", record.tags);
            println!("related_sessions: {:?}", record.related_sessions);
            println!("summary: {:?}", record.summary);
            println!("compressed_context: {:?}", record.compressed_context);
            println!("entry_count: {}", record.entries.len());

            if !record.entries.is_empty() {
                println!(
                    "
entries:"
                );
                for (idx, entry) in record.entries.iter().enumerate() {
                    println!(
                        "[{}] kind={:?} created_at={} tags={:?}",
                        idx + 1,
                        entry.kind,
                        entry.created_at,
                        entry.tags
                    );
                    println!("  {}", truncate_for_cli(&entry.content, 320));
                }
            }

            Ok(())
        }
        MemoryCommand::Search { query, tag } => {
            let hits = store.search(&query, tag.as_deref())?;
            if hits.is_empty() {
                println!("no memory hits found");
                return Ok(());
            }

            for hit in hits {
                println!(
                    "{} | kind={} | updated_at={} | tags={:?}",
                    hit.session_id, hit.kind, hit.updated_at, hit.tags
                );
                println!("  {}", hit.preview);
            }

            Ok(())
        }
    }
}

fn extension_cmd(command: ExtensionCommand) -> Result<()> {
    let loaded = ensure_loaded_config(None)?;
    let gateway = build_gateway_handle(&loaded, None)?;

    match command {
        ExtensionCommand::List => {
            print_extension_snapshot(&gateway.list_extensions(), &gateway.extension_policies());
            Ok(())
        }
        ExtensionCommand::Validate => {
            let report = gateway.validate_extensions()?;
            print_extension_validation_report(&report);
            if report.is_ok() {
                Ok(())
            } else {
                bail!("extension validation failed")
            }
        }
        ExtensionCommand::Reload => {
            let result = gateway.reload_extensions()?;
            println!("extension reload succeeded");
            print_extension_snapshot(&result.extensions, &result.policies);
            Ok(())
        }
    }
}

fn print_extension_snapshot(extensions: &[ExtensionStatus], policies: &PolicyConfig) {
    print_extension_policy_summary(policies);

    if extensions.is_empty() {
        println!("extensions: none");
        return;
    }

    println!("extensions:");
    for extension in extensions {
        println!(
            "- {}@{} | source={} | enabled={} | active={} | tools={} | skills={} | workflows={} | mcp_servers={} | error={}",
            extension.name,
            extension.version,
            extension.source,
            extension.enabled,
            extension.active,
            extension.tools.len(),
            extension.skills.len(),
            extension.workflows.len(),
            extension.mcp_servers.len(),
            extension.error.as_deref().unwrap_or("<none>"),
        );
    }
}

fn print_extension_validation_report(report: &ExtensionValidationReport) {
    print_extension_snapshot(&report.extensions, &report.policies);

    if report.issues.is_empty() {
        println!("validation: ok");
        return;
    }

    println!("validation: failed");
    for issue in &report.issues {
        match issue.extension.as_deref() {
            Some(extension) => println!("- {}: {}", extension, issue.message),
            None => println!("- {}", issue.message),
        }
    }
}

fn print_extension_policy_summary(policies: &PolicyConfig) {
    println!(
        "policies: exec={} webhook={} cron={} mcp={} hot_reload={}",
        policies.allow_exec,
        policies.allow_webhook,
        policies.allow_cron,
        policies.allow_mcp,
        policies.hot_reload_enabled,
    );
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
                    "{} {} | type={} | model={} | family={} | tools={} | sessions={} | context_window_chars={} | budget_tier={} | api_key_env={:?} | api_key_present={}",
                    marker,
                    profile.name,
                    profile.provider_type,
                    profile.model,
                    profile.capabilities.family,
                    profile.capabilities.supports_tools,
                    profile.capabilities.supports_sessions,
                    profile.capabilities.context_window_chars,
                    profile.capabilities.budget_tier,
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
        println!(
            "
effective_profile:"
        );
        println!("  profile: {}", profile.profile);
        println!("  provider_type: {}", profile.provider_type);
        println!("  model: {}", profile.model);
        println!("  base_url: {:?}", profile.base_url);
        println!("  api_key_env: {:?}", profile.api_key_env);
        println!("  api_key_present: {}", profile.api_key_present);
        println!("  timeout_ms: {}", profile.timeout_ms);
        println!("  max_retries: {}", profile.max_retries);
        println!("  supports_tools: {}", profile.supports_tools);
        println!(
            "  supports_tool_call_shadow_messages: {}",
            profile.supports_tool_call_shadow_messages
        );
    }

    if let Some(provider_failure) = &trace.provider_failure {
        println!(
            "
provider_failure:"
        );
        println!("  kind: {}", provider_failure.kind);
        println!("  status_code: {:?}", provider_failure.status_code);
        println!("  retryable: {}", provider_failure.retryable);
        println!("  message: {}", provider_failure.message);
    }

    if let Some(ingress) = &trace.ingress {
        println!("\ningress:");
        println!("  kind: {}", ingress.kind);
        println!("  channel: {:?}", ingress.channel);
        println!("  source: {:?}", ingress.source);
        println!("  remote_addr: {:?}", ingress.remote_addr);
        println!("  display_name: {:?}", ingress.display_name);
        println!("  actor_id: {:?}", ingress.actor_id);
        println!("  thread_id: {:?}", ingress.thread_id);
        println!("  thread_title: {:?}", ingress.thread_title);
        println!("  reply_target: {:?}", ingress.reply_target);
        println!("  gateway_url: {:?}", ingress.gateway_url);
    }

    if let Some(governance) = &trace.governance {
        println!(
            "
governance:"
        );
        println!("  deployment_profile: {}", governance.deployment_profile);
        println!("  workspace_name: {}", governance.workspace_name);
        println!("  auth_mode: {}", governance.auth_mode);
        println!(
            "  audit_retention_days: {}",
            governance.audit_retention_days
        );
        println!("  event_replay_window: {}", governance.event_replay_window);
        println!("  redact_inputs: {}", governance.redact_inputs);
    }

    println!(
        "
summary:"
    );
    println!("  tool_calls: {}", summary.tool_calls);
    println!(
        "  capability_invocations: {}",
        trace.capability_invocations.len()
    );
    println!("  skill_calls: {}", summary.skill_calls);
    println!("  workflow_steps: {}", trace.step_traces.len());
    println!("  provider_attempts: {}", trace.provider_attempts.len());
    println!("  model_selections: {}", trace.model_selections.len());
    println!("  memory_reads: {}", trace.memory_reads.len());
    println!("  memory_writes: {}", trace.memory_writes.len());
    println!("  active_extensions: {}", trace.active_extensions.len());
    println!("  used_extensions: {}", trace.used_extensions.len());
    println!("  compression: {}", trace.compression.is_some());

    if !trace.active_extensions.is_empty() {
        println!(
            "
== active extensions =="
        );
        for extension in &trace.active_extensions {
            println!(
                "{}@{} | source={} | enabled={} | active={} | error={:?}",
                extension.name,
                extension.version,
                extension.source,
                extension.enabled,
                extension.active,
                extension.error,
            );
        }
    }

    if !trace.provider_attempts.is_empty() {
        println!(
            "
== provider attempts =="
        );
        for (idx, attempt) in trace.provider_attempts.iter().enumerate() {
            println!("[{}]", idx + 1);
            println!("  attempt: {}/{}", attempt.attempt, attempt.max_attempts);
            println!("  status: {}", attempt.status);
            println!("  error_kind: {:?}", attempt.error_kind);
            println!("  status_code: {:?}", attempt.status_code);
            println!("  retryable: {}", attempt.retryable);
            println!("  message: {:?}", attempt.message);
        }
    }

    if !trace.used_extensions.is_empty() {
        println!(
            "
== used extensions =="
        );
        for usage in &trace.used_extensions {
            println!(
                "{}@{} | {}:{}",
                usage.name, usage.version, usage.component_kind, usage.component_name,
            );
        }
    }

    if !trace.model_selections.is_empty() {
        println!(
            "
== model selections =="
        );
        for (idx, selection) in trace.model_selections.iter().enumerate() {
            println!("[{}]", idx + 1);
            println!("  scope: {}", selection.scope);
            println!("  requested_profile: {:?}", selection.requested_profile);
            println!("  selected_profile: {}", selection.selected_profile);
            println!("  selected_model: {}", selection.selected_model);
            println!("  reason: {}", selection.reason);
            println!("  context_window_chars: {}", selection.context_window_chars);
            println!("  budget_tier: {}", selection.budget_tier);
        }
    }

    if !trace.memory_reads.is_empty() {
        println!(
            "
== memory reads =="
        );
        for (idx, read) in trace.memory_reads.iter().enumerate() {
            println!("[{}]", idx + 1);
            println!("  session_id: {}", read.session_id);
            println!("  source: {}", read.source);
            println!("  tags: {:?}", read.tags);
            println!("  preview: {}", read.preview);
        }
    }

    if let Some(compression) = &trace.compression {
        println!(
            "
== compression =="
        );
        println!(
            "  original_message_count: {}",
            compression.original_message_count
        );
        println!("  kept_recent_count: {}", compression.kept_recent_count);
        println!("  summary_preview: {}", compression.summary_preview);
    }

    if let Some(side_effect_summary) = &trace.side_effect_summary {
        println!(
            "
== side-effect summary =="
        );
        println!("  total: {}", side_effect_summary.total);
        println!("  failed: {}", side_effect_summary.failed);
        println!("  high_risk: {}", side_effect_summary.high_risk);
        println!(
            "  capability_kinds: {:?}",
            side_effect_summary.capability_kinds
        );
    }

    if !trace.capability_invocations.is_empty() {
        println!(
            "
== capability invocations =="
        );

        for (idx, invocation) in trace.capability_invocations.iter().enumerate() {
            println!("[{}]", idx + 1);
            println!("  job_id: {}", invocation.job_id);
            println!("  call_id: {:?}", invocation.call_id);
            println!("  tool_name: {}", invocation.tool_name);
            println!("  kind: {}", invocation.kind.label());
            println!("  risk: {}", invocation.risk.label());
            println!("  permission_scopes: {:?}", invocation.permission_scopes);
            println!("  status: {}", invocation.status);
            println!("  summary: {}", invocation.summary);
            println!("  target: {:?}", invocation.target);
            println!("  node_id: {:?}", invocation.node_id);
            println!("  capability_route: {:?}", invocation.capability_route);
            println!("  disconnect_context: {:?}", invocation.disconnect_context);
            println!("  started_at: {}", invocation.started_at);
            println!("  finished_at: {:?}", invocation.finished_at);
            println!("  duration_ms: {:?}", invocation.duration_ms());
            println!("  error: {:?}", invocation.error);
        }
    }

    if !trace.memory_writes.is_empty() {
        println!(
            "
== memory writes =="
        );
        for (idx, write) in trace.memory_writes.iter().enumerate() {
            println!("[{}]", idx + 1);
            println!("  session_id: {}", write.session_id);
            println!("  kind: {}", write.kind);
            println!("  tags: {:?}", write.tags);
            println!("  preview: {}", write.preview);
        }
    }

    if let Ok(loaded) = load_config() {
        let redacted = redact_mosaic_config(&loaded.config);
        println!("\ncurrent_workspace_config:");
        println!("  active_profile: {}", redacted.active_profile);
        println!(
            "  deployment: profile={} workspace={}",
            redacted.deployment.profile, redacted.deployment.workspace_name
        );
        println!(
            "  auth: operator_env={:?} operator_present={} webchat_env={:?} webchat_present={} telegram_env={:?} telegram_present={}",
            redacted.auth.operator_token_env,
            redacted.auth.operator_token_present,
            redacted.auth.webchat_shared_secret_env,
            redacted.auth.webchat_shared_secret_present,
            redacted.auth.telegram_secret_token_env,
            redacted.auth.telegram_secret_token_present
        );
        println!(
            "  session_store_root_dir: {}",
            redacted.session_store_root_dir
        );
        println!("  inspect_runs_dir: {}", redacted.inspect_runs_dir);
        println!(
            "  audit: root_dir={} retention_days={} replay_window={} redact_inputs={}",
            redacted.audit.root_dir,
            redacted.audit.retention_days,
            redacted.audit.event_replay_window,
            redacted.audit.redact_inputs
        );
        println!(
            "  observability: metrics={} readiness={} slow_consumer_lag_threshold={}",
            redacted.observability.enable_metrics,
            redacted.observability.enable_readiness,
            redacted.observability.slow_consumer_lag_threshold
        );
        println!(
            "  extension_manifest_count: {}",
            redacted.extension_manifest_count
        );
        println!(
            "  policies: exec={} webhook={} cron={} mcp={} hot_reload={}",
            redacted.policies.allow_exec,
            redacted.policies.allow_webhook,
            redacted.policies.allow_cron,
            redacted.policies.allow_mcp,
            redacted.policies.hot_reload_enabled
        );
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
            println!("  node_id: {:?}", call.node_id);
            println!("  capability_route: {:?}", call.capability_route);
            println!("  disconnect_context: {:?}", call.disconnect_context);
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
    bootstrap::build_local_gateway(tokio::runtime::Handle::current(), loaded, app_cfg)
}

fn gateway_client_from_loaded(
    loaded: &LoadedMosaicConfig,
    url: impl Into<String>,
) -> GatewayClient {
    GatewayClient::new(url.into()).with_bearer_token(operator_token_from_loaded(loaded))
}

fn operator_token_from_loaded(loaded: &LoadedMosaicConfig) -> Option<String> {
    loaded
        .config
        .auth
        .operator_token_env
        .as_deref()
        .and_then(|name| env::var(name).ok())
        .or_else(|| env::var("MOSAIC_OPERATOR_TOKEN").ok())
        .or_else(|| env::var("MOSAIC_GATEWAY_TOKEN").ok())
}

fn save_incident_bundle(
    loaded: &LoadedMosaicConfig,
    bundle: &IncidentBundleDto,
    out: Option<PathBuf>,
) -> Result<PathBuf> {
    let path = out.unwrap_or_else(|| {
        resolve_workspace_relative_path(&loaded.config.audit.root_dir)
            .unwrap_or_else(|_| PathBuf::from(&loaded.config.audit.root_dir))
            .join("incidents")
            .join(format!("{}.json", bundle.identifier))
    });
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&path, serde_json::to_string_pretty(bundle)?)?;
    Ok(path)
}

fn print_gateway_status(
    health: &HealthResponse,
    readiness: &ReadinessResponse,
    metrics: &MetricsResponse,
) -> Result<()> {
    println!(
        "health: status={} active_profile={} deployment={} auth={} sessions={} replay_window={} transport={}",
        health.status,
        health.active_profile,
        health.deployment_profile,
        health.auth_mode,
        health.session_count,
        health.event_replay_window,
        health.transport,
    );
    println!(
        "readiness: status={} session_store_ready={} audit_ready={} replay_buffered={} slow_consumer_threshold={} extensions={}",
        readiness.status,
        readiness.session_store_ready,
        readiness.audit_ready,
        readiness.replay_events_buffered,
        readiness.slow_consumer_lag_threshold,
        readiness.extension_count,
    );
    println!(
        "metrics: completed_runs={} failed_runs={} capability_jobs_total={} audit_events_total={} auth_denials_total={} lagged_events_total={} open_jobs={}",
        metrics.completed_runs_total,
        metrics.failed_runs_total,
        metrics.capability_jobs_total,
        metrics.audit_events_total,
        metrics.auth_denials_total,
        metrics.broadcast_lag_events_total,
        metrics.capability_job_count,
    );
    Ok(())
}

fn print_gateway_audit_events(events: &[GatewayAuditEventDto]) -> Result<()> {
    if events.is_empty() {
        println!("no audit events found");
        return Ok(());
    }

    for event in events {
        println!(
            "{} | kind={} | outcome={} | session={} | gateway_run={} | correlation={} | channel={:?} | actor={:?}",
            event.emitted_at,
            event.kind,
            event.outcome,
            event.session_id.as_deref().unwrap_or("-"),
            event.gateway_run_id.as_deref().unwrap_or("-"),
            event.correlation_id.as_deref().unwrap_or("-"),
            event.channel,
            event.actor,
        );
        println!(
            "  summary: {}{}",
            truncate_for_cli(&event.summary, 240),
            if event.redacted { " [redacted]" } else { "" }
        );
        if let Some(target) = event.target.as_deref() {
            println!("  target: {}", target);
        }
    }
    Ok(())
}

fn print_gateway_replay_window(replay: &ReplayWindowResponse) -> Result<()> {
    println!(
        "replay_window: capacity={} buffered={} dropped_total={}",
        replay.capacity,
        replay.events.len(),
        replay.dropped_events_total,
    );
    if replay.events.is_empty() {
        println!("no replay events captured");
        return Ok(());
    }
    for envelope in &replay.events {
        println!(
            "{} | run={} | corr={} | session={} | route={} | event={}",
            envelope.emitted_at,
            envelope.gateway_run_id,
            envelope.correlation_id,
            envelope.session_id.as_deref().unwrap_or("-"),
            envelope.session_route,
            gateway_event_label(&envelope.event),
        );
    }
    Ok(())
}

fn print_gateway_incident_bundle(bundle: &IncidentBundleDto, path: &Path) -> Result<()> {
    println!("incident: {}", bundle.identifier);
    println!("saved_bundle: {}", path.display());
    println!("generated_at: {}", bundle.generated_at);
    println!("deployment_profile: {}", bundle.deployment_profile);
    println!("auth_mode: {}", bundle.auth_mode);
    println!("redaction_policy: {}", bundle.redaction_policy);
    println!("trace_run_id: {}", bundle.trace.run_id);
    println!("gateway_run_id: {:?}", bundle.trace.gateway_run_id);
    println!("correlation_id: {:?}", bundle.trace.correlation_id);
    println!("audit_events: {}", bundle.audit_events.len());
    println!(
        "metrics: completed_runs={} failed_runs={} audit_events_total={} lagged_events_total={}",
        bundle.metrics.completed_runs_total,
        bundle.metrics.failed_runs_total,
        bundle.metrics.audit_events_total,
        bundle.metrics.broadcast_lag_events_total,
    );
    Ok(())
}

fn print_adapter_statuses(adapters: &[AdapterStatusDto]) -> Result<()> {
    if adapters.is_empty() {
        println!("no adapters found");
        return Ok(());
    }

    for adapter in adapters {
        println!(
            "{} | channel={} | transport={} | path={} | status={} | outbound_ready={}",
            adapter.name,
            adapter.channel,
            adapter.transport,
            adapter.ingress_path,
            adapter.status,
            adapter.outbound_ready,
        );
        println!("  {}", adapter.detail);
    }

    Ok(())
}

fn print_adapter_doctor(adapters: &[AdapterStatusDto]) -> Result<()> {
    println!("adapter doctor:");
    print_adapter_statuses(adapters)?;
    if adapters.iter().any(|adapter| adapter.status == "error") {
        bail!("adapter doctor found errors");
    }
    println!("adapter doctor: ok");
    Ok(())
}

fn print_session_list(sessions: &[SessionSummary]) -> Result<()> {
    if sessions.is_empty() {
        println!("no sessions found");
        return Ok(());
    }

    for session in sessions {
        println!(
            "{} | {} | profile={} | model={} | route={} | messages={} | refs={} | updated_at={} | gateway_run={} | correlation={}",
            session.id,
            session.title,
            session.provider_profile,
            session.model,
            session.session_route,
            session.message_count,
            session.reference_count,
            session.updated_at,
            session.last_gateway_run_id.as_deref().unwrap_or("-"),
            session.last_correlation_id.as_deref().unwrap_or("-"),
        );
        println!(
            "  channel={:?} actor={:?} thread={:?}",
            session.channel_context.channel,
            session
                .channel_context
                .actor_name
                .as_ref()
                .or(session.channel_context.actor_id.as_ref()),
            session
                .channel_context
                .thread_title
                .as_ref()
                .or(session.channel_context.thread_id.as_ref()),
        );
        if let Some(preview) = session.last_message_preview.as_deref() {
            println!("  last: {}", preview);
        }
        if let Some(summary) = session.memory_summary_preview.as_deref() {
            println!("  memory: {}", summary);
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
            "{} | {} | profile={} | model={} | route={} | messages={} | refs={} | updated_at={} | gateway_run={} | correlation={}",
            session.id,
            session.title,
            session.provider_profile,
            session.model,
            session.session_route,
            session.message_count,
            session.reference_count,
            session.updated_at,
            session.last_gateway_run_id.as_deref().unwrap_or("-"),
            session.last_correlation_id.as_deref().unwrap_or("-"),
        );
        println!(
            "  channel={:?} actor={:?} thread={:?}",
            session.channel_context.channel,
            session
                .channel_context
                .actor_name
                .as_ref()
                .or(session.channel_context.actor_id.as_ref()),
            session
                .channel_context
                .thread_title
                .as_ref()
                .or(session.channel_context.thread_id.as_ref()),
        );
        if let Some(preview) = session.last_message_preview.as_deref() {
            println!("  last: {}", preview);
        }
        if let Some(summary) = session.memory_summary_preview.as_deref() {
            println!("  memory: {}", summary);
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
    println!("channel: {:?}", session.channel_context.channel);
    println!("actor_id: {:?}", session.channel_context.actor_id);
    println!("actor_name: {:?}", session.channel_context.actor_name);
    println!("thread_id: {:?}", session.channel_context.thread_id);
    println!("thread_title: {:?}", session.channel_context.thread_title);
    println!("reply_target: {:?}", session.channel_context.reply_target);
    println!(
        "last_gateway_run_id: {:?}",
        session.gateway.last_gateway_run_id
    );
    println!(
        "last_correlation_id: {:?}",
        session.gateway.last_correlation_id
    );
    println!("message_count: {}", session.transcript.len());
    println!("memory_summary: {:?}", session.memory_summary);
    println!("compressed_context: {:?}", session.compressed_context);
    println!("reference_count: {}", session.references.len());

    if !session.references.is_empty() {
        println!("\nreferences:");
        for reference in &session.references {
            println!(
                "- {} | reason={} | created_at={}",
                reference.session_id, reference.reason, reference.created_at
            );
        }
    }

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

fn local_memory_store() -> Result<FileMemoryStore> {
    Ok(FileMemoryStore::new(resolve_workspace_relative_path(
        ".mosaic/memory",
    )?))
}

fn tui_extension_status(loaded: &LoadedMosaicConfig) -> Result<(String, String, Vec<String>)> {
    let report = validate_extension_set(&loaded.config, None, &env::current_dir()?);
    let extension_summary = summarize_extension_names(&report.extensions);
    let policy_summary = format!(
        "Policies exec={} webhook={} cron={} mcp={} hot_reload={}",
        report.policies.allow_exec,
        report.policies.allow_webhook,
        report.policies.allow_cron,
        report.policies.allow_mcp,
        report.policies.hot_reload_enabled,
    );
    let errors = report
        .issues
        .iter()
        .map(|issue| match issue.extension.as_deref() {
            Some(extension) => format!("{}: {}", extension, issue.message),
            None => issue.message.clone(),
        })
        .collect();

    Ok((extension_summary, policy_summary, errors))
}

fn summarize_extension_names(extensions: &[ExtensionStatus]) -> String {
    if extensions.is_empty() {
        return "Extensions none".to_owned();
    }

    let preview = extensions
        .iter()
        .take(3)
        .map(|extension| format!("{}@{}", extension.name, extension.version))
        .collect::<Vec<_>>()
        .join(", ");
    let remaining = extensions.len().saturating_sub(3);

    if remaining == 0 {
        format!("Extensions {}", preview)
    } else {
        format!("Extensions {} (+{} more)", preview, remaining)
    }
}

fn local_cli_ingress(gateway_url: Option<String>) -> IngressTrace {
    IngressTrace {
        kind: "local_cli".to_owned(),
        channel: Some("cli".to_owned()),
        source: Some("mosaic-cli".to_owned()),
        remote_addr: None,
        display_name: None,
        actor_id: None,
        thread_id: None,
        thread_title: None,
        reply_target: None,
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
        actor_id: None,
        thread_id: None,
        thread_title: None,
        reply_target: None,
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
        GatewayEvent::CapabilityJobUpdated { job } => format!(
            "capability_job id={} status={} kind={}",
            job.id, job.status, job.kind
        ),
        GatewayEvent::CronUpdated { registration } => {
            format!(
                "cron_updated id={} schedule={}",
                registration.id, registration.schedule
            )
        }
        GatewayEvent::ExtensionsReloaded {
            extensions,
            policies,
        } => {
            format!(
                "extensions_reloaded count={} exec={} webhook={} cron={} mcp={} hot_reload={}",
                extensions.len(),
                policies.allow_exec,
                policies.allow_webhook,
                policies.allow_cron,
                policies.allow_mcp,
                policies.hot_reload_enabled,
            )
        }
        GatewayEvent::ExtensionReloadFailed { error } => {
            format!(
                "extension_reload_failed error={}",
                truncate_for_cli(error, 80)
            )
        }
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
        "  deployment: profile={} workspace={}",
        redacted.deployment.profile, redacted.deployment.workspace_name
    );
    println!(
        "  auth: operator_env={:?} operator_present={} webchat_env={:?} webchat_present={} telegram_env={:?} telegram_present={}",
        redacted.auth.operator_token_env,
        redacted.auth.operator_token_present,
        redacted.auth.webchat_shared_secret_env,
        redacted.auth.webchat_shared_secret_present,
        redacted.auth.telegram_secret_token_env,
        redacted.auth.telegram_secret_token_present
    );
    println!(
        "  session_store_root_dir: {}",
        redacted.session_store_root_dir
    );
    println!("  inspect_runs_dir: {}", redacted.inspect_runs_dir);
    println!(
        "  audit: root_dir={} retention_days={} replay_window={} redact_inputs={}",
        redacted.audit.root_dir,
        redacted.audit.retention_days,
        redacted.audit.event_replay_window,
        redacted.audit.redact_inputs
    );
    println!(
        "  observability: metrics={} readiness={} slow_consumer_lag_threshold={}",
        redacted.observability.enable_metrics,
        redacted.observability.enable_readiness,
        redacted.observability.slow_consumer_lag_threshold
    );
    println!(
        "  extension_manifest_count: {}",
        redacted.extension_manifest_count
    );
    println!(
        "  policies: exec={} webhook={} cron={} mcp={} hot_reload={}",
        redacted.policies.allow_exec,
        redacted.policies.allow_webhook,
        redacted.policies.allow_cron,
        redacted.policies.allow_mcp,
        redacted.policies.hot_reload_enabled
    );
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
    use std::{fs, path::PathBuf};

    use clap::{CommandFactory, Parser};
    use mosaic_tool_core::ToolSource;

    use super::{
        Cli, DispatchCommand, ExtensionCommand, MemoryCommand, ModelCommand, SessionCommand,
        SetupCommand,
    };

    fn repo_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("cli crate should live under repo root")
            .to_path_buf()
    }

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
    fn parses_memory_subcommands() {
        let cli = Cli::parse_from(["mosaic", "memory", "show", "demo"]);
        assert_eq!(
            cli.dispatch(),
            DispatchCommand::Memory {
                command: MemoryCommand::Show {
                    session: "demo".to_owned(),
                },
            }
        );

        let cli = Cli::parse_from(["mosaic", "memory", "search", "summary", "--tag", "note"]);
        assert_eq!(
            cli.dispatch(),
            DispatchCommand::Memory {
                command: MemoryCommand::Search {
                    query: "summary".to_owned(),
                    tag: Some("note".to_owned()),
                },
            }
        );
    }

    #[test]
    fn parses_extension_subcommands() {
        let cli = Cli::parse_from(["mosaic", "extension", "reload"]);
        assert_eq!(
            cli.dispatch(),
            DispatchCommand::Extension {
                command: ExtensionCommand::Reload,
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
                attach: None,
                command: super::GatewayCliCommand::Serve {
                    local: false,
                    http: Some("127.0.0.1:8080".to_owned()),
                },
            }
        );
    }

    #[test]
    fn parses_node_subcommands() {
        let cli = Cli::parse_from(["mosaic", "node", "list"]);
        assert_eq!(
            cli.dispatch(),
            DispatchCommand::Node {
                command: super::NodeCliCommand::List,
            }
        );

        let cli = Cli::parse_from(["mosaic", "node", "attach", "node-a", "--session", "demo"]);
        assert_eq!(
            cli.dispatch(),
            DispatchCommand::Node {
                command: super::NodeCliCommand::Attach {
                    node_id: "node-a".to_owned(),
                    session: Some("demo".to_owned()),
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
    fn top_level_help_mentions_quick_start_and_docs() {
        let help = Cli::command().render_long_help().to_string();
        assert!(help.contains("mosaic setup init"));
        assert!(help.contains("docs/getting-started.md"));
        assert!(help.contains("examples/providers/openai.yaml"));
    }

    #[test]
    fn readme_and_docs_reference_first_run_commands() {
        let root = repo_root();
        let readme = fs::read_to_string(root.join("README.md")).expect("README should load");
        for required in [
            "mosaic setup init",
            "mosaic setup validate",
            "mosaic setup doctor",
            "mosaic tui",
            "mosaic session list",
            "mosaic inspect .mosaic/runs/<run-id>.json",
            "docs/getting-started.md",
            "docs/cli.md",
            "examples/README.md",
        ] {
            assert!(readme.contains(required), "README missing {required}");
        }

        let getting_started =
            fs::read_to_string(root.join("docs/getting-started.md")).expect("guide should load");
        for required in [
            "mosaic setup init",
            "mosaic setup validate",
            "mosaic setup doctor",
            "mosaic model list",
            "mosaic tui",
            "mosaic inspect .mosaic/runs/<run-id>.json",
        ] {
            assert!(
                getting_started.contains(required),
                "getting-started guide missing {required}"
            );
        }
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
