use std::{
    env, fs, io,
    path::{Path, PathBuf},
    sync::Arc,
};

use anyhow::{Context, Result, anyhow, bail};
use clap::{Parser, Subcommand, ValueEnum};
use mosaic_config::{
    ACTIVE_PROFILE_ENV, DEFAULT_PRODUCT_ACTIVE_PROFILE, DEV_MOCK_PROFILE,
    InitWorkspaceConfigOptions, LoadConfigOptions, LoadedMosaicConfig, PolicyConfig, ProviderUsage,
    ValidationLevel, doctor_mosaic_config, init_workspace_config, load_mosaic_config,
    redact_mosaic_config, save_mosaic_config, validate_mosaic_config,
};
use mosaic_control_protocol::{
    AdapterStatusDto, CapabilityJobDto, CronRegistrationDto, GatewayAuditEventDto, GatewayEvent,
    HealthResponse, IncidentBundleDto, IngressTrace, MetricsResponse, ReadinessResponse,
    ReplayWindowResponse, RunDetailDto, RunResponse, RunSummaryDto, SessionSummaryDto,
};
use mosaic_extension_core::{ExtensionStatus, ExtensionValidationReport, validate_extension_set};
use mosaic_gateway::{GatewayHandle, GatewayRunError, GatewayRunResult};
use mosaic_inspect::RunTrace;
use mosaic_memory::{FileMemoryStore, MemoryStore};
use mosaic_provider::ProviderProfileRegistry;
use mosaic_runtime::events::RunEventSink;
use mosaic_sdk::GatewayClient;
use mosaic_session_core::SessionSummary;
use mosaic_tool_core::Tool;
use tracing_subscriber::EnvFilter;

mod automation_cmd;
mod bootstrap;
mod gateway_cmd;
mod inspect_cmd;
mod node_cmd;
mod output;
mod run_cmd;
mod session_cmd;

const CLI_ABOUT: &str = "Self-hosted AI assistant control plane for sessions, the TUI, Gateway routing, and trace inspection.";
const CLI_AFTER_HELP: &str = "Quick start:
  mosaic setup init
  mosaic setup validate
  mosaic setup doctor
  mosaic config show
  mosaic model list
  mosaic tui          # main operator console
  mosaic run <app> --tui   # single-run observer

Operator groups:
  setup/config         bootstrap and explain merged configuration
  tui/run/session/model manage conversations and provider routing
  inspect              explain one saved run
  gateway/adapter/node operate the control plane edges and devices
  capability/cron/extension/memory operate automations and state

Docs:
  docs/getting-started.md
  docs/configuration.md
  docs/cli.md
  docs/deployment.md
  docs/security.md
  docs/release.md

Examples:
  examples/providers/openai.yaml
  examples/workflows/research-brief.yaml
  examples/deployment/production.config.yaml

Delivery:
  make smoke
  make release-check
  make package";
const SETUP_AFTER_HELP: &str = "When to use it:
  Use `setup` before first run, after config edits, or when an operator needs to diagnose why Mosaic will not start cleanly.

Examples:
  mosaic setup init
  mosaic setup init --profile anthropic-sonnet
  mosaic setup init --dev-mock
  mosaic setup validate
  mosaic setup doctor";
const CONFIG_AFTER_HELP: &str = "When to use it:
  Use `config` when you need to explain which merged settings Mosaic will actually run and which layer supplied them.

Examples:
  mosaic config show
  mosaic config sources
  mosaic config show --json";
const GATEWAY_AFTER_HELP: &str = "When to use it:
  Use `gateway` to inspect control-plane health, replay audit state, or expose the local Gateway over HTTP.

Examples:
  mosaic gateway status
  mosaic gateway serve --local
  mosaic gateway serve --http 127.0.0.1:8080
  mosaic gateway runs
  mosaic gateway show-run <gateway-run-id>
  mosaic gateway cancel <gateway-run-id>
  mosaic gateway retry <gateway-run-id>
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
    #[arg(
        long,
        global = true,
        help = "Start the TUI or TUI-backed flow in the session resume browser"
    )]
    resume: bool,
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Initialize workspace state and diagnose configuration problems before operating Mosaic.
    Setup {
        #[command(subcommand)]
        command: SetupCommand,
    },
    /// Show the merged operator config and the layers that supplied it.
    Config {
        #[command(subcommand)]
        command: ConfigCommand,
    },
    /// Start the main operator chat console.
    Tui {
        #[arg(
            long,
            help = "Resume or create this session as the active operator conversation"
        )]
        session: Option<String>,
        #[arg(
            long,
            help = "Use this provider profile for future turns submitted from the TUI"
        )]
        profile: Option<String>,
        #[arg(
            long,
            help = "Attach the operator console to a remote HTTP Gateway instead of the local workspace Gateway"
        )]
        attach: Option<String>,
    },
    /// Execute one app YAML run. Add --tui to watch a single run in the terminal observer.
    Run {
        file: PathBuf,
        #[arg(long, conflicts_with = "workflow")]
        skill: Option<String>,
        #[arg(long, conflicts_with = "skill")]
        workflow: Option<String>,
        #[arg(
            long,
            help = "Bind this run to a session id so transcript, memory, and routing persist across turns"
        )]
        session: Option<String>,
        #[arg(
            long,
            help = "Override the provider profile used for this run or TUI session"
        )]
        profile: Option<String>,
        #[arg(
            long,
            help = "Attach this run to a remote HTTP Gateway instead of the local workspace Gateway"
        )]
        attach: Option<String>,
        #[arg(
            long,
            help = "Show the terminal observer while this single file-backed run executes; use `mosaic tui` for the long-lived operator console"
        )]
        tui: bool,
    },
    /// List persisted operator conversations or inspect one saved transcript.
    Session {
        #[arg(long)]
        attach: Option<String>,
        #[command(subcommand)]
        command: SessionCommand,
    },
    /// List configured provider profiles or switch the active one for future turns.
    Model {
        #[command(subcommand)]
        command: ModelCommand,
    },
    /// Summarize one saved run trace. Add --verbose for provider, tool, workflow, and memory detail.
    Inspect {
        file: PathBuf,
        #[arg(
            long,
            help = "Include provider, tool, workflow, memory, and governance details"
        )]
        verbose: bool,
        #[arg(long, help = "Print the saved trace as machine-readable JSON")]
        json: bool,
    },
    /// Inspect or serve the control-plane Gateway.
    Gateway {
        #[arg(long)]
        attach: Option<String>,
        #[command(subcommand)]
        command: GatewayCliCommand,
    },
    /// Inspect channel adapter readiness and ingress exposure.
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
    /// Validate and reload extension manifests.
    Extension {
        #[command(subcommand)]
        command: ExtensionCommand,
    },
    /// Inspect saved memory summaries and cross-session references.
    Memory {
        #[command(subcommand)]
        command: MemoryCommand,
    },
}

