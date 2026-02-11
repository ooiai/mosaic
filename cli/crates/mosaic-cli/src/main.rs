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

use mosaic_agents::{
    AddAgentInput, AgentStore, UpdateAgentInput, agent_routes_path, agents_file_path,
};
use mosaic_channels::{
    AddChannelInput, ChannelRepository, ChannelSendOptions, ChannelTemplateDefaults,
    UpdateChannelInput, channels_events_dir, channels_file_path, format_channel_for_output,
};
use mosaic_gateway::{GatewayClient, GatewayRequest, HttpGatewayClient};
use mosaic_memory::{MemoryIndexOptions, MemoryStore, memory_index_path, memory_status_path};
use mosaic_plugins::{ExtensionRegistry, RegistryRoots};

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
use mosaic_security::{
    SecurityAuditOptions, SecurityAuditor, SecurityBaselineConfig, apply_baseline, report_to_sarif,
};
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
    Memory(MemoryArgs),
    Security(SecurityArgs),
    Agents(AgentsArgs),
    Plugins(PluginsArgs),
    Skills(SkillsArgs),
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
    #[arg(long)]
    agent: Option<String>,
}

#[derive(Args, Debug, Clone)]
struct ChatArgs {
    #[arg(long)]
    session: Option<String>,
    #[arg(long)]
    prompt: Option<String>,
    #[arg(long)]
    agent: Option<String>,
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
        chat_id: Option<String>,
        #[arg(long)]
        token_env: Option<String>,
        #[arg(long)]
        default_parse_mode: Option<String>,
        #[arg(long)]
        default_title: Option<String>,
        #[arg(long)]
        default_block: Vec<String>,
        #[arg(long)]
        default_metadata: Option<String>,
    },
    Update {
        channel_id: String,
        #[arg(long)]
        name: Option<String>,
        #[arg(long)]
        endpoint: Option<String>,
        #[arg(long)]
        chat_id: Option<String>,
        #[arg(long)]
        token_env: Option<String>,
        #[arg(long, conflicts_with = "token_env")]
        clear_token_env: bool,
        #[arg(long)]
        default_parse_mode: Option<String>,
        #[arg(long)]
        default_title: Option<String>,
        #[arg(long)]
        default_block: Vec<String>,
        #[arg(long)]
        default_metadata: Option<String>,
        #[arg(
            long,
            conflicts_with_all = [
                "default_parse_mode",
                "default_title",
                "default_block",
                "default_metadata"
            ]
        )]
        clear_defaults: bool,
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
        parse_mode: Option<String>,
        #[arg(long)]
        title: Option<String>,
        #[arg(long)]
        block: Vec<String>,
        #[arg(long)]
        metadata: Option<String>,
        #[arg(long)]
        idempotency_key: Option<String>,
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
    Export {
        #[arg(long)]
        out: Option<PathBuf>,
    },
    Import {
        #[arg(long)]
        file: PathBuf,
        #[arg(long)]
        replace: bool,
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

#[derive(Args, Debug, Clone)]
struct MemoryArgs {
    #[command(subcommand)]
    command: MemoryCommand,
}

#[derive(Subcommand, Debug, Clone)]
enum MemoryCommand {
    Index {
        #[arg(long, default_value = ".")]
        path: String,
        #[arg(long, default_value_t = 500)]
        max_files: usize,
        #[arg(long, default_value_t = 262_144)]
        max_file_size: usize,
        #[arg(long, default_value_t = 16_384)]
        max_content_bytes: usize,
    },
    Search {
        query: String,
        #[arg(long, default_value_t = 20)]
        limit: usize,
    },
    Status,
}

#[derive(Args, Debug, Clone)]
struct SecurityArgs {
    #[command(subcommand)]
    command: SecurityCommand,
}

#[derive(Subcommand, Debug, Clone)]
enum SecurityCommand {
    Audit {
        #[arg(long, default_value = ".")]
        path: String,
        #[arg(long)]
        deep: bool,
        #[arg(long, default_value_t = 800)]
        max_files: usize,
        #[arg(long, default_value_t = 262_144)]
        max_file_size: usize,
        #[arg(long)]
        baseline: Option<String>,
        #[arg(long)]
        no_baseline: bool,
        #[arg(long)]
        update_baseline: bool,
        #[arg(long)]
        sarif: bool,
        #[arg(long)]
        sarif_output: Option<String>,
    },
    Baseline {
        #[command(subcommand)]
        command: SecurityBaselineCommand,
    },
}

#[derive(Subcommand, Debug, Clone)]
enum SecurityBaselineCommand {
    Show {
        #[arg(long)]
        path: Option<String>,
    },
    Add {
        #[arg(long)]
        path: Option<String>,
        #[arg(long = "fingerprint")]
        fingerprints: Vec<String>,
        #[arg(long = "category")]
        categories: Vec<String>,
        #[arg(long = "match-path")]
        match_paths: Vec<String>,
    },
    Remove {
        #[arg(long)]
        path: Option<String>,
        #[arg(long = "fingerprint")]
        fingerprints: Vec<String>,
        #[arg(long = "category")]
        categories: Vec<String>,
        #[arg(long = "match-path")]
        match_paths: Vec<String>,
    },
    Clear {
        #[arg(long)]
        path: Option<String>,
    },
}

#[derive(Args, Debug, Clone)]
struct AgentsArgs {
    #[command(subcommand)]
    command: AgentsCommand,
}

#[derive(Subcommand, Debug, Clone)]
enum AgentsCommand {
    List,
    Add {
        #[arg(long)]
        name: String,
        #[arg(long)]
        id: Option<String>,
        #[arg(long)]
        profile: Option<String>,
        #[arg(long)]
        model: Option<String>,
        #[arg(long)]
        temperature: Option<f32>,
        #[arg(long)]
        max_turns: Option<u32>,
        #[arg(long)]
        tools_enabled: Option<bool>,
        #[arg(long, value_enum)]
        guard_mode: Option<GuardModeArg>,
        #[arg(long)]
        set_default: bool,
        #[arg(long = "route")]
        route_keys: Vec<String>,
    },
    Update {
        agent_id: String,
        #[arg(long)]
        name: Option<String>,
        #[arg(long)]
        profile: Option<String>,
        #[arg(long)]
        model: Option<String>,
        #[arg(long)]
        clear_model: bool,
        #[arg(long)]
        temperature: Option<f32>,
        #[arg(long)]
        clear_temperature: bool,
        #[arg(long)]
        max_turns: Option<u32>,
        #[arg(long)]
        clear_max_turns: bool,
        #[arg(long)]
        tools_enabled: Option<bool>,
        #[arg(long)]
        clear_tools_enabled: bool,
        #[arg(long, value_enum)]
        guard_mode: Option<GuardModeArg>,
        #[arg(long)]
        clear_guard_mode: bool,
        #[arg(long)]
        set_default: bool,
        #[arg(long = "route")]
        route_keys: Vec<String>,
    },
    Show {
        agent_id: String,
    },
    Remove {
        agent_id: String,
    },
    Default {
        agent_id: Option<String>,
    },
    Route {
        #[command(subcommand)]
        command: AgentsRouteCommand,
    },
}

#[derive(Subcommand, Debug, Clone)]
enum AgentsRouteCommand {
    List,
    Set {
        route_key: String,
        agent_id: String,
    },
    Remove {
        route_key: String,
    },
    Resolve {
        #[arg(long)]
        route: Option<String>,
    },
}

#[derive(Args, Debug, Clone)]
struct PluginsArgs {
    #[command(subcommand)]
    command: PluginsCommand,
}

#[derive(Subcommand, Debug, Clone)]
enum PluginsCommand {
    List,
    Info {
        plugin_id: String,
    },
    Check {
        plugin_id: Option<String>,
    },
    Install {
        #[arg(long)]
        path: String,
        #[arg(long)]
        force: bool,
    },
    Remove {
        plugin_id: String,
    },
}

#[derive(Args, Debug, Clone)]
struct SkillsArgs {
    #[command(subcommand)]
    command: SkillsCommand,
}

#[derive(Subcommand, Debug, Clone)]
enum SkillsCommand {
    List,
    Info {
        skill_id: String,
    },
    Check {
        skill_id: Option<String>,
    },
    Install {
        #[arg(long)]
        path: String,
        #[arg(long)]
        force: bool,
    },
    Remove {
        skill_id: String,
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
    active_agent_id: Option<String>,
    active_profile_name: String,
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
        Commands::Memory(args) => handle_memory(&cli, args),
        Commands::Security(args) => handle_security(&cli, args),
        Commands::Agents(args) => handle_agents(&cli, args),
        Commands::Plugins(args) => handle_plugins(&cli, args),
        Commands::Skills(args) => handle_skills(&cli, args),
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
            let runtime = build_runtime(cli, None, None)?;
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
    let runtime = build_runtime(cli, args.agent.as_deref(), Some("ask"))?;
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
            "agent_id": runtime.active_agent_id,
            "profile": runtime.active_profile_name,
        }));
    } else {
        println!("{}", result.response.trim());
        println!("session: {}", result.session_id);
        if let Some(agent_id) = &runtime.active_agent_id {
            println!("agent: {agent_id}");
        }
    }
    Ok(())
}

