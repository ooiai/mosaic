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
        "mcp",
        "channels",
        "nodes",
        "devices",
        "pairing",
        "hooks",
        "cron",
        "webhooks",
        "tts",
        "voicecall",
        "browser",
        "logs",
        "observability",
        "system",
        "approvals",
        "sandbox",
        "safety",
        "memory",
        "knowledge",
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
fn knowledge_help_includes_runtime_and_dataset_commands() {
    let help = run_help(&["knowledge", "--help"]);
    for name in ["ingest", "search", "ask", "evaluate", "datasets"] {
        assert!(
            help.contains(name),
            "knowledge --help missing expected subcommand: {name}\n{help}"
        );
    }

    let ingest_help = run_help(&["knowledge", "ingest", "--help"]);
    for option in [
        "--source",
        "--path",
        "--url",
        "--url-file",
        "--continue-on-error",
        "--report-out",
        "--header",
        "--header-env",
        "--http-retries",
        "--http-retry-backoff-ms",
        "--mcp-server",
        "--namespace",
    ] {
        assert!(
            ingest_help.contains(option),
            "knowledge ingest --help missing expected option: {option}\n{ingest_help}"
        );
    }

    let ask_help = run_help(&["knowledge", "ask", "--help"]);
    for option in ["--min-score", "--references-only"] {
        assert!(
            ask_help.contains(option),
            "knowledge ask --help missing expected option: {option}\n{ask_help}"
        );
    }

    let search_help = run_help(&["knowledge", "search", "--help"]);
    assert!(
        search_help.contains("--min-score"),
        "knowledge search --help missing expected option: --min-score\n{search_help}"
    );

    let evaluate_help = run_help(&["knowledge", "evaluate", "--help"]);
    for option in [
        "--query",
        "--query-file",
        "--namespace",
        "--top-k",
        "--min-score",
        "--baseline",
        "--no-baseline",
        "--update-baseline",
        "--fail-on-regression",
        "--max-coverage-drop",
        "--max-avg-top-score-drop",
        "--history-window",
        "--report-out",
    ] {
        assert!(
            evaluate_help.contains(option),
            "knowledge evaluate --help missing expected option: {option}\n{evaluate_help}"
        );
    }

    let datasets_help = run_help(&["knowledge", "datasets", "--help"]);
    for name in ["list", "remove"] {
        assert!(
            datasets_help.contains(name),
            "knowledge datasets --help missing expected subcommand: {name}\n{datasets_help}"
        );
    }

    let datasets_list_help = run_help(&["knowledge", "datasets", "list", "--help"]);
    assert!(
        datasets_list_help.contains("--namespace"),
        "knowledge datasets list --help missing expected option: --namespace\n{datasets_list_help}"
    );

    let datasets_remove_help = run_help(&["knowledge", "datasets", "remove", "--help"]);
    assert!(
        datasets_remove_help.contains("--dry-run"),
        "knowledge datasets remove --help missing expected option: --dry-run\n{datasets_remove_help}"
    );
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
        "replay",
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

    let logs_help = run_help(&["channels", "logs", "--help"]);
    assert!(
        logs_help.contains("--summary"),
        "channels logs --help missing expected option --summary:\n{logs_help}"
    );

    let replay_help = run_help(&["channels", "replay", "--help"]);
    for option in [
        "--tail",
        "--since-minutes",
        "--limit",
        "--batch-size",
        "--min-attempt",
        "--http-status",
        "--include-non-retryable",
        "--reason",
        "--apply",
        "--max-apply",
        "--require-full-payload",
        "--stop-on-error",
        "--report-out",
        "--token-env",
    ] {
        assert!(
            replay_help.contains(option),
            "channels replay --help missing expected option: {option}\n{replay_help}"
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
        "diagnose",
        "stop",
        "uninstall",
    ];

    for name in expected {
        assert!(
            help.contains(name),
            "gateway --help missing expected subcommand: {name}\n{help}"
        );
    }

    let health_help = run_help(&["gateway", "health", "--help"]);
    for option in ["--verbose", "--repair"] {
        assert!(
            health_help.contains(option),
            "gateway health --help missing expected option {option}:\n{health_help}"
        );
    }
}

#[test]
#[allow(deprecated)]
fn mcp_help_includes_management_commands() {
    let help = run_help(&["mcp", "--help"]);
    let expected = [
        "list", "add", "update", "show", "check", "diagnose", "repair", "enable", "disable",
        "remove",
    ];

    for name in expected {
        assert!(
            help.contains(name),
            "mcp --help missing expected subcommand: {name}\n{help}"
        );
    }

    let add_help = run_help(&["mcp", "add", "--help"]);
    for option in ["--arg", "--env", "--env-from", "--cwd", "--disabled"] {
        assert!(
            add_help.contains(option),
            "mcp add --help missing expected option {option}:\n{add_help}"
        );
    }

    let update_help = run_help(&["mcp", "update", "--help"]);
    for option in [
        "--name",
        "--command",
        "--arg",
        "--clear-args",
        "--env",
        "--clear-env",
        "--env-from",
        "--clear-env-from",
        "--cwd",
        "--clear-cwd",
        "--enable",
        "--disable",
    ] {
        assert!(
            update_help.contains(option),
            "mcp update --help missing expected option {option}:\n{update_help}"
        );
    }

    let check_help = run_help(&["mcp", "check", "--help"]);
    for option in ["--all", "--deep", "--timeout-ms", "--report-out"] {
        assert!(
            check_help.contains(option),
            "mcp check --help missing expected option {option}:\n{check_help}"
        );
    }

    let diagnose_help = run_help(&["mcp", "diagnose", "--help"]);
    for option in ["--timeout-ms", "--report-out"] {
        assert!(
            diagnose_help.contains(option),
            "mcp diagnose --help missing expected option {option}:\n{diagnose_help}"
        );
    }

    let repair_help = run_help(&["mcp", "repair", "--help"]);
    for option in [
        "--all",
        "--timeout-ms",
        "--clear-missing-cwd",
        "--set-env-from",
        "--report-out",
    ] {
        assert!(
            repair_help.contains(option),
            "mcp repair --help missing expected option {option}:\n{repair_help}"
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
fn configure_help_includes_keys_get_set_unset_patch_preview_template_commands() {
    let help = run_help(&["configure", "--help"]);
    for token in [
        "--show",
        "--base-url",
        "keys",
        "get",
        "set",
        "unset",
        "patch",
        "preview",
        "template",
    ] {
        assert!(
            help.contains(token),
            "configure --help missing expected token: {token}\n{help}"
        );
    }

    let patch_help = run_help(&["configure", "patch", "--help"]);
    for option in ["--set", "--file", "--target-profile", "--dry-run"] {
        assert!(
            patch_help.contains(option),
            "configure patch --help missing expected option: {option}\n{patch_help}"
        );
    }

    let preview_help = run_help(&["configure", "preview", "--help"]);
    for option in ["--set", "--file", "--target-profile"] {
        assert!(
            preview_help.contains(option),
            "configure preview --help missing expected option: {option}\n{preview_help}"
        );
    }

    let template_help = run_help(&["configure", "template", "--help"]);
    for option in ["--format", "--defaults", "--target-profile"] {
        assert!(
            template_help.contains(option),
            "configure template --help missing expected option: {option}\n{template_help}"
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
    let expected = [
        "list", "add", "remove", "enable", "disable", "run", "logs", "replay",
    ];

    for name in expected {
        assert!(
            help.contains(name),
            "hooks --help missing expected subcommand: {name}\n{help}"
        );
    }
}

#[test]
#[allow(deprecated)]
fn hooks_logs_help_includes_summary_and_time_window_options() {
    let help = run_help(&["hooks", "logs", "--help"]);
    for option in ["--summary", "--since-minutes"] {
        assert!(
            help.contains(option),
            "hooks logs --help missing expected option: {option}\n{help}"
        );
    }
}

#[test]
#[allow(deprecated)]
fn hooks_replay_help_includes_apply_options() {
    let help = run_help(&["hooks", "replay", "--help"]);
    for option in [
        "--apply",
        "--stop-on-error",
        "--limit",
        "--batch-size",
        "--since-minutes",
        "--reason",
        "--retryable-only",
        "--max-apply",
        "--report-out",
    ] {
        assert!(
            help.contains(option),
            "hooks replay --help missing expected option: {option}\n{help}"
        );
    }
}

#[test]
#[allow(deprecated)]
fn cron_help_includes_lifecycle_commands() {
    let help = run_help(&["cron", "--help"]);
    let expected = [
        "list", "add", "remove", "enable", "disable", "run", "tick", "logs", "replay",
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
fn cron_logs_help_includes_summary_and_time_window_options() {
    let help = run_help(&["cron", "logs", "--help"]);
    for option in ["--summary", "--since-minutes"] {
        assert!(
            help.contains(option),
            "cron logs --help missing expected option: {option}\n{help}"
        );
    }
}

#[test]
#[allow(deprecated)]
fn cron_replay_help_includes_apply_options() {
    let help = run_help(&["cron", "replay", "--help"]);
    for option in [
        "--apply",
        "--stop-on-error",
        "--limit",
        "--batch-size",
        "--since-minutes",
        "--reason",
        "--retryable-only",
        "--max-apply",
        "--report-out",
    ] {
        assert!(
            help.contains(option),
            "cron replay --help missing expected option: {option}\n{help}"
        );
    }
}

#[test]
#[allow(deprecated)]
fn webhooks_help_includes_lifecycle_commands() {
    let help = run_help(&["webhooks", "--help"]);
    let expected = [
        "list", "add", "remove", "enable", "disable", "trigger", "resolve", "logs", "replay",
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
fn webhooks_logs_help_includes_summary_and_time_window_options() {
    let help = run_help(&["webhooks", "logs", "--help"]);
    for option in ["--summary", "--since-minutes"] {
        assert!(
            help.contains(option),
            "webhooks logs --help missing expected option: {option}\n{help}"
        );
    }
}

#[test]
#[allow(deprecated)]
fn webhooks_replay_help_includes_apply_options() {
    let help = run_help(&["webhooks", "replay", "--help"]);
    for option in [
        "--apply",
        "--stop-on-error",
        "--limit",
        "--batch-size",
        "--since-minutes",
        "--reason",
        "--retryable-only",
        "--secret",
        "--max-apply",
        "--report-out",
    ] {
        assert!(
            help.contains(option),
            "webhooks replay --help missing expected option: {option}\n{help}"
        );
    }
}

#[test]
#[allow(deprecated)]
fn tts_help_includes_voices_and_speak_commands() {
    let help = run_help(&["tts", "--help"]);
    let expected = ["voices", "speak", "diagnose"];

    for name in expected {
        assert!(
            help.contains(name),
            "tts --help missing expected subcommand: {name}\n{help}"
        );
    }

    let diagnose_help = run_help(&["tts", "diagnose", "--help"]);
    for option in [
        "--voice",
        "--format",
        "--text",
        "--out",
        "--timeout-ms",
        "--report-out",
    ] {
        assert!(
            diagnose_help.contains(option),
            "tts diagnose --help missing expected option: {option}\n{diagnose_help}"
        );
    }
}

#[test]
#[allow(deprecated)]
fn voicecall_help_includes_start_status_send_history_stop_commands() {
    let help = run_help(&["voicecall", "--help"]);
    let expected = ["start", "status", "send", "history", "stop"];

    for name in expected {
        assert!(
            help.contains(name),
            "voicecall --help missing expected subcommand: {name}\n{help}"
        );
    }

    let send_help = run_help(&["voicecall", "send", "--help"]);
    for option in ["--parse-mode", "--token-env"] {
        assert!(
            send_help.contains(option),
            "voicecall send --help missing expected option: {option}\n{send_help}"
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

    let add_help = run_help(&["agents", "add", "--help"]);
    assert!(
        add_help.contains("--skill"),
        "agents add --help missing expected option --skill:\n{add_help}"
    );

    let update_help = run_help(&["agents", "update", "--help"]);
    assert!(
        update_help.contains("--skill"),
        "agents update --help missing expected option --skill:\n{update_help}"
    );
    assert!(
        update_help.contains("--clear-skills"),
        "agents update --help missing expected option --clear-skills:\n{update_help}"
    );
}

#[test]
#[allow(deprecated)]
fn nodes_help_includes_control_plane_commands() {
    let help = run_help(&["nodes", "--help"]);
    let expected = ["list", "status", "diagnose", "run", "invoke"];

    for name in expected {
        assert!(
            help.contains(name),
            "nodes --help missing expected subcommand: {name}\n{help}"
        );
    }
}

#[test]
#[allow(deprecated)]
fn nodes_diagnose_help_includes_repair_and_stale_threshold() {
    let help = run_help(&["nodes", "diagnose", "--help"]);
    let expected = ["--stale-after-minutes", "--repair"];
    for option in expected {
        assert!(
            help.contains(option),
            "nodes diagnose --help missing expected option: {option}\n{help}"
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
        "diagnose",
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
fn browser_diagnose_help_includes_repair_and_stale_threshold() {
    let help = run_help(&["browser", "diagnose", "--help"]);
    let expected = [
        "--stale-after-minutes",
        "--probe-url",
        "--probe-timeout-ms",
        "--artifact-max-age-hours",
        "--repair",
    ];
    for option in expected {
        assert!(
            help.contains(option),
            "browser diagnose --help missing expected option: {option}\n{help}"
        );
    }
}

#[test]
#[allow(deprecated)]
fn memory_help_includes_index_and_search_commands() {
    let help = run_help(&["memory", "--help"]);
    let expected = ["index", "search", "status", "clear", "prune", "policy"];

    for name in expected {
        assert!(
            help.contains(name),
            "memory --help missing expected subcommand: {name}\n{help}"
        );
    }
}

#[test]
#[allow(deprecated)]
fn memory_index_help_includes_incremental_option() {
    let help = run_help(&["memory", "index", "--help"]);
    let expected = [
        "--incremental",
        "--namespace",
        "--stale-after-hours",
        "--retain-missing",
    ];
    for option in expected {
        assert!(
            help.contains(option),
            "memory index --help missing expected option {option}:\n{help}"
        );
    }
}

#[test]
#[allow(deprecated)]
fn memory_search_help_includes_namespace_option() {
    let help = run_help(&["memory", "search", "--help"]);
    assert!(
        help.contains("--namespace"),
        "memory search --help missing expected option --namespace:\n{help}"
    );
}

#[test]
#[allow(deprecated)]
fn memory_status_help_includes_all_namespaces_option() {
    let help = run_help(&["memory", "status", "--help"]);
    assert!(
        help.contains("--all-namespaces"),
        "memory status --help missing expected option --all-namespaces:\n{help}"
    );
}

#[test]
#[allow(deprecated)]
fn memory_prune_help_includes_document_quota_option() {
    let help = run_help(&["memory", "prune", "--help"]);
    assert!(
        help.contains("--max-documents-per-namespace"),
        "memory prune --help missing expected option --max-documents-per-namespace:\n{help}"
    );
}

#[test]
#[allow(deprecated)]
fn memory_policy_help_includes_get_set_apply_commands() {
    let help = run_help(&["memory", "policy", "--help"]);
    let expected = ["get", "set", "apply"];
    for command in expected {
        assert!(
            help.contains(command),
            "memory policy --help missing expected subcommand {command}:\n{help}"
        );
    }

    let apply_help = run_help(&["memory", "policy", "apply", "--help"]);
    assert!(
        apply_help.contains("--force"),
        "memory policy apply --help missing expected option --force:\n{apply_help}"
    );
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
fn security_audit_help_includes_filter_options() {
    let help = run_help(&["security", "audit", "--help"]);
    let expected = ["--min-severity", "--category", "--top"];
    for option in expected {
        assert!(
            help.contains(option),
            "security audit --help missing expected option: {option}\n{help}"
        );
    }
}

#[test]
#[allow(deprecated)]
fn plugins_help_includes_management_commands() {
    let help = run_help(&["plugins", "--help"]);
    let expected = [
        "list", "info", "check", "install", "enable", "disable", "doctor", "run", "remove",
    ];

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
fn observability_help_includes_report_and_export_commands() {
    let help = run_help(&["observability", "--help"]);
    let expected = ["report", "export"];

    for name in expected {
        assert!(
            help.contains(name),
            "observability --help missing expected subcommand: {name}\n{help}"
        );
    }

    let report_help = run_help(&["observability", "report", "--help"]);
    for option in ["--audit-tail", "--compare-window"] {
        assert!(
            report_help.contains(option),
            "observability report --help missing expected option: {option}\n{report_help}"
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
fn safety_help_includes_get_check_report_commands() {
    let help = run_help(&["safety", "--help"]);
    let expected = ["get", "check", "report"];

    for name in expected {
        assert!(
            help.contains(name),
            "safety --help missing expected subcommand: {name}\n{help}"
        );
    }
}

#[test]
#[allow(deprecated)]
fn safety_report_help_includes_audit_tail_option() {
    let help = run_help(&["safety", "report", "--help"]);
    for token in ["--command", "--audit-tail", "--compare-window"] {
        assert!(
            help.contains(token),
            "safety report --help missing expected token: {token}\n{help}"
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
    let expected = [
        "--prompt",
        "--session",
        "--agent",
        "--focus",
        "--no-inspector",
    ];
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
