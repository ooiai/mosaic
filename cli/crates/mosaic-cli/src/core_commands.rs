use std::collections::BTreeMap;
use std::fs;
use std::io::{self, Read, Write};

use serde_json::{Value, json};

use mosaic_agent::AgentRunOptions;
use mosaic_core::config::{ConfigFile, ConfigManager, ProfileConfig, RunGuardMode, StateConfig};
use mosaic_core::error::{MosaicError, Result};
use mosaic_core::models::ModelRoutingStore;
use mosaic_core::session::SessionStore;

use super::{
    ChatArgs, Cli, ConfigureArgs, ConfigureCommand, ConfigurePatchArgs, ConfigureTemplateArgs,
    ConfigureTemplateFormatArg, ModelAliasesCommand, ModelFallbacksCommand, ModelsArgs,
    ModelsCommand, PROJECT_STATE_DIR, SessionArgs, SessionCommand, SetupArgs, build_runtime,
    print_json, resolve_effective_model, resolve_state_paths,
};

pub(super) fn handle_setup(cli: &Cli, args: SetupArgs) -> Result<()> {
    let paths = resolve_state_paths(cli.project_state)?;
    paths.ensure_dirs()?;
    let manager = ConfigManager::new(paths.config_path.clone());
    let mut config = manager.load_or_default(paths.mode)?;
    let profile = config.profiles.entry(cli.profile.clone()).or_default();
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
    let ConfigureArgs {
        command,
        show,
        base_url,
        model,
        api_key_env,
        temperature,
        max_turns,
        tools_enabled,
        guard_mode,
    } = args;
    let paths = resolve_state_paths(cli.project_state)?;
    paths.ensure_dirs()?;
    let manager = ConfigManager::new(paths.config_path.clone());
    let mut config = manager.load()?;

    if let Some(command) = command {
        let has_legacy_flags = show
            || base_url.is_some()
            || model.is_some()
            || api_key_env.is_some()
            || temperature.is_some()
            || max_turns.is_some()
            || tools_enabled.is_some()
            || guard_mode.is_some();
        if has_legacy_flags {
            return Err(MosaicError::Validation(
                "configure subcommands cannot be combined with legacy configure flags".to_string(),
            ));
        }
        return handle_configure_subcommand(cli, &manager, &mut config, command);
    }

    let mut changed = false;
    {
        let profile = config.profiles.entry(cli.profile.clone()).or_default();
        if let Some(base_url) = base_url {
            profile.provider.base_url = base_url;
            changed = true;
        }
        if let Some(model) = model {
            profile.provider.model = model;
            changed = true;
        }
        if let Some(api_key_env) = api_key_env {
            profile.provider.api_key_env = api_key_env;
            changed = true;
        }
        if let Some(temperature) = temperature {
            profile.agent.temperature = temperature;
            changed = true;
        }
        if let Some(max_turns) = max_turns {
            profile.agent.max_turns = max_turns;
            changed = true;
        }
        if let Some(tools_enabled) = tools_enabled {
            profile.tools.enabled = tools_enabled;
            changed = true;
        }
        if let Some(guard_mode) = guard_mode {
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
    } else if show || !changed {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ConfigureKey {
    ProviderBaseUrl,
    ProviderModel,
    ProviderApiKeyEnv,
    AgentTemperature,
    AgentMaxTurns,
    ToolsEnabled,
    ToolsRunGuardMode,
}

#[derive(Debug, Clone, Copy)]
struct ConfigureKeySpec {
    key: ConfigureKey,
    name: &'static str,
    aliases: &'static [&'static str],
    value_type: &'static str,
    description: &'static str,
}

const CONFIGURE_KEY_SPECS: [ConfigureKeySpec; 7] = [
    ConfigureKeySpec {
        key: ConfigureKey::ProviderBaseUrl,
        name: "provider.base_url",
        aliases: &["base_url"],
        value_type: "string",
        description: "OpenAI-compatible provider base URL",
    },
    ConfigureKeySpec {
        key: ConfigureKey::ProviderModel,
        name: "provider.model",
        aliases: &["model"],
        value_type: "string",
        description: "default model identifier for ask/chat",
    },
    ConfigureKeySpec {
        key: ConfigureKey::ProviderApiKeyEnv,
        name: "provider.api_key_env",
        aliases: &["api_key_env"],
        value_type: "string",
        description: "environment variable name for API key",
    },
    ConfigureKeySpec {
        key: ConfigureKey::AgentTemperature,
        name: "agent.temperature",
        aliases: &["temperature"],
        value_type: "float",
        description: "sampling temperature, expected in range [0.0, 2.0]",
    },
    ConfigureKeySpec {
        key: ConfigureKey::AgentMaxTurns,
        name: "agent.max_turns",
        aliases: &["max_turns"],
        value_type: "integer",
        description: "maximum turns per run",
    },
    ConfigureKeySpec {
        key: ConfigureKey::ToolsEnabled,
        name: "tools.enabled",
        aliases: &["tools_enabled"],
        value_type: "boolean",
        description: "toggle tool usage for the agent",
    },
    ConfigureKeySpec {
        key: ConfigureKey::ToolsRunGuardMode,
        name: "tools.run.guard_mode",
        aliases: &["guard_mode"],
        value_type: "enum(confirm_dangerous|all_confirm|unrestricted)",
        description: "command execution confirmation policy",
    },
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ConfigurePatchOperation {
    Patch,
    Preview,
}

impl ConfigurePatchOperation {
    fn action_name(self) -> &'static str {
        match self {
            Self::Patch => "patch",
            Self::Preview => "preview",
        }
    }

    fn dry_run(self, requested_dry_run: bool) -> bool {
        match self {
            Self::Patch => requested_dry_run,
            Self::Preview => true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ConfigureTemplateFormat {
    Json,
    Toml,
}

impl ConfigureTemplateFormat {
    fn as_str(self) -> &'static str {
        match self {
            Self::Json => "json",
            Self::Toml => "toml",
        }
    }
}

impl From<ConfigureTemplateFormatArg> for ConfigureTemplateFormat {
    fn from(value: ConfigureTemplateFormatArg) -> Self {
        match value {
            ConfigureTemplateFormatArg::Json => Self::Json,
            ConfigureTemplateFormatArg::Toml => Self::Toml,
        }
    }
}

fn handle_configure_subcommand(
    cli: &Cli,
    manager: &ConfigManager,
    config: &mut ConfigFile,
    command: ConfigureCommand,
) -> Result<()> {
    match command {
        ConfigureCommand::Keys => {
            let profile = config.profiles.entry(cli.profile.clone()).or_default();
            let keys = CONFIGURE_KEY_SPECS
                .iter()
                .map(|spec| {
                    json!({
                        "name": spec.name,
                        "aliases": spec.aliases,
                        "type": spec.value_type,
                        "description": spec.description,
                        "value": configure_value(profile, spec.key),
                        "default": configure_default_value(spec.key),
                    })
                })
                .collect::<Vec<_>>();
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "action": "keys",
                    "profile": cli.profile,
                    "keys": keys,
                }));
            } else {
                println!("profile: {}", cli.profile);
                println!("supported keys:");
                for spec in &CONFIGURE_KEY_SPECS {
                    let aliases = if spec.aliases.is_empty() {
                        "<none>".to_string()
                    } else {
                        spec.aliases.join(", ")
                    };
                    println!(
                        "- {} ({}) aliases: {}\n  {}\n  current: {}\n  default: {}",
                        spec.name,
                        spec.value_type,
                        aliases,
                        spec.description,
                        configure_value(profile, spec.key),
                        configure_default_value(spec.key)
                    );
                }
            }
            return Ok(());
        }
        ConfigureCommand::Patch(args) => {
            return handle_configure_patch(
                cli,
                manager,
                config,
                args,
                ConfigurePatchOperation::Patch,
            );
        }
        ConfigureCommand::Preview(args) => {
            let patch_args = ConfigurePatchArgs {
                set: args.set,
                file: args.file,
                target_profile: args.target_profile,
                dry_run: true,
            };
            return handle_configure_patch(
                cli,
                manager,
                config,
                patch_args,
                ConfigurePatchOperation::Preview,
            );
        }
        ConfigureCommand::Template(args) => {
            return handle_configure_template(cli, manager, config, args);
        }
        _ => {}
    }

    let profile = config.profiles.entry(cli.profile.clone()).or_default();
    let (action, key_name, value, changed) = match command {
        ConfigureCommand::Get { key } => {
            let key = parse_configure_key(&key)?;
            (
                "get",
                configure_key_name(key).to_string(),
                configure_value(profile, key),
                false,
            )
        }
        ConfigureCommand::Set { key, value } => {
            let key = parse_configure_key(&key)?;
            let configured = set_configure_value(profile, key, &value)?;
            ("set", configure_key_name(key).to_string(), configured, true)
        }
        ConfigureCommand::Unset { key } => {
            let key = parse_configure_key(&key)?;
            let configured = unset_configure_value(profile, key);
            (
                "unset",
                configure_key_name(key).to_string(),
                configured,
                true,
            )
        }
        ConfigureCommand::Keys
        | ConfigureCommand::Patch(_)
        | ConfigureCommand::Preview(_)
        | ConfigureCommand::Template(_) => unreachable!(),
    };

    config.active_profile = cli.profile.clone();
    if changed {
        manager.save(config)?;
    }

    if cli.json {
        print_json(&json!({
            "ok": true,
            "action": action,
            "changed": changed,
            "profile": cli.profile,
            "key": key_name,
            "value": value,
            "config_path": manager.path().display().to_string(),
        }));
    } else {
        println!("profile: {}", cli.profile);
        println!("action: {action}");
        println!("key: {key_name}");
        println!("value: {}", value);
        if changed {
            println!("config path: {}", manager.path().display());
        }
    }
    Ok(())
}

