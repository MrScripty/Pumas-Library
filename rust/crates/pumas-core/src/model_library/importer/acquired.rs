//! Acquired model intent and read-only output reconciliation. Only the existing
//! acquisition consumer settles its exact lease after this output proof returns.

use super::*;
use crate::acquisition::{AcquiredArtifactUse, AcquisitionConsumerReceipt};

pub(super) fn recovery_required(message: &str) -> PumasError {
    PumasError::Validation {
        field: "import.acquired_recovery_required".into(),
        message: message.into(),
    }
}

impl ModelImporter {
    fn validate_acquired_model_intent(
        acquired: &AcquiredArtifactUse,
        receipt: &AcquisitionConsumerReceipt,
        spec: &ModelImportSpec,
    ) -> Result<()> {
        let record = acquired.record();
        if record.files.is_empty()
            || record.files.len() != record.manifest.files().len()
            || !record
                .manifest
                .files()
                .iter()
                .any(|file| file.logical_path() == spec.path)
            || record
                .manifest
                .files()
                .iter()
                .any(|file| file.expected_sha256().is_none())
            || receipt.payload != serde_json::to_value(spec)?
        {
            return Err(PumasError::Validation {
                field: "import.acquired".into(),
                message: "Acquired model publication requires the exact selected primary, a fully digest-verified set and its receipt-bound import specification".into(),
            });
        }
        Self::validate_acquired_payload_paths(
            &record
                .files
                .iter()
                .map(|file| file.path.as_str())
                .collect::<Vec<_>>(),
        )?;
        Ok(())
    }

    /// Qualify and atomically publish a supported safe model representation from
    /// held acquisition descriptors. `spec.path` identifies the selected primary
    /// weight file; complete package layout is preserved. Transfer success alone
    /// grants neither model qualification nor permission to execute custom code.
    /// Uses the existing issued receipt, copied-byte verifier and publisher.
    pub async fn import_acquired_model(
        &self,
        acquired: &AcquiredArtifactUse,
        receipt: &AcquisitionConsumerReceipt,
        spec: &ModelImportSpec,
    ) -> Result<ModelImportResult> {
        Self::validate_acquired_model_intent(acquired, receipt, spec)?;
        if !matches!(
            acquired.record().phase,
            crate::acquisition::AcquisitionPhase::Using { .. }
        ) {
            return Err(recovery_required(
                "An adopted model operation cannot be imported again",
            ));
        }
        acquired.require_issued_receipt(receipt).await?;
        let result = self.import_acquired_owned(acquired, receipt, spec).await?;
        if result.success {
            Ok(result)
        } else {
            Err(PumasError::ImportFailed {
                message: result
                    .error
                    .unwrap_or_else(|| "Acquired model import was refused".into()),
            })
        }
    }

    /// Read-only proof of the existing exact receipt-bound publication. Use as
    /// the consumer reconciliation callback; this never replays import or transfer.
    pub async fn reconcile_acquired_model(
        &self,
        acquired: &AcquiredArtifactUse,
        receipt: &AcquisitionConsumerReceipt,
        spec: &ModelImportSpec,
        model_id: &str,
    ) -> Result<ModelImportResult> {
        Self::validate_acquired_model_intent(acquired, receipt, spec)?;
        self.observe_acquired_output(acquired, receipt, spec, model_id)
            .await
    }

