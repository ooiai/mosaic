use std::io::{self, IsTerminal, Read};

use mosaic_agent::AgentRunOptions;
use mosaic_core::config::RunGuardMode;
use mosaic_core::error::{MosaicError, Result};
use mosaic_tui::{TuiFocus, TuiOptions, TuiRuntime, run_tui};
use serde_json::json;

use crate::runtime_context::build_runtime;
use crate::utils::print_json;
use crate::{Cli, TuiArgs, TuiFocusArg};

pub(super) async fn handle_tui(cli: &Cli, args: TuiArgs) -> Result<()> {
    let has_prompt = args.prompt.is_some();
    let is_tty = io::stdin().is_terminal() && io::stdout().is_terminal();
    let interactive_mode = is_tty && !has_prompt;

    if interactive_mode {
        if cli.json {
            return Err(MosaicError::Validation(
                "--json is only supported with non-interactive `mosaic tui --prompt ...`"
                    .to_string(),
            ));
        }

        let runtime = build_runtime(cli, args.agent.as_deref(), Some("tui"))?;
        let policy_summary = format!(
            "tools={} guard={}",
            if runtime.agent.profile().tools.enabled {
                "on"
            } else {
                "off"
            },
            guard_mode_label(&runtime.agent.profile().tools.run.guard_mode)
        );
        let cwd = std::env::current_dir().map_err(|err| MosaicError::Io(err.to_string()))?;
        run_tui(
            TuiRuntime {
                agent: runtime.agent,
                profile_name: runtime.active_profile_name,
                agent_id: runtime.active_agent_id,
                policy_summary,
            },
            TuiOptions {
                initial_session_id: args.session,
                initial_focus: map_focus(args.focus),
                show_inspector: !args.no_inspector,
                yes: cli.yes,
                cwd,
            },
        )
        .await?;
        return Ok(());
    }

    let prompt = resolve_tui_prompt(args.prompt)?;
    let prompt = prompt.ok_or_else(|| {
        MosaicError::Validation(
            "non-interactive tui mode requires --prompt <text> (or run in a TTY for fullscreen)"
                .to_string(),
        )
    })?;

    let runtime = build_runtime(cli, args.agent.as_deref(), Some("tui"))?;
    let result = runtime
        .agent
        .ask(
            &prompt,
            AgentRunOptions {
                session_id: args.session,
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

fn resolve_tui_prompt(prompt: Option<String>) -> Result<Option<String>> {
    let Some(prompt) = prompt else {
        return Ok(None);
    };

    if prompt != "-" {
        if prompt.trim().is_empty() {
            return Err(MosaicError::Validation(
                "prompt cannot be empty".to_string(),
            ));
        }
        return Ok(Some(prompt));
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
    Ok(Some(trimmed.to_string()))
}

fn map_focus(focus: TuiFocusArg) -> TuiFocus {
    match focus {
        TuiFocusArg::Messages => TuiFocus::Messages,
        TuiFocusArg::Input => TuiFocus::Input,
        TuiFocusArg::Sessions => TuiFocus::Sessions,
        TuiFocusArg::Inspector => TuiFocus::Inspector,
    }
}

fn guard_mode_label(mode: &RunGuardMode) -> &'static str {
    match mode {
        RunGuardMode::ConfirmDangerous => "confirm_dangerous",
        RunGuardMode::AllConfirm => "all_confirm",
        RunGuardMode::Unrestricted => "unrestricted",
    }
}