async fn handle_chat(cli: &Cli, args: ChatArgs) -> Result<()> {
    let runtime = build_runtime(cli, args.agent.as_deref(), Some("chat"))?;
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
                "agent_id": runtime.active_agent_id,
                "profile": runtime.active_profile_name,
            }));
            return Ok(());
        }
        println!("{}", result.response.trim());
        println!("session: {}", result.session_id);
        if let Some(agent_id) = &runtime.active_agent_id {
            println!("agent: {agent_id}");
        }
    } else if cli.json {
        return Err(MosaicError::Validation(
            "chat in --json mode requires --prompt".to_string(),
        ));
    }

    println!("Entering chat mode. Type /help for commands, /exit to quit.");
    if let Some(id) = &session_id {
        println!("Resumed session: {id}");
    }
    if let Some(agent_id) = &runtime.active_agent_id {
        println!("Using agent: {agent_id}");
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
                    agent: None,
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
                        "{} name={} kind={} endpoint={} target={} defaults={} last_login={} last_send={} last_error={}",
                        channel.id,
                        channel.name,
                        channel.kind,
                        channel.endpoint_masked.unwrap_or_else(|| "-".to_string()),
                        channel.target_masked.unwrap_or_else(|| "-".to_string()),
                        channel.has_template_defaults,
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
            chat_id,
            token_env,
            default_parse_mode,
            default_title,
            default_block,
            default_metadata,
        } => {
            let default_metadata = default_metadata
                .map(|value| parse_json_input(&value, "channels add default metadata"))
                .transpose()?;
            let entry = repository.add(AddChannelInput {
                name,
                kind,
                endpoint,
                target: chat_id,
                token_env,
                template_defaults: ChannelTemplateDefaults {
                    parse_mode: default_parse_mode,
                    title: default_title,
                    blocks: default_block,
                    metadata: default_metadata,
                },
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
        ChannelsCommand::Update {
            channel_id,
            name,
            endpoint,
            chat_id,
            token_env,
            clear_token_env,
            default_parse_mode,
            default_title,
            default_block,
            default_metadata,
            clear_defaults,
        } => {
            let default_metadata = default_metadata
                .map(|value| parse_json_input(&value, "channels update default metadata"))
                .transpose()?;
            let template_defaults = if default_parse_mode.is_some()
                || default_title.is_some()
                || !default_block.is_empty()
                || default_metadata.is_some()
            {
                Some(ChannelTemplateDefaults {
                    parse_mode: default_parse_mode,
                    title: default_title,
                    blocks: default_block,
                    metadata: default_metadata,
                })
            } else {
                None
            };
            let updated = repository.update(
                &channel_id,
                UpdateChannelInput {
                    name,
                    endpoint,
                    target: chat_id,
                    token_env,
                    clear_token_env,
                    template_defaults,
                    clear_template_defaults: clear_defaults,
                },
            )?;
            let rendered = format_channel_for_output(&updated);
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "channel": rendered,
                }));
            } else {
                println!("Channel updated: {}", rendered.id);
            }
        }
        ChannelsCommand::Login {
            channel_id,
            token_env,
        } => {
            let login = repository.login(&channel_id, token_env.as_deref())?;
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
            parse_mode,
            title,
            block,
            metadata,
            idempotency_key,
            token_env,
        } => {
            let metadata = metadata
                .map(|value| parse_json_input(&value, "channels send metadata"))
                .transpose()?;
            let result = repository
                .send_with_options(
                    &channel_id,
                    &text,
                    token_env,
                    false,
                    ChannelSendOptions {
                        parse_mode,
                        title,
                        blocks: block,
                        idempotency_key,
                        metadata,
                    },
                )
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
                    "target_masked": result.target_masked,
                    "parse_mode": result.parse_mode,
                    "idempotency_key": result.idempotency_key,
                    "deduplicated": result.deduplicated,
                    "rate_limited_ms": result.rate_limited_ms,
                    "event_path": result.event_path,
                }));
            } else {
                println!("Message sent via {}", result.delivered_via);
                if let Some(endpoint) = result.endpoint_masked {
                    println!("endpoint: {endpoint}");
                }
                if let Some(target) = result.target_masked {
                    println!("target: {target}");
                }
                if let Some(parse_mode) = result.parse_mode {
                    println!("parse_mode: {parse_mode}");
                }
                if let Some(key) = result.idempotency_key {
                    println!("idempotency_key: {key}");
                }
                if result.deduplicated {
                    println!("deduplicated: true");
                }
                if let Some(waited) = result.rate_limited_ms {
                    println!("rate_limited_ms: {waited}");
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
                        "{} channel={} kind={} status={} attempt={} http={} parse_mode={} idempotency_key={} deduplicated={} rate_limited_ms={} error={} preview={}",
                        event.ts.to_rfc3339(),
                        event.channel_id,
                        event.kind,
                        event.delivery_status,
                        event.attempt,
                        event
                            .http_status
                            .map(|value| value.to_string())
                            .unwrap_or_else(|| "-".to_string()),
                        event.parse_mode.unwrap_or_else(|| "-".to_string()),
                        event.idempotency_key.unwrap_or_else(|| "-".to_string()),
                        event.deduplicated,
                        event
                            .rate_limited_ms
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
                        "{} aliases={} endpoint={} token_env={} probe={} bearer_token={} parse_mode={} template={} idempotency={} rate_limit_report={}",
                        capability.kind,
                        if capability.aliases.is_empty() {
                            "-".to_string()
                        } else {
                            capability.aliases.join(",")
                        },
                        capability.supports_endpoint,
                        capability.supports_token_env,
                        capability.supports_test_probe,
                        capability.supports_bearer_token,
                        capability.supports_parse_mode,
                        capability.supports_message_template,
                        capability.supports_idempotency_key,
                        capability.supports_rate_limit_report
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
                        "{} name={} kind={} endpoint={} target={} last_send={} last_error={}",
                        entry.id,
                        entry.name,
                        entry.kind,
                        entry.endpoint_masked.unwrap_or_else(|| "-".to_string()),
                        entry.target_masked.unwrap_or_else(|| "-".to_string()),
                        entry
                            .last_send_at
                            .map(|value| value.to_rfc3339())
                            .unwrap_or_else(|| "-".to_string()),
                        entry.last_error.unwrap_or_else(|| "-".to_string())
                    );
                }
            }
        }
        ChannelsCommand::Export { out } => {
            let file = repository.export_channels()?;
            let payload = json!({
                "schema": "mosaic.channels.export.v1",
                "exported_at": Utc::now(),
                "channels_file": file,
            });
            if let Some(path) = out {
                if let Some(parent) = path.parent().filter(|value| !value.as_os_str().is_empty()) {
                    std::fs::create_dir_all(parent)?;
                }
                let rendered = serde_json::to_string_pretty(&payload).map_err(|err| {
                    MosaicError::Validation(format!("failed to encode channels export JSON: {err}"))
                })?;
                std::fs::write(&path, rendered)?;
                if cli.json {
                    print_json(&json!({
                        "ok": true,
                        "path": path.display().to_string(),
                        "channels": payload["channels_file"]["channels"].as_array().map_or(0usize, |v| v.len()),
                    }));
                } else {
                    println!(
                        "Exported {} channels to {}",
                        payload["channels_file"]["channels"]
                            .as_array()
                            .map_or(0usize, |items| items.len()),
                        path.display()
                    );
                }
            } else if cli.json {
                print_json(&json!({
                    "ok": true,
                    "export": payload,
                }));
            } else {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&payload).map_err(|err| {
                        MosaicError::Validation(format!(
                            "failed to render channels export JSON: {err}"
                        ))
                    })?
                );
            }
        }
        ChannelsCommand::Import { file, replace } => {
            let raw = std::fs::read_to_string(&file).map_err(|err| {
                MosaicError::Config(format!(
                    "failed to read channels import file {}: {err}",
                    file.display()
                ))
            })?;
            let value = serde_json::from_str::<Value>(&raw).map_err(|err| {
                MosaicError::Validation(format!(
                    "invalid channels import JSON {}: {err}",
                    file.display()
                ))
            })?;
            let import_value = value
                .as_object()
                .and_then(|obj| obj.get("channels_file"))
                .cloned()
                .unwrap_or(value);
            let summary = repository.import_channels_json(import_value, replace)?;
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "file": file.display().to_string(),
                    "summary": summary,
                }));
            } else {
                println!(
                    "Import complete from {}: total={} imported={} updated={} skipped={} replace={}",
                    file.display(),
                    summary.total,
                    summary.imported,
                    summary.updated,
                    summary.skipped,
                    summary.replace
                );
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
                    "target_masked": result.target_masked,
                    "parse_mode": result.parse_mode,
                    "idempotency_key": result.idempotency_key,
                    "deduplicated": result.deduplicated,
                    "rate_limited_ms": result.rate_limited_ms,
                    "event_path": result.event_path,
                }));
            } else {
                println!("Channel test passed for {}", result.channel_id);
                println!("attempts: {}", result.attempts);
                if let Some(endpoint) = result.endpoint_masked {
                    println!("endpoint: {endpoint}");
                }
                if let Some(target) = result.target_masked {
                    println!("target: {target}");
                }
                if let Some(waited) = result.rate_limited_ms {
                    println!("rate_limited_ms: {waited}");
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

fn handle_memory(cli: &Cli, args: MemoryArgs) -> Result<()> {
    let paths = resolve_state_paths(cli.project_state)?;
    paths.ensure_dirs()?;
    let store = MemoryStore::new(
        memory_index_path(&paths.data_dir),
        memory_status_path(&paths.data_dir),
    );

    match args.command {
        MemoryCommand::Index {
            path,
            max_files,
            max_file_size,
            max_content_bytes,
        } => {
            let cwd = std::env::current_dir().map_err(|err| MosaicError::Io(err.to_string()))?;
            let root = {
                let raw = PathBuf::from(path);
                if raw.is_absolute() {
                    raw
                } else {
                    cwd.join(raw)
                }
            };
            let result = store.index(MemoryIndexOptions {
                root,
                max_files,
                max_file_size,
                max_content_bytes,
            })?;
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "index": result,
                }));
            } else {
                println!("memory indexed documents: {}", result.indexed_documents);
                println!("memory skipped files: {}", result.skipped_files);
                println!("index path: {}", result.index_path);
            }
        }
        MemoryCommand::Search { query, limit } => {
            let result = store.search(&query, Some(limit))?;
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "result": result,
                }));
            } else if result.hits.is_empty() {
                println!("No memory hits.");
            } else {
                println!(
                    "memory search hits: {} (showing {})",
                    result.total_hits,
                    result.hits.len()
                );
                for hit in result.hits {
                    println!("{} score={} {}", hit.path, hit.score, hit.snippet);
                }
            }
        }
        MemoryCommand::Status => {
            let status = store.status()?;
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "status": status,
                }));
            } else {
                println!("indexed documents: {}", status.indexed_documents);
                println!(
                    "last indexed at: {}",
                    status
                        .last_indexed_at
                        .map(|value| value.to_rfc3339())
                        .unwrap_or_else(|| "-".to_string())
                );
                println!("index path: {}", status.index_path);
            }
        }
    }
    Ok(())
}

