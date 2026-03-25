use std::{env, fs, path::{Path, PathBuf}, sync::Arc};

use anyhow::{Context, Result, anyhow, bail};
use clap::{Parser, Subcommand};
use mosaic_config::{
    ACTIVE_PROFILE_ENV, ConfigSourceKind, DoctorStatus, LoadConfigOptions, LoadedMosaicConfig,
    ValidationLevel, doctor_mosaic_config, init_workspace_config, load_from_file,
    load_mosaic_config, redact_mosaic_config, save_mosaic_config, validate_mosaic_config,
};
use mosaic_inspect::RunTrace;
use mosaic_provider::ProviderProfileRegistry;
use mosaic_runtime::{AgentRuntime, RunError, RunRequest, RunResult};
use mosaic_session_core::{FileSessionStore, SessionStore};

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
        #[arg(long)]
        skill: Option<String>,
        #[arg(long)]
        session: Option<String>,
        #[arg(long)]
        profile: Option<String>,
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
    },
    Setup {
        #[command(subcommand)]
        command: SetupCommand,
    },
    Session {
        #[command(subcommand)]
        command: SessionCommand,
    },
    Model {
        #[command(subcommand)]
        command: ModelCommand,
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
    Show {
        id: String,
    },
}

#[derive(Debug, Subcommand, PartialEq, Eq)]
enum ModelCommand {
    List,
    Use {
        profile: String,
    },
}

#[derive(Debug, PartialEq, Eq)]
enum DispatchCommand {
    Tui {
        resume: bool,
        session: Option<String>,
        profile: Option<String>,
    },
    Run {
        file: PathBuf,
        skill: Option<String>,
        session: Option<String>,
        profile: Option<String>,
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
        command: SessionCommand,
    },
    Model {
        command: ModelCommand,
    },
}

impl Cli {
    fn dispatch(self) -> DispatchCommand {
        match self.command {
            None => DispatchCommand::Tui {
                resume: self.resume,
                session: None,
                profile: None,
            },
            Some(Commands::Tui { session, profile }) => DispatchCommand::Tui {
                resume: self.resume,
                session,
                profile,
            },
            Some(Commands::Run {
                file,
                skill,
                session,
                profile,
                tui,
            }) => DispatchCommand::Run {
                file,
                skill,
                session,
                profile,
                tui,
                resume: self.resume,
            },
            Some(Commands::Inspect { file }) => DispatchCommand::Inspect { file },
            Some(Commands::Setup { command }) => DispatchCommand::Setup { command },
            Some(Commands::Session { command }) => DispatchCommand::Session { command },
            Some(Commands::Model { command }) => DispatchCommand::Model { command },
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
        } => tui_cmd(resume, session, profile).await,
        DispatchCommand::Run {
            file,
            skill,
            session,
            profile,
            tui,
            resume,
        } => run_cmd(file, skill, session, profile, tui, resume).await,
        DispatchCommand::Inspect { file } => inspect_cmd(file),
        DispatchCommand::Setup { command } => setup_cmd(command),
        DispatchCommand::Session { command } => session_cmd(command),
        DispatchCommand::Model { command } => model_cmd(command),
    }
}

