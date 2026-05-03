use super::{ModelIndex, ModelLibraryUpdateRecord};
use crate::models::{
    ModelFactFamily, ModelLibraryChangeKind, ModelLibraryRefreshScope, ModelLibraryUpdateEvent,
    ModelLibraryUpdateFeed,
};
use crate::{PumasError, Result};
use rusqlite::types::Type;
use rusqlite::{params, Connection, OptionalExtension};

const MODEL_LIBRARY_UPDATE_CURSOR_PREFIX: &str = "model-library-updates:";

impl ModelFactFamily {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::ModelRecord => "model_record",
            Self::Metadata => "metadata",
            Self::PackageFacts => "package_facts",
            Self::DependencyBindings => "dependency_bindings",
            Self::Validation => "validation",
            Self::SearchIndex => "search_index",
        }
    }

    fn from_db(column_index: usize, value: &str) -> rusqlite::Result<Self> {
        match value {
            "model_record" => Ok(Self::ModelRecord),
            "metadata" => Ok(Self::Metadata),
            "package_facts" => Ok(Self::PackageFacts),
            "dependency_bindings" => Ok(Self::DependencyBindings),
            "validation" => Ok(Self::Validation),
            "search_index" => Ok(Self::SearchIndex),
            other => invalid_enum(column_index, "model fact family", other),
        }
    }
}

impl ModelLibraryChangeKind {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::ModelAdded => "model_added",
            Self::ModelRemoved => "model_removed",
            Self::MetadataModified => "metadata_modified",
            Self::PackageFactsModified => "package_facts_modified",
            Self::StaleFactsInvalidated => "stale_facts_invalidated",
            Self::DependencyBindingModified => "dependency_binding_modified",
        }
    }

    fn from_db(column_index: usize, value: &str) -> rusqlite::Result<Self> {
        match value {
            "model_added" => Ok(Self::ModelAdded),
            "model_removed" => Ok(Self::ModelRemoved),
            "metadata_modified" => Ok(Self::MetadataModified),
            "package_facts_modified" => Ok(Self::PackageFactsModified),
            "stale_facts_invalidated" => Ok(Self::StaleFactsInvalidated),
            "dependency_binding_modified" => Ok(Self::DependencyBindingModified),
            other => invalid_enum(column_index, "model-library change kind", other),
        }
    }
}

impl ModelLibraryRefreshScope {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Summary => "summary",
            Self::Detail => "detail",
            Self::SummaryAndDetail => "summary_and_detail",
        }
    }

    fn from_db(column_index: usize, value: &str) -> rusqlite::Result<Self> {
        match value {
            "summary" => Ok(Self::Summary),
            "detail" => Ok(Self::Detail),
            "summary_and_detail" => Ok(Self::SummaryAndDetail),
            other => invalid_enum(column_index, "model-library refresh scope", other),
        }
    }
}

fn invalid_enum<T>(column_index: usize, label: &str, value: &str) -> rusqlite::Result<T> {
    Err(rusqlite::Error::FromSqlConversionFailure(
        column_index,
        Type::Text,
        Box::new(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("invalid {label}: {value}"),
        )),
    ))
}

impl ModelIndex {
    pub(super) fn ensure_model_library_updates_schema(conn: &Connection) -> Result<()> {
        conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS model_library_update_events (
              event_id INTEGER PRIMARY KEY AUTOINCREMENT,
              model_id TEXT NOT NULL,
              selected_artifact_id TEXT,
              change_kind TEXT NOT NULL CHECK (change_kind IN (
                'model_added',
                'model_removed',
                'metadata_modified',
                'package_facts_modified',
                'stale_facts_invalidated',
                'dependency_binding_modified'
              )),
              fact_family TEXT NOT NULL CHECK (fact_family IN (
                'model_record',
                'metadata',
                'package_facts',
                'dependency_bindings',
                'validation',
                'search_index'
              )),
              refresh_scope TEXT NOT NULL CHECK (refresh_scope IN (
                'summary',
                'detail',
                'summary_and_detail'
              )),
              producer_revision TEXT,
              created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
            );

            CREATE INDEX IF NOT EXISTS idx_model_library_update_events_event
              ON model_library_update_events(event_id);
            CREATE INDEX IF NOT EXISTS idx_model_library_update_events_model
              ON model_library_update_events(model_id, event_id);
            ",
        )?;

