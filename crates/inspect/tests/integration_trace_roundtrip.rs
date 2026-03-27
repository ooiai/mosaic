use std::{
    fs,
    path::PathBuf,
    process,
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use mosaic_inspect::{IngressTrace, RunTrace};

static COUNTER: AtomicU64 = AtomicU64::new(0);

fn temp_dir(label: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock should be after epoch")
        .as_nanos();
    let count = COUNTER.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "mosaic-inspect-integration-{label}-{}-{nanos}-{count}",
        process::id()
    ))
}

#[test]
fn persists_and_recovers_trace_with_ingress_metadata() {
    let dir = temp_dir("trace");
    let mut trace = RunTrace::new("inspect me".to_owned());
    trace.bind_session("demo");
    trace.bind_ingress(IngressTrace {
        kind: "webchat".to_owned(),
        channel: Some("webchat".to_owned()),
        source: Some("integration-test".to_owned()),
        remote_addr: Some("127.0.0.1".to_owned()),
        display_name: Some("Operator".to_owned()),
        actor_id: Some("42".to_owned()),
        thread_id: None,
        thread_title: None,
        reply_target: Some("webchat:demo".to_owned()),
        gateway_url: Some("http://127.0.0.1:8080".to_owned()),
    });
    trace.finish_ok("done".to_owned());

    let path = trace.save_to_dir(&dir).expect("trace should save");
    let bytes = fs::read(path).expect("saved trace should be readable");
    let loaded: RunTrace = serde_json::from_slice(&bytes).expect("trace should deserialize");

    assert_eq!(loaded.status(), "success");
    assert_eq!(loaded.session_id.as_deref(), Some("demo"));
    assert_eq!(
        loaded
            .ingress
            .as_ref()
            .and_then(|ingress| ingress.channel.as_deref()),
        Some("webchat")
    );

    fs::remove_dir_all(dir).ok();
}