fn parse_configure_key(raw: &str) -> Result<ConfigureKey> {
    let key = raw.trim();
    if key.is_empty() {
        return Err(MosaicError::Validation(
            "configure key cannot be empty".to_string(),
        ));
    }
    let normalized = normalize_configure_key(key);
    for spec in &CONFIGURE_KEY_SPECS {
        if normalize_configure_key(spec.name) == normalized {
            return Ok(spec.key);
        }
        for alias in spec.aliases {
            if normalize_configure_key(alias) == normalized {
                return Ok(spec.key);
            }
        }
    }
    Err(MosaicError::Validation(format!(
        "unsupported configure key '{key}'. run `mosaic configure keys` to list supported keys"
    )))
}

fn configure_key_name(key: ConfigureKey) -> &'static str {
    CONFIGURE_KEY_SPECS
        .iter()
        .find(|spec| spec.key == key)
        .map(|spec| spec.name)
        .expect("configure key spec exists")
}

fn configure_key_group(key: ConfigureKey) -> &'static str {
    match key {
        ConfigureKey::ProviderBaseUrl
        | ConfigureKey::ProviderModel
        | ConfigureKey::ProviderApiKeyEnv => "provider",
        ConfigureKey::AgentTemperature | ConfigureKey::AgentMaxTurns => "agent",
        ConfigureKey::ToolsEnabled | ConfigureKey::ToolsRunGuardMode => "tools",
    }
}

