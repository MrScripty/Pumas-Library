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
    /// Bounded identity candidates only; no metadata projection or index write.
    #[cfg(feature = "s3")]
    pub(crate) fn publication_inspection_candidates(&self) -> Result<Vec<(String, String)>> {
        let conn = self.conn.lock().map_err(|_| PumasError::Database {
            message: "Failed to acquire connection lock".into(),
            source: None,
        })?;
        // Count non-null publication claims, not unrelated library models.
        // Keep malformed non-null claims in the candidate set. json_type also
        // accepts JSON5, so retain strictly invalid JSON for explicit refusal
        // within this same read snapshot rather than silently filtering it out.
        let mut statement = conn.prepare(
            "SELECT id, path, length(CAST(id AS BLOB)), length(CAST(path AS BLOB)),
                    json_valid(metadata_json) FROM models
             WHERE CASE WHEN json_valid(metadata_json)
                        THEN json_type(metadata_json, '$.import_publication') != 'null'
                        ELSE 1 END
             ORDER BY id LIMIT 129",
        )?;
        let mut rows = statement.query([])?;
        let mut result = Vec::new();
        while let Some(row) = rows.next()? {
            if !row.get::<_, bool>(4)? {
                return Err(PumasError::Other(
                    "Indexed publication metadata is not valid JSON".into(),
                ));
            }
            if result.len() == 128 || row.get::<_, i64>(2)? > 1024 || row.get::<_, i64>(3)? > 4096 {
                return Err(PumasError::Validation {
                    field: "s3.inspection.capacity".into(),
                    message: "Publication observation exceeds its capacity".into(),
                });
            }
            result.push((row.get(0)?, row.get(1)?));
        }
        Ok(result)
    }

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
            if has_publication(current) || has_publication(record) {
                // Observing an existing legacy row grants no authority to
                // replace it with a copied publication. Cold admission needs
                // an absent row, and every existing protocol row keeps its ID.
                if !has_publication(current)
                    || !has_publication(record)
                    || current.metadata.pointer("/import_publication/id")
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
    #[test]
    #[cfg(feature = "s3")]
    fn publication_inspection_filters_absent_and_null_claims_before_the_bound() {
        let temp = tempfile::tempdir().unwrap();
        let index = ModelIndex::new(temp.path().join("models.db")).unwrap();
        for count in 0..256 {
            let mut unrelated = record(false);
            unrelated.id = format!("aaa/unrelated/{count:03}");
            unrelated.path = unrelated.id.clone();
            unrelated.metadata = if count % 2 == 0 {
                serde_json::json!({})
            } else {
                serde_json::json!({"import_publication": null})
            };
            index.upsert(&unrelated).unwrap();
        }
        let mut expected = Vec::new();
        // A malformed non-null claim must still face canonical receipt checks.
        for (count, claim) in [
            serde_json::json!({}),
            serde_json::json!(false),
            serde_json::json!("bad"),
        ]
        .into_iter()
        .enumerate()
        {
            let mut candidate = record(false);
            candidate.id = format!("zzz/publication/{count}");
            candidate.path = candidate.id.clone();
            candidate.metadata["import_publication"] = claim;
            index.upsert(&candidate).unwrap();
            expected.push((candidate.id, candidate.path));
        }
        let before: i64 = index
            .conn
            .lock()
            .unwrap()
            .query_row("SELECT total_changes()", [], |row| row.get(0))
            .unwrap();
        let mut events = index.subscribe_model_library_update_events();
        assert_eq!(index.publication_inspection_candidates().unwrap(), expected);
        assert_eq!(index.publication_inspection_candidates().unwrap(), expected);
        assert!(events.try_recv().is_err());
        let after: i64 = index
            .conn
            .lock()
            .unwrap()
            .query_row("SELECT total_changes()", [], |row| row.get(0))
            .unwrap();
        assert_eq!(before, after, "inspection must not mutate the index");
    }

    #[test]
    #[cfg(feature = "s3")]
    fn publication_inspection_keeps_claim_capacity_identity_and_json_refusals() {
        let temp = tempfile::tempdir().unwrap();
        let index = ModelIndex::new(temp.path().join("models.db")).unwrap();
        for count in 0..128 {
            let mut candidate = record(false);
            candidate.id = format!("publication/{count:03}");
            candidate.path = candidate.id.clone();
            index.upsert(&candidate).unwrap();
        }
        assert_eq!(
            index.publication_inspection_candidates().unwrap().len(),
            128
        );
        let extra = record(false);
        index.upsert(&extra).unwrap();
        assert!(
            matches!(index.publication_inspection_candidates(), Err(PumasError::Validation { field, .. }) if field == "s3.inspection.capacity")
        );
        index.delete(&extra.id).unwrap();
        let mut oversized = index.get("publication/000").unwrap().unwrap();
        oversized.path = "é".repeat(2049);
        index.upsert(&oversized).unwrap();
        assert!(
            matches!(index.publication_inspection_candidates(), Err(PumasError::Validation { field, .. }) if field == "s3.inspection.capacity")
        );
        oversized.path = oversized.id.clone();
        index.upsert(&oversized).unwrap();
        index.delete("publication/127").unwrap();
        let mut oversized_id = record(false);
        oversized_id.id = "é".repeat(513);
        index.upsert(&oversized_id).unwrap();
        assert!(
            matches!(index.publication_inspection_candidates(), Err(PumasError::Validation { field, .. }) if field == "s3.inspection.capacity")
        );
        index.delete(&oversized_id.id).unwrap();
        // The FTS triggers reject broken JSON outright, but SQLite accepts
        // JSON5 and can normalize these invalid strict-JSON claims to null.
        for invalid in [
            r#"{"import_publication":NaN}"#,
            r#"{"import_publication":null,}"#,
            "{import_publication:null}",
        ] {
            index
                .conn
                .lock()
                .unwrap()
                .execute(
                    "UPDATE models SET metadata_json = ?1 WHERE id = ?2",
                    params![invalid, oversized.id],
                )
                .unwrap();
            assert!(
                index.publication_inspection_candidates().is_err(),
                "{invalid}"
            );
        }
    }

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
    fn copied_import_conditional_projection_cannot_replace_another_identity() {
        let temp = tempfile::tempdir().unwrap();
        let index = ModelIndex::new(temp.path().join("models.db")).unwrap();
        let mut legacy = record(true);
        legacy.metadata = serde_json::json!({"sentinel": "unrelated legacy asset"});
        let mut unrelated = record(false);
        unrelated.metadata["import_publication"]["id"] =
            serde_json::json!("061387f9-713d-414e-a919-7ebd924f60f8");
        for (current, proposed) in [
            (legacy.clone(), record(false)),
            (legacy.clone(), record(true)),
            (record(false), legacy.clone()),
            (record(true), legacy),
            (record(false), unrelated),
        ] {
            index.upsert(&current).unwrap();
            let before = serde_json::to_vec(&index.get(&current.id).unwrap().unwrap()).unwrap();
            let mut events = index.subscribe_model_library_update_events();
            assert_eq!(
                index
                    .upsert_projection_if_unchanged(&proposed, Some(&current))
                    .unwrap(),
                ProjectionCommit::Conflict
            );
            assert_eq!(
                serde_json::to_vec(&index.get(&current.id).unwrap().unwrap()).unwrap(),
                before
            );
            assert!(events.try_recv().is_err());
        }
        let mut cold = record(true);
        cold.id = "vision/test/cold".into();
        cold.path = cold.id.clone();
        assert_eq!(
            index.upsert_projection_if_unchanged(&cold, None).unwrap(),
            ProjectionCommit::Changed
        );
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
