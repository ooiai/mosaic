use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryStatus {
    pub indexed_documents: usize,
    pub last_indexed_at: Option<String>,
}

impl Default for MemoryStatus {
    fn default() -> Self {
        Self {
            indexed_documents: 0,
            last_indexed_at: None,
        }
    }
}
