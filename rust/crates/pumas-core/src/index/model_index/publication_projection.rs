//! Conditional metadata projections. Expected rows are captured before any
//! filesystem observation. Transactions contain SQL and serialization only.
use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ProjectionCommit {
    Changed,
    Unchanged,
    Conflict,
}
impl ProjectionCommit {
    pub(crate) fn changed(self) -> bool {
        self == Self::Changed
    }
}

pub(crate) fn has_publication(record: &ModelRecord) -> bool {
    record
        .metadata
        .get("import_publication")
        .is_some_and(|value| !value.is_null())
}

impl ModelIndex {
    pub(crate) fn import_publication_owner(
        &self,
        publication_id: &str,
    ) -> Result<Option<ModelRecord>> {
        let conn = self.conn.lock().map_err(|_| PumasError::Database {
            message: "Failed to acquire connection lock".into(),
            source: None,
        })?;
        Ok(conn.query_row("SELECT id, path, cleaned_name, official_name, model_type, tags_json, hashes_json, metadata_json, updated_at FROM models WHERE json_extract(metadata_json, '$.import_publication.id') = ?1 LIMIT 1", params![publication_id], Self::row_to_record).optional()?)
    }

    pub(crate) fn upsert_projection_if_unchanged(
        &self,
        record: &ModelRecord,
        expected: Option<&ModelRecord>,
    ) -> Result<ProjectionCommit> {
        self.commit_publication_projection(record, expected, false)
    }

    pub(crate) fn finalize_import_if_unchanged(
        &self,
        record: &ModelRecord,
        expected: &ModelRecord,
    ) -> Result<ProjectionCommit> {
        self.commit_publication_projection(record, Some(expected), true)
    }

    fn commit_publication_projection(
        &self,
        record: &ModelRecord,
        expected: Option<&ModelRecord>,
        producer: bool,
    ) -> Result<ProjectionCommit> {
        let mut conn = self.conn.lock().map_err(|_| PumasError::Database {
            message: "Failed to acquire connection lock".into(),
            source: None,
        })?;
        let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let current = Self::projection_row(&tx, &record.id)?;
        if serde_json::to_value(&current)? != serde_json::to_value(expected)? {
            return Ok(ProjectionCommit::Conflict);
        }
        if let Some(current) = &current {
            if has_publication(current) {
                if current.metadata.pointer("/import_publication/id")
                    != record.metadata.pointer("/import_publication/id")
                {
                    return Ok(ProjectionCommit::Conflict);
                }
                let was_ready = crate::models::copied_import_ready_value(&current.metadata);
                let next_ready = crate::models::copied_import_ready_value(&record.metadata);
                if was_ready && !next_ready && !producer {
                    let claimed: bool = tx.query_row(
                        "SELECT EXISTS(SELECT 1 FROM intent_deletion_claims WHERE model_id = ?1)",
                        params![record.id],
                        |row| row.get(0),
                    )?;
                    if claimed {
                        return Ok(ProjectionCommit::Conflict);
                    }
                }
                if !was_ready && next_ready && !producer {
                    return Ok(ProjectionCommit::Conflict);
                }
                if producer && (was_ready || !next_ready) {
                    return Ok(ProjectionCommit::Conflict);
                }
            } else if producer {
                return Ok(ProjectionCommit::Conflict);
            }
        } else if producer {
            return Ok(ProjectionCommit::Conflict);
        }
        let (changed, event) = self.upsert_with_conn(&tx, record)?;
        tx.commit()?;
        if let Some(event) = event {
            self.publish_model_library_update_event_with_conn(&conn, event)?;
        }
        Ok(if changed {
            ProjectionCommit::Changed
        } else {
            ProjectionCommit::Unchanged
        })
    }

