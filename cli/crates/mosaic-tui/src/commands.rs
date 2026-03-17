use mosaic_core::session::SessionSummary;

use crate::TuiAgentOption;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TuiInputCommand<'a> {
    Help,
    Agent,
    Agents,
    AgentSet(&'a str),
    Session,
    SessionSet(&'a str),
    NewSession,
    Status,
    Models,
    Skills,
    Docs,
    Logs,
    Doctor,
    Memory,
    Knowledge,
    Plugins,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CommandPaletteItem {
    pub(crate) label: &'static str,
    pub(crate) insert_text: &'static str,
    pub(crate) description: &'static str,
    pub(crate) implemented: bool,
    pub(crate) shell_hint: Option<&'static str>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum CommandSuggestionSource {
    Local,
    Shell,
    Agent,
    Session,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CommandSuggestion {
    pub(crate) label: String,
    pub(crate) insert_text: String,
    pub(crate) description: String,
    pub(crate) implemented: bool,
    pub(crate) shell_hint: Option<String>,
    pub(crate) source: CommandSuggestionSource,
}

const COMMANDS: [CommandPaletteItem; 14] = [
    CommandPaletteItem {
        label: "/help",
        insert_text: "/help",
        description: "Open the fullscreen TUI help overlay",
        implemented: true,
        shell_hint: None,
    },
    CommandPaletteItem {
        label: "/agents",
        insert_text: "/agents",
        description: "Open the agent picker",
        implemented: true,
        shell_hint: None,
    },
    CommandPaletteItem {
        label: "/agent [id]",
        insert_text: "/agent ",
        description: "Browse and select from available agents (if any)",
        implemented: true,
        shell_hint: None,
    },
    CommandPaletteItem {
        label: "/session [id]",
        insert_text: "/session ",
        description: "Show the active session or resume a session by id",
        implemented: true,
        shell_hint: None,
    },
    CommandPaletteItem {
        label: "/clear, /new",
        insert_text: "/clear",
        description: "Clear the current conversation and start a fresh session",
        implemented: true,
        shell_hint: None,
    },
    CommandPaletteItem {
        label: "/status",
        insert_text: "/status",
        description: "Print the active runtime summary",
        implemented: true,
        shell_hint: None,
    },
    CommandPaletteItem {
        label: "/models",
        insert_text: "/models",
        description: "Inspect the active model routing summary inside the TUI",
        implemented: true,
        shell_hint: None,
    },
    CommandPaletteItem {
        label: "/skills",
        insert_text: "/skills",
        description: "List installed skills inside the TUI",
        implemented: true,
        shell_hint: None,
    },
    CommandPaletteItem {
        label: "/docs",
        insert_text: "/docs",
        description: "List docs topics inside the TUI",
        implemented: true,
        shell_hint: None,
    },
    CommandPaletteItem {
        label: "/logs",
        insert_text: "/logs",
        description: "Show recent local logs inside the TUI",
        implemented: true,
        shell_hint: None,
    },
    CommandPaletteItem {
        label: "/doctor",
        insert_text: "/doctor",
        description: "Run diagnostics inside the TUI",
        implemented: true,
        shell_hint: None,
    },
    CommandPaletteItem {
        label: "/memory",
        insert_text: "/memory",
        description: "Inspect memory namespace status inside the TUI",
        implemented: true,
        shell_hint: None,
    },
    CommandPaletteItem {
        label: "/knowledge",
        insert_text: "/knowledge",
        description: "List knowledge datasets inside the TUI",
        implemented: true,
        shell_hint: None,
    },
    CommandPaletteItem {
        label: "/plugins",
        insert_text: "/plugins",
        description: "Inspect installed plugins inside the TUI",
        implemented: true,
        shell_hint: None,
    },
];

impl From<CommandPaletteItem> for CommandSuggestion {
    fn from(value: CommandPaletteItem) -> Self {
        Self {
            label: value.label.to_string(),
            insert_text: value.insert_text.to_string(),
            description: value.description.to_string(),
            implemented: value.implemented,
            shell_hint: value.shell_hint.map(str::to_string),
            source: if value.shell_hint.is_some() {
                CommandSuggestionSource::Shell
            } else {
                CommandSuggestionSource::Local
            },
        }
    }
}

pub(crate) fn parse_input_command(input: &str) -> Option<TuiInputCommand<'_>> {
    if matches!(input, "/help" | "/?") {
        return Some(TuiInputCommand::Help);
    }
    if let Some(rest) = input.strip_prefix("/agent") {
        if rest.is_empty() {
            return Some(TuiInputCommand::Agent);
        }
        if rest.chars().next().is_some_and(char::is_whitespace) {
            let requested = rest.trim();
            if requested.is_empty() {
                return Some(TuiInputCommand::Agent);
            }
            return Some(TuiInputCommand::AgentSet(requested));
        }
    }
    if input == "/agents" {
        return Some(TuiInputCommand::Agents);
    }
    if let Some(rest) = input.strip_prefix("/session") {
        if rest.is_empty() {
            return Some(TuiInputCommand::Session);
        }
        if rest.chars().next().is_some_and(char::is_whitespace) {
            let requested = rest.trim();
            if requested.is_empty() {
                return Some(TuiInputCommand::Session);
            }
            return Some(TuiInputCommand::SessionSet(requested));
        }
    }
    if matches!(input, "/new" | "/clear") {
        return Some(TuiInputCommand::NewSession);
    }
    if input == "/status" {
        return Some(TuiInputCommand::Status);
    }
    if input == "/models" {
        return Some(TuiInputCommand::Models);
    }
    if input == "/skills" {
        return Some(TuiInputCommand::Skills);
    }
    if input == "/docs" {
        return Some(TuiInputCommand::Docs);
    }
    if input == "/logs" {
        return Some(TuiInputCommand::Logs);
    }
    if input == "/doctor" {
        return Some(TuiInputCommand::Doctor);
    }
    if input == "/memory" {
        return Some(TuiInputCommand::Memory);
    }
    if input == "/knowledge" {
        return Some(TuiInputCommand::Knowledge);
    }
    if input == "/plugins" {
        return Some(TuiInputCommand::Plugins);
    }
    None
}

pub(crate) fn command_palette_items(input: &str) -> Vec<CommandPaletteItem> {
    let trimmed = input.trim();
    if !trimmed.starts_with('/') {
        return Vec::new();
    }
    let token = trimmed.split_whitespace().next().unwrap_or(trimmed);
    COMMANDS
        .into_iter()
        .filter(|command| {
            token == "/"
                || command.insert_text.trim_end().starts_with(token)
                || command.label.starts_with(token)
        })
        .collect()
}

pub(crate) fn command_suggestions(
    input: &str,
    agents: &[TuiAgentOption],
    sessions: &[SessionSummary],
    active_session_id: Option<&str>,
) -> Vec<CommandSuggestion> {
    let raw = input.trim_start();
    if !raw.starts_with('/') {
        return Vec::new();
    }

    if let Some(query) = raw.strip_prefix("/agent").map(str::trim) {
        if raw == "/agent" || raw.starts_with("/agent ") {
            let suggestions = agent_suggestions(query, agents);
            if !suggestions.is_empty() {
                return suggestions;
            }
            return agent_fallback_suggestions();
        }
    }

    if let Some(query) = raw.strip_prefix("/session").map(str::trim) {
        if raw == "/session" || raw.starts_with("/session ") {
            let suggestions = session_suggestions(query, sessions, active_session_id);
            if !suggestions.is_empty() {
                return suggestions;
            }
            return session_fallback_suggestions();
        }
    }

    command_palette_items(input)
        .into_iter()
        .map(CommandSuggestion::from)
        .collect()
}

pub(crate) fn selected_command_suggestion(
    input: &str,
    agents: &[TuiAgentOption],
    sessions: &[SessionSummary],
    active_session_id: Option<&str>,
    selected_index: usize,
) -> Option<CommandSuggestion> {
    let items = command_suggestions(input, agents, sessions, active_session_id);
    if items.is_empty() {
        return None;
    }
    Some(items[selected_index.min(items.len() - 1)].clone())
}

fn agent_suggestions(query: &str, agents: &[TuiAgentOption]) -> Vec<CommandSuggestion> {
    let query = query.to_ascii_lowercase();
    agents
        .iter()
        .filter(|agent| {
            query.is_empty()
                || agent.id.to_ascii_lowercase().starts_with(&query)
                || agent.name.to_ascii_lowercase().starts_with(&query)
                || agent.id.to_ascii_lowercase().contains(&query)
                || agent.name.to_ascii_lowercase().contains(&query)
        })
        .map(|agent| {
            let mut description = format!("agent • profile={}", agent.profile_name);
            if agent.is_default {
                description.push_str(" • default");
            }
            if !agent.route_keys.is_empty() {
                description.push_str(&format!(" • routes={}", agent.route_keys.join(",")));
            }
            CommandSuggestion {
                label: format!("/agent {}", agent.id),
                insert_text: format!("/agent {}", agent.id),
                description,
                implemented: true,
                shell_hint: None,
                source: CommandSuggestionSource::Agent,
            }
        })
        .collect()
}

fn session_suggestions(
    query: &str,
    sessions: &[SessionSummary],
    active_session_id: Option<&str>,
) -> Vec<CommandSuggestion> {
    let query = query.to_ascii_lowercase();
    sessions
        .iter()
        .filter(|session| {
            let short = short_session_id(&session.session_id);
            query.is_empty()
                || session.session_id.to_ascii_lowercase().starts_with(&query)
                || short.to_ascii_lowercase().starts_with(&query)
                || session.session_id.to_ascii_lowercase().contains(&query)
                || short.to_ascii_lowercase().contains(&query)
        })
        .take(10)
        .map(|session| {
            let runtime = session.runtime.as_ref().map_or_else(
                || format!("{} events", session.event_count),
                |runtime| {
                    format!(
                        "{} / {}",
                        runtime.profile_name,
                        runtime.agent_id.as_deref().unwrap_or("<none>")
                    )
                },
            );
            let active_suffix = if active_session_id == Some(session.session_id.as_str()) {
                " • active"
            } else {
                ""
            };
            CommandSuggestion {
                label: format!("/session {}", short_session_id(&session.session_id)),
                insert_text: format!("/session {}", session.session_id),
                description: format!("resume session • {runtime}{active_suffix}"),
                implemented: true,
                shell_hint: None,
                source: CommandSuggestionSource::Session,
            }
        })
        .collect()
}

fn agent_fallback_suggestions() -> Vec<CommandSuggestion> {
    vec![
        CommandSuggestion {
            label: "/agents".to_string(),
            insert_text: "/agents".to_string(),
            description: "Open the agent picker or confirm whether any agents are configured"
                .to_string(),
            implemented: true,
            shell_hint: None,
            source: CommandSuggestionSource::Local,
        },
        CommandSuggestion {
            label: "mosaic agents add --id <id> --name <name> --model <model>".to_string(),
            insert_text: "/agents".to_string(),
            description:
                "No matching configured agents. Add one from the shell, then reopen the picker."
                    .to_string(),
            implemented: false,
            shell_hint: Some(
                "mosaic agents add --id <id> --name <name> --model <model>".to_string(),
            ),
            source: CommandSuggestionSource::Shell,
        },
    ]
}

fn session_fallback_suggestions() -> Vec<CommandSuggestion> {
    vec![
        CommandSuggestion {
            label: "/session".to_string(),
            insert_text: "/session".to_string(),
            description: "Show the active session id in the current conversation".to_string(),
            implemented: true,
            shell_hint: None,
            source: CommandSuggestionSource::Local,
        },
        CommandSuggestion {
            label: "/clear".to_string(),
            insert_text: "/clear".to_string(),
            description: "No matching saved session. Start a fresh session instead.".to_string(),
            implemented: true,
            shell_hint: None,
            source: CommandSuggestionSource::Local,
        },
    ]
}

fn short_session_id(session_id: &str) -> String {
    session_id.chars().take(8).collect()
}
