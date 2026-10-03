//! Preserve indexed projection data while observing publication-owned gates.
use super::*;

pub(super) fn claims_publication(value: &Value) -> bool {
    value
        .get("import_publication")
        .is_some_and(|value| !value.is_null())
}

/// Malformed or missing evidence is unavailable. Collection observations keep
/// real storage failures distinguishable from a model's invalid evidence.
pub(super) fn evidence_or_unavailable<T>(
    result: Result<T>,
    unavailable: T,
    preserve_io_errors: bool,
) -> Result<T> {
    match result {
        Ok(value) => Ok(value),
        Err(error) if preserve_io_errors && operational_error(&error) => Err(error),
        Err(_) => Ok(unavailable),
    }
}

fn operational_error(error: &PumasError) -> bool {
    match error {
        PumasError::Io {
            source: Some(source),
            ..
        } if source.kind() == std::io::ErrorKind::NotFound => false,
        PumasError::Io { .. } | PumasError::Database { .. } => true,
        PumasError::Json { source, .. } => source.as_ref().is_some_and(serde_json::Error::is_io),
        PumasError::Validation { field, .. }
            if field == super::super::importer::publication::EVIDENCE_SIZE_FIELD =>
        {
            false
        }
        _ => true,
    }
}

fn retain_nonobject_evidence(record: &mut ModelRecord) {
    if !record.metadata.is_object() {
        let raw = std::mem::replace(&mut record.metadata, serde_json::json!({}));
        record.metadata["unparsed_index_metadata"] = raw;
    }
}

fn mark_record_unavailable(record: &mut ModelRecord, path: &Path, code: &str, message: &str) {
    retain_nonobject_evidence(record);
    if !claims_publication(&record.metadata) {
        // This fallback is reached only for an observed publication claim.
        // Preserve that claim so later projections cannot treat it as legacy.
        record.metadata["import_publication"] = serde_json::json!({
            "version": 0, "id": "", "confirmed": false,
        });
    }
    record.metadata["import_state"] = serde_json::json!("pending");
    record.metadata["validation_state"] = serde_json::json!("invalid");
    record.metadata["validation_errors"] = serde_json::json!([{
        "code": code, "message": message, "path": path.display().to_string(),
    }]);
}

impl ModelLibrary {
    pub(super) fn observe_publication_records(&self, records: &mut [ModelRecord]) -> Result<()> {
        for record in records {
            if super::super::importer::publication::indexed_publication_ready(
                &self.library_root,
                &record.id,
                &record.metadata,
            ) {
                continue;
            }
            let model_dir = self.library_root.join(&record.id);
            // Decode only the authority fields, not the extensible projection.
            let mut gates = serde_json::Map::new();
            for field in [
                "import_publication",
                "import_state",
                "validation_state",
                "validation_errors",
            ] {
                if let Some(value) = record.metadata.get(field) {
                    gates.insert(field.into(), value.clone());
                }
            }
            let mut metadata: ModelMetadata = match serde_json::from_value(Value::Object(gates)) {
                Ok(metadata) => metadata,
                Err(_) => {
                    mark_record_unavailable(record, &model_dir, "import_publication_index_invalid", "Indexed copied-publication gates are malformed; original identity evidence is retained for manual diagnosis");
                    continue;
                }
            };
            self.observe_import_readiness_with_io_policy(&model_dir, &mut metadata, true)?;
            let observed = serde_json::to_value(&metadata)?;
            retain_nonobject_evidence(record);
            let original = record.metadata.as_object_mut().expect("object projection");
            for field in ["import_state", "validation_state", "validation_errors"] {
                if let Some(value) = observed.get(field) {
                    original.insert(field.into(), value.clone());
                } else {
                    original.remove(field);
                }
            }
            // Existing producer identity, including malformed raw evidence, is
            // never replaced by observation. A receipt-only claim gets the
            // unavailable identity produced by the shared readiness owner.
            if !original
                .get("import_publication")
                .is_some_and(|value| !value.is_null())
            {
                if let Some(value) = observed.get("import_publication") {
                    original.insert("import_publication".into(), value.clone());
                }
            }
        }
        Ok(())
    }

    pub(super) async fn refresh_retained_publication_record_async(
        &self,
        record: &ModelRecord,
    ) -> Result<bool> {
        let library = self.clone();
        let expected = record.clone();
        tokio::task::spawn_blocking(move || library.refresh_retained_publication_record(&expected))
            .await
            .map_err(|error| {
                PumasError::Other(format!(
                    "Failed to join retained publication observation task: {error}"
                ))
            })?
    }

    pub(super) fn refresh_retained_publication_record(&self, record: &ModelRecord) -> Result<bool> {
        let mut diagnostic = record.clone();
        match self.indexed_model_dir(record) {
            Ok(_) => self.observe_publication_records(std::slice::from_mut(&mut diagnostic))?,
            Err(PumasError::InvalidParams { message }) => mark_record_unavailable(
                &mut diagnostic,
                Path::new(&record.path),
                "import_publication_index_path_invalid",
                &message,
            ),
            Err(error) => return Err(error),
        }
        let changed = self
            .index
            .upsert_projection_if_unchanged(&diagnostic, Some(record))?
            .changed();
        if changed {
            self.index.delete_model_package_facts_cache(&record.id)?;
        }
        Ok(changed)
    }
}
