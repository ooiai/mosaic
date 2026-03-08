use assert_cmd::Command;
use serde_json::Value;
use tempfile::tempdir;

#[test]
#[allow(deprecated)]
fn memory_index_search_status_flow() {
    let temp = tempdir().expect("tempdir");
    std::fs::write(
        temp.path().join("README.txt"),
        "Rust memory search supports local indexing",
    )
    .expect("write readme");
    std::fs::write(temp.path().join("notes.md"), "memory memory rust cli").expect("write notes");

    let index_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "memory",
            "index",
            "--path",
            ".",
            "--max-files",
            "50",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let index_json: Value = serde_json::from_slice(&index_output).expect("index json");
    assert_eq!(index_json["ok"], true);
    assert!(
        index_json["index"]["indexed_documents"]
            .as_u64()
            .unwrap_or(0)
            >= 2
    );

    let search_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "memory",
            "search",
            "rust",
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
    assert!(search_json["result"]["total_hits"].as_u64().unwrap_or(0) >= 1);

    let status_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args(["--project-state", "--json", "memory", "status"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let status_json: Value = serde_json::from_slice(&status_output).expect("status json");
    assert_eq!(status_json["ok"], true);
    assert!(
        status_json["status"]["indexed_documents"]
            .as_u64()
            .unwrap_or(0)
            >= 2
    );

    let clear_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args(["--project-state", "--json", "memory", "clear"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let clear_json: Value = serde_json::from_slice(&clear_output).expect("clear json");
    assert_eq!(clear_json["ok"], true);
    assert_eq!(clear_json["cleared"]["removed_index"], true);
    assert_eq!(clear_json["cleared"]["removed_status"], true);

    let status_after_clear_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args(["--project-state", "--json", "memory", "status"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let status_after_clear_json: Value =
        serde_json::from_slice(&status_after_clear_output).expect("status-after-clear json");
    assert_eq!(status_after_clear_json["ok"], true);
    assert_eq!(status_after_clear_json["status"]["indexed_documents"], 0);
    assert!(status_after_clear_json["status"]["last_indexed_at"].is_null());
}

#[test]
#[allow(deprecated)]
fn memory_index_missing_path_returns_validation_error() {
    let temp = tempdir().expect("tempdir");
    let output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "memory",
            "index",
            "--path",
            "./missing-dir",
        ])
        .assert()
        .failure()
        .code(7)
        .get_output()
        .stdout
        .clone();

    let json: Value = serde_json::from_slice(&output).expect("error json");
    assert_eq!(json["ok"], false);
    assert_eq!(json["error"]["code"], "validation");
}

#[test]
#[allow(deprecated)]
fn memory_incremental_index_reuses_unchanged_documents() {
    let temp = tempdir().expect("tempdir");
    std::fs::write(temp.path().join("a.md"), "alpha memory baseline").expect("write a");
    std::fs::write(temp.path().join("b.md"), "beta memory baseline").expect("write b");

    let first = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "memory",
            "index",
            "--path",
            ".",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let first: Value = serde_json::from_slice(&first).expect("first json");
    assert_eq!(first["ok"], true);
    assert_eq!(first["index"]["incremental"], false);
    assert!(first["index"]["indexed_documents"].as_u64().unwrap_or(0) >= 2);

    std::thread::sleep(std::time::Duration::from_millis(10));
    std::fs::write(temp.path().join("a.md"), "alpha memory changed").expect("rewrite a");
    std::fs::remove_file(temp.path().join("b.md")).expect("remove b");
    std::fs::write(temp.path().join("c.md"), "gamma memory added").expect("write c");

    let second = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "memory",
            "index",
            "--path",
            ".",
            "--incremental",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let second: Value = serde_json::from_slice(&second).expect("second json");
    assert_eq!(second["ok"], true);
    assert_eq!(second["index"]["incremental"], true);
    assert_eq!(second["index"]["removed_documents"], 1);
    assert!(second["index"]["reindexed_documents"].as_u64().unwrap_or(0) >= 1);
    assert!(
        second["index"]["indexed_documents"].as_u64().unwrap_or(0)
            >= second["index"]["reindexed_documents"].as_u64().unwrap_or(0)
    );
}

#[test]
#[allow(deprecated)]
fn memory_namespace_isolation_flow() {
    let temp = tempdir().expect("tempdir");
    std::fs::write(temp.path().join("notes.md"), "gateway memory namespace").expect("write notes");

    let default_index = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "memory",
            "index",
            "--path",
            ".",
            "--namespace",
            "default",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let default_index: Value = serde_json::from_slice(&default_index).expect("default index json");
    assert_eq!(default_index["ok"], true);
    assert_eq!(default_index["namespace"], "default");
    assert!(
        default_index["index"]["indexed_documents"]
            .as_u64()
            .unwrap_or(0)
            >= 1
    );

    let ops_index = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "memory",
            "index",
            "--path",
            ".",
            "--namespace",
            "ops",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let ops_index: Value = serde_json::from_slice(&ops_index).expect("ops index json");
    assert_eq!(ops_index["ok"], true);
    assert_eq!(ops_index["namespace"], "ops");
    assert!(
        ops_index["index"]["indexed_documents"]
            .as_u64()
            .unwrap_or(0)
            >= 1
    );

    let clear_ops = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "memory",
            "clear",
            "--namespace",
            "ops",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let clear_ops: Value = serde_json::from_slice(&clear_ops).expect("clear ops json");
    assert_eq!(clear_ops["ok"], true);
    assert_eq!(clear_ops["namespace"], "ops");

    let status_default = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "memory",
            "status",
            "--namespace",
            "default",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let status_default: Value = serde_json::from_slice(&status_default).expect("status default");
    assert_eq!(status_default["ok"], true);
    assert_eq!(status_default["namespace"], "default");
    assert!(
        status_default["status"]["indexed_documents"]
            .as_u64()
            .unwrap_or(0)
            >= 1
    );
}

#[test]
#[allow(deprecated)]
fn memory_incremental_stale_reindex_flow() {
    let temp = tempdir().expect("tempdir");
    std::fs::write(temp.path().join("a.md"), "stale memory sample").expect("write a");

    Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "memory",
            "index",
            "--path",
            ".",
        ])
        .assert()
        .success();

    let second = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "memory",
            "index",
            "--path",
            ".",
            "--incremental",
            "--stale-after-hours",
            "0",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let second: Value = serde_json::from_slice(&second).expect("second json");
    assert_eq!(second["ok"], true);
    assert_eq!(second["index"]["incremental"], true);
    assert!(
        second["index"]["stale_reindexed_documents"]
            .as_u64()
            .unwrap_or(0)
            >= 1
    );
}

#[test]
#[allow(deprecated)]
fn memory_invalid_namespace_returns_validation_error() {
    let temp = tempdir().expect("tempdir");
    let output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "memory",
            "status",
            "--namespace",
            "bad/namespace",
        ])
        .assert()
        .failure()
        .code(7)
        .get_output()
        .stdout
        .clone();
    let output: Value = serde_json::from_slice(&output).expect("error json");
    assert_eq!(output["ok"], false);
    assert_eq!(output["error"]["code"], "validation");
}

