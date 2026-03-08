use std::path::Path;
use std::time::Duration;

use assert_cmd::Command;
use serde_json::Value;
use tempfile::tempdir;

#[allow(deprecated)]
fn setup_project(temp: &tempfile::TempDir) {
    Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "setup",
            "--base-url",
            "mock://mock-model",
            "--model",
            "mock-model",
        ])
        .assert()
        .success();
}

#[test]
#[allow(deprecated)]
fn knowledge_local_ingest_search_and_ask_flow() {
    let temp = tempdir().expect("tempdir");
    setup_project(&temp);

    let docs_dir = temp.path().join("docs");
    std::fs::create_dir_all(&docs_dir).expect("create docs dir");
    std::fs::write(
        docs_dir.join("ops.md"),
        "# Gateway Retry\nUse exponential backoff for transient failures.\n",
    )
    .expect("write md");

    let ingest_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "knowledge",
            "ingest",
            "--source",
            "local_md",
            "--path",
            "docs",
            "--namespace",
            "kb",
            "--incremental",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let ingest_json: Value = serde_json::from_slice(&ingest_output).expect("ingest json");
    assert_eq!(ingest_json["ok"], true);
    assert_eq!(ingest_json["source"], "local_md");
    assert_eq!(ingest_json["namespace"], "kb");
    assert!(ingest_json["documents_seen"].as_u64().unwrap_or(0) >= 1);

    let search_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "knowledge",
            "search",
            "exponential backoff",
            "--namespace",
            "kb",
            "--limit",
            "5",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let search_json: Value = serde_json::from_slice(&search_output).expect("search json");
    assert_eq!(search_json["ok"], true);
    assert!(search_json["total_hits"].as_u64().unwrap_or(0) >= 1);

    let capture_path = temp.path().join("mock-chat-request.json");
    let ask_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .env("MOSAIC_MOCK_CHAT_RESPONSE", "knowledge-answer")
        .env("MOSAIC_MOCK_CHAT_CAPTURE_PATH", &capture_path)
        .args([
            "--project-state",
            "--json",
            "knowledge",
            "ask",
            "How should retry be configured?",
            "--namespace",
            "kb",
            "--top-k",
            "3",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let ask_json: Value = serde_json::from_slice(&ask_output).expect("ask json");
    assert_eq!(ask_json["ok"], true);
    assert_eq!(ask_json["response"], "knowledge-answer");
    assert!(
        ask_json["references"]
            .as_array()
            .expect("references array")
            .len()
            >= 1
    );

    let captured: Value =
        serde_json::from_slice(&std::fs::read(&capture_path).expect("read capture file"))
            .expect("capture json");
    let messages = captured["messages"].as_array().expect("messages array");
    assert!(messages.iter().any(|message| {
        message["content"]
            .as_str()
            .unwrap_or_default()
            .contains("Knowledge snippets from local knowledge base")
    }));
    assert!(messages.iter().any(|message| {
        message["content"]
            .as_str()
            .unwrap_or_default()
            .contains("ops.md")
    }));
}

#[test]
#[allow(deprecated)]
fn knowledge_local_ingest_report_out_flow() {
    let temp = tempdir().expect("tempdir");
    setup_project(&temp);

    let docs_dir = temp.path().join("docs");
    std::fs::create_dir_all(&docs_dir).expect("create docs dir");
    std::fs::write(
        docs_dir.join("ops.md"),
        "# Retry Guide\nUse exponential backoff with jitter.\n",
    )
    .expect("write md");

    let report_path = temp
        .path()
        .join("reports")
        .join("knowledge-local-report.json");
    let ingest_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "knowledge",
            "ingest",
            "--source",
            "local_md",
            "--path",
            "docs",
            "--namespace",
            "kb_local_report",
            "--report-out",
            report_path.to_string_lossy().as_ref(),
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let ingest_json: Value = serde_json::from_slice(&ingest_output).expect("ingest json");
    assert_eq!(ingest_json["ok"], true);
    assert_eq!(ingest_json["source"], "local_md");
    assert_eq!(ingest_json["namespace"], "kb_local_report");
    assert!(
        report_path.exists(),
        "local_md report should be written when --report-out is provided"
    );

    let report_raw = std::fs::read_to_string(&report_path).expect("read report");
    let report_json: Value = serde_json::from_str(&report_raw).expect("report json");
    assert_eq!(report_json["ok"], true);
    assert_eq!(report_json["source"], "local_md");
    assert_eq!(report_json["namespace"], "kb_local_report");
    assert!(
        report_json["summary"]["documents_seen"]
            .as_u64()
            .unwrap_or(0)
            >= 1,
        "expected summary.documents_seen in local report"
    );
    assert!(
        report_json["entries"]
            .as_array()
            .expect("entries array")
            .is_empty(),
        "non-http local report entries should be empty"
    );
}

#[test]
#[allow(deprecated)]
fn knowledge_search_min_score_filters_low_relevance_hits() {
    let temp = tempdir().expect("tempdir");
    setup_project(&temp);

    let docs_dir = temp.path().join("docs");
    std::fs::create_dir_all(&docs_dir).expect("create docs dir");
    std::fs::write(
        docs_dir.join("high.md"),
        "# Retry Backoff\nretry backoff retry backoff keeps systems stable.\n",
    )
    .expect("write high md");
    std::fs::write(
        docs_dir.join("low.md"),
        "# Retry Only\nretry once and fail fast.\n",
    )
    .expect("write low md");

    Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "knowledge",
            "ingest",
            "--source",
            "local_md",
            "--path",
            "docs",
            "--namespace",
            "kb_min_score",
        ])
        .assert()
        .success();

    let search_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "knowledge",
            "search",
            "retry backoff",
            "--namespace",
            "kb_min_score",
            "--limit",
            "5",
            "--min-score",
            "10",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let search_json: Value = serde_json::from_slice(&search_output).expect("search json");
    assert_eq!(search_json["ok"], true);
    assert_eq!(search_json["min_score"], 10);
    let hits = search_json["hits"].as_array().expect("hits array");
    assert!(!hits.is_empty(), "expected at least one filtered hit");
    assert!(
        hits.iter()
            .all(|hit| hit["score"].as_u64().unwrap_or(0) >= 10),
        "all hits should satisfy min-score"
    );
    assert!(
        hits.iter()
            .all(|hit| !hit["path"].as_str().unwrap_or_default().contains("low-md")),
        "low-score doc should be filtered out"
    );
}