fn handle_security(cli: &Cli, args: SecurityArgs) -> Result<()> {
    let paths = resolve_state_paths(cli.project_state)?;
    paths.ensure_dirs()?;
    let auditor = SecurityAuditor::new();
    let cwd = std::env::current_dir().map_err(|err| MosaicError::Io(err.to_string()))?;

    match args.command {
        SecurityCommand::Audit {
            path,
            deep,
            max_files,
            max_file_size,
            baseline,
            no_baseline,
            update_baseline,
            sarif,
            sarif_output,
        } => {
            if no_baseline && update_baseline {
                return Err(MosaicError::Validation(
                    "--no-baseline and --update-baseline cannot be used together".to_string(),
                ));
            }
            let root = {
                let raw = PathBuf::from(path);
                if raw.is_absolute() {
                    raw
                } else {
                    cwd.join(raw)
                }
            };
            let mut report = auditor.audit(SecurityAuditOptions {
                root,
                deep,
                max_files,
                max_file_size,
            })?;
            let baseline_path = resolve_baseline_path(&paths, &cwd, baseline);
            let baseline_path_display = baseline_path.display().to_string();
            let mut baseline_added = 0usize;
            let mut baseline_enabled = false;
            let mut sarif_output_path = None;

            if !no_baseline || update_baseline {
                let mut baseline_config =
                    SecurityBaselineConfig::load_optional(&baseline_path)?.unwrap_or_default();
                if !no_baseline {
                    baseline_enabled = true;
                    let applied = apply_baseline(report, &baseline_config);
                    report = applied.report;
                    report.summary.baseline_path = Some(baseline_path_display.clone());
                }
                if update_baseline {
                    baseline_enabled = true;
                    baseline_added = baseline_config.add_findings(&report.findings);
                    baseline_config.save_to_path(&baseline_path)?;
                    if !report.findings.is_empty() {
                        report.summary.ignored += report.findings.len();
                        report.findings.clear();
                        report.summary.findings = 0;
                        report.summary.high = 0;
                        report.summary.medium = 0;
                        report.summary.low = 0;
                        report.summary.ok = true;
                    }
                    report.summary.baseline_path = Some(baseline_path_display.clone());
                }
            }

            let sarif_value = if sarif || sarif_output.is_some() {
                Some(report_to_sarif(&report))
            } else {
                None
            };
            if let Some(raw_path) = sarif_output {
                let output_path = resolve_output_path(&cwd, &raw_path);
                if let Some(parent) = output_path.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                let encoded = serde_json::to_string_pretty(
                    sarif_value
                        .as_ref()
                        .ok_or_else(|| MosaicError::Unknown("sarif value missing".to_string()))?,
                )
                .map_err(|err| {
                    MosaicError::Validation(format!("failed to encode sarif JSON: {err}"))
                })?;
                std::fs::write(&output_path, encoded)?;
                sarif_output_path = Some(output_path.display().to_string());
            }

            if sarif {
                print_json(
                    sarif_value
                        .as_ref()
                        .ok_or_else(|| MosaicError::Unknown("sarif value missing".to_string()))?,
                );
                return Ok(());
            }

            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "report": report,
                    "baseline": {
                        "enabled": baseline_enabled,
                        "updated": update_baseline,
                        "added": baseline_added,
                        "path": if baseline_enabled {
                            Some(baseline_path_display.clone())
                        } else {
                            None
                        },
                    },
                    "sarif_output": sarif_output_path,
                }));
            } else {
                println!(
                    "security audit summary: findings={} high={} medium={} low={} ignored={} scanned={} skipped={}",
                    report.summary.findings,
                    report.summary.high,
                    report.summary.medium,
                    report.summary.low,
                    report.summary.ignored,
                    report.summary.scanned_files,
                    report.summary.skipped_files
                );
                if baseline_enabled {
                    println!("baseline: {baseline_path_display}");
                }
                if update_baseline {
                    println!("baseline updated: added {baseline_added} fingerprints");
                }
                if let Some(sarif_output_path) = sarif_output_path {
                    println!("sarif: {sarif_output_path}");
                }
                if report.findings.is_empty() {
                    println!("No security findings.");
                } else {
                    for finding in report.findings {
                        println!(
                            "[{:?}] {}:{} {} ({})",
                            finding.severity,
                            finding.path,
                            finding
                                .line
                                .map(|value| value.to_string())
                                .unwrap_or_else(|| "-".to_string()),
                            finding.title,
                            finding.category
                        );
                        if let Some(suggestion) = finding.suggestion {
                            println!("  suggestion: {suggestion}");
                        }
                    }
                }
            }
        }
        SecurityCommand::Baseline { command } => {
            handle_security_baseline(cli, &paths, &cwd, command)?;
        }
    }
    Ok(())
}

