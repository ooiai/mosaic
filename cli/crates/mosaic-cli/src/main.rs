use std::collections::BTreeMap;
use std::io::{self, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, Utc};
use clap::{Args, Parser, Subcommand, ValueEnum};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tiny_http::{Method, Response, Server};

use mosaic_channels::{
    AddChannelInput, ChannelRepository, DEFAULT_CHANNEL_TOKEN_ENV, channels_events_dir,
    channels_file_path, format_channel_for_output,
};
use mosaic_gateway::{GatewayClient, GatewayRequest, HttpGatewayClient};

use mosaic_agent::{AgentRunOptions, AgentRunner};
use mosaic_core::audit::AuditStore;
use mosaic_core::config::{
    ConfigManager, DEFAULT_PROFILE, ProfileConfig, RunGuardMode, StateConfig,
};
use mosaic_core::error::{MosaicError, Result};
use mosaic_core::provider::Provider;
use mosaic_core::session::SessionStore;
use mosaic_core::state::{StateMode, StatePaths};
use mosaic_ops::{
    ApprovalMode, ApprovalStore, RuntimePolicy, SandboxProfile, SandboxStore, SystemEventStore,
    collect_logs, list_profiles, snapshot_presence, system_events_path,
};
use mosaic_provider_openai::OpenAiCompatibleProvider;
use mosaic_tools::ToolExecutor;

const PROJECT_STATE_DIR: &str = ".mosaic";

#[derive(Parser, Debug)]
#[command(name = "mosaic", version, about = "Mosaic local agent CLI")]
struct Cli {
    #[arg(long, default_value = DEFAULT_PROFILE)]
    profile: String,
    #[arg(long)]
    project_state: bool,
    #[arg(long)]
    json: bool,
    #[arg(long)]
    yes: bool,
    #[arg(long)]
    debug: bool,
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug, Clone)]
enum Commands {
    #[command(visible_alias = "onboard")]
    Setup(SetupArgs),
    Configure(ConfigureArgs),
    Models(ModelsArgs),
    #[command(visible_alias = "message")]
    Ask(AskArgs),
    #[command(visible_alias = "agent")]
    Chat(ChatArgs),
    Session(SessionArgs),
    Gateway(GatewayArgs),
    Channels(ChannelsArgs),
    Logs(LogsArgs),
    System(SystemArgs),
    Approvals(ApprovalsArgs),
    Sandbox(SandboxArgs),
    Status,
    Health,
    Doctor,
}

#[derive(Args, Debug, Clone)]
struct SetupArgs {
    #[arg(long)]
    base_url: Option<String>,
    #[arg(long)]
    model: Option<String>,
    #[arg(long)]
    api_key_env: Option<String>,
    #[arg(long)]
    temperature: Option<f32>,
    #[arg(long)]
    max_turns: Option<u32>,
    #[arg(long)]
    tools_enabled: Option<bool>,
    #[arg(long, value_enum)]
    guard_mode: Option<GuardModeArg>,
}

#[derive(Args, Debug, Clone)]
struct ConfigureArgs {
    #[arg(long)]
    show: bool,
    #[arg(long)]
    base_url: Option<String>,
    #[arg(long)]
    model: Option<String>,
    #[arg(long)]
    api_key_env: Option<String>,
    #[arg(long)]
    temperature: Option<f32>,
    #[arg(long)]
    max_turns: Option<u32>,
    #[arg(long)]
    tools_enabled: Option<bool>,
    #[arg(long, value_enum)]
    guard_mode: Option<GuardModeArg>,
}

#[derive(Args, Debug, Clone)]
struct ModelsArgs {
    #[command(subcommand)]
    command: ModelsCommand,
}

#[derive(Subcommand, Debug, Clone)]
enum ModelsCommand {
    List,
}

#[derive(Args, Debug, Clone)]
struct AskArgs {
    prompt: String,
    #[arg(long)]
    session: Option<String>,
}

#[derive(Args, Debug, Clone)]
struct ChatArgs {
    #[arg(long)]
    session: Option<String>,
    #[arg(long)]
    prompt: Option<String>,
}

#[derive(Args, Debug, Clone)]
struct SessionArgs {
    #[command(subcommand)]
    command: SessionCommand,
}

#[derive(Subcommand, Debug, Clone)]
enum SessionCommand {
    List,
    Show {
        session_id: String,
    },
    Resume {
        session_id: String,
    },
    Clear {
        session_id: Option<String>,
        #[arg(long)]
        all: bool,
    },
}

#[derive(Args, Debug, Clone)]
struct GatewayArgs {
    #[command(subcommand)]
    command: GatewayCommand,
}

#[derive(Subcommand, Debug, Clone)]
enum GatewayCommand {
    Run {
        #[arg(long, default_value = "127.0.0.1")]
        host: String,
        #[arg(long, default_value_t = 8787)]
        port: u16,
    },
    Status,
    Health,
    Call {
        method: String,
        #[arg(long)]
        params: Option<String>,
    },
    Probe,
    Discover,
    Stop,
    #[command(hide = true)]
    Serve {
        #[arg(long)]
        host: String,
        #[arg(long)]
        port: u16,
    },
}

#[derive(Args, Debug, Clone)]
struct ChannelsArgs {
    #[command(subcommand)]
    command: ChannelsCommand,
}

#[derive(Subcommand, Debug, Clone)]
enum ChannelsCommand {
    List,
    Status,
    Add {
        #[arg(long)]
        name: String,
        #[arg(long, default_value = "slack_webhook")]
        kind: String,
        #[arg(long)]
        endpoint: Option<String>,
        #[arg(long)]
        token_env: Option<String>,
    },
    Login {
        channel_id: String,
        #[arg(long)]
        token_env: Option<String>,
    },
    Send {
        channel_id: String,
        #[arg(long)]
        text: String,
        #[arg(long)]
        token_env: Option<String>,
    },
    Test {
        channel_id: String,
        #[arg(long)]
        token_env: Option<String>,
    },
    Logs {
        #[arg(long)]
        channel: Option<String>,
        #[arg(long, default_value_t = 50)]
        tail: usize,
    },
    Capabilities {
        #[arg(long)]
        channel: Option<String>,
        #[arg(long)]
        target: Option<String>,
    },
    Resolve {
        #[arg(long)]
        channel: String,
        query: Vec<String>,
    },
    Remove {
        channel_id: String,
    },
    Logout {
        channel_id: String,
    },
}

