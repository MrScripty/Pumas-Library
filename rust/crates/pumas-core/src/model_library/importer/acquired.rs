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
