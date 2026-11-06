//! Explicit schema identity for the model index, kept in `PRAGMA user_version`.
//!
//! The identity covers the base, governance, package-facts, update-feed and
//! FTS5 search objects. The intent tables keep their own identity in
//! `intent_schema_meta`. A database without an identity is new or was written
//! by 0.8.0 or earlier; it is recognised by the established shape checks once,
//! brought to the current shape and stamped. A newer or unknown identity is
//! refused before any write.

use super::*;

/// The model index schema this build reads and writes.
pub(super) const INDEX_SCHEMA_VERSION: i64 = 1;

/// The `models` columns every Pumas Library release has written.
const MODEL_COLUMNS: [&str; 9] = [
    "id",
    "path",
    "cleaned_name",
    "official_name",
    "model_type",
    "tags_json",
    "hashes_json",
    "metadata_json",
    "updated_at",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum IndexSchemaIdentity {
    /// No identity recorded: a new database, or one written before the
    /// identity existed.
    Unstamped,
    /// Stamped with [`INDEX_SCHEMA_VERSION`].
    Current,
}

impl ModelIndex {
    /// Read the schema identity, refusing a newer, unknown or unrecognisable
    /// database with a reason. Performs no writes.
    pub(super) fn inspect_index_schema_identity(conn: &Connection) -> Result<IndexSchemaIdentity> {
        let version: i64 = conn.pragma_query_value(None, "user_version", |row| row.get(0))?;
        match version {
            0 => {
                Self::recognise_unstamped_index(conn)?;
                Ok(IndexSchemaIdentity::Unstamped)
            }
            INDEX_SCHEMA_VERSION => Ok(IndexSchemaIdentity::Current),
            newer if newer > INDEX_SCHEMA_VERSION => Err(index_schema_error(format!(
                "model index schema version {newer} was written by a newer Pumas Library; \
                 this version supports schema {INDEX_SCHEMA_VERSION}. Open the library with \
                 that newer release"
            ))),
            other => Err(index_schema_error(format!(
                "model index schema version {other} is not one Pumas Library writes"
            ))),
        }
    }

    /// An unstamped database is new (no `models` table) or carries the
    /// `models` table every earlier release created.
    fn recognise_unstamped_index(conn: &Connection) -> Result<()> {
        let mut statement = conn.prepare("PRAGMA table_info(models)")?;
        let columns = statement
            .query_map([], |row| {
                Ok((row.get::<_, String>(1)?, row.get::<_, i64>(5)?))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        if columns.is_empty() {
            return Ok(());
        }
        let recognised = columns.len() == MODEL_COLUMNS.len()
            && columns.iter().zip(MODEL_COLUMNS).enumerate().all(
                |(position, ((name, primary_key), expected))| {
                    name == expected && *primary_key == i64::from(position == 0)
                },
            );
        if recognised {
            Ok(())
        } else {
            Err(index_schema_error(
                "model index has no schema version and its models table is not one Pumas \
                 Library created"
                    .to_string(),
            ))
        }
    }

    /// Bring the index to the current schema and record its identity, as one
    /// transaction. Returns the identity found before the change.
    pub(super) fn ensure_index_schema(
        conn: &mut Connection,
        fts5_config: &FTS5Config,
    ) -> Result<IndexSchemaIdentity> {
        let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        // Another process may have stamped the database since the first look.
        let identity = Self::inspect_index_schema_identity(&tx)?;
        Self::ensure_schema(&tx, identity)?;
        let fts5 = FTS5Manager::new(fts5_config);
        match identity {
            IndexSchemaIdentity::Unstamped => {
                fts5.ensure_setup(&tx)?;
                tx.pragma_update(None, "user_version", INDEX_SCHEMA_VERSION)?;
            }
            IndexSchemaIdentity::Current => fts5.ensure_present(&tx)?,
        }
        tx.commit()?;
        Ok(identity)
    }
}

fn index_schema_error(message: String) -> PumasError {
    PumasError::Database {
        message,
        source: None,
    }
}

#[cfg(test)]
mod tests;