#[derive(Debug, Subcommand, PartialEq, Eq)]
#[command(after_help = SETUP_AFTER_HELP)]
enum SetupCommand {
    Init {
        #[arg(long)]
        force: bool,
        #[arg(
            long,
            help = "Use this built-in provider profile as the initial active profile"
        )]
        profile: Option<String>,
        #[arg(
            long,
            conflicts_with = "profile",
            help = "Generate a dev-only mock template instead of the real-provider-first default"
        )]
        dev_mock: bool,
    },
    Validate,
    Doctor,
}

#[derive(Debug, Subcommand, PartialEq, Eq)]
#[command(after_help = CONFIG_AFTER_HELP)]
enum ConfigCommand {
    Show {
        #[arg(
            long,
            help = "Print the merged operator config as machine-readable JSON"
        )]
        json: bool,
    },
    Sources {
        #[arg(long, help = "Print the config source stack as machine-readable JSON")]
        json: bool,
    },
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
    Runs,
    ShowRun {
        id: String,
    },
    Cancel {
        id: String,
    },
    Retry {
        id: String,
    },
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
    Setup {
        command: SetupCommand,
    },
    Config {
        command: ConfigCommand,
    },
    Session {
        attach: Option<String>,
        command: SessionCommand,
    },
    Model {
        command: ModelCommand,
    },
    Inspect {
        file: PathBuf,
        verbose: bool,
        json: bool,
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
            Some(Commands::Setup { command }) => DispatchCommand::Setup { command },
            Some(Commands::Config { command }) => DispatchCommand::Config { command },
            Some(Commands::Session { attach, command }) => {
                DispatchCommand::Session { attach, command }
            }
            Some(Commands::Model { command }) => DispatchCommand::Model { command },
            Some(Commands::Inspect {
                file,
                verbose,
                json,
            }) => DispatchCommand::Inspect {
                file,
                verbose,
                json,
            },
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
        } => run_cmd::run_cmd(file, skill, workflow, session, profile, attach, tui, resume).await,
        DispatchCommand::Setup { command } => setup_cmd(command),
        DispatchCommand::Config { command } => config_cmd(command),
        DispatchCommand::Session { attach, command } => {
            session_cmd::session_cmd(attach, command).await
        }
        DispatchCommand::Model { command } => model_cmd(command),
        DispatchCommand::Inspect {
            file,
            verbose,
            json,
        } => inspect_cmd::inspect_cmd(file, verbose, json),
        DispatchCommand::Gateway { attach, command } => {
            gateway_cmd::gateway_cmd(attach, command).await
        }
        DispatchCommand::Adapter { attach, command } => adapter_cmd(attach, command).await,
        DispatchCommand::Node { command } => node_cmd::node_cmd(command).await,
        DispatchCommand::Capability { attach, command } => {
            automation_cmd::capability_cmd(attach, command).await
        }
        DispatchCommand::Cron { attach, command } => {
            automation_cmd::cron_cmd(attach, command).await
        }
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

fn finish_gateway_outcome(
    outcome: std::result::Result<GatewayRunResult, GatewayRunError>,
) -> Result<()> {
    match outcome {
        Ok(result) => run_cmd::finish_successful_gateway_run(result),
        Err(err) => finish_failed_gateway_run(err),
    }
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
    let rendered = output::render_next_steps(steps);
    if !rendered.is_empty() {
        print!("{}", rendered);
    }
}

