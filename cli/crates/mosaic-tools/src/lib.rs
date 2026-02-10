use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use walkdir::WalkDir;

use mosaic_core::config::RunGuardMode;
use mosaic_core::error::{MosaicError, Result};
use mosaic_ops::{ApprovalDecision, RuntimePolicy, evaluate_approval, evaluate_sandbox};

const MAX_DEFAULT_SEARCH_RESULTS: usize = 50;

#[derive(Debug, Clone)]
pub struct ToolExecutor {
    guard_mode: RunGuardMode,
    runtime_policy: Option<RuntimePolicy>,
}

#[derive(Debug, Clone)]
pub struct ToolContext {
    pub cwd: PathBuf,
    pub yes: bool,
    pub interactive: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunCommandOutput {
    pub command: String,
    pub cwd: String,
    pub approved_by: String,
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
    pub duration_ms: u128,
}

#[derive(Debug, Clone)]
enum GuardDecision {
    AllowAuto,
    NeedsConfirmation {
        reason: String,
    },
    Blocked {
        reason: String,
        suggestion: Option<String>,
    },
}

#[derive(Debug, Deserialize)]
struct ReadFileArgs {
    path: String,
}

#[derive(Debug, Deserialize)]
struct WriteFileArgs {
    path: String,
    content: String,
}

#[derive(Debug, Deserialize)]
struct SearchTextArgs {
    query: String,
    path: Option<String>,
    max_results: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct RunCommandArgs {
    command: String,
}

impl ToolExecutor {
    pub fn new(guard_mode: RunGuardMode, runtime_policy: Option<RuntimePolicy>) -> Self {
        Self {
            guard_mode,
            runtime_policy,
        }
    }

    pub fn execute(&self, name: &str, args: Value, context: &ToolContext) -> Result<Value> {
        match name {
            "read_file" => self.read_file(args, context),
            "write_file" => self.write_file(args, context),
            "search_text" => self.search_text(args, context),
            "run_cmd" => self.run_cmd(args, context),
            _ => Err(MosaicError::Tool(format!("unknown tool '{name}'"))),
        }
    }

    fn read_file(&self, args: Value, context: &ToolContext) -> Result<Value> {
        let parsed: ReadFileArgs = serde_json::from_value(args)?;
        let path = self.resolve_in_cwd(&context.cwd, &parsed.path)?;
        let content = fs::read_to_string(&path).map_err(|err| {
            MosaicError::Tool(format!("failed to read {}: {err}", path.display()))
        })?;
        Ok(json!({
            "path": path.display().to_string(),
            "content": content,
        }))
    }

    fn write_file(&self, args: Value, context: &ToolContext) -> Result<Value> {
        let parsed: WriteFileArgs = serde_json::from_value(args)?;
        let path = self.resolve_in_cwd(&context.cwd, &parsed.path)?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&path, parsed.content.as_bytes()).map_err(|err| {
            MosaicError::Tool(format!("failed to write {}: {err}", path.display()))
        })?;
        Ok(json!({
            "path": path.display().to_string(),
            "written": true,
        }))
    }

    fn search_text(&self, args: Value, context: &ToolContext) -> Result<Value> {
        let parsed: SearchTextArgs = serde_json::from_value(args)?;
        if parsed.query.trim().is_empty() {
            return Err(MosaicError::Tool(
                "search query cannot be empty".to_string(),
            ));
        }
        let root = match parsed.path {
            Some(path) => self.resolve_in_cwd(&context.cwd, &path)?,
            None => context.cwd.clone(),
        };
        let max_results = parsed.max_results.unwrap_or(MAX_DEFAULT_SEARCH_RESULTS);
        let regex = Regex::new(&parsed.query).ok();
        let mut matches = Vec::new();
        for entry in WalkDir::new(&root).into_iter().flatten() {
            let path = entry.path();
            if !path.is_file() || should_skip(path) {
                continue;
            }
            let content = match fs::read_to_string(path) {
                Ok(value) => value,
                Err(_) => continue,
            };
            for (idx, line) in content.lines().enumerate() {
                let matched = if let Some(re) = &regex {
                    re.is_match(line)
                } else {
                    line.contains(&parsed.query)
                };
                if matched {
                    matches.push(json!({
                        "path": path.display().to_string(),
                        "line_number": idx + 1,
                        "line": line,
                    }));
                    if matches.len() >= max_results {
                        return Ok(json!({ "matches": matches, "truncated": true }));
                    }
                }
            }
        }
        Ok(json!({ "matches": matches, "truncated": false }))
    }