fn handle_security_baseline(
    cli: &Cli,
    paths: &StatePaths,
    cwd: &std::path::Path,
    command: SecurityBaselineCommand,
) -> Result<()> {
    match command {
        SecurityBaselineCommand::Show { path } => {
            let baseline_path = resolve_baseline_path(paths, cwd, path);
            let exists = baseline_path.exists();
            let baseline = SecurityBaselineConfig::load_optional(&baseline_path)?
                .unwrap_or_else(SecurityBaselineConfig::default);
            let stats = json!({
                "fingerprints": baseline.ignored_fingerprints.len(),
                "categories": baseline.ignored_categories.len(),
                "paths": baseline.ignored_paths.len(),
            });
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "baseline": baseline,
                    "path": baseline_path.display().to_string(),
                    "exists": exists,
                    "stats": stats,
                }));
            } else {
                println!("baseline path: {}", baseline_path.display());
                println!("exists: {}", exists);
                println!(
                    "entries: fingerprints={} categories={} paths={}",
                    baseline.ignored_fingerprints.len(),
                    baseline.ignored_categories.len(),
                    baseline.ignored_paths.len()
                );
            }
        }
        SecurityBaselineCommand::Add {
            path,
            fingerprints,
            categories,
            match_paths,
        } => {
            let baseline_path = resolve_baseline_path(paths, cwd, path);
            let mut baseline = SecurityBaselineConfig::load_optional(&baseline_path)?
                .unwrap_or_else(SecurityBaselineConfig::default);
            let fingerprints = normalize_non_empty_list(fingerprints, "fingerprint")?;
            let categories = normalize_non_empty_list(categories, "category")?;
            let match_paths = normalize_non_empty_list(match_paths, "match-path")?;
            if fingerprints.is_empty() && categories.is_empty() && match_paths.is_empty() {
                return Err(MosaicError::Validation(
                    "baseline add requires at least one of --fingerprint/--category/--match-path"
                        .to_string(),
                ));
            }

            let mut added = 0usize;
            for value in fingerprints {
                if !baseline.ignored_fingerprints.contains(&value) {
                    baseline.ignored_fingerprints.push(value);
                    added += 1;
                }
            }
            for value in categories {
                if !baseline.ignored_categories.contains(&value) {
                    baseline.ignored_categories.push(value);
                    added += 1;
                }
            }
            for value in match_paths {
                if !baseline.ignored_paths.contains(&value) {
                    baseline.ignored_paths.push(value);
                    added += 1;
                }
            }
            baseline.save_to_path(&baseline_path)?;

            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "added": added,
                    "path": baseline_path.display().to_string(),
                    "baseline": baseline,
                }));
            } else {
                println!("baseline updated: added {added} entries");
                println!("path: {}", baseline_path.display());
            }
        }
        SecurityBaselineCommand::Remove {
            path,
            fingerprints,
            categories,
            match_paths,
        } => {
            let baseline_path = resolve_baseline_path(paths, cwd, path);
            let exists = baseline_path.exists();
            let mut baseline = SecurityBaselineConfig::load_optional(&baseline_path)?
                .unwrap_or_else(SecurityBaselineConfig::default);
            let fingerprints = normalize_non_empty_list(fingerprints, "fingerprint")?;
            let categories = normalize_non_empty_list(categories, "category")?;
            let match_paths = normalize_non_empty_list(match_paths, "match-path")?;
            if fingerprints.is_empty() && categories.is_empty() && match_paths.is_empty() {
                return Err(MosaicError::Validation(
                    "baseline remove requires at least one of --fingerprint/--category/--match-path"
                        .to_string(),
                ));
            }

            let mut removed = 0usize;
            removed += remove_matching(&mut baseline.ignored_fingerprints, &fingerprints);
            removed += remove_matching(&mut baseline.ignored_categories, &categories);
            removed += remove_matching(&mut baseline.ignored_paths, &match_paths);
            if exists {
                baseline.save_to_path(&baseline_path)?;
            }

            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "removed": removed,
                    "path": baseline_path.display().to_string(),
                    "exists": exists,
                    "baseline": baseline,
                }));
            } else {
                println!("baseline updated: removed {removed} entries");
                println!("path: {}", baseline_path.display());
            }
        }
        SecurityBaselineCommand::Clear { path } => {
            let baseline_path = resolve_baseline_path(paths, cwd, path);
            let old = SecurityBaselineConfig::load_optional(&baseline_path)?
                .unwrap_or_else(SecurityBaselineConfig::default);
            let removed = old.ignored_fingerprints.len()
                + old.ignored_categories.len()
                + old.ignored_paths.len();
            let baseline = SecurityBaselineConfig::default();
            baseline.save_to_path(&baseline_path)?;
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "cleared": removed,
                    "path": baseline_path.display().to_string(),
                    "baseline": baseline,
                }));
            } else {
                println!("baseline cleared: removed {removed} entries");
                println!("path: {}", baseline_path.display());
            }
        }
    }
    Ok(())
}

