use assert_cmd::Command;
use serde_json::Value;
use tempfile::tempdir;

#[test]
#[allow(deprecated)]
fn plugins_and_skills_list_info_check_flow() {
    let temp = tempdir().expect("tempdir");
    let state_root = temp.path().join(".mosaic");
    let plugin_dir = state_root.join("plugins").join("demo");
    let plugin_hooks = plugin_dir.join("hooks");
    let skill_dir = state_root.join("skills").join("writer");
    std::fs::create_dir_all(&plugin_dir).expect("create plugin dir");
    std::fs::create_dir_all(&plugin_hooks).expect("create plugin hooks");
    std::fs::create_dir_all(&skill_dir).expect("create skill dir");
    std::fs::write(
        plugin_dir.join("plugin.toml"),
        "[plugin]\nid = \"demo\"\nname = \"Demo Plugin\"\nversion = \"0.1.0\"\n\n[runtime]\nrun = \"hooks/run.sh\"\ndoctor = \"hooks/doctor.sh\"\n",
    )
    .expect("write plugin manifest");
    std::fs::write(
        plugin_hooks.join("run.sh"),
        "#!/bin/sh\necho plugin-run:$1\n",
    )
    .expect("write plugin run hook");
    std::fs::write(
        plugin_hooks.join("doctor.sh"),
        "#!/bin/sh\necho plugin-doctor-ok\n",
    )
    .expect("write plugin doctor hook");
    std::fs::write(
        skill_dir.join("SKILL.md"),
        "# Writer\nCreate short release notes.\n",
    )
    .expect("write skill file");

    let plugins_list = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args(["--project-state", "--json", "plugins", "list"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let plugins_list: Value = serde_json::from_slice(&plugins_list).expect("plugins list json");
    assert_eq!(plugins_list["ok"], true);
    assert!(
        plugins_list["plugins"]
            .as_array()
            .expect("plugins array")
            .iter()
            .any(|item| item["id"].as_str() == Some("demo"))
    );
    assert!(
        plugins_list["plugins"]
            .as_array()
            .expect("plugins array")
            .iter()
            .all(|item| item["enabled"].is_boolean())
    );

    let plugins_list_project = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "plugins",
            "list",
            "--source",
            "project",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let plugins_list_project: Value =
        serde_json::from_slice(&plugins_list_project).expect("plugins list project json");
    assert_eq!(plugins_list_project["ok"], true);
    assert_eq!(plugins_list_project["source_filter"], "project");
    assert!(
        plugins_list_project["plugins"]
            .as_array()
            .expect("project plugins array")
            .iter()
            .all(|item| item["source"].as_str() == Some("project"))
    );

    let plugins_info = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args(["--project-state", "--json", "plugins", "info", "demo"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let plugins_info: Value = serde_json::from_slice(&plugins_info).expect("plugins info json");
    assert_eq!(plugins_info["ok"], true);
    assert_eq!(plugins_info["plugin"]["id"], "demo");
    assert_eq!(plugins_info["plugin"]["manifest_valid"], true);
    assert_eq!(plugins_info["plugin"]["enabled"], true);

    let plugins_disable = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args(["--project-state", "--json", "plugins", "disable", "demo"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let plugins_disable: Value =
        serde_json::from_slice(&plugins_disable).expect("plugins disable json");
    assert_eq!(plugins_disable["ok"], true);
    assert_eq!(plugins_disable["enabled"], false);

    let plugins_info_disabled = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args(["--project-state", "--json", "plugins", "info", "demo"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let plugins_info_disabled: Value =
        serde_json::from_slice(&plugins_info_disabled).expect("plugins info disabled json");
    assert_eq!(plugins_info_disabled["ok"], true);
    assert_eq!(plugins_info_disabled["plugin"]["enabled"], false);

    let plugins_check = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args(["--project-state", "--json", "plugins", "check", "demo"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let plugins_check: Value = serde_json::from_slice(&plugins_check).expect("plugins check json");
    assert_eq!(plugins_check["ok"], true);
    assert_eq!(plugins_check["report"]["ok"], true);
    assert_eq!(plugins_check["report"]["checked"], 1);

    let plugins_doctor = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args(["--project-state", "--json", "plugins", "doctor"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let plugins_doctor: Value =
        serde_json::from_slice(&plugins_doctor).expect("plugins doctor json");
    assert_eq!(plugins_doctor["ok"], true);
    assert_eq!(plugins_doctor["doctor"]["plugins_total"], 1);
    assert_eq!(plugins_doctor["doctor"]["disabled_plugins"], 1);
    assert_eq!(
        plugins_doctor["doctor"]["runtime_missing_run_hooks"]
            .as_array()
            .expect("runtime missing run hooks")
            .len(),
        0
    );

    let plugins_enable = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args(["--project-state", "--json", "plugins", "enable", "demo"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let plugins_enable: Value =
        serde_json::from_slice(&plugins_enable).expect("plugins enable json");
    assert_eq!(plugins_enable["ok"], true);
    assert_eq!(plugins_enable["enabled"], true);

    let plugins_run = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--yes",
            "--json",
            "plugins",
            "run",
            "demo",
            "--hook",
            "run",
            "--arg",
            "alpha",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let plugins_run: Value = serde_json::from_slice(&plugins_run).expect("plugins run json");
    assert_eq!(plugins_run["ok"], true);
    assert_eq!(plugins_run["plugin_id"], "demo");
    assert_eq!(plugins_run["hook"], "run");
    assert_eq!(plugins_run["exit_code"], 0);
    assert_eq!(plugins_run["timed_out"], false);
    assert_eq!(plugins_run["timeout_ms"], 15000);
    assert_eq!(plugins_run["sandbox_profile"], "standard");
    assert_eq!(plugins_run["approved_by"], "flag_yes");
    assert_eq!(plugins_run["output_limit_bytes"], 262144);
    assert!(plugins_run["stdout_bytes"].is_number());
    assert!(plugins_run["stderr_bytes"].is_number());
    assert!(plugins_run["stdout_truncated"].is_boolean());
    assert!(plugins_run["stderr_truncated"].is_boolean());
    assert!(plugins_run.get("resource_limits").is_some());
    assert!(plugins_run.get("resource_metrics").is_some());
    assert!(
        plugins_run["resource_metrics"].is_null() || plugins_run["resource_metrics"].is_object()
    );
    assert!(
        plugins_run["command"]["rendered"]
            .as_str()
            .expect("rendered command")
            .contains("hooks/run.sh alpha")
    );
    let event_log_path = plugins_run["event_log_path"]
        .as_str()
        .expect("event log path");
    let event_log_raw = std::fs::read_to_string(event_log_path).expect("read event log");
    assert!(event_log_raw.contains("\"plugin_id\":\"demo\""));
    assert!(event_log_raw.contains("\"hook\":\"run\""));
    assert!(event_log_raw.contains("\"ok\":true"));
    assert!(event_log_raw.contains("\"approved_by\":\"flag_yes\""));
    assert!(event_log_raw.contains("\"output_limit_bytes\":262144"));
    assert!(
        plugins_run["stdout"]
            .as_str()
            .expect("plugins run stdout")
            .contains("plugin-run:alpha")
    );

    let skills_list = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args(["--project-state", "--json", "skills", "list"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let skills_list: Value = serde_json::from_slice(&skills_list).expect("skills list json");
    assert_eq!(skills_list["ok"], true);
    assert!(
        skills_list["skills"]
            .as_array()
            .expect("skills array")
            .iter()
            .any(|item| item["id"].as_str() == Some("writer"))
    );

    let skills_list_project = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "skills",
            "list",
            "--source",
            "project",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let skills_list_project: Value =
        serde_json::from_slice(&skills_list_project).expect("skills list project json");
    assert_eq!(skills_list_project["ok"], true);
    assert_eq!(skills_list_project["source_filter"], "project");
    assert!(
        skills_list_project["skills"]
            .as_array()
            .expect("project skills array")
            .iter()
            .all(|item| item["source"].as_str() == Some("project"))
    );

    let skills_info = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args(["--project-state", "--json", "skills", "info", "writer"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let skills_info: Value = serde_json::from_slice(&skills_info).expect("skills info json");
    assert_eq!(skills_info["ok"], true);
    assert_eq!(skills_info["skill"]["id"], "writer");
    assert_eq!(skills_info["skill"]["title"], "Writer");

    let skills_check = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args(["--project-state", "--json", "skills", "check", "writer"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let skills_check: Value = serde_json::from_slice(&skills_check).expect("skills check json");
    assert_eq!(skills_check["ok"], true);
    assert_eq!(skills_check["report"]["ok"], true);
    assert_eq!(skills_check["report"]["checked"], 1);
}

#[test]
#[allow(deprecated)]
fn plugins_info_missing_returns_validation_error() {
    let temp = tempdir().expect("tempdir");
    let output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args(["--project-state", "--json", "plugins", "info", "missing"])
        .assert()
        .failure()
        .code(7)
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output).expect("json error");
    assert_eq!(json["ok"], false);
    assert_eq!(json["error"]["code"], "validation");
}

#[test]
#[allow(deprecated)]
fn plugins_and_skills_install_remove_flow() {
    let temp = tempdir().expect("tempdir");
    let plugin_source = temp.path().join("sample-plugin");
    let skill_source = temp.path().join("writer");
    std::fs::create_dir_all(&plugin_source).expect("create plugin source");
    std::fs::create_dir_all(&skill_source).expect("create skill source");
    std::fs::write(
        plugin_source.join("plugin.toml"),
        "[plugin]\nid = \"sample_plugin\"\nname = \"Sample Plugin\"\nversion = \"0.1.0\"\n",
    )
    .expect("write plugin manifest");
    std::fs::write(
        skill_source.join("SKILL.md"),
        "# Writer\nGenerate concise notes.\n",
    )
    .expect("write skill file");

    let plugin_install = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "plugins",
            "install",
            "--path",
            "sample-plugin",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let plugin_install: Value =
        serde_json::from_slice(&plugin_install).expect("plugin install json");
    assert_eq!(plugin_install["ok"], true);
    assert_eq!(plugin_install["installed"]["id"], "sample_plugin");

    let skill_install = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "skills",
            "install",
            "--path",
            "writer",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let skill_install: Value = serde_json::from_slice(&skill_install).expect("skill install json");
    assert_eq!(skill_install["ok"], true);
    assert_eq!(skill_install["installed"]["id"], "writer");

    let plugin_remove = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "plugins",
            "remove",
            "sample_plugin",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let plugin_remove: Value = serde_json::from_slice(&plugin_remove).expect("plugin remove json");
    assert_eq!(plugin_remove["ok"], true);
    assert_eq!(plugin_remove["removed"], true);

    let skill_remove = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args(["--project-state", "--json", "skills", "remove", "writer"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let skill_remove: Value = serde_json::from_slice(&skill_remove).expect("skill remove json");
    assert_eq!(skill_remove["ok"], true);
    assert_eq!(skill_remove["removed"], true);
}

#[test]
#[allow(deprecated)]
fn plugins_run_timeout_returns_tool_error_and_logs_event() {
    let temp = tempdir().expect("tempdir");
    let state_root = temp.path().join(".mosaic");
    let plugin_dir = state_root.join("plugins").join("slow");
    let plugin_hooks = plugin_dir.join("hooks");
    std::fs::create_dir_all(&plugin_hooks).expect("create plugin hooks");
    std::fs::write(
        plugin_dir.join("plugin.toml"),
        "[plugin]\nid = \"slow\"\nname = \"Slow Plugin\"\nversion = \"0.1.0\"\n\n[runtime]\nrun = \"hooks/run.sh\"\ntimeout_ms = 2000\n",
    )
    .expect("write plugin manifest");
    std::fs::write(
        plugin_hooks.join("run.sh"),
        "#!/bin/sh\nsleep 1\necho done\n",
    )
    .expect("write run hook");

    let output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--yes",
            "--json",
            "plugins",
            "run",
            "slow",
            "--timeout-ms",
            "10",
        ])
        .assert()
        .failure()
        .code(5)
        .get_output()
        .stdout
        .clone();
    let payload: Value = serde_json::from_slice(&output).expect("plugins run timeout json");
    assert_eq!(payload["ok"], false);
    assert_eq!(payload["error"]["code"], "tool");
    assert!(
        payload["error"]["message"]
            .as_str()
            .expect("timeout message")
            .contains("timed out")
    );

    let event_path = state_root
        .join("data")
        .join("plugin-events")
        .join("slow.jsonl");
    let event_raw = std::fs::read_to_string(event_path).expect("read plugin timeout event");
    assert!(event_raw.contains("\"timed_out\":true"));
    assert!(event_raw.contains("\"ok\":false"));
}

#[test]
#[cfg(unix)]
#[allow(deprecated)]
fn plugins_run_cpu_watchdog_returns_tool_error_and_logs_event() {
    let temp = tempdir().expect("tempdir");
    let state_root = temp.path().join(".mosaic");
    let plugin_dir = state_root.join("plugins").join("cpuwatchdog");
    let plugin_hooks = plugin_dir.join("hooks");
    std::fs::create_dir_all(&plugin_hooks).expect("create plugin hooks");
    std::fs::write(
        plugin_dir.join("plugin.toml"),
        "[plugin]\nid = \"cpuwatchdog\"\nname = \"Cpu Watchdog\"\nversion = \"0.1.0\"\n\n[runtime]\nrun = \"hooks/run.sh\"\nmax_cpu_ms = 100\n",
    )
    .expect("write plugin manifest");
    std::fs::write(
        plugin_hooks.join("run.sh"),
        "#!/bin/sh\nsleep 2\necho done\n",
    )
    .expect("write run hook");

    let output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--yes",
            "--json",
            "plugins",
            "run",
            "cpuwatchdog",
            "--timeout-ms",
            "5000",
        ])
        .assert()
        .failure()
        .code(5)
        .get_output()
        .stdout
        .clone();
    let payload: Value = serde_json::from_slice(&output).expect("plugins run cpu watchdog json");
    assert_eq!(payload["ok"], false);
    assert_eq!(payload["error"]["code"], "tool");
    assert!(
        payload["error"]["message"]
            .as_str()
            .expect("watchdog message")
            .contains("resource watchdog exceeded")
    );

    let event_path = state_root
        .join("data")
        .join("plugin-events")
        .join("cpuwatchdog.jsonl");
    let event_raw = std::fs::read_to_string(event_path).expect("read plugin cpu watchdog event");
    assert!(event_raw.contains("\"resource_limit_error\""));
    assert!(event_raw.contains("resource watchdog exceeded"));
    assert!(event_raw.contains("\"ok\":false"));
}

#[test]
#[cfg(unix)]
#[allow(deprecated)]
fn plugins_run_cpu_watchdog_uses_runtime_override() {
    let temp = tempdir().expect("tempdir");
    let state_root = temp.path().join(".mosaic");
    let plugin_dir = state_root.join("plugins").join("cpuwatchdogoverride");
    let plugin_hooks = plugin_dir.join("hooks");
    std::fs::create_dir_all(&plugin_hooks).expect("create plugin hooks");
    std::fs::write(
        plugin_dir.join("plugin.toml"),
        "[plugin]\nid = \"cpuwatchdogoverride\"\nname = \"Cpu Watchdog Override\"\nversion = \"0.1.0\"\n\n[runtime]\nrun = \"hooks/run.sh\"\nmax_cpu_ms = 5000\ncpu_watchdog_ms = 300\n",
    )
    .expect("write plugin manifest");
    std::fs::write(
        plugin_hooks.join("run.sh"),
        "#!/bin/sh\nsleep 2\necho done\n",
    )
    .expect("write run hook");

    let output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--yes",
            "--json",
            "plugins",
            "run",
            "cpuwatchdogoverride",
            "--timeout-ms",
            "5000",
        ])
        .assert()
        .failure()
        .code(5)
        .get_output()
        .stdout
        .clone();
    let payload: Value =
        serde_json::from_slice(&output).expect("plugins run cpu watchdog override json");
    assert_eq!(payload["ok"], false);
    assert_eq!(payload["error"]["code"], "tool");
    assert!(
        payload["error"]["message"]
            .as_str()
            .expect("watchdog override message")
            .contains("budget_ms=300")
    );

    let event_path = state_root
        .join("data")
        .join("plugin-events")
        .join("cpuwatchdogoverride.jsonl");
    let event_raw =
        std::fs::read_to_string(event_path).expect("read plugin cpu watchdog override event");
    assert!(event_raw.contains("budget_ms=300"));
    assert!(event_raw.contains("\"ok\":false"));
}

#[test]
#[cfg(unix)]
#[allow(deprecated)]
fn plugins_run_resource_limit_exceeded_returns_tool_error_and_logs_event() {
    let temp = tempdir().expect("tempdir");
    let state_root = temp.path().join(".mosaic");
    let plugin_dir = state_root.join("plugins").join("rsslimit");
    let plugin_hooks = plugin_dir.join("hooks");
    std::fs::create_dir_all(&plugin_hooks).expect("create plugin hooks");
    std::fs::write(
        plugin_dir.join("plugin.toml"),
        "[plugin]\nid = \"rsslimit\"\nname = \"Rss Limit\"\nversion = \"0.1.0\"\n\n[runtime]\nrun = \"hooks/run.sh\"\nmax_rss_kb = 1\n",
    )
    .expect("write plugin manifest");
    std::fs::write(
        plugin_hooks.join("run.sh"),
        "#!/bin/sh\nsleep 0.3\necho rss-limit\n",
    )
    .expect("write run hook");

    let output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--yes",
            "--json",
            "plugins",
            "run",
            "rsslimit",
        ])
        .assert()
        .failure()
        .code(5)
        .get_output()
        .stdout
        .clone();
    let payload: Value = serde_json::from_slice(&output).expect("plugins run rss limit json");
    assert_eq!(payload["ok"], false);
    assert_eq!(payload["error"]["code"], "tool");
    assert!(
        payload["error"]["message"]
            .as_str()
            .expect("rss limit message")
            .contains("resource limit exceeded")
    );

    let event_path = state_root
        .join("data")
        .join("plugin-events")
        .join("rsslimit.jsonl");
    let event_raw = std::fs::read_to_string(event_path).expect("read plugin rss limit event");
    assert!(event_raw.contains("\"resource_rlimits_applied\":false"));
    assert!(event_raw.contains("\"resource_limit_error\""));
    assert!(event_raw.contains("\"max_rss_kb\":1"));
    assert!(event_raw.contains("\"ok\":false"));
}

#[test]
#[cfg(unix)]
#[allow(deprecated)]
fn plugins_run_reports_resource_rlimits_applied_for_cpu_limit() {
    let temp = tempdir().expect("tempdir");
    let state_root = temp.path().join(".mosaic");
    let plugin_dir = state_root.join("plugins").join("cpulimit");
    let plugin_hooks = plugin_dir.join("hooks");
    std::fs::create_dir_all(&plugin_hooks).expect("create plugin hooks");
    std::fs::write(
        plugin_dir.join("plugin.toml"),
        "[plugin]\nid = \"cpulimit\"\nname = \"Cpu Limit\"\nversion = \"0.1.0\"\n\n[runtime]\nrun = \"hooks/run.sh\"\nmax_cpu_ms = 500\n",
    )
    .expect("write plugin manifest");
    std::fs::write(plugin_hooks.join("run.sh"), "#!/bin/sh\necho cpu-limit\n")
        .expect("write run hook");

    let output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--yes",
            "--json",
            "plugins",
            "run",
            "cpulimit",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let payload: Value = serde_json::from_slice(&output).expect("plugins run cpu limit json");
    assert_eq!(payload["ok"], true);
    assert_eq!(payload["resource_rlimits_applied"], true);

    let event_path = state_root
        .join("data")
        .join("plugin-events")
        .join("cpulimit.jsonl");
    let event_raw = std::fs::read_to_string(event_path).expect("read plugin cpu limit event");
    assert!(event_raw.contains("\"resource_rlimits_applied\":true"));
}

#[test]
#[cfg(any(
    target_os = "linux",
    target_os = "android",
    target_os = "freebsd",
    target_os = "dragonfly",
    target_os = "netbsd",
    target_os = "openbsd"
))]
#[allow(deprecated)]
fn plugins_run_reports_resource_rlimits_applied_for_memory_limit() {
    let temp = tempdir().expect("tempdir");
    let state_root = temp.path().join(".mosaic");
    let plugin_dir = state_root.join("plugins").join("memlimit");
    let plugin_hooks = plugin_dir.join("hooks");
    std::fs::create_dir_all(&plugin_hooks).expect("create plugin hooks");
    std::fs::write(
        plugin_dir.join("plugin.toml"),
        "[plugin]\nid = \"memlimit\"\nname = \"Memory Limit\"\nversion = \"0.1.0\"\n\n[runtime]\nrun = \"hooks/run.sh\"\nmax_rss_kb = 524288\n",
    )
    .expect("write plugin manifest");
    std::fs::write(plugin_hooks.join("run.sh"), "#!/bin/sh\necho mem-limit\n")
        .expect("write run hook");

    let output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--yes",
            "--json",
            "plugins",
            "run",
            "memlimit",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let payload: Value = serde_json::from_slice(&output).expect("plugins run memory limit json");
    assert_eq!(payload["ok"], true);
    assert_eq!(payload["resource_rlimits_applied"], true);

    let event_path = state_root
        .join("data")
        .join("plugin-events")
        .join("memlimit.jsonl");
    let event_raw = std::fs::read_to_string(event_path).expect("read plugin memory limit event");
    assert!(event_raw.contains("\"resource_rlimits_applied\":true"));
}

#[test]
#[allow(deprecated)]
fn plugins_run_output_is_truncated_when_over_limit() {
    let temp = tempdir().expect("tempdir");
    let state_root = temp.path().join(".mosaic");
    let plugin_dir = state_root.join("plugins").join("loud");
    let plugin_hooks = plugin_dir.join("hooks");
    std::fs::create_dir_all(&plugin_hooks).expect("create plugin hooks");
    std::fs::write(
        plugin_dir.join("plugin.toml"),
        "[plugin]\nid = \"loud\"\nname = \"Loud\"\nversion = \"0.1.0\"\n\n[runtime]\nrun = \"hooks/run.sh\"\nmax_output_bytes = 8\n",
    )
    .expect("write plugin manifest");
    std::fs::write(
        plugin_hooks.join("run.sh"),
        "#!/bin/sh\nprintf 'abcdefghijklmnopqrstuvwxyz'\n",
    )
    .expect("write run hook");

    let output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--yes",
            "--json",
            "plugins",
            "run",
            "loud",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let payload: Value = serde_json::from_slice(&output).expect("plugins run output json");
    assert_eq!(payload["ok"], true);
    assert_eq!(payload["output_limit_bytes"], 8);
    assert_eq!(payload["stdout_truncated"], true);
    assert_eq!(payload["stderr_truncated"], false);
    assert_eq!(
        payload["stdout"]
            .as_str()
            .expect("stdout text")
            .chars()
            .count(),
        8
    );

    let event_path = state_root
        .join("data")
        .join("plugin-events")
        .join("loud.jsonl");
    let event_raw = std::fs::read_to_string(event_path).expect("read plugin loud event");
    assert!(event_raw.contains("\"stdout_truncated\":true"));
    assert!(event_raw.contains("\"output_limit_bytes\":8"));
}