    fn run_cmd(&self, args: Value, context: &ToolContext) -> Result<Value> {
        let parsed: RunCommandArgs = serde_json::from_value(args)?;
        let decision = self.classify_command(&parsed.command);
        let mut confirmation_reasons = Vec::new();
        let mut auto_approved_by: Option<String> = None;

        if let Some(runtime_policy) = &self.runtime_policy {
            if let Some(reason) = evaluate_sandbox(&parsed.command, runtime_policy.sandbox.profile)
            {
                return Err(MosaicError::SandboxDenied(reason));
            }

            match evaluate_approval(&parsed.command, &runtime_policy.approval) {
                ApprovalDecision::Auto { approved_by } => {
                    auto_approved_by = Some(approved_by);
                }
                ApprovalDecision::NeedsConfirmation { reason } => {
                    confirmation_reasons.push(reason);
                }
                ApprovalDecision::Deny { reason } => {
                    return Err(MosaicError::ApprovalRequired(reason));
                }
            }
        }

        let approved_by = match decision {
            GuardDecision::AllowAuto => {
                if confirmation_reasons.is_empty() {
                    auto_approved_by.unwrap_or_else(|| "auto_safe".to_string())
                } else if context.yes {
                    "flag_yes".to_string()
                } else if context.interactive {
                    let reason = confirmation_reasons.join("; ");
                    if confirm_command(&parsed.command, &reason)? {
                        "user_prompt".to_string()
                    } else {
                        return Err(MosaicError::ApprovalRequired(
                            "command execution cancelled by user".to_string(),
                        ));
                    }
                } else {
                    let reason = confirmation_reasons.join("; ");
                    return Err(MosaicError::ApprovalRequired(format!(
                        "command requires approval: {reason}. rerun with --yes"
                    )));
                }
            }
            GuardDecision::NeedsConfirmation { reason } => {
                confirmation_reasons.push(reason);
                let reason = confirmation_reasons.join("; ");
                if context.yes {
                    "flag_yes".to_string()
                } else if context.interactive {
                    if confirm_command(&parsed.command, &reason)? {
                        "user_prompt".to_string()
                    } else {
                        return Err(MosaicError::ApprovalRequired(
                            "command execution cancelled by user".to_string(),
                        ));
                    }
                } else {
                    return Err(MosaicError::ApprovalRequired(format!(
                        "command requires approval: {reason}. rerun with --yes"
                    )));
                }
            }
            GuardDecision::Blocked { reason, suggestion } => {
                let suffix = suggestion
                    .map(|value| format!(" suggestion: {value}"))
                    .unwrap_or_default();
                return Err(MosaicError::Tool(format!(
                    "blocked command '{}': {reason}.{suffix}",
                    parsed.command
                )));
            }
        };

        let shell = std::env::var("SHELL").unwrap_or_else(|_| "zsh".to_string());
        let started = Instant::now();
        let output = Command::new(shell)
            .arg("-lc")
            .arg(&parsed.command)
            .current_dir(&context.cwd)
            .output()
            .map_err(|err| MosaicError::Tool(format!("failed to execute command: {err}")))?;
        let elapsed = started.elapsed().as_millis();
        let exit_code = output.status.code().unwrap_or(-1);
        let result = RunCommandOutput {
            command: parsed.command,
            cwd: context.cwd.display().to_string(),
            approved_by,
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
            exit_code,
            duration_ms: elapsed,
        };
        Ok(serde_json::to_value(result)?)
    }

