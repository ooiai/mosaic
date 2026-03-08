use std::cmp::Reverse;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use walkdir::WalkDir;

use mosaic_core::error::{MosaicError, Result};

const DEFAULT_MAX_FILES: usize = 500;
const DEFAULT_MAX_FILE_SIZE: usize = 256 * 1024;
const DEFAULT_MAX_CONTENT_BYTES: usize = 16 * 1024;
const DEFAULT_SEARCH_LIMIT: usize = 20;
const CURRENT_MEMORY_CLEANUP_POLICY_VERSION: u32 = 1;
pub const MEMORY_DEFAULT_NAMESPACE: &str = "default";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryStatus {
    pub indexed_documents: usize,
    pub last_indexed_at: Option<DateTime<Utc>>,
    pub index_path: String,
    pub status_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryDocument {
    pub id: String,
    pub path: String,
    pub content: String,
    pub size_bytes: u64,
    pub source_modified_unix_ms: Option<i64>,
    pub indexed_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MemorySearchHit {
    pub id: String,
    pub path: String,
    pub score: usize,
    pub snippet: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct MemorySearchResult {
    pub query: String,
    pub total_hits: usize,
    pub hits: Vec<MemorySearchHit>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MemoryIndexResult {
    pub incremental: bool,
    pub indexed_documents: usize,
    pub reused_documents: usize,
    pub reindexed_documents: usize,
    pub stale_reindexed_documents: usize,
    pub removed_documents: usize,
    pub retained_missing_documents: usize,
    pub skipped_files: usize,
    pub index_path: String,
    pub status_path: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct MemoryClearResult {
    pub removed_index: bool,
    pub removed_status: bool,
    pub index_path: String,
    pub status_path: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct MemoryNamespaceStatus {
    pub namespace: String,
    pub indexed_documents: usize,
    pub last_indexed_at: Option<DateTime<Utc>>,
    pub index_path: String,
    pub status_path: String,
    pub exists: bool,
}

#[derive(Debug, Clone)]
pub struct MemoryPruneOptions {
    pub max_namespaces: Option<usize>,
    pub max_age_hours: Option<u64>,
    pub max_documents_per_namespace: Option<usize>,
    pub dry_run: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct MemoryPruneResult {
    pub dry_run: bool,
    pub evaluated_namespaces: usize,
    pub removed_count: usize,
    pub removed_namespaces: Vec<String>,
    pub kept_namespaces: Vec<String>,
    pub max_namespaces: Option<usize>,
    pub max_age_hours: Option<u64>,
    pub max_documents_per_namespace: Option<usize>,
    pub removed_due_to_max_namespaces: Vec<String>,
    pub removed_due_to_max_age_hours: Vec<String>,
    pub removed_due_to_max_documents_per_namespace: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryCleanupPolicy {
    pub version: u32,
    pub enabled: bool,
    pub max_namespaces: Option<usize>,
    pub max_age_hours: Option<u64>,
    pub max_documents_per_namespace: Option<usize>,
    pub min_interval_minutes: Option<u64>,
    pub last_run_at: Option<DateTime<Utc>>,
    pub last_run_removed_count: Option<usize>,
}

#[derive(Debug, Clone)]
pub struct MemoryCleanupPolicyStore {
    path: PathBuf,
}

#[derive(Debug, Clone)]
pub struct MemoryIndexOptions {
    pub root: PathBuf,
    pub incremental: bool,
    pub stale_after_hours: Option<u64>,
    pub retain_missing: bool,
    pub max_files: usize,
    pub max_file_size: usize,
    pub max_content_bytes: usize,
}

impl Default for MemoryIndexOptions {
    fn default() -> Self {
        Self {
            root: PathBuf::from("."),
            incremental: false,
            stale_after_hours: None,
            retain_missing: false,
            max_files: DEFAULT_MAX_FILES,
            max_file_size: DEFAULT_MAX_FILE_SIZE,
            max_content_bytes: DEFAULT_MAX_CONTENT_BYTES,
        }
    }
}

impl Default for MemoryPruneOptions {
    fn default() -> Self {
        Self {
            max_namespaces: None,
            max_age_hours: None,
            max_documents_per_namespace: None,
            dry_run: false,
        }
    }
}

impl Default for MemoryCleanupPolicy {
    fn default() -> Self {
        Self {
            version: CURRENT_MEMORY_CLEANUP_POLICY_VERSION,
            enabled: false,
            max_namespaces: None,
            max_age_hours: None,
            max_documents_per_namespace: None,
            min_interval_minutes: None,
            last_run_at: None,
            last_run_removed_count: None,
        }
    }
}

impl MemoryCleanupPolicy {
    pub fn has_limits(&self) -> bool {
        self.max_namespaces.is_some()
            || self.max_age_hours.is_some()
            || self.max_documents_per_namespace.is_some()
    }

    pub fn validate(&self) -> Result<()> {
        if self.version != CURRENT_MEMORY_CLEANUP_POLICY_VERSION {
            return Err(MosaicError::Config(format!(
                "unsupported memory cleanup policy version {}, expected {}",
                self.version, CURRENT_MEMORY_CLEANUP_POLICY_VERSION
            )));
        }
        if self.max_namespaces == Some(0) {
            return Err(MosaicError::Validation(
                "memory cleanup max_namespaces must be greater than 0".to_string(),
            ));
        }
        if self.max_age_hours == Some(0) {
            return Err(MosaicError::Validation(
                "memory cleanup max_age_hours must be greater than 0".to_string(),
            ));
        }
        if self.max_documents_per_namespace == Some(0) {
            return Err(MosaicError::Validation(
                "memory cleanup max_documents_per_namespace must be greater than 0".to_string(),
            ));
        }
        if self.min_interval_minutes == Some(0) {
            return Err(MosaicError::Validation(
                "memory cleanup min_interval_minutes must be greater than 0".to_string(),
            ));
        }
        if self.enabled && !self.has_limits() {
            return Err(MosaicError::Validation(
                "memory cleanup policy is enabled but no limits are configured".to_string(),
            ));
        }
        Ok(())
    }
}

impl MemoryCleanupPolicyStore {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn load_or_default(&self) -> Result<MemoryCleanupPolicy> {
        if !self.path.exists() {
            return Ok(MemoryCleanupPolicy::default());
        }
        let raw = std::fs::read_to_string(&self.path)?;
        let mut policy: MemoryCleanupPolicy = toml::from_str(&raw).map_err(|err| {
            MosaicError::Config(format!(
                "invalid memory cleanup policy {}: {err}",
                self.path.display()
            ))
        })?;
        if policy.version == 0 {
            policy.version = CURRENT_MEMORY_CLEANUP_POLICY_VERSION;
        }
        policy.validate()?;
        Ok(policy)
    }

    pub fn save(&self, policy: &MemoryCleanupPolicy) -> Result<()> {
        let mut policy = policy.clone();
        policy.version = CURRENT_MEMORY_CLEANUP_POLICY_VERSION;
        policy.validate()?;
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let rendered = toml::to_string_pretty(&policy)?;
        std::fs::write(&self.path, rendered)?;
        Ok(())
    }

    pub fn mark_run(&self, removed_count: usize) -> Result<MemoryCleanupPolicy> {
        let mut policy = self.load_or_default()?;
        policy.last_run_at = Some(Utc::now());
        policy.last_run_removed_count = Some(removed_count);
        self.save(&policy)?;
        Ok(policy)
    }
}

#[derive(Debug, Clone)]
pub struct MemoryStore {
    index_path: PathBuf,
    status_path: PathBuf,
}

impl MemoryStore {
    pub fn new(index_path: PathBuf, status_path: PathBuf) -> Self {
        Self {
            index_path,
            status_path,
        }
    }

    pub fn index(&self, options: MemoryIndexOptions) -> Result<MemoryIndexResult> {
        let root = canonicalize_root(&options.root)?;
        let existing_documents = if options.incremental {
            self.load_documents()?
        } else {
            Vec::new()
        };
        let existing_by_path = existing_documents
            .into_iter()
            .map(|doc| (doc.path.clone(), doc))
            .collect::<HashMap<_, _>>();
        let mut documents = Vec::new();
        let mut skipped = 0usize;
        let mut reused = 0usize;
        let mut reindexed = 0usize;
        let mut stale_reindexed = 0usize;

        for entry in WalkDir::new(&root).into_iter().flatten() {
            if documents.len() >= options.max_files {
                break;
            }
            let path = entry.path();
            if path.is_dir() {
                continue;
            }
            if should_skip(path) {
                continue;
            }

            let metadata = match std::fs::metadata(path) {
                Ok(metadata) => metadata,
                Err(_) => {
                    skipped += 1;
                    continue;
                }
            };
            if metadata.len() as usize > options.max_file_size {
                skipped += 1;
                continue;
            }

            let source_modified_unix_ms = metadata.modified().ok().and_then(system_time_to_unix_ms);

            let relative = path
                .strip_prefix(&root)
                .ok()
                .map(|value| value.to_string_lossy().to_string())
                .unwrap_or_else(|| path.to_string_lossy().to_string());

            if options.incremental
                && let Some(existing) = existing_by_path.get(&relative)
                && existing.size_bytes == metadata.len()
                && existing.source_modified_unix_ms == source_modified_unix_ms
            {
                if is_stale(existing, options.stale_after_hours) {
                    stale_reindexed += 1;
                } else {
                    documents.push(existing.clone());
                    reused += 1;
                    continue;
                }
            }

            let content = match std::fs::read_to_string(path) {
                Ok(content) => content,
                Err(_) => {
                    skipped += 1;
                    if options.incremental
                        && let Some(existing) = existing_by_path.get(&relative)
                    {
                        documents.push(existing.clone());
                        reused += 1;
                    }
                    continue;
                }
            };
            if content.trim().is_empty() {
                skipped += 1;
                if options.incremental
                    && let Some(existing) = existing_by_path.get(&relative)
                {
                    documents.push(existing.clone());
                    reused += 1;
                }
                continue;
            }

            let truncated = truncate_to_bytes(&content, options.max_content_bytes);
            let existing_id = existing_by_path.get(&relative).map(|doc| doc.id.clone());
            documents.push(MemoryDocument {
                id: existing_id.unwrap_or_else(|| format!("mem_{}", uuid::Uuid::new_v4())),
                path: relative,
                content: truncated,
                size_bytes: metadata.len(),
                source_modified_unix_ms,
                indexed_at: Utc::now(),
            });
            reindexed += 1;
        }

        documents.sort_by_key(|doc| Reverse(doc.path.clone()));
        let mut retained_missing_documents = 0usize;
        let removed_documents = if options.incremental {
            let indexed_paths = documents
                .iter()
                .map(|doc| doc.path.clone())
                .collect::<HashSet<_>>();
            let missing_paths = existing_by_path
                .keys()
                .filter(|path| !indexed_paths.contains(*path))
                .cloned()
                .collect::<Vec<_>>();
            if options.retain_missing {
                for path in &missing_paths {
                    if let Some(existing) = existing_by_path.get(path) {
                        documents.push(existing.clone());
                        retained_missing_documents += 1;
                    }
                }
                0
            } else {
                missing_paths.len()
            }
        } else {
            0
        };
        documents.sort_by_key(|doc| Reverse(doc.path.clone()));
        self.save_documents(&documents)?;
        let status = MemoryStatus {
            indexed_documents: documents.len(),
            last_indexed_at: Some(Utc::now()),
            index_path: self.index_path.display().to_string(),
            status_path: self.status_path.display().to_string(),
        };
        self.save_status(&status)?;

        Ok(MemoryIndexResult {
            incremental: options.incremental,
            indexed_documents: status.indexed_documents,
            reused_documents: if options.incremental { reused } else { 0 },
            reindexed_documents: if options.incremental {
                reindexed
            } else {
                status.indexed_documents
            },
            stale_reindexed_documents: if options.incremental {
                stale_reindexed
            } else {
                0
            },
            removed_documents,
            retained_missing_documents,
            skipped_files: skipped,
            index_path: status.index_path,
            status_path: status.status_path,
        })
    }

    pub fn status(&self) -> Result<MemoryStatus> {
        if !self.status_path.exists() {
            return Ok(MemoryStatus {
                indexed_documents: 0,
                last_indexed_at: None,
                index_path: self.index_path.display().to_string(),
                status_path: self.status_path.display().to_string(),
            });
        }
        let raw = std::fs::read_to_string(&self.status_path)?;
        let mut status = serde_json::from_str::<MemoryStatus>(&raw).map_err(|err| {
            MosaicError::Validation(format!(
                "invalid memory status JSON {}: {err}",
                self.status_path.display()
            ))
        })?;
        status.index_path = self.index_path.display().to_string();
        status.status_path = self.status_path.display().to_string();
        Ok(status)
    }

    pub fn search(&self, query: &str, limit: Option<usize>) -> Result<MemorySearchResult> {
        let query = query.trim();
        if query.is_empty() {
            return Err(MosaicError::Validation(
                "memory search query cannot be empty".to_string(),
            ));
        }
        let docs = self.load_documents()?;
        let limit = limit.unwrap_or(DEFAULT_SEARCH_LIMIT);
        let query_tokens = tokenize_query(query);

        let mut hits = docs
            .into_iter()
            .filter_map(|doc| {
                let score = relevance_score(&doc.content, &doc.path, query, &query_tokens);
                if score == 0 {
                    return None;
                }
                let snippet = find_snippet(&doc.content, query);
                Some(MemorySearchHit {
                    id: doc.id,
                    path: doc.path,
                    score,
                    snippet,
                })
            })
            .collect::<Vec<_>>();

        hits.sort_by(|lhs, rhs| rhs.score.cmp(&lhs.score).then(lhs.path.cmp(&rhs.path)));
        let total_hits = hits.len();
        if hits.len() > limit {
            hits.truncate(limit);
        }

        Ok(MemorySearchResult {
            query: query.to_string(),
            total_hits,
            hits,
        })
    }

    pub fn clear(&self) -> Result<MemoryClearResult> {
        let removed_index = remove_if_exists(&self.index_path)?;
        let removed_status = remove_if_exists(&self.status_path)?;
        Ok(MemoryClearResult {
            removed_index,
            removed_status,
            index_path: self.index_path.display().to_string(),
            status_path: self.status_path.display().to_string(),
        })
    }

    fn save_documents(&self, docs: &[MemoryDocument]) -> Result<()> {
        if let Some(parent) = self.index_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut lines = String::new();
        for doc in docs {
            let line = serde_json::to_string(doc).map_err(|err| {
                MosaicError::Validation(format!("failed to encode memory document JSON: {err}"))
            })?;
            lines.push_str(&line);
            lines.push('\n');
        }
        std::fs::write(&self.index_path, lines)?;
        Ok(())
    }

    fn save_status(&self, status: &MemoryStatus) -> Result<()> {
        if let Some(parent) = self.status_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let encoded = serde_json::to_string_pretty(status).map_err(|err| {
            MosaicError::Validation(format!("failed to encode memory status JSON: {err}"))
        })?;
        std::fs::write(&self.status_path, encoded)?;
        Ok(())
    }

    fn load_documents(&self) -> Result<Vec<MemoryDocument>> {
        if !self.index_path.exists() {
            return Ok(Vec::new());
        }
        let raw = std::fs::read_to_string(&self.index_path)?;
        let mut docs = Vec::new();
        for line in raw.lines() {
            if line.trim().is_empty() {
                continue;
            }
            let doc = serde_json::from_str::<MemoryDocument>(line).map_err(|err| {
                MosaicError::Validation(format!(
                    "invalid memory document JSON {}: {err}",
                    self.index_path.display()
                ))
            })?;
            docs.push(doc);
        }
        Ok(docs)
    }
}

pub fn memory_index_path(data_dir: &Path) -> PathBuf {
    memory_index_path_for_namespace(data_dir, MEMORY_DEFAULT_NAMESPACE)
}

pub fn memory_status_path(data_dir: &Path) -> PathBuf {
    memory_status_path_for_namespace(data_dir, MEMORY_DEFAULT_NAMESPACE)
}

pub fn memory_cleanup_policy_path(policy_dir: &Path) -> PathBuf {
    policy_dir.join("memory.toml")
}

pub fn memory_index_path_for_namespace(data_dir: &Path, namespace: &str) -> PathBuf {
    if namespace == MEMORY_DEFAULT_NAMESPACE {
        data_dir.join("memory/index.jsonl")
    } else {
        data_dir
            .join("memory/namespaces")
            .join(namespace)
            .join("index.jsonl")
    }
}

pub fn memory_status_path_for_namespace(data_dir: &Path, namespace: &str) -> PathBuf {
    if namespace == MEMORY_DEFAULT_NAMESPACE {
        data_dir.join("memory/status.json")
    } else {
        data_dir
            .join("memory/namespaces")
            .join(namespace)
            .join("status.json")
    }
}

pub fn list_memory_namespace_statuses(data_dir: &Path) -> Result<Vec<MemoryNamespaceStatus>> {
    let mut namespaces = vec![MEMORY_DEFAULT_NAMESPACE.to_string()];
    let namespaces_root = data_dir.join("memory/namespaces");
    if namespaces_root.exists() {
        for entry in std::fs::read_dir(&namespaces_root)? {
            let entry = entry?;
            if entry.file_type()?.is_dir() {
                let raw = entry.file_name().to_string_lossy().to_string();
                let namespace = raw.trim().to_lowercase();
                if !namespace.is_empty() && namespace != MEMORY_DEFAULT_NAMESPACE {
                    namespaces.push(namespace);
                }
            }
        }
    }
    namespaces.sort();
    namespaces.dedup();

    let mut results = Vec::new();
    for namespace in namespaces {
        let store = MemoryStore::new(
            memory_index_path_for_namespace(data_dir, &namespace),
            memory_status_path_for_namespace(data_dir, &namespace),
        );
        let status = store.status()?;
        let index_exists = PathBuf::from(&status.index_path).exists();
        let status_exists = PathBuf::from(&status.status_path).exists();
        let exists = index_exists || status_exists;
        if namespace != MEMORY_DEFAULT_NAMESPACE && !exists {
            continue;
        }
        results.push(MemoryNamespaceStatus {
            namespace,
            indexed_documents: status.indexed_documents,
            last_indexed_at: status.last_indexed_at,
            index_path: status.index_path,
            status_path: status.status_path,
            exists,
        });
    }
    results.sort_by(|lhs, rhs| lhs.namespace.cmp(&rhs.namespace));
    Ok(results)
}

pub fn prune_memory_namespaces(
    data_dir: &Path,
    options: MemoryPruneOptions,
) -> Result<MemoryPruneResult> {
    if options.max_namespaces == Some(0) {
        return Err(MosaicError::Validation(
            "max_namespaces must be greater than 0".to_string(),
        ));
    }
    if options.max_age_hours == Some(0) {
        return Err(MosaicError::Validation(
            "max_age_hours must be greater than 0".to_string(),
        ));
    }
    if options.max_documents_per_namespace == Some(0) {
        return Err(MosaicError::Validation(
            "max_documents_per_namespace must be greater than 0".to_string(),
        ));
    }

    let mut statuses = list_memory_namespace_statuses(data_dir)?
        .into_iter()
        .filter(|entry| entry.namespace != MEMORY_DEFAULT_NAMESPACE)
        .collect::<Vec<_>>();
    let evaluated_namespaces = statuses.len();
    let now = Utc::now();

    let mut remove_set = HashSet::new();
    let mut removed_due_to_max_age_hours = HashSet::new();
    if let Some(max_age_hours) = options.max_age_hours {
        let max_age_hours = i64::try_from(max_age_hours).unwrap_or(i64::MAX);
        for status in &statuses {
            let is_old = match status.last_indexed_at {
                Some(value) => (now - value).num_hours() >= max_age_hours,
                None => true,
            };
            if is_old {
                remove_set.insert(status.namespace.clone());
                removed_due_to_max_age_hours.insert(status.namespace.clone());
            }
        }
    }

    let mut removed_due_to_max_namespaces = HashSet::new();
    if let Some(max_namespaces) = options.max_namespaces
        && max_namespaces < statuses.len()
    {
        statuses.sort_by(
            |lhs, rhs| match (lhs.last_indexed_at, rhs.last_indexed_at) {
                (Some(a), Some(b)) => b.cmp(&a),
                (Some(_), None) => std::cmp::Ordering::Less,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                (None, None) => lhs.namespace.cmp(&rhs.namespace),
            },
        );
        for status in statuses.iter().skip(max_namespaces) {
            remove_set.insert(status.namespace.clone());
            removed_due_to_max_namespaces.insert(status.namespace.clone());
        }
    }

    let mut removed_due_to_max_documents_per_namespace = HashSet::new();
    if let Some(max_documents_per_namespace) = options.max_documents_per_namespace {
        for status in &statuses {
            if status.indexed_documents > max_documents_per_namespace {
                remove_set.insert(status.namespace.clone());
                removed_due_to_max_documents_per_namespace.insert(status.namespace.clone());
            }
        }
    }

    let mut removed_namespaces = remove_set.into_iter().collect::<Vec<_>>();
    removed_namespaces.sort();
    if !options.dry_run {
        for namespace in &removed_namespaces {
            let root = data_dir.join("memory/namespaces").join(namespace);
            if root.exists() {
                std::fs::remove_dir_all(root)?;
            }
        }
    }

    let removed_set = removed_namespaces.iter().cloned().collect::<HashSet<_>>();
    let mut kept_namespaces = statuses
        .iter()
        .map(|status| status.namespace.clone())
        .filter(|namespace| !removed_set.contains(namespace))
        .collect::<Vec<_>>();
    kept_namespaces.sort();
    let mut removed_due_to_max_namespaces = removed_due_to_max_namespaces
        .into_iter()
        .collect::<Vec<_>>();
    removed_due_to_max_namespaces.sort();
    let mut removed_due_to_max_age_hours =
        removed_due_to_max_age_hours.into_iter().collect::<Vec<_>>();
    removed_due_to_max_age_hours.sort();
    let mut removed_due_to_max_documents_per_namespace = removed_due_to_max_documents_per_namespace
        .into_iter()
        .collect::<Vec<_>>();
    removed_due_to_max_documents_per_namespace.sort();

    Ok(MemoryPruneResult {
        dry_run: options.dry_run,
        evaluated_namespaces,
        removed_count: removed_namespaces.len(),
        removed_namespaces,
        kept_namespaces,
        max_namespaces: options.max_namespaces,
        max_age_hours: options.max_age_hours,
        max_documents_per_namespace: options.max_documents_per_namespace,
        removed_due_to_max_namespaces,
        removed_due_to_max_age_hours,
        removed_due_to_max_documents_per_namespace,
    })
}

fn canonicalize_root(root: &Path) -> Result<PathBuf> {
    if root.exists() {
        return root.canonicalize().map_err(|err| {
            MosaicError::Io(format!(
                "failed to resolve memory root {}: {err}",
                root.display()
            ))
        });
    }
    Err(MosaicError::Validation(format!(
        "memory root path does not exist: {}",
        root.display()
    )))
}

fn should_skip(path: &Path) -> bool {
    let text = path.to_string_lossy();
    text.contains("/.git/")
        || text.contains("/target/")
        || text.contains("/node_modules/")
        || text.contains("/.pnpm-store/")
        || text.contains("/.mosaic/")
}

fn truncate_to_bytes(content: &str, max_bytes: usize) -> String {
    if content.len() <= max_bytes {
        return content.to_string();
    }
    let mut end = max_bytes;
    while !content.is_char_boundary(end) {
        end -= 1;
    }
    let mut truncated = content[..end].to_string();
    truncated.push_str("\n...[truncated]");
    truncated
}

fn system_time_to_unix_ms(value: SystemTime) -> Option<i64> {
    value
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|duration| i64::try_from(duration.as_millis()).ok())
}

fn is_stale(document: &MemoryDocument, stale_after_hours: Option<u64>) -> bool {
    let Some(hours) = stale_after_hours else {
        return false;
    };
    let Ok(limit) = i64::try_from(hours) else {
        return false;
    };
    let elapsed = Utc::now() - document.indexed_at;
    elapsed.num_hours() >= limit
}

fn find_snippet(content: &str, query: &str) -> String {
    let lower_query = query.to_lowercase();
    for line in content.lines() {
        if line.to_lowercase().contains(&lower_query) {
            return truncate_to_bytes(line, 160);
        }
    }
    truncate_to_bytes(content, 160)
}

fn tokenize_query(query: &str) -> Vec<String> {
    let mut tokens = query
        .split(|ch: char| !ch.is_alphanumeric() && ch != '_')
        .map(|token| token.trim().to_lowercase())
        .filter(|token| !token.is_empty())
        .collect::<Vec<_>>();
    tokens.sort();
    tokens.dedup();
    tokens
}

fn relevance_score(content: &str, path: &str, query: &str, query_tokens: &[String]) -> usize {
    let lower_query = query.to_lowercase();
    let lower_content = content.to_lowercase();
    let lower_path = path.to_lowercase();
    let mut score = 0usize;

    // Exact phrase matches in content are the strongest signal.
    let phrase_hits = lower_content.matches(&lower_query).count();
    score += phrase_hits * 8;
    // Path exact phrase match helps route to target files faster.
    let path_phrase_hits = lower_path.matches(&lower_query).count();
    score += path_phrase_hits * 12;

    for token in query_tokens {
        let content_hits = lower_content.matches(token).count();
        if content_hits > 0 {
            score += content_hits * 3;
        }
        let path_hits = lower_path.matches(token).count();
        if path_hits > 0 {
            score += path_hits * 5;
        }
    }

    score
}

fn remove_if_exists(path: &Path) -> Result<bool> {
    if path.exists() {
        std::fs::remove_file(path)?;
        Ok(true)
    } else {
        Ok(false)
    }
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::*;

    #[test]
    fn index_search_and_status_flow() {
        let temp = tempdir().expect("tempdir");
        let docs_dir = temp.path().join("docs");
        std::fs::create_dir_all(&docs_dir).expect("create docs dir");
        std::fs::write(
            docs_dir.join("a.txt"),
            "Rust CLI memory index with search support",
        )
        .expect("write a.txt");
        std::fs::write(docs_dir.join("b.txt"), "Another rust file with memory data")
            .expect("write b.txt");

        let store = MemoryStore::new(
            temp.path().join("state/memory/index.jsonl"),
            temp.path().join("state/memory/status.json"),
        );

        let result = store
            .index(MemoryIndexOptions {
                root: docs_dir,
                ..MemoryIndexOptions::default()
            })
            .expect("index");
        assert_eq!(result.indexed_documents, 2);

        let search = store.search("rust", Some(10)).expect("search");
        assert_eq!(search.total_hits, 2);
        assert_eq!(search.hits.len(), 2);

        let status = store.status().expect("status");
        assert_eq!(status.indexed_documents, 2);
        assert!(status.last_indexed_at.is_some());
    }

    #[test]
    fn search_empty_query_fails() {
        let temp = tempdir().expect("tempdir");
        let store = MemoryStore::new(
            temp.path().join("state/memory/index.jsonl"),
            temp.path().join("state/memory/status.json"),
        );
        let err = store.search("", None).expect_err("expected err");
        assert!(matches!(err, MosaicError::Validation(_)));
    }

    #[test]
    fn clear_removes_index_and_status_files() {
        let temp = tempdir().expect("tempdir");
        let docs_dir = temp.path().join("docs");
        std::fs::create_dir_all(&docs_dir).expect("create docs dir");
        std::fs::write(docs_dir.join("a.txt"), "memory clear sample").expect("write a.txt");

        let store = MemoryStore::new(
            temp.path().join("state/memory/index.jsonl"),
            temp.path().join("state/memory/status.json"),
        );
        store
            .index(MemoryIndexOptions {
                root: docs_dir,
                ..MemoryIndexOptions::default()
            })
            .expect("index");
        assert!(store.index_path.exists());
        assert!(store.status_path.exists());

        let cleared = store.clear().expect("clear");
        assert!(cleared.removed_index);
        assert!(cleared.removed_status);
        assert!(!store.index_path.exists());
        assert!(!store.status_path.exists());

        let status = store.status().expect("status after clear");
        assert_eq!(status.indexed_documents, 0);
        assert!(status.last_indexed_at.is_none());
    }

    #[test]
    fn incremental_index_reuses_unchanged_and_tracks_removed() {
        let temp = tempdir().expect("tempdir");
        let docs_dir = temp.path().join("docs");
        std::fs::create_dir_all(&docs_dir).expect("create docs dir");
        std::fs::write(docs_dir.join("a.txt"), "alpha memory item").expect("write a");
        std::fs::write(docs_dir.join("b.txt"), "beta memory item").expect("write b");

        let store = MemoryStore::new(
            temp.path().join("state/memory/index.jsonl"),
            temp.path().join("state/memory/status.json"),
        );
        let first = store
            .index(MemoryIndexOptions {
                root: docs_dir.clone(),
                ..MemoryIndexOptions::default()
            })
            .expect("first index");
        assert!(!first.incremental);
        assert_eq!(first.indexed_documents, 2);
        assert_eq!(first.reindexed_documents, 2);

        std::thread::sleep(std::time::Duration::from_millis(10));
        std::fs::write(docs_dir.join("a.txt"), "alpha memory item changed").expect("rewrite a");
        std::fs::remove_file(docs_dir.join("b.txt")).expect("remove b");
        std::fs::write(docs_dir.join("c.txt"), "gamma memory item").expect("write c");

        let second = store
            .index(MemoryIndexOptions {
                root: docs_dir,
                incremental: true,
                ..MemoryIndexOptions::default()
            })
            .expect("second index");
        assert!(second.incremental);
        assert_eq!(second.indexed_documents, 2);
        assert_eq!(second.reindexed_documents, 2);
        assert_eq!(second.reused_documents, 0);
        assert_eq!(second.stale_reindexed_documents, 0);
        assert_eq!(second.removed_documents, 1);
        assert_eq!(second.retained_missing_documents, 0);
    }

    #[test]
    fn search_scores_path_and_phrase_signals() {
        let temp = tempdir().expect("tempdir");
        let docs_dir = temp.path().join("docs");
        std::fs::create_dir_all(docs_dir.join("gateway")).expect("create docs dir");
        std::fs::write(
            docs_dir.join("gateway/retry.md"),
            "retry policy for gateway retry loops",
        )
        .expect("write retry");
        std::fs::write(
            docs_dir.join("notes.md"),
            "this doc briefly mentions gateway and retry",
        )
        .expect("write notes");

        let store = MemoryStore::new(
            temp.path().join("state/memory/index.jsonl"),
            temp.path().join("state/memory/status.json"),
        );
        store
            .index(MemoryIndexOptions {
                root: docs_dir,
                ..MemoryIndexOptions::default()
            })
            .expect("index");

        let result = store.search("gateway retry", Some(2)).expect("search");
        assert_eq!(result.total_hits, 2);
        let first = &result.hits[0];
        assert_eq!(first.path, "gateway/retry.md");
        assert!(first.score >= result.hits[1].score);
    }

    #[test]
    fn incremental_retains_missing_when_requested() {
        let temp = tempdir().expect("tempdir");
        let docs_dir = temp.path().join("docs");
        std::fs::create_dir_all(&docs_dir).expect("create docs dir");
        std::fs::write(docs_dir.join("a.txt"), "alpha memory").expect("write a");
        std::fs::write(docs_dir.join("b.txt"), "beta memory").expect("write b");
        let store = MemoryStore::new(
            temp.path().join("state/memory/index.jsonl"),
            temp.path().join("state/memory/status.json"),
        );
        store
            .index(MemoryIndexOptions {
                root: docs_dir.clone(),
                ..MemoryIndexOptions::default()
            })
            .expect("initial index");
        std::fs::remove_file(docs_dir.join("b.txt")).expect("remove b");
        let second = store
            .index(MemoryIndexOptions {
                root: docs_dir,
                incremental: true,
                retain_missing: true,
                ..MemoryIndexOptions::default()
            })
            .expect("incremental retain");
        assert_eq!(second.removed_documents, 0);
        assert_eq!(second.retained_missing_documents, 1);
        let search = store.search("beta", Some(5)).expect("search beta");
        assert_eq!(search.total_hits, 1);
    }

    #[test]
    fn list_and_prune_namespaces_flow() {
        let temp = tempdir().expect("tempdir");
        let data_dir = temp.path().join("state/data");
        std::fs::create_dir_all(data_dir.join("memory/namespaces/ops")).expect("create ops dir");
        std::fs::create_dir_all(data_dir.join("memory/namespaces/research"))
            .expect("create research dir");
        std::fs::write(
            data_dir.join("memory/namespaces/ops/status.json"),
            serde_json::json!({
                "indexed_documents": 3,
                "last_indexed_at": "2026-03-01T00:00:00Z",
                "index_path": data_dir.join("memory/namespaces/ops/index.jsonl").display().to_string(),
                "status_path": data_dir.join("memory/namespaces/ops/status.json").display().to_string(),
            })
            .to_string(),
        )
        .expect("write ops status");
        std::fs::write(
            data_dir.join("memory/namespaces/research/status.json"),
            serde_json::json!({
                "indexed_documents": 2,
                "last_indexed_at": "2026-03-05T00:00:00Z",
                "index_path": data_dir.join("memory/namespaces/research/index.jsonl").display().to_string(),
                "status_path": data_dir.join("memory/namespaces/research/status.json").display().to_string(),
            })
            .to_string(),
        )
        .expect("write research status");

        let listed = list_memory_namespace_statuses(&data_dir).expect("list namespaces");
        assert!(listed.iter().any(|item| item.namespace == "default"));
        assert!(listed.iter().any(|item| item.namespace == "ops"));
        assert!(listed.iter().any(|item| item.namespace == "research"));

        let dry_pruned = prune_memory_namespaces(
            &data_dir,
            MemoryPruneOptions {
                max_namespaces: Some(1),
                dry_run: true,
                ..MemoryPruneOptions::default()
            },
        )
        .expect("dry prune");
        assert_eq!(dry_pruned.removed_count, 1);
        assert_eq!(dry_pruned.removed_namespaces, vec!["ops".to_string()]);
        assert_eq!(
            dry_pruned.removed_due_to_max_namespaces,
            vec!["ops".to_string()]
        );
        assert!(dry_pruned.removed_due_to_max_age_hours.is_empty());
        assert!(
            dry_pruned
                .removed_due_to_max_documents_per_namespace
                .is_empty()
        );
        assert!(data_dir.join("memory/namespaces/ops").exists());

        let applied = prune_memory_namespaces(
            &data_dir,
            MemoryPruneOptions {
                max_namespaces: Some(1),
                dry_run: false,
                ..MemoryPruneOptions::default()
            },
        )
        .expect("apply prune");
        assert_eq!(applied.removed_namespaces, vec!["ops".to_string()]);
        assert_eq!(
            applied.removed_due_to_max_namespaces,
            vec!["ops".to_string()]
        );
        assert!(!data_dir.join("memory/namespaces/ops").exists());
        assert!(data_dir.join("memory/namespaces/research").exists());
    }

    #[test]
    fn prune_namespaces_by_document_quota() {
        let temp = tempdir().expect("tempdir");
        let data_dir = temp.path().join("state/data");
        std::fs::create_dir_all(data_dir.join("memory/namespaces/heavy"))
            .expect("create heavy dir");
        std::fs::create_dir_all(data_dir.join("memory/namespaces/light"))
            .expect("create light dir");
        std::fs::write(
            data_dir.join("memory/namespaces/heavy/status.json"),
            serde_json::json!({
                "indexed_documents": 12,
                "last_indexed_at": "2026-03-05T00:00:00Z",
                "index_path": data_dir.join("memory/namespaces/heavy/index.jsonl").display().to_string(),
                "status_path": data_dir.join("memory/namespaces/heavy/status.json").display().to_string(),
            })
            .to_string(),
        )
        .expect("write heavy status");
        std::fs::write(
            data_dir.join("memory/namespaces/light/status.json"),
            serde_json::json!({
                "indexed_documents": 2,
                "last_indexed_at": "2026-03-05T00:00:00Z",
                "index_path": data_dir.join("memory/namespaces/light/index.jsonl").display().to_string(),
                "status_path": data_dir.join("memory/namespaces/light/status.json").display().to_string(),
            })
            .to_string(),
        )
        .expect("write light status");

        let dry_run = prune_memory_namespaces(
            &data_dir,
            MemoryPruneOptions {
                max_documents_per_namespace: Some(5),
                dry_run: true,
                ..MemoryPruneOptions::default()
            },
        )
        .expect("quota dry run");
        assert_eq!(dry_run.removed_namespaces, vec!["heavy".to_string()]);
        assert_eq!(dry_run.kept_namespaces, vec!["light".to_string()]);
        assert_eq!(
            dry_run.removed_due_to_max_documents_per_namespace,
            vec!["heavy".to_string()]
        );
        assert!(data_dir.join("memory/namespaces/heavy").exists());

        let applied = prune_memory_namespaces(
            &data_dir,
            MemoryPruneOptions {
                max_documents_per_namespace: Some(5),
                dry_run: false,
                ..MemoryPruneOptions::default()
            },
        )
        .expect("quota apply");
        assert_eq!(applied.removed_namespaces, vec!["heavy".to_string()]);
        assert!(!data_dir.join("memory/namespaces/heavy").exists());
        assert!(data_dir.join("memory/namespaces/light").exists());
    }

    #[test]
    fn cleanup_policy_store_round_trip_and_mark_run() {
        let temp = tempdir().expect("tempdir");
        let policy_path = temp.path().join("policy/memory.toml");
        let store = MemoryCleanupPolicyStore::new(policy_path.clone());
        let mut policy = store.load_or_default().expect("load default");
        assert!(!policy.enabled);
        assert!(policy.last_run_at.is_none());
        policy.enabled = true;
        policy.max_documents_per_namespace = Some(100);
        policy.min_interval_minutes = Some(60);
        store.save(&policy).expect("save policy");

        let loaded = store.load_or_default().expect("load saved");
        assert!(loaded.enabled);
        assert_eq!(loaded.max_documents_per_namespace, Some(100));
        assert_eq!(loaded.min_interval_minutes, Some(60));
        assert_eq!(store.path(), policy_path.as_path());

        let marked = store.mark_run(3).expect("mark run");
        assert_eq!(marked.last_run_removed_count, Some(3));
        assert!(marked.last_run_at.is_some());
    }

    #[test]
    fn cleanup_policy_enabled_requires_limits() {
        let policy = MemoryCleanupPolicy {
            version: CURRENT_MEMORY_CLEANUP_POLICY_VERSION,
            enabled: true,
            max_namespaces: None,
            max_age_hours: None,
            max_documents_per_namespace: None,
            min_interval_minutes: None,
            last_run_at: None,
            last_run_removed_count: None,
        };
        let err = policy.validate().expect_err("expected validation error");
        assert!(matches!(err, MosaicError::Validation(_)));
    }
}
