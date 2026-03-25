use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CronRegistration {
    pub id: String,
    pub schedule: String,
    pub input: String,
    pub session_id: Option<String>,
    pub profile: Option<String>,
    pub skill: Option<String>,
    pub workflow: Option<String>,
    pub created_at: DateTime<Utc>,
    pub last_triggered_at: Option<DateTime<Utc>>,
}

impl CronRegistration {
    pub fn new(
        id: impl Into<String>,
        schedule: impl Into<String>,
        input: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            schedule: schedule.into(),
            input: input.into(),
            session_id: None,
            profile: None,
            skill: None,
            workflow: None,
            created_at: Utc::now(),
            last_triggered_at: None,
        }
    }

    pub fn mark_triggered(&mut self) {
        self.last_triggered_at = Some(Utc::now());
    }
}

pub trait CronStore: Send + Sync {
    fn load(&self, id: &str) -> Result<Option<CronRegistration>>;
    fn save(&self, registration: &CronRegistration) -> Result<()>;
    fn list(&self) -> Result<Vec<CronRegistration>>;
}

#[derive(Debug, Clone)]
pub struct FileCronStore {
    root: PathBuf,
}

impl FileCronStore {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    fn path_for(&self, id: &str) -> PathBuf {
        self.root.join(format!("{id}.json"))
    }

    fn ensure_root(&self) -> Result<()> {
        fs::create_dir_all(&self.root)?;
        Ok(())
    }
}

impl CronStore for FileCronStore {
    fn load(&self, id: &str) -> Result<Option<CronRegistration>> {
        let path = self.path_for(id);
        if !path.exists() {
            return Ok(None);
        }

        let bytes = fs::read(path)?;
        Ok(Some(serde_json::from_slice(&bytes)?))
    }

    fn save(&self, registration: &CronRegistration) -> Result<()> {
        self.ensure_root()?;
        let path = self.path_for(&registration.id);
        fs::write(path, serde_json::to_vec_pretty(registration)?)?;
        Ok(())
    }

    fn list(&self) -> Result<Vec<CronRegistration>> {
        if !Path::new(&self.root).exists() {
            return Ok(Vec::new());
        }

        let mut registrations: Vec<CronRegistration> = Vec::new();
        for entry in fs::read_dir(&self.root)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|value| value.to_str()) != Some("json") {
                continue;
            }

            let bytes = fs::read(path)?;
            registrations.push(serde_json::from_slice(&bytes)?);
        }

        registrations.sort_by(|left, right| left.id.cmp(&right.id));
        Ok(registrations)
    }
}

#[cfg(test)]
mod tests {
    use std::{
        process,
        sync::atomic::{AtomicU64, Ordering},
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::{CronRegistration, CronStore, FileCronStore};

    static COUNTER: AtomicU64 = AtomicU64::new(0);

    fn temp_dir(label: &str) -> std::path::PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock should be after epoch")
            .as_nanos();
        let count = COUNTER.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!(
            "mosaic-scheduler-core-{label}-{}-{nanos}-{count}",
            process::id()
        ))
    }

    #[test]
    fn file_store_roundtrips_registration() {
        let store = FileCronStore::new(temp_dir("roundtrip"));
        let mut registration = CronRegistration::new("daily", "0 * * * *", "check status");
        registration.session_id = Some("demo".to_owned());
        registration.profile = Some("mock".to_owned());

        store.save(&registration).expect("save should succeed");
        let loaded = store
            .load("daily")
            .expect("load should succeed")
            .expect("registration should exist");

        assert_eq!(loaded.id, "daily");
        assert_eq!(loaded.schedule, "0 * * * *");
        assert_eq!(loaded.session_id.as_deref(), Some("demo"));
        assert_eq!(loaded.profile.as_deref(), Some("mock"));
    }

    #[test]
    fn file_store_lists_sorted_registrations() {
        let store = FileCronStore::new(temp_dir("list"));
        store
            .save(&CronRegistration::new("z-job", "* * * * *", "z"))
            .expect("save should succeed");
        store
            .save(&CronRegistration::new("a-job", "* * * * *", "a"))
            .expect("save should succeed");

        let ids = store
            .list()
            .expect("list should succeed")
            .into_iter()
            .map(|registration| registration.id)
            .collect::<Vec<_>>();

        assert_eq!(ids, vec!["a-job", "z-job"]);
    }

    #[test]
    fn mark_triggered_sets_timestamp() {
        let mut registration = CronRegistration::new("nightly", "0 0 * * *", "run nightly");
        assert!(registration.last_triggered_at.is_none());

        registration.mark_triggered();

        assert!(registration.last_triggered_at.is_some());
    }
}
