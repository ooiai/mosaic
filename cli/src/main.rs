use std::{fs, path::PathBuf};

use anyhow::Result;
use clap::{Parser, Subcommand};
use mosaic_config::load_from_file;
use mosaic_inspect::RunTrace;
use mosaic_runtime::{AgentRuntime, RunError, RunRequest, RunResult};

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
        #[arg(long, help = "Show the TUI while the run executes")]
        tui: bool,
    },
    Inspect {
        file: PathBuf,
    },
    Tui,
}

#[derive(Debug, PartialEq, Eq)]
enum DispatchCommand {
    Tui {
        resume: bool,
    },
    Run {
        file: PathBuf,
        skill: Option<String>,
        tui: bool,
        resume: bool,
    },
    Inspect {
        file: PathBuf,
    },
}

impl Cli {
    fn dispatch(self) -> DispatchCommand {
        match self.command {
            None | Some(Commands::Tui) => DispatchCommand::Tui {
                resume: self.resume,
            },
            Some(Commands::Run { file, skill, tui }) => DispatchCommand::Run {
                file,
                skill,
                tui,
                resume: self.resume,
            },
            Some(Commands::Inspect { file }) => DispatchCommand::Inspect { file },
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    match Cli::parse().dispatch() {
        DispatchCommand::Tui { resume } => {
            mosaic_tui::run(resume)?;
            Ok(())
        }
        DispatchCommand::Run {
            file,
            skill,
            tui,
            resume,
        } => run_cmd(file, skill, tui, resume).await,
        DispatchCommand::Inspect { file } => inspect_cmd(file),
    }
}

async fn run_cmd(file: PathBuf, skill: Option<String>, tui: bool, resume: bool) -> Result<()> {
    let cfg = load_from_file(&file)?;

    if tui {
        return run_cmd_with_tui(cfg, skill, resume).await;
    }

    let sinks = bootstrap::build_cli_only_sinks();
    let runtime = AgentRuntime::new(bootstrap::build_runtime_context(&cfg, sinks.event_sink)?);
    let outcome = runtime
        .run(RunRequest {
            system: cfg.agent.system,
            input: cfg.task.input,
            skill,
        })
        .await;

    finish_run_outcome(outcome)
}

async fn run_cmd_with_tui(
    cfg: mosaic_config::AppConfig,
    skill: Option<String>,
    resume: bool,
) -> Result<()> {
    let sinks = bootstrap::build_cli_and_tui_sinks();
    let event_buffer = sinks
        .tui_buffer
        .expect("cli+tui sink builder should provide a TUI buffer");
    let runtime = AgentRuntime::new(bootstrap::build_runtime_context(&cfg, sinks.event_sink)?);

    let request = RunRequest {
        system: cfg.agent.system,
        input: cfg.task.input,
        skill,
    };

    let runtime_handle = tokio::spawn(async move { runtime.run(request).await });
    let tui_handle = tokio::task::spawn_blocking(move || {
        mosaic_tui::run_until_complete_with_event_buffer(resume, event_buffer)
    });

    let runtime_outcome = runtime_handle.await?;
    let tui_join = tui_handle.await;

    let run_result = finish_run_outcome(runtime_outcome);

    match (run_result, tui_join) {
        (Err(err), _) => Err(err),
        (Ok(()), Err(err)) => Err(err.into()),
        (Ok(()), Ok(tui_result)) => {
            tui_result?;
            Ok(())
        }
    }
}

fn finish_run_outcome(outcome: std::result::Result<RunResult, RunError>) -> Result<()> {
    match outcome {
        Ok(result) => persist_successful_run(result),
        Err(err) => persist_failed_run(err),
    }
}

fn persist_successful_run(result: RunResult) -> Result<()> {
    let path = result.trace.save_to_default_dir()?;
    println!("{}", result.output);
    println!("saved trace: {}", path.display());
    Ok(())
}

fn persist_failed_run(err: RunError) -> Result<()> {
    let (source, trace) = err.into_parts();
    let path = trace.save_to_default_dir()?;

    println!("saved trace: {}", path.display());

    Err(source)
}

fn inspect_cmd(file: PathBuf) -> Result<()> {
    let content = fs::read_to_string(file)?;
    let trace: RunTrace = serde_json::from_str(&content)?;
    let summary = trace.summary();

    println!("run_id: {}", trace.run_id);
    println!("status: {}", summary.status);
    println!("started_at: {}", trace.started_at);
    println!("finished_at: {:?}", trace.finished_at);
    println!("duration_ms: {:?}", summary.duration_ms);
    println!("input: {}", trace.input);
    println!("output: {:?}", trace.output);
    println!("error: {:?}", trace.error);

    println!("\nsummary:");
    println!("  tool_calls: {}", summary.tool_calls);
    println!("  skill_calls: {}", summary.skill_calls);

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

    use super::{Cli, DispatchCommand};

    #[test]
    fn defaults_to_tui_when_no_subcommand_is_present() {
        let cli = Cli::parse_from(["mosaic"]);

        assert_eq!(cli.dispatch(), DispatchCommand::Tui { resume: false });
    }

    #[test]
    fn accepts_resume_flag_without_forcing_a_subcommand() {
        let cli = Cli::parse_from(["mosaic", "--resume"]);

        assert_eq!(cli.dispatch(), DispatchCommand::Tui { resume: true });
    }

    #[test]
    fn parses_tui_subcommand_with_resume_flag() {
        let cli = Cli::parse_from(["mosaic", "tui", "--resume"]);

        assert_eq!(cli.dispatch(), DispatchCommand::Tui { resume: true });
    }

    #[test]
    fn parses_run_subcommand() {
        let cli = Cli::parse_from([
            "mosaic",
            "run",
            "examples/basic-agent.yaml",
            "--skill",
            "summarize",
        ]);

        assert_eq!(
            cli.dispatch(),
            DispatchCommand::Run {
                file: "examples/basic-agent.yaml".into(),
                skill: Some("summarize".to_owned()),
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
