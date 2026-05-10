use super::model_library_updates::model_library_update_cursor;
use super::{ModelIndex, ModelPackageFactsCacheRecord, ModelPackageFactsCacheScope};
use crate::models::{ModelFactFamily, ModelLibraryChangeKind, ModelLibraryRefreshScope};
use crate::models::{
    ModelPackageFactsSummarySnapshot, ModelPackageFactsSummarySnapshotItem,
    ModelPackageFactsSummaryStatus, ResolvedModelPackageFactsSummary,
    PACKAGE_FACTS_CONTRACT_VERSION,
};
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

        if changed {
            let refresh_scope = match record.cache_scope {
                ModelPackageFactsCacheScope::Summary => ModelLibraryRefreshScope::Summary,
                ModelPackageFactsCacheScope::Detail => ModelLibraryRefreshScope::SummaryAndDetail,
            };
            let event_id = Self::append_model_library_update_event_with_conn(
                &conn,
                &record.model_id,
                ModelLibraryChangeKind::PackageFactsModified,
                ModelFactFamily::PackageFacts,
                refresh_scope,
                if record.selected_artifact_id.is_empty() {
                    None
                } else {
                    Some(record.selected_artifact_id.clone())
                },
                record.producer_revision.clone(),
            )?;
            self.publish_model_library_update_event_with_conn(&conn, event_id)?;
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

    pub fn count_model_package_facts_cache_rows(&self, model_id: &str) -> Result<usize> {
        let conn = self.conn.lock().map_err(|_| PumasError::Database {
            message: "Failed to acquire connection lock".to_string(),
            source: None,
        })?;

        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM model_package_facts_cache WHERE model_id = ?1",
            params![model_id],
            |row| row.get(0),
        )?;

        Ok(count as usize)
    }

    pub fn count_model_package_facts_cache_rows_without_selected_artifact(
        &self,
        model_id: &str,
    ) -> Result<usize> {
        let conn = self.conn.lock().map_err(|_| PumasError::Database {
            message: "Failed to acquire connection lock".to_string(),
            source: None,
        })?;

        let count: i64 = conn.query_row(
            "SELECT COUNT(*)
             FROM model_package_facts_cache
             WHERE model_id = ?1 AND selected_artifact_id = ''",
            params![model_id],
            |row| row.get(0),
        )?;

        Ok(count as usize)
    }

    pub fn list_model_package_facts_summary_snapshot(
        &self,
        limit: usize,
        offset: usize,
    ) -> Result<ModelPackageFactsSummarySnapshot> {
        let conn = self.conn.lock().map_err(|_| PumasError::Database {
            message: "Failed to acquire connection lock".to_string(),
            source: None,
        })?;
        let limit = if limit == 0 { 500 } else { limit.min(1000) };

        let mut stmt = conn.prepare(
            "SELECT
                models.id,
                model_package_facts_cache.selected_artifact_id,
                model_package_facts_cache.package_facts_contract_version,
                model_package_facts_cache.facts_json
             FROM models
             LEFT JOIN model_package_facts_cache
               ON model_package_facts_cache.model_id = models.id
              AND model_package_facts_cache.selected_artifact_id = ''
              AND model_package_facts_cache.cache_scope = 'summary'
             ORDER BY models.id ASC
             LIMIT ?1 OFFSET ?2",
        )?;
        let items = stmt
            .query_map(params![limit as i64, offset as i64], |row| {
                let model_id: String = row.get(0)?;
                let selected_artifact_id: Option<String> = row.get(1)?;
                let package_facts_contract_version: Option<i64> = row.get(2)?;
                let facts_json: Option<String> = row.get(3)?;
                let (status, summary) = classify_package_facts_summary_cache_row(
                    None,
                    selected_artifact_id.as_deref(),
                    package_facts_contract_version,
                    facts_json.as_deref(),
                );
                Ok(ModelPackageFactsSummarySnapshotItem {
                    model_id,
                    status,
                    summary,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        let cursor = model_library_update_cursor(
            Self::current_model_library_update_event_id_with_conn(&conn)?,
        );

        Ok(ModelPackageFactsSummarySnapshot { cursor, items })
    }
}

pub(super) fn classify_package_facts_summary_cache_row(
    expected_selected_artifact_id: Option<&str>,
    row_selected_artifact_id: Option<&str>,
    row_package_facts_contract_version: Option<i64>,
    facts_json: Option<&str>,
) -> (
    ModelPackageFactsSummaryStatus,
    Option<ResolvedModelPackageFactsSummary>,
) {
    let Some(facts_json) = facts_json else {
        return (ModelPackageFactsSummaryStatus::Missing, None);
    };
    let expected_selected_artifact_id =
        normalized_selected_artifact_id(expected_selected_artifact_id);
    let row_selected_artifact_id = normalized_selected_artifact_id(row_selected_artifact_id);
    if row_selected_artifact_id != expected_selected_artifact_id {
        return (ModelPackageFactsSummaryStatus::WrongSelectedArtifact, None);
    }
    if row_package_facts_contract_version != Some(i64::from(PACKAGE_FACTS_CONTRACT_VERSION)) {
        return (ModelPackageFactsSummaryStatus::StaleContract, None);
    }

    match serde_json::from_str::<ResolvedModelPackageFactsSummary>(&facts_json) {
        Ok(summary) if summary.package_facts_contract_version != PACKAGE_FACTS_CONTRACT_VERSION => {
            (ModelPackageFactsSummaryStatus::StaleContract, None)
        }
        Ok(summary)
            if expected_selected_artifact_id.is_some()
                && normalized_selected_artifact_id(
                    summary.model_ref.selected_artifact_id.as_deref(),
                ) != expected_selected_artifact_id =>
        {
            (ModelPackageFactsSummaryStatus::WrongSelectedArtifact, None)
        }
        Ok(summary) => (ModelPackageFactsSummaryStatus::Cached, Some(summary)),
        Err(_) => (ModelPackageFactsSummaryStatus::Invalid, None),
    }
}

fn normalized_selected_artifact_id(value: Option<&str>) -> Option<&str> {
    value.and_then(|value| {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        }
    })
}
