//! SQLite model index for storing and querying model metadata.

mod dependency_profiles;
mod governance;
mod metadata_overlays;
mod model_library_updates;
mod model_selector_snapshot;
mod package_facts_cache;

use crate::models::{
    ModelFactFamily, ModelLibraryChangeKind, ModelLibraryRefreshScope, ModelLibraryUpdateEvent,
};
use crate::{PumasError, Result};
use rusqlite::{params, Connection, OpenFlags, OptionalExtension, Row};
use serde::{Deserialize, Serialize};
#[cfg(test)]
use serde_json::Value;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Instant;
use tokio::sync::broadcast;
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

/// Package-facts cache scope.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ModelPackageFactsCacheScope {
    Summary,
    Detail,
}

impl ModelPackageFactsCacheScope {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Summary => "summary",
            Self::Detail => "detail",
        }
    }
}

/// Durable package-facts cache row owned by a model record.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub struct ModelPackageFactsCacheRecord {
    pub model_id: String,
    pub selected_artifact_id: String,
    pub cache_scope: ModelPackageFactsCacheScope,
    pub package_facts_contract_version: i64,
    pub producer_revision: Option<String>,
    pub source_fingerprint: String,
    pub facts_json: String,
    pub cached_at: String,
    pub updated_at: String,
}

/// Durable model-library update row stored in cursor order.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub struct ModelLibraryUpdateRecord {
    pub event_id: i64,
    pub model_id: String,
    pub change_kind: ModelLibraryChangeKind,
    pub fact_family: ModelFactFamily,
    pub refresh_scope: ModelLibraryRefreshScope,
    pub selected_artifact_id: Option<String>,
    pub producer_revision: Option<String>,
    pub created_at: String,
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

/// Active task-signature mapping row used by metadata v2 classification.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct TaskSignatureMapping {
    pub id: i64,
    pub signature_key: String,
    pub mapping_version: i64,
    pub input_modalities: Vec<String>,
    pub output_modalities: Vec<String>,
    pub task_type_primary: String,
    pub priority: i64,
    pub status: String,
    pub source: String,
}

/// Dependency profile row.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct DependencyProfileRecord {
    pub profile_id: String,
    pub profile_version: i64,
    pub profile_hash: Option<String>,
    pub environment_kind: String,
    pub spec_json: String,
    pub created_at: String,
}

/// Model dependency binding row joined with profile fields.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ModelDependencyBindingRecord {
    pub binding_id: String,
    pub model_id: String,
    pub profile_id: String,
    pub profile_version: i64,
    pub binding_kind: String,
    pub backend_key: Option<String>,
    pub platform_selector: Option<String>,
    pub status: String,
    pub priority: i64,
    pub attached_by: Option<String>,
    pub attached_at: String,
    pub profile_hash: Option<String>,
    pub environment_kind: Option<String>,
    pub spec_json: Option<String>,
}

/// Dependency binding history event row.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct DependencyBindingHistoryRecord {
    pub event_id: i64,
    pub binding_id: String,
    pub model_id: String,
    pub actor: String,
    pub action: String,
    pub old_value_json: Option<String>,
    pub new_value_json: Option<String>,
    pub reason: Option<String>,
    pub created_at: String,
}

/// Active architecture-based model-type resolver rule.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ModelTypeArchRule {
    pub pattern: String,
    pub match_style: String,
    pub model_type: String,
    pub priority: i64,
}

/// Active config.model_type-based resolver rule.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ModelTypeConfigRule {
    pub config_model_type: String,
    pub model_type: String,
    pub priority: i64,
}

/// Active model metadata overlay row.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ModelMetadataOverlayRecord {
    pub overlay_id: String,
    pub model_id: String,
    pub overlay_json: String,
    pub status: String,
    pub reason: Option<String>,
    pub created_at: String,
    pub created_by: String,
}

/// Metadata overlay/baseline history event.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ModelMetadataHistoryRecord {
    pub event_id: i64,
    pub model_id: String,
    pub overlay_id: Option<String>,
    pub actor: String,
    pub action: String,
    pub field_path: Option<String>,
    pub old_value_json: Option<String>,
    pub new_value_json: Option<String>,
    pub reason: Option<String>,
    pub created_at: String,
}

/// SQLite foreign-key violation row from `PRAGMA foreign_key_check`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ForeignKeyViolation {
    pub table: String,
    pub rowid: Option<i64>,
    pub parent: String,
    pub fk_index: i64,
}

/// Summary of model-id references remapped while replacing an index model id.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub struct ModelIdRemapSummary {
    pub metadata_baseline_rows_copied: usize,
    pub metadata_overlay_rows_remapped: usize,
    pub metadata_history_rows_remapped: usize,
    pub dependency_binding_rows_remapped: usize,
    pub dependency_binding_history_rows_remapped: usize,
    pub package_facts_cache_rows_invalidated: usize,
    pub link_exclusion_rows_remapped: usize,
}

/// SQLite model index with FTS5 support.
#[derive(Clone)]
pub struct ModelIndex {
    db_path: PathBuf,
    conn: Arc<Mutex<Connection>>,
    fts5_config: FTS5Config,
    update_tx: broadcast::Sender<ModelLibraryUpdateEvent>,
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
        let (update_tx, _) = broadcast::channel(256);

        // Configure connection
        Self::configure_connection(&conn)?;

        // Ensure schema
        Self::ensure_schema(&conn)?;

        let index = Self {
            db_path,
            conn: Arc::new(Mutex::new(conn)),
            fts5_config: FTS5Config::default(),
            update_tx,
        };

        // Ensure FTS5 is set up
        index.ensure_fts5()?;