#[test]
#[allow(deprecated)]
fn memory_status_all_namespaces_and_prune_flow() {
    let temp = tempdir().expect("tempdir");
    std::fs::write(temp.path().join("notes.md"), "memory prune namespace ops")
        .expect("write notes");

    Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "memory",
            "index",
            "--path",
            ".",
            "--namespace",
            "ops",
        ])
        .assert()
        .success();
    Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "memory",
            "index",
            "--path",
            ".",
            "--namespace",
            "research",
        ])
        .assert()
        .success();

    let status_all = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "memory",
            "status",
            "--all-namespaces",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let status_all: Value = serde_json::from_slice(&status_all).expect("status all json");
    assert_eq!(status_all["ok"], true);
    assert!(
        status_all["namespaces"]
            .as_array()
            .expect("namespaces")
            .iter()
            .any(|item| item["namespace"] == "ops")
    );
    assert!(
        status_all["namespaces"]
            .as_array()
            .expect("namespaces")
            .iter()
            .any(|item| item["namespace"] == "research")
    );

    let prune_dry = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "memory",
            "prune",
            "--max-namespaces",
            "1",
            "--dry-run",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let prune_dry: Value = serde_json::from_slice(&prune_dry).expect("prune dry json");
    assert_eq!(prune_dry["ok"], true);
    assert_eq!(prune_dry["prune"]["dry_run"], true);
    assert!(prune_dry["prune"]["removed_count"].as_u64().unwrap_or(0) >= 1);

    let prune_apply = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "memory",
            "prune",
            "--max-namespaces",
            "1",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let prune_apply: Value = serde_json::from_slice(&prune_apply).expect("prune apply json");
    assert_eq!(prune_apply["ok"], true);
    assert_eq!(prune_apply["prune"]["dry_run"], false);
}

#[test]
#[allow(deprecated)]
fn memory_prune_requires_policy_options() {
    let temp = tempdir().expect("tempdir");
    let output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args(["--project-state", "--json", "memory", "prune"])
        .assert()
        .failure()
        .code(7)
        .get_output()
        .stdout
        .clone();
    let output: Value = serde_json::from_slice(&output).expect("error json");
    assert_eq!(output["ok"], false);
    assert_eq!(output["error"]["code"], "validation");
}

