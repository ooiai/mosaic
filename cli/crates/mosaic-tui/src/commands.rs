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
    if input == "/new" {
        return Some(TuiInputCommand::NewSession);
    }
    if input == "/status" {
        return Some(TuiInputCommand::Status);
    }
    None
}