fn configure_value(profile: &ProfileConfig, key: ConfigureKey) -> Value {
    match key {
        ConfigureKey::ProviderBaseUrl => json!(profile.provider.base_url),
        ConfigureKey::ProviderModel => json!(profile.provider.model),
        ConfigureKey::ProviderApiKeyEnv => json!(profile.provider.api_key_env),
        ConfigureKey::AgentTemperature => json!(profile.agent.temperature),
        ConfigureKey::AgentMaxTurns => json!(profile.agent.max_turns),
        ConfigureKey::ToolsEnabled => json!(profile.tools.enabled),
        ConfigureKey::ToolsRunGuardMode => json!(guard_mode_name(&profile.tools.run.guard_mode)),
    }
}

fn configure_default_value(key: ConfigureKey) -> Value {
    let defaults = ProfileConfig::default();
    configure_value(&defaults, key)
}

fn set_configure_value(profile: &mut ProfileConfig, key: ConfigureKey, raw: &str) -> Result<Value> {
    let value = raw.trim();
    if value.is_empty() {
        return Err(MosaicError::Validation(
            "configure value cannot be empty".to_string(),
        ));
    }
    set_configure_value_from_json(profile, key, &Value::String(value.to_string()))
}

fn set_configure_value_from_json(
    profile: &mut ProfileConfig,
    key: ConfigureKey,
    value: &Value,
) -> Result<Value> {
    match key {
        ConfigureKey::ProviderBaseUrl => {
            let parsed = require_string_value(value, "provider.base_url")?;
            profile.provider.base_url = parsed;
            Ok(json!(profile.provider.base_url))
        }
        ConfigureKey::ProviderModel => {
            let parsed = require_string_value(value, "provider.model")?;
            profile.provider.model = parsed;
            Ok(json!(profile.provider.model))
        }
        ConfigureKey::ProviderApiKeyEnv => {
            let parsed = require_string_value(value, "provider.api_key_env")?;
            profile.provider.api_key_env = parsed;
            Ok(json!(profile.provider.api_key_env))
        }
        ConfigureKey::AgentTemperature => {
            let parsed = parse_float_value(value, "agent.temperature")?;
            if !parsed.is_finite() {
                return Err(MosaicError::Validation(
                    "agent.temperature must be finite".to_string(),
                ));
            }
            profile.agent.temperature = parsed;
            Ok(json!(profile.agent.temperature))
        }
        ConfigureKey::AgentMaxTurns => {
            let parsed = parse_u32_value(value, "agent.max_turns")?;
            if parsed == 0 {
                return Err(MosaicError::Validation(
                    "agent.max_turns must be greater than 0".to_string(),
                ));
            }
            profile.agent.max_turns = parsed;
            Ok(json!(profile.agent.max_turns))
        }
        ConfigureKey::ToolsEnabled => {
            let parsed = parse_bool_from_value(value, "tools.enabled")?;
            profile.tools.enabled = parsed;
            Ok(json!(profile.tools.enabled))
        }
        ConfigureKey::ToolsRunGuardMode => {
            let parsed = parse_guard_mode_from_value(value, "tools.run.guard_mode")?;
            profile.tools.run.guard_mode = parsed;
            Ok(json!(guard_mode_name(&profile.tools.run.guard_mode)))
        }
    }
}