fn handle_agents(cli: &Cli, args: AgentsArgs) -> Result<()> {
    let paths = resolve_state_paths(cli.project_state)?;
    paths.ensure_dirs()?;
    let manager = ConfigManager::new(paths.config_path.clone());
    let store = AgentStore::new(
        agents_file_path(&paths.data_dir),
        agent_routes_path(&paths.data_dir),
    );
    store.ensure_dirs()?;

    match args.command {
        AgentsCommand::List => {
            let agents = store.list()?;
            let routes = store.load_routes()?;
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "agents": agents,
                    "routes": routes,
                }));
            } else if agents.is_empty() {
                println!("No agents found.");
            } else {
                println!("agents: {}", agents.len());
                if let Some(default_agent_id) = &routes.default_agent_id {
                    println!("default agent: {default_agent_id}");
                }
                for agent in agents {
                    println!(
                        "- {} ({}) profile={} model={} temperature={} max_turns={}",
                        agent.id,
                        agent.name,
                        agent.profile,
                        agent.model.unwrap_or_else(|| "-".to_string()),
                        agent
                            .temperature
                            .map(|value| value.to_string())
                            .unwrap_or_else(|| "-".to_string()),
                        agent
                            .max_turns
                            .map(|value| value.to_string())
                            .unwrap_or_else(|| "-".to_string())
                    );
                }
            }
        }
        AgentsCommand::Add {
            name,
            id,
            profile,
            model,
            temperature,
            max_turns,
            tools_enabled,
            guard_mode,
            set_default,
            route_keys,
        } => {
            if !manager.exists() {
                return Err(MosaicError::Config(
                    "config file not found. run `mosaic setup` first".to_string(),
                ));
            }
            let config = manager.load()?;
            let profile = profile.unwrap_or_else(|| cli.profile.clone());
            let _ = config.resolve_profile(Some(&profile))?;

            let created = store.add(AddAgentInput {
                id,
                name,
                profile,
                model,
                temperature,
                max_turns,
                tools_enabled,
                guard_mode: guard_mode.map(Into::into),
            })?;
            if set_default {
                store.set_default(&created.id)?;
            }
            for route_key in route_keys {
                store.set_route(&route_key, &created.id)?;
            }
            let routes = store.load_routes()?;
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "agent": created,
                    "routes": routes,
                }));
            } else {
                println!("Created agent {} ({})", created.id, created.name);
                println!("profile: {}", created.profile);
            }
        }
        AgentsCommand::Update {
            agent_id,
            name,
            profile,
            model,
            clear_model,
            temperature,
            clear_temperature,
            max_turns,
            clear_max_turns,
            tools_enabled,
            clear_tools_enabled,
            guard_mode,
            clear_guard_mode,
            set_default,
            route_keys,
        } => {
            if !manager.exists() {
                return Err(MosaicError::Config(
                    "config file not found. run `mosaic setup` first".to_string(),
                ));
            }
            let config = manager.load()?;
            if let Some(profile_name) = profile.as_deref() {
                let _ = config.resolve_profile(Some(profile_name))?;
            }

            let updated = store.update(
                &agent_id,
                UpdateAgentInput {
                    name,
                    profile,
                    model,
                    clear_model,
                    temperature,
                    clear_temperature,
                    max_turns,
                    clear_max_turns,
                    tools_enabled,
                    clear_tools_enabled,
                    guard_mode: guard_mode.map(Into::into),
                    clear_guard_mode,
                },
            )?;

            if set_default {
                store.set_default(&updated.id)?;
            }
            for route_key in route_keys {
                store.set_route(&route_key, &updated.id)?;
            }
            let routes = store.load_routes()?;
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "agent": updated,
                    "routes": routes,
                }));
            } else {
                println!("Updated agent {} ({})", updated.id, updated.name);
                println!("profile: {}", updated.profile);
            }
        }
        AgentsCommand::Show { agent_id } => {
            let agent = store
                .get(&agent_id)?
                .ok_or_else(|| MosaicError::Validation(format!("agent '{agent_id}' not found")))?;
            let routes = store.load_routes()?;
            let route_keys = routes
                .routes
                .iter()
                .filter_map(|(route, id)| {
                    if id == &agent.id {
                        Some(route.clone())
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>();
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "agent": agent,
                    "is_default": routes.default_agent_id.as_deref() == Some(agent_id.as_str()),
                    "route_keys": route_keys,
                }));
            } else {
                println!("id: {}", agent.id);
                println!("name: {}", agent.name);
                println!("profile: {}", agent.profile);
                println!("model: {}", agent.model.unwrap_or_else(|| "-".to_string()));
                println!(
                    "temperature: {}",
                    agent
                        .temperature
                        .map(|value| value.to_string())
                        .unwrap_or_else(|| "-".to_string())
                );
                println!(
                    "max_turns: {}",
                    agent
                        .max_turns
                        .map(|value| value.to_string())
                        .unwrap_or_else(|| "-".to_string())
                );
                println!(
                    "tools_enabled: {}",
                    agent
                        .tools_enabled
                        .map(|value| value.to_string())
                        .unwrap_or_else(|| "-".to_string())
                );
                println!(
                    "guard_mode: {}",
                    agent
                        .guard_mode
                        .map(|value| format!("{value:?}"))
                        .unwrap_or_else(|| "-".to_string())
                );
                println!(
                    "default: {}",
                    routes.default_agent_id.as_deref() == Some(agent_id.as_str())
                );
                if route_keys.is_empty() {
                    println!("routes: <none>");
                } else {
                    println!("routes: {}", route_keys.join(", "));
                }
            }
        }
        AgentsCommand::Remove { agent_id } => {
            let removed = store.remove(&agent_id)?;
            if !removed {
                return Err(MosaicError::Validation(format!(
                    "agent '{agent_id}' not found"
                )));
            }
            let routes = store.load_routes()?;
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "removed": true,
                    "agent_id": agent_id,
                    "routes": routes,
                }));
            } else {
                println!("Removed agent {agent_id}");
            }
        }
        AgentsCommand::Default { agent_id } => match agent_id {
            Some(agent_id) => {
                let routes = store.set_default(&agent_id)?;
                if cli.json {
                    print_json(&json!({
                        "ok": true,
                        "default_agent_id": routes.default_agent_id,
                    }));
                } else {
                    println!(
                        "Default agent: {}",
                        routes.default_agent_id.unwrap_or_default()
                    );
                }
            }
            None => {
                let routes = store.load_routes()?;
                if cli.json {
                    print_json(&json!({
                        "ok": true,
                        "default_agent_id": routes.default_agent_id,
                    }));
                } else {
                    println!(
                        "default agent: {}",
                        routes
                            .default_agent_id
                            .unwrap_or_else(|| "<none>".to_string())
                    );
                }
            }
        },
        AgentsCommand::Route { command } => match command {
            AgentsRouteCommand::List => {
                let routes = store.load_routes()?;
                if cli.json {
                    print_json(&json!({
                        "ok": true,
                        "routes": routes.routes,
                        "default_agent_id": routes.default_agent_id,
                    }));
                } else if routes.routes.is_empty() {
                    println!("No route bindings.");
                } else {
                    if let Some(default_agent_id) = routes.default_agent_id {
                        println!("default: {default_agent_id}");
                    }
                    for (route, agent_id) in routes.routes {
                        println!("{route} -> {agent_id}");
                    }
                }
            }
            AgentsRouteCommand::Set {
                route_key,
                agent_id,
            } => {
                let routes = store.set_route(&route_key, &agent_id)?;
                if cli.json {
                    print_json(&json!({
                        "ok": true,
                        "route_key": route_key,
                        "agent_id": agent_id,
                        "routes": routes,
                    }));
                } else {
                    println!("Bound route {route_key} -> {agent_id}");
                }
            }
            AgentsRouteCommand::Remove { route_key } => {
                let (routes, removed) = store.remove_route(&route_key)?;
                if cli.json {
                    print_json(&json!({
                        "ok": true,
                        "removed": removed,
                        "route_key": route_key,
                        "routes": routes,
                    }));
                } else if removed {
                    println!("Removed route {route_key}");
                } else {
                    println!("Route {route_key} not found.");
                }
            }
            AgentsRouteCommand::Resolve { route } => {
                let resolved = store.resolve_for_runtime(None, route.as_deref())?;
                if cli.json {
                    print_json(&json!({
                        "ok": true,
                        "route": route,
                        "agent_id": resolved,
                    }));
                } else {
                    println!(
                        "resolved agent: {}",
                        resolved.unwrap_or_else(|| "<none>".to_string())
                    );
                }
            }
        },
    }
    Ok(())
}

