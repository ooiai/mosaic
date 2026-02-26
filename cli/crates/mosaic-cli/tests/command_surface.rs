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
fn root_help_includes_openclaw_parity_commands() {
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

    let visible_aliases = ["onboard", "message", "agent"];
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
    let expected = ["list", "add", "update", "show", "remove", "default", "route"];

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
fn pairing_help_includes_request_and_approval_commands() {
    let help = run_help(&["pairing", "--help"]);
    let expected = ["list", "request", "approve"];

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
    let expected = ["open", "visit", "history", "show", "clear"];

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
    let expected = ["index", "search", "status"];

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
}

#[test]
#[allow(deprecated)]
fn logs_help_includes_streaming_options() {
    let help = run_help(&["logs", "--help"]);
    let expected = ["--follow", "--tail"];

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
    let expected = ["event", "presence"];

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
    let expected = ["get", "set", "allowlist"];

    for name in expected {
        assert!(
            help.contains(name),
            "approvals --help missing expected subcommand: {name}\n{help}"
        );
    }
}

#[test]
#[allow(deprecated)]
fn sandbox_help_includes_profile_commands() {
    let help = run_help(&["sandbox", "--help"]);
    let expected = ["list", "explain"];

    for name in expected {
        assert!(
            help.contains(name),
            "sandbox --help missing expected subcommand: {name}\n{help}"
        );
    }
}