#[test]
#[allow(deprecated)]
fn knowledge_ask_references_only_respects_min_score() {
    let temp = tempdir().expect("tempdir");
    setup_project(&temp);

    let docs_dir = temp.path().join("docs");
    std::fs::create_dir_all(&docs_dir).expect("create docs dir");
    std::fs::write(
        docs_dir.join("strong.md"),
        "# Gateway Retry\nretry backoff retry backoff with jitter.\n",
    )
    .expect("write strong md");
    std::fs::write(docs_dir.join("weak.md"), "# Gateway Note\nretry maybe.\n")
        .expect("write weak md");

    Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "knowledge",
            "ingest",
            "--source",
            "local_md",
            "--path",
            "docs",
            "--namespace",
            "kb_refs_min_score",
        ])
        .assert()
        .success();

    let ask_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "knowledge",
            "ask",
            "How should retry backoff work?",
            "--namespace",
            "kb_refs_min_score",
            "--top-k",
            "5",
            "--min-score",
            "10",
            "--references-only",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let ask_json: Value = serde_json::from_slice(&ask_output).expect("ask json");
    assert_eq!(ask_json["ok"], true);
    assert_eq!(ask_json["mode"], "references_only");
    assert_eq!(ask_json["min_score"], 10);
    let refs = ask_json["references"].as_array().expect("references array");
    assert!(!refs.is_empty(), "expected at least one reference");
    assert!(
        refs.iter()
            .all(|item| item["score"].as_u64().unwrap_or(0) >= 10),
        "references should satisfy min-score"
    );
    assert!(
        refs.iter().all(|item| {
            !item["path"]
                .as_str()
                .unwrap_or_default()
                .contains("weak-md")
        }),
        "weak reference should be filtered by min-score"
    );
}