fn handle_plugins(cli: &Cli, args: PluginsArgs) -> Result<()> {
    let paths = resolve_state_paths(cli.project_state)?;
    paths.ensure_dirs()?;
    let registry = ExtensionRegistry::new(RegistryRoots::from_state_root(paths.root_dir.clone()));

    match args.command {
        PluginsCommand::List => {
            let plugins = registry.list_plugins()?;
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "count": plugins.len(),
                    "plugins": plugins,
                }));
            } else if plugins.is_empty() {
                println!("No plugins found.");
            } else {
                println!("plugins: {}", plugins.len());
                for plugin in plugins {
                    println!(
                        "- {} ({}) source={:?} version={} manifest_valid={}",
                        plugin.id,
                        plugin.name,
                        plugin.source,
                        plugin.version.unwrap_or_else(|| "-".to_string()),
                        plugin.manifest_valid
                    );
                }
            }
        }
        PluginsCommand::Info { plugin_id } => {
            let plugin = registry.plugin_info(&plugin_id)?;
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "plugin": plugin,
                }));
            } else {
                println!("id: {}", plugin.id);
                println!("name: {}", plugin.name);
                println!("source: {:?}", plugin.source);
                println!(
                    "version: {}",
                    plugin.version.unwrap_or_else(|| "-".to_string())
                );
                println!(
                    "description: {}",
                    plugin.description.unwrap_or_else(|| "-".to_string())
                );
                println!("path: {}", plugin.path);
                println!("manifest path: {}", plugin.manifest_path);
                println!("manifest valid: {}", plugin.manifest_valid);
                if let Some(error) = plugin.manifest_error {
                    println!("manifest error: {error}");
                }
            }
        }
        PluginsCommand::Check { plugin_id } => {
            let report = registry.check_plugins(plugin_id.as_deref())?;
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "report": report,
                }));
            } else {
                println!(
                    "plugin checks: checked={} failed={} ok={}",
                    report.checked, report.failed, report.ok
                );
                for result in report.results {
                    println!(
                        "- {} source={:?} ok={}",
                        result.id, result.source, result.ok
                    );
                    for check in result.checks {
                        let status = if check.ok { "OK" } else { "WARN" };
                        println!("  [{status}] {}: {}", check.name, check.detail);
                    }
                }
            }
        }
        PluginsCommand::Install { path, force } => {
            let cwd = std::env::current_dir().map_err(|err| MosaicError::Io(err.to_string()))?;
            let source = {
                let value = PathBuf::from(path);
                if value.is_absolute() {
                    value
                } else {
                    cwd.join(value)
                }
            };
            let outcome = registry.install_plugin_from_path(&source, force)?;
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "installed": outcome,
                }));
            } else {
                println!(
                    "Installed plugin {} -> {}",
                    outcome.id, outcome.installed_path
                );
                if outcome.replaced {
                    println!("replaced existing plugin package");
                }
            }
        }
        PluginsCommand::Remove { plugin_id } => {
            let removed = registry.remove_project_plugin(&plugin_id)?;
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "removed": removed,
                    "plugin_id": plugin_id,
                }));
            } else if removed {
                println!("Removed plugin {plugin_id}");
            } else {
                println!("Plugin {plugin_id} not found in project scope.");
            }
        }
    }
    Ok(())
}