    /// Rebuild pruning never discards a copied-import fence. An absent payload
    /// remains diagnostic until an explicit supported mutation disposes of it.
    pub(crate) fn prune_legacy_projection_if_unchanged(
        &self,
        expected: &ModelRecord,
    ) -> Result<bool> {
        if has_publication(expected) {
            return Ok(false);
        }
        let mut conn = self.conn.lock().map_err(|_| PumasError::Database {
            message: "Failed to acquire connection lock".into(),
            source: None,
        })?;
        let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let current = Self::projection_row(&tx, &expected.id)?;
        if serde_json::to_value(&current)? != serde_json::to_value(Some(expected))? {
            return Ok(false);
        }
        let changed = tx.execute("DELETE FROM models WHERE id = ?1", params![expected.id])? > 0;
        let event = if changed {
            Some(Self::append_model_library_update_event_with_conn(
                &tx,
                &expected.id,
                ModelLibraryChangeKind::ModelRemoved,
                ModelFactFamily::ModelRecord,
                ModelLibraryRefreshScope::SummaryAndDetail,
                None,
                None,
            )?)
        } else {
            None
        };
        tx.commit()?;
        if let Some(event) = event {
            self.publish_model_library_update_event_with_conn(&conn, event)?;
        }
        Ok(changed)
    }

    pub(super) fn projection_row(conn: &Connection, id: &str) -> Result<Option<ModelRecord>> {
        Ok(conn.query_row("SELECT id, path, cleaned_name, official_name, model_type, tags_json, hashes_json, metadata_json, updated_at FROM models WHERE id = ?1", params![id], Self::row_to_record).optional()?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn record(ready: bool) -> ModelRecord {
        ModelRecord {
            id: "vision/test/model".into(),
            path: "vision/test/model".into(),
            cleaned_name: "model".into(),
            official_name: "model".into(),
            model_type: "vision".into(),
            tags: vec![],
            hashes: HashMap::new(),
            updated_at: "2026-10-03T00:00:00Z".into(),
            metadata: serde_json::json!({
                "import_publication": {"version":1,"id":"5c542afa-eafd-410a-8b57-d56296d2e2a4","confirmed":ready},
                "import_state": if ready { "ready" } else { "pending" },
                "validation_state": if ready { "valid" } else { "invalid" }
            }),
        }
    }
    #[test]
    fn copied_import_conditional_projection_cannot_downgrade_newer_ready() {
        let temp = tempfile::tempdir().unwrap();
        let index = ModelIndex::new(temp.path().join("models.db")).unwrap();
        let watcher = ModelIndex::new(temp.path().join("models.db")).unwrap();
        let pending = record(false);
        index.upsert(&pending).unwrap();
        let observed = watcher.get(&pending.id).unwrap().unwrap();
        assert_eq!(
            watcher
                .upsert_projection_if_unchanged(&record(true), Some(&observed))
                .unwrap(),
            ProjectionCommit::Conflict
        );
        assert_eq!(
            index
                .finalize_import_if_unchanged(&record(true), &pending)
                .unwrap(),
            ProjectionCommit::Changed
        );
        let mut events = watcher.subscribe_model_library_update_events();
        assert_eq!(
            watcher
                .upsert_projection_if_unchanged(&pending, Some(&observed))
                .unwrap(),
            ProjectionCommit::Conflict
        );
        assert!(events.try_recv().is_err());
        assert!(crate::models::copied_import_ready_value(
            &watcher.get(&pending.id).unwrap().unwrap().metadata
        ));
        assert!(!watcher
            .prune_legacy_projection_if_unchanged(&pending)
            .unwrap());
    }
    #[test]
    fn copied_import_conditional_commit_checks_full_observed_row() {
        let temp = tempfile::tempdir().unwrap();
        let index = ModelIndex::new(temp.path().join("models.db")).unwrap();
        let pending = record(false);
        index.upsert(&pending).unwrap();
        let mut changed = pending.clone();
        changed.official_name = "concurrent diagnostic".into();
        index.upsert(&changed).unwrap();
        assert_eq!(
            index
                .finalize_import_if_unchanged(&record(true), &pending)
                .unwrap(),
            ProjectionCommit::Conflict
        );
        assert_eq!(
            index.get(&pending.id).unwrap().unwrap().official_name,
            changed.official_name
        );
        let mut legacy = record(false);
        legacy.metadata = serde_json::json!({});
        index.upsert(&legacy).unwrap();
        let mut newer = legacy.clone();
        newer.official_name = "newer".into();
        index.upsert(&newer).unwrap();
        assert!(!index.prune_legacy_projection_if_unchanged(&legacy).unwrap());
        assert!(index.prune_legacy_projection_if_unchanged(&newer).unwrap());
    }
}