        Ok(index)
    }

    /// Open an existing model index with a read-only SQLite connection.
    pub fn open_read_only(db_path: impl Into<PathBuf>) -> Result<Self> {
        let db_path = db_path.into();
        let conn = Connection::open_with_flags(&db_path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
        Self::configure_read_only_connection(&conn)?;
        let (update_tx, _) = broadcast::channel(1);

        Ok(Self {
            db_path,
            conn: Arc::new(Mutex::new(conn)),
            fts5_config: FTS5Config::default(),
            update_tx,
        })
    }

    /// Configure connection with optimal settings.
    fn configure_connection(conn: &Connection) -> Result<()> {
        conn.execute_batch(
            "
            PRAGMA foreign_keys=ON;
            PRAGMA journal_mode=WAL;
            PRAGMA busy_timeout=30000;
            PRAGMA synchronous=NORMAL;
            PRAGMA temp_store=MEMORY;
            ",
        )?;
        Ok(())
    }

    fn configure_read_only_connection(conn: &Connection) -> Result<()> {
        conn.execute_batch(
            "
            PRAGMA foreign_keys=ON;
            PRAGMA query_only=ON;
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

        // Per-model link exclusion: models excluded from app linking
        conn.execute(
            "CREATE TABLE IF NOT EXISTS model_link_exclusions (
                model_id TEXT NOT NULL,
                app_id TEXT NOT NULL,
                excluded_at TEXT NOT NULL,
                PRIMARY KEY (model_id, app_id)
            )",
            [],
        )?;

        Self::ensure_metadata_v2_schema(conn)?;
        Self::ensure_package_facts_cache_schema(conn)?;
        Self::ensure_model_library_updates_schema(conn)?;
        Self::seed_metadata_v2_rows(conn)?;

        Ok(())
    }

    /// Ensure FTS5 virtual table and triggers exist.
    fn ensure_fts5(&self) -> Result<()> {
        let conn = self.conn.lock().map_err(|_| PumasError::Database {
            message: "Failed to acquire connection lock".to_string(),
            source: None,
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
    ///
    /// Returns `true` when SQLite inserted or updated a row and `false` when the
    /// existing row already matched the projected record.
    pub fn upsert(&self, record: &ModelRecord) -> Result<bool> {
        let conn = self.conn.lock().map_err(|_| PumasError::Database {
            message: "Failed to acquire connection lock".to_string(),
            source: None,
        })?;

        let tags_json = serde_json::to_string(&record.tags)?;
        let hashes_json = serde_json::to_string(&record.hashes)?;
        let metadata_json = serde_json::to_string_pretty(&record.metadata)?;

        let existing = conn
            .query_row(
                "SELECT 1 FROM models WHERE id = ?1",
                params![record.id],
                |_| Ok(()),
            )
            .optional()?
            .is_some();
        let changed = conn.execute(
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
                 updated_at=excluded.updated_at
             WHERE path != excluded.path
                OR cleaned_name != excluded.cleaned_name
                OR official_name != excluded.official_name
                OR model_type != excluded.model_type
                OR tags_json != excluded.tags_json
                OR hashes_json != excluded.hashes_json
                OR metadata_json != excluded.metadata_json
                OR updated_at != excluded.updated_at",
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
        )? > 0;

        if changed {
            debug!("Upserted model: {}", record.id);
            let change_kind = if existing {
                ModelLibraryChangeKind::MetadataModified
            } else {
                ModelLibraryChangeKind::ModelAdded
            };
            let event_id = Self::append_model_library_update_event_with_conn(
                &conn,
                &record.id,
                change_kind,
                ModelFactFamily::ModelRecord,
                ModelLibraryRefreshScope::SummaryAndDetail,
                None,
                Some(record.updated_at.clone()),
            )?;
            self.publish_model_library_update_event_with_conn(&conn, event_id)?;
        }
        Ok(changed)
    }

    /// Replace `old_id` with `record.id` while preserving durable references.
    ///
    /// Package-facts cache rows are deliberately invalidated because their JSON
    /// payload embeds model references and is cheaper to regenerate than rewrite.
    pub fn replace_model_id_preserving_references(
        &self,
        old_id: &str,
        record: &ModelRecord,
    ) -> Result<ModelIdRemapSummary> {
        if old_id == record.id {
            self.upsert(record)?;
            return Ok(ModelIdRemapSummary::default());
        }

        let mut conn = self.conn.lock().map_err(|_| PumasError::Database {
            message: "Failed to acquire connection lock".to_string(),
            source: None,
        })?;
        let tx = conn.transaction()?;

        let old_exists = tx
            .query_row(
                "SELECT 1 FROM models WHERE id = ?1",
                params![old_id],
                |_| Ok(()),
            )
            .optional()?
            .is_some();
        if !old_exists {
            return Err(PumasError::ModelNotFound {
                model_id: old_id.to_string(),
            });
        }

        let target_exists = tx
            .query_row(
                "SELECT 1 FROM models WHERE id = ?1",
                params![record.id],
                |_| Ok(()),
            )
            .optional()?
            .is_some();
        if target_exists {
            return Err(PumasError::Validation {
                field: "model_id".to_string(),
                message: format!("target model id already exists: {}", record.id),
            });
        }

        let tags_json = serde_json::to_string(&record.tags)?;
        let hashes_json = serde_json::to_string(&record.hashes)?;
        let metadata_json = serde_json::to_string_pretty(&record.metadata)?;

        tx.execute(
            "INSERT INTO models (id, path, cleaned_name, official_name, model_type,
                                 tags_json, hashes_json, metadata_json, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
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

        let metadata_baseline_rows_copied = tx.execute(
            "INSERT OR IGNORE INTO model_metadata_baselines (
                model_id, schema_version, baseline_json, created_at, created_by
             )
             SELECT ?1, schema_version, baseline_json, created_at, created_by
             FROM model_metadata_baselines
             WHERE model_id = ?2",
            params![record.id, old_id],
        )?;
        let metadata_overlay_rows_remapped = tx.execute(
            "UPDATE model_metadata_overlays SET model_id = ?1 WHERE model_id = ?2",
            params![record.id, old_id],
        )?;
        let metadata_history_rows_remapped = tx.execute(
            "UPDATE model_metadata_history SET model_id = ?1 WHERE model_id = ?2",
            params![record.id, old_id],
        )?;
        let dependency_binding_rows_remapped = tx.execute(
            "UPDATE model_dependency_bindings SET model_id = ?1 WHERE model_id = ?2",
            params![record.id, old_id],
        )?;
        let dependency_binding_history_rows_remapped = tx.execute(
            "UPDATE dependency_binding_history SET model_id = ?1 WHERE model_id = ?2",
            params![record.id, old_id],
        )?;
        let package_facts_cache_rows_invalidated = tx.execute(
            "DELETE FROM model_package_facts_cache WHERE model_id = ?1",
            params![old_id],
        )?;
        tx.execute(
            "INSERT OR IGNORE INTO model_link_exclusions (model_id, app_id, excluded_at)
             SELECT ?1, app_id, excluded_at
             FROM model_link_exclusions
             WHERE model_id = ?2",
            params![record.id, old_id],
        )?;
        let link_exclusion_rows_deleted = tx.execute(
            "DELETE FROM model_link_exclusions WHERE model_id = ?1",
            params![old_id],
        )?;

        tx.execute("DELETE FROM models WHERE id = ?1", params![old_id])?;

        let mut event_ids = Vec::new();

        event_ids.push(Self::append_model_library_update_event_with_conn(
            &tx,
            old_id,
            ModelLibraryChangeKind::ModelRemoved,
            ModelFactFamily::ModelRecord,
            ModelLibraryRefreshScope::SummaryAndDetail,
            None,
            None,
        )?);
        event_ids.push(Self::append_model_library_update_event_with_conn(
            &tx,
            &record.id,
            ModelLibraryChangeKind::ModelAdded,
            ModelFactFamily::ModelRecord,
            ModelLibraryRefreshScope::SummaryAndDetail,
            None,
            Some(record.updated_at.clone()),
        )?);
        if dependency_binding_rows_remapped > 0 || dependency_binding_history_rows_remapped > 0 {
            event_ids.push(Self::append_model_library_update_event_with_conn(
                &tx,
                &record.id,
                ModelLibraryChangeKind::DependencyBindingModified,
                ModelFactFamily::DependencyBindings,
                ModelLibraryRefreshScope::SummaryAndDetail,
                None,
                Some(record.updated_at.clone()),
            )?);
        }
        if package_facts_cache_rows_invalidated > 0 {
            event_ids.push(Self::append_model_library_update_event_with_conn(
                &tx,
                &record.id,
                ModelLibraryChangeKind::PackageFactsModified,
                ModelFactFamily::PackageFacts,
                ModelLibraryRefreshScope::SummaryAndDetail,
                None,
                Some(record.updated_at.clone()),
            )?);
        }

        tx.commit()?;
        for event_id in event_ids {
            self.publish_model_library_update_event_with_conn(&conn, event_id)?;
        }

        Ok(ModelIdRemapSummary {
            metadata_baseline_rows_copied,
            metadata_overlay_rows_remapped,
            metadata_history_rows_remapped,
            dependency_binding_rows_remapped,
            dependency_binding_history_rows_remapped,
            package_facts_cache_rows_invalidated,
            link_exclusion_rows_remapped: link_exclusion_rows_deleted,
        })
    }

    /// Get a model by ID.
    pub fn get(&self, id: &str) -> Result<Option<ModelRecord>> {
        let conn = self.conn.lock().map_err(|_| PumasError::Database {
            message: "Failed to acquire connection lock".to_string(),
            source: None,
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
        let conn = self.conn.lock().map_err(|_| PumasError::Database {
            message: "Failed to acquire connection lock".to_string(),
            source: None,
        })?;

        let rows_affected = conn.execute("DELETE FROM models WHERE id = ?1", params![id])?;

        if rows_affected > 0 {
            debug!("Deleted model: {}", id);
            let event_id = Self::append_model_library_update_event_with_conn(
                &conn,
                id,
                ModelLibraryChangeKind::ModelRemoved,
                ModelFactFamily::ModelRecord,
                ModelLibraryRefreshScope::SummaryAndDetail,
                None,
                None,
            )?;
            self.publish_model_library_update_event_with_conn(&conn, event_id)?;
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

        let conn = self.conn.lock().map_err(|_| PumasError::Database {
            message: "Failed to acquire connection lock".to_string(),
            source: None,
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
        let conn = self.conn.lock().map_err(|_| PumasError::Database {
            message: "Failed to acquire connection lock".to_string(),
            source: None,
        })?;

        let mut stmt = conn.prepare("SELECT id FROM models ORDER BY id")?;
        let rows = stmt.query_map([], |row| row.get(0))?;

        let mut ids = Vec::new();
        for row in rows {
            ids.push(row?);
        }

        Ok(ids)
    }

    /// Find a model by hash value (sha256 or blake3).
    ///
    /// Searches the `hashes_json` column using SQLite's `json_extract()` for
    /// matching hash values. Used by the library merge system for content-based
    /// duplicate detection.
    pub fn find_by_hash(&self, hash: &str) -> Result<Option<ModelRecord>> {
        let conn = self.conn.lock().map_err(|_| PumasError::Database {
            message: "Failed to acquire connection lock".to_string(),
            source: None,
        })?;

        let result = conn
            .query_row(
                "SELECT id, path, cleaned_name, official_name, model_type,
                        tags_json, hashes_json, metadata_json, updated_at
                 FROM models
                 WHERE json_extract(hashes_json, '$.sha256') = ?1
                    OR json_extract(hashes_json, '$.blake3') = ?1
                 LIMIT 1",
                params![hash],
                Self::row_to_record,
            )
            .optional()?;

        Ok(result)
    }

    /// Get the count of models.
    pub fn count(&self) -> Result<usize> {
        let conn = self.conn.lock().map_err(|_| PumasError::Database {
            message: "Failed to acquire connection lock".to_string(),
            source: None,
        })?;

        let count: usize = conn.query_row("SELECT COUNT(*) FROM models", [], |row| row.get(0))?;

        Ok(count)
    }

    /// List SQLite foreign-key violations for integrity checks.
    pub fn list_foreign_key_violations(&self) -> Result<Vec<ForeignKeyViolation>> {
        let conn = self.conn.lock().map_err(|_| PumasError::Database {
            message: "Failed to acquire connection lock".to_string(),
            source: None,
        })?;

        let mut stmt = conn.prepare("PRAGMA foreign_key_check")?;
        let rows = stmt.query_map([], |row| {
            Ok(ForeignKeyViolation {
                table: row.get(0)?,
                rowid: row.get(1)?,
                parent: row.get(2)?,
                fk_index: row.get(3)?,
            })
        })?;

        let mut violations = Vec::new();
        for row in rows {
            violations.push(row?);
        }
        Ok(violations)
    }

    /// Rebuild the FTS5 index.
    pub fn rebuild_fts5(&self) -> Result<()> {
        let conn = self.conn.lock().map_err(|_| PumasError::Database {
            message: "Failed to acquire connection lock".to_string(),
            source: None,
        })?;

        let fts5_manager = FTS5Manager::new(&self.fts5_config);
        fts5_manager.rebuild(&conn)?;

        debug!("Rebuilt FTS5 index");
        Ok(())
    }

    /// Optimize the FTS5 index.
    pub fn optimize_fts5(&self) -> Result<()> {
        let conn = self.conn.lock().map_err(|_| PumasError::Database {
            message: "Failed to acquire connection lock".to_string(),
            source: None,
        })?;

        let fts5_manager = FTS5Manager::new(&self.fts5_config);
        fts5_manager.optimize(&conn)?;

        debug!("Optimized FTS5 index");
        Ok(())
    }

    /// Checkpoint the WAL file.
    pub fn checkpoint_wal(&self) -> Result<()> {
        let conn = self.conn.lock().map_err(|_| PumasError::Database {
            message: "Failed to acquire connection lock".to_string(),
            source: None,
        })?;

        // Use query_row since PRAGMA wal_checkpoint returns results
        let _: i32 = conn.query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |row| row.get(0))?;
        debug!("Checkpointed WAL");
        Ok(())
    }

    /// Clear all models from the index.
    pub fn clear(&self) -> Result<()> {
        let conn = self.conn.lock().map_err(|_| PumasError::Database {
            message: "Failed to acquire connection lock".to_string(),
            source: None,
        })?;

        let removed_ids = {
            let mut stmt = conn.prepare("SELECT id FROM models ORDER BY id ASC")?;
            let rows = stmt
                .query_map([], |row| row.get::<_, String>(0))?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            rows
        };

        // Delete from FTS5 table first, then models table.
        // This avoids "Execute returned results" error from FTS5 triggers since the
        // AFTER DELETE trigger will find nothing to delete from the FTS5 table.
        let fts_table = &self.fts5_config.table_name;
        conn.execute_batch(&format!("DELETE FROM {}; DELETE FROM models;", fts_table))?;
        let mut event_ids = Vec::with_capacity(removed_ids.len());
        for model_id in removed_ids {
            event_ids.push(Self::append_model_library_update_event_with_conn(
                &conn,
                &model_id,
                ModelLibraryChangeKind::ModelRemoved,
                ModelFactFamily::ModelRecord,
                ModelLibraryRefreshScope::SummaryAndDetail,
                None,
                None,
            )?);
        }
        for event_id in event_ids {
            self.publish_model_library_update_event_with_conn(&conn, event_id)?;
        }
        debug!("Cleared model index");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn pinned_profile_spec(package: &str, version: &str) -> String {
        serde_json::json!({
            "python_packages": [
                {"name": package, "version": version}
            ]
        })
        .to_string()
    }

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

    fn create_package_facts_cache_record(
        model_id: &str,
        scope: ModelPackageFactsCacheScope,
        source_fingerprint: &str,
        facts_json: String,
    ) -> ModelPackageFactsCacheRecord {
        ModelPackageFactsCacheRecord {
            model_id: model_id.to_string(),
            selected_artifact_id: String::new(),
            cache_scope: scope,
            package_facts_contract_version: 1,
            producer_revision: Some("test-producer".to_string()),
            source_fingerprint: source_fingerprint.to_string(),
            facts_json,
            cached_at: "2026-05-02T00:00:00.000Z".to_string(),
            updated_at: "2026-05-02T00:00:00.000Z".to_string(),
        }
    }

    #[test]
    fn test_upsert_and_get() {
        let (index, _temp) = create_test_index();

        let record = create_test_record("test-model-1", "Test Model One", "checkpoint");
        assert!(index.upsert(&record).unwrap());

        let loaded = index.get("test-model-1").unwrap();
        assert!(loaded.is_some());
        let loaded = loaded.unwrap();
        assert_eq!(loaded.official_name, "Test Model One");
        assert_eq!(loaded.model_type, "checkpoint");
    }

    #[test]
    fn test_upsert_returns_false_when_record_is_unchanged() {
        let (index, _temp) = create_test_index();

        let record = create_test_record("stable-row", "Stable Row", "checkpoint");
        assert!(index.upsert(&record).unwrap());
        assert!(!index.upsert(&record).unwrap());
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
    fn test_model_library_update_feed_tracks_model_record_changes() {
        let (index, _temp) = create_test_index();
        let initial_cursor = index.current_model_library_update_cursor().unwrap();

        let mut record = create_test_record("feed-model", "Feed Model", "llm");
        assert!(index.upsert(&record).unwrap());
        assert!(!index.upsert(&record).unwrap());
        record.official_name = "Feed Model Updated".to_string();
        record.updated_at = "2024-01-02T00:00:00Z".to_string();
        assert!(index.upsert(&record).unwrap());
        assert!(index.delete("feed-model").unwrap());

        let feed = index
            .list_model_library_updates_since(Some(&initial_cursor), 100)
            .unwrap();
        assert!(!feed.stale_cursor);
        assert!(!feed.snapshot_required);
        assert_eq!(feed.events.len(), 3);
        assert_eq!(
            feed.events[0].change_kind,
            ModelLibraryChangeKind::ModelAdded
        );
        assert_eq!(
            feed.events[1].change_kind,
            ModelLibraryChangeKind::MetadataModified
        );
        assert_eq!(
            feed.events[2].change_kind,
            ModelLibraryChangeKind::ModelRemoved
        );
        assert!(feed
            .events
            .iter()
            .all(|event| event.refresh_scope == ModelLibraryRefreshScope::SummaryAndDetail));
    }

    #[test]
    fn test_model_library_update_feed_tracks_package_detail_changes_only() {
        let (index, _temp) = create_test_index();

        index
            .upsert(&create_test_record("facts-model", "Facts Model", "llm"))
            .unwrap();
        let cursor_after_model_add = index.current_model_library_update_cursor().unwrap();
        let summary = create_package_facts_cache_record(
            "facts-model",
            ModelPackageFactsCacheScope::Summary,
            "fingerprint-v1",
            serde_json::json!({"summary": true}).to_string(),
        );
        assert!(index.upsert_model_package_facts_cache(&summary).unwrap());
        let detail = create_package_facts_cache_record(
            "facts-model",
            ModelPackageFactsCacheScope::Detail,
            "fingerprint-v1",
            serde_json::json!({"detail": true}).to_string(),
        );
        assert!(index.upsert_model_package_facts_cache(&detail).unwrap());

        let feed = index
            .list_model_library_updates_since(Some(&cursor_after_model_add), 100)
            .unwrap();
        assert!(feed.events.iter().any(|event| {
            event.change_kind == ModelLibraryChangeKind::PackageFactsModified
                && event.fact_family == ModelFactFamily::PackageFacts
                && event.refresh_scope == ModelLibraryRefreshScope::Summary
        }));
        assert!(feed.events.iter().any(|event| {
            event.change_kind == ModelLibraryChangeKind::PackageFactsModified
                && event.fact_family == ModelFactFamily::PackageFacts
                && event.refresh_scope == ModelLibraryRefreshScope::SummaryAndDetail
        }));
    }

    #[test]
    fn test_model_library_update_feed_reports_invalid_cursor_as_stale() {
        let (index, _temp) = create_test_index();

        let feed = index
            .list_model_library_updates_since(Some("not-a-valid-cursor"), 100)
            .unwrap();
        assert!(feed.stale_cursor);
        assert!(feed.snapshot_required);
        assert!(feed.events.is_empty());
    }

    #[tokio::test]
    async fn test_model_library_update_broadcast_publishes_model_record_changes() {
        let (index, _temp) = create_test_index();
        let mut receiver = index.subscribe_model_library_update_events();

        index
            .upsert(&create_test_record(
                "broadcast-model",
                "Broadcast Model",
                "llm",
            ))
            .unwrap();

        let event = tokio::time::timeout(std::time::Duration::from_secs(1), receiver.recv())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(event.model_id, "broadcast-model");
        assert_eq!(event.change_kind, ModelLibraryChangeKind::ModelAdded);
        assert_eq!(event.fact_family, ModelFactFamily::ModelRecord);
        assert_eq!(
            event.refresh_scope,
            ModelLibraryRefreshScope::SummaryAndDetail
        );
    }

    #[tokio::test]
    async fn test_model_library_update_broadcast_publishes_transactional_replace_after_commit() {
        let (index, _temp) = create_test_index();
        index
            .upsert(&create_test_record("old-model", "Old Model", "llm"))
            .unwrap();
        let mut receiver = index.subscribe_model_library_update_events();

        index
            .replace_model_id_preserving_references(
                "old-model",
                &create_test_record("new-model", "New Model", "llm"),
            )
            .unwrap();

        let removed = tokio::time::timeout(std::time::Duration::from_secs(1), receiver.recv())
            .await
            .unwrap()
            .unwrap();
        let added = tokio::time::timeout(std::time::Duration::from_secs(1), receiver.recv())
            .await
            .unwrap()
            .unwrap();

        assert_eq!(removed.model_id, "old-model");
        assert_eq!(removed.change_kind, ModelLibraryChangeKind::ModelRemoved);
        assert_eq!(added.model_id, "new-model");
        assert_eq!(added.change_kind, ModelLibraryChangeKind::ModelAdded);
        assert!(index.get("new-model").unwrap().is_some());
        assert!(index.get("old-model").unwrap().is_none());
    }

    #[test]
    fn test_package_facts_cache_round_trips_detail_json() {
        let (index, _temp) = create_test_index();

        index
            .upsert(&create_test_record("facts-model", "Facts Model", "llm"))
            .unwrap();
        let facts_json = serde_json::json!({
            "package_facts_contract_version": 1,
            "artifact": {"artifact_kind": "safetensors"}
        })
        .to_string();
        let record = create_package_facts_cache_record(
            "facts-model",
            ModelPackageFactsCacheScope::Detail,
            "fingerprint-v1",
            facts_json.clone(),
        );

        assert!(index.upsert_model_package_facts_cache(&record).unwrap());

        let loaded = index
            .get_model_package_facts_cache("facts-model", None, ModelPackageFactsCacheScope::Detail)
            .unwrap()
            .unwrap();
        assert_eq!(loaded.model_id, "facts-model");
        assert_eq!(loaded.cache_scope, ModelPackageFactsCacheScope::Detail);
        assert_eq!(loaded.source_fingerprint, "fingerprint-v1");
        assert_eq!(loaded.facts_json, facts_json);
    }

    #[test]
    fn test_package_facts_cache_upsert_reports_changes() {
        let (index, _temp) = create_test_index();

        index
            .upsert(&create_test_record("facts-model", "Facts Model", "llm"))
            .unwrap();
        let first = create_package_facts_cache_record(
            "facts-model",
            ModelPackageFactsCacheScope::Summary,
            "fingerprint-v1",
            serde_json::json!({"summary": {"backend_hints": ["mlx"]}}).to_string(),
        );
        assert!(index.upsert_model_package_facts_cache(&first).unwrap());
        assert!(!index.upsert_model_package_facts_cache(&first).unwrap());

        let mut changed = first.clone();
        changed.source_fingerprint = "fingerprint-v2".to_string();
        changed.facts_json =
            serde_json::json!({"summary": {"backend_hints": ["vllm"]}}).to_string();
        changed.updated_at = "2026-05-02T00:01:00.000Z".to_string();
        assert!(index.upsert_model_package_facts_cache(&changed).unwrap());

        let loaded = index
            .get_model_package_facts_cache(
                "facts-model",
                Some(""),
                ModelPackageFactsCacheScope::Summary,
            )
            .unwrap()
            .unwrap();
        assert_eq!(loaded.cached_at, first.cached_at);
        assert_eq!(loaded.updated_at, changed.updated_at);
        assert_eq!(loaded.source_fingerprint, "fingerprint-v2");
    }

    #[test]
    fn test_package_facts_cache_delete_removes_model_rows() {
        let (index, _temp) = create_test_index();

        index
            .upsert(&create_test_record("facts-model", "Facts Model", "llm"))
            .unwrap();
        let record = create_package_facts_cache_record(
            "facts-model",
            ModelPackageFactsCacheScope::Detail,
            "fingerprint-v1",
            serde_json::json!({"detail": true}).to_string(),
        );
        index.upsert_model_package_facts_cache(&record).unwrap();

        assert_eq!(
            index
                .delete_model_package_facts_cache("facts-model")
                .unwrap(),
            1
        );
        assert!(index
            .get_model_package_facts_cache("facts-model", None, ModelPackageFactsCacheScope::Detail)
            .unwrap()
            .is_none());
    }

    #[test]
    fn test_package_facts_cache_cascades_when_model_deleted() {
        let (index, _temp) = create_test_index();

        index
            .upsert(&create_test_record("facts-model", "Facts Model", "llm"))
            .unwrap();
        let record = create_package_facts_cache_record(
            "facts-model",
            ModelPackageFactsCacheScope::Detail,
            "fingerprint-v1",
            serde_json::json!({"detail": true}).to_string(),
        );
        index.upsert_model_package_facts_cache(&record).unwrap();

        assert!(index.delete("facts-model").unwrap());
        assert!(index
            .get_model_package_facts_cache("facts-model", None, ModelPackageFactsCacheScope::Detail)
            .unwrap()
            .is_none());
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

    #[test]
    fn test_clear_appends_model_library_update_events() {
        let (index, _temp) = create_test_index();

        index
            .upsert(&create_test_record("m1", "Model 1", "type"))
            .unwrap();
        index
            .upsert(&create_test_record("m2", "Model 2", "type"))
            .unwrap();
        let cursor = index.current_model_library_update_cursor().unwrap();

        index.clear().unwrap();

        let feed = index
            .list_model_library_updates_since(Some(&cursor), 100)
            .unwrap();
        assert_eq!(feed.events.len(), 2);
        assert!(feed.events.iter().all(|event| {
            event.change_kind == ModelLibraryChangeKind::ModelRemoved
                && event.fact_family == ModelFactFamily::ModelRecord
                && event.refresh_scope == ModelLibraryRefreshScope::SummaryAndDetail
        }));
        assert_eq!(
            feed.events
                .iter()
                .map(|event| event.model_id.as_str())
                .collect::<Vec<_>>(),
            vec!["m1", "m2"]
        );
    }

    #[test]
    fn test_find_by_hash_sha256_found() {
        let (index, _temp) = create_test_index();

        let mut record = create_test_record("model-hash", "Hash Model", "checkpoint");
        record.hashes = HashMap::from([
            ("sha256".to_string(), "deadbeef1234".to_string()),
            ("blake3".to_string(), "cafebabe5678".to_string()),
        ]);
        index.upsert(&record).unwrap();

        let found = index.find_by_hash("deadbeef1234").unwrap();
        assert!(found.is_some());
        assert_eq!(found.unwrap().id, "model-hash");
    }

    #[test]
    fn test_find_by_hash_blake3_found() {
        let (index, _temp) = create_test_index();

        let mut record = create_test_record("model-hash-b3", "Blake3 Model", "lora");
        record.hashes = HashMap::from([
            ("sha256".to_string(), "aaa111".to_string()),
            ("blake3".to_string(), "bbb222".to_string()),
        ]);
        index.upsert(&record).unwrap();

        let found = index.find_by_hash("bbb222").unwrap();
        assert!(found.is_some());
        assert_eq!(found.unwrap().id, "model-hash-b3");
    }

    #[test]
    fn test_find_by_hash_not_found() {
        let (index, _temp) = create_test_index();

        index
            .upsert(&create_test_record("m1", "Model 1", "type"))
            .unwrap();

        let found = index.find_by_hash("nonexistent_hash_value").unwrap();
        assert!(found.is_none());
    }

    #[test]
    fn test_metadata_v2_schema_tables_exist() {
        let (index, _temp) = create_test_index();
        let conn = index.conn.lock().unwrap();

        let mut stmt = conn
            .prepare(
                "SELECT name
                 FROM sqlite_master
                 WHERE type='table' AND name IN (
                    'task_signature_mappings',
                    'model_type_arch_rules',
                    'model_type_config_rules',
                    'model_metadata_baselines',
                    'model_metadata_overlays',
                    'model_metadata_history',
                    'dependency_profiles',
                    'model_dependency_bindings',
                    'dependency_binding_history',
                    'model_package_facts_cache'
                 )",
            )
            .unwrap();
        let rows = stmt
            .query_map([], |row| row.get::<_, String>(0))
            .unwrap()
            .collect::<std::result::Result<Vec<_>, _>>()
            .unwrap();

        assert_eq!(rows.len(), 10);
    }

    #[test]
    fn test_list_foreign_key_violations_returns_empty_when_clean() {
        let (index, _temp) = create_test_index();
        let violations = index.list_foreign_key_violations().unwrap();
        assert!(violations.is_empty());
    }

    #[test]
    fn test_list_foreign_key_violations_detects_rows() {
        let (index, _temp) = create_test_index();
        let conn = index.conn.lock().unwrap();
        conn.execute("PRAGMA foreign_keys = OFF", []).unwrap();
        conn.execute(
            "INSERT INTO model_dependency_bindings (
                binding_id, model_id, profile_id, profile_version, binding_kind,
                backend_key, platform_selector, status, priority, attached_by, attached_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, NULL, NULL, ?6, ?7, NULL, strftime('%Y-%m-%dT%H:%M:%fZ','now'))",
            params![
                "b1",
                "missing-model",
                "missing-profile",
                1_i64,
                "required_core",
                "active",
                10_i64
            ],
        )
        .unwrap();
        conn.execute("PRAGMA foreign_keys = ON", []).unwrap();
        drop(conn);

        let violations = index.list_foreign_key_violations().unwrap();
        assert_eq!(violations.len(), 2);
        assert!(violations
            .iter()
            .any(|violation| violation.table == "model_dependency_bindings"));
        assert!(violations
            .iter()
            .all(|violation| violation.parent == "models"
                || violation.parent == "dependency_profiles"));
    }

    #[test]
    fn test_seeded_active_task_signature_mapping_exists() {
        let (index, _temp) = create_test_index();

        let mapping = index
            .get_active_task_signature_mapping("text->image")
            .unwrap();
        assert!(mapping.is_some());

        let mapping = mapping.unwrap();
        assert_eq!(mapping.signature_key, "text->image");
        assert_eq!(mapping.task_type_primary, "text-to-image");
        assert_eq!(mapping.status, "active");
        assert_eq!(mapping.input_modalities, vec!["text".to_string()]);
        assert_eq!(mapping.output_modalities, vec!["image".to_string()]);
    }

    #[test]
    fn test_upsert_pending_task_signature_mapping_is_single_row() {
        let (index, _temp) = create_test_index();
        let signature = "text+image->audio";
        let inputs = vec!["text".to_string(), "image".to_string()];
        let outputs = vec!["audio".to_string()];

        index
            .upsert_pending_task_signature_mapping(signature, &inputs, &outputs)
            .unwrap();
        index
            .upsert_pending_task_signature_mapping(signature, &inputs, &outputs)
            .unwrap();

        let conn = index.conn.lock().unwrap();
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM task_signature_mappings
                 WHERE signature_key = ?1 AND status = 'pending'",
                params![signature],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn test_active_model_type_rules_are_seeded() {
        let (index, _temp) = create_test_index();

        let arch_rules = index.list_active_model_type_arch_rules().unwrap();
        let config_rules = index.list_active_model_type_config_rules().unwrap();

        assert!(!arch_rules.is_empty());
        assert!(!config_rules.is_empty());
        assert!(arch_rules.iter().any(|r| r.pattern == "ForCausalLM"));
        assert!(arch_rules.iter().any(|r| r.pattern == "MossTTSDelayModel"));
        assert!(config_rules.iter().any(|r| r.config_model_type == "llama"));
        assert!(config_rules
            .iter()
            .any(|r| r.config_model_type == "moss_tts_delay"));
        assert!(config_rules
            .iter()
            .any(|r| r.config_model_type == "text-generation"));
        assert!(config_rules
            .iter()
            .any(|r| r.config_model_type == "text-ranking"));
        assert!(config_rules.iter().any(|r| r.config_model_type == "vlm"));
        assert!(config_rules.iter().any(|r| r.config_model_type == "llm"));
        assert!(config_rules
            .iter()
            .any(|r| r.config_model_type == "reranker"));
    }

    #[test]
    fn test_resolve_model_type_hint_uses_seeded_rules() {
        let (index, _temp) = create_test_index();

        assert_eq!(
            index
                .resolve_model_type_hint("text-generation")
                .unwrap()
                .as_deref(),
            Some("llm")
        );
        assert_eq!(
            index
                .resolve_model_type_hint("image-classification")
                .unwrap()
                .as_deref(),
            Some("vision")
        );
        assert_eq!(
            index
                .resolve_model_type_hint("image-text-to-text")
                .unwrap()
                .as_deref(),
            Some("vlm")
        );
        assert_eq!(
            index.resolve_model_type_hint("llm").unwrap().as_deref(),
            Some("llm")
        );
        assert_eq!(
            index
                .resolve_model_type_hint("text-ranking")
                .unwrap()
                .as_deref(),
            Some("reranker")
        );
        assert_eq!(
            index
                .resolve_model_type_hint("reranker")
                .unwrap()
                .as_deref(),
            Some("reranker")
        );
        assert_eq!(
            index.resolve_model_type_hint("not-a-known-type").unwrap(),
            None
        );
    }

    #[test]
    fn test_reopen_repairs_stale_multimodal_hint_rules() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("models.db");
        let index = ModelIndex::new(&db_path).unwrap();

        {
            let conn = index.conn.lock().unwrap();
            conn.execute(
                "UPDATE model_type_config_rules
                 SET model_type = 'vision'
                 WHERE config_model_type IN (
                   'image-to-text',
                   'image-text-to-text',
                   'visual-question-answering',
                   'document-question-answering',
                   'video-text-to-text'
                 )",
                [],
            )
            .unwrap();
        }

        drop(index);

        let repaired = ModelIndex::new(&db_path).unwrap();
        assert_eq!(
            repaired
                .resolve_model_type_hint("image-text-to-text")
                .unwrap()
                .as_deref(),
            Some("vlm")
        );
        assert_eq!(
            repaired
                .resolve_model_type_hint("image-to-text")
                .unwrap()
                .as_deref(),
            Some("vlm")
        );
    }

    #[test]
    fn test_metadata_overlay_lifecycle_and_effective_resolution() {
        let (index, _temp) = create_test_index();
        let model_id = "m-overlay";

        index
            .upsert(&create_test_record(model_id, "Overlay Model", "llm"))
            .unwrap();

        index
            .apply_metadata_overlay(
                model_id,
                "ov1",
                &serde_json::json!({
                    "description": "patched description",
                    "new_flag": true
                }),
                "tester",
                Some("first-edit"),
            )
            .unwrap();

        let active = index
            .get_active_metadata_overlay(model_id)
            .unwrap()
            .unwrap();
        assert_eq!(active.overlay_id, "ov1");

        let effective = index
            .get_effective_metadata_json(model_id)
            .unwrap()
            .unwrap();
        let effective: Value = serde_json::from_str(&effective).unwrap();
        assert_eq!(effective.get("description").unwrap(), "patched description");
        assert_eq!(effective.get("new_flag").unwrap(), true);
        assert_eq!(effective.get("family").unwrap(), "test");

        index
            .apply_metadata_overlay(
                model_id,
                "ov2",
                &serde_json::json!({
                    "family": "patched-family",
                    "new_flag": null
                }),
                "tester-2",
                Some("second-edit"),
            )
            .unwrap();

        let active = index
            .get_active_metadata_overlay(model_id)
            .unwrap()
            .unwrap();
        assert_eq!(active.overlay_id, "ov2");

        let effective = index
            .get_effective_metadata_json(model_id)
            .unwrap()
            .unwrap();
        let effective: Value = serde_json::from_str(&effective).unwrap();
        assert_eq!(effective.get("family").unwrap(), "patched-family");
        assert!(effective.get("new_flag").is_none());

        let history = index.list_model_metadata_history(model_id).unwrap();
        assert!(history
            .iter()
            .any(|event| event.action == "baseline_created"));
        assert!(history
            .iter()
            .any(|event| event.action == "overlay_created"));
        assert!(history
            .iter()
            .any(|event| event.action == "overlay_superseded"));

        let reset = index
            .reset_metadata_overlay(model_id, "tester-3", Some("reset"))
            .unwrap();
        assert!(reset);
        assert!(index
            .get_active_metadata_overlay(model_id)
            .unwrap()
            .is_none());

        let effective = index
            .get_effective_metadata_json(model_id)
            .unwrap()
            .unwrap();
        let effective: Value = serde_json::from_str(&effective).unwrap();
        assert_eq!(effective.get("family").unwrap(), "test");
        assert_eq!(effective.get("description").unwrap(), "A test model");

        let history = index.list_model_metadata_history(model_id).unwrap();
        assert!(history
            .iter()
            .any(|event| event.action == "reset_to_original"));
    }

    #[test]
    fn test_reset_metadata_overlay_without_active_row_returns_false() {
        let (index, _temp) = create_test_index();
        index
            .upsert(&create_test_record("m-no-overlay", "No Overlay", "llm"))
            .unwrap();

        let reset = index
            .reset_metadata_overlay("m-no-overlay", "tester", Some("noop"))
            .unwrap();
        assert!(!reset);
    }

    #[test]
    fn test_dependency_binding_order_is_deterministic() {
        let (index, _temp) = create_test_index();
        let now = chrono::Utc::now().to_rfc3339();

        index
            .upsert(&create_test_record("m1", "Model 1", "llm"))
            .unwrap();

        index
            .upsert_dependency_profile(&DependencyProfileRecord {
                profile_id: "p1".to_string(),
                profile_version: 1,
                profile_hash: Some("h1".to_string()),
                environment_kind: "python-venv".to_string(),
                spec_json: pinned_profile_spec("torch", "==2.4.0"),
                created_at: now.clone(),
            })
            .unwrap();
        index
            .upsert_dependency_profile(&DependencyProfileRecord {
                profile_id: "p2".to_string(),
                profile_version: 1,
                profile_hash: Some("h2".to_string()),
                environment_kind: "python-venv".to_string(),
                spec_json: pinned_profile_spec("torch", "==2.5.0"),
                created_at: now.clone(),
            })
            .unwrap();

        index
            .upsert_model_dependency_binding(&ModelDependencyBindingRecord {
                binding_id: "b2".to_string(),
                model_id: "m1".to_string(),
                profile_id: "p2".to_string(),
                profile_version: 1,
                binding_kind: "required_core".to_string(),
                backend_key: Some("transformers".to_string()),
                platform_selector: Some("linux-x86_64-cuda".to_string()),
                status: "active".to_string(),
                priority: 100,
                attached_by: Some("test".to_string()),
                attached_at: now.clone(),
                profile_hash: None,
                environment_kind: None,
                spec_json: None,
            })
            .unwrap();
        index
            .upsert_model_dependency_binding(&ModelDependencyBindingRecord {
                binding_id: "b1".to_string(),
                model_id: "m1".to_string(),
                profile_id: "p1".to_string(),
                profile_version: 1,
                binding_kind: "required_core".to_string(),
                backend_key: Some("candle".to_string()),
                platform_selector: Some("linux-x86_64-cuda".to_string()),
                status: "active".to_string(),
                priority: 100,
                attached_by: Some("test".to_string()),
                attached_at: now,
                profile_hash: None,
                environment_kind: None,
                spec_json: None,
            })
            .unwrap();

        let bindings = index
            .list_active_model_dependency_bindings("m1", None)
            .unwrap();

        assert_eq!(bindings.len(), 2);
        assert_eq!(bindings[0].binding_id, "b1");
        assert_eq!(bindings[1].binding_id, "b2");
    }

    #[test]
    fn test_dependency_profile_exists_lookup() {
        let (index, _temp) = create_test_index();
        let now = chrono::Utc::now().to_rfc3339();

        assert!(!index.dependency_profile_exists("torch-cu121", 1).unwrap());

        index
            .upsert_dependency_profile(&DependencyProfileRecord {
                profile_id: "torch-cu121".to_string(),
                profile_version: 1,
                profile_hash: Some("hash-1".to_string()),
                environment_kind: "python-venv".to_string(),
                spec_json: pinned_profile_spec("torch", "==2.5.1+cu121"),
                created_at: now,
            })
            .unwrap();

        assert!(index.dependency_profile_exists("torch-cu121", 1).unwrap());
    }

    #[test]
    fn test_dependency_profile_upsert_is_noop_when_content_is_unchanged() {
        let (index, _temp) = create_test_index();
        let first_created_at = "2026-03-10T00:00:00Z".to_string();
        let second_created_at = "2026-03-10T00:05:00Z".to_string();

        index
            .upsert_dependency_profile(&DependencyProfileRecord {
                profile_id: "p1".to_string(),
                profile_version: 1,
                profile_hash: Some("ignored".to_string()),
                environment_kind: "python-venv".to_string(),
                spec_json: pinned_profile_spec("torch", "==2.5.1"),
                created_at: first_created_at.clone(),
            })
            .unwrap();

        assert!(!index
            .upsert_dependency_profile(&DependencyProfileRecord {
                profile_id: "p1".to_string(),
                profile_version: 1,
                profile_hash: Some("still-ignored".to_string()),
                environment_kind: "python-venv".to_string(),
                spec_json: pinned_profile_spec("torch", "==2.5.1"),
                created_at: second_created_at,
            })
            .unwrap());

        let persisted = index.get_dependency_profile("p1", 1).unwrap().unwrap();
        assert_eq!(persisted.created_at, first_created_at);
        assert_eq!(persisted.environment_kind, "python-venv");
    }

    #[test]
    fn test_dependency_binding_history_records_create_and_update() {
        let (index, _temp) = create_test_index();
        let now = chrono::Utc::now().to_rfc3339();

        index
            .upsert(&create_test_record("m1", "Model 1", "llm"))
            .unwrap();
        index
            .upsert_dependency_profile(&DependencyProfileRecord {
                profile_id: "p1".to_string(),
                profile_version: 1,
                profile_hash: Some("h1".to_string()),
                environment_kind: "python-venv".to_string(),
                spec_json: pinned_profile_spec("torch", "==2.5.0"),
                created_at: now.clone(),
            })
            .unwrap();

        let mut binding = ModelDependencyBindingRecord {
            binding_id: "b1".to_string(),
            model_id: "m1".to_string(),
            profile_id: "p1".to_string(),
            profile_version: 1,
            binding_kind: "required_core".to_string(),
            backend_key: Some("transformers".to_string()),
            platform_selector: Some("linux-x86_64".to_string()),
            status: "active".to_string(),
            priority: 100,
            attached_by: Some("tester".to_string()),
            attached_at: now.clone(),
            profile_hash: None,
            environment_kind: None,
            spec_json: None,
        };

        assert!(index.upsert_model_dependency_binding(&binding).unwrap());
        // Idempotent upsert should not append history.
        assert!(!index.upsert_model_dependency_binding(&binding).unwrap());

        binding.priority = 200;
        binding.status = "deprecated".to_string();
        assert!(index.upsert_model_dependency_binding(&binding).unwrap());

        let history = index.list_dependency_binding_history("m1").unwrap();
        assert_eq!(history.len(), 2);
        assert_eq!(history[0].action, "binding_created");
        assert_eq!(history[1].action, "binding_updated");
        assert_eq!(history[1].reason.as_deref(), Some("status-changed"));

        let old_json: Value =
            serde_json::from_str(history[1].old_value_json.as_ref().unwrap()).unwrap();
        let new_json: Value =
            serde_json::from_str(history[1].new_value_json.as_ref().unwrap()).unwrap();
        assert_eq!(old_json.get("priority").unwrap(), 100);
        assert_eq!(new_json.get("priority").unwrap(), 200);
        assert_eq!(old_json.get("status").unwrap(), "active");
        assert_eq!(new_json.get("status").unwrap(), "deprecated");
    }

    #[test]
    fn test_dependency_profile_rejects_non_exact_pin_syntax() {
        let (index, _temp) = create_test_index();
        let err = index
            .upsert_dependency_profile(&DependencyProfileRecord {
                profile_id: "torch-open".to_string(),
                profile_version: 1,
                profile_hash: None,
                environment_kind: "python-venv".to_string(),
                spec_json: serde_json::json!({
                    "python_packages": [
                        {"name": "torch", "version": ">=2.5.0"}
                    ]
                })
                .to_string(),
                created_at: chrono::Utc::now().to_rfc3339(),
            })
            .unwrap_err();

        match err {
            PumasError::Validation { field, message } => {
                assert!(field.contains("python_packages"));
                assert!(message.contains("invalid_dependency_pin"));
            }
            other => panic!("expected validation error, got {other:?}"),
        }
    }

    #[test]
    fn test_dependency_profile_version_is_immutable_for_changed_content() {
        let (index, _temp) = create_test_index();
        let now = chrono::Utc::now().to_rfc3339();
        index
            .upsert_dependency_profile(&DependencyProfileRecord {
                profile_id: "torch-cu121".to_string(),
                profile_version: 7,
                profile_hash: None,
                environment_kind: "python-venv".to_string(),
                spec_json: pinned_profile_spec("torch", "==2.5.1+cu121"),
                created_at: now.clone(),
            })
            .unwrap();

        let err = index
            .upsert_dependency_profile(&DependencyProfileRecord {
                profile_id: "torch-cu121".to_string(),
                profile_version: 7,
                profile_hash: None,
                environment_kind: "python-venv".to_string(),
                spec_json: pinned_profile_spec("torch", "==2.6.0+cu121"),
                created_at: now,
            })
            .unwrap_err();

        match err {
            PumasError::Validation { field, message } => {
                assert_eq!(field, "dependency_profiles.torch-cu121:7");
                assert!(message.contains("dependency_profile_version_immutable"));
            }
            other => panic!("expected validation error, got {other:?}"),
        }
    }
}