fn handle_skills(cli: &Cli, args: SkillsArgs) -> Result<()> {
    let paths = resolve_state_paths(cli.project_state)?;
    paths.ensure_dirs()?;
    let registry = ExtensionRegistry::new(RegistryRoots::from_state_root(paths.root_dir.clone()));

    match args.command {
        SkillsCommand::List => {
            let skills = registry.list_skills()?;
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "count": skills.len(),
                    "skills": skills,
                }));
            } else if skills.is_empty() {
                println!("No skills found.");
            } else {
                println!("skills: {}", skills.len());
                for skill in skills {
                    println!("- {} ({}) source={:?}", skill.id, skill.title, skill.source);
                }
            }
        }
        SkillsCommand::Info { skill_id } => {
            let skill = registry.skill_info(&skill_id)?;
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "skill": skill,
                }));
            } else {
                println!("id: {}", skill.id);
                println!("title: {}", skill.title);
                println!(
                    "description: {}",
                    skill.description.unwrap_or_else(|| "-".to_string())
                );
                println!("source: {:?}", skill.source);
                println!("path: {}", skill.path);
                println!("skill file: {}", skill.skill_file);
            }
        }
        SkillsCommand::Check { skill_id } => {
            let report = registry.check_skills(skill_id.as_deref())?;
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "report": report,
                }));
            } else {
                println!(
                    "skill checks: checked={} failed={} ok={}",
                    report.checked, report.failed, report.ok
                );
                for result in report.results {
                    println!(
                        "- {} source={:?} ok={}",
                        result.id, result.source, result.ok
                    );
                    for check in result.checks {
                        let status = if check.ok { "OK" } else { "WARN" };
                        println!("  [{status}] {}: {}", check.name, check.detail);
                    }
                }
            }
        }
        SkillsCommand::Install { path, force } => {
            let cwd = std::env::current_dir().map_err(|err| MosaicError::Io(err.to_string()))?;
            let source = {
                let value = PathBuf::from(path);
                if value.is_absolute() {
                    value
                } else {
                    cwd.join(value)
                }
            };
            let outcome = registry.install_skill_from_path(&source, force)?;
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "installed": outcome,
                }));
            } else {
                println!(
                    "Installed skill {} -> {}",
                    outcome.id, outcome.installed_path
                );
                if outcome.replaced {
                    println!("replaced existing skill package");
                }
            }
        }
        SkillsCommand::Remove { skill_id } => {
            let removed = registry.remove_project_skill(&skill_id)?;
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "removed": removed,
                    "skill_id": skill_id,
                }));
            } else if removed {
                println!("Removed skill {skill_id}");
            } else {
                println!("Skill {skill_id} not found in project scope.");
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
    let agent_store = AgentStore::new(
        agents_file_path(&paths.data_dir),
        agent_routes_path(&paths.data_dir),
    );
    let latest_session = store.latest_session_id()?;
    let (agents_count, default_agent_id) = match (agent_store.list(), agent_store.load_routes()) {
        (Ok(agents), Ok(routes)) => (agents.len(), routes.default_agent_id),
        _ => (0, None),
    };
    if !manager.exists() {
        if cli.json {
            print_json(&json!({
                "ok": true,
                "configured": false,
                "state_mode": paths.mode,
                "config_path": manager.path().display().to_string(),
                "latest_session": latest_session,
                "agents_count": agents_count,
                "default_agent_id": default_agent_id,
            }));
        } else {
            println!("configured: no");
            println!("config path: {}", manager.path().display());
            println!("state mode: {:?}", paths.mode);
            println!("agents: {}", agents_count);
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
            "agents_count": agents_count,
            "default_agent_id": default_agent_id,
        }));
    } else {
        println!("configured: yes");
        println!("profile: {}", resolved.profile_name);
        println!("provider: {:?}", resolved.profile.provider.kind);
        println!("base url: {}", resolved.profile.provider.base_url);
        println!("model: {}", resolved.profile.provider.model);
        println!("state mode: {:?}", paths.mode);
        println!("agents: {}", agents_count);
        if let Some(default_agent_id) = default_agent_id {
            println!("default agent: {default_agent_id}");
        }
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

    let memory_store = MemoryStore::new(
        memory_index_path(&paths.data_dir),
        memory_status_path(&paths.data_dir),
    );
    match memory_store.status() {
        Ok(status) => {
            checks.push(run_check(
                "memory_index",
                true,
                format!(
                    "indexed_documents={} index_path={}",
                    status.indexed_documents, status.index_path
                ),
            ));
        }
        Err(err) => {
            checks.push(run_check(
                "memory_index",
                false,
                format!("failed to load memory status: {err}"),
            ));
        }
    }

    let agent_store = AgentStore::new(
        agents_file_path(&paths.data_dir),
        agent_routes_path(&paths.data_dir),
    );
    match agent_store.check_integrity() {
        Ok(report) => {
            checks.push(run_check(
                "agents_integrity",
                report.ok,
                format!(
                    "agents={} routes={} default={}",
                    report.agents_count,
                    report.routes_count,
                    report.default_agent_id.unwrap_or_else(|| "-".to_string())
                ),
            ));
        }
        Err(err) => {
            checks.push(run_check(
                "agents_integrity",
                false,
                format!("failed to inspect agents: {err}"),
            ));
        }
    }

    let extension_registry =
        ExtensionRegistry::new(RegistryRoots::from_state_root(paths.root_dir.clone()));
    match extension_registry.check_plugins(None) {
        Ok(report) => {
            checks.push(run_check(
                "plugins_check",
                report.ok,
                format!("checked={} failed={}", report.checked, report.failed),
            ));
        }
        Err(err) => {
            checks.push(run_check(
                "plugins_check",
                false,
                format!("failed to run plugin checks: {err}"),
            ));
        }
    }
    match extension_registry.check_skills(None) {
        Ok(report) => {
            checks.push(run_check(
                "skills_check",
                report.ok,
                format!("checked={} failed={}", report.checked, report.failed),
            ));
        }
        Err(err) => {
            checks.push(run_check(
                "skills_check",
                false,
                format!("failed to run skill checks: {err}"),
            ));
        }
    }

    let security_root = std::env::current_dir().map_err(|err| MosaicError::Io(err.to_string()))?;
    let baseline_path = paths.root_dir.join("security").join("baseline.toml");
    match SecurityBaselineConfig::load_optional(&baseline_path) {
        Ok(Some(baseline)) => {
            checks.push(run_check(
                "security_baseline",
                true,
                format!(
                    "path={} fingerprints={} categories={} paths={}",
                    baseline_path.display(),
                    baseline.ignored_fingerprints.len(),
                    baseline.ignored_categories.len(),
                    baseline.ignored_paths.len(),
                ),
            ));
        }
        Ok(None) => {
            checks.push(run_check(
                "security_baseline",
                true,
                format!("path={} (not configured)", baseline_path.display()),
            ));
        }
        Err(err) => {
            checks.push(run_check(
                "security_baseline",
                false,
                format!("failed to load security baseline: {err}"),
            ));
        }
    }
    let security_report = SecurityAuditor::new().audit(SecurityAuditOptions {
        root: security_root,
        deep: false,
        max_files: 200,
        max_file_size: 131_072,
    });
    match security_report {
        Ok(report) => {
            checks.push(run_check(
                "security_audit",
                report.summary.high == 0,
                format!(
                    "findings={} high={} medium={} low={} scanned={}",
                    report.summary.findings,
                    report.summary.high,
                    report.summary.medium,
                    report.summary.low,
                    report.summary.scanned_files
                ),
            ));
        }
        Err(err) => {
            checks.push(run_check(
                "security_audit",
                false,
                format!("failed to run security audit: {err}"),
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

fn resolve_baseline_path(
    paths: &StatePaths,
    cwd: &std::path::Path,
    raw: Option<String>,
) -> PathBuf {
    raw.map_or_else(
        || paths.root_dir.join("security").join("baseline.toml"),
        |value| {
            let path = PathBuf::from(value);
            if path.is_absolute() {
                path
            } else {
                cwd.join(path)
            }
        },
    )
}

fn resolve_output_path(cwd: &std::path::Path, raw: &str) -> PathBuf {
    let path = PathBuf::from(raw);
    if path.is_absolute() {
        path
    } else {
        cwd.join(path)
    }
}

fn normalize_non_empty_list(values: Vec<String>, field_name: &str) -> Result<Vec<String>> {
    let mut normalized = values
        .into_iter()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();
    if normalized.len()
        != normalized
            .iter()
            .collect::<std::collections::HashSet<_>>()
            .len()
    {
        normalized.sort();
        normalized.dedup();
    }
    for value in &normalized {
        if value.trim().is_empty() {
            return Err(MosaicError::Validation(format!(
                "{field_name} entry cannot be empty"
            )));
        }
    }
    Ok(normalized)
}

fn remove_matching(target: &mut Vec<String>, values: &[String]) -> usize {
    let before = target.len();
    target.retain(|item| !values.contains(item));
    before.saturating_sub(target.len())
}

fn build_runtime(
    cli: &Cli,
    requested_agent_id: Option<&str>,
    route_hint: Option<&str>,
) -> Result<RuntimeContext> {
    let state_paths = resolve_state_paths(cli.project_state)?;
    state_paths.ensure_dirs()?;
    let manager = ConfigManager::new(state_paths.config_path.clone());
    let config = manager.load()?;
    let agent_store = AgentStore::new(
        agents_file_path(&state_paths.data_dir),
        agent_routes_path(&state_paths.data_dir),
    );
    let resolved = agent_store.resolve_effective_profile(
        &config,
        &cli.profile,
        requested_agent_id,
        route_hint,
    )?;
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
    Ok(RuntimeContext {
        provider,
        agent,
        active_agent_id: resolved.agent_id,
        active_profile_name: resolved.profile_name,
    })
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