#[test]
#[allow(deprecated)]
fn knowledge_evaluate_generates_summary_and_report() {
    let temp = tempdir().expect("tempdir");
    setup_project(&temp);

    let docs_dir = temp.path().join("docs");
    std::fs::create_dir_all(&docs_dir).expect("create docs dir");
    std::fs::write(
        docs_dir.join("retry.md"),
        "# Retry Backoff\nretry backoff retry backoff with jitter and caps.\n",
    )
    .expect("write retry md");
    std::fs::write(
        docs_dir.join("sandbox.md"),
        "# Sandbox Policy\nsandbox policy defaults to restricted profile.\n",
    )
    .expect("write sandbox md");

    Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "knowledge",
            "ingest",
            "--source",
            "local_md",
            "--path",
            "docs",
            "--namespace",
            "kb_eval",
        ])
        .assert()
        .success();

    let query_file = temp.path().join("queries.txt");
    std::fs::write(
        &query_file,
        "sandbox policy\nmissing topic\n# comment line should be ignored\n",
    )
    .expect("write query file");

    let report_path = temp.path().join("reports").join("knowledge-eval.json");
    let output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "knowledge",
            "evaluate",
            "--query",
            "retry backoff",
            "--query-file",
            query_file.to_string_lossy().as_ref(),
            "--namespace",
            "kb_eval",
            "--top-k",
            "5",
            "--min-score",
            "3",
            "--report-out",
            report_path.to_string_lossy().as_ref(),
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let payload: Value = serde_json::from_slice(&output).expect("evaluate json");
    assert_eq!(payload["ok"], true);
    assert_eq!(payload["namespace"], "kb_eval");
    assert_eq!(payload["top_k"], 5);
    assert_eq!(payload["min_score"], 3);
    assert_eq!(payload["summary"]["query_total"], 3);
    assert_eq!(payload["summary"]["with_hits"], 2);
    assert_eq!(payload["summary"]["without_hits"], 1);
    assert!(
        payload["summary"]["coverage_rate"].as_f64().unwrap_or(0.0) > 0.6,
        "coverage should reflect two matching queries out of three"
    );
    assert_eq!(payload["queries"].as_array().expect("queries").len(), 3);
    assert!(
        report_path.exists(),
        "report should be written when --report-out is provided"
    );

    let report_raw = std::fs::read_to_string(&report_path).expect("read report");
    let report_payload: Value = serde_json::from_str(&report_raw).expect("report json");
    assert_eq!(report_payload["ok"], true);
    assert_eq!(report_payload["summary"]["query_total"], 3);
    assert_eq!(report_payload["summary"]["with_hits"], 2);
}

#[test]
#[allow(deprecated)]
fn knowledge_evaluate_baseline_compare_and_regression_fail() {
    let temp = tempdir().expect("tempdir");
    setup_project(&temp);

    let docs_dir = temp.path().join("docs");
    std::fs::create_dir_all(&docs_dir).expect("create docs dir");
    std::fs::write(
        docs_dir.join("ops.md"),
        "# Retry Guide\nretry jitter retry jitter retry jitter.\n",
    )
    .expect("write md");

    Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "knowledge",
            "ingest",
            "--source",
            "local_md",
            "--path",
            "docs",
            "--namespace",
            "kb_eval_baseline",
        ])
        .assert()
        .success();

    let baseline_path = temp
        .path()
        .join("baselines")
        .join("knowledge-eval-baseline.json");
    let baseline_create_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "knowledge",
            "evaluate",
            "--query",
            "retry jitter",
            "--namespace",
            "kb_eval_baseline",
            "--top-k",
            "5",
            "--min-score",
            "3",
            "--baseline",
            baseline_path.to_string_lossy().as_ref(),
            "--update-baseline",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let baseline_create_json: Value =
        serde_json::from_slice(&baseline_create_output).expect("baseline create json");
    assert_eq!(baseline_create_json["ok"], true);
    assert_eq!(baseline_create_json["baseline"]["updated"], true);
    assert_eq!(baseline_create_json["baseline"]["found"], true);
    assert_eq!(baseline_create_json["baseline"]["regressed"], false);
    assert!(baseline_path.exists(), "baseline file should be written");

    let compare_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "knowledge",
            "evaluate",
            "--query",
            "missing topic",
            "--namespace",
            "kb_eval_baseline",
            "--top-k",
            "5",
            "--min-score",
            "3",
            "--baseline",
            baseline_path.to_string_lossy().as_ref(),
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let compare_json: Value = serde_json::from_slice(&compare_output).expect("compare json");
    assert_eq!(compare_json["ok"], true);
    assert_eq!(compare_json["baseline"]["enabled"], true);
    assert_eq!(compare_json["baseline"]["found"], true);
    assert_eq!(compare_json["baseline"]["regressed"], true);
    assert!(
        compare_json["baseline"]["reasons"]
            .as_array()
            .expect("reasons array")
            .len()
            >= 1,
        "expected at least one regression reason"
    );
    assert!(
        compare_json["baseline"]["deltas"]["coverage_rate"]
            .as_f64()
            .unwrap_or(0.0)
            < 0.0,
        "coverage delta should be negative for regression run"
    );

    let failed = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "knowledge",
            "evaluate",
            "--query",
            "missing topic",
            "--namespace",
            "kb_eval_baseline",
            "--top-k",
            "5",
            "--min-score",
            "3",
            "--baseline",
            baseline_path.to_string_lossy().as_ref(),
            "--fail-on-regression",
        ])
        .output()
        .expect("run command");
    assert!(
        !failed.status.success(),
        "evaluate should fail when --fail-on-regression is set and regression is detected"
    );
    let stderr = String::from_utf8_lossy(&failed.stderr);
    assert!(
        stderr.contains("knowledge evaluation regressed against baseline"),
        "unexpected stderr: {stderr}"
    );
}

