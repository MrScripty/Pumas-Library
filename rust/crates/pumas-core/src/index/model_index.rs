//! SQLite model index for storing and querying model metadata.

use crate::{PumasError, Result};
use rusqlite::{params, Connection, OptionalExtension, Row};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Instant;
use tracing::{debug, error, warn};

use super::fts5::{FTS5Config, FTS5Manager};
use super::query::build_fts5_query;

/// A record in the model index.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelRecord {
    pub id: String,
    pub path: String,
    pub cleaned_name: String,
    pub official_name: String,
    pub model_type: String,
    pub tags: Vec<String>,
    pub hashes: HashMap<String, String>,
    pub metadata: serde_json::Value,
    pub updated_at: String,
}

/// Search result from the model index.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchResult {
    pub models: Vec<ModelRecord>,
    pub total_count: usize,
    pub query_time_ms: f64,
    pub query: String,
}

/// SQLite model index with FTS5 support.
pub struct ModelIndex {
    db_path: PathBuf,
    conn: Arc<Mutex<Connection>>,
    fts5_config: FTS5Config,
}

impl ModelIndex {
    /// Create or open a model index at the given path.
    pub fn new(db_path: impl Into<PathBuf>) -> Result<Self> {
        let db_path = db_path.into();

        // Ensure parent directory exists
        if let Some(parent) = db_path.parent() {
            if !parent.exists() {
                std::fs::create_dir_all(parent).map_err(|e| PumasError::Io {
                    message: format!("Failed to create directory {}", parent.display()),
                    path: Some(parent.to_path_buf()),
                    source: Some(e),
                })?;
            }
        }

        let conn = Connection::open(&db_path)?;

        // Configure connection
        Self::configure_connection(&conn)?;

        // Ensure schema
        Self::ensure_schema(&conn)?;

        let index = Self {
            db_path,
            conn: Arc::new(Mutex::new(conn)),
            fts5_config: FTS5Config::default(),
        };

        // Ensure FTS5 is set up
        index.ensure_fts5()?;

        Ok(index)
    }

    /// Configure connection with optimal settings.
    fn configure_connection(conn: &Connection) -> Result<()> {
        conn.execute_batch(
            "
            PRAGMA journal_mode=WAL;
            PRAGMA busy_timeout=30000;
            PRAGMA synchronous=NORMAL;
            PRAGMA temp_store=MEMORY;
            ",
        )?;
        Ok(())
    }

    /// Ensure the base schema exists.
    fn ensure_schema(conn: &Connection) -> Result<()> {
        conn.execute(
            "CREATE TABLE IF NOT EXISTS models (
                id TEXT PRIMARY KEY,
                path TEXT NOT NULL,
                cleaned_name TEXT NOT NULL,
                official_name TEXT NOT NULL,
                model_type TEXT NOT NULL,
                tags_json TEXT NOT NULL,
                hashes_json TEXT NOT NULL,
                metadata_json TEXT NOT NULL,
                updated_at TEXT NOT NULL
            )",
            [],
        )?;