fn setup_cmd(command: SetupCommand) -> Result<()> {
    match command {
        SetupCommand::Init {
            force,
            profile,
            dev_mock,
        } => {
            let cwd = env::current_dir()?;
            let selected_profile = if dev_mock {
                DEV_MOCK_PROFILE.to_owned()
            } else {
                profile
                    .clone()
                    .unwrap_or_else(|| DEFAULT_PRODUCT_ACTIVE_PROFILE.to_owned())
            };
            let dev_mock_mode = selected_profile == DEV_MOCK_PROFILE;
            let path = init_workspace_config(
                &cwd,
                &InitWorkspaceConfigOptions {
                    force,
                    active_profile: profile,
                    dev_mock,
                },
            )?;
            println!("workspace initialized");
            println!("config_path: {}", path.display());
            println!("active_profile: {}", selected_profile);
            println!(
                "setup_mode: {}",
                if dev_mock_mode {
                    "dev-only-mock"
                } else {
                    "real-provider-first"
                }
            );
            if dev_mock_mode {
                print_next_steps([
                    "mosaic setup validate",
                    "mosaic setup doctor",
                    "mosaic model list",
                    "mosaic tui",
                ]);
            } else {
                print_next_steps([
                    "mosaic setup validate",
                    "mosaic setup doctor",
                    "mosaic model list",
                    "mosaic tui",
                ]);
            }
            Ok(())
        }
        SetupCommand::Validate => {
            let loaded = load_config()?;
            let report = validate_mosaic_config(&loaded.config);
            println!("{}", output::render_config_show(&loaded, &report));

            if report.has_errors() {
                bail!(
                    "configuration validation failed
next: run `mosaic setup doctor` and compare `.mosaic/config.yaml` with `docs/configuration.md`"
                )
            }

            print_next_steps(["mosaic setup doctor", "mosaic config sources", "mosaic tui"]);
            Ok(())
        }
        SetupCommand::Doctor => {
            let loaded = load_config()?;
            let doctor = doctor_mosaic_config(&loaded.config, &current_dir()?);
            println!(
                "{}",
                output::render_config_show(&loaded, &doctor.validation)
            );
            println!();
            println!(
                "{}",
                output::render_doctor_report(&doctor, &redact_mosaic_config(&loaded.config))
            );

            if doctor.has_errors() {
                bail!(
                    "configuration doctor found errors
next: fix the reported checks, then rerun `mosaic setup doctor`"
                )
            }

            print_next_steps(["mosaic model list", "mosaic tui", "mosaic gateway status"]);
            Ok(())
        }
    }
}

fn config_cmd(command: ConfigCommand) -> Result<()> {
    let loaded = load_config()?;
    let validation = validate_mosaic_config(&loaded.config);

    match command {
        ConfigCommand::Show { json } => {
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&serde_json::json!({
                        "workspace_config_path": loaded.workspace_config_path,
                        "user_config_path": loaded.user_config_path,
                        "sources": loaded.sources,
                        "config": redact_mosaic_config(&loaded.config),
                        "onboarding": output::render_onboarding_json(&loaded, &validation),
                        "validation": validation,
                    }))?
                );
            } else {
                println!("{}", output::render_config_show(&loaded, &validation));
                print_next_steps(["mosaic config sources", "mosaic setup doctor"]);
            }
            Ok(())
        }
        ConfigCommand::Sources { json } => {
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&serde_json::json!({
                        "active_profile": loaded.config.active_profile,
                        "workspace_config_path": loaded.workspace_config_path,
                        "user_config_path": loaded.user_config_path,
                        "sources": loaded.sources,
                        "config": redact_mosaic_config(&loaded.config),
                        "onboarding": output::render_onboarding_json(&loaded, &validation),
                        "validation": validation,
                    }))?
                );
            } else {
                println!("{}", output::render_config_sources(&loaded, &validation));
                print_next_steps(["mosaic config show", "mosaic setup validate"]);
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
            let profiles = registry.list();
            let validation = validate_mosaic_config(&loaded.config);
            let active = registry.active_profile();
            let active_state = if active.usage == ProviderUsage::DevOnlyMock {
                "dev-mock"
            } else if validation.issues.iter().any(|issue| {
                issue.field == "active_profile"
                    || issue
                        .field
                        .starts_with(&format!("profiles.{}.", active.name))
            }) {
                "pending-provider-configuration"
            } else if active.api_key_env.is_some() && !active.api_key_present() {
                "pending-provider-credentials"
            } else {
                "ready"
            };

            println!("model summary:");
            println!("  active_profile: {}", loaded.config.active_profile);
            println!("  active_profile_usage: {}", active.usage.label());
            println!("  onboarding_state: {}", active_state);
            println!("  profile_count: {}", profiles.len());
            println!("profiles:");
            for profile in profiles {
                let marker = if profile.name == loaded.config.active_profile {
                    '*'
                } else {
                    ' '
                };
                println!(
                    "  {} {} | usage={} | type={} | model={} | family={} | tools={} | sessions={} | context_window_chars={} | budget_tier={} | api_key_env={:?} | api_key_present={}",
                    marker,
                    profile.name,
                    profile.usage.label(),
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

            println!("model updated");
            println!("  active_profile: {}", profile);
            println!(
                "  workspace_config: {}",
                loaded.workspace_config_path.display()
            );

            if env::var(ACTIVE_PROFILE_ENV).is_ok() {
                println!(
                    "note: {} is currently set and will still override the workspace profile at runtime",
                    ACTIVE_PROFILE_ENV
                );
            }

            print_next_steps(["mosaic model list", "mosaic tui"]);
            Ok(())
        }
    }
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
        "{}",
        output::render_gateway_status(health, readiness, metrics)
    );
    Ok(())
}

fn print_run_list(runs: &[RunSummaryDto]) -> Result<()> {
    println!("run summary:");
    println!("  total: {}", runs.len());
    if runs.is_empty() {
        return Ok(());
    }

    println!("runs:");
    for run in runs {
        println!(
            "  - {} | status={} | session={} | route={} | profile={} | model={} | retry_of={} | finished_at={}",
            run.gateway_run_id,
            run.status.label(),
            run.session_id.as_deref().unwrap_or("-"),
            run.session_route,
            run.effective_profile
                .as_deref()
                .or(run.requested_profile.as_deref())
                .unwrap_or("-"),
            run.effective_model.as_deref().unwrap_or("-"),
            run.retry_of.as_deref().unwrap_or("-"),
            run.finished_at
                .map(|value| value.to_rfc3339())
                .unwrap_or_else(|| "-".to_owned()),
        );
        println!(
            "    correlation={} run_id={} provider={} failure_kind={}",
            run.correlation_id,
            run.run_id,
            run.effective_provider_type.as_deref().unwrap_or("-"),
            run.failure_kind.as_deref().unwrap_or("-"),
        );
        println!(
            "    input={} output={} error={}",
            truncate_for_cli(&run.input_preview, 120),
            run.output_preview
                .as_deref()
                .map(|value| truncate_for_cli(value, 120))
                .unwrap_or_else(|| "-".to_owned()),
            run.error
                .as_deref()
                .map(|value| truncate_for_cli(value, 120))
                .unwrap_or_else(|| "-".to_owned()),
        );
    }

    Ok(())
}