#[test]
#[allow(deprecated)]
fn knowledge_evaluate_history_trend_and_file_flow() {
    let temp = tempdir().expect("tempdir");
    setup_project(&temp);

    let docs_dir = temp.path().join("docs");
    std::fs::create_dir_all(&docs_dir).expect("create docs dir");
    std::fs::write(
        docs_dir.join("ops.md"),
        "# Retry Guide\nretry jitter retry jitter retry jitter.\n",
    )
    .expect("write md");

    Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "knowledge",
            "ingest",
            "--source",
            "local_md",
            "--path",
            "docs",
            "--namespace",
            "kb_eval_history",
        ])
        .assert()
        .success();

    let first_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "knowledge",
            "evaluate",
            "--query",
            "retry jitter",
            "--namespace",
            "kb_eval_history",
            "--history-window",
            "5",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let first_json: Value = serde_json::from_slice(&first_output).expect("first evaluate json");
    assert_eq!(first_json["ok"], true);
    assert_eq!(first_json["history"]["entries_before"], 0);
    assert_eq!(first_json["history"]["entries_after"], 1);
    assert!(first_json["history"]["previous"].is_null());
    assert!(first_json["history"]["delta_vs_previous"].is_null());

    let second_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "knowledge",
            "evaluate",
            "--query",
            "missing topic",
            "--namespace",
            "kb_eval_history",
            "--history-window",
            "5",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let second_json: Value = serde_json::from_slice(&second_output).expect("second evaluate json");
    assert_eq!(second_json["ok"], true);
    assert_eq!(second_json["history"]["entries_before"], 1);
    assert_eq!(second_json["history"]["entries_after"], 2);
    assert!(second_json["history"]["previous"].is_object());
    assert!(second_json["history"]["delta_vs_previous"].is_object());
    assert!(
        second_json["history"]["delta_vs_previous"]["coverage_rate"]
            .as_f64()
            .unwrap_or(0.0)
            < 0.0,
        "coverage_rate delta should be negative when current run has no hits"
    );
    assert_eq!(
        second_json["history"]["window_before_current"]["samples"],
        1
    );
    assert_eq!(second_json["history"]["window_with_current"]["samples"], 2);

    let history_path = second_json["history"]["path"]
        .as_str()
        .expect("history path");
    let history_raw = std::fs::read_to_string(history_path).expect("read history file");
    let line_count = history_raw
        .lines()
        .filter(|line| !line.trim().is_empty())
        .count();
    assert_eq!(
        line_count, 2,
        "history file should include two evaluation samples"
    );
}

#[test]
#[allow(deprecated)]
fn knowledge_evaluate_rejects_invalid_history_window() {
    let temp = tempdir().expect("tempdir");
    setup_project(&temp);

    let output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "knowledge",
            "evaluate",
            "--query",
            "retry",
            "--history-window",
            "0",
        ])
        .output()
        .expect("run evaluate");
    assert!(
        !output.status.success(),
        "evaluate should fail when --history-window is 0"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("history_window must be greater than 0"),
        "unexpected stderr: {stderr}"
    );
}