    fn validate_acquired_gguf_bundle_intent(
        acquired: &AcquiredArtifactUse,
        receipt: &AcquisitionConsumerReceipt,
        spec: &ModelImportSpec,
    ) -> Result<()> {
        let record = acquired.record();
        // GGUF owns its weights. This bounded bundle accepts inert data/text
        // auxiliaries, not another weight format or executable model code.
        const AUXILIARY_EXTENSIONS: &[&str] =
            &["json", "txt", "md", "model", "tiktoken", "vocab", "merges"];
        if record.manifest.files().len() < 2
            || record.files.len() != record.manifest.files().len()
            || !record
                .manifest
                .files()
                .iter()
                .any(|file| file.logical_path() == spec.path)
            || !Path::new(&spec.path)
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("gguf"))
            || record.manifest.files().iter().any(|file| {
                file.expected_sha256().is_none()
                    || (file.logical_path() != spec.path
                        && !Path::new(file.logical_path())
                            .extension()
                            .and_then(|ext| ext.to_str())
                            .is_some_and(|ext| {
                                AUXILIARY_EXTENSIONS.contains(&ext.to_ascii_lowercase().as_str())
                            }))
            })
            || receipt.payload != serde_json::to_value(spec)?
        {
            return Err(PumasError::Validation { field: "import.acquired_bundle".into(),
                message: "Acquired GGUF bundle requires one exact primary GGUF, digest-verified data/text auxiliaries and its bound import specification".into() });
        }
        Ok(())
    }

    /// Publish an explicitly complete GGUF-plus-auxiliaries selection from held
    /// verified descriptors. The spec identifies the primary logical GGUF path;
    /// auxiliary paths are preserved exactly. Call only after receipt issuance.
    /// Other weight/executable formats, sharded GGUF and implicit auxiliary
    /// discovery remain unsupported. Every selected copy is verified before the
    /// existing model publisher may expose the complete output.
    pub async fn import_acquired_gguf_bundle(
        &self,
        acquired: &AcquiredArtifactUse,
        receipt: &AcquisitionConsumerReceipt,
        spec: &ModelImportSpec,
    ) -> Result<ModelImportResult> {
        Self::validate_acquired_gguf_bundle_intent(acquired, receipt, spec)?;
        if !matches!(
            acquired.record().phase,
            crate::acquisition::AcquisitionPhase::Using { .. }
        ) {
            return Err(recovery_required(
                "An adopted model operation cannot be imported again",
            ));
        }
        acquired.require_issued_receipt(receipt).await?;
        let result = self.import_acquired_owned(acquired, receipt, spec).await?;
        if !result.success {
            return Err(PumasError::ImportFailed {
                message: result
                    .error
                    .unwrap_or_else(|| "Acquired GGUF bundle import was refused".into()),
            });
        }
        Ok(result)
    }

    /// Prove an already confirmed complete GGUF bundle as the callback to the
    /// existing consumer's reconcile operation. Exact paths, every input digest,
    /// current acquisition binding and acknowledged model publication must match.
    /// No transfer, import, output mutation or index repair occurs.
    pub async fn reconcile_acquired_gguf_bundle(
        &self,
        acquired: &AcquiredArtifactUse,
        receipt: &AcquisitionConsumerReceipt,
        spec: &ModelImportSpec,
        model_id: &str,
    ) -> Result<ModelImportResult> {
        Self::validate_acquired_gguf_bundle_intent(acquired, receipt, spec)?;
        self.observe_acquired_output(acquired, receipt, spec, model_id)
            .await
    }

    pub(super) fn validate_acquired_gguf_intent(
        acquired: &AcquiredArtifactUse,
        receipt: &AcquisitionConsumerReceipt,
        spec: &ModelImportSpec,
    ) -> Result<()> {
        let record = acquired.record();
        if record.manifest.files().len() != 1
            || record.files.len() != 1
            || record.manifest.files()[0].logical_path() != spec.path
            || record.manifest.files()[0].expected_sha256().is_none()
            || !Path::new(&spec.path)
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("gguf"))
        {
            return Err(PumasError::Validation {
                field: "import.acquired".into(),
                message: "Acquired GGUF import requires one digest-verified file and its exact logical path".into(),
            });
        }
        if receipt.payload != serde_json::to_value(spec)? {
            return Err(PumasError::Validation {
                field: "import.acquired".into(),
                message: "Model publication receipt must bind the exact import specification"
                    .into(),
            });
        }
        Ok(())
    }

    /// Observe an already confirmed acquired GGUF output under the lifecycle-
    /// owned model root. Use as the validation callback to
    /// `AcquisitionConsumer::reconcile`; that owner acknowledges the exact use.
    /// The model ID is a candidate to inspect, never publication authority.
    /// Pending, legacy unbound, missing or changed outputs remain unresolved.
    /// No transfer, import, index repair, cleanup or Pending promotion occurs.
    pub async fn reconcile_acquired_gguf(
        &self,
        acquired: &AcquiredArtifactUse,
        receipt: &AcquisitionConsumerReceipt,
        spec: &ModelImportSpec,
        model_id: &str,
    ) -> Result<ModelImportResult> {
        Self::validate_acquired_gguf_intent(acquired, receipt, spec)?;
        self.observe_acquired_output(acquired, receipt, spec, model_id)
            .await
    }

    pub(super) async fn observe_acquired_output(
        &self,
        acquired: &AcquiredArtifactUse,
        receipt: &AcquisitionConsumerReceipt,
        spec: &ModelImportSpec,
        model_id: &str,
    ) -> Result<ModelImportResult> {
        acquired.require_issued_receipt(receipt).await?;
        let authority = self.library.mutation_authority()?;
        let library = self.library.clone();
        let receipt = receipt.clone();
        let spec = spec.clone();
        let model_id = model_id.to_owned();
        acquired
            .run_blocking("prove confirmed acquired model output", move || {
                // This closure has no mutation or cleanup effects. An output
                // refusal is the observation result, not an unsettled producer.
                Ok(publication::reconcile_acquired_output(
                    &library, &authority, &receipt, &spec, &model_id,
                ))
            })
            .await?
    }
}
