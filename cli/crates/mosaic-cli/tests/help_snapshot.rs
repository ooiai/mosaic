use assert_cmd::Command;

#[test]
#[allow(deprecated)]
fn root_help_matches_snapshot() {
    let output = Command::cargo_bin("mosaic")
        .expect("binary")
        .arg("--help")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let actual = String::from_utf8(output).expect("stdout is utf8");
    let expected = include_str!("snapshots/help.txt");
    assert_eq!(actual, expected);
}