#[test]
#[allow(deprecated)]
fn knowledge_datasets_list_and_remove_flow() {
    let temp = tempdir().expect("tempdir");
    setup_project(&temp);

    let docs_dir = temp.path().join("docs");
    std::fs::create_dir_all(&docs_dir).expect("create docs dir");
    std::fs::write(
        docs_dir.join("ops.md"),
        "# Retry Guide\nretry jitter retry jitter retry jitter.\n",
    )
    .expect("write md");

    Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "knowledge",
            "ingest",
            "--source",
            "local_md",
            "--path",
            "docs",
            "--namespace",
            "kb_ds",
        ])
        .assert()
        .success();

    Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "knowledge",
            "evaluate",
            "--query",
            "retry jitter",
            "--namespace",
            "kb_ds",
            "--update-baseline",
        ])
        .assert()
        .success();

    let list_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args(["--project-state", "--json", "knowledge", "datasets", "list"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let list_json: Value = serde_json::from_slice(&list_output).expect("datasets list json");
    assert_eq!(list_json["ok"], true);
    let datasets = list_json["datasets"].as_array().expect("datasets array");
    let dataset = datasets
        .iter()
        .find(|item| item["namespace"] == "kb_ds")
        .expect("kb_ds dataset missing");

    assert_eq!(dataset["stage_exists"], true);
    assert!(dataset["staged_files"].as_u64().unwrap_or(0) >= 1);
    assert!(dataset["indexed_documents"].as_u64().unwrap_or(0) >= 1);
    assert_eq!(dataset["baseline_exists"], true);
    assert!(dataset["history_samples"].as_u64().unwrap_or(0) >= 1);

    let stage_root = dataset["stage_root"].as_str().expect("stage_root");
    let memory_index_path = dataset["memory_index_path"]
        .as_str()
        .expect("memory_index_path");
    let memory_status_path = dataset["memory_status_path"]
        .as_str()
        .expect("memory_status_path");
    let baseline_path = dataset["baseline_path"].as_str().expect("baseline_path");
    let history_path = dataset["history_path"].as_str().expect("history_path");

    assert!(Path::new(stage_root).exists(), "stage root should exist");
    assert!(
        Path::new(memory_index_path).exists(),
        "memory index should exist"
    );
    assert!(
        Path::new(memory_status_path).exists(),
        "memory status should exist"
    );
    assert!(Path::new(baseline_path).exists(), "baseline should exist");
    assert!(Path::new(history_path).exists(), "history should exist");

    let dry_run_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "knowledge",
            "datasets",
            "remove",
            "kb_ds",
            "--dry-run",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let dry_run_json: Value = serde_json::from_slice(&dry_run_output).expect("dry-run json");
    assert_eq!(dry_run_json["ok"], true);
    assert_eq!(dry_run_json["result"]["namespace"], "kb_ds");
    assert_eq!(dry_run_json["result"]["dry_run"], true);
    assert_eq!(dry_run_json["result"]["removed_stage"], true);
    assert_eq!(dry_run_json["result"]["removed_memory_index"], true);
    assert_eq!(dry_run_json["result"]["removed_memory_status"], true);
    assert_eq!(dry_run_json["result"]["removed_baseline"], true);
    assert_eq!(dry_run_json["result"]["removed_history"], true);
    assert!(
        Path::new(stage_root).exists(),
        "dry-run should not delete stage files"
    );
    assert!(
        Path::new(memory_index_path).exists(),
        "dry-run should not delete memory index"
    );

    let remove_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "knowledge",
            "datasets",
            "remove",
            "kb_ds",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let remove_json: Value = serde_json::from_slice(&remove_output).expect("remove json");
    assert_eq!(remove_json["ok"], true);
    assert_eq!(remove_json["result"]["namespace"], "kb_ds");
    assert_eq!(remove_json["result"]["dry_run"], false);
    assert_eq!(remove_json["result"]["removed_stage"], true);
    assert_eq!(remove_json["result"]["removed_memory_index"], true);
    assert_eq!(remove_json["result"]["removed_memory_status"], true);
    assert_eq!(remove_json["result"]["removed_baseline"], true);
    assert_eq!(remove_json["result"]["removed_history"], true);

    assert!(
        !Path::new(stage_root).exists(),
        "stage root should be removed"
    );
    assert!(
        !Path::new(memory_index_path).exists(),
        "memory index should be removed"
    );
    assert!(
        !Path::new(memory_status_path).exists(),
        "memory status should be removed"
    );
    assert!(
        !Path::new(baseline_path).exists(),
        "baseline should be removed"
    );
    assert!(
        !Path::new(history_path).exists(),
        "history should be removed"
    );

    let list_after_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "knowledge",
            "datasets",
            "list",
            "--namespace",
            "kb_ds",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let list_after_json: Value =
        serde_json::from_slice(&list_after_output).expect("datasets list after json");
    assert_eq!(list_after_json["ok"], true);
    assert_eq!(list_after_json["count"], 0);
    assert_eq!(
        list_after_json["datasets"]
            .as_array()
            .expect("datasets array")
            .len(),
        0
    );
}

#[test]
#[allow(deprecated)]
fn knowledge_datasets_remove_rejects_default_namespace() {
    let temp = tempdir().expect("tempdir");
    setup_project(&temp);

    let output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "knowledge",
            "datasets",
            "remove",
            "default",
        ])
        .output()
        .expect("run command");
    assert!(!output.status.success(), "command should fail");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("knowledge datasets remove does not support namespace 'default'"),
        "unexpected stderr: {stderr}"
    );
}

