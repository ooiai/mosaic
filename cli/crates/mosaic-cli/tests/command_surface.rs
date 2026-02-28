use assert_cmd::Command;

fn run_help(args: &[&str]) -> String {
    #[allow(deprecated)]
    let output = Command::cargo_bin("mosaic")
        .expect("binary")
        .args(args)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    String::from_utf8(output).expect("stdout is utf8")
}

#[test]
#[allow(deprecated)]
fn root_help_includes_expected_commands() {
    let help = run_help(&["--help"]);
    let expected = [
        "setup",
        "configure",
        "models",
        "ask",
        "chat",
        "session",
        "gateway",
        "channels",
        "nodes",
        "devices",
        "pairing",
        "hooks",
        "cron",
        "webhooks",
        "browser",
        "logs",
        "system",
        "approvals",
        "sandbox",
        "memory",
        "security",
        "agents",
        "plugins",
        "skills",
        "completion",
        "directory",
        "dashboard",
        "update",
        "reset",
        "uninstall",
        "docs",
        "dns",
        "tui",
        "qr",
        "clawbot",
        "status",
        "health",
        "doctor",
    ];

    for name in expected {
        assert!(
            help.contains(name),
            "root --help missing expected command: {name}\n{help}"
        );
    }

    let visible_aliases = [
        "onboard", "config", "message", "agent", "sessions", "daemon", "node", "acp",
    ];
    for alias in visible_aliases {
        assert!(
            help.contains(alias),
            "root --help missing expected visible alias: {alias}\n{help}"
        );
    }
}

#[test]
#[allow(deprecated)]
fn channels_help_includes_operational_commands() {
    let help = run_help(&["channels", "--help"]);
    let expected = [
        "add",
        "update",
        "list",
        "status",
        "login",
        "send",
        "test",
        "logs",
        "capabilities",
        "resolve",
        "export",
        "import",
        "rotate-token-env",
        "remove",
        "logout",
    ];

    for name in expected {
        assert!(
            help.contains(name),
            "channels --help missing expected subcommand: {name}\n{help}"
        );
    }
}

#[test]
#[allow(deprecated)]
fn gateway_help_includes_lifecycle_commands() {
    let help = run_help(&["gateway", "--help"]);
    let expected = [
        "install",
        "start",
        "restart",
        "status",
        "health",
        "call",
        "probe",
        "discover",
        "stop",
        "uninstall",
    ];

    for name in expected {
        assert!(
            help.contains(name),
            "gateway --help missing expected subcommand: {name}\n{help}"
        );
    }
}

#[test]
#[allow(deprecated)]
fn models_help_includes_resolution_commands() {
    let help = run_help(&["models", "--help"]);
    let expected = ["list", "status", "resolve", "set", "aliases", "fallbacks"];

    for name in expected {
        assert!(
            help.contains(name),
            "models --help missing expected subcommand: {name}\n{help}"
        );
    }

    let list_help = run_help(&["models", "list", "--help"]);
    for option in ["--query", "--limit"] {
        assert!(
            list_help.contains(option),
            "models list --help missing expected option: {option}\n{list_help}"
        );
    }
}

#[test]
#[allow(deprecated)]
fn ask_help_includes_prompt_file_and_script_options() {
    let help = run_help(&["ask", "--help"]);
    for option in ["--prompt-file", "--script"] {
        assert!(
            help.contains(option),
            "ask --help missing option {option}:\n{help}"
        );
    }
}

#[test]
#[allow(deprecated)]
fn chat_help_includes_prompt_file_and_script_options() {
    let help = run_help(&["chat", "--help"]);
    for option in ["--prompt-file", "--script"] {
        assert!(
            help.contains(option),
            "chat --help missing option {option}:\n{help}"
        );
    }
}

#[test]
#[allow(deprecated)]
fn hooks_help_includes_lifecycle_commands() {
    let help = run_help(&["hooks", "--help"]);
    let expected = ["list", "add", "remove", "enable", "disable", "run", "logs"];

    for name in expected {
        assert!(
            help.contains(name),
            "hooks --help missing expected subcommand: {name}\n{help}"
        );
    }
}

#[test]
#[allow(deprecated)]
fn cron_help_includes_lifecycle_commands() {
    let help = run_help(&["cron", "--help"]);
    let expected = [
        "list", "add", "remove", "enable", "disable", "run", "tick", "logs",
    ];

    for name in expected {
        assert!(
            help.contains(name),
            "cron --help missing expected subcommand: {name}\n{help}"
        );
    }
}