fn unset_configure_value(profile: &mut ProfileConfig, key: ConfigureKey) -> Value {
    let defaults = ProfileConfig::default();
    match key {
        ConfigureKey::ProviderBaseUrl => {
            profile.provider.base_url = defaults.provider.base_url;
            json!(profile.provider.base_url)
        }
        ConfigureKey::ProviderModel => {
            profile.provider.model = defaults.provider.model;
            json!(profile.provider.model)
        }
        ConfigureKey::ProviderApiKeyEnv => {
            profile.provider.api_key_env = defaults.provider.api_key_env;
            json!(profile.provider.api_key_env)
        }
        ConfigureKey::AgentTemperature => {
            profile.agent.temperature = defaults.agent.temperature;
            json!(profile.agent.temperature)
        }
        ConfigureKey::AgentMaxTurns => {
            profile.agent.max_turns = defaults.agent.max_turns;
            json!(profile.agent.max_turns)
        }
        ConfigureKey::ToolsEnabled => {
            profile.tools.enabled = defaults.tools.enabled;
            json!(profile.tools.enabled)
        }
        ConfigureKey::ToolsRunGuardMode => {
            profile.tools.run.guard_mode = defaults.tools.run.guard_mode;
            json!(guard_mode_name(&profile.tools.run.guard_mode))
        }
    }
}