#[test]
#[allow(deprecated)]
fn knowledge_evaluate_rejects_conflicting_baseline_flags() {
    let temp = tempdir().expect("tempdir");
    setup_project(&temp);

    let output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "knowledge",
            "evaluate",
            "--query",
            "retry",
            "--no-baseline",
            "--update-baseline",
        ])
        .output()
        .expect("run evaluate");
    assert!(
        !output.status.success(),
        "evaluate should fail on conflicting baseline flags"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("--no-baseline and --update-baseline cannot be used together"),
        "unexpected stderr: {stderr}"
    );
}

#[test]
#[allow(deprecated)]
fn knowledge_evaluate_requires_query_input() {
    let temp = tempdir().expect("tempdir");
    setup_project(&temp);

    let output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "knowledge",
            "evaluate",
            "--namespace",
            "kb_eval",
        ])
        .output()
        .expect("run evaluate");
    assert!(
        !output.status.success(),
        "evaluate should fail without query input"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("knowledge evaluate requires at least one --query or --query-file entry"),
        "unexpected stderr: {stderr}"
    );
}

#[test]
#[allow(deprecated)]
fn knowledge_http_ingest_flow() {
    let temp = tempdir().expect("tempdir");
    setup_project(&temp);

    let server = tiny_http::Server::http("127.0.0.1:0").expect("start tiny server");
    let base = format!("http://{}", server.server_addr());
    let handle = std::thread::spawn(move || {
        if let Ok(Some(request)) = server.recv_timeout(Duration::from_secs(10)) {
            let response = tiny_http::Response::from_string(
                "# HTTP KB\nHTTP source documents can be indexed.\n",
            );
            let _ = request.respond(response);
        }
    });

    let url = format!("{base}/kb.md");
    let ingest_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "knowledge",
            "ingest",
            "--source",
            "http",
            "--url",
            &url,
            "--namespace",
            "httpkb",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let ingest_json: Value = serde_json::from_slice(&ingest_output).expect("ingest json");
    assert_eq!(ingest_json["ok"], true);
    assert_eq!(ingest_json["source"], "http");
    assert_eq!(ingest_json["namespace"], "httpkb");
    assert!(ingest_json["documents_seen"].as_u64().unwrap_or(0) >= 1);
    handle.join().expect("server thread join");
}

