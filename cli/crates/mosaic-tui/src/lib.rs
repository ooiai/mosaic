use std::future::Future;
use std::io;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use crossterm::event::{self, Event};
use crossterm::execute;
use crossterm::terminal::{self, EnterAlternateScreen, LeaveAlternateScreen};
use mosaic_agent::{AgentEvent, AgentRunOptions, AgentRunner};
use mosaic_core::error::{MosaicError, Result};
use mosaic_core::session::SessionRuntimeMetadata;
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::mpsc;

mod commands;
mod events;
mod keys;
mod pickers;
mod render;
mod state;

use events::AppEvent;
use keys::handle_key;
use render::render;
use state::{InspectorLine, TuiState};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TuiFocus {
    Messages,
    Input,
    Sessions,
    Inspector,
}

impl TuiFocus {
    fn next(self, inspector_visible: bool) -> Self {
        match (self, inspector_visible) {
            (Self::Messages, _) => Self::Input,
            (Self::Input, _) => Self::Sessions,
            (Self::Sessions, true) => Self::Inspector,
            (Self::Sessions, false) => Self::Messages,
            (Self::Inspector, _) => Self::Messages,
        }
    }

    fn prev(self, inspector_visible: bool) -> Self {
        match (self, inspector_visible) {
            (Self::Messages, true) => Self::Inspector,
            (Self::Messages, false) => Self::Sessions,
            (Self::Input, _) => Self::Messages,
            (Self::Sessions, _) => Self::Input,
            (Self::Inspector, _) => Self::Sessions,
        }
    }
}

#[derive(Clone)]
pub struct TuiRuntime {
    pub agent: AgentRunner,
    pub profile_name: String,
    pub agent_id: Option<String>,
    pub model_name: String,
    pub session_metadata: SessionRuntimeMetadata,
    pub policy_summary: String,
    pub switch_agent: Arc<dyn Fn(&str) -> Result<SwitchedTuiRuntime> + Send + Sync>,
    pub load_session_runtime: Arc<dyn Fn(Option<&str>) -> Result<SwitchedTuiRuntime> + Send + Sync>,
    pub list_agents: Arc<dyn Fn() -> Result<Vec<TuiAgentOption>> + Send + Sync>,
    pub run_local_command: Arc<dyn Fn(TuiLocalCommand) -> TuiLocalCommandFuture + Send + Sync>,
}