        Ok(())
    }

    pub(crate) fn append_model_library_update_event_with_conn(
        conn: &Connection,
        model_id: &str,
        change_kind: ModelLibraryChangeKind,
        fact_family: ModelFactFamily,
        refresh_scope: ModelLibraryRefreshScope,
        selected_artifact_id: Option<String>,
        producer_revision: Option<String>,
    ) -> Result<i64> {
        conn.execute(
            "INSERT INTO model_library_update_events (
                model_id,
                selected_artifact_id,
                change_kind,
                fact_family,
                refresh_scope,
                producer_revision
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                model_id,
                selected_artifact_id,
                change_kind.as_str(),
                fact_family.as_str(),
                refresh_scope.as_str(),
                producer_revision,
            ],
        )?;

        Ok(conn.last_insert_rowid())
    }

    pub fn current_model_library_update_cursor(&self) -> Result<String> {
        let conn = self.conn.lock().map_err(|_| PumasError::Database {
            message: "Failed to acquire connection lock".to_string(),
            source: None,
        })?;
        let current = Self::current_model_library_update_event_id_with_conn(&conn)?;
        Ok(model_library_update_cursor(current))
    }

    pub fn list_model_library_updates_since(
        &self,
        cursor: Option<&str>,
        limit: usize,
    ) -> Result<ModelLibraryUpdateFeed> {
        let conn = self.conn.lock().map_err(|_| PumasError::Database {
            message: "Failed to acquire connection lock".to_string(),
            source: None,
        })?;
        let current = Self::current_model_library_update_event_id_with_conn(&conn)?;
        let limit = if limit == 0 { 100 } else { limit.min(1000) };
        let Some(cursor) = cursor else {
            return Ok(ModelLibraryUpdateFeed {
                cursor: model_library_update_cursor(current),
                events: Vec::new(),
                stale_cursor: false,
                snapshot_required: false,
            });
        };

        let Some(after_event_id) = parse_model_library_update_cursor(cursor) else {
            return Ok(ModelLibraryUpdateFeed {
                cursor: model_library_update_cursor(current),
                events: Vec::new(),
                stale_cursor: true,
                snapshot_required: true,
            });
        };

        if Self::is_stale_model_library_update_cursor_with_conn(&conn, after_event_id)? {
            return Ok(ModelLibraryUpdateFeed {
                cursor: model_library_update_cursor(current),
                events: Vec::new(),
                stale_cursor: true,
                snapshot_required: true,
            });
        }

        let mut stmt = conn.prepare(
            "SELECT
                event_id,
                model_id,
                change_kind,
                fact_family,
                refresh_scope,
                selected_artifact_id,
                producer_revision,
                created_at
             FROM model_library_update_events
             WHERE event_id > ?1
             ORDER BY event_id ASC
             LIMIT ?2",
        )?;
        let rows = stmt
            .query_map(params![after_event_id, limit as i64], row_to_update_record)?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        let cursor = rows
            .last()
            .map(|row| model_library_update_cursor(row.event_id))
            .unwrap_or_else(|| model_library_update_cursor(current));
        let events = rows
            .into_iter()
            .map(ModelLibraryUpdateEvent::from)
            .collect();

        Ok(ModelLibraryUpdateFeed {
            cursor,
            events,
            stale_cursor: false,
            snapshot_required: false,
        })
    }

    pub(super) fn current_model_library_update_event_id_with_conn(
        conn: &Connection,
    ) -> Result<i64> {
        Ok(conn.query_row(
            "SELECT COALESCE(MAX(event_id), 0) FROM model_library_update_events",
            [],
            |row| row.get(0),
        )?)
    }

    fn is_stale_model_library_update_cursor_with_conn(
        conn: &Connection,
        after_event_id: i64,
    ) -> Result<bool> {
        if after_event_id == 0 {
            return Ok(false);
        }
        let oldest = conn
            .query_row(
                "SELECT MIN(event_id) FROM model_library_update_events",
                [],
                |row| row.get::<_, Option<i64>>(0),
            )
            .optional()?
            .flatten();
        Ok(oldest.is_some_and(|oldest| after_event_id < oldest - 1))
    }
}

impl From<ModelLibraryUpdateRecord> for ModelLibraryUpdateEvent {
    fn from(record: ModelLibraryUpdateRecord) -> Self {
        Self {
            cursor: model_library_update_cursor(record.event_id),
            model_id: record.model_id,
            change_kind: record.change_kind,
            fact_family: record.fact_family,
            refresh_scope: record.refresh_scope,
            selected_artifact_id: record.selected_artifact_id,
            producer_revision: record.producer_revision.or(Some(record.created_at)),
        }
    }
}

pub(super) fn model_library_update_cursor(event_id: i64) -> String {
    format!("{MODEL_LIBRARY_UPDATE_CURSOR_PREFIX}{event_id}")
}

fn parse_model_library_update_cursor(cursor: &str) -> Option<i64> {
    cursor
        .strip_prefix(MODEL_LIBRARY_UPDATE_CURSOR_PREFIX)?
        .parse::<i64>()
        .ok()
}

fn row_to_update_record(row: &rusqlite::Row<'_>) -> rusqlite::Result<ModelLibraryUpdateRecord> {
    let change_kind_value: String = row.get(2)?;
    let fact_family_value: String = row.get(3)?;
    let refresh_scope_value: String = row.get(4)?;
    Ok(ModelLibraryUpdateRecord {
        event_id: row.get(0)?,
        model_id: row.get(1)?,
        change_kind: ModelLibraryChangeKind::from_db(2, &change_kind_value)?,
        fact_family: ModelFactFamily::from_db(3, &fact_family_value)?,
        refresh_scope: ModelLibraryRefreshScope::from_db(4, &refresh_scope_value)?,
        selected_artifact_id: row.get(5)?,
        producer_revision: row.get(6)?,
        created_at: row.get(7)?,
    })
}