#[test]
#[allow(deprecated)]
fn knowledge_http_ingest_retries_and_header_env_flow() {
    let temp = tempdir().expect("tempdir");
    setup_project(&temp);

    let server = tiny_http::Server::http("127.0.0.1:0").expect("start tiny server");
    let base = format!("http://{}", server.server_addr());
    let handle = std::thread::spawn(move || {
        let mut attempts = 0usize;
        let mut header_seen_all = true;
        while attempts < 2 {
            let request = server
                .recv_timeout(Duration::from_secs(10))
                .ok()
                .flatten()
                .expect("request");
            attempts = attempts.saturating_add(1);
            let header_ok = request.headers().iter().any(|header| {
                header
                    .field
                    .as_str()
                    .as_str()
                    .eq_ignore_ascii_case("x-kb-token")
                    && header.value.as_str() == "secret-token"
            });
            if !header_ok {
                header_seen_all = false;
            }

            let response = if attempts == 1 {
                tiny_http::Response::from_string("temporary upstream failure").with_status_code(502)
            } else {
                tiny_http::Response::from_string(
                    "# HTTP KB Retry\nHTTP source documents can be indexed after retry.\n",
                )
            };
            let _ = request.respond(response);
        }
        (attempts, header_seen_all)
    });

    let url = format!("{base}/kb-retry.md");
    let ingest_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .env("MOSAIC_KB_TOKEN_TEST", "secret-token")
        .args([
            "--project-state",
            "--json",
            "knowledge",
            "ingest",
            "--source",
            "http",
            "--url",
            &url,
            "--header-env",
            "x-kb-token=MOSAIC_KB_TOKEN_TEST",
            "--http-retries",
            "3",
            "--http-retry-backoff-ms",
            "10",
            "--namespace",
            "httpkb_retry",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let ingest_json: Value = serde_json::from_slice(&ingest_output).expect("ingest json");
    assert_eq!(ingest_json["ok"], true);
    assert_eq!(ingest_json["source"], "http");
    assert_eq!(ingest_json["namespace"], "httpkb_retry");
    assert!(ingest_json["documents_seen"].as_u64().unwrap_or(0) >= 1);

    let (attempts, header_seen_all) = handle.join().expect("server thread join");
    assert_eq!(attempts, 2, "expected one retry before success");
    assert!(
        header_seen_all,
        "retry requests should preserve custom headers"
    );
}

#[test]
#[allow(deprecated)]
fn knowledge_http_ingest_continue_on_error_and_report_flow() {
    let temp = tempdir().expect("tempdir");
    setup_project(&temp);

    let server = tiny_http::Server::http("127.0.0.1:0").expect("start tiny server");
    let base = format!("http://{}", server.server_addr());
    let handle = std::thread::spawn(move || {
        for _ in 0..2 {
            let request = server
                .recv_timeout(Duration::from_secs(10))
                .ok()
                .flatten()
                .expect("request");
            let url = request.url().to_string();
            let response = if url.contains("bad") {
                tiny_http::Response::from_string("upstream failed").with_status_code(502)
            } else {
                tiny_http::Response::from_string("# HTTP Good\nGood document.\n")
            };
            let _ = request.respond(response);
        }
    });

    let bad_url = format!("{base}/bad.md");
    let good_url = format!("{base}/good.md");
    let report_path = temp
        .path()
        .join("reports")
        .join("knowledge-http-report.json");
    let ingest_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "knowledge",
            "ingest",
            "--source",
            "http",
            "--url",
            &bad_url,
            "--url",
            &good_url,
            "--continue-on-error",
            "--http-retries",
            "1",
            "--report-out",
            report_path.to_string_lossy().as_ref(),
            "--namespace",
            "httpkb_report",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let ingest_json: Value = serde_json::from_slice(&ingest_output).expect("ingest json");
    assert_eq!(ingest_json["ok"], true);
    assert_eq!(
        ingest_json["source_detail"]["continue_on_error"],
        Value::Bool(true)
    );
    assert_eq!(ingest_json["source_detail"]["fetched_total"], 2);
    assert_eq!(ingest_json["source_detail"]["fetched_failed"], 1);
    assert_eq!(ingest_json["source_detail"]["fetched_succeeded"], 1);

    let report_raw = std::fs::read_to_string(&report_path).expect("read report");
    let report_json: Value = serde_json::from_str(&report_raw).expect("report json");
    assert_eq!(report_json["summary"]["fetched_total"], 2);
    assert_eq!(report_json["summary"]["fetched_failed"], 1);
    assert_eq!(report_json["summary"]["fetched_succeeded"], 1);
    assert_eq!(report_json["entries"].as_array().expect("entries").len(), 2);

    handle.join().expect("server thread join");
}

#[test]
#[allow(deprecated)]
fn knowledge_http_ingest_fail_fast_by_default() {
    let temp = tempdir().expect("tempdir");
    setup_project(&temp);

    let server = tiny_http::Server::http("127.0.0.1:0").expect("start tiny server");
    let base = format!("http://{}", server.server_addr());
    let handle = std::thread::spawn(move || {
        if let Ok(Some(request)) = server.recv_timeout(Duration::from_secs(10)) {
            let _ = request
                .respond(tiny_http::Response::from_string("upstream failed").with_status_code(502));
        }
    });

    let bad_url = format!("{base}/bad.md");
    let output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "knowledge",
            "ingest",
            "--source",
            "http",
            "--url",
            &bad_url,
            "--http-retries",
            "1",
            "--namespace",
            "httpkb_fail_fast",
        ])
        .output()
        .expect("run command");
    assert!(
        !output.status.success(),
        "command should fail without continue mode"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("--continue-on-error"),
        "stderr should hint continue mode, got: {stderr}"
    );

    handle.join().expect("server thread join");
}

