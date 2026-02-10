use assert_cmd::Command;
use tempfile::tempdir;

#[test]
#[allow(deprecated)]
fn ask_without_setup_returns_config_exit_code() {
    let temp = tempdir().expect("tempdir");
    Command::cargo_bin("mosaic")
        .expect("binary")
        .current_dir(temp.path())
        .args(["--project-state", "ask", "hello"])
        .assert()
        .failure()
        .code(2);
}