fn handle_configure_patch(
    cli: &Cli,
    manager: &ConfigManager,
    config: &mut ConfigFile,
    args: ConfigurePatchArgs,
    operation: ConfigurePatchOperation,
) -> Result<()> {
    let ConfigurePatchArgs {
        set,
        file,
        target_profile,
        dry_run,
    } = args;
    let action_name = operation.action_name();
    let dry_run = operation.dry_run(dry_run);
    let target_profile = target_profile.unwrap_or_else(|| cli.profile.clone());
    let target_profile_exists = config.profiles.contains_key(&target_profile);
    let mut assignments = Vec::<ConfigurePatchAssignment>::new();
    let mut file_path = None;
    if let Some(path) = file {
        assignments.extend(load_configure_patch_file(&path)?);
        file_path = Some(path);
    }
    assignments.extend(
        set.iter()
            .map(|entry| parse_configure_patch_set(entry))
            .collect::<Result<Vec<_>>>()?,
    );
    if assignments.is_empty() {
        return Err(MosaicError::Validation(format!(
            "configure {} requires at least one update via --set or --file",
            action_name
        )));
    }

    // Keep latest assignment when the same key appears multiple times.
    let mut merged = Vec::<ConfigurePatchAssignment>::new();
    for assignment in assignments {
        if let Some(existing) = merged.iter_mut().find(|entry| entry.key == assignment.key) {
            *existing = assignment;
        } else {
            merged.push(assignment);
        }
    }

    let current_profile = config
        .profiles
        .get(&target_profile)
        .cloned()
        .unwrap_or_default();
    let mut staged_profile = current_profile.clone();
    let mut updates = Vec::with_capacity(merged.len());
    let mut groups = BTreeMap::<&'static str, (usize, usize)>::new();
    let mut changed_keys = 0usize;
    for assignment in merged {
        let from = configure_value(&staged_profile, assignment.key);
        let value =
            set_configure_value_from_json(&mut staged_profile, assignment.key, &assignment.value)?;
        let changed = from != value;
        let group = configure_key_group(assignment.key);
        let group_entry = groups.entry(group).or_insert((0, 0));
        group_entry.0 += 1;
        if changed {
            changed_keys += 1;
            group_entry.1 += 1;
        }
        updates.push(json!({
            "key": configure_key_name(assignment.key),
            "group": group,
            "from": from,
            "to": value.clone(),
            "value": value,
            "changed": changed,
            "source": assignment.source,
        }));
    }
    let group_summaries = groups
        .into_iter()
        .map(|(group, (updated, changed))| {
            json!({
                "group": group,
                "updated": updated,
                "changed": changed,
                "unchanged": updated.saturating_sub(changed),
            })
        })
        .collect::<Vec<_>>();
    let changed = changed_keys > 0;

    if !dry_run {
        config
            .profiles
            .insert(target_profile.clone(), staged_profile);
        manager.save(config)?;
    }

    if cli.json {
        print_json(&json!({
            "ok": true,
            "action": action_name,
            "changed": changed,
            "dry_run": dry_run,
            "saved": !dry_run,
            "profile": cli.profile,
            "target_profile": target_profile,
            "target_profile_exists": target_profile_exists,
            "updated": updates.len(),
            "changed_keys": changed_keys,
            "groups": group_summaries,
            "updates": updates,
            "file": file_path,
            "config_path": manager.path().display().to_string(),
        }));
    } else {
        println!("profile: {}", cli.profile);
        println!("target_profile: {}", target_profile);
        println!("action: {action_name}");
        println!("dry_run: {dry_run}");
        println!("saved: {}", !dry_run);
        println!("updated: {}", updates.len());
        println!("changed_keys: {changed_keys}");
        for group in &group_summaries {
            println!(
                "* group {}: updated={}, changed={}, unchanged={}",
                group["group"], group["updated"], group["changed"], group["unchanged"]
            );
        }
        if let Some(path) = file_path {
            println!("file: {path}");
        }
        for update in updates {
            println!(
                "- {}: {} -> {} (changed={})",
                update["key"], update["from"], update["to"], update["changed"]
            );
        }
        if !dry_run {
            println!("config path: {}", manager.path().display());
        }
    }
    Ok(())
}