#[test]
#[allow(deprecated)]
fn knowledge_mcp_ingest_flow() {
    let temp = tempdir().expect("tempdir");
    setup_project(&temp);

    let mcp_docs = temp.path().join("mcp-docs");
    std::fs::create_dir_all(&mcp_docs).expect("create mcp docs");
    std::fs::write(
        mcp_docs.join("guide.md"),
        "# MCP Guide\nMCP docs are ingested via configured cwd.\n",
    )
    .expect("write mcp doc");

    let add_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "mcp",
            "add",
            "--name",
            "docs",
            "--command",
            "cat",
            "--cwd",
            mcp_docs.to_string_lossy().as_ref(),
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let add_json: Value = serde_json::from_slice(&add_output).expect("mcp add json");
    let server_id = add_json["server"]["id"].as_str().expect("mcp server id");

    let ingest_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "knowledge",
            "ingest",
            "--source",
            "mcp",
            "--mcp-server",
            server_id,
            "--namespace",
            "mcpkb",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let ingest_json: Value = serde_json::from_slice(&ingest_output).expect("ingest json");
    assert_eq!(ingest_json["ok"], true);
    assert_eq!(ingest_json["source"], "mcp");
    assert_eq!(ingest_json["namespace"], "mcpkb");
    assert!(ingest_json["documents_seen"].as_u64().unwrap_or(0) >= 1);

    let search_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "knowledge",
            "search",
            "configured cwd",
            "--namespace",
            "mcpkb",
            "--limit",
            "5",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let search_json: Value = serde_json::from_slice(&search_output).expect("search json");
    assert_eq!(search_json["ok"], true);
    assert!(search_json["total_hits"].as_u64().unwrap_or(0) >= 1);
}

#[test]
#[allow(deprecated)]
fn knowledge_ask_references_only_skips_model_call() {
    let temp = tempdir().expect("tempdir");
    setup_project(&temp);

    let docs_dir = temp.path().join("docs");
    std::fs::create_dir_all(&docs_dir).expect("create docs dir");
    std::fs::write(
        docs_dir.join("ops.md"),
        "# Retry Guide\nUse exponential backoff with jitter.\n",
    )
    .expect("write md");

    Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "knowledge",
            "ingest",
            "--source",
            "local_md",
            "--path",
            "docs",
            "--namespace",
            "kb_refs",
        ])
        .assert()
        .success();

    let capture_path = temp.path().join("mock-chat-request.json");
    let ask_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .env("MOSAIC_MOCK_CHAT_CAPTURE_PATH", &capture_path)
        .args([
            "--project-state",
            "--json",
            "knowledge",
            "ask",
            "What should retry strategy be?",
            "--namespace",
            "kb_refs",
            "--top-k",
            "5",
            "--references-only",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let ask_json: Value = serde_json::from_slice(&ask_output).expect("ask json");
    assert_eq!(ask_json["ok"], true);
    assert_eq!(ask_json["mode"], "references_only");
    assert_eq!(ask_json["namespace"], "kb_refs");
    assert!(ask_json["total_references"].as_u64().unwrap_or(0) >= 1);
    assert!(
        ask_json["references"]
            .as_array()
            .expect("references array")
            .len()
            >= 1
    );
    assert!(
        !capture_path.exists(),
        "references-only mode should not call provider chat endpoint"
    );
}

#[test]
#[allow(deprecated)]
fn knowledge_ask_references_only_rejects_session_or_agent_flags() {
    let temp = tempdir().expect("tempdir");
    setup_project(&temp);

    let output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "knowledge",
            "ask",
            "hello",
            "--references-only",
            "--session",
            "sess_1",
        ])
        .output()
        .expect("run command");
    assert!(!output.status.success(), "command should fail");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("--references-only cannot be combined with --session or --agent"),
        "unexpected stderr: {stderr}"
    );
}
