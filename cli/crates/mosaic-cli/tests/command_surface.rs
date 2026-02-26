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