fn handle_configure_template(
    cli: &Cli,
    manager: &ConfigManager,
    config: &ConfigFile,
    args: ConfigureTemplateArgs,
) -> Result<()> {
    let ConfigureTemplateArgs {
        format,
        defaults,
        target_profile,
    } = args;
    let format = ConfigureTemplateFormat::from(format);
    let target_profile = target_profile.unwrap_or_else(|| cli.profile.clone());
    let target_profile_exists = config.profiles.contains_key(&target_profile);
    let template_profile = if defaults {
        ProfileConfig::default()
    } else {
        config
            .profiles
            .get(&target_profile)
            .cloned()
            .unwrap_or_default()
    };
    let template_json = build_configure_patch_template_value(&template_profile);
    let template = render_configure_patch_template(&template_json, format)?;

    if cli.json {
        print_json(&json!({
            "ok": true,
            "action": "template",
            "profile": cli.profile,
            "target_profile": target_profile,
            "target_profile_exists": target_profile_exists,
            "defaults": defaults,
            "format": format.as_str(),
            "template": template,
            "template_json": template_json,
            "config_path": manager.path().display().to_string(),
        }));
    } else {
        println!("profile: {}", cli.profile);
        println!("target_profile: {}", target_profile);
        println!("format: {}", format.as_str());
        println!("defaults: {defaults}");
        println!();
        print!("{template}");
        if !template.ends_with('\n') {
            println!();
        }
    }
    Ok(())
}

fn build_configure_patch_template_value(profile: &ProfileConfig) -> Value {
    json!({
        "provider": {
            "base_url": profile.provider.base_url,
            "model": profile.provider.model,
            "api_key_env": profile.provider.api_key_env,
        },
        "agent": {
            "temperature": profile.agent.temperature,
            "max_turns": profile.agent.max_turns,
        },
        "tools": {
            "enabled": profile.tools.enabled,
            "run": {
                "guard_mode": guard_mode_name(&profile.tools.run.guard_mode),
            },
        },
    })
}

fn render_configure_patch_template(
    template_json: &Value,
    format: ConfigureTemplateFormat,
) -> Result<String> {
    match format {
        ConfigureTemplateFormat::Json => {
            serde_json::to_string_pretty(template_json).map_err(|err| {
                MosaicError::Validation(format!("failed to render JSON template: {err}"))
            })
        }
        ConfigureTemplateFormat::Toml => toml::to_string_pretty(template_json).map_err(|err| {
            MosaicError::Validation(format!("failed to render TOML template: {err}"))
        }),
    }
}

#[derive(Debug, Clone)]
struct ConfigurePatchAssignment {
    key: ConfigureKey,
    value: Value,
    source: String,
}

fn load_configure_patch_file(path: &str) -> Result<Vec<ConfigurePatchAssignment>> {
    let raw = fs::read_to_string(path).map_err(|err| {
        MosaicError::Io(format!("failed reading configure patch file {path}: {err}"))
    })?;

    let parsed = parse_configure_patch_document(path, &raw)?;
    let object = parsed.as_object().ok_or_else(|| {
        MosaicError::Validation(format!(
            "configure patch file '{}' must be a JSON/TOML object",
            path
        ))
    })?;
    if object.is_empty() {
        return Err(MosaicError::Validation(format!(
            "configure patch file '{}' cannot be empty",
            path
        )));
    }

    let mut flattened = Vec::<(String, Value)>::new();
    flatten_configure_patch_object("", object, &mut flattened)?;
    let mut updates = Vec::with_capacity(flattened.len());
    for (key, value) in flattened {
        let parsed_key = parse_configure_key(&key)?;
        updates.push(ConfigurePatchAssignment {
            key: parsed_key,
            value,
            source: "file".to_string(),
        });
    }
    Ok(updates)
}