#[derive(Args, Debug, Clone)]
struct LogsArgs {
    #[arg(long)]
    follow: bool,
    #[arg(long, default_value_t = 100)]
    tail: usize,
}

#[derive(Args, Debug, Clone)]
struct SystemArgs {
    #[command(subcommand)]
    command: SystemCommand,
}

#[derive(Subcommand, Debug, Clone)]
enum SystemCommand {
    Event {
        name: String,
        #[arg(long)]
        data: Option<String>,
    },
    Presence,
}

#[derive(Args, Debug, Clone)]
struct ApprovalsArgs {
    #[command(subcommand)]
    command: ApprovalsCommand,
}

#[derive(Subcommand, Debug, Clone)]
enum ApprovalsCommand {
    Get,
    Set {
        #[arg(value_enum)]
        mode: ApprovalModeArg,
    },
    Allowlist {
        #[command(subcommand)]
        command: AllowlistCommand,
    },
}

#[derive(Subcommand, Debug, Clone)]
enum AllowlistCommand {
    Add { prefix: String },
    Remove { prefix: String },
}

#[derive(Args, Debug, Clone)]
struct SandboxArgs {
    #[command(subcommand)]
    command: SandboxCommand,
}

#[derive(Subcommand, Debug, Clone)]
enum SandboxCommand {
    List,
    Explain {
        #[arg(long, value_enum)]
        profile: Option<SandboxProfileArg>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct GatewayState {
    running: bool,
    host: String,
    port: u16,
    pid: u32,
    started_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
#[value(rename_all = "snake_case")]
enum GuardModeArg {
    ConfirmDangerous,
    AllConfirm,
    Unrestricted,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
#[value(rename_all = "snake_case")]
enum ApprovalModeArg {
    Deny,
    Confirm,
    Allowlist,
}

impl From<ApprovalModeArg> for ApprovalMode {
    fn from(value: ApprovalModeArg) -> Self {
        match value {
            ApprovalModeArg::Deny => Self::Deny,
            ApprovalModeArg::Confirm => Self::Confirm,
            ApprovalModeArg::Allowlist => Self::Allowlist,
        }
    }
}

#[derive(Debug, Clone, Copy, ValueEnum)]
#[value(rename_all = "snake_case")]
enum SandboxProfileArg {
    Restricted,
    Standard,
    Elevated,
}

impl From<SandboxProfileArg> for SandboxProfile {
    fn from(value: SandboxProfileArg) -> Self {
        match value {
            SandboxProfileArg::Restricted => Self::Restricted,
            SandboxProfileArg::Standard => Self::Standard,
            SandboxProfileArg::Elevated => Self::Elevated,
        }
    }
}

impl From<GuardModeArg> for RunGuardMode {
    fn from(value: GuardModeArg) -> Self {
        match value {
            GuardModeArg::ConfirmDangerous => Self::ConfirmDangerous,
            GuardModeArg::AllConfirm => Self::AllConfirm,
            GuardModeArg::Unrestricted => Self::Unrestricted,
        }
    }
}

struct RuntimeContext {
    provider: Arc<dyn Provider>,
    agent: AgentRunner,
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    let json_mode = cli.json;
    let result = run(cli).await;
    if let Err(err) = result {
        if json_mode {
            print_json(&json!({
                "ok": false,
                "error": {
                    "code": err.code(),
                    "message": err.to_string(),
                    "exit_code": err.exit_code(),
                }
            }));
        } else {
            eprintln!("error [{}]: {}", err.code(), err);
        }
        std::process::exit(err.exit_code());
    }
}

async fn run(cli: Cli) -> Result<()> {
    if cli.debug {
        eprintln!(
            "[debug] profile={} project_state={} json={}",
            cli.profile, cli.project_state, cli.json
        );
    }
    match cli.command.clone() {
        Commands::Setup(args) => handle_setup(&cli, args),
        Commands::Configure(args) => handle_configure(&cli, args),
        Commands::Models(args) => handle_models(&cli, args).await,
        Commands::Ask(args) => handle_ask(&cli, args).await,
        Commands::Chat(args) => handle_chat(&cli, args).await,
        Commands::Session(args) => handle_session(&cli, args).await,
        Commands::Gateway(args) => handle_gateway(&cli, args).await,
        Commands::Channels(args) => handle_channels(&cli, args).await,
        Commands::Logs(args) => handle_logs(&cli, args).await,
        Commands::System(args) => handle_system(&cli, args),
        Commands::Approvals(args) => handle_approvals(&cli, args),
        Commands::Sandbox(args) => handle_sandbox(&cli, args),
        Commands::Status => handle_status(&cli),
        Commands::Health => handle_health(&cli).await,
        Commands::Doctor => handle_doctor(&cli).await,
    }
}

fn handle_setup(cli: &Cli, args: SetupArgs) -> Result<()> {
    let paths = resolve_state_paths(cli.project_state)?;
    paths.ensure_dirs()?;
    let manager = ConfigManager::new(paths.config_path.clone());
    let mut config = manager.load_or_default(paths.mode)?;
    let profile = config
        .profiles
        .entry(cli.profile.clone())
        .or_insert_with(ProfileConfig::default);
    if let Some(base_url) = args.base_url {
        profile.provider.base_url = base_url;
    }
    if let Some(model) = args.model {
        profile.provider.model = model;
    }
    if let Some(api_key_env) = args.api_key_env {
        profile.provider.api_key_env = api_key_env;
    }
    if let Some(temperature) = args.temperature {
        profile.agent.temperature = temperature;
    }
    if let Some(max_turns) = args.max_turns {
        profile.agent.max_turns = max_turns;
    }
    if let Some(tools_enabled) = args.tools_enabled {
        profile.tools.enabled = tools_enabled;
    }
    if let Some(guard_mode) = args.guard_mode {
        profile.tools.run.guard_mode = guard_mode.into();
    }
    config.active_profile = cli.profile.clone();
    config.state = StateConfig {
        mode: paths.mode,
        project_dir: PROJECT_STATE_DIR.to_string(),
    };
    manager.save(&config)?;

    if cli.json {
        print_json(&json!({
            "ok": true,
            "config_path": manager.path().display().to_string(),
            "profile": cli.profile,
            "mode": paths.mode,
        }));
    } else {
        println!("Setup complete.");
        println!("Config: {}", manager.path().display());
        println!("Profile: {}", cli.profile);
        println!("Mode: {:?}", paths.mode);
    }
    Ok(())
}

fn handle_configure(cli: &Cli, args: ConfigureArgs) -> Result<()> {
    let paths = resolve_state_paths(cli.project_state)?;
    paths.ensure_dirs()?;
    let manager = ConfigManager::new(paths.config_path.clone());
    let mut config = manager.load()?;

    let mut changed = false;
    {
        let profile = config
            .profiles
            .entry(cli.profile.clone())
            .or_insert_with(ProfileConfig::default);
        if let Some(base_url) = args.base_url {
            profile.provider.base_url = base_url;
            changed = true;
        }
        if let Some(model) = args.model {
            profile.provider.model = model;
            changed = true;
        }
        if let Some(api_key_env) = args.api_key_env {
            profile.provider.api_key_env = api_key_env;
            changed = true;
        }
        if let Some(temperature) = args.temperature {
            profile.agent.temperature = temperature;
            changed = true;
        }
        if let Some(max_turns) = args.max_turns {
            profile.agent.max_turns = max_turns;
            changed = true;
        }
        if let Some(tools_enabled) = args.tools_enabled {
            profile.tools.enabled = tools_enabled;
            changed = true;
        }
        if let Some(guard_mode) = args.guard_mode {
            profile.tools.run.guard_mode = guard_mode.into();
            changed = true;
        }
    }

    config.active_profile = cli.profile.clone();
    if changed {
        manager.save(&config)?;
    }
    let resolved = config.resolve_profile(Some(&cli.profile))?;
    if cli.json {
        print_json(&json!({
            "ok": true,
            "changed": changed,
            "profile": resolved.profile_name,
            "config_path": manager.path().display().to_string(),
            "config": resolved,
        }));
    } else if args.show || !changed {
        println!("Config path: {}", manager.path().display());
        println!("Profile: {}", resolved.profile_name);
        println!("Provider base URL: {}", resolved.profile.provider.base_url);
        println!("Model: {}", resolved.profile.provider.model);
        println!("API key env: {}", resolved.profile.provider.api_key_env);
        println!("Tools enabled: {}", resolved.profile.tools.enabled);
        println!("Guard mode: {:?}", resolved.profile.tools.run.guard_mode);
    } else {
        println!(
            "Configuration updated for profile '{}'.",
            resolved.profile_name
        );
    }
    Ok(())
}

async fn handle_models(cli: &Cli, args: ModelsArgs) -> Result<()> {
    match args.command {
        ModelsCommand::List => {
            let runtime = build_runtime(cli)?;
            let models = runtime.provider.list_models().await?;
            if cli.json {
                print_json(&json!({ "ok": true, "models": models }));
            } else {
                for model in &models {
                    if let Some(owner) = &model.owned_by {
                        println!("{} ({owner})", model.id);
                    } else {
                        println!("{}", model.id);
                    }
                }
                println!("Total models: {}", models.len());
            }
        }
    }
    Ok(())
}

async fn handle_ask(cli: &Cli, args: AskArgs) -> Result<()> {
    let runtime = build_runtime(cli)?;
    let result = runtime
        .agent
        .ask(
            &args.prompt,
            AgentRunOptions {
                session_id: args.session,
                cwd: std::env::current_dir().map_err(|err| MosaicError::Io(err.to_string()))?,
                yes: cli.yes,
                interactive: false,
            },
        )
        .await?;
    if cli.json {
        print_json(&json!({
            "ok": true,
            "session_id": result.session_id,
            "response": result.response,
            "turns": result.turns,
        }));
    } else {
        println!("{}", result.response.trim());
        println!("session: {}", result.session_id);
    }
    Ok(())
}

async fn handle_chat(cli: &Cli, args: ChatArgs) -> Result<()> {
    let runtime = build_runtime(cli)?;
    let mut session_id = args.session;

    if let Some(prompt) = args.prompt {
        let result = runtime
            .agent
            .ask(
                &prompt,
                AgentRunOptions {
                    session_id: session_id.clone(),
                    cwd: std::env::current_dir().map_err(|err| MosaicError::Io(err.to_string()))?,
                    yes: cli.yes,
                    interactive: true,
                },
            )
            .await?;
        session_id = Some(result.session_id.clone());
        if cli.json {
            print_json(&json!({
                "ok": true,
                "session_id": result.session_id,
                "response": result.response,
                "turns": result.turns,
            }));
            return Ok(());
        }
        println!("{}", result.response.trim());
        println!("session: {}", result.session_id);
    } else if cli.json {
        return Err(MosaicError::Validation(
            "chat in --json mode requires --prompt".to_string(),
        ));
    }

    println!("Entering chat mode. Type /help for commands, /exit to quit.");
    if let Some(id) = &session_id {
        println!("Resumed session: {id}");
    }
    loop {
        print!("you> ");
        io::stdout()
            .flush()
            .map_err(|err| MosaicError::Io(err.to_string()))?;
        let mut input = String::new();
        io::stdin()
            .read_line(&mut input)
            .map_err(|err| MosaicError::Io(err.to_string()))?;
        let prompt = input.trim();
        if prompt.is_empty() {
            continue;
        }
        if matches!(prompt, "/exit" | "exit" | "quit") {
            println!("Bye.");
            break;
        }
        if prompt == "/help" {
            println!("/help     Show help");
            println!("/session  Show current session id");
            println!("/exit     Exit chat");
            continue;
        }
        if prompt == "/session" {
            if let Some(id) = &session_id {
                println!("session: {id}");
            } else {
                println!("session: <new session>");
            }
            continue;
        }

        let result = runtime
            .agent
            .ask(
                prompt,
                AgentRunOptions {
                    session_id: session_id.clone(),
                    cwd: std::env::current_dir().map_err(|err| MosaicError::Io(err.to_string()))?,
                    yes: cli.yes,
                    interactive: true,
                },
            )
            .await?;
        session_id = Some(result.session_id.clone());
        println!("assistant> {}", result.response.trim());
    }
    Ok(())
}

async fn handle_session(cli: &Cli, args: SessionArgs) -> Result<()> {
    let paths = resolve_state_paths(cli.project_state)?;
    let store = SessionStore::new(paths.sessions_dir.clone());
    store.ensure_dirs()?;
    match args.command {
        SessionCommand::List => {
            let sessions = store.list_sessions()?;
            if cli.json {
                print_json(&json!({ "ok": true, "sessions": sessions }));
            } else if sessions.is_empty() {
                println!("No sessions found.");
            } else {
                for session in sessions {
                    let last = session
                        .last_updated
                        .map(|value| value.to_rfc3339())
                        .unwrap_or_else(|| "-".to_string());
                    println!(
                        "{} events={} last={}",
                        session.session_id, session.event_count, last
                    );
                }
            }
        }
        SessionCommand::Show { session_id } => {
            let events = store.read_events(&session_id)?;
            if cli.json {
                print_json(&json!({ "ok": true, "session_id": session_id, "events": events }));
            } else {
                println!("Session: {session_id}");
                for event in events {
                    println!(
                        "{} {} {} {}",
                        event.ts.to_rfc3339(),
                        event.id,
                        format!("{:?}", event.kind),
                        event.payload
                    );
                }
            }
        }
        SessionCommand::Resume { session_id } => {
            handle_chat(
                cli,
                ChatArgs {
                    session: Some(session_id),
                    prompt: None,
                },
            )
            .await?;
        }
        SessionCommand::Clear { session_id, all } => {
            if all {
                let removed = store.clear_all()?;
                if cli.json {
                    print_json(&json!({ "ok": true, "removed": removed }));
                } else {
                    println!("Removed {removed} sessions.");
                }
            } else {
                let session_id = session_id.ok_or_else(|| {
                    MosaicError::Validation(
                        "session id is required unless --all is provided".to_string(),
                    )
                })?;
                store.clear_session(&session_id)?;
                if cli.json {
                    print_json(&json!({ "ok": true, "removed_session": session_id }));
                } else {
                    println!("Removed session {session_id}");
                }
            }
        }
    }
    Ok(())
}

async fn handle_gateway(cli: &Cli, args: GatewayArgs) -> Result<()> {
    let paths = resolve_state_paths(cli.project_state)?;
    paths.ensure_dirs()?;
    let gateway_path = paths.data_dir.join("gateway.json");
    match args.command {
        GatewayCommand::Run { host, port } => {
            if let Some(existing) = load_json_file_opt::<GatewayState>(&gateway_path)?
                && is_process_alive(existing.pid)
                && probe_gateway_health(&existing.host, existing.port).await
            {
                if cli.json {
                    print_json(&json!({
                        "ok": true,
                        "gateway": existing,
                        "message": "gateway already running",
                    }));
                } else {
                    println!(
                        "Gateway already running on {}:{}",
                        existing.host, existing.port
                    );
                }
                return Ok(());
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
            save_json_file(&gateway_path, &state)?;
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "gateway": state,
                    "path": gateway_path.display().to_string(),
                }));
            } else {
                println!("Gateway is running.");
                println!("host: {}", state.host);
                println!("port: {}", state.port);
                println!("pid: {}", state.pid);
                println!("state: {}", gateway_path.display());
            }
        }
        GatewayCommand::Status => {
            let state: Option<GatewayState> = load_json_file_opt(&gateway_path)?;
            let running = match &state {
                Some(value) => {
                    if gateway_test_mode() {
                        value.running
                    } else {
                        is_process_alive(value.pid)
                            && probe_gateway_health(&value.host, value.port).await
                    }
                }
                None => false,
            };
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "running": running,
                    "gateway": state,
                    "path": gateway_path.display().to_string(),
                }));
            } else if let Some(state) = state {
                println!(
                    "gateway: running={} host={} port={} pid={} updated={}",
                    running,
                    state.host,
                    state.port,
                    state.pid,
                    state.updated_at.to_rfc3339()
                );
            } else {
                println!("gateway: not running");
            }
        }
        GatewayCommand::Health => {
            let state: Option<GatewayState> = load_json_file_opt(&gateway_path)?;
            let process_alive = state.as_ref().is_some_and(|v| {
                if gateway_test_mode() {
                    v.running
                } else {
                    is_process_alive(v.pid)
                }
            });
            let endpoint_healthy = if let Some(value) = &state {
                if gateway_test_mode() {
                    value.running
                } else {
                    probe_gateway_health(&value.host, value.port).await
                }
            } else {
                false
            };
            let mut checks = vec![
                run_check(
                    "gateway_state_file",
                    gateway_path.exists(),
                    "gateway state file",
                ),
                run_check(
                    "gateway_process",
                    process_alive,
                    if process_alive {
                        "gateway process is alive"
                    } else {
                        "gateway process is not alive"
                    },
                ),
                run_check(
                    "gateway_endpoint",
                    endpoint_healthy,
                    if endpoint_healthy {
                        "GET /health reachable"
                    } else {
                        "GET /health unreachable"
                    },
                ),
            ];
            if let Some(state) = state {
                checks.push(run_check(
                    "gateway_target",
                    true,
                    format!("{}:{} (pid={})", state.host, state.port, state.pid),
                ));
            }
            emit_checks(cli.json, "gateway_health", checks)?;
        }
        GatewayCommand::Call { method, params } => {
            if gateway_test_mode() {
                let state: Option<GatewayState> = load_json_file_opt(&gateway_path)?;
                if !state.as_ref().is_some_and(|value| value.running) {
                    return Err(MosaicError::GatewayUnavailable(
                        "gateway is not running in test mode".to_string(),
                    ));
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
                    _ => {
                        return Err(MosaicError::GatewayProtocol(format!(
                            "unknown gateway method '{}' in test mode",
                            method
                        )));
                    }
                };
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

            let (host, port) = resolve_gateway_target(&gateway_path)?;
            let client = HttpGatewayClient::new(&host, port)?;
            let params = params
                .as_deref()
                .map(|value| parse_json_input(value, "gateway params"))
                .transpose()?;
            let request = GatewayRequest::new(method.clone(), params);
            let request_id = request.id.clone();
            let response = client.call(request).await?;
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
            if gateway_test_mode() {
                let state: Option<GatewayState> = load_json_file_opt(&gateway_path)?;
                if !state.as_ref().is_some_and(|value| value.running) {
                    return Err(MosaicError::GatewayUnavailable(
                        "gateway is not running in test mode".to_string(),
                    ));
                }
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

            let (host, port) = resolve_gateway_target(&gateway_path)?;
            let client = HttpGatewayClient::new(&host, port)?;
            let probe = client.probe().await?;
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
            if gateway_test_mode() {
                let methods = vec!["health", "status", "echo"];
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
                }
                return Ok(());
            }

            let (host, port) = resolve_gateway_target(&gateway_path)?;
            let client = HttpGatewayClient::new(&host, port)?;
            let discovery = client.discover().await?;
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
        GatewayCommand::Stop => {
            let state: GatewayState = load_json_file_opt(&gateway_path)?.ok_or_else(|| {
                MosaicError::Config("gateway state file not found; not running".to_string())
            })?;
            let was_alive = if gateway_test_mode() {
                state.running
            } else {
                is_process_alive(state.pid)
            };
            let stopped = if was_alive && !gateway_test_mode() {
                stop_process(state.pid)?
            } else {
                false
            };
            let now = Utc::now();
            let next = GatewayState {
                running: false,
                host: state.host.clone(),
                port: state.port,
                pid: state.pid,
                started_at: state.started_at,
                updated_at: now,
            };
            save_json_file(&gateway_path, &next)?;
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "was_running": was_alive,
                    "stopped": stopped || !was_alive,
                    "gateway": next,
                }));
            } else if was_alive {
                println!(
                    "Gateway {} (pid={})",
                    if stopped {
                        "stopped"
                    } else {
                        "stop signal sent"
                    },
                    state.pid
                );
            } else {
                println!("Gateway process was not running.");
            }
        }
        GatewayCommand::Serve { host, port } => {
            run_gateway_http_server(&host, port)?;
        }
    }
    Ok(())
}

