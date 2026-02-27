use std::fs;
use std::io::{self, Read, Write};

use serde_json::json;

use mosaic_agent::AgentRunOptions;
use mosaic_core::config::{ConfigManager, ProfileConfig, StateConfig};
use mosaic_core::error::{MosaicError, Result};
use mosaic_core::models::ModelRoutingStore;
use mosaic_core::session::SessionStore;

use super::{
    ChatArgs, Cli, ConfigureArgs, ModelAliasesCommand, ModelFallbacksCommand, ModelsArgs,
    ModelsCommand, PROJECT_STATE_DIR, SessionArgs, SessionCommand, SetupArgs, build_runtime,
    print_json, resolve_effective_model, resolve_state_paths,
};

pub(super) fn handle_setup(cli: &Cli, args: SetupArgs) -> Result<()> {
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

pub(super) fn handle_configure(cli: &Cli, args: ConfigureArgs) -> Result<()> {
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

pub(super) async fn handle_models(cli: &Cli, args: ModelsArgs) -> Result<()> {
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
        ModelsCommand::Status => {
            let paths = resolve_state_paths(cli.project_state)?;
            paths.ensure_dirs()?;
            let manager = ConfigManager::new(paths.config_path.clone());
            let config = manager.load()?;
            let resolved = config.resolve_profile(Some(&cli.profile))?;
            let model_store = ModelRoutingStore::new(paths.models_path.clone());
            let profile_models = model_store.profile(&resolved.profile_name)?;
            let current_model = resolved.profile.provider.model.clone();
            let (effective_model, used_alias) =
                resolve_effective_model(&profile_models, &current_model);

            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "profile": resolved.profile_name,
                    "base_url": resolved.profile.provider.base_url,
                    "api_key_env": resolved.profile.provider.api_key_env,
                    "current_model": current_model,
                    "effective_model": effective_model,
                    "used_alias": used_alias,
                    "aliases": profile_models.aliases,
                    "fallbacks": profile_models.fallbacks,
                    "models_path": model_store.path().display().to_string(),
                }));
            } else {
                println!("profile: {}", resolved.profile_name);
                println!("base url: {}", resolved.profile.provider.base_url);
                println!("api key env: {}", resolved.profile.provider.api_key_env);
                println!("current model: {}", current_model);
                if let Some(alias) = used_alias {
                    println!("effective model: {} (alias: {alias})", effective_model);
                } else {
                    println!("effective model: {}", effective_model);
                }
                if profile_models.aliases.is_empty() {
                    println!("aliases: <empty>");
                } else {
                    println!("aliases:");
                    for (alias, target) in profile_models.aliases {
                        println!("- {alias} => {target}");
                    }
                }
                if profile_models.fallbacks.is_empty() {
                    println!("fallbacks: <empty>");
                } else {
                    println!("fallbacks:");
                    for fallback in profile_models.fallbacks {
                        println!("- {fallback}");
                    }
                }
                println!("models path: {}", model_store.path().display());
            }
        }
        ModelsCommand::Resolve { model } => {
            let paths = resolve_state_paths(cli.project_state)?;
            paths.ensure_dirs()?;
            let manager = ConfigManager::new(paths.config_path.clone());
            let config = manager.load()?;
            let resolved = config.resolve_profile(Some(&cli.profile))?;
            let model_store = ModelRoutingStore::new(paths.models_path.clone());
            let profile_models = model_store.profile(&resolved.profile_name)?;
            let requested_model = match model {
                Some(model) => {
                    let model = model.trim();
                    if model.is_empty() {
                        return Err(MosaicError::Validation("model cannot be empty".to_string()));
                    }
                    model.to_string()
                }
                None => resolved.profile.provider.model.clone(),
            };
            if requested_model.trim().is_empty() {
                return Err(MosaicError::Validation("model cannot be empty".to_string()));
            }
            let (effective_model, used_alias) =
                resolve_effective_model(&profile_models, &requested_model);
            let mut fallback_chain = Vec::new();
            for fallback in &profile_models.fallbacks {
                let fallback = fallback.trim();
                if fallback.is_empty() {
                    continue;
                }
                let (effective_fallback, _) = resolve_effective_model(&profile_models, fallback);
                if effective_fallback == effective_model
                    || fallback_chain.contains(&effective_fallback)
                {
                    continue;
                }
                fallback_chain.push(effective_fallback);
            }

            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "profile": resolved.profile_name,
                    "current_model": resolved.profile.provider.model,
                    "requested_model": requested_model,
                    "effective_model": effective_model,
                    "used_alias": used_alias,
                    "fallback_chain": fallback_chain,
                    "aliases": profile_models.aliases,
                    "models_path": model_store.path().display().to_string(),
                }));
            } else {
                println!("profile: {}", resolved.profile_name);
                println!("current model: {}", resolved.profile.provider.model);
                println!("requested model: {requested_model}");
                if let Some(alias) = used_alias {
                    println!("effective model: {} (alias: {alias})", effective_model);
                } else {
                    println!("effective model: {effective_model}");
                }
                if fallback_chain.is_empty() {
                    println!("fallback chain: <empty>");
                } else {
                    println!("fallback chain:");
                    for fallback in fallback_chain {
                        println!("- {fallback}");
                    }
                }
                println!("models path: {}", model_store.path().display());
            }
        }
        ModelsCommand::Set { model } => {
            let requested_model = model.trim();
            if requested_model.is_empty() {
                return Err(MosaicError::Validation("model cannot be empty".to_string()));
            }
            let paths = resolve_state_paths(cli.project_state)?;
            paths.ensure_dirs()?;
            let manager = ConfigManager::new(paths.config_path.clone());
            let mut config = manager.load()?;
            let model_store = ModelRoutingStore::new(paths.models_path.clone());
            let profile_models = model_store.profile(&cli.profile)?;
            let (effective_model, used_alias) =
                resolve_effective_model(&profile_models, requested_model);

            let profile = config.profiles.get_mut(&cli.profile).ok_or_else(|| {
                MosaicError::Config(format!("profile '{}' not found", cli.profile))
            })?;
            let previous_model = profile.provider.model.clone();
            profile.provider.model = effective_model.clone();
            config.active_profile = cli.profile.clone();
            manager.save(&config)?;

            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "profile": cli.profile,
                    "requested_model": requested_model,
                    "effective_model": effective_model,
                    "used_alias": used_alias,
                    "previous_model": previous_model,
                }));
            } else if let Some(alias) = used_alias {
                println!(
                    "updated profile '{}' model: {} -> {} (from alias '{}')",
                    cli.profile, previous_model, effective_model, alias
                );
            } else {
                println!(
                    "updated profile '{}' model: {} -> {}",
                    cli.profile, previous_model, effective_model
                );
            }
        }
        ModelsCommand::Aliases { command } => {
            let paths = resolve_state_paths(cli.project_state)?;
            paths.ensure_dirs()?;
            let manager = ConfigManager::new(paths.config_path.clone());
            let config = manager.load()?;
            let _ = config.resolve_profile(Some(&cli.profile))?;
            let model_store = ModelRoutingStore::new(paths.models_path.clone());
            let profile_models = match command {
                ModelAliasesCommand::List => model_store.profile(&cli.profile)?,
                ModelAliasesCommand::Set { alias, model } => {
                    model_store.set_alias(&cli.profile, &alias, &model)?
                }
                ModelAliasesCommand::Remove { alias } => {
                    model_store.remove_alias(&cli.profile, &alias)?
                }
                ModelAliasesCommand::Clear => model_store.clear_aliases(&cli.profile)?,
            };
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "profile": cli.profile,
                    "aliases": profile_models.aliases,
                    "models_path": model_store.path().display().to_string(),
                }));
            } else if profile_models.aliases.is_empty() {
                println!("aliases: <empty>");
                println!("models path: {}", model_store.path().display());
            } else {
                println!("aliases:");
                for (alias, target) in profile_models.aliases {
                    println!("- {alias} => {target}");
                }
                println!("models path: {}", model_store.path().display());
            }
        }
        ModelsCommand::Fallbacks { command } => {
            let paths = resolve_state_paths(cli.project_state)?;
            paths.ensure_dirs()?;
            let manager = ConfigManager::new(paths.config_path.clone());
            let config = manager.load()?;
            let _ = config.resolve_profile(Some(&cli.profile))?;
            let model_store = ModelRoutingStore::new(paths.models_path.clone());
            let profile_models = match command {
                ModelFallbacksCommand::List => model_store.profile(&cli.profile)?,
                ModelFallbacksCommand::Add { model } => {
                    model_store.add_fallback(&cli.profile, &model)?
                }
                ModelFallbacksCommand::Remove { model } => {
                    model_store.remove_fallback(&cli.profile, &model)?
                }
                ModelFallbacksCommand::Clear => model_store.clear_fallbacks(&cli.profile)?,
            };
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "profile": cli.profile,
                    "fallbacks": profile_models.fallbacks,
                    "models_path": model_store.path().display().to_string(),
                }));
            } else if profile_models.fallbacks.is_empty() {
                println!("fallbacks: <empty>");
                println!("models path: {}", model_store.path().display());
            } else {
                println!("fallbacks:");
                for fallback in profile_models.fallbacks {
                    println!("- {fallback}");
                }
                println!("models path: {}", model_store.path().display());
            }
        }
    }
    Ok(())
}