async fn tui_cmd(
    resume: bool,
    session: Option<String>,
    profile: Option<String>,
) -> Result<()> {
    let loaded = ensure_loaded_config(profile.clone())?;
    let session_store_root = resolve_workspace_path(&loaded.config.session_store.root_dir)?;
    let runs_dir = resolve_workspace_path(&loaded.config.inspect.runs_dir)?;
    let session_store = Arc::new(FileSessionStore::new(session_store_root));
    let session_id = resolve_tui_session_id(&session_store, session.as_deref())?;
    let active_profile = profile.unwrap_or_else(|| loaded.config.active_profile.clone());
    let active_profile_config = loaded
        .config
        .profiles
        .get(&active_profile)
        .ok_or_else(|| anyhow!("unknown provider profile: {}", active_profile))?;
    let event_buffer = mosaic_tui::build_tui_event_buffer();
    let event_sink = mosaic_tui::build_tui_event_sink(event_buffer.clone());
    let runtime = Arc::new(AgentRuntime::new(bootstrap::build_runtime_context(
        &loaded.config,
        None,
        event_sink,
    )?));
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
    let context = mosaic_tui::InteractiveSessionContext {
        runtime,
        runtime_handle: tokio::runtime::Handle::current(),
        event_buffer,
        session_store: session_store as Arc<dyn SessionStore>,
        session_id,
        system: None,
        runs_dir,
        active_profile,
        active_model: active_profile_config.model.clone(),
        available_profiles,
    };

    tokio::task::spawn_blocking(move || mosaic_tui::run_interactive_session(resume, context))
        .await??;

    Ok(())
}

async fn run_cmd(
    file: PathBuf,
    skill: Option<String>,
    session: Option<String>,
    profile: Option<String>,
    tui: bool,
    resume: bool,
) -> Result<()> {
    let app_cfg = load_from_file(&file)?;
    let loaded = ensure_loaded_config(profile.clone())?;
    let runs_dir = resolve_workspace_path(&loaded.config.inspect.runs_dir)?;

    if tui {
        return run_cmd_with_tui(loaded, app_cfg, skill, session, profile, resume).await;
    }

    let sinks = bootstrap::build_cli_only_sinks();
    let runtime = AgentRuntime::new(bootstrap::build_runtime_context(
        &loaded.config,
        Some(&app_cfg),
        sinks.event_sink,
    )?);
    let outcome = runtime
        .run(RunRequest {
            system: app_cfg.agent.system,
            input: app_cfg.task.input,
            skill,
            session_id: session,
            profile,
        })
        .await;

    finish_run_outcome(outcome, &runs_dir)
}

async fn run_cmd_with_tui(
    loaded: LoadedMosaicConfig,
    app_cfg: mosaic_config::AppConfig,
    skill: Option<String>,
    session: Option<String>,
    profile: Option<String>,
    resume: bool,
) -> Result<()> {
    let sinks = bootstrap::build_cli_and_tui_sinks();
    let event_buffer = sinks
        .tui_buffer
        .expect("cli+tui sink builder should provide a TUI buffer");
    let runtime = AgentRuntime::new(bootstrap::build_runtime_context(
        &loaded.config,
        Some(&app_cfg),
        sinks.event_sink,
    )?);

    let request = RunRequest {
        system: app_cfg.agent.system,
        input: app_cfg.task.input,
        skill,
        session_id: session,
        profile,
    };

    let runs_dir = resolve_workspace_path(&loaded.config.inspect.runs_dir)?;
    let runtime_handle = tokio::spawn(async move { runtime.run(request).await });
    let tui_handle = tokio::task::spawn_blocking(move || {
        mosaic_tui::run_until_complete_with_event_buffer(resume, event_buffer)
    });

    let runtime_outcome = runtime_handle.await?;
    let tui_join = tui_handle.await;

    let run_result = finish_run_outcome(runtime_outcome, &runs_dir);

    match (run_result, tui_join) {
        (Err(err), _) => Err(err),
        (Ok(()), Err(err)) => Err(err.into()),
        (Ok(()), Ok(tui_result)) => {
            tui_result?;
            Ok(())
        }
    }
}

fn finish_run_outcome(outcome: std::result::Result<RunResult, RunError>, runs_dir: &Path) -> Result<()> {
    match outcome {
        Ok(result) => persist_successful_run(result, runs_dir),
        Err(err) => persist_failed_run(err, runs_dir),
    }
}

fn persist_successful_run(result: RunResult, runs_dir: &Path) -> Result<()> {
    let path = result.trace.save_to_dir(runs_dir)?;
    println!("{}", result.output);
    println!("saved trace: {}", path.display());
    Ok(())
}