fn print_run_detail(run: &RunDetailDto) -> Result<()> {
    let summary = &run.summary;
    println!("gateway_run_id: {}", summary.gateway_run_id);
    println!("correlation_id: {}", summary.correlation_id);
    println!("run_id: {}", summary.run_id);
    println!("status: {}", summary.status.label());
    println!("session_id: {:?}", summary.session_id);
    println!("session_route: {}", summary.session_route);
    println!("requested_profile: {:?}", summary.requested_profile);
    println!("effective_profile: {:?}", summary.effective_profile);
    println!(
        "effective_provider_type: {:?}",
        summary.effective_provider_type
    );
    println!("effective_model: {:?}", summary.effective_model);
    println!("skill: {:?}", summary.skill);
    println!("workflow: {:?}", summary.workflow);
    println!("retry_of: {:?}", summary.retry_of);
    println!("created_at: {}", summary.created_at);
    println!("updated_at: {}", summary.updated_at);
    println!("finished_at: {:?}", summary.finished_at);
    println!("failure_kind: {:?}", summary.failure_kind);
    println!("trace_path: {:?}", summary.trace_path);
    println!("ingress: {:?}", run.ingress);
    println!("outbound_deliveries: {}", run.outbound_deliveries.len());
    println!("input: {}", truncate_for_cli(&summary.input_preview, 240));
    println!(
        "output: {}",
        run.summary
            .output_preview
            .as_deref()
            .map(|value| truncate_for_cli(value, 240))
            .unwrap_or_else(|| "-".to_owned())
    );
    println!(
        "error: {}",
        run.summary
            .error
            .as_deref()
            .map(|value| truncate_for_cli(value, 240))
            .unwrap_or_else(|| "-".to_owned())
    );
    println!("submission_system: {:?}", run.submission.system);
    println!("submission_skill: {:?}", run.submission.skill);
    println!("submission_workflow: {:?}", run.submission.workflow);
    println!("submission_session_id: {:?}", run.submission.session_id);
    println!("submission_profile: {:?}", run.submission.profile);
    for (index, delivery) in run.outbound_deliveries.iter().enumerate() {
        println!(
            "delivery[{index}]: channel={} adapter={} target={} status={} retries={} error_kind={:?} error={:?}",
            delivery.message.channel,
            delivery.message.adapter,
            delivery.message.reply_target,
            delivery.result.status.label(),
            delivery.result.retry_count,
            delivery.result.error_kind,
            delivery.result.error
        );
    }
    Ok(())
}

fn print_gateway_audit_events(events: &[GatewayAuditEventDto]) -> Result<()> {
    if events.is_empty() {
        println!("audit summary:");
        println!("  events: 0");
        return Ok(());
    }

    println!("audit summary:");
    println!("  events: {}", events.len());
    println!("  latest_at: {}", events[0].emitted_at);
    println!("events:");
    for event in events {
        println!(
            "  - {} | kind={} | outcome={} | session={} | gateway_run={} | correlation={} | channel={:?} | actor={:?}",
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
            "    summary: {}{}",
            truncate_for_cli(&event.summary, 240),
            if event.redacted { " [redacted]" } else { "" }
        );
        if let Some(target) = event.target.as_deref() {
            println!("    target: {}", target);
        }
    }
    Ok(())
}

