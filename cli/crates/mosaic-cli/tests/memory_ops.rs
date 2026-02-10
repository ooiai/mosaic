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