        // Create indexes for common queries
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_models_type ON models(model_type)",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_models_updated ON models(updated_at)",
            [],
        )?;

        Ok(())
    }

    /// Ensure FTS5 virtual table and triggers exist.
    fn ensure_fts5(&self) -> Result<()> {
        let conn = self.conn.lock().map_err(|_| {
            PumasError::Database {
                message: "Failed to acquire connection lock".to_string(),
                source: None,
            }
        })?;

        let fts5_manager = FTS5Manager::new(&self.fts5_config);
        fts5_manager.ensure_setup(&conn)?;

        Ok(())
    }

    /// Get the database path.
    pub fn db_path(&self) -> &Path {
        &self.db_path
    }

    /// Insert or update a model record.
    pub fn upsert(&self, record: &ModelRecord) -> Result<()> {
        let conn = self.conn.lock().map_err(|_| {
            PumasError::Database {
                message: "Failed to acquire connection lock".to_string(),
                source: None,
            }
        })?;

        let tags_json = serde_json::to_string(&record.tags)?;
        let hashes_json = serde_json::to_string(&record.hashes)?;
        let metadata_json = serde_json::to_string_pretty(&record.metadata)?;

        conn.execute(
            "INSERT INTO models (id, path, cleaned_name, official_name, model_type,
                                 tags_json, hashes_json, metadata_json, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
             ON CONFLICT(id) DO UPDATE SET
                 path=excluded.path,
                 cleaned_name=excluded.cleaned_name,
                 official_name=excluded.official_name,
                 model_type=excluded.model_type,
                 tags_json=excluded.tags_json,
                 hashes_json=excluded.hashes_json,
                 metadata_json=excluded.metadata_json,
                 updated_at=excluded.updated_at",
            params![
                record.id,
                record.path,
                record.cleaned_name,
                record.official_name,
                record.model_type,
                tags_json,
                hashes_json,
                metadata_json,
                record.updated_at,
            ],
        )?;

        debug!("Upserted model: {}", record.id);
        Ok(())
    }

    /// Get a model by ID.
    pub fn get(&self, id: &str) -> Result<Option<ModelRecord>> {
        let conn = self.conn.lock().map_err(|_| {
            PumasError::Database {
                message: "Failed to acquire connection lock".to_string(),
                source: None,
            }
        })?;

        let result = conn
            .query_row(
                "SELECT id, path, cleaned_name, official_name, model_type,
                        tags_json, hashes_json, metadata_json, updated_at
                 FROM models WHERE id = ?1",
                params![id],
                Self::row_to_record,
            )
            .optional()?;

        Ok(result)
    }

    /// Delete a model by ID.
    pub fn delete(&self, id: &str) -> Result<bool> {
        let conn = self.conn.lock().map_err(|_| {
            PumasError::Database {
                message: "Failed to acquire connection lock".to_string(),
                source: None,
            }
        })?;

        let rows_affected = conn.execute("DELETE FROM models WHERE id = ?1", params![id])?;

        if rows_affected > 0 {
            debug!("Deleted model: {}", id);
        }

        Ok(rows_affected > 0)
    }

    /// Search models using FTS5 full-text search.
    pub fn search(
        &self,
        query: &str,
        model_types: Option<&[String]>,
        tags: Option<&[String]>,
        limit: usize,
        offset: usize,
    ) -> Result<SearchResult> {
        let start = Instant::now();

        let conn = self.conn.lock().map_err(|_| {
            PumasError::Database {
                message: "Failed to acquire connection lock".to_string(),
                source: None,
            }
        })?;

        let fts5_query = if query.trim().is_empty() {
            String::new()
        } else {
            build_fts5_query(query)
        };

        let (models, total_count) = if fts5_query.is_empty() {
            // Empty query - return all models
            self.search_all(&conn, model_types, tags, limit, offset)?
        } else {
            // FTS5 search
            self.search_fts5(&conn, &fts5_query, model_types, tags, limit, offset)?
        };

        let query_time_ms = start.elapsed().as_secs_f64() * 1000.0;

        Ok(SearchResult {
            models,
            total_count,
            query_time_ms,
            query: fts5_query,
        })
    }

    /// Search all models without FTS5.
    fn search_all(
        &self,
        conn: &Connection,
        model_types: Option<&[String]>,
        tags: Option<&[String]>,
        limit: usize,
        offset: usize,
    ) -> Result<(Vec<ModelRecord>, usize)> {
        // Build WHERE clause
        let mut where_clause = String::from("WHERE 1=1");
        let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        // Add model type filter
        if let Some(types) = model_types {
            if !types.is_empty() {
                let placeholders: Vec<_> = types.iter().map(|_| "?").collect();
                where_clause.push_str(&format!(" AND model_type IN ({})", placeholders.join(",")));
                for t in types {
                    params_vec.push(Box::new(t.clone()));
                }
            }
        }

        // Get total count first
        let count_sql = format!("SELECT COUNT(*) FROM models {}", where_clause);
        let total_count: usize = {
            let mut stmt = conn.prepare(&count_sql)?;
            let params_refs: Vec<&dyn rusqlite::ToSql> =
                params_vec.iter().map(|p| p.as_ref()).collect();
            stmt.query_row(params_refs.as_slice(), |row| row.get(0))?
        };

        // Build full query with pagination
        let sql = format!(
            "SELECT id, path, cleaned_name, official_name, model_type, \
             tags_json, hashes_json, metadata_json, updated_at \
             FROM models {} ORDER BY updated_at DESC LIMIT {} OFFSET {}",
            where_clause, limit, offset
        );

        let mut stmt = conn.prepare(&sql)?;
        let params_refs: Vec<&dyn rusqlite::ToSql> =
            params_vec.iter().map(|p| p.as_ref()).collect();

        let rows = stmt.query_map(params_refs.as_slice(), Self::row_to_record)?;

        let mut models = Vec::new();
        for row in rows {
            match row {
                Ok(record) => {
                    // Apply tag filter in post-processing
                    if let Some(required_tags) = tags {
                        if required_tags
                            .iter()
                            .all(|t| record.tags.iter().any(|rt| rt.eq_ignore_ascii_case(t)))
                        {
                            models.push(record);
                        }
                    } else {
                        models.push(record);
                    }
                }
                Err(e) => {
                    warn!("Error reading model row: {}", e);
                }
            }
        }

        Ok((models, total_count))
    }

    /// Search using FTS5.
    fn search_fts5(
        &self,
        conn: &Connection,
        fts5_query: &str,
        model_types: Option<&[String]>,
        tags: Option<&[String]>,
        limit: usize,
        offset: usize,
    ) -> Result<(Vec<ModelRecord>, usize)> {
        let table_name = &self.fts5_config.table_name;

        // Build WHERE clause
        let mut where_parts = vec![format!("{} MATCH ?", table_name)];
        let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = vec![Box::new(fts5_query.to_string())];

        // Add model type filter
        if let Some(types) = model_types {
            if !types.is_empty() {
                let placeholders: Vec<_> = types.iter().map(|_| "?").collect();
                where_parts.push(format!("m.model_type IN ({})", placeholders.join(",")));
                for t in types {
                    params_vec.push(Box::new(t.clone()));
                }
            }
        }

        let where_clause = where_parts.join(" AND ");

        // Get total count first
        let count_sql = format!(
            "SELECT COUNT(*) FROM {} ms JOIN models m ON ms.id = m.id WHERE {}",
            table_name, where_clause
        );

        let total_count: usize = {
            let mut stmt = conn.prepare(&count_sql)?;
            let params_refs: Vec<&dyn rusqlite::ToSql> =
                params_vec.iter().map(|p| p.as_ref()).collect();
            match stmt.query_row(params_refs.as_slice(), |row| row.get(0)) {
                Ok(count) => count,
                Err(e) => {
                    // FTS5 query may fail - return 0
                    error!("FTS5 count query failed: {}", e);
                    0
                }
            }
        };

        // Build full query with pagination
        let sql = format!(
            "SELECT m.id, m.path, m.cleaned_name, m.official_name, m.model_type, \
             m.tags_json, m.hashes_json, m.metadata_json, m.updated_at \
             FROM {} ms JOIN models m ON ms.id = m.id \
             WHERE {} ORDER BY rank LIMIT {} OFFSET {}",
            table_name, where_clause, limit, offset
        );

        let mut stmt = conn.prepare(&sql)?;
        let params_refs: Vec<&dyn rusqlite::ToSql> =
            params_vec.iter().map(|p| p.as_ref()).collect();

        let rows = match stmt.query_map(params_refs.as_slice(), Self::row_to_record) {
            Ok(rows) => rows,
            Err(e) => {
                error!("FTS5 search failed: {}", e);
                return Ok((vec![], 0));
            }
        };

        let mut models = Vec::new();
        for row in rows {
            match row {
                Ok(record) => {
                    // Apply tag filter in post-processing
                    if let Some(required_tags) = tags {
                        if required_tags
                            .iter()
                            .all(|t| record.tags.iter().any(|rt| rt.eq_ignore_ascii_case(t)))
                        {
                            models.push(record);
                        }
                    } else {
                        models.push(record);
                    }
                }
                Err(e) => {
                    warn!("Error reading model row: {}", e);
                }
            }
        }

        Ok((models, total_count))
    }

    /// Convert a row to a ModelRecord.
    fn row_to_record(row: &Row) -> rusqlite::Result<ModelRecord> {
        let tags_json: String = row.get(5)?;
        let hashes_json: String = row.get(6)?;
        let metadata_json: String = row.get(7)?;

        let tags: Vec<String> = serde_json::from_str(&tags_json).unwrap_or_default();
        let hashes: HashMap<String, String> =
            serde_json::from_str(&hashes_json).unwrap_or_default();
        let metadata: serde_json::Value =
            serde_json::from_str(&metadata_json).unwrap_or(serde_json::Value::Null);

        Ok(ModelRecord {
            id: row.get(0)?,
            path: row.get(1)?,
            cleaned_name: row.get(2)?,
            official_name: row.get(3)?,
            model_type: row.get(4)?,
            tags,
            hashes,
            metadata,
            updated_at: row.get(8)?,
        })
    }

    /// Get all model IDs.
    pub fn get_all_ids(&self) -> Result<Vec<String>> {
        let conn = self.conn.lock().map_err(|_| {
            PumasError::Database {
                message: "Failed to acquire connection lock".to_string(),
                source: None,
            }
        })?;

        let mut stmt = conn.prepare("SELECT id FROM models ORDER BY id")?;
        let rows = stmt.query_map([], |row| row.get(0))?;

        let mut ids = Vec::new();
        for row in rows {
            ids.push(row?);
        }

        Ok(ids)
    }

    /// Get the count of models.
    pub fn count(&self) -> Result<usize> {
        let conn = self.conn.lock().map_err(|_| {
            PumasError::Database {
                message: "Failed to acquire connection lock".to_string(),
                source: None,
            }
        })?;

        let count: usize = conn.query_row("SELECT COUNT(*) FROM models", [], |row| row.get(0))?;

        Ok(count)
    }

    /// Rebuild the FTS5 index.
    pub fn rebuild_fts5(&self) -> Result<()> {
        let conn = self.conn.lock().map_err(|_| {
            PumasError::Database {
                message: "Failed to acquire connection lock".to_string(),
                source: None,
            }
        })?;

        let fts5_manager = FTS5Manager::new(&self.fts5_config);
        fts5_manager.rebuild(&conn)?;

        debug!("Rebuilt FTS5 index");
        Ok(())
    }

    /// Optimize the FTS5 index.
    pub fn optimize_fts5(&self) -> Result<()> {
        let conn = self.conn.lock().map_err(|_| {
            PumasError::Database {
                message: "Failed to acquire connection lock".to_string(),
                source: None,
            }
        })?;

        let fts5_manager = FTS5Manager::new(&self.fts5_config);
        fts5_manager.optimize(&conn)?;

        debug!("Optimized FTS5 index");
        Ok(())
    }

    /// Checkpoint the WAL file.
    pub fn checkpoint_wal(&self) -> Result<()> {
        let conn = self.conn.lock().map_err(|_| {
            PumasError::Database {
                message: "Failed to acquire connection lock".to_string(),
                source: None,
            }
        })?;

        conn.execute("PRAGMA wal_checkpoint(TRUNCATE)", [])?;
        debug!("Checkpointed WAL");
        Ok(())
    }

    /// Clear all models from the index.
    pub fn clear(&self) -> Result<()> {
        let conn = self.conn.lock().map_err(|_| {
            PumasError::Database {
                message: "Failed to acquire connection lock".to_string(),
                source: None,
            }
        })?;

        conn.execute("DELETE FROM models", [])?;
        debug!("Cleared model index");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_test_index() -> (ModelIndex, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("models.db");
        let index = ModelIndex::new(&db_path).unwrap();
        (index, temp_dir)
    }

    fn create_test_record(id: &str, name: &str, model_type: &str) -> ModelRecord {
        ModelRecord {
            id: id.to_string(),
            path: format!("models/{}", id),
            cleaned_name: name.to_lowercase().replace(' ', "_"),
            official_name: name.to_string(),
            model_type: model_type.to_string(),
            tags: vec!["test".to_string()],
            hashes: HashMap::from([("sha256".to_string(), "abc123".to_string())]),
            metadata: serde_json::json!({"family": "test", "description": "A test model"}),
            updated_at: "2024-01-01T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn test_upsert_and_get() {
        let (index, _temp) = create_test_index();

        let record = create_test_record("test-model-1", "Test Model One", "checkpoint");
        index.upsert(&record).unwrap();

        let loaded = index.get("test-model-1").unwrap();
        assert!(loaded.is_some());
        let loaded = loaded.unwrap();
        assert_eq!(loaded.official_name, "Test Model One");
        assert_eq!(loaded.model_type, "checkpoint");
    }

    #[test]
    fn test_delete() {
        let (index, _temp) = create_test_index();

        let record = create_test_record("delete-me", "Delete Me", "lora");
        index.upsert(&record).unwrap();

        assert!(index.get("delete-me").unwrap().is_some());

        let deleted = index.delete("delete-me").unwrap();
        assert!(deleted);

        assert!(index.get("delete-me").unwrap().is_none());
    }

    #[test]
    fn test_search_all() {
        let (index, _temp) = create_test_index();

        // Add multiple models
        for i in 1..=5 {
            let record = create_test_record(
                &format!("model-{}", i),
                &format!("Model Number {}", i),
                if i % 2 == 0 { "lora" } else { "checkpoint" },
            );
            index.upsert(&record).unwrap();
        }

        // Search all
        let result = index.search("", None, None, 10, 0).unwrap();
        assert_eq!(result.total_count, 5);
        assert_eq!(result.models.len(), 5);
    }

    #[test]
    fn test_search_fts5() {
        let (index, _temp) = create_test_index();

        let record1 = create_test_record("llama-7b", "Llama 7B", "llm");
        let record2 = create_test_record("stable-diffusion", "Stable Diffusion v1.5", "diffusion");
        let record3 = create_test_record("llama-13b", "Llama 13B", "llm");

        index.upsert(&record1).unwrap();
        index.upsert(&record2).unwrap();
        index.upsert(&record3).unwrap();

        // Search for "llama"
        let result = index.search("llama", None, None, 10, 0).unwrap();
        assert_eq!(result.models.len(), 2);

        // Search for "stable"
        let result = index.search("stable", None, None, 10, 0).unwrap();
        assert_eq!(result.models.len(), 1);
        assert_eq!(result.models[0].id, "stable-diffusion");
    }

    #[test]
    fn test_search_by_type() {
        let (index, _temp) = create_test_index();

        let record1 = create_test_record("model-1", "Model One", "checkpoint");
        let record2 = create_test_record("model-2", "Model Two", "lora");
        let record3 = create_test_record("model-3", "Model Three", "checkpoint");

        index.upsert(&record1).unwrap();
        index.upsert(&record2).unwrap();
        index.upsert(&record3).unwrap();

        // Search by type
        let types = vec!["checkpoint".to_string()];
        let result = index.search("", Some(&types), None, 10, 0).unwrap();
        assert_eq!(result.models.len(), 2);
    }

    #[test]
    fn test_count() {
        let (index, _temp) = create_test_index();

        assert_eq!(index.count().unwrap(), 0);

        index
            .upsert(&create_test_record("m1", "Model 1", "type"))
            .unwrap();
        index
            .upsert(&create_test_record("m2", "Model 2", "type"))
            .unwrap();

        assert_eq!(index.count().unwrap(), 2);
    }

    #[test]
    fn test_clear() {
        let (index, _temp) = create_test_index();

        index
            .upsert(&create_test_record("m1", "Model 1", "type"))
            .unwrap();
        index
            .upsert(&create_test_record("m2", "Model 2", "type"))
            .unwrap();

        assert_eq!(index.count().unwrap(), 2);

        index.clear().unwrap();

        assert_eq!(index.count().unwrap(), 0);
    }
}
