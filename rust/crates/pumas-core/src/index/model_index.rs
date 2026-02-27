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
        Self::seed_metadata_v2_rows(conn)?;

        Ok(())
    }

    /// Create metadata v2/additional governance tables.
    fn ensure_metadata_v2_schema(conn: &Connection) -> Result<()> {
        conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS task_signature_mappings (
              id INTEGER PRIMARY KEY AUTOINCREMENT,
              signature_key TEXT NOT NULL,
              mapping_version INTEGER NOT NULL,
              input_modalities_json TEXT NOT NULL,
              output_modalities_json TEXT NOT NULL,
              task_type_primary TEXT NOT NULL,
              priority INTEGER NOT NULL DEFAULT 100,
              status TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'pending', 'deprecated')),
              source TEXT NOT NULL DEFAULT 'system',
              supersedes_id INTEGER,
              change_reason TEXT,
              notes TEXT,
              created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
              updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
              FOREIGN KEY (supersedes_id) REFERENCES task_signature_mappings(id),
              UNIQUE(signature_key, mapping_version)
            );

            CREATE INDEX IF NOT EXISTS idx_task_signature_mappings_lookup
              ON task_signature_mappings(status, signature_key, priority, mapping_version DESC);

            CREATE UNIQUE INDEX IF NOT EXISTS idx_task_signature_mappings_one_active
              ON task_signature_mappings(signature_key)
              WHERE status = 'active';

            CREATE UNIQUE INDEX IF NOT EXISTS idx_task_signature_mappings_one_pending
              ON task_signature_mappings(signature_key)
              WHERE status = 'pending';

            CREATE TABLE IF NOT EXISTS model_type_arch_rules (
              id INTEGER PRIMARY KEY AUTOINCREMENT,
              pattern TEXT NOT NULL,
              match_style TEXT NOT NULL CHECK (match_style IN ('exact', 'prefix', 'suffix', 'wildcard')),
              model_type TEXT NOT NULL CHECK (model_type IN ('llm', 'diffusion', 'audio', 'vision', 'embedding', 'unknown')),
              priority INTEGER NOT NULL DEFAULT 100,
              status TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'pending', 'deprecated')),
              source TEXT NOT NULL DEFAULT 'system',
              notes TEXT,
              created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
              updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
            );

            CREATE UNIQUE INDEX IF NOT EXISTS idx_model_type_arch_rules_active_unique
              ON model_type_arch_rules(pattern, match_style)
              WHERE status = 'active';

            CREATE INDEX IF NOT EXISTS idx_model_type_arch_rules_lookup
              ON model_type_arch_rules(status, priority, pattern, match_style);

            CREATE TABLE IF NOT EXISTS model_type_config_rules (
              id INTEGER PRIMARY KEY AUTOINCREMENT,
              config_model_type TEXT NOT NULL,
              model_type TEXT NOT NULL CHECK (model_type IN ('llm', 'diffusion', 'audio', 'vision', 'embedding', 'unknown')),
              priority INTEGER NOT NULL DEFAULT 100,
              status TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'pending', 'deprecated')),
              source TEXT NOT NULL DEFAULT 'system',
              notes TEXT,
              created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
              updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
            );

            CREATE UNIQUE INDEX IF NOT EXISTS idx_model_type_config_rules_active_unique
              ON model_type_config_rules(config_model_type)
              WHERE status = 'active';

            CREATE INDEX IF NOT EXISTS idx_model_type_config_rules_lookup
              ON model_type_config_rules(status, priority, config_model_type);

            CREATE TABLE IF NOT EXISTS model_metadata_baselines (
              model_id TEXT PRIMARY KEY,
              schema_version INTEGER NOT NULL,
              baseline_json TEXT NOT NULL CHECK (json_valid(baseline_json)),
              created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
              created_by TEXT NOT NULL DEFAULT 'pumas-library',
              FOREIGN KEY (model_id) REFERENCES models(id) ON DELETE CASCADE
            );

            CREATE TRIGGER IF NOT EXISTS trg_model_metadata_baselines_no_update
            BEFORE UPDATE ON model_metadata_baselines
            FOR EACH ROW
            BEGIN
              SELECT RAISE(ABORT, 'model_metadata_baselines is immutable');
            END;

            CREATE TABLE IF NOT EXISTS model_metadata_overlays (
              overlay_id TEXT PRIMARY KEY,
              model_id TEXT NOT NULL,
              overlay_json TEXT NOT NULL CHECK (json_valid(overlay_json)),
              status TEXT NOT NULL DEFAULT 'active'
                CHECK (status IN ('active', 'superseded', 'reverted')),
              reason TEXT,
              created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
              created_by TEXT NOT NULL,
              FOREIGN KEY (model_id) REFERENCES models(id) ON DELETE CASCADE
            );

            CREATE INDEX IF NOT EXISTS idx_model_metadata_overlays_model
              ON model_metadata_overlays(model_id, created_at);

            CREATE UNIQUE INDEX IF NOT EXISTS idx_model_metadata_overlays_one_active
              ON model_metadata_overlays(model_id)
              WHERE status = 'active';

            CREATE TABLE IF NOT EXISTS model_metadata_history (
              event_id INTEGER PRIMARY KEY AUTOINCREMENT,
              model_id TEXT NOT NULL,
              overlay_id TEXT,
              actor TEXT NOT NULL,
              action TEXT NOT NULL
                CHECK (action IN (
                  'baseline_created',
                  'overlay_created',
                  'overlay_superseded',
                  'overlay_reverted',
                  'reset_to_original',
                  'field_updated'
                )),
              field_path TEXT,
              old_value_json TEXT,
              new_value_json TEXT,
              reason TEXT,
              created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
              FOREIGN KEY (model_id) REFERENCES models(id) ON DELETE CASCADE,
              FOREIGN KEY (overlay_id) REFERENCES model_metadata_overlays(overlay_id) ON DELETE SET NULL
            );

            CREATE INDEX IF NOT EXISTS idx_model_metadata_history_model
              ON model_metadata_history(model_id, created_at);

            CREATE TABLE IF NOT EXISTS dependency_profiles (
              profile_id TEXT NOT NULL,
              profile_version INTEGER NOT NULL,
              profile_hash TEXT,
              environment_kind TEXT NOT NULL,
              spec_json TEXT NOT NULL CHECK (json_valid(spec_json)),
              created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
              PRIMARY KEY (profile_id, profile_version)
            );

            CREATE INDEX IF NOT EXISTS idx_dependency_profiles_hash
              ON dependency_profiles(profile_hash);

            CREATE TABLE IF NOT EXISTS model_dependency_bindings (
              binding_id TEXT PRIMARY KEY,
              model_id TEXT NOT NULL,
              profile_id TEXT NOT NULL,
              profile_version INTEGER NOT NULL,
              binding_kind TEXT NOT NULL,
              backend_key TEXT,
              platform_selector TEXT,
              status TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'deprecated')),
              priority INTEGER NOT NULL DEFAULT 100,
              attached_by TEXT,
              attached_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
              FOREIGN KEY (model_id) REFERENCES models(id) ON DELETE CASCADE,
              FOREIGN KEY (profile_id, profile_version) REFERENCES dependency_profiles(profile_id, profile_version)
            );

            CREATE INDEX IF NOT EXISTS idx_model_dependency_bindings_model
              ON model_dependency_bindings(model_id, status, binding_kind, backend_key, priority, binding_id);

            CREATE TABLE IF NOT EXISTS dependency_binding_history (
              event_id INTEGER PRIMARY KEY AUTOINCREMENT,
              binding_id TEXT NOT NULL,
              model_id TEXT NOT NULL,
              actor TEXT NOT NULL,
              action TEXT NOT NULL,
              old_value_json TEXT,
              new_value_json TEXT,
              reason TEXT,
              created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
              FOREIGN KEY (binding_id) REFERENCES model_dependency_bindings(binding_id) ON DELETE CASCADE,
              FOREIGN KEY (model_id) REFERENCES models(id) ON DELETE CASCADE
            );
            ",
        )?;

        Ok(())
    }

    /// Seed idempotent baseline rows for mapping/rule tables.
    fn seed_metadata_v2_rows(conn: &Connection) -> Result<()> {
        conn.execute_batch(
            "
            INSERT OR IGNORE INTO task_signature_mappings (
              signature_key, mapping_version, input_modalities_json, output_modalities_json, task_type_primary, priority, status, source
            ) VALUES
              ('text->text', 1, '[\"text\"]', '[\"text\"]', 'text-generation', 100, 'active', 'system'),
              ('text->image', 1, '[\"text\"]', '[\"image\"]', 'text-to-image', 100, 'active', 'system'),
              ('image->image', 1, '[\"image\"]', '[\"image\"]', 'image-to-image', 100, 'active', 'system'),
              ('text+image->image', 1, '[\"text\",\"image\"]', '[\"image\"]', 'text-image-to-image', 100, 'active', 'system'),
              ('text->audio', 1, '[\"text\"]', '[\"audio\"]', 'text-to-audio', 100, 'active', 'system'),
              ('audio->audio', 1, '[\"audio\"]', '[\"audio\"]', 'audio-to-audio', 100, 'active', 'system'),
              ('audio->text', 1, '[\"audio\"]', '[\"text\"]', 'audio-to-text', 100, 'active', 'system'),
              ('text->embedding', 1, '[\"text\"]', '[\"embedding\"]', 'text-embedding', 100, 'active', 'system'),
              ('image->embedding', 1, '[\"image\"]', '[\"embedding\"]', 'image-embedding', 100, 'active', 'system'),
              ('audio->embedding', 1, '[\"audio\"]', '[\"embedding\"]', 'audio-embedding', 100, 'active', 'system'),
              ('image->text', 1, '[\"image\"]', '[\"text\"]', 'image-to-text', 100, 'active', 'system'),
              ('video->text', 1, '[\"video\"]', '[\"text\"]', 'video-to-text', 100, 'active', 'system'),
              ('text+image->text', 1, '[\"text\",\"image\"]', '[\"text\"]', 'visual-question-answering', 100, 'active', 'system'),
              ('text+video->text', 1, '[\"text\",\"video\"]', '[\"text\"]', 'video-question-answering', 100, 'active', 'system'),
              ('text->video', 1, '[\"text\"]', '[\"video\"]', 'text-to-video', 100, 'active', 'system'),
              ('text->3d', 1, '[\"text\"]', '[\"3d\"]', 'text-to-3d', 100, 'active', 'system'),
              ('image->3d', 1, '[\"image\"]', '[\"3d\"]', 'image-to-3d', 100, 'active', 'system');

            INSERT OR IGNORE INTO model_type_arch_rules (pattern, match_style, model_type, priority, status, source) VALUES
              ('ForCausalLM', 'suffix', 'llm', 100, 'active', 'system'),
              ('ForMaskedLM', 'suffix', 'llm', 100, 'active', 'system'),
              ('ForConditionalGeneration', 'suffix', 'llm', 100, 'active', 'system'),
              ('ForSequenceClassification', 'suffix', 'llm', 100, 'active', 'system'),
              ('ForTokenClassification', 'suffix', 'llm', 100, 'active', 'system'),
              ('ForQuestionAnswering', 'suffix', 'llm', 100, 'active', 'system'),
              ('ForSpeechSeq2Seq', 'suffix', 'audio', 100, 'active', 'system'),
              ('ForAudioClassification', 'suffix', 'audio', 100, 'active', 'system'),
              ('Whisper', 'prefix', 'audio', 100, 'active', 'system'),
              ('Encodec', 'prefix', 'audio', 100, 'active', 'system'),
              ('ForImageClassification', 'suffix', 'vision', 100, 'active', 'system'),
              ('ForObjectDetection', 'suffix', 'vision', 100, 'active', 'system'),
              ('ForSemanticSegmentation', 'suffix', 'vision', 100, 'active', 'system'),
              ('ForImageSegmentation', 'suffix', 'vision', 100, 'active', 'system'),
              ('CLIPVisionModel', 'prefix', 'vision', 100, 'active', 'system'),
              ('UNet2DConditionModel', 'exact', 'diffusion', 100, 'active', 'system'),
              ('UNet2DModel', 'exact', 'diffusion', 100, 'active', 'system'),
              ('AutoencoderKL', 'exact', 'diffusion', 100, 'active', 'system'),
              ('VQModel', 'exact', 'diffusion', 100, 'active', 'system'),
              ('StableDiffusion*Pipeline', 'wildcard', 'diffusion', 100, 'active', 'system'),
              ('DiffusionPipeline', 'exact', 'diffusion', 100, 'active', 'system');

            INSERT OR IGNORE INTO model_type_config_rules (config_model_type, model_type, priority, status, source) VALUES
              ('llama', 'llm', 100, 'active', 'system'),
              ('mistral', 'llm', 100, 'active', 'system'),
              ('mixtral', 'llm', 100, 'active', 'system'),
              ('gpt2', 'llm', 100, 'active', 'system'),
              ('gpt_neo', 'llm', 100, 'active', 'system'),
              ('gpt_neox', 'llm', 100, 'active', 'system'),
              ('gptj', 'llm', 100, 'active', 'system'),
              ('phi', 'llm', 100, 'active', 'system'),
              ('phi3', 'llm', 100, 'active', 'system'),
              ('qwen2', 'llm', 100, 'active', 'system'),
              ('qwen3', 'llm', 100, 'active', 'system'),
              ('gemma', 'llm', 100, 'active', 'system'),
              ('gemma2', 'llm', 100, 'active', 'system'),
              ('gemma3', 'llm', 100, 'active', 'system'),
              ('deepseek_v2', 'llm', 100, 'active', 'system'),
              ('deepseek_v3', 'llm', 100, 'active', 'system'),
              ('falcon', 'llm', 100, 'active', 'system'),
              ('mpt', 'llm', 100, 'active', 'system'),
              ('bloom', 'llm', 100, 'active', 'system'),
              ('opt', 'llm', 100, 'active', 'system'),
              ('codegen', 'llm', 100, 'active', 'system'),
              ('starcoder2', 'llm', 100, 'active', 'system'),
              ('rwkv', 'llm', 100, 'active', 'system'),
              ('rwkv5', 'llm', 100, 'active', 'system'),
              ('rwkv6', 'llm', 100, 'active', 'system'),
              ('mamba', 'llm', 100, 'active', 'system'),
              ('mamba2', 'llm', 100, 'active', 'system'),
              ('jamba', 'llm', 100, 'active', 'system'),
              ('dbrx', 'llm', 100, 'active', 'system'),
              ('stablelm', 'llm', 100, 'active', 'system'),
              ('stable_diffusion', 'diffusion', 100, 'active', 'system'),
              ('sdxl', 'diffusion', 100, 'active', 'system'),
              ('kandinsky', 'diffusion', 100, 'active', 'system'),
              ('pixart', 'diffusion', 100, 'active', 'system'),
              ('whisper', 'audio', 100, 'active', 'system'),
              ('wav2vec2', 'audio', 100, 'active', 'system'),
              ('hubert', 'audio', 100, 'active', 'system'),
              ('wavlm', 'audio', 100, 'active', 'system'),
              ('seamless_m4t', 'audio', 100, 'active', 'system'),
              ('bark', 'audio', 100, 'active', 'system'),
              ('musicgen', 'audio', 100, 'active', 'system'),
              ('encodec', 'audio', 100, 'active', 'system'),
              ('speecht5', 'audio', 100, 'active', 'system'),
              ('mms', 'audio', 100, 'active', 'system'),
              ('vit', 'vision', 100, 'active', 'system'),
              ('swin', 'vision', 100, 'active', 'system'),
              ('convnext', 'vision', 100, 'active', 'system'),
              ('deit', 'vision', 100, 'active', 'system'),
              ('beit', 'vision', 100, 'active', 'system'),
              ('dinov2', 'vision', 100, 'active', 'system'),
              ('clip', 'vision', 100, 'active', 'system'),
              ('siglip', 'vision', 100, 'active', 'system'),
              ('blip', 'vision', 100, 'active', 'system'),
              ('blip2', 'vision', 100, 'active', 'system'),
              ('sentence-transformers', 'embedding', 100, 'active', 'system'),
              ('bge', 'embedding', 100, 'active', 'system'),
              ('e5', 'embedding', 100, 'active', 'system'),
              ('gte', 'embedding', 100, 'active', 'system'),
              ('jina-embeddings', 'embedding', 100, 'active', 'system');
            ",
        )?;

        conn.execute_batch(
            "
            INSERT OR IGNORE INTO model_metadata_baselines (model_id, schema_version, baseline_json, created_at, created_by)
            SELECT
              m.id,
              COALESCE(CAST(json_extract(m.metadata_json, '$.schema_version') AS INTEGER), 1),
              m.metadata_json,
              m.updated_at,
              'pumas-library'
            FROM models m;

            INSERT INTO model_metadata_history (
              model_id, overlay_id, actor, action, field_path, old_value_json, new_value_json, reason, created_at
            )
            SELECT
              b.model_id,
              NULL,
              'pumas-library',
              'baseline_created',
              NULL,
              NULL,
              b.baseline_json,
              'migration-backfill',
              b.created_at
            FROM model_metadata_baselines b
            WHERE NOT EXISTS (
              SELECT 1
              FROM model_metadata_history h
              WHERE h.model_id = b.model_id AND h.action = 'baseline_created'
            );
            ",
        )?;

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
    pub fn upsert(&self, record: &ModelRecord) -> Result<()> {
        let conn = self.conn.lock().map_err(|_| PumasError::Database {
            message: "Failed to acquire connection lock".to_string(),
            source: None,
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

    // ========================================
    // Link Exclusion Methods
    // ========================================

    /// Set whether a model is excluded from linking for a given app.
    pub fn set_link_exclusion(&self, model_id: &str, app_id: &str, excluded: bool) -> Result<()> {
        let conn = self.conn.lock().map_err(|_| PumasError::Database {
            message: "Failed to acquire connection lock".to_string(),
            source: None,
        })?;

        if excluded {
            let now = chrono::Utc::now().to_rfc3339();
            conn.execute(
                "INSERT OR IGNORE INTO model_link_exclusions (model_id, app_id, excluded_at)
                 VALUES (?1, ?2, ?3)",
                params![model_id, app_id, now],
            )?;
        } else {
            conn.execute(
                "DELETE FROM model_link_exclusions WHERE model_id = ?1 AND app_id = ?2",
                params![model_id, app_id],
            )?;
        }

        Ok(())
    }

    /// Check if a model is excluded from linking for a given app.
    pub fn is_link_excluded(&self, model_id: &str, app_id: &str) -> Result<bool> {
        let conn = self.conn.lock().map_err(|_| PumasError::Database {
            message: "Failed to acquire connection lock".to_string(),
            source: None,
        })?;

        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM model_link_exclusions WHERE model_id = ?1 AND app_id = ?2",
            params![model_id, app_id],
            |row| row.get(0),
        )?;

        Ok(count > 0)
    }

    /// Get all excluded model IDs for a given app.
    pub fn get_excluded_model_ids(&self, app_id: &str) -> Result<Vec<String>> {
        let conn = self.conn.lock().map_err(|_| PumasError::Database {
            message: "Failed to acquire connection lock".to_string(),
            source: None,
        })?;

        let mut stmt =
            conn.prepare("SELECT model_id FROM model_link_exclusions WHERE app_id = ?1")?;
        let rows = stmt.query_map(params![app_id], |row| row.get(0))?;

        let mut ids = Vec::new();
        for row in rows {
            ids.push(row?);
        }

        Ok(ids)
    }

    /// Resolve an active task-signature mapping row.
    pub fn get_active_task_signature_mapping(
        &self,
        signature_key: &str,
    ) -> Result<Option<TaskSignatureMapping>> {
        let conn = self.conn.lock().map_err(|_| PumasError::Database {
            message: "Failed to acquire connection lock".to_string(),
            source: None,
        })?;

        let row = conn
            .query_row(
                "SELECT
                    id,
                    signature_key,
                    mapping_version,
                    input_modalities_json,
                    output_modalities_json,
                    task_type_primary,
                    priority,
                    status,
                    source
                 FROM task_signature_mappings
                 WHERE status = 'active' AND signature_key = ?1
                 ORDER BY priority ASC, mapping_version DESC
                 LIMIT 1",
                params![signature_key],
                |row| {
                    let input_json: String = row.get(3)?;
                    let output_json: String = row.get(4)?;
                    Ok(TaskSignatureMapping {
                        id: row.get(0)?,
                        signature_key: row.get(1)?,
                        mapping_version: row.get(2)?,
                        input_modalities: serde_json::from_str(&input_json).unwrap_or_default(),
                        output_modalities: serde_json::from_str(&output_json).unwrap_or_default(),
                        task_type_primary: row.get(5)?,
                        priority: row.get(6)?,
                        status: row.get(7)?,
                        source: row.get(8)?,
                    })
                },
            )
            .optional()?;

        Ok(row)
    }

    /// Upsert the per-signature pending mapping row used for runtime discovery.
    pub fn upsert_pending_task_signature_mapping(
        &self,
        signature_key: &str,
        input_modalities: &[String],
        output_modalities: &[String],
    ) -> Result<()> {
        let mut conn = self.conn.lock().map_err(|_| PumasError::Database {
            message: "Failed to acquire connection lock".to_string(),
            source: None,
        })?;

        let tx = conn.transaction()?;

        let existing_pending_id: Option<i64> = tx
            .query_row(
                "SELECT id
                 FROM task_signature_mappings
                 WHERE signature_key = ?1 AND status = 'pending'
                 LIMIT 1",
                params![signature_key],
                |row| row.get(0),
            )
            .optional()?;

        if let Some(pending_id) = existing_pending_id {
            tx.execute(
                "UPDATE task_signature_mappings
                 SET
                   input_modalities_json = ?1,
                   output_modalities_json = ?2,
                   updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
                 WHERE id = ?3",
                params![
                    serde_json::to_string(input_modalities)?,
                    serde_json::to_string(output_modalities)?,
                    pending_id
                ],
            )?;
        } else {
            let next_version: i64 = tx.query_row(
                "SELECT COALESCE(MAX(mapping_version), 0) + 1
                 FROM task_signature_mappings
                 WHERE signature_key = ?1",
                params![signature_key],
                |row| row.get(0),
            )?;

            tx.execute(
                "INSERT INTO task_signature_mappings (
                   signature_key,
                   mapping_version,
                   input_modalities_json,
                   output_modalities_json,
                   task_type_primary,
                   priority,
                   status,
                   source,
                   notes
                 ) VALUES (?1, ?2, ?3, ?4, 'unknown', 100, 'pending', 'runtime-discovered', 'auto-staged unknown signature')",
                params![
                    signature_key,
                    next_version,
                    serde_json::to_string(input_modalities)?,
                    serde_json::to_string(output_modalities)?,
                ],
            )?;
        }

        tx.commit()?;
        Ok(())
    }

    /// List active architecture/class model-type resolver rules.
    pub fn list_active_model_type_arch_rules(&self) -> Result<Vec<ModelTypeArchRule>> {
        let conn = self.conn.lock().map_err(|_| PumasError::Database {
            message: "Failed to acquire connection lock".to_string(),
            source: None,
        })?;

        let mut stmt = conn.prepare(
            "SELECT pattern, match_style, model_type, priority
             FROM model_type_arch_rules
             WHERE status = 'active'
             ORDER BY priority ASC, pattern ASC, match_style ASC",
        )?;

        let rows = stmt.query_map([], |row| {
            Ok(ModelTypeArchRule {
                pattern: row.get(0)?,
                match_style: row.get(1)?,
                model_type: row.get(2)?,
                priority: row.get(3)?,
            })
        })?;

        let mut rules = Vec::new();
        for row in rows {
            rules.push(row?);
        }
        Ok(rules)
    }

    /// List active config.model_type resolver rules.
    pub fn list_active_model_type_config_rules(&self) -> Result<Vec<ModelTypeConfigRule>> {
        let conn = self.conn.lock().map_err(|_| PumasError::Database {
            message: "Failed to acquire connection lock".to_string(),
            source: None,
        })?;

        let mut stmt = conn.prepare(
            "SELECT config_model_type, model_type, priority
             FROM model_type_config_rules
             WHERE status = 'active'
             ORDER BY priority ASC, config_model_type ASC",
        )?;

        let rows = stmt.query_map([], |row| {
            Ok(ModelTypeConfigRule {
                config_model_type: row.get(0)?,
                model_type: row.get(1)?,
                priority: row.get(2)?,
            })
        })?;

        let mut rules = Vec::new();
        for row in rows {
            rules.push(row?);
        }
        Ok(rules)
    }

    /// Clear all models from the index.
    pub fn clear(&self) -> Result<()> {
        let conn = self.conn.lock().map_err(|_| PumasError::Database {
            message: "Failed to acquire connection lock".to_string(),
            source: None,
        })?;

        // Delete from FTS5 table first, then models table.
        // This avoids "Execute returned results" error from FTS5 triggers since the
        // AFTER DELETE trigger will find nothing to delete from the FTS5 table.
        let fts_table = &self.fts5_config.table_name;
        conn.execute_batch(&format!("DELETE FROM {}; DELETE FROM models;", fts_table))?;
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
                    'dependency_binding_history'
                 )",
            )
            .unwrap();
        let rows = stmt
            .query_map([], |row| row.get::<_, String>(0))
            .unwrap()
            .collect::<std::result::Result<Vec<_>, _>>()
            .unwrap();

        assert_eq!(rows.len(), 9);
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
        assert!(config_rules.iter().any(|r| r.config_model_type == "llama"));
    }
}
