use assert_cmd::Command;
use serde_json::Value;

#[test]
#[allow(deprecated)]
fn docs_lists_topics_in_json() {
    let output = Command::cargo_bin("mosaic")
        .expect("binary")
        .args(["--json", "docs"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output).expect("json");
    assert_eq!(json["ok"], true);
    let topics = json["topics"].as_array().expect("topics array");
    assert!(topics.iter().any(|item| item["topic"] == "cli"));
    assert!(topics.iter().any(|item| item["topic"] == "gateway"));
}

#[test]
#[allow(deprecated)]
fn docs_topic_returns_expected_url() {
    let output = Command::cargo_bin("mosaic")
        .expect("binary")
        .args(["--json", "docs", "channels"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output).expect("json");
    assert_eq!(json["ok"], true);
    assert_eq!(json["topic"], "channels");
    assert_eq!(json["url"], "cli/docs/channels-slack.md");
}

#[test]
#[allow(deprecated)]
fn dns_resolve_localhost_returns_addresses() {
    let output = Command::cargo_bin("mosaic")
        .expect("binary")
        .args(["--json", "dns", "resolve", "localhost", "--port", "443"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output).expect("json");
    assert_eq!(json["ok"], true);
    assert_eq!(json["host"], "localhost");
    let addresses = json["addresses"].as_array().expect("addresses array");
    assert!(!addresses.is_empty());
}

#[test]
#[allow(deprecated)]
fn dns_resolve_invalid_host_returns_network_error() {
    let output = Command::cargo_bin("mosaic")
        .expect("binary")
        .args(["--json", "dns", "resolve", "nonexistent.invalid"])
        .assert()
        .failure()
        .code(4)
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output).expect("json");
    assert_eq!(json["ok"], false);
    assert_eq!(json["error"]["code"], "network");
}
