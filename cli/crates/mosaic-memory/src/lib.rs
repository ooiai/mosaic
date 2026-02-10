use std::cmp::Reverse;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use walkdir::WalkDir;

use mosaic_core::error::{MosaicError, Result};

const DEFAULT_MAX_FILES: usize = 500;
const DEFAULT_MAX_FILE_SIZE: usize = 256 * 1024;
const DEFAULT_MAX_CONTENT_BYTES: usize = 16 * 1024;
const DEFAULT_SEARCH_LIMIT: usize = 20;

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
    pub indexed_documents: usize,
    pub skipped_files: usize,
    pub index_path: String,
    pub status_path: String,
}

#[derive(Debug, Clone)]
pub struct MemoryIndexOptions {
    pub root: PathBuf,
    pub max_files: usize,
    pub max_file_size: usize,
    pub max_content_bytes: usize,
}

impl Default for MemoryIndexOptions {
    fn default() -> Self {
        Self {
            root: PathBuf::from("."),
            max_files: DEFAULT_MAX_FILES,
            max_file_size: DEFAULT_MAX_FILE_SIZE,
            max_content_bytes: DEFAULT_MAX_CONTENT_BYTES,
        }
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
        let mut documents = Vec::new();
        let mut skipped = 0usize;

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

            let content = match std::fs::read_to_string(path) {
                Ok(content) => content,
                Err(_) => {
                    skipped += 1;
                    continue;
                }
            };
            if content.trim().is_empty() {
                skipped += 1;
                continue;
            }

            let relative = path
                .strip_prefix(&root)
                .ok()
                .map(|value| value.to_string_lossy().to_string())
                .unwrap_or_else(|| path.to_string_lossy().to_string());
            let truncated = truncate_to_bytes(&content, options.max_content_bytes);
            documents.push(MemoryDocument {
                id: format!("mem_{}", uuid::Uuid::new_v4()),
                path: relative,
                content: truncated,
                size_bytes: metadata.len(),
                indexed_at: Utc::now(),
            });
        }

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
            indexed_documents: status.indexed_documents,
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
        let lower_query = query.to_lowercase();

        let mut hits = docs
            .into_iter()
            .filter_map(|doc| {
                let lower_content = doc.content.to_lowercase();
                let score = lower_content.matches(&lower_query).count();
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
    data_dir.join("memory/index.jsonl")
}

pub fn memory_status_path(data_dir: &Path) -> PathBuf {
    data_dir.join("memory/status.json")
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

fn find_snippet(content: &str, query: &str) -> String {
    let lower_query = query.to_lowercase();
    for line in content.lines() {
        if line.to_lowercase().contains(&lower_query) {
            return truncate_to_bytes(line, 160);
        }
    }
    truncate_to_bytes(content, 160)
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
}
