use serde_json::json;

use mosaic_agents::{
    AddAgentInput, AgentStore, UpdateAgentInput, agent_routes_path, agents_file_path,
};
use mosaic_core::config::ConfigManager;
use mosaic_core::error::{MosaicError, Result};
use mosaic_plugins::{ExtensionRegistry, RegistryRoots};

use super::{AgentsArgs, AgentsCommand, AgentsRouteCommand, Cli, print_json, resolve_state_paths};

pub(super) fn handle_agents(cli: &Cli, args: AgentsArgs) -> Result<()> {
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
                let agent_rows = agents
                    .iter()
                    .map(|agent| {
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
                        json!({
                            "id": &agent.id,
                            "name": &agent.name,
                            "profile": &agent.profile,
                            "skills": &agent.skills,
                            "model": &agent.model,
                            "temperature": &agent.temperature,
                            "max_turns": &agent.max_turns,
                            "tools_enabled": &agent.tools_enabled,
                            "guard_mode": &agent.guard_mode,
                            "created_at": &agent.created_at,
                            "updated_at": &agent.updated_at,
                            "is_default": routes.default_agent_id.as_deref() == Some(agent.id.as_str()),
                            "route_keys": route_keys,
                        })
                    })
                    .collect::<Vec<_>>();
                print_json(&json!({
                    "ok": true,
                    "agents": agent_rows,
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
                    let skills = if agent.skills.is_empty() {
                        "-".to_string()
                    } else {
                        agent.skills.join(",")
                    };
                    let tags = {
                        let mut tags = Vec::new();
                        if routes.default_agent_id.as_deref() == Some(agent.id.as_str()) {
                            tags.push("default".to_string());
                        }
                        if !route_keys.is_empty() {
                            tags.push(format!("routes={}", route_keys.join(",")));
                        }
                        if tags.is_empty() {
                            "-".to_string()
                        } else {
                            tags.join(" ")
                        }
                    };
                    println!(
                        "- {} ({}) profile={} skills={} tags={} model={} temperature={} max_turns={}",
                        agent.id,
                        agent.name,
                        agent.profile,
                        skills,
                        tags,
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
        AgentsCommand::Current {
            agent,
            session,
            route,
        } => {
            if !manager.exists() {
                return Err(MosaicError::Config(
                    "config file not found. run `mosaic setup` first".to_string(),
                ));
            }
            let config = manager.load()?;
            let routes = store.load_routes()?;
            let normalized_route = route.as_deref().map(normalize_route_key).transpose()?;
            let session_store = mosaic_core::session::SessionStore::new(paths.sessions_dir.clone());
            let session_runtime = match session.as_deref() {
                Some(session_id) => session_store.latest_runtime_metadata(session_id)?,
                None => None,
            };
            let session_agent_id = if agent.is_none() {
                session_runtime
                    .as_ref()
                    .and_then(|runtime| runtime.agent_id.clone())
            } else {
                None
            };
            let route_agent_id = normalized_route
                .as_ref()
                .and_then(|route_key| routes.routes.get(route_key).cloned());
            let resolved = store.resolve_effective_profile(
                &config,
                &cli.profile,
                agent.as_deref().or(session_agent_id.as_deref()),
                normalized_route.as_deref(),
            )?;
            let resolution_source = if agent.is_some() {
                "explicit_agent"
            } else if session_agent_id.is_some() {
                "session_runtime"
            } else if route_agent_id.is_some() {
                "route_binding"
            } else if routes.default_agent_id.is_some() {
                "default_agent"
            } else {
                "cli_profile"
            };
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "resolution_source": resolution_source,
                    "explicit_agent_id": agent,
                    "session_id": session,
                    "session_agent_id": session_agent_id,
                    "session_profile_name": session_runtime.as_ref().map(|runtime| runtime.profile_name.clone()),
                    "route_key": normalized_route,
                    "route_agent_id": route_agent_id,
                    "default_agent_id": routes.default_agent_id,
                    "resolved_agent_id": resolved.agent_id,
                    "resolved_profile_name": resolved.profile_name,
                    "profile": resolved.profile,
                }));
            } else {
                println!("resolution source: {resolution_source}");
                println!(
                    "resolved agent: {}",
                    resolved.agent_id.as_deref().unwrap_or("<none>")
                );
                println!("resolved profile: {}", resolved.profile_name);
                println!("explicit agent: {}", agent.as_deref().unwrap_or("<none>"));
                println!("session: {}", session.as_deref().unwrap_or("<none>"));
                println!(
                    "session agent: {}",
                    session_agent_id.as_deref().unwrap_or("<none>")
                );
                println!("route: {}", normalized_route.as_deref().unwrap_or("<none>"));
                println!(
                    "route agent: {}",
                    route_agent_id.as_deref().unwrap_or("<none>")
                );
                println!(
                    "default agent: {}",
                    routes.default_agent_id.as_deref().unwrap_or("<none>")
                );
            }
        }
        AgentsCommand::Add {
            name,
            id,
            profile,
            skills,
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
            let validated_skills = resolve_skill_ids(&paths.root_dir, skills)?;

            let created = store.add(AddAgentInput {
                id,
                name,
                profile,
                skills: validated_skills,
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
                if created.skills.is_empty() {
                    println!("skills: <none>");
                } else {
                    println!("skills: {}", created.skills.join(", "));
                }
            }
        }
        AgentsCommand::Update {
            agent_id,
            name,
            profile,
            skills,
            clear_skills,
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
            let resolved_skills = if skills.is_empty() {
                None
            } else {
                Some(resolve_skill_ids(&paths.root_dir, skills)?)
            };

            let updated = store.update(
                &agent_id,
                UpdateAgentInput {
                    name,
                    profile,
                    skills: resolved_skills,
                    clear_skills,
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
                if updated.skills.is_empty() {
                    println!("skills: <none>");
                } else {
                    println!("skills: {}", updated.skills.join(", "));
                }
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
                if agent.skills.is_empty() {
                    println!("skills: <none>");
                } else {
                    println!("skills: {}", agent.skills.join(", "));
                }
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

fn normalize_route_key(raw: &str) -> Result<String> {
    let value = raw.trim();
    if value.is_empty() {
        return Err(MosaicError::Validation(
            "route key cannot be empty".to_string(),
        ));
    }
    Ok(value.to_string())
}

fn resolve_skill_ids(state_root: &std::path::Path, skills: Vec<String>) -> Result<Vec<String>> {
    if skills.is_empty() {
        return Ok(Vec::new());
    }
    let registry = ExtensionRegistry::new(RegistryRoots::from_state_root(state_root.to_path_buf()));
    let available = registry
        .list_skills()?
        .into_iter()
        .map(|skill| skill.id)
        .collect::<std::collections::BTreeSet<_>>();
    let mut resolved = Vec::new();
    for raw in skills {
        let id = raw.trim();
        if id.is_empty() {
            return Err(MosaicError::Validation(
                "agent skill id cannot be empty".to_string(),
            ));
        }
        if !available.contains(id) {
            return Err(MosaicError::Validation(format!(
                "skill '{id}' not found; run `mosaic skills list` first"
            )));
        }
        if !resolved.iter().any(|existing| existing == id) {
            resolved.push(id.to_string());
        }
    }
    Ok(resolved)
}