async fn handle_channels(cli: &Cli, args: ChannelsArgs) -> Result<()> {
    let paths = resolve_state_paths(cli.project_state)?;
    paths.ensure_dirs()?;
    let channels_path = channels_file_path(&paths.data_dir);
    let channel_events_dir = channels_events_dir(&paths.data_dir);
    let repository = ChannelRepository::new(channels_path.clone(), channel_events_dir);
    match args.command {
        ChannelsCommand::List => {
            let channels = repository.list()?;
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "channels": channels,
                    "path": channels_path.display().to_string(),
                }));
            } else if channels.is_empty() {
                println!("No channels configured.");
            } else {
                for channel in channels {
                    println!(
                        "{} name={} kind={} endpoint={} last_login={} last_send={} last_error={}",
                        channel.id,
                        channel.name,
                        channel.kind,
                        channel.endpoint_masked.unwrap_or_else(|| "-".to_string()),
                        channel
                            .last_login_at
                            .map(|v| v.to_rfc3339())
                            .unwrap_or_else(|| "-".to_string()),
                        channel
                            .last_send_at
                            .map(|v| v.to_rfc3339())
                            .unwrap_or_else(|| "-".to_string()),
                        channel.last_error.unwrap_or_else(|| "-".to_string())
                    );
                }
            }
        }
        ChannelsCommand::Status => {
            let status = repository.status()?;
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "status": status,
                }));
            } else {
                println!("channels total: {}", status.total_channels);
                println!("channels healthy: {}", status.healthy_channels);
                println!("channels with errors: {}", status.channels_with_errors);
                if let Some(last_send_at) = status.last_send_at {
                    println!("last send at: {}", last_send_at.to_rfc3339());
                }
                if !status.kinds.is_empty() {
                    println!("kinds:");
                    for (kind, count) in status.kinds {
                        println!("- {kind}: {count}");
                    }
                }
            }
        }
        ChannelsCommand::Add {
            name,
            kind,
            endpoint,
            token_env,
        } => {
            let entry = repository.add(AddChannelInput {
                name,
                kind,
                endpoint,
                token_env,
            })?;
            let rendered = format_channel_for_output(&entry);
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "channel": rendered,
                    "path": channels_path.display().to_string(),
                }));
            } else {
                println!("Channel added: {}", rendered.id);
            }
        }
        ChannelsCommand::Login {
            channel_id,
            token_env,
        } => {
            let token_env = token_env.unwrap_or_else(|| DEFAULT_CHANNEL_TOKEN_ENV.to_string());
            let login = repository.login(&channel_id, &token_env)?;
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "channel_id": channel_id,
                    "token_env": login.token_env,
                    "token_present": login.token_present,
                    "channel": format_channel_for_output(&login.channel),
                }));
            } else {
                println!("Channel login recorded for {channel_id}");
                println!(
                    "token env {} {}",
                    login.token_env,
                    if login.token_present {
                        "found"
                    } else {
                        "not found"
                    }
                );
            }
        }
        ChannelsCommand::Send {
            channel_id,
            text,
            token_env,
        } => {
            let result = repository
                .send(&channel_id, &text, token_env, false)
                .await?;
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "channel_id": result.channel_id,
                    "kind": result.kind,
                    "delivered_via": result.delivered_via,
                    "attempts": result.attempts,
                    "http_status": result.http_status,
                    "endpoint_masked": result.endpoint_masked,
                    "event_path": result.event_path,
                }));
            } else {
                println!("Message sent via {}", result.delivered_via);
                if let Some(endpoint) = result.endpoint_masked {
                    println!("endpoint: {endpoint}");
                }
            }
        }
        ChannelsCommand::Logs { channel, tail } => {
            let events = repository.logs(channel.as_deref(), tail)?;
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "events": events,
                    "channel": channel,
                }));
            } else if events.is_empty() {
                println!("No channel events found.");
            } else {
                for event in events {
                    println!(
                        "{} channel={} kind={} status={} attempt={} http={} error={} preview={}",
                        event.ts.to_rfc3339(),
                        event.channel_id,
                        event.kind,
                        event.delivery_status,
                        event.attempt,
                        event
                            .http_status
                            .map(|value| value.to_string())
                            .unwrap_or_else(|| "-".to_string()),
                        event.error.unwrap_or_else(|| "-".to_string()),
                        event.text_preview
                    );
                }
            }
        }
        ChannelsCommand::Capabilities { channel, target } => {
            let capabilities = repository.capabilities(channel.as_deref(), target.as_deref())?;
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "capabilities": capabilities,
                }));
            } else if capabilities.is_empty() {
                println!("No channel capabilities resolved.");
            } else {
                for capability in capabilities {
                    println!(
                        "{} aliases={} endpoint={} token_env={} probe={} bearer_token={}",
                        capability.kind,
                        if capability.aliases.is_empty() {
                            "-".to_string()
                        } else {
                            capability.aliases.join(",")
                        },
                        capability.supports_endpoint,
                        capability.supports_token_env,
                        capability.supports_test_probe,
                        capability.supports_bearer_token
                    );
                }
            }
        }
        ChannelsCommand::Resolve { channel, query } => {
            let query = query.join(" ");
            let entries = repository.resolve(&channel, &query)?;
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "entries": entries,
                    "channel": channel,
                    "query": query,
                }));
            } else if entries.is_empty() {
                println!("No channels resolved.");
            } else {
                for entry in entries {
                    println!(
                        "{} name={} kind={} endpoint={} last_send={} last_error={}",
                        entry.id,
                        entry.name,
                        entry.kind,
                        entry.endpoint_masked.unwrap_or_else(|| "-".to_string()),
                        entry
                            .last_send_at
                            .map(|value| value.to_rfc3339())
                            .unwrap_or_else(|| "-".to_string()),
                        entry.last_error.unwrap_or_else(|| "-".to_string())
                    );
                }
            }
        }
        ChannelsCommand::Remove { channel_id } => {
            let removed = repository.remove(&channel_id)?;
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "removed": format_channel_for_output(&removed),
                }));
            } else {
                println!("Removed channel {}", removed.id);
            }
        }
        ChannelsCommand::Logout { channel_id } => {
            let logged_out = repository.logout(&channel_id)?;
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "channel": format_channel_for_output(&logged_out),
                }));
            } else {
                println!("Cleared token env for channel {}", logged_out.id);
            }
        }
        ChannelsCommand::Test {
            channel_id,
            token_env,
        } => {
            let probe_text = "mosaic channel connectivity probe";
            let result = repository
                .send(&channel_id, probe_text, token_env, true)
                .await?;
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "channel_id": result.channel_id,
                    "kind": result.kind,
                    "probe": result.probe,
                    "attempts": result.attempts,
                    "http_status": result.http_status,
                    "endpoint_masked": result.endpoint_masked,
                    "event_path": result.event_path,
                }));
            } else {
                println!("Channel test passed for {}", result.channel_id);
                println!("attempts: {}", result.attempts);
                if let Some(endpoint) = result.endpoint_masked {
                    println!("endpoint: {endpoint}");
                }
            }
        }
    }
    Ok(())
}

