use std::path::{Path, PathBuf};

use crate::index::ModelIndex;
use crate::models::{ModelLibrarySelectorSnapshot, ModelLibrarySelectorSnapshotRequest};
use crate::Result;

const DB_FILENAME: &str = "models.db";

/// Read-only model-library view for snapshot-style consumers.
///
/// This type opens the existing SQLite model index without claiming instance
/// ownership, starting watchers, creating schema, or running reconciliation.
pub struct PumasReadOnlyLibrary {
    library_root: PathBuf,
    index: ModelIndex,
}

impl PumasReadOnlyLibrary {
    pub fn open(library_root: impl Into<PathBuf>) -> Result<Self> {
        let library_root = library_root.into();
        let index = ModelIndex::open_read_only(library_root.join(DB_FILENAME))?;
        Ok(Self {
            library_root,
            index,
        })
    }

    pub fn library_root(&self) -> &Path {
        &self.library_root
    }

    pub fn model_library_selector_snapshot(
        &self,
        request: ModelLibrarySelectorSnapshotRequest,
    ) -> Result<ModelLibrarySelectorSnapshot> {
        self.index.list_model_library_selector_snapshot(&request)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::index::ModelRecord;
    use crate::models::{ModelArtifactState, ModelEntryPathState};
    use std::collections::HashMap;
    use tempfile::TempDir;

    fn create_record(id: &str) -> ModelRecord {
        ModelRecord {
            id: id.to_string(),
            path: format!("models/{id}"),
            cleaned_name: "read_only_selector".to_string(),
            official_name: "Read Only Selector".to_string(),
            model_type: "llm".to_string(),
            tags: vec!["read-only".to_string()],
            hashes: HashMap::from([("sha256".to_string(), "abc123".to_string())]),
            metadata: serde_json::json!({
                "entry_path": "/models/read-only/model.gguf",
                "validation_state": "valid",
                "task_type_primary": "text-generation"
            }),
            updated_at: "2026-05-06T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn read_only_library_projects_selector_snapshot_without_owner_lifecycle() {
        let temp = TempDir::new().unwrap();
        let writer = ModelIndex::new(temp.path().join(DB_FILENAME)).unwrap();
        writer
            .upsert(&create_record("llm/example/read-only"))
            .unwrap();
        drop(writer);

        let read_only = PumasReadOnlyLibrary::open(temp.path()).unwrap();
        let snapshot = read_only
            .model_library_selector_snapshot(ModelLibrarySelectorSnapshotRequest::default())
            .unwrap();

        assert_eq!(read_only.library_root(), temp.path());
        assert_eq!(snapshot.rows.len(), 1);
        assert_eq!(snapshot.rows[0].model_id, "llm/example/read-only");
        assert_eq!(
            snapshot.rows[0].entry_path_state,
            ModelEntryPathState::Ready
        );
        assert_eq!(snapshot.rows[0].artifact_state, ModelArtifactState::Ready);
    }

    #[test]
    fn read_only_library_requires_existing_index() {
        let temp = TempDir::new().unwrap();

        let result = PumasReadOnlyLibrary::open(temp.path());

        assert!(result.is_err());
        assert!(!temp.path().join(DB_FILENAME).exists());
    }
}
