use std::{
    fs,
    path::PathBuf,
    process,
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use mosaic_session_core::{
    FileSessionStore, SessionRecord, SessionStore, TranscriptMessage, TranscriptRole,
};

static COUNTER: AtomicU64 = AtomicU64::new(0);

fn temp_dir(label: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock should be after epoch")
        .as_nanos();
    let count = COUNTER.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "mosaic-session-integration-{label}-{}-{nanos}-{count}",
        process::id()
    ))
}

#[test]
fn file_session_store_roundtrips_transcript_and_metadata() {
    let dir = temp_dir("session");
    let store = FileSessionStore::new(&dir);
    let mut session = SessionRecord::new("demo", "Demo", "mock", "mock", "mock");
    session.gateway.route = "gateway.local/demo".to_owned();
    session.transcript.push(TranscriptMessage::new(
        TranscriptRole::User,
        "hello mosaic".to_owned(),
        None,
    ));

    store.save(&session).expect("session should save");
    let loaded = store
        .load("demo")
        .expect("session load should succeed")
        .expect("session should exist");

    assert_eq!(loaded.id, "demo");
    assert_eq!(loaded.gateway.route, "gateway.local/demo");
    assert_eq!(loaded.transcript.len(), 1);
    assert_eq!(loaded.transcript[0].content, "hello mosaic");

    fs::remove_dir_all(dir).ok();
}