async fn handle_logs(cli: &Cli, args: LogsArgs) -> Result<()> {
    let paths = resolve_state_paths(cli.project_state)?;
    paths.ensure_dirs()?;

    if !args.follow {
        let entries = collect_logs(&paths.data_dir, args.tail)?;
        if cli.json {
            print_json(&json!({
                "ok": true,
                "logs": entries,
            }));
        } else if entries.is_empty() {
            println!("No logs found.");
        } else {
            for entry in entries {
                println!(
                    "{} [{}] {}",
                    entry
                        .ts
                        .map(|value| value.to_rfc3339())
                        .unwrap_or_else(|| "-".to_string()),
                    entry.source,
                    entry.payload
                );
            }
        }
        return Ok(());
    }

    let mut printed = 0usize;
    loop {
        let entries = collect_logs(&paths.data_dir, args.tail.max(200))?;
        if entries.len() > printed {
            for entry in entries.iter().skip(printed) {
                println!(
                    "{} [{}] {}",
                    entry
                        .ts
                        .map(|value| value.to_rfc3339())
                        .unwrap_or_else(|| "-".to_string()),
                    entry.source,
                    entry.payload
                );
            }
            printed = entries.len();
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
}

fn handle_system(cli: &Cli, args: SystemArgs) -> Result<()> {
    let paths = resolve_state_paths(cli.project_state)?;
    paths.ensure_dirs()?;
    let store = SystemEventStore::new(system_events_path(&paths.data_dir));
    match args.command {
        SystemCommand::Event { name, data } => {
            let data = data
                .as_deref()
                .map(|value| parse_json_input(value, "system event data"))
                .transpose()?
                .unwrap_or(Value::Null);
            let event = store.append_event(&name, data)?;
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "event": event,
                    "path": store.path().display().to_string(),
                }));
            } else {
                println!("event recorded: {}", event.name);
                println!("path: {}", store.path().display());
            }
        }
        SystemCommand::Presence => {
            let cwd = std::env::current_dir().map_err(|err| MosaicError::Io(err.to_string()))?;
            let presence = snapshot_presence(&cwd);
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "presence": presence,
                }));
            } else {
                println!("presence:");
                println!("hostname: {}", presence.hostname);
                println!("pid: {}", presence.pid);
                println!("cwd: {}", presence.cwd);
                println!("ts: {}", presence.ts.to_rfc3339());
            }
        }
    }
    Ok(())
}