    fn classify_command(&self, command: &str) -> GuardDecision {
        let cmd = command.trim().to_lowercase();
        let blocked_patterns = [
            "rm -rf /",
            "mkfs",
            "shutdown",
            "reboot",
            "dd if=",
            "git reset --hard",
            "git clean -fd",
            ":(){:|:&};:",
        ];
        if blocked_patterns.iter().any(|pattern| cmd.contains(pattern)) {
            return GuardDecision::Blocked {
                reason: "high-risk destructive command".to_string(),
                suggestion: Some(
                    "use safer scoped commands and review the target path".to_string(),
                ),
            };
        }

        match self.guard_mode {
            RunGuardMode::Unrestricted => GuardDecision::AllowAuto,
            RunGuardMode::AllConfirm => GuardDecision::NeedsConfirmation {
                reason: "all commands require confirmation in this profile".to_string(),
            },
            RunGuardMode::ConfirmDangerous => {
                if is_safe_read_command(&cmd) {
                    GuardDecision::AllowAuto
                } else if is_dangerous_or_mutating(&cmd) {
                    GuardDecision::NeedsConfirmation {
                        reason: "detected network/write/system-impacting operation".to_string(),
                    }
                } else {
                    GuardDecision::NeedsConfirmation {
                        reason: "unknown command safety".to_string(),
                    }
                }
            }
        }
    }

    fn resolve_in_cwd(&self, cwd: &Path, user_path: &str) -> Result<PathBuf> {
        let path = PathBuf::from(user_path);
        let absolute = if path.is_absolute() {
            path
        } else {
            cwd.join(path)
        };
        ensure_within(cwd, &absolute)?;
        Ok(absolute)
    }
}

fn should_skip(path: &Path) -> bool {
    let text = path.display().to_string();
    text.contains("/.git/")
        || text.contains("/target/")
        || text.contains("/node_modules/")
        || text.contains("/.pnpm-store/")
}

fn ensure_within(cwd: &Path, path: &Path) -> Result<()> {
    let cwd = cwd.canonicalize().map_err(|err| {
        MosaicError::Tool(format!(
            "failed to resolve current working directory {}: {err}",
            cwd.display()
        ))
    })?;

    let candidate = canonicalize_virtual(path)?;

    if candidate.starts_with(&cwd) {
        Ok(())
    } else {
        Err(MosaicError::Tool(format!(
            "path {} is outside workspace {}",
            candidate.display(),
            cwd.display()
        )))
    }
}

fn canonicalize_virtual(path: &Path) -> Result<PathBuf> {
    if path.exists() {
        return path.canonicalize().map_err(|err| {
            MosaicError::Tool(format!("failed to resolve {}: {err}", path.display()))
        });
    }

    let mut anchor = path;
    while !anchor.exists() {
        anchor = anchor.parent().ok_or_else(|| {
            MosaicError::Tool(format!(
                "failed to resolve {}: no existing parent directory",
                path.display()
            ))
        })?;
    }
    let anchored = anchor.canonicalize().map_err(|err| {
        MosaicError::Tool(format!("failed to resolve {}: {err}", anchor.display()))
    })?;
    let suffix = path.strip_prefix(anchor).map_err(|err| {
        MosaicError::Tool(format!(
            "failed to resolve relative path from {}: {err}",
            path.display()
        ))
    })?;
    Ok(anchored.join(suffix))
}

fn confirm_command(command: &str, reason: &str) -> Result<bool> {
    print!("Command requires confirmation ({reason}): `{command}`. Continue? [y/N]: ");
    io::stdout()
        .flush()
        .map_err(|err| MosaicError::Tool(err.to_string()))?;
    let mut buf = String::new();
    io::stdin()
        .read_line(&mut buf)
        .map_err(|err| MosaicError::Tool(err.to_string()))?;
    let choice = buf.trim().to_lowercase();
    Ok(choice == "y" || choice == "yes")
}

fn is_safe_read_command(command: &str) -> bool {
    let prefixes = [
        "ls", "pwd", "cat ", "head ", "tail ", "wc ", "date", "echo ", "rg ", "find ",
    ];
    prefixes
        .iter()
        .any(|prefix| command == *prefix || command.starts_with(prefix))
}

fn is_dangerous_or_mutating(command: &str) -> bool {
    let patterns = [
        "rm ",
        "mv ",
        "cp ",
        "curl ",
        "wget ",
        "ssh ",
        "scp ",
        "chmod ",
        "chown ",
        "sudo ",
        "git push",
        "git commit",
        "git reset",
        "cargo publish",
        "npm publish",
        ">",
        ">>",
    ];
    patterns.iter().any(|pattern| command.contains(pattern))
}

#[cfg(test)]
mod tests {
    use mosaic_ops::{ApprovalMode, ApprovalPolicy, RuntimePolicy, SandboxPolicy, SandboxProfile};

