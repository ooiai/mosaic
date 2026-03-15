#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TuiInputCommand<'a> {
    Agent,
    Agents,
    AgentSet(&'a str),
    Session,
    SessionSet(&'a str),
    NewSession,
    Status,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CommandPaletteItem {
    pub(crate) label: &'static str,
    pub(crate) insert_text: &'static str,
    pub(crate) description: &'static str,
    pub(crate) implemented: bool,
}

const COMMANDS: [CommandPaletteItem; 11] = [
    CommandPaletteItem {
        label: "/add-dir <directory>",
        insert_text: "/add-dir ",
        description: "Add a directory to the allowed list for file access",
        implemented: false,
    },
    CommandPaletteItem {
        label: "/agent [id]",
        insert_text: "/agent ",
        description: "Browse and select from available agents (if any)",
        implemented: true,
    },
    CommandPaletteItem {
        label: "/allow-all, /yolo",
        insert_text: "/allow-all",
        description: "Enable all permissions (tools, paths, and URLs)",
        implemented: false,
    },
    CommandPaletteItem {
        label: "/changelog [summarize] [version | last <N> | …]",
        insert_text: "/changelog ",
        description: "Display changelog for CLI versions. Add 'summarize' to get an AI summary.",
        implemented: false,
    },
    CommandPaletteItem {
        label: "/clear, /new",
        insert_text: "/clear",
        description: "Clear the conversation history",
        implemented: true,
    },
    CommandPaletteItem {
        label: "/compact",
        insert_text: "/compact",
        description: "Summarize conversation history to reduce context window usage",
        implemented: false,
    },
    CommandPaletteItem {
        label: "/context",
        insert_text: "/context",
        description: "Show context window token usage and visualization",
        implemented: false,
    },
    CommandPaletteItem {
        label: "/copy",
        insert_text: "/copy",
        description: "Copy the last response to the clipboard",
        implemented: false,
    },
    CommandPaletteItem {
        label: "/cwd, /cd [directory]",
        insert_text: "/cwd",
        description: "Change working directory or show current directory",
        implemented: false,
    },
    CommandPaletteItem {
        label: "/delegate [prompt]",
        insert_text: "/delegate ",
        description: "Send this session to GitHub and Copilot will create a PR",
        implemented: false,
    },
    CommandPaletteItem {
        label: "/status",
        insert_text: "/status",
        description: "Print the active runtime summary",
        implemented: true,
    },
];

pub(crate) fn parse_input_command(input: &str) -> Option<TuiInputCommand<'_>> {
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

pub(crate) fn selected_command_palette_item(
    input: &str,
    selected_index: usize,
) -> Option<CommandPaletteItem> {
    let items = command_palette_items(input);
    if items.is_empty() {
        return None;
    }
    Some(items[selected_index.min(items.len() - 1)])
}
