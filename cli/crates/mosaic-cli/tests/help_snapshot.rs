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

#[test]
#[allow(deprecated)]
fn pairing_help_lists_request_subcommand() {
    let output = Command::cargo_bin("mosaic")
        .expect("binary")
        .args(["pairing", "--help"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let actual = String::from_utf8(output).expect("stdout is utf8");
    assert!(
        actual.contains("request"),
        "pairing --help should expose request subcommand for local pairing workflow"
    );
}

#[test]
#[allow(deprecated)]
fn channels_help_matches_snapshot() {
    let output = Command::cargo_bin("mosaic")
        .expect("binary")
        .args(["channels", "--help"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let actual = String::from_utf8(output).expect("stdout is utf8");
    let expected = include_str!("snapshots/channels_help.txt");
    assert_eq!(actual, expected);
}

#[test]
#[allow(deprecated)]
fn gateway_help_matches_snapshot() {
    let output = Command::cargo_bin("mosaic")
        .expect("binary")
        .args(["gateway", "--help"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let actual = String::from_utf8(output).expect("stdout is utf8");
    let expected = include_str!("snapshots/gateway_help.txt");
    assert_eq!(actual, expected);
}