#[test]
#[allow(deprecated)]
fn memory_prune_document_quota_removes_heavy_namespace() {
    let temp = tempdir().expect("tempdir");
    let heavy_root = temp.path().join("heavy");
    let light_root = temp.path().join("light");
    std::fs::create_dir_all(&heavy_root).expect("create heavy");
    std::fs::create_dir_all(&light_root).expect("create light");
    std::fs::write(heavy_root.join("a.md"), "heavy alpha").expect("write heavy a");
    std::fs::write(heavy_root.join("b.md"), "heavy beta").expect("write heavy b");
    std::fs::write(heavy_root.join("c.md"), "heavy gamma").expect("write heavy c");
    std::fs::write(light_root.join("a.md"), "light alpha").expect("write light a");

    Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "memory",
            "index",
            "--path",
            "heavy",
            "--namespace",
            "heavy",
        ])
        .assert()
        .success();
    Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "memory",
            "index",
            "--path",
            "light",
            "--namespace",
            "light",
        ])
        .assert()
        .success();

    let dry = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "memory",
            "prune",
            "--max-documents-per-namespace",
            "2",
            "--dry-run",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let dry: Value = serde_json::from_slice(&dry).expect("dry prune json");
    assert_eq!(dry["ok"], true);
    assert_eq!(
        dry["prune"]["removed_namespaces"],
        serde_json::json!(["heavy"])
    );
    assert_eq!(
        dry["prune"]["removed_due_to_max_documents_per_namespace"],
        serde_json::json!(["heavy"])
    );

    let apply = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "memory",
            "prune",
            "--max-documents-per-namespace",
            "2",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let apply: Value = serde_json::from_slice(&apply).expect("apply prune json");
    assert_eq!(apply["ok"], true);
    assert_eq!(
        apply["prune"]["removed_namespaces"],
        serde_json::json!(["heavy"])
    );

    let status_all = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "memory",
            "status",
            "--all-namespaces",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let status_all: Value = serde_json::from_slice(&status_all).expect("status all json");
    let namespaces = status_all["namespaces"]
        .as_array()
        .expect("namespaces array");
    assert!(namespaces.iter().any(|entry| entry["namespace"] == "light"));
    assert!(!namespaces.iter().any(|entry| entry["namespace"] == "heavy"));
}

#[test]
#[allow(deprecated)]
fn memory_policy_set_get_and_apply_flow() {
    let temp = tempdir().expect("tempdir");
    let heavy_root = temp.path().join("heavy");
    std::fs::create_dir_all(&heavy_root).expect("create heavy");
    std::fs::write(heavy_root.join("a.md"), "heavy alpha").expect("write heavy a");
    std::fs::write(heavy_root.join("b.md"), "heavy beta").expect("write heavy b");
    std::fs::write(heavy_root.join("c.md"), "heavy gamma").expect("write heavy c");
    Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "memory",
            "index",
            "--path",
            "heavy",
            "--namespace",
            "heavy",
        ])
        .assert()
        .success();

    let set_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "memory",
            "policy",
            "set",
            "--enabled",
            "true",
            "--max-documents-per-namespace",
            "2",
            "--min-interval-minutes",
            "60",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let set_json: Value = serde_json::from_slice(&set_output).expect("set json");
    assert_eq!(set_json["ok"], true);
    assert_eq!(set_json["policy"]["enabled"], true);
    assert_eq!(set_json["policy"]["max_documents_per_namespace"], 2);

    let get_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args(["--project-state", "--json", "memory", "policy", "get"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let get_json: Value = serde_json::from_slice(&get_output).expect("get json");
    assert_eq!(get_json["ok"], true);
    assert_eq!(get_json["policy"]["enabled"], true);

    let apply_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args(["--project-state", "--json", "memory", "policy", "apply"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let apply_json: Value = serde_json::from_slice(&apply_output).expect("apply json");
    assert_eq!(apply_json["ok"], true);
    assert_eq!(apply_json["applied"], true);
    assert_eq!(apply_json["skipped"], false);
    assert_eq!(
        apply_json["prune"]["removed_due_to_max_documents_per_namespace"],
        serde_json::json!(["heavy"])
    );

    let second_apply_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args(["--project-state", "--json", "memory", "policy", "apply"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let second_apply_json: Value =
        serde_json::from_slice(&second_apply_output).expect("second apply json");
    assert_eq!(second_apply_json["ok"], true);
    assert_eq!(second_apply_json["applied"], false);
    assert_eq!(second_apply_json["skipped"], true);
    assert_eq!(second_apply_json["reason"], "interval_not_elapsed");

    let forced_apply_output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "memory",
            "policy",
            "apply",
            "--force",
            "--dry-run",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let forced_apply_json: Value =
        serde_json::from_slice(&forced_apply_output).expect("forced apply json");
    assert_eq!(forced_apply_json["ok"], true);
    assert_eq!(forced_apply_json["applied"], true);
    assert_eq!(forced_apply_json["dry_run"], true);
}

#[test]
#[allow(deprecated)]
fn memory_policy_set_enabled_without_limits_fails() {
    let temp = tempdir().expect("tempdir");
    let output = Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--project-state",
            "--json",
            "memory",
            "policy",
            "set",
            "--enabled",
            "true",
            "--clear-limits",
        ])
        .assert()
        .failure()
        .code(7)
        .get_output()
        .stdout
        .clone();
    let output: Value = serde_json::from_slice(&output).expect("error json");
    assert_eq!(output["ok"], false);
    assert_eq!(output["error"]["code"], "validation");
}
