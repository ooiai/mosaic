use std::{
    fs,
    path::PathBuf,
    process,
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use mosaic_scheduler_core::{CronRegistration, CronStore, FileCronStore};

static COUNTER: AtomicU64 = AtomicU64::new(0);

fn temp_dir(label: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock should be after epoch")
        .as_nanos();
    let count = COUNTER.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "mosaic-scheduler-integration-{label}-{}-{nanos}-{count}",
        process::id()
    ))
}

#[test]
fn file_cron_store_roundtrips_multiple_registrations() {
    let dir = temp_dir("cron");
    let store = FileCronStore::new(&dir);
    store
        .save(&CronRegistration::new("daily", "0 9 * * *", "daily report"))
        .expect("daily cron should save");
    store
        .save(&CronRegistration::new(
            "weekly",
            "0 9 * * 1",
            "weekly report",
        ))
        .expect("weekly cron should save");

    let registrations = store.list().expect("cron list should succeed");
    assert_eq!(registrations.len(), 2);
    assert_eq!(registrations[0].id, "daily");
    assert_eq!(registrations[1].id, "weekly");

    fs::remove_dir_all(dir).ok();
}
