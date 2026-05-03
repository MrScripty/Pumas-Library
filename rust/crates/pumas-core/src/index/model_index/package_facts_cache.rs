use super::{ModelIndex, ModelPackageFactsCacheRecord, ModelPackageFactsCacheScope};
use crate::models::{ModelFactFamily, ModelLibraryChangeKind, ModelLibraryRefreshScope};
use crate::{PumasError, Result};
use rusqlite::types::Type;
use rusqlite::{params, Connection, OptionalExtension};

impl ModelPackageFactsCacheScope {
    fn from_db(column_index: usize, value: &str) -> rusqlite::Result<Self> {
        match value {
            "summary" => Ok(Self::Summary),
            "detail" => Ok(Self::Detail),
            other => Err(rusqlite::Error::FromSqlConversionFailure(
                column_index,
                Type::Text,
                Box::new(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("invalid model package facts cache scope: {other}"),
                )),
            )),
        }
    }
}

impl ModelIndex {
    pub(super) fn ensure_package_facts_cache_schema(conn: &Connection) -> Result<()> {
        conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS model_package_facts_cache (
              model_id TEXT NOT NULL,
              selected_artifact_id TEXT NOT NULL DEFAULT '',
              cache_scope TEXT NOT NULL CHECK (cache_scope IN ('summary', 'detail')),
              package_facts_contract_version INTEGER NOT NULL,
              producer_revision TEXT,
              source_fingerprint TEXT NOT NULL,
              facts_json TEXT NOT NULL CHECK (json_valid(facts_json)),
              cached_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
              updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
              PRIMARY KEY (model_id, selected_artifact_id, cache_scope),
              FOREIGN KEY (model_id) REFERENCES models(id) ON DELETE CASCADE
            );

            CREATE INDEX IF NOT EXISTS idx_model_package_facts_cache_model
              ON model_package_facts_cache(model_id, cache_scope, selected_artifact_id);
            ",
        )?;

        Ok(())
    }

    pub fn upsert_model_package_facts_cache(
        &self,
        record: &ModelPackageFactsCacheRecord,
    ) -> Result<bool> {
        let conn = self.conn.lock().map_err(|_| PumasError::Database {
            message: "Failed to acquire connection lock".to_string(),
            source: None,
        })?;

        let changed = conn.execute(
            "INSERT INTO model_package_facts_cache (
                model_id,
                selected_artifact_id,
                cache_scope,
                package_facts_contract_version,
                producer_revision,
                source_fingerprint,
                facts_json,
                cached_at,
                updated_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
             ON CONFLICT(model_id, selected_artifact_id, cache_scope) DO UPDATE SET
                package_facts_contract_version = excluded.package_facts_contract_version,
                producer_revision = excluded.producer_revision,
                source_fingerprint = excluded.source_fingerprint,
                facts_json = excluded.facts_json,
                updated_at = excluded.updated_at
             WHERE package_facts_contract_version IS NOT excluded.package_facts_contract_version
                OR producer_revision IS NOT excluded.producer_revision
                OR source_fingerprint IS NOT excluded.source_fingerprint
                OR facts_json IS NOT excluded.facts_json",
            params![
                record.model_id,
                record.selected_artifact_id,
                record.cache_scope.as_str(),
                record.package_facts_contract_version,
                record.producer_revision,
                record.source_fingerprint,
                record.facts_json,
                record.cached_at,
                record.updated_at,
            ],
        )? > 0;

        if changed && record.cache_scope == ModelPackageFactsCacheScope::Detail {
            Self::append_model_library_update_event_with_conn(
                &conn,
                &record.model_id,
                ModelLibraryChangeKind::PackageFactsModified,
                ModelFactFamily::PackageFacts,
                ModelLibraryRefreshScope::SummaryAndDetail,
                if record.selected_artifact_id.is_empty() {
                    None
                } else {
                    Some(record.selected_artifact_id.clone())
                },
                record.producer_revision.clone(),
            )?;
        }

        Ok(changed)
    }

    pub fn get_model_package_facts_cache(
        &self,
        model_id: &str,
        selected_artifact_id: Option<&str>,
        cache_scope: ModelPackageFactsCacheScope,
    ) -> Result<Option<ModelPackageFactsCacheRecord>> {
        let conn = self.conn.lock().map_err(|_| PumasError::Database {
            message: "Failed to acquire connection lock".to_string(),
            source: None,
        })?;
        let selected_artifact_id = selected_artifact_id.unwrap_or_default();

        let result = conn
            .query_row(
                "SELECT
                    model_id,
                    selected_artifact_id,
                    cache_scope,
                    package_facts_contract_version,
                    producer_revision,
                    source_fingerprint,
                    facts_json,
                    cached_at,
                    updated_at
                 FROM model_package_facts_cache
                 WHERE model_id = ?1
                   AND selected_artifact_id = ?2
                   AND cache_scope = ?3",
                params![model_id, selected_artifact_id, cache_scope.as_str()],
                |row| {
                    let cache_scope_value: String = row.get(2)?;
                    let cache_scope = ModelPackageFactsCacheScope::from_db(2, &cache_scope_value)?;
                    Ok(ModelPackageFactsCacheRecord {
                        model_id: row.get(0)?,
                        selected_artifact_id: row.get(1)?,
                        cache_scope,
                        package_facts_contract_version: row.get(3)?,
                        producer_revision: row.get(4)?,
                        source_fingerprint: row.get(5)?,
                        facts_json: row.get(6)?,
                        cached_at: row.get(7)?,
                        updated_at: row.get(8)?,
                    })
                },
            )
            .optional()?;

        Ok(result)
    }

    pub fn delete_model_package_facts_cache(&self, model_id: &str) -> Result<usize> {
        let conn = self.conn.lock().map_err(|_| PumasError::Database {
            message: "Failed to acquire connection lock".to_string(),
            source: None,
        })?;

        Ok(conn.execute(
            "DELETE FROM model_package_facts_cache WHERE model_id = ?1",
            params![model_id],
        )?)
    }
}