#[test]
#[allow(deprecated)]
fn webhooks_help_includes_lifecycle_commands() {
    let help = run_help(&["webhooks", "--help"]);
    let expected = [
        "list", "add", "remove", "enable", "disable", "trigger", "resolve", "logs",
    ];

    for name in expected {
        assert!(
            help.contains(name),
            "webhooks --help missing expected subcommand: {name}\n{help}"
        );
    }
}

#[test]
#[allow(deprecated)]
fn agents_help_includes_management_commands() {
    let help = run_help(&["agents", "--help"]);
    let expected = [
        "list", "add", "update", "show", "remove", "default", "route",
    ];

    for name in expected {
        assert!(
            help.contains(name),
            "agents --help missing expected subcommand: {name}\n{help}"
        );
    }
}

#[test]
#[allow(deprecated)]
fn nodes_help_includes_control_plane_commands() {
    let help = run_help(&["nodes", "--help"]);
    let expected = ["list", "status", "run", "invoke"];

    for name in expected {
        assert!(
            help.contains(name),
            "nodes --help missing expected subcommand: {name}\n{help}"
        );
    }
}

#[test]
#[allow(deprecated)]
fn devices_help_includes_lifecycle_commands() {
    let help = run_help(&["devices", "--help"]);
    let expected = ["list", "approve", "reject", "rotate", "revoke"];

    for name in expected {
        assert!(
            help.contains(name),
            "devices --help missing expected subcommand: {name}\n{help}"
        );
    }
}

#[test]
#[allow(deprecated)]
fn pairing_help_includes_request_approval_and_reject_commands() {
    let help = run_help(&["pairing", "--help"]);
    let expected = ["list", "request", "approve", "reject"];

    for name in expected {
        assert!(
            help.contains(name),
            "pairing --help missing expected subcommand: {name}\n{help}"
        );
    }
}

#[test]
#[allow(deprecated)]
fn browser_help_includes_navigation_and_history_commands() {
    let help = run_help(&["browser", "--help"]);
    let expected = [
        "start",
        "stop",
        "status",
        "open",
        "visit",
        "navigate",
        "history",
        "tabs",
        "show",
        "focus",
        "snapshot",
        "screenshot",
        "close",
        "clear",
    ];

    for name in expected {
        assert!(
            help.contains(name),
            "browser --help missing expected command or alias: {name}\n{help}"
        );
    }
}

#[test]
#[allow(deprecated)]
fn memory_help_includes_index_and_search_commands() {
    let help = run_help(&["memory", "--help"]);
    let expected = ["index", "search", "status", "clear"];

    for name in expected {
        assert!(
            help.contains(name),
            "memory --help missing expected subcommand: {name}\n{help}"
        );
    }
}

#[test]
#[allow(deprecated)]
fn security_help_includes_audit_and_baseline_commands() {
    let help = run_help(&["security", "--help"]);
    let expected = ["audit", "baseline"];

    for name in expected {
        assert!(
            help.contains(name),
            "security --help missing expected subcommand: {name}\n{help}"
        );
    }
}

#[test]
#[allow(deprecated)]
fn plugins_help_includes_management_commands() {
    let help = run_help(&["plugins", "--help"]);
    let expected = ["list", "info", "check", "install", "remove"];

    for name in expected {
        assert!(
            help.contains(name),
            "plugins --help missing expected subcommand: {name}\n{help}"
        );
    }

    let list_help = run_help(&["plugins", "list", "--help"]);
    assert!(
        list_help.contains("--source"),
        "plugins list --help missing expected option --source:\n{list_help}"
    );
}

#[test]
#[allow(deprecated)]
fn skills_help_includes_management_commands() {
    let help = run_help(&["skills", "--help"]);
    let expected = ["list", "info", "check", "install", "remove"];

    for name in expected {
        assert!(
            help.contains(name),
            "skills --help missing expected subcommand: {name}\n{help}"
        );
    }

    let list_help = run_help(&["skills", "list", "--help"]);
    assert!(
        list_help.contains("--source"),
        "skills list --help missing expected option --source:\n{list_help}"
    );
}

#[test]
#[allow(deprecated)]
fn logs_help_includes_streaming_options() {
    let help = run_help(&["logs", "--help"]);
    let expected = ["--follow", "--tail", "--source"];

    for name in expected {
        assert!(
            help.contains(name),
            "logs --help missing expected option: {name}\n{help}"
        );
    }
}