fn persist_failed_run(err: RunError, runs_dir: &Path) -> Result<()> {
    let (source, trace) = err.into_parts();
    let path = trace.save_to_dir(runs_dir)?;

    println!("saved trace: {}", path.display());

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

fn session_cmd(command: SessionCommand) -> Result<()> {
    let loaded = ensure_loaded_config(None)?;
    let store = FileSessionStore::new(resolve_workspace_path(&loaded.config.session_store.root_dir)?);

    match command {
        SessionCommand::List => {
            let sessions = store.list()?;

            if sessions.is_empty() {
                println!("no sessions found");
                return Ok(());
            }

            for session in sessions {
                println!(
                    "{} | {} | profile={} | model={} | messages={} | updated_at={}",
                    session.id,
                    session.title,
                    session.provider_profile,
                    session.model,
                    session.message_count,
                    session.updated_at
                );
                if let Some(preview) = session.last_message_preview {
                    println!("  last: {}", preview);
                }
            }

            Ok(())
        }
        SessionCommand::Show { id } => {
            let session = store
                .load(&id)?
                .ok_or_else(|| anyhow!("session not found: {}", id))?;

            println!("id: {}", session.id);
            println!("title: {}", session.title);
            println!("created_at: {}", session.created_at);
            println!("updated_at: {}", session.updated_at);
            println!("provider_profile: {}", session.provider_profile);
            println!("provider_type: {}", session.provider_type);
            println!("model: {}", session.model);
            println!("last_run_id: {:?}", session.last_run_id);
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
    println!("session_id: {:?}", trace.session_id);
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

    println!("\nsummary:");
    println!("  tool_calls: {}", summary.tool_calls);
    println!("  skill_calls: {}", summary.skill_calls);

    if let Ok(loaded) = load_config() {
        let redacted = redact_mosaic_config(&loaded.config);
        println!("\ncurrent_workspace_config:");
        println!("  active_profile: {}", redacted.active_profile);
        println!("  session_store_root_dir: {}", redacted.session_store_root_dir);
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

    Ok(())
}

fn load_config() -> Result<LoadedMosaicConfig> {
    load_mosaic_config(&LoadConfigOptions::default())
}

fn resolve_tui_session_id(
    store: &FileSessionStore,
    requested: Option<&str>,
) -> Result<String> {
    if let Some(session_id) = requested {
        return Ok(session_id.to_owned());
    }

    Ok(store
        .list()?
        .into_iter()
        .next()
        .map(|session| session.id)
        .unwrap_or_else(|| "default".to_owned()))
}

fn ensure_loaded_config(active_profile_override: Option<String>) -> Result<LoadedMosaicConfig> {
    let mut options = LoadConfigOptions::default();
    options.overrides.active_profile = active_profile_override;

    let loaded = load_mosaic_config(&options)?;
    let report = validate_mosaic_config(&loaded.config);
    if report.has_errors() {
        bail!("configuration is invalid:\n{}", render_validation_issues(&report));
    }

    Ok(loaded)
}

fn current_dir() -> Result<PathBuf> {
    env::current_dir().context("failed to resolve current directory")
}

fn resolve_workspace_path(path: &str) -> Result<PathBuf> {
    let path = PathBuf::from(path);
    if path.is_absolute() {
        return Ok(path);
    }

    Ok(current_dir()?.join(path))
}

fn print_config_summary(loaded: &LoadedMosaicConfig) {
    println!("config sources:");
    for source in &loaded.sources {
        println!("  [{}] {}", config_source_label(&source.kind), source.detail);
    }

    let redacted = redact_mosaic_config(&loaded.config);
    println!("\nconfig summary:");
    println!("  schema_version: {}", redacted.schema_version);
    println!("  active_profile: {}", redacted.active_profile);
    println!("  session_store_root_dir: {}", redacted.session_store_root_dir);
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
                session: Some("demo".to_owned()),
                profile: Some("gpt-5.4-mini".to_owned()),
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
                session: None,
                profile: None,
                tui: true,
                resume: true,
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
}
