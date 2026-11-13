//! Finish a copied publication whose final index commit was never acknowledged.
//!
//! A copied import confirms its receipt and writes Ready `metadata.json`
//! before it commits the Ready index row against the Pending row it wrote
//! first. When another writer changed that row in between, the commit
//! conflicts and the publication is retained: Ready on disk, Pending in the
//! index. Observation never promotes it. This explicit operation repeats the
//! finalization proof (confirmed receipt, same publication identity, the
//! exact payload rehashed) under held root authority, then commits the Ready
//! index row from the canonical metadata.
use super::*;
use serde::{Deserialize, Serialize};

/// What [`ModelLibrary::finish_retained_import`] did.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RetainedImportOutcome {
    /// The index now acknowledges the Ready publication.
    Acknowledged,
    /// The index already acknowledged it; nothing changed.
    AlreadyReady,
}

fn refuse(model_id: &str, reason: &str) -> PumasError {
    PumasError::Validation {
        field: "import_publication".into(),
        message: format!("Cannot finish the copied import of {model_id}: {reason}"),
    }
}

impl ModelLibrary {
    pub async fn finish_retained_import(&self, model_id: &str) -> Result<RetainedImportOutcome> {
        let library = self.clone();
        let model_id = model_id.to_string();
        self.store_lifetime
            .spawn_blocking(move || library.finish_retained_import_sync(&model_id))
            .await
            .map_err(|error| {
                PumasError::Other(format!("Failed to join retained import task: {error}"))
            })?
    }

    fn finish_retained_import_sync(&self, model_id: &str) -> Result<RetainedImportOutcome> {
        let authority = self.mutation_authority()?;
        let _grant = authority.root().try_acquire_execution_grant()?;
        let current = self
            .index
            .get(model_id)?
            .ok_or_else(|| PumasError::ModelNotFound {
                model_id: model_id.to_string(),
            })?;
        if !publication_observation::claims_publication(&current.metadata) {
            return Err(refuse(model_id, "it is not a copied publication"));
        }
        if crate::models::copied_import_ready_value(&current.metadata) {
            return Ok(RetainedImportOutcome::AlreadyReady);
        }
        let indexed_id = current
            .metadata
            .pointer("/import_publication/id")
            .and_then(Value::as_str)
            .filter(|id| !id.is_empty())
            .ok_or_else(|| refuse(model_id, "its indexed publication identity is missing"))?
            .to_string();
        let model_dir = self.indexed_model_dir(&current)?;
        if self.get_model_id(&model_dir).as_deref() != Some(model_id) {
            return Err(refuse(model_id, "its indexed path does not belong to it"));
        }
        let destination = authority.root().resolve(&model_dir)?;
        let canonical =
            super::super::importer::publication::read_held_canonical_import_metadata(&destination)?
                .ok_or_else(|| refuse(model_id, "its metadata.json is missing"))?;
        if canonical
            .import_publication
            .as_ref()
            .map(|identity| identity.id.as_str())
            != Some(indexed_id.as_str())
        {
            return Err(refuse(
                model_id,
                "metadata.json and the index name different publications",
            ));
        }
        if canonical
            .model_id
            .as_deref()
            .is_some_and(|canonical_id| canonical_id != model_id)
        {
            return Err(refuse(model_id, "metadata.json names a different model"));
        }
        if !canonical.copied_import_ready() {
            return Err(refuse(
                model_id,
                "the import never confirmed its payload; delete it and import the files again",
            ));
        }
        // Rehash: the same proof the import's own finalization required.
        if !super::super::importer::publication::held_confirmed_receipt_and_content_match(
            &destination,
            &canonical,
        )? {
            return Err(refuse(
                model_id,
                "its confirmed receipt or payload no longer matches; delete it and import the files again",
            ));
        }
        self.finalize_import_index(&current, &destination, &canonical)?;
        self.index.delete_model_package_facts_cache(model_id)?;
        tracing::info!(model_id, "Finished a retained copied publication");
        Ok(RetainedImportOutcome::Acknowledged)
    }
}