#[test]
#[allow(deprecated)]
fn system_help_includes_event_and_presence_commands() {
    let help = run_help(&["system", "--help"]);
    let expected = ["event", "presence", "list"];

    for name in expected {
        assert!(
            help.contains(name),
            "system --help missing expected subcommand: {name}\n{help}"
        );
    }
}

#[test]
#[allow(deprecated)]
fn approvals_help_includes_policy_commands() {
    let help = run_help(&["approvals", "--help"]);
    let expected = ["get", "set", "check", "allowlist"];

    for name in expected {
        assert!(
            help.contains(name),
            "approvals --help missing expected subcommand: {name}\n{help}"
        );
    }
}

#[test]
#[allow(deprecated)]
fn approvals_allowlist_help_includes_list_add_remove() {
    let help = run_help(&["approvals", "allowlist", "--help"]);
    let expected = ["list", "add", "remove"];

    for name in expected {
        assert!(
            help.contains(name),
            "approvals allowlist --help missing expected subcommand: {name}\n{help}"
        );
    }
}

#[test]
#[allow(deprecated)]
fn sandbox_help_includes_profile_commands() {
    let help = run_help(&["sandbox", "--help"]);
    let expected = ["get", "set", "check", "list", "explain"];

    for name in expected {
        assert!(
            help.contains(name),
            "sandbox --help missing expected subcommand: {name}\n{help}"
        );
    }
}

#[test]
#[allow(deprecated)]
fn completion_help_includes_shell_and_install_commands() {
    let help = run_help(&["completion", "--help"]);
    let expected = ["shell", "install"];

    for name in expected {
        assert!(
            help.contains(name),
            "completion --help missing expected subcommand: {name}\n{help}"
        );
    }
}

#[test]
#[allow(deprecated)]
fn directory_help_includes_ensure_and_writable_flags() {
    let help = run_help(&["directory", "--help"]);
    for option in ["--ensure", "--check-writable"] {
        assert!(
            help.contains(option),
            "directory --help missing expected option: {option}\n{help}"
        );
    }
}

#[test]
#[allow(deprecated)]
fn dns_help_includes_resolve_command() {
    let help = run_help(&["dns", "--help"]);
    assert!(
        help.contains("resolve"),
        "dns --help missing expected subcommand: resolve\n{help}"
    );
}

#[test]
#[allow(deprecated)]
fn tui_help_includes_prompt_and_session_options() {
    let help = run_help(&["tui", "--help"]);
    let expected = ["--prompt", "--session", "--agent"];
    for name in expected {
        assert!(
            help.contains(name),
            "tui --help missing expected option: {name}\n{help}"
        );
    }
}

#[test]
#[allow(deprecated)]
fn qr_help_includes_encode_and_pairing() {
    let help = run_help(&["qr", "--help"]);
    let expected = ["encode", "pairing"];
    for name in expected {
        assert!(
            help.contains(name),
            "qr --help missing expected subcommand: {name}\n{help}"
        );
    }
}

#[test]
#[allow(deprecated)]
fn clawbot_help_includes_ask_chat_send_status() {
    let help = run_help(&["clawbot", "--help"]);
    let expected = ["ask", "chat", "send", "status"];
    for name in expected {
        assert!(
            help.contains(name),
            "clawbot --help missing expected subcommand: {name}\n{help}"
        );
    }
}

#[test]
#[allow(deprecated)]
fn clawbot_ask_help_includes_prompt_file_and_script_options() {
    let help = run_help(&["clawbot", "ask", "--help"]);
    for option in ["--prompt-file", "--script"] {
        assert!(
            help.contains(option),
            "clawbot ask --help missing option {option}:\n{help}"
        );
    }
}

#[test]
#[allow(deprecated)]
fn clawbot_chat_help_includes_prompt_file_and_script_options() {
    let help = run_help(&["clawbot", "chat", "--help"]);
    for option in ["--prompt-file", "--script"] {
        assert!(
            help.contains(option),
            "clawbot chat --help missing option {option}:\n{help}"
        );
    }
}

#[test]
#[allow(deprecated)]
fn clawbot_send_help_includes_text_file_option() {
    let help = run_help(&["clawbot", "send", "--help"]);
    assert!(
        help.contains("--text-file"),
        "clawbot send --help missing --text-file option:\n{help}"
    );
}