pub(super) async fn handle_ask(cli: &Cli, args: super::AskArgs) -> Result<()> {
    let runtime = build_runtime(cli, args.agent.as_deref(), Some("ask"))?;
    let mut session_id = args.session;

    if let Some(script_path) = args.script {
        let prompts = resolve_script_prompts(script_path)?;
        let mut run_results = Vec::with_capacity(prompts.len());
        let mut total_turns = 0u32;
        for (index, prompt) in prompts.into_iter().enumerate() {
            let result = runtime
                .agent
                .ask(
                    &prompt,
                    AgentRunOptions {
                        session_id: session_id.clone(),
                        cwd: std::env::current_dir()
                            .map_err(|err| MosaicError::Io(err.to_string()))?,
                        yes: cli.yes,
                        interactive: false,
                    },
                )
                .await?;
            session_id = Some(result.session_id.clone());
            total_turns = total_turns.saturating_add(result.turns);
            run_results.push(json!({
                "index": index + 1,
                "prompt": prompt,
                "response": result.response,
                "turns": result.turns,
                "session_id": result.session_id,
            }));
        }

        if cli.json {
            print_json(&json!({
                "ok": true,
                "mode": "script",
                "session_id": session_id,
                "runs": run_results,
                "run_count": run_results.len(),
                "total_turns": total_turns,
                "agent_id": runtime.active_agent_id,
                "profile": runtime.active_profile_name,
            }));
        } else {
            for run in run_results {
                println!("you> {}", run["prompt"].as_str().unwrap_or_default());
                println!(
                    "assistant> {}",
                    run["response"].as_str().unwrap_or_default().trim()
                );
                println!(
                    "session: {}",
                    run["session_id"].as_str().unwrap_or_default()
                );
            }
            if let Some(agent_id) = &runtime.active_agent_id {
                println!("agent: {agent_id}");
            }
        }
        return Ok(());
    }

    let prompt = resolve_prompt_source(args.prompt, args.prompt_file)?;
    let result = runtime
        .agent
        .ask(
            &prompt,
            AgentRunOptions {
                session_id,
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

pub(super) async fn handle_chat(cli: &Cli, args: ChatArgs) -> Result<()> {
    let runtime = build_runtime(cli, args.agent.as_deref(), Some("chat"))?;
    let mut session_id = args.session;
    let initial_prompt = resolve_prompt_source_optional(args.prompt, args.prompt_file)?;

    if let Some(script_path) = args.script {
        let prompts = resolve_script_prompts(script_path)?;
        let mut run_results = Vec::with_capacity(prompts.len());
        let mut total_turns = 0u32;
        for (index, prompt) in prompts.into_iter().enumerate() {
            let result = runtime
                .agent
                .ask(
                    &prompt,
                    AgentRunOptions {
                        session_id: session_id.clone(),
                        cwd: std::env::current_dir()
                            .map_err(|err| MosaicError::Io(err.to_string()))?,
                        yes: cli.yes,
                        interactive: true,
                    },
                )
                .await?;
            session_id = Some(result.session_id.clone());
            total_turns = total_turns.saturating_add(result.turns);
            run_results.push(json!({
                "index": index + 1,
                "prompt": prompt,
                "response": result.response,
                "turns": result.turns,
                "session_id": result.session_id,
            }));
        }

        if cli.json {
            print_json(&json!({
                "ok": true,
                "mode": "script",
                "session_id": session_id,
                "runs": run_results,
                "run_count": run_results.len(),
                "total_turns": total_turns,
                "agent_id": runtime.active_agent_id,
                "profile": runtime.active_profile_name,
            }));
            return Ok(());
        }

        for run in run_results {
            println!("you> {}", run["prompt"].as_str().unwrap_or_default());
            println!(
                "assistant> {}",
                run["response"].as_str().unwrap_or_default().trim()
            );
            println!(
                "session: {}",
                run["session_id"].as_str().unwrap_or_default()
            );
        }
        if let Some(agent_id) = &runtime.active_agent_id {
            println!("agent: {agent_id}");
        }
        return Ok(());
    }

    if let Some(prompt) = initial_prompt {
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
            "chat in --json mode requires one of --prompt, --prompt-file, or --script".to_string(),
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
        match parse_chat_repl_command(prompt) {
            ChatReplCommand::Exit => {
                println!("Bye.");
                break;
            }
            ChatReplCommand::Help => {
                println!("/help     Show help");
                println!("/status   Show profile/agent/session");
                println!("/agent    Show active agent");
                println!("/session  Show current session id");
                println!("/new      Start a new chat session");
                println!("/exit     Exit chat");
                continue;
            }
            ChatReplCommand::Session => {
                println!("session: {}", format_chat_session(session_id.as_deref()));
                continue;
            }
            ChatReplCommand::New => {
                session_id = None;
                println!(
                    "session reset: {}",
                    format_chat_session(session_id.as_deref())
                );
                continue;
            }
            ChatReplCommand::Status => {
                println!("profile: {}", runtime.active_profile_name);
                println!(
                    "agent: {}",
                    format_chat_agent(runtime.active_agent_id.as_deref())
                );
                println!("session: {}", format_chat_session(session_id.as_deref()));
                continue;
            }
            ChatReplCommand::Agent => {
                println!(
                    "agent: {}",
                    format_chat_agent(runtime.active_agent_id.as_deref())
                );
                continue;
            }
            ChatReplCommand::Prompt(prompt) => {
                let result = runtime
                    .agent
                    .ask(
                        prompt,
                        AgentRunOptions {
                            session_id: session_id.clone(),
                            cwd: std::env::current_dir()
                                .map_err(|err| MosaicError::Io(err.to_string()))?,
                            yes: cli.yes,
                            interactive: true,
                        },
                    )
                    .await?;
                session_id = Some(result.session_id.clone());
                println!("assistant> {}", result.response.trim());
            }
        }
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ChatReplCommand<'a> {
    Exit,
    Help,
    Session,
    New,
    Status,
    Agent,
    Prompt(&'a str),
}

fn parse_chat_repl_command(prompt: &str) -> ChatReplCommand<'_> {
    match prompt {
        "/exit" | "exit" | "quit" => ChatReplCommand::Exit,
        "/help" => ChatReplCommand::Help,
        "/session" => ChatReplCommand::Session,
        "/new" => ChatReplCommand::New,
        "/status" => ChatReplCommand::Status,
        "/agent" => ChatReplCommand::Agent,
        _ => ChatReplCommand::Prompt(prompt),
    }
}

fn format_chat_session(session_id: Option<&str>) -> String {
    session_id
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| "<new session>".to_string())
}

fn format_chat_agent(agent_id: Option<&str>) -> String {
    agent_id
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| "<none>".to_string())
}

fn resolve_prompt_source(prompt: Option<String>, prompt_file: Option<String>) -> Result<String> {
    match (prompt, prompt_file) {
        (Some(prompt), None) => resolve_prompt_input(prompt),
        (None, Some(path)) => resolve_prompt_file(path),
        (Some(_), Some(_)) => Err(MosaicError::Validation(
            "provide either prompt text or --prompt-file, not both".to_string(),
        )),
        (None, None) => Err(MosaicError::Validation("prompt is required".to_string())),
    }
}

fn resolve_prompt_source_optional(
    prompt: Option<String>,
    prompt_file: Option<String>,
) -> Result<Option<String>> {
    match (prompt, prompt_file) {
        (None, None) => Ok(None),
        (Some(prompt), None) => Ok(Some(resolve_prompt_input(prompt)?)),
        (None, Some(path)) => Ok(Some(resolve_prompt_file(path)?)),
        (Some(_), Some(_)) => Err(MosaicError::Validation(
            "provide either prompt text or --prompt-file, not both".to_string(),
        )),
    }
}

fn resolve_prompt_file(path: String) -> Result<String> {
    let source = if path == "-" {
        let mut stdin_prompt = String::new();
        io::stdin()
            .read_to_string(&mut stdin_prompt)
            .map_err(|err| MosaicError::Io(err.to_string()))?;
        stdin_prompt
    } else {
        fs::read_to_string(&path)
            .map_err(|err| MosaicError::Io(format!("failed to read prompt file {}: {err}", path)))?
    };
    let trimmed = source.trim();
    if trimmed.is_empty() {
        return Err(MosaicError::Validation("prompt file is empty".to_string()));
    }
    Ok(trimmed.to_string())
}

fn resolve_prompt_input(prompt: String) -> Result<String> {
    if prompt != "-" {
        let trimmed = prompt.trim();
        if trimmed.is_empty() {
            return Err(MosaicError::Validation(
                "prompt cannot be empty".to_string(),
            ));
        }
        return Ok(prompt);
    }

    let mut stdin_prompt = String::new();
    io::stdin()
        .read_to_string(&mut stdin_prompt)
        .map_err(|err| MosaicError::Io(err.to_string()))?;
    let trimmed = stdin_prompt.trim();
    if trimmed.is_empty() {
        return Err(MosaicError::Validation(
            "stdin prompt is empty; provide content via pipe".to_string(),
        ));
    }
    Ok(trimmed.to_string())
}

fn resolve_script_prompts(path: String) -> Result<Vec<String>> {
    let source = if path == "-" {
        let mut stdin_source = String::new();
        io::stdin()
            .read_to_string(&mut stdin_source)
            .map_err(|err| MosaicError::Io(err.to_string()))?;
        stdin_source
    } else {
        fs::read_to_string(&path)
            .map_err(|err| MosaicError::Io(format!("failed to read script file {}: {err}", path)))?
    };
    let prompts = source
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();
    if prompts.is_empty() {
        return Err(MosaicError::Validation(
            "script is empty; provide at least one non-empty line".to_string(),
        ));
    }
    Ok(prompts)
}

#[cfg(test)]
mod repl_tests {
    use super::*;

    #[test]
    fn parse_chat_repl_commands() {
        assert!(matches!(
            parse_chat_repl_command("/exit"),
            ChatReplCommand::Exit
        ));
        assert!(matches!(
            parse_chat_repl_command("/help"),
            ChatReplCommand::Help
        ));
        assert!(matches!(
            parse_chat_repl_command("/session"),
            ChatReplCommand::Session
        ));
        assert!(matches!(
            parse_chat_repl_command("/new"),
            ChatReplCommand::New
        ));
        assert!(matches!(
            parse_chat_repl_command("/status"),
            ChatReplCommand::Status
        ));
        assert!(matches!(
            parse_chat_repl_command("/agent"),
            ChatReplCommand::Agent
        ));
        assert!(matches!(
            parse_chat_repl_command("hello"),
            ChatReplCommand::Prompt("hello")
        ));
    }

    #[test]
    fn chat_display_helpers_return_fallback_labels() {
        assert_eq!(format_chat_session(None), "<new session>");
        assert_eq!(format_chat_agent(None), "<none>");
    }

    #[test]
    fn resolve_prompt_input_validates_non_empty_prompt() {
        let err = resolve_prompt_input("   ".to_string()).expect_err("expected validation");
        assert!(matches!(err, MosaicError::Validation(_)));
    }
}

pub(super) async fn handle_session(cli: &Cli, args: SessionArgs) -> Result<()> {
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
                        "{} {} {:?} {}",
                        event.ts.to_rfc3339(),
                        event.id,
                        event.kind,
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
                    prompt_file: None,
                    script: None,
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
