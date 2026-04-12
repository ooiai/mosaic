use std::{
    fs,
    path::PathBuf,
    process,
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use mosaic_memory::{
    FileMemoryStore, MemoryEntry, MemoryEntryKind, MemoryStore, SessionMemoryRecord,
};

static COUNTER: AtomicU64 = AtomicU64::new(0);

fn temp_dir(label: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock should be after epoch")
        .as_nanos();
    let count = COUNTER.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "mosaic-memory-integration-{label}-{}-{nanos}-{count}",
        process::id()
    ))
}

#[test]
fn file_memory_store_roundtrips_session_memory_on_disk() {
    let dir = temp_dir("memory");
    let store = FileMemoryStore::new(&dir);
    let mut record = SessionMemoryRecord::new("demo");
    record.entries.push(MemoryEntry::new(
        MemoryEntryKind::Summary,
        "Operator prefers concise status updates.",
        vec!["session".to_owned(), "summary".to_owned()],
    ));

    store.save_session(&record).expect("memory should save");
    let loaded = store
        .load_session("demo")
        .expect("memory load should succeed")
        .expect("memory session should exist");

    assert_eq!(loaded.session_id, "demo");
    assert_eq!(loaded.entries.len(), 1);
    assert_eq!(
        loaded.entries[0].content,
        "Operator prefers concise status updates."
    );

    fs::remove_dir_all(dir).ok();
}