fn handle_approvals(cli: &Cli, args: ApprovalsArgs) -> Result<()> {
    let paths = resolve_state_paths(cli.project_state)?;
    paths.ensure_dirs()?;
    let store = ApprovalStore::new(paths.approvals_policy_path.clone());
    let policy = match args.command {
        ApprovalsCommand::Get => store.load_or_default()?,
        ApprovalsCommand::Set { mode } => store.set_mode(mode.into())?,
        ApprovalsCommand::Allowlist { command } => match command {
            AllowlistCommand::Add { prefix } => store.add_allowlist(&prefix)?,
            AllowlistCommand::Remove { prefix } => store.remove_allowlist(&prefix)?,
        },
    };

    if cli.json {
        print_json(&json!({
            "ok": true,
            "policy": policy,
            "path": store.path().display().to_string(),
        }));
    } else {
        println!("approvals mode: {:?}", policy.mode);
        if policy.allowlist.is_empty() {
            println!("allowlist: <empty>");
        } else {
            println!("allowlist:");
            for item in policy.allowlist {
                println!("- {item}");
            }
        }
        println!("path: {}", store.path().display());
    }
    Ok(())
}

fn handle_sandbox(cli: &Cli, args: SandboxArgs) -> Result<()> {
    let paths = resolve_state_paths(cli.project_state)?;
    paths.ensure_dirs()?;
    let store = SandboxStore::new(paths.sandbox_policy_path.clone());
    let policy = store.load_or_default()?;
    match args.command {
        SandboxCommand::List => {
            let profiles = list_profiles();
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "current": policy.profile,
                    "profiles": profiles,
                    "path": store.path().display().to_string(),
                }));
            } else {
                println!("current sandbox profile: {:?}", policy.profile);
                for profile in profiles {
                    println!("- {:?}: {}", profile.profile, profile.description);
                }
            }
        }
        SandboxCommand::Explain { profile } => {
            let profile = profile.map(Into::into).unwrap_or(policy.profile);
            let info = mosaic_ops::profile_info(profile);
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "profile": info,
                    "path": store.path().display().to_string(),
                }));
            } else {
                println!("sandbox profile: {:?}", info.profile);
                println!("{}", info.description);
                if info.blocked_examples.is_empty() {
                    println!("blocked examples: <none>");
                } else {
                    println!("blocked examples:");
                    for example in info.blocked_examples {
                        println!("- {example}");
                    }
                }
            }
        }
    }
    Ok(())
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