pub struct SwitchedTuiRuntime {
    pub agent: AgentRunner,
    pub profile_name: String,
    pub agent_id: Option<String>,
    pub model_name: String,
    pub session_metadata: SessionRuntimeMetadata,
    pub policy_summary: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TuiLocalCommand {
    Models,
    Skills,
    Docs,
    Logs,
    Doctor,
    Memory,
    Knowledge,
    Plugins,
}

impl TuiLocalCommand {
    pub fn slash_name(self) -> &'static str {
        match self {
            Self::Models => "/models",
            Self::Skills => "/skills",
            Self::Docs => "/docs",
            Self::Logs => "/logs",
            Self::Doctor => "/doctor",
            Self::Memory => "/memory",
            Self::Knowledge => "/knowledge",
            Self::Plugins => "/plugins",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TuiLocalCommandOutput {
    pub title: String,
    pub body: String,
    pub status: String,
    pub inspector_detail: String,
}

pub type TuiLocalCommandFuture =
    Pin<Box<dyn Future<Output = Result<TuiLocalCommandOutput>> + Send>>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TuiAgentOption {
    pub id: String,
    pub name: String,
    pub profile_name: String,
    pub is_default: bool,
    pub route_keys: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct TuiOptions {
    pub initial_session_id: Option<String>,
    pub initial_focus: TuiFocus,
    pub show_inspector: bool,
    pub yes: bool,
    pub cwd: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TuiStartupContext {
    pub custom_instruction_count: usize,
    pub mcp_server_count: usize,
    pub skill_count: usize,
    pub agent_count: usize,
    pub git_branch: Option<String>,
    pub pending_requests: usize,
}

pub async fn run_tui(runtime: TuiRuntime, options: TuiOptions) -> Result<()> {
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen).map_err(map_io)?;
    terminal::enable_raw_mode().map_err(map_io)?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend).map_err(map_io)?;
    let app_result = run_app(&mut terminal, runtime, options).await;

    terminal::disable_raw_mode().map_err(map_io)?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen).map_err(map_io)?;
    terminal.show_cursor().map_err(map_io)?;

    app_result
}

async fn run_app(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    mut runtime: TuiRuntime,
    options: TuiOptions,
) -> Result<()> {
    let session_store = runtime.agent.session_store().clone();
    session_store.ensure_dirs()?;

    let mut sessions = session_store.list_sessions()?;
    let mut active_session_id = options.initial_session_id.clone();
    if active_session_id.is_none() {
        active_session_id = session_store.latest_session_id()?;
    }

    let mut state = TuiState::new(
        options.initial_focus,
        options.show_inspector,
        std::mem::take(&mut sessions),
        active_session_id.clone(),
        &runtime.profile_name,
        runtime.agent_id.as_deref(),
        &runtime.policy_summary,
    );
    state.agents = (runtime.list_agents)().unwrap_or_default();

    if let Some(session_id) = active_session_id {
        state.load_session(&session_store, Some(session_id))?;
    }

    let (app_tx, mut app_rx) = mpsc::unbounded_channel::<AppEvent>();
    let cwd_display = options.cwd.display().to_string();
    let startup_context = build_startup_context(
        &options.cwd,
        (runtime.list_agents)()
            .map(|agents| agents.len())
            .unwrap_or(0),
    );

    let mut should_exit = false;
    while !should_exit {
        while let Ok(event) = app_rx.try_recv() {
            match event {
                AppEvent::Agent(agent_event) => {
                    state.apply_agent_event(agent_event);
                }
                AppEvent::AskDone(result) => {
                    state.running = false;
                    match result {
                        Ok(run) => {
                            state.active_session_id = Some(run.session_id);
                            state.status = "completed".to_string();
                            state.refresh_sessions(&session_store)?;
                        }
                        Err(err) => {
                            state.status = format!("error: {}", err);
                            state.inspector.push(InspectorLine {
                                kind: "error".to_string(),
                                detail: err.to_string(),
                            });
                        }
                    }
                }
            }
        }

        terminal
            .draw(|frame| {
                render(
                    frame,
                    &state,
                    runtime.agent_id.as_deref(),
                    &runtime.profile_name,
                    &runtime.model_name,
                    &runtime.policy_summary,
                    &cwd_display,
                    &startup_context,
                )
            })
            .map_err(map_io)?;

        if event::poll(Duration::from_millis(40)).map_err(map_io)? {
            let Event::Key(key) = event::read().map_err(map_io)? else {
                continue;
            };
            if handle_key(
                &mut state,
                &session_store,
                key,
                &mut runtime,
                &options,
                &app_tx,
            )
            .await?
            {
                should_exit = true;
            }
        }
    }

    Ok(())
}

fn spawn_agent_task(
    state: &TuiState,
    runtime: &TuiRuntime,
    options: &TuiOptions,
    app_tx: &mpsc::UnboundedSender<AppEvent>,
    prompt: String,
) {
    let tx_done = app_tx.clone();
    let tx_events = app_tx.clone();
    let runner = runtime.agent.clone();
    let cwd = options.cwd.clone();
    let yes = options.yes;
    let session_id = state.active_session_id.clone();
    let session_metadata = runtime.session_metadata.clone();
    tokio::spawn(async move {
        let callback = Arc::new(move |event: AgentEvent| {
            let _ = tx_events.send(AppEvent::Agent(event));
        });
        let result = runner
            .ask(
                &prompt,
                AgentRunOptions {
                    session_id,
                    session_metadata,
                    cwd,
                    yes,
                    interactive: true,
                    event_callback: Some(callback),
                },
            )
            .await;
        let _ = tx_done.send(AppEvent::AskDone(result));
    });
}

fn short_id(session_id: &str) -> String {
    if session_id.len() <= 8 {
        return session_id.to_string();
    }
    session_id[..8].to_string()
}

fn summarize_json(value: Option<&Value>) -> String {
    let Some(value) = value else {
        return "{}".to_string();
    };
    let rendered = match value {
        Value::String(text) => text.clone(),
        _ => value.to_string(),
    };
    if rendered.len() <= 120 {
        return rendered;
    }
    format!("{}...", &rendered[..120])
}

fn build_startup_context(cwd: &Path, agent_count: usize) -> TuiStartupContext {
    let repo_root = find_repo_root(cwd).unwrap_or_else(|| cwd.to_path_buf());
    TuiStartupContext {
        custom_instruction_count: usize::from(
            repo_root.join(".github/copilot-instructions.md").is_file(),
        ),
        mcp_server_count: count_mcp_servers(&repo_root),
        skill_count: count_child_directories(&repo_root.join(".github/skills")),
        agent_count: agent_count.max(count_matching_files(
            &repo_root.join(".github/agents"),
            ".agent.md",
        )),
        git_branch: read_git_branch(&repo_root),
        pending_requests: 0,
    }
}

fn find_repo_root(start: &Path) -> Option<PathBuf> {
    start
        .ancestors()
        .find(|path| path.join(".git").exists())
        .map(Path::to_path_buf)
}

fn count_child_directories(path: &Path) -> usize {
    std::fs::read_dir(path)
        .ok()
        .into_iter()
        .flat_map(|entries| entries.filter_map(|entry| entry.ok()))
        .filter(|entry| entry.file_type().is_ok_and(|file_type| file_type.is_dir()))
        .count()
}

fn count_matching_files(path: &Path, suffix: &str) -> usize {
    std::fs::read_dir(path)
        .ok()
        .into_iter()
        .flat_map(|entries| entries.filter_map(|entry| entry.ok()))
        .filter(|entry| entry.file_type().is_ok_and(|file_type| file_type.is_file()))
        .filter(|entry| {
            entry
                .file_name()
                .to_str()
                .is_some_and(|file_name| file_name.ends_with(suffix))
        })
        .count()
}

fn count_mcp_servers(repo_root: &Path) -> usize {
    // Count MCP server JSON definitions under .github/mcp/
    let mcp_dir = repo_root.join(".github").join("mcp");
    let json_count = count_matching_files(&mcp_dir, ".json");
    if json_count > 0 {
        return json_count;
    }
    // Also try .github/mcp-servers/ directory
    let alt_dir = repo_root.join(".github").join("mcp-servers");
    count_matching_files(&alt_dir, ".json")
}

fn read_git_branch(repo_root: &Path) -> Option<String> {
    let git_dir = resolve_git_dir(repo_root)?;
    let head = std::fs::read_to_string(git_dir.join("HEAD")).ok()?;
    parse_git_branch(&head)
}

fn resolve_git_dir(repo_root: &Path) -> Option<PathBuf> {
    let dot_git = repo_root.join(".git");
    if dot_git.is_dir() {
        return Some(dot_git);
    }
    if !dot_git.is_file() {
        return None;
    }
    let pointer = std::fs::read_to_string(dot_git).ok()?;
    let git_dir = pointer.strip_prefix("gitdir: ")?.trim();
    let git_dir = PathBuf::from(git_dir);
    if git_dir.is_absolute() {
        Some(git_dir)
    } else {
        Some(repo_root.join(git_dir))
    }
}

fn parse_git_branch(head: &str) -> Option<String> {
    let trimmed = head.trim();
    if trimmed.is_empty() {
        return None;
    }
    if let Some(reference) = trimmed.strip_prefix("ref: ") {
        return reference.rsplit('/').next().map(str::to_string);
    }
    Some(trimmed.chars().take(7).collect())
}

fn map_io(err: impl std::fmt::Display) -> MosaicError {
    MosaicError::Io(err.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::{
        TuiInputCommand, command_palette_items, command_suggestions, parse_input_command,
    };
    use crate::render::{
        compose_header_line, compose_input_placeholder, compose_shortcuts_line,
        compose_startup_environment_line, compose_startup_location_line, compose_status_line,
    };
    use crate::state::TuiAction;
    use mosaic_core::session::SessionSummary;

    fn state_with_sessions(show_inspector: bool) -> TuiState {
        TuiState::new(
            TuiFocus::Messages,
            show_inspector,
            vec![
                SessionSummary {
                    session_id: "session-a".to_string(),
                    event_count: 2,
                    last_updated: None,
                    created_at: None,
                    title: None,
                    runtime: Some(SessionRuntimeMetadata {
                        agent_id: Some("writer".to_string()),
                        profile_name: "default".to_string(),
                    }),
                },
                SessionSummary {
                    session_id: "session-b".to_string(),
                    event_count: 3,
                    last_updated: None,
                    created_at: None,
                    title: None,
                    runtime: Some(SessionRuntimeMetadata {
                        agent_id: Some("reviewer".to_string()),
                        profile_name: "default".to_string(),
                    }),
                },
            ],
            Some("session-a".to_string()),
            "default",
            Some("writer"),
            "confirm_dangerous",
        )
    }

    #[test]
    fn reducer_cycles_focus_across_visible_panes() {
        let mut state = state_with_sessions(true);
        assert_eq!(state.focus, TuiFocus::Messages);

        state.reduce(TuiAction::CycleFocus);
        assert_eq!(state.focus, TuiFocus::Input);

        state.reduce(TuiAction::CycleFocus);
        assert_eq!(state.focus, TuiFocus::Sessions);

        state.reduce(TuiAction::CycleFocus);
        assert_eq!(state.focus, TuiFocus::Inspector);

        state.reduce(TuiAction::CycleFocus);
        assert_eq!(state.focus, TuiFocus::Messages);
    }

    #[test]
    fn reducer_toggle_inspector_updates_focus() {
        let mut state = state_with_sessions(true);
        state.focus = TuiFocus::Inspector;
        state.reduce(TuiAction::ToggleInspector);

        assert!(!state.show_inspector);
        assert_eq!(state.focus, TuiFocus::Messages);
    }

    #[test]
    fn reducer_toggles_help_overlay() {
        let mut state = state_with_sessions(true);
        assert!(!state.show_help);

        state.reduce(TuiAction::ToggleHelp);
        assert!(state.show_help);

        state.reduce(TuiAction::ToggleHelp);
        assert!(!state.show_help);
    }

    #[test]
    fn reducer_session_navigation_wraps() {
        let mut state = state_with_sessions(true);
        assert_eq!(state.selected_session, 0);

        state.reduce(TuiAction::SelectPrevSession);
        assert_eq!(state.selected_session, 1);

        state.reduce(TuiAction::SelectNextSession);
        assert_eq!(state.selected_session, 0);
    }

    #[test]
    fn agent_event_bridge_populates_inspector_and_messages() {
        let mut state = state_with_sessions(true);

        state.apply_agent_event(AgentEvent::ToolCall {
            session_id: "session-a".to_string(),
            name: "read_file".to_string(),
            args: serde_json::json!({ "path": "README.md" }),
        });
        state.apply_agent_event(AgentEvent::ToolResult {
            session_id: "session-a".to_string(),
            name: "read_file".to_string(),
            result: serde_json::json!({ "content": "ok" }),
        });
        state.apply_agent_event(AgentEvent::Assistant {
            session_id: "session-a".to_string(),
            text: "done".to_string(),
        });

        assert_eq!(state.messages.len(), 1);
        assert_eq!(state.messages[0].role, "assistant");
        assert_eq!(state.inspector.len(), 2);
        assert_eq!(state.inspector[0].kind, "tool_call");
        assert_eq!(state.inspector[1].kind, "tool_result");
    }

    #[test]
    fn local_command_output_populates_transcript_and_inspector() {
        let mut state = state_with_sessions(true);
        state.focus = TuiFocus::Input;

        state.apply_local_command_output(
            TuiLocalCommand::Models,
            TuiLocalCommandOutput {
                title: "Model routing summary".to_string(),
                body: "current model: mock-model".to_string(),
                status: "loaded /models".to_string(),
                inspector_detail: "/models -> current model: mock-model".to_string(),
            },
        );

        assert_eq!(state.messages.len(), 2);
        assert_eq!(state.messages[0].role, "user");
        assert_eq!(state.messages[0].text, "/models");
        assert_eq!(state.messages[1].role, "system");
        assert!(state.messages[1].text.contains("Model routing summary"));
        assert!(state.messages[1].text.contains("current model: mock-model"));
        assert_eq!(
            state.inspector.last().map(|entry| entry.kind.as_str()),
            Some("local_command")
        );
        assert_eq!(state.status, "loaded /models");
        assert!(!state.show_startup_surface());
    }

    #[test]
    fn state_status_includes_policy_summary() {
        let state = state_with_sessions(true);
        let rendered = compose_status_line(
            &state.status,
            "default",
            Some("writer"),
            state.active_session_id.as_deref(),
            "confirm_dangerous",
        );
        assert!(rendered.contains("policy=confirm_dangerous"));
        assert!(rendered.contains("agent=writer"));
        assert!(rendered.contains("session=session-a"));
    }

    #[test]
    fn header_line_mentions_quick_actions() {
        let rendered = compose_header_line(
            false,
            TuiFocus::Input,
            "default",
            Some("writer"),
            "confirm_dangerous",
        );
        assert!(rendered.contains("Mosaic CLI"));
        assert!(rendered.contains("/agent"));
        assert!(rendered.contains("focus=input"));
    }

    #[test]
    fn shortcuts_line_mentions_core_shortcuts() {
        let rendered = compose_shortcuts_line(false, true);
        assert!(rendered.contains("Ctrl+A"));
        assert!(rendered.contains("Ctrl+S"));
        assert!(rendered.contains("Shift+Tab"));
        assert!(rendered.contains("q quit"));
    }

    #[test]
    fn command_palette_filters_slash_commands() {
        let all = command_palette_items("/");
        assert!(!all.is_empty());
        assert!(all.iter().any(|command| command.insert_text == "/help"));
        assert!(all.iter().any(|command| command.insert_text == "/models"));
        assert!(all.iter().any(|command| command.insert_text == "/memory"));
        assert!(all.iter().any(|command| command.insert_text == "/plugins"));

        let agent_only = command_palette_items("/agent writer");
        assert!(
            agent_only
                .iter()
                .any(|command| command.insert_text == "/agent ")
        );
        assert!(
            agent_only
                .iter()
                .any(|command| command.insert_text == "/agents")
        );

        let status_only = command_palette_items("/status");
        assert_eq!(status_only.len(), 1);
        assert_eq!(status_only[0].insert_text, "/status");
    }

    #[test]
    fn command_suggestions_include_agent_and_session_context() {
        let agents = vec![TuiAgentOption {
            id: "writer".to_string(),
            name: "Writer".to_string(),
            profile_name: "default".to_string(),
            is_default: true,
            route_keys: vec!["code".to_string()],
        }];
        let sessions = vec![SessionSummary {
            session_id: "session-a-1234".to_string(),
            event_count: 2,
            last_updated: None,
            created_at: None,
            title: None,
            runtime: Some(SessionRuntimeMetadata {
                agent_id: Some("writer".to_string()),
                profile_name: "default".to_string(),
            }),
        }];

        let agent_matches = command_suggestions("/agent wr", &agents, &sessions, None);
        assert_eq!(agent_matches[0].insert_text, "/agent writer");

        let session_matches = command_suggestions(
            "/session session-a",
            &agents,
            &sessions,
            Some("session-a-1234"),
        );
        assert_eq!(session_matches[0].insert_text, "/session session-a-1234");
        assert!(session_matches[0].description.contains("active"));
    }

    #[test]
    fn command_suggestions_fall_back_when_no_agent_matches_exist() {
        let suggestions = command_suggestions("/agent reviewer", &[], &[], None);
        assert!(!suggestions.is_empty());
        assert!(suggestions.iter().any(|item| item.insert_text == "/agents"));
        assert!(suggestions.iter().any(|item| {
            item.shell_hint.as_deref()
                == Some("mosaic agents add --id <id> --name <name> --model <model>")
        }));
    }

    #[test]
    fn startup_environment_line_pluralizes_counts() {
        assert_eq!(
            compose_startup_environment_line(1, 0, 3, 2),
            "Environment loaded: 1 custom instruction, 3 skills, 2 agents"
        );
        assert_eq!(
            compose_startup_environment_line(2, 0, 1, 1),
            "Environment loaded: 2 custom instructions, 1 skill, 1 agent"
        );
        assert_eq!(
            compose_startup_environment_line(1, 2, 3, 1),
            "Environment loaded: 1 custom instruction, 2 MCP servers, 3 skills, 1 agent"
        );
    }

    #[test]
    fn startup_location_line_includes_branch_when_present() {
        let rendered = compose_startup_location_line("/tmp/project", Some("main"));
        assert_eq!(rendered, "/tmp/project [main]");
    }

    #[test]
    fn startup_placeholder_mentions_copilot_style_affordances() {
        let rendered = compose_input_placeholder(true, false);
        assert!(rendered.contains("@ to mention files"));
        assert!(rendered.contains("# for issues/PRs"));
        assert!(rendered.contains("/ for commands"));
    }

    #[test]
    fn parse_git_branch_handles_ref_and_detached_head() {
        assert_eq!(
            parse_git_branch("ref: refs/heads/main\n"),
            Some("main".to_string())
        );
        assert_eq!(
            parse_git_branch("0123456789abcdef"),
            Some("0123456".to_string())
        );
    }

    #[test]
    fn startup_surface_is_visible_on_launch_even_with_session_history() {
        let mut state = state_with_sessions(true);
        state.focus = TuiFocus::Input;
        assert!(state.show_startup_surface());
    }

    #[test]
    fn agent_events_hide_startup_surface() {
        let mut state = state_with_sessions(true);
        state.apply_agent_event(AgentEvent::Assistant {
            session_id: "session-a".to_string(),
            text: "done".to_string(),
        });
        assert!(!state.show_startup_surface());
    }

    #[test]
    fn parse_input_command_recognizes_agent_switch() {
        assert_eq!(parse_input_command("/help"), Some(TuiInputCommand::Help));
        assert_eq!(parse_input_command("/agent"), Some(TuiInputCommand::Agent));
        assert_eq!(
            parse_input_command("/agents"),
            Some(TuiInputCommand::Agents)
        );
        assert_eq!(
            parse_input_command("/agent writer"),
            Some(TuiInputCommand::AgentSet("writer"))
        );
        assert_eq!(
            parse_input_command("/agent   reviewer"),
            Some(TuiInputCommand::AgentSet("reviewer"))
        );
        assert_eq!(
            parse_input_command("/session"),
            Some(TuiInputCommand::Session)
        );
        assert_eq!(
            parse_input_command("/session abc123"),
            Some(TuiInputCommand::SessionSet("abc123"))
        );
        assert_eq!(
            parse_input_command("/new"),
            Some(TuiInputCommand::NewSession)
        );
        assert_eq!(
            parse_input_command("/clear"),
            Some(TuiInputCommand::NewSession)
        );
        assert_eq!(
            parse_input_command("/status"),
            Some(TuiInputCommand::Status)
        );
        assert_eq!(
            parse_input_command("/models"),
            Some(TuiInputCommand::Models)
        );
        assert_eq!(
            parse_input_command("/skills"),
            Some(TuiInputCommand::Skills)
        );
        assert_eq!(parse_input_command("/docs"), Some(TuiInputCommand::Docs));
        assert_eq!(parse_input_command("/logs"), Some(TuiInputCommand::Logs));
        assert_eq!(
            parse_input_command("/doctor"),
            Some(TuiInputCommand::Doctor)
        );
        assert_eq!(
            parse_input_command("/memory"),
            Some(TuiInputCommand::Memory)
        );
        assert_eq!(
            parse_input_command("/knowledge"),
            Some(TuiInputCommand::Knowledge)
        );
        assert_eq!(
            parse_input_command("/plugins"),
            Some(TuiInputCommand::Plugins)
        );
        assert_eq!(parse_input_command("hello"), None);
    }
}
