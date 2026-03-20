use std::{fs, path::PathBuf};

use anyhow::Result;
use clap::{Parser, Subcommand};
use mosaic_config::load_from_file;
use mosaic_inspect::RunTrace;
use mosaic_runtime::{AgentRuntime, RunRequest};

mod bootstrap;

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
            Some(Commands::Run { file, skill }) => DispatchCommand::Run { file, skill },
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
        DispatchCommand::Run { file, skill } => run_cmd(file, skill).await,
        DispatchCommand::Inspect { file } => inspect_cmd(file),
    }
}

async fn run_cmd(file: PathBuf, skill: Option<String>) -> Result<()> {
    let cfg = load_from_file(&file)?;
    let runtime = AgentRuntime::new(bootstrap::build_runtime_context(&cfg)?);

    let result = runtime
        .run(RunRequest {
            system: cfg.agent.system,
            input: cfg.task.input,
            skill,
        })
        .await?;

    let path = result.trace.save_to_default_dir()?;
    println!("{}", result.output);
    println!("saved trace: {}", path.display());
    Ok(())
}

fn inspect_cmd(file: PathBuf) -> Result<()> {
    let content = fs::read_to_string(file)?;
    let trace: RunTrace = serde_json::from_str(&content)?;

    println!("run_id: {}", trace.run_id);
    println!("started_at: {}", trace.started_at);
    println!("finished_at: {:?}", trace.finished_at);
    println!("input: {}", trace.input);
    println!("output: {:?}", trace.output);
    println!("error: {:?}", trace.error);
    println!("tool_calls: {}", trace.tool_calls.len());
    println!("skill_calls: {}", trace.skill_calls.len());

    Ok(())
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
}
