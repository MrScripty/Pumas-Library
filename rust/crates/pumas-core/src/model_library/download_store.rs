//! Download persistence for crash recovery and restart resume.
//!
//! Persists download state to a JSON file so that paused or errored downloads
//! can be restored after the application restarts. Only non-terminal downloads
//! (Paused, Error) are persisted; completed and cancelled downloads are removed.

use crate::error::Result;
use crate::metadata::{atomic_read_json, atomic_write_json};
use crate::model_library::types::DownloadRequest;
use crate::models::DownloadStatus;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tracing::{debug, warn};

/// A single persisted download entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistedDownload {
    pub download_id: String,
    pub repo_id: String,
    /// Primary filename (first file or legacy single-file download).
    pub filename: String,
    /// All filenames in this download (for multi-file models).
    /// Empty means legacy single-file download (use `filename` field).
    #[serde(default)]
    pub filenames: Vec<String>,
    pub dest_dir: PathBuf,
    pub total_bytes: Option<u64>,
    pub status: DownloadStatus,
    pub download_request: DownloadRequest,
    pub created_at: String,
    /// Known SHA256 from HuggingFace LFS metadata (avoids recomputation on import).
    #[serde(default)]
    pub known_sha256: Option<String>,
}

/// All persisted downloads (the JSON root object).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct DownloadStoreData {
    downloads: Vec<PersistedDownload>,
}

/// Manages download persistence to `downloads.json`.
pub struct DownloadPersistence {
    path: PathBuf,
}

impl DownloadPersistence {
    /// Create a new persistence store at `{data_dir}/downloads.json`.
    pub fn new(data_dir: &Path) -> Self {
        Self {
            path: data_dir.join("downloads.json"),
        }
    }

    /// Upsert a download entry (insert or update by download_id).
    pub fn save(&self, download: &PersistedDownload) -> Result<()> {
        let mut data = self.load_data();
        if let Some(existing) = data.downloads.iter_mut().find(|d| d.download_id == download.download_id) {
            *existing = download.clone();
        } else {
            data.downloads.push(download.clone());
        }
        self.write_data(&data)
    }

    /// Remove a download entry by ID.
    pub fn remove(&self, download_id: &str) -> Result<()> {
        let mut data = self.load_data();
        let before = data.downloads.len();
        data.downloads.retain(|d| d.download_id != download_id);
        if data.downloads.len() < before {
            self.write_data(&data)?;
        }
        Ok(())
    }

    /// Load all persisted downloads.
    pub fn load_all(&self) -> Vec<PersistedDownload> {
        self.load_data().downloads
    }

    /// Read store data, returning empty on any error.
    fn load_data(&self) -> DownloadStoreData {
        match atomic_read_json::<DownloadStoreData>(&self.path) {
            Ok(Some(data)) => data,
            Ok(None) => DownloadStoreData::default(),
            Err(e) => {
                warn!("Failed to read download store at {}: {}", self.path.display(), e);
                DownloadStoreData::default()
            }
        }
    }

    /// Write store data atomically.
    fn write_data(&self, data: &DownloadStoreData) -> Result<()> {
        debug!("Writing {} downloads to {}", data.downloads.len(), self.path.display());
        atomic_write_json(&self.path, data, false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn make_request() -> DownloadRequest {
        DownloadRequest {
            repo_id: "test/model".to_string(),
            family: "test".to_string(),
            official_name: "Test Model".to_string(),
            model_type: Some("llm".to_string()),
            quant: Some("Q4_K_M".to_string()),
            filename: None,
        }
    }

    #[test]
    fn test_save_and_load() {
        let tmp = TempDir::new().unwrap();
        let store = DownloadPersistence::new(tmp.path());

        let entry = PersistedDownload {
            download_id: "dl-1".to_string(),
            repo_id: "test/model".to_string(),
            filename: "model.gguf".to_string(),
            filenames: vec!["model.gguf".to_string()],
            dest_dir: tmp.path().to_path_buf(),
            total_bytes: Some(1000),
            status: DownloadStatus::Paused,
            download_request: make_request(),
            created_at: "2025-01-01T00:00:00Z".to_string(),
            known_sha256: None,
        };

        store.save(&entry).unwrap();
        let loaded = store.load_all();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].download_id, "dl-1");
        assert_eq!(loaded[0].status, DownloadStatus::Paused);
    }

    #[test]
    fn test_upsert() {
        let tmp = TempDir::new().unwrap();
        let store = DownloadPersistence::new(tmp.path());

        let mut entry = PersistedDownload {
            download_id: "dl-1".to_string(),
            repo_id: "test/model".to_string(),
            filename: "model.gguf".to_string(),
            filenames: vec!["model.gguf".to_string()],
            dest_dir: tmp.path().to_path_buf(),
            total_bytes: Some(1000),
            status: DownloadStatus::Paused,
            download_request: make_request(),
            created_at: "2025-01-01T00:00:00Z".to_string(),
            known_sha256: None,
        };

        store.save(&entry).unwrap();
        entry.status = DownloadStatus::Error;
        store.save(&entry).unwrap();

        let loaded = store.load_all();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].status, DownloadStatus::Error);
    }

    #[test]
    fn test_remove() {
        let tmp = TempDir::new().unwrap();
        let store = DownloadPersistence::new(tmp.path());

        let entry = PersistedDownload {
            download_id: "dl-1".to_string(),
            repo_id: "test/model".to_string(),
            filename: "model.gguf".to_string(),
            filenames: vec!["model.gguf".to_string()],
            dest_dir: tmp.path().to_path_buf(),
            total_bytes: Some(1000),
            status: DownloadStatus::Paused,
            download_request: make_request(),
            created_at: "2025-01-01T00:00:00Z".to_string(),
            known_sha256: None,
        };

        store.save(&entry).unwrap();
        assert_eq!(store.load_all().len(), 1);

        store.remove("dl-1").unwrap();
        assert_eq!(store.load_all().len(), 0);
    }

    #[test]
    fn test_load_empty() {
        let tmp = TempDir::new().unwrap();
        let store = DownloadPersistence::new(tmp.path());
        assert_eq!(store.load_all().len(), 0);
    }
}