fn parse_configure_patch_document(path: &str, raw: &str) -> Result<Value> {
    let lower = path.to_ascii_lowercase();
    if lower.ends_with(".json") {
        return serde_json::from_str::<Value>(raw).map_err(|err| {
            MosaicError::Validation(format!(
                "invalid JSON configure patch file '{}': {err}",
                path
            ))
        });
    }
    if lower.ends_with(".toml") {
        let parsed = toml::from_str::<toml::Value>(raw).map_err(|err| {
            MosaicError::Validation(format!(
                "invalid TOML configure patch file '{}': {err}",
                path
            ))
        })?;
        return serde_json::to_value(parsed).map_err(|err| {
            MosaicError::Validation(format!(
                "failed to map TOML configure patch file '{}' to JSON: {err}",
                path
            ))
        });
    }
    if let Ok(json_value) = serde_json::from_str::<Value>(raw) {
        return Ok(json_value);
    }
    let parsed = toml::from_str::<toml::Value>(raw).map_err(|err| {
        MosaicError::Validation(format!(
            "configure patch file '{}' must be valid JSON or TOML: {err}",
            path
        ))
    })?;
    serde_json::to_value(parsed).map_err(|err| {
        MosaicError::Validation(format!(
            "failed to map configure patch file '{}' to JSON: {err}",
            path
        ))
    })
}

fn flatten_configure_patch_object(
    prefix: &str,
    object: &serde_json::Map<String, Value>,
    out: &mut Vec<(String, Value)>,
) -> Result<()> {
    for (key, value) in object {
        let next_key = if prefix.is_empty() {
            key.clone()
        } else {
            format!("{prefix}.{key}")
        };
        match value {
            Value::Object(nested) => flatten_configure_patch_object(&next_key, nested, out)?,
            Value::String(_) | Value::Bool(_) | Value::Number(_) => {
                out.push((next_key, value.clone()));
            }
            Value::Array(_) | Value::Null => {
                return Err(MosaicError::Validation(format!(
                    "configure patch key '{}' must be a scalar string/number/bool value",
                    next_key
                )));
            }
        }
    }
    Ok(())
}

fn parse_configure_patch_set(raw: &str) -> Result<ConfigurePatchAssignment> {
    let Some((key, value)) = raw.split_once('=') else {
        return Err(MosaicError::Validation(format!(
            "invalid patch assignment '{}', expected KEY=VALUE",
            raw
        )));
    };
    let parsed_key = parse_configure_key(key)?;
    let trimmed_value = value.trim();
    if trimmed_value.is_empty() {
        return Err(MosaicError::Validation(format!(
            "invalid patch assignment '{}', value cannot be empty",
            raw
        )));
    }
    Ok(ConfigurePatchAssignment {
        key: parsed_key,
        value: Value::String(trimmed_value.to_string()),
        source: "set".to_string(),
    })
}

fn normalize_configure_key(raw: &str) -> String {
    raw.trim().to_ascii_lowercase().replace('-', "_")
}

fn parse_bool_value(raw: &str) -> Result<bool> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "true" | "1" | "yes" | "y" | "on" => Ok(true),
        "false" | "0" | "no" | "n" | "off" => Ok(false),
        _ => Err(MosaicError::Validation(format!(
            "invalid boolean value '{raw}', expected true/false"
        ))),
    }
}

fn parse_bool_from_value(value: &Value, field_name: &str) -> Result<bool> {
    match value {
        Value::Bool(parsed) => Ok(*parsed),
        Value::String(raw) => parse_bool_value(raw),
        Value::Number(number) => match number.as_i64() {
            Some(0) => Ok(false),
            Some(1) => Ok(true),
            _ => Err(MosaicError::Validation(format!(
                "{field_name} must be true/false or 0/1"
            ))),
        },
        _ => Err(MosaicError::Validation(format!(
            "{field_name} must be a boolean-compatible value"
        ))),
    }
}

fn parse_guard_mode(raw: &str) -> Result<RunGuardMode> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "confirm_dangerous" => Ok(RunGuardMode::ConfirmDangerous),
        "all_confirm" => Ok(RunGuardMode::AllConfirm),
        "unrestricted" => Ok(RunGuardMode::Unrestricted),
        _ => Err(MosaicError::Validation(format!(
            "invalid guard mode '{raw}', expected confirm_dangerous|all_confirm|unrestricted"
        ))),
    }
}

fn parse_guard_mode_from_value(value: &Value, field_name: &str) -> Result<RunGuardMode> {
    match value {
        Value::String(raw) => parse_guard_mode(raw),
        _ => Err(MosaicError::Validation(format!(
            "{field_name} must be a string value"
        ))),
    }
}