fn print_gateway_replay_window(replay: &ReplayWindowResponse) -> Result<()> {
    println!("replay summary:");
    println!("  capacity: {}", replay.capacity);
    println!("  buffered: {}", replay.events.len());
    println!("  dropped_total: {}", replay.dropped_events_total);
    if replay.events.is_empty() {
        return Ok(());
    }
    println!("events:");
    for envelope in &replay.events {
        println!(
            "  - {} | run={} | corr={} | session={} | route={} | event={}",
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
    if let Some(run) = &bundle.run {
        println!("run_status: {}", run.summary.status.label());
        println!("run_failure_kind: {:?}", run.summary.failure_kind);
    }
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
    println!("adapter summary:");
    println!("  adapters: {}", adapters.len());
    println!(
        "  errors: {}",
        adapters
            .iter()
            .filter(|adapter| adapter.status == "error")
            .count()
    );
    println!(
        "  warnings: {}",
        adapters
            .iter()
            .filter(|adapter| adapter.status == "warning")
            .count()
    );
    if adapters.is_empty() {
        return Ok(());
    }

    println!("adapters:");
    for adapter in adapters {
        println!(
            "  - {} | channel={} | transport={} | path={} | status={} | outbound_ready={}",
            adapter.name,
            adapter.channel,
            adapter.transport,
            adapter.ingress_path,
            adapter.status,
            adapter.outbound_ready,
        );
        if !adapter.capabilities.is_empty() {
            println!("    capabilities: {}", adapter.capabilities.join(", "));
        }
        println!("    {}", adapter.detail);
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
    println!("session summary:");
    println!("  source: local");
    println!("  total: {}", sessions.len());
    if sessions.is_empty() {
        return Ok(());
    }

    println!("sessions:");
    for session in sessions {
        println!(
            "  - {} | {} | profile={} | model={} | route={} | messages={} | refs={} | run_status={} | current_run={} | updated_at={} | gateway_run={} | correlation={}",
            session.id,
            session.title,
            session.provider_profile,
            session.model,
            session.session_route,
            session.message_count,
            session.reference_count,
            session.run.status.label(),
            session.run.current_run_id.as_deref().unwrap_or("-"),
            session.updated_at,
            session.last_gateway_run_id.as_deref().unwrap_or("-"),
            session.last_correlation_id.as_deref().unwrap_or("-"),
        );
        println!(
            "    channel={:?} actor={:?} thread={:?}",
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
            println!("    last: {}", preview);
        }
        if let Some(summary) = session.memory_summary_preview.as_deref() {
            println!("    memory: {}", summary);
        }
        if let Some(error) = session.run.last_error.as_deref() {
            println!(
                "    run_error(kind={}): {}",
                session.run.last_failure_kind.as_deref().unwrap_or("-"),
                truncate_for_cli(error, 160)
            );
        }
    }

    Ok(())
}

fn print_remote_session_list(sessions: &[SessionSummaryDto]) -> Result<()> {
    println!("session summary:");
    println!("  source: remote");
    println!("  total: {}", sessions.len());
    if sessions.is_empty() {
        return Ok(());
    }

    println!("sessions:");
    for session in sessions {
        println!(
            "  - {} | {} | profile={} | model={} | route={} | messages={} | refs={} | run_status={} | current_run={} | updated_at={} | gateway_run={} | correlation={}",
            session.id,
            session.title,
            session.provider_profile,
            session.model,
            session.session_route,
            session.message_count,
            session.reference_count,
            session.run.status.label(),
            session.run.current_run_id.as_deref().unwrap_or("-"),
            session.updated_at,
            session.last_gateway_run_id.as_deref().unwrap_or("-"),
            session.last_correlation_id.as_deref().unwrap_or("-"),
        );
        println!(
            "    channel={:?} actor={:?} thread={:?}",
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
            println!("    last: {}", preview);
        }
        if let Some(summary) = session.memory_summary_preview.as_deref() {
            println!("    memory: {}", summary);
        }
        if let Some(error) = session.run.last_error.as_deref() {
            println!(
                "    run_error(kind={}): {}",
                session.run.last_failure_kind.as_deref().unwrap_or("-"),
                truncate_for_cli(error, 160)
            );
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
        adapter: Some("cli_local".to_owned()),
        source: Some("mosaic-cli".to_owned()),
        remote_addr: None,
        display_name: None,
        actor_id: None,
        conversation_id: None,
        thread_id: None,
        thread_title: None,
        reply_target: None,
        message_id: None,
        received_at: None,
        raw_event_id: None,
        session_hint: None,
        profile_hint: None,
        control_command: None,
        original_text: None,
        gateway_url,
    }
}

fn remote_cli_ingress(gateway_url: &str) -> IngressTrace {
    IngressTrace {
        kind: "remote_operator".to_owned(),
        channel: Some("cli".to_owned()),
        adapter: Some("cli_remote".to_owned()),
        source: Some("mosaic-cli".to_owned()),
        remote_addr: None,
        display_name: None,
        actor_id: None,
        conversation_id: None,
        thread_id: None,
        thread_title: None,
        reply_target: None,
        message_id: None,
        received_at: None,
        raw_event_id: None,
        session_hint: None,
        profile_hint: None,
        control_command: None,
        original_text: None,
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
        GatewayEvent::InboundReceived {
            ingress,
            text_preview,
        } => format!(
            "inbound_received channel={} adapter={} conversation={} preview={}",
            ingress.channel.as_deref().unwrap_or("-"),
            ingress.adapter.as_deref().unwrap_or("-"),
            ingress.conversation_id.as_deref().unwrap_or("-"),
            truncate_for_cli(text_preview, 80),
        ),
        GatewayEvent::RunSubmitted { profile, .. } => format!("run_submitted profile={profile}"),
        GatewayEvent::Runtime(_) => "runtime_event".to_owned(),
        GatewayEvent::RunUpdated { run } => format!(
            "run_updated id={} status={} failure_kind={}",
            run.gateway_run_id,
            run.status.label(),
            run.failure_kind.as_deref().unwrap_or("-"),
        ),
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
        GatewayEvent::OutboundDelivered { delivery } => format!(
            "outbound_delivered channel={} target={} provider_message_id={}",
            delivery.message.channel,
            delivery.message.reply_target,
            delivery
                .result
                .provider_message_id
                .as_deref()
                .unwrap_or("-"),
        ),
        GatewayEvent::OutboundFailed { delivery } => format!(
            "outbound_failed channel={} target={} error_kind={} error={}",
            delivery.message.channel,
            delivery.message.reply_target,
            delivery.result.error_kind.as_deref().unwrap_or("-"),
            truncate_for_cli(delivery.result.error.as_deref().unwrap_or("-"), 80),
        ),
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

fn validation_level_label(level: &ValidationLevel) -> &'static str {
    match level {
        ValidationLevel::Error => "error",
        ValidationLevel::Warning => "warning",
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
        Cli, ConfigCommand, DispatchCommand, ExtensionCommand, MemoryCommand, ModelCommand,
        SessionCommand, SetupCommand,
    };

    fn repo_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("cli crate should live under repo root")
            .to_path_buf()
    }

    const REQUIRED_CRATE_README_SECTIONS: &[&str] = &[
        "## Positioning",
        "## Architecture Layer",
        "## Responsibilities",
        "## Out of Scope",
        "## Public Boundary",
        "## Why This Is In `crates/`",
        "## Relationships",
        "## Minimal Use",
        "## Testing",
        "## Current Limitations",
        "## Roadmap",
    ];

    const CRATE_READMES: &[(&str, &str)] = &[
        (
            "crates/channel-telegram/README.md",
            "mosaic-channel-telegram",
        ),
        ("crates/config/README.md", "mosaic-config"),
        (
            "crates/control-protocol/README.md",
            "mosaic-control-protocol",
        ),
        ("crates/extension-core/README.md", "mosaic-extension-core"),
        ("crates/gateway/README.md", "mosaic-gateway"),
        ("crates/inspect/README.md", "mosaic-inspect"),
        ("crates/mcp-core/README.md", "mosaic-mcp-core"),
        ("crates/memory/README.md", "mosaic-memory"),
        ("crates/node-protocol/README.md", "mosaic-node-protocol"),
        ("crates/provider/README.md", "mosaic-provider"),
        ("crates/runtime/README.md", "mosaic-runtime"),
        ("crates/scheduler-core/README.md", "mosaic-scheduler-core"),
        ("crates/sdk/README.md", "mosaic-sdk"),
        ("crates/session-core/README.md", "mosaic-session-core"),
        ("crates/skill-core/README.md", "mosaic-skill-core"),
        ("crates/tool-core/README.md", "mosaic-tool-core"),
        ("crates/tui/README.md", "mosaic-tui"),
        ("crates/workflow/README.md", "mosaic-workflow"),
    ];

    const ROOT_CRATE_GUIDE_LINKS: &[&str] = &[
        "./cli/README.md",
        "./crates/channel-telegram/README.md",
        "./crates/config/README.md",
        "./crates/control-protocol/README.md",
        "./crates/extension-core/README.md",
        "./crates/gateway/README.md",
        "./crates/inspect/README.md",
        "./crates/mcp-core/README.md",
        "./crates/memory/README.md",
        "./crates/node-protocol/README.md",
        "./crates/provider/README.md",
        "./crates/runtime/README.md",
        "./crates/scheduler-core/README.md",
        "./crates/sdk/README.md",
        "./crates/session-core/README.md",
        "./crates/skill-core/README.md",
        "./crates/tool-core/README.md",
        "./crates/tui/README.md",
        "./crates/workflow/README.md",
    ];

    fn subcommand_help(name: &str) -> String {
        let mut command = Cli::command();
        let subcommand = command
            .find_subcommand_mut(name)
            .expect("subcommand should exist");
        let mut buffer = Vec::new();
        subcommand
            .write_long_help(&mut buffer)
            .expect("subcommand help should render");
        String::from_utf8(buffer).expect("subcommand help should be utf8")
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
        let cli = Cli::parse_from([
            "mosaic",
            "inspect",
            ".mosaic/runs/demo.json",
            "--verbose",
            "--json",
        ]);

        assert_eq!(
            cli.dispatch(),
            DispatchCommand::Inspect {
                file: ".mosaic/runs/demo.json".into(),
                verbose: true,
                json: true,
            }
        );
    }

    #[test]
    fn parses_config_subcommands() {
        let cli = Cli::parse_from(["mosaic", "config", "show"]);
        assert_eq!(
            cli.dispatch(),
            DispatchCommand::Config {
                command: ConfigCommand::Show { json: false },
            }
        );

        let cli = Cli::parse_from(["mosaic", "config", "sources", "--json"]);
        assert_eq!(
            cli.dispatch(),
            DispatchCommand::Config {
                command: ConfigCommand::Sources { json: true },
            }
        );
    }

    #[test]
    fn parses_setup_subcommands() {
        let cli = Cli::parse_from([
            "mosaic",
            "setup",
            "init",
            "--force",
            "--profile",
            "anthropic-sonnet",
        ]);
        assert_eq!(
            cli.dispatch(),
            DispatchCommand::Setup {
                command: SetupCommand::Init {
                    force: true,
                    profile: Some("anthropic-sonnet".to_owned()),
                    dev_mock: false,
                },
            }
        );

        let cli = Cli::parse_from(["mosaic", "setup", "init", "--dev-mock"]);
        assert_eq!(
            cli.dispatch(),
            DispatchCommand::Setup {
                command: SetupCommand::Init {
                    force: false,
                    profile: None,
                    dev_mock: true,
                },
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
    fn tui_help_marks_operator_console_entrypoint() {
        let help = subcommand_help("tui");
        assert!(help.contains("main operator chat console"));
        assert!(help.contains("active operator conversation"));
        assert!(help.contains("remote HTTP Gateway"));
    }

    #[test]
    fn run_help_mentions_single_run_observer() {
        let help = subcommand_help("run");
        assert!(help.contains("single run in the terminal observer"));
        assert!(help.contains("long-lived operator console"));
        assert!(help.contains("Bind this run to a session id"));
    }

    #[test]
    fn config_help_mentions_merged_settings_and_json() {
        let help = subcommand_help("config");
        assert!(help.contains("merged operator config"));
        assert!(help.contains("mosaic config show"));
        assert!(help.contains("mosaic config show --json"));
    }

    #[test]
    fn inspect_help_mentions_verbose_and_json() {
        let help = subcommand_help("inspect");
        assert!(help.contains("provider, tool, workflow, and memory detail"));
        assert!(help.contains("--verbose"));
        assert!(help.contains("--json"));
    }

    #[test]
    fn setup_help_explains_when_to_use() {
        let help = subcommand_help("setup");
        assert!(help.contains("When to use it"));
        assert!(help.contains("diagnose why Mosaic will not start cleanly"));
        assert!(help.contains("mosaic setup init --profile anthropic-sonnet"));
        assert!(help.contains("mosaic setup init --dev-mock"));
    }

    #[test]
    fn top_level_help_mentions_quick_start_docs_and_operator_groups() {
        let help = Cli::command().render_long_help().to_string();
        assert!(help.contains("mosaic setup init"));
        assert!(help.contains("mosaic config show"));
        assert!(help.contains("Operator groups"));
        assert!(help.contains("docs/getting-started.md"));
        assert!(help.contains("docs/deployment.md"));
        assert!(help.contains("docs/security.md"));
        assert!(help.contains("docs/release.md"));
        assert!(help.contains("examples/providers/openai.yaml"));
        assert!(help.contains("examples/deployment/production.config.yaml"));
        assert!(help.contains("make release-check"));
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
            "docs/channels.md",
            "docs/full-stack.md",
            "docs/real-vs-mock-acceptance.md",
            "docs/residual-mock-first-audit.md",
            "docs/provider-runtime-policy-matrix.md",
            "docs/writer-ownership.md",
            "docs/deployment.md",
            "docs/security.md",
            "docs/session-inspect-incident.md",
            "docs/testing.md",
            "docs/non-tui-architecture-audit.md",
            "docs/release.md",
            "examples/README.md",
            "examples/TESTING.md",
            "examples/channels/README.md",
            "examples/full-stack/README.md",
            "examples/full-stack/openai-webchat.config.yaml",
            "examples/deployment/production.config.yaml",
            "make release-check",
            "make test-golden",
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
            "docs/deployment.md",
            "docs/operations.md",
        ] {
            assert!(
                getting_started.contains(required),
                "getting-started guide missing {required}"
            );
        }

        let cli_reference =
            fs::read_to_string(root.join("docs/cli.md")).expect("cli reference should load");
        for required in [
            "mosaic config show",
            "mosaic config sources",
            "mosaic inspect .mosaic/runs/<run-id>.json --verbose",
            "mosaic inspect .mosaic/runs/<run-id>.json --json",
            "Operator Groupings",
            "make release-check",
            "make test-golden",
            "make test-real",
            "make package",
        ] {
            assert!(
                cli_reference.contains(required),
                "cli reference missing {required}"
            );
        }

        let testing_guide =
            fs::read_to_string(root.join("docs/testing.md")).expect("testing guide should load");
        for required in [
            "make test-unit",
            "make test-integration",
            "make test-golden",
            "MOSAIC_REAL_TESTS=1 make test-real",
            "./scripts/test-full-stack-example.sh mock",
            "OPENAI_API_KEY=... ./scripts/test-full-stack-example.sh openai-webchat",
            "Per-Crate Matrix",
            "Release-Blocking Real Lanes",
            "Flaky Test Policy",
        ] {
            assert!(
                testing_guide.contains(required),
                "testing guide missing {required}"
            );
        }
    }

    #[test]
    fn delivery_artifacts_exist_and_env_example_lists_required_variables() {
        let root = repo_root();
        for required in [
            ".env.example",
            "docs/deployment.md",
            "docs/operations.md",
            "docs/security.md",
            "docs/testing.md",
            "docs/channels.md",
            "docs/full-stack.md",
            "docs/real-vs-mock-acceptance.md",
            "docs/residual-mock-first-audit.md",
            "docs/provider-runtime-policy-matrix.md",
            "docs/writer-ownership.md",
            "docs/session-inspect-incident.md",
            "docs/non-tui-architecture-audit.md",
            "docs/release.md",
            "docs/compatibility.md",
            "docs/upgrade.md",
            "examples/channels/README.md",
            "examples/channels/webchat-message.json",
            "examples/channels/webchat-openai-message.json",
            "examples/channels/telegram-update.json",
            "examples/full-stack/README.md",
            "examples/full-stack/mock-telegram.config.yaml",
            "examples/full-stack/openai-webchat.config.yaml",
            "examples/full-stack/openai-telegram.config.yaml",
            "examples/deployment/README.md",
            "examples/deployment/production.config.yaml",
            "examples/deployment/mosaic.service",
            "scripts/release-smoke.sh",
            "scripts/test-full-stack-example.sh",
            "scripts/test-golden-examples.sh",
            "scripts/test-real-integrations.sh",
            "scripts/verify-delivery-artifacts.sh",
        ] {
            assert!(
                root.join(required).is_file(),
                "missing delivery artifact {required}"
            );
        }

        let env_example =
            fs::read_to_string(root.join(".env.example")).expect("env example should load");
        for required in [
            "MOSAIC_ACTIVE_PROFILE",
            "MOSAIC_OPERATOR_TOKEN",
            "MOSAIC_WEBCHAT_SHARED_SECRET",
            "MOSAIC_TELEGRAM_SECRET_TOKEN",
            "OPENAI_API_KEY",
            "AZURE_OPENAI_API_KEY",
            "ANTHROPIC_API_KEY",
        ] {
            assert!(
                env_example.contains(required),
                ".env.example missing {required}"
            );
        }
    }

    #[test]
    fn every_workspace_crate_has_a_structured_readme() {
        let root = repo_root();
        for (relative_path, package_name) in CRATE_READMES {
            let readme_path = root.join(relative_path);
            assert!(
                readme_path.is_file(),
                "missing crate README {relative_path}"
            );
            let readme = fs::read_to_string(&readme_path)
                .unwrap_or_else(|_| panic!("crate README should load: {relative_path}"));

            for heading in REQUIRED_CRATE_README_SECTIONS {
                assert!(
                    readme.contains(heading),
                    "{relative_path} missing required section {heading}"
                );
            }

            let test_command = format!("cargo test -p {package_name}");
            assert!(
                readme.contains(&test_command),
                "{relative_path} missing testing command {test_command}"
            );
        }
    }

    #[test]
    fn root_readme_links_every_workspace_crate_guide() {
        let root = repo_root();
        let readme = fs::read_to_string(root.join("README.md")).expect("README should load");
        assert!(readme.contains("## Crate Guide"));

        for link in ROOT_CRATE_GUIDE_LINKS {
            assert!(
                readme.contains(link),
                "root README missing crate guide link {link}"
            );
        }
    }

    #[test]
    fn selected_crate_guides_reference_key_public_api_names() {
        let root = repo_root();

        let provider_readme = fs::read_to_string(root.join("crates/provider/README.md"))
            .expect("provider README should load");
        for required in [
            "ProviderProfileRegistry",
            "LlmProvider",
            "build_provider_from_profile",
        ] {
            assert!(
                provider_readme.contains(required),
                "provider README missing public API {required}"
            );
        }

        let tool_readme = fs::read_to_string(root.join("crates/tool-core/README.md"))
            .expect("tool-core README should load");
        for required in ["ToolRegistry", "Tool", "EchoTool"] {
            assert!(
                tool_readme.contains(required),
                "tool-core README missing public API {required}"
            );
        }

        let runtime_readme = fs::read_to_string(root.join("crates/runtime/README.md"))
            .expect("runtime README should load");
        for required in ["AgentRuntime", "RunRequest", "RuntimeContext"] {
            assert!(
                runtime_readme.contains(required),
                "runtime README missing public API {required}"
            );
        }

        let gateway_readme = fs::read_to_string(root.join("crates/gateway/README.md"))
            .expect("gateway README should load");
        for required in ["GatewayHandle", "http_router", "serve_http"] {
            assert!(
                gateway_readme.contains(required),
                "gateway README missing public API {required}"
            );
        }

        let session_readme = fs::read_to_string(root.join("crates/session-core/README.md"))
            .expect("session-core README should load");
        for required in ["SessionStore", "FileSessionStore", "SessionRecord"] {
            assert!(
                session_readme.contains(required),
                "session-core README missing public API {required}"
            );
        }
    }

    #[test]
    fn non_tui_architecture_audit_covers_scope_matrix_and_follow_up_plan() {
        let root = repo_root();
        let audit = fs::read_to_string(root.join("docs/non-tui-architecture-audit.md"))
            .expect("non-tui architecture audit should load");

        for required in [
            "# Non-TUI Architecture Audit",
            "## Scope",
            "## AGENTS Alignment Matrix",
            "## Crate-by-Crate Debt Register",
            "## Remediation Priority Order",
            "crates/tui",
            "cli/src/main.rs:575-613",
            "crates/gateway/src/lib.rs:523-538",
            "crates/runtime/src/lib.rs:240-313",
            "crates/extension-core/src/lib.rs:257-365",
            "specs/plan_i1.md",
            "specs/plan_h5.md",
        ] {
            assert!(
                audit.contains(required),
                "non-tui architecture audit missing {required}"
            );
        }

        let plan = fs::read_to_string(root.join("specs/plan_i1.md")).expect("plan_i1 should load");
        for required in [
            "# Plan i1",
            "cli",
            "gateway",
            "runtime",
            "session / memory / inspect",
            "extension-core",
            "webchat",
            "cargo test --workspace",
        ] {
            assert!(plan.contains(required), "plan_i1 missing {required}");
        }
    }

    #[test]
    fn writer_ownership_guide_covers_gateway_runtime_session_and_inspect_boundaries() {
        let root = repo_root();
        let guide = fs::read_to_string(root.join("docs/writer-ownership.md"))
            .expect("writer ownership guide should load");

        for required in [
            "# Writer Ownership",
            "## Gateway Writers",
            "## Runtime Writers",
            "## Session-Core Boundary",
            "## Inspect Boundary",
            "## Interaction Entry Consistency",
            "gateway_run_id",
            "current_gateway_run_id",
            "transcript messages",
            "memory summaries",
            "mosaic-channel-telegram",
            "mosaic-gateway::ingress",
        ] {
            assert!(
                guide.contains(required),
                "writer ownership guide missing {required}"
            );
        }
    }

    #[test]
    fn docs_and_examples_bind_to_one_full_stack_path() {
        let root = repo_root();

        let readme = fs::read_to_string(root.join("README.md")).expect("README should load");
        for required in [
            "docs/channels.md",
            "docs/full-stack.md",
            "docs/real-vs-mock-acceptance.md",
            "docs/provider-runtime-policy-matrix.md",
            "docs/session-inspect-incident.md",
            "examples/channels/",
            "examples/full-stack/",
        ] {
            assert!(readme.contains(required), "README missing {required}");
        }

        let full_stack = fs::read_to_string(root.join("docs/full-stack.md"))
            .expect("full-stack guide should load");
        for required in [
            "examples/full-stack/openai-webchat.config.yaml",
            "examples/full-stack/mock-telegram.config.yaml",
            "examples/full-stack/openai-telegram.config.yaml",
            "examples/channels/webchat-openai-message.json",
            "examples/channels/telegram-update.json",
            "./scripts/test-full-stack-example.sh mock",
            "MOSAIC_REAL_TESTS=1 OPENAI_API_KEY=... ./scripts/test-full-stack-example.sh openai-webchat",
            "mosaic gateway --attach http://127.0.0.1:18080 incident <run-id>",
        ] {
            assert!(
                full_stack.contains(required),
                "full-stack guide missing {required}"
            );
        }

        let channels =
            fs::read_to_string(root.join("docs/channels.md")).expect("channels guide should load");
        for required in [
            "examples/channels/webchat-message.json",
            "examples/channels/telegram-update.json",
            "POST /ingress/webchat",
            "POST /ingress/telegram",
        ] {
            assert!(
                channels.contains(required),
                "channels guide missing {required}"
            );
        }

        let session_flow = fs::read_to_string(root.join("docs/session-inspect-incident.md"))
            .expect("session/inspect/incident guide should load");
        for required in [
            "mosaic session show telegram--100123-99",
            "mosaic inspect .mosaic/runs/<run-id>.json --verbose",
            "mosaic gateway incident <run-id>",
        ] {
            assert!(
                session_flow.contains(required),
                "session/inspect/incident guide missing {required}"
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