fn gateway_test_mode() -> bool {
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

fn run_gateway_http_server(host: &str, port: u16) -> Result<()> {
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
                    "methods": ["health", "status", "echo"],
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

fn handle_status(cli: &Cli) -> Result<()> {
    let paths = resolve_state_paths(cli.project_state)?;
    let manager = ConfigManager::new(paths.config_path.clone());
    let store = SessionStore::new(paths.sessions_dir.clone());
    let latest_session = store.latest_session_id()?;
    if !manager.exists() {
        if cli.json {
            print_json(&json!({
                "ok": true,
                "configured": false,
                "state_mode": paths.mode,
                "config_path": manager.path().display().to_string(),
                "latest_session": latest_session,
            }));
        } else {
            println!("configured: no");
            println!("config path: {}", manager.path().display());
            println!("state mode: {:?}", paths.mode);
        }
        return Ok(());
    }

    let config = manager.load()?;
    let resolved = config.resolve_profile(Some(&cli.profile))?;
    if cli.json {
        print_json(&json!({
            "ok": true,
            "configured": true,
            "profile": resolved.profile_name,
            "provider": resolved.profile.provider,
            "tools": resolved.profile.tools,
            "state_mode": paths.mode,
            "config_path": manager.path().display().to_string(),
            "latest_session": latest_session,
        }));
    } else {
        println!("configured: yes");
        println!("profile: {}", resolved.profile_name);
        println!("provider: {:?}", resolved.profile.provider.kind);
        println!("base url: {}", resolved.profile.provider.base_url);
        println!("model: {}", resolved.profile.provider.model);
        println!("state mode: {:?}", paths.mode);
        if let Some(latest) = latest_session {
            println!("latest session: {latest}");
        }
    }
    Ok(())
}

async fn handle_health(cli: &Cli) -> Result<()> {
    let paths = resolve_state_paths(cli.project_state)?;
    let manager = ConfigManager::new(paths.config_path.clone());
    let mut checks = vec![];
    checks.push(run_check(
        "state_dirs",
        paths.ensure_dirs().is_ok(),
        "state paths ready",
    ));
    checks.push(run_check(
        "state_writable",
        paths.is_writable().is_ok(),
        "state paths writable",
    ));

    if manager.exists() {
        let config = manager.load()?;
        checks.push(run_check("config", true, "config valid"));
        let resolved = config.resolve_profile(Some(&cli.profile))?;
        let provider = OpenAiCompatibleProvider::from_profile(&resolved.profile)?;
        let health = provider.health().await?;
        checks.push(run_check(
            "provider",
            health.ok,
            format!(
                "{} (latency={}ms)",
                health.detail,
                health.latency_ms.unwrap_or(0)
            ),
        ));
    } else {
        checks.push(run_check("config", false, "run `mosaic setup` first"));
    }

    emit_checks(cli.json, "health", checks)
}

async fn handle_doctor(cli: &Cli) -> Result<()> {
    let paths = resolve_state_paths(cli.project_state)?;
    let manager = ConfigManager::new(paths.config_path.clone());
    let channels_repo = ChannelRepository::new(
        channels_file_path(&paths.data_dir),
        channels_events_dir(&paths.data_dir),
    );
    let mut checks = vec![];

    checks.push(run_check(
        "config_exists",
        manager.exists(),
        "config file presence",
    ));
    checks.push(run_check(
        "state_writable",
        paths.is_writable().is_ok(),
        "state directories writable",
    ));
    checks.push(run_check(
        "rg_binary",
        binary_in_path("rg"),
        "ripgrep available for search_text tool",
    ));

    if manager.exists() {
        let config = manager.load()?;
        let resolved = config.resolve_profile(Some(&cli.profile))?;
        let api_key_exists = std::env::var(&resolved.profile.provider.api_key_env).is_ok();
        checks.push(run_check(
            "api_key_env",
            api_key_exists,
            format!(
                "environment variable {} {}",
                resolved.profile.provider.api_key_env,
                if api_key_exists { "found" } else { "missing" }
            ),
        ));

        if api_key_exists {
            let provider = OpenAiCompatibleProvider::from_profile(&resolved.profile)?;
            let provider_health = provider.health().await?;
            checks.push(run_check(
                "provider_connectivity",
                provider_health.ok,
                provider_health.detail,
            ));
        } else {
            checks.push(run_check(
                "provider_connectivity",
                false,
                "skipped because API key env is missing",
            ));
        }
    }

    match channels_repo.doctor_checks() {
        Ok(channel_checks) => {
            for check in channel_checks {
                checks.push(run_check(check.name, check.ok, check.detail));
            }
        }
        Err(err) => {
            checks.push(run_check(
                "channels_file",
                false,
                format!("failed to inspect channels: {err}"),
            ));
        }
    }

    let approval_store = ApprovalStore::new(paths.approvals_policy_path.clone());
    match approval_store.load_or_default() {
        Ok(policy) => {
            checks.push(run_check(
                "approvals_policy",
                true,
                format!(
                    "mode={:?} allowlist_size={} path={}",
                    policy.mode,
                    policy.allowlist.len(),
                    approval_store.path().display()
                ),
            ));
        }
        Err(err) => {
            checks.push(run_check(
                "approvals_policy",
                false,
                format!("failed to load approvals policy: {err}"),
            ));
        }
    }

    let sandbox_store = SandboxStore::new(paths.sandbox_policy_path.clone());
    match sandbox_store.load_or_default() {
        Ok(policy) => {
            checks.push(run_check(
                "sandbox_policy",
                true,
                format!(
                    "profile={:?} path={}",
                    policy.profile,
                    sandbox_store.path().display()
                ),
            ));
        }
        Err(err) => {
            checks.push(run_check(
                "sandbox_policy",
                false,
                format!("failed to load sandbox policy: {err}"),
            ));
        }
    }

    emit_checks(cli.json, "doctor", checks)
}

fn run_check(
    name: impl Into<String>,
    ok: bool,
    detail: impl Into<String>,
) -> BTreeMap<String, Value> {
    let mut map = BTreeMap::new();
    map.insert("name".to_string(), Value::String(name.into()));
    map.insert(
        "status".to_string(),
        Value::String(if ok { "ok" } else { "warn" }.to_string()),
    );
    map.insert("detail".to_string(), Value::String(detail.into()));
    map
}

fn emit_checks(json_mode: bool, kind: &str, checks: Vec<BTreeMap<String, Value>>) -> Result<()> {
    if json_mode {
        print_json(&json!({
            "ok": true,
            "type": kind,
            "checks": checks,
        }));
    } else {
        println!("{kind}:");
        for check in checks {
            let status = check
                .get("status")
                .and_then(Value::as_str)
                .unwrap_or("warn")
                .to_uppercase();
            let name = check.get("name").and_then(Value::as_str).unwrap_or("-");
            let detail = check.get("detail").and_then(Value::as_str).unwrap_or("-");
            println!("[{status}] {name}: {detail}");
        }
    }
    Ok(())
}

fn resolve_state_paths(project_state: bool) -> Result<StatePaths> {
    let mode = if project_state {
        StateMode::Project
    } else {
        StateMode::Xdg
    };
    let cwd = std::env::current_dir().map_err(|err| MosaicError::Io(err.to_string()))?;
    StatePaths::resolve(mode, &cwd, PROJECT_STATE_DIR)
}

fn resolve_gateway_target(gateway_path: &std::path::Path) -> Result<(String, u16)> {
    let state: Option<GatewayState> = load_json_file_opt(gateway_path)?;
    if let Some(state) = state {
        return Ok((state.host, state.port));
    }
    Ok(("127.0.0.1".to_string(), 8787))
}

fn parse_json_input(raw: &str, field_name: &str) -> Result<Value> {
    serde_json::from_str(raw).map_err(|err| {
        MosaicError::Validation(format!(
            "{field_name} must be valid JSON, parse error: {err}"
        ))
    })
}

fn build_runtime(cli: &Cli) -> Result<RuntimeContext> {
    let state_paths = resolve_state_paths(cli.project_state)?;
    state_paths.ensure_dirs()?;
    let manager = ConfigManager::new(state_paths.config_path.clone());
    let config = manager.load()?;
    let resolved = config.resolve_profile(Some(&cli.profile))?;
    let provider: Arc<dyn Provider> =
        Arc::new(OpenAiCompatibleProvider::from_profile(&resolved.profile)?);
    let session_store = SessionStore::new(state_paths.sessions_dir.clone());
    let audit_store = AuditStore::new(
        state_paths.audit_dir.clone(),
        state_paths.audit_log_path.clone(),
    );
    let approval_store = ApprovalStore::new(state_paths.approvals_policy_path.clone());
    let sandbox_store = SandboxStore::new(state_paths.sandbox_policy_path.clone());
    let tool_executor = ToolExecutor::new(
        resolved.profile.tools.run.guard_mode.clone(),
        Some(RuntimePolicy {
            approval: approval_store.load_or_default()?,
            sandbox: sandbox_store.load_or_default()?,
        }),
    );
    let agent = AgentRunner::new(
        provider.clone(),
        resolved.profile.clone(),
        session_store,
        audit_store,
        tool_executor,
    );
    Ok(RuntimeContext { provider, agent })
}

fn load_json_file_opt<T>(path: &std::path::Path) -> Result<Option<T>>
where
    T: DeserializeOwned,
{
    if !path.exists() {
        return Ok(None);
    }
    let raw = std::fs::read_to_string(path)?;
    let parsed = serde_json::from_str::<T>(&raw).map_err(|err| {
        MosaicError::Validation(format!("invalid JSON {}: {err}", path.display()))
    })?;
    Ok(Some(parsed))
}

fn save_json_file<T>(path: &std::path::Path, value: &T) -> Result<()>
where
    T: Serialize,
{
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let raw = serde_json::to_string_pretty(value).map_err(|err| {
        MosaicError::Validation(format!(
            "failed to serialize JSON {}: {err}",
            path.display()
        ))
    })?;
    std::fs::write(path, raw)?;
    Ok(())
}

fn print_json(value: &Value) {
    let rendered = serde_json::to_string_pretty(value).unwrap_or_else(|_| "{}".to_string());
    println!("{rendered}");
}

fn binary_in_path(name: &str) -> bool {
    if PathBuf::from(name).is_absolute() {
        return PathBuf::from(name).exists();
    }
    std::env::var_os("PATH")
        .and_then(|paths| {
            std::env::split_paths(&paths).find_map(|dir| {
                let full = dir.join(name);
                if full.exists() { Some(full) } else { None }
            })
        })
        .is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cli_accepts_openclaw_aliases() {
        let alias_ask = Cli::try_parse_from(["mosaic", "message", "hello"]).unwrap();
        assert!(matches!(alias_ask.command, Commands::Ask(_)));

        let alias_chat = Cli::try_parse_from(["mosaic", "agent"]).unwrap();
        assert!(matches!(alias_chat.command, Commands::Chat(_)));

        let alias_setup = Cli::try_parse_from(["mosaic", "onboard"]).unwrap();
        assert!(matches!(alias_setup.command, Commands::Setup(_)));
    }
}