fn parse_float_value(value: &Value, field_name: &str) -> Result<f32> {
    match value {
        Value::Number(number) => Ok(number.as_f64().ok_or_else(|| {
            MosaicError::Validation(format!("{field_name} must be a finite float value"))
        })? as f32),
        Value::String(raw) => raw.trim().parse::<f32>().map_err(|err| {
            MosaicError::Validation(format!("invalid float for {field_name}: {err}"))
        }),
        _ => Err(MosaicError::Validation(format!(
            "{field_name} must be a float-compatible value"
        ))),
    }
}

fn parse_u32_value(value: &Value, field_name: &str) -> Result<u32> {
    match value {
        Value::Number(number) => {
            let Some(parsed) = number.as_u64() else {
                return Err(MosaicError::Validation(format!(
                    "{field_name} must be a non-negative integer"
                )));
            };
            u32::try_from(parsed)
                .map_err(|_| MosaicError::Validation(format!("{field_name} is too large for u32")))
        }
        Value::String(raw) => raw.trim().parse::<u32>().map_err(|err| {
            MosaicError::Validation(format!("invalid integer for {field_name}: {err}"))
        }),
        _ => Err(MosaicError::Validation(format!(
            "{field_name} must be an integer-compatible value"
        ))),
    }
}

fn require_string_value(value: &Value, field_name: &str) -> Result<String> {
    match value {
        Value::String(raw) => {
            let trimmed = raw.trim();
            if trimmed.is_empty() {
                return Err(MosaicError::Validation(format!(
                    "{field_name} cannot be empty"
                )));
            }
            Ok(trimmed.to_string())
        }
        _ => Err(MosaicError::Validation(format!(
            "{field_name} must be a string value"
        ))),
    }
}

fn guard_mode_name(mode: &RunGuardMode) -> &'static str {
    match mode {
        RunGuardMode::ConfirmDangerous => "confirm_dangerous",
        RunGuardMode::AllConfirm => "all_confirm",
        RunGuardMode::Unrestricted => "unrestricted",
    }
}

pub(super) async fn handle_models(cli: &Cli, args: ModelsArgs) -> Result<()> {
    match args.command {
        ModelsCommand::List { query, limit } => {
            let runtime = build_runtime(cli, None, None)?;
            let query = normalize_models_query(query)?;
            let limit = normalize_models_limit(limit)?;
            let mut models = runtime.provider.list_models().await?;
            let total_models = models.len();
            if let Some(query) = query.as_ref() {
                let query_lc = query.to_ascii_lowercase();
                models.retain(|model| model.id.to_ascii_lowercase().contains(&query_lc));
            }
            let matched_models = models.len();
            if let Some(limit) = limit
                && models.len() > limit
            {
                models.truncate(limit);
            }
            let returned_models = models.len();
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "query": query,
                    "limit": limit,
                    "total_models": total_models,
                    "matched_models": matched_models,
                    "returned_models": returned_models,
                    "models": models,
                }));
            } else {
                for model in &models {
                    if let Some(owner) = &model.owned_by {
                        println!("{} ({owner})", model.id);
                    } else {
                        println!("{}", model.id);
                    }
                }
                println!("Total models: {total_models}");
                if let Some(query) = query {
                    println!("Query: {query}");
                }
                if let Some(limit) = limit {
                    println!("Limit: {limit}");
                }
                println!("Matched models: {matched_models}");
                println!("Returned models: {returned_models}");
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
                        event_callback: None,
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
                event_callback: None,
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
                        event_callback: None,
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
                    event_callback: None,
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
                            event_callback: None,
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

fn normalize_models_query(query: Option<String>) -> Result<Option<String>> {
    match query {
        Some(value) => {
            let trimmed = value.trim();
            if trimmed.is_empty() {
                return Err(MosaicError::Validation(
                    "--query cannot be empty".to_string(),
                ));
            }
            Ok(Some(trimmed.to_string()))
        }
        None => Ok(None),
    }
}

fn normalize_models_limit(limit: Option<usize>) -> Result<Option<usize>> {
    match limit {
        Some(0) => Err(MosaicError::Validation(
            "--limit must be greater than 0".to_string(),
        )),
        _ => Ok(limit),
    }
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