    use super::*;
    use tempfile::tempdir;

    #[test]
    fn read_write_search_tool_flow() {
        let temp = tempdir().unwrap();
        let executor = ToolExecutor::new(RunGuardMode::ConfirmDangerous, None);
        let ctx = ToolContext {
            cwd: temp.path().to_path_buf(),
            yes: true,
            interactive: false,
        };

        executor
            .execute(
                "write_file",
                json!({"path":"notes/hello.txt","content":"rust cli rocks"}),
                &ctx,
            )
            .unwrap();
        let read = executor
            .execute("read_file", json!({"path":"notes/hello.txt"}), &ctx)
            .unwrap();
        assert_eq!(read["content"], "rust cli rocks");

        let found = executor
            .execute(
                "search_text",
                json!({"query":"rust","path":"notes","max_results":10}),
                &ctx,
            )
            .unwrap();
        assert_eq!(found["matches"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn blocks_high_risk_command() {
        let temp = tempdir().unwrap();
        let executor = ToolExecutor::new(RunGuardMode::ConfirmDangerous, None);
        let ctx = ToolContext {
            cwd: temp.path().to_path_buf(),
            yes: true,
            interactive: false,
        };

        let err = executor
            .execute("run_cmd", json!({"command":"git reset --hard"}), &ctx)
            .unwrap_err();
        assert!(err.to_string().contains("blocked command"));
    }

    #[test]
    fn run_command_executes_when_yes_is_set() {
        let temp = tempdir().unwrap();
        let executor = ToolExecutor::new(RunGuardMode::ConfirmDangerous, None);
        let ctx = ToolContext {
            cwd: temp.path().to_path_buf(),
            yes: true,
            interactive: false,
        };

        let result = executor
            .execute("run_cmd", json!({"command":"echo cli-test"}), &ctx)
            .unwrap();
        assert_eq!(result["exit_code"], 0);
        assert!(
            result["stdout"]
                .as_str()
                .unwrap_or_default()
                .contains("cli-test")
        );
    }

    #[test]
    fn approval_policy_deny_blocks_run_command() {
        let temp = tempdir().unwrap();
        let mut approval = ApprovalPolicy::default();
        approval.mode = ApprovalMode::Deny;
        let policy = RuntimePolicy {
            approval,
            sandbox: SandboxPolicy::default(),
        };
        let executor = ToolExecutor::new(RunGuardMode::ConfirmDangerous, Some(policy));
        let ctx = ToolContext {
            cwd: temp.path().to_path_buf(),
            yes: true,
            interactive: false,
        };

        let err = executor
            .execute("run_cmd", json!({"command":"echo blocked"}), &ctx)
            .unwrap_err();
        assert!(matches!(err, MosaicError::ApprovalRequired(_)));
    }

    #[test]
    fn sandbox_restricted_blocks_network_command() {
        let temp = tempdir().unwrap();
        let mut sandbox = SandboxPolicy::default();
        sandbox.profile = SandboxProfile::Restricted;
        let policy = RuntimePolicy {
            approval: ApprovalPolicy::default(),
            sandbox,
        };
        let executor = ToolExecutor::new(RunGuardMode::ConfirmDangerous, Some(policy));
        let ctx = ToolContext {
            cwd: temp.path().to_path_buf(),
            yes: true,
            interactive: false,
        };

        let err = executor
            .execute(
                "run_cmd",
                json!({"command":"curl https://example.com"}),
                &ctx,
            )
            .unwrap_err();
        assert!(matches!(err, MosaicError::SandboxDenied(_)));
    }
}
