//! Explicit receipt-bound GGUF roles. Structural import is not native inference
//! qualification; compatibility and semantic quality remain separate gates.
use super::*;
use crate::acquisition::{AcquiredArtifactUse, AcquisitionConsumerReceipt};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AcquiredGgufVisionTask {
    ImageToText,
}

/// Exact authored roles, serialized unchanged into the issued consumer receipt.
/// Projector naming supports the existing sibling launch protocol; names alone
/// never qualify either file's bytes or model/runtime compatibility.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AcquiredGgufVisionSpec {
    pub primary_model: ModelImportSpec,
    pub vision_projector: String,
    pub task: AcquiredGgufVisionTask,
}

impl AcquiredGgufVisionSpec {
    pub(super) fn validate_paths(&self) -> Result<()> {
        let primary = &self.primary_model.path;
        let projector = &self.vision_projector;
        let flat_gguf = |value: &str| {
            !value.is_empty()
                && !value.contains(['/', '\\', '\r', '\n'])
                && Path::new(value)
                    .extension()
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("gguf"))
        };
        if !flat_gguf(primary)
            || !flat_gguf(projector)
            || primary.eq_ignore_ascii_case(projector)
            || primary.to_ascii_lowercase().contains("mmproj")
            || !projector.to_ascii_lowercase().contains("mmproj")
        {
            return Err(super::acquired::recovery_required(
                "Vision import requires distinct flat primary-model and mmproj GGUF roles",
            ));
        }
        ModelImporter::validate_acquired_payload_paths(&[primary, projector])
    }
}

impl ModelImporter {
    fn validate_acquired_vision_intent(
        acquired: &AcquiredArtifactUse,
        receipt: &AcquisitionConsumerReceipt,
        spec: &AcquiredGgufVisionSpec,
    ) -> Result<()> {
        spec.validate_paths()?;
        let record = acquired.record();
        if record.files.len() != 2
            || record.manifest.files().len() != 2
            || record.manifest.files().iter().any(|file| {
                file.expected_sha256().is_none()
                    || ![
                        spec.primary_model.path.as_str(),
                        spec.vision_projector.as_str(),
                    ]
                    .contains(&file.logical_path())
            })
            || !record
                .files
                .iter()
                .any(|file| file.path == spec.primary_model.path)
            || !record
                .files
                .iter()
                .any(|file| file.path == spec.vision_projector)
            || receipt.payload != serde_json::to_value(spec)?
        {
            return Err(super::acquired::recovery_required("Vision publication requires exactly the two digest-verified roles and their unchanged issued specification"));
        }
        Ok(())
    }

    /// Copy and publish the exact declared pair. Both held GGUF descriptors are
    /// structurally checked and copied bytes rehashed before publication. This
    /// does not establish pretrained inference or projector compatibility.
    pub async fn import_acquired_gguf_vision(
        &self,
        acquired: &AcquiredArtifactUse,
        receipt: &AcquisitionConsumerReceipt,
        spec: &AcquiredGgufVisionSpec,
    ) -> Result<ModelImportResult> {
        Self::validate_acquired_vision_intent(acquired, receipt, spec)?;
        if !matches!(
            acquired.record().phase,
            crate::acquisition::AcquisitionPhase::Using { .. }
        ) {
            return Err(super::acquired::recovery_required(
                "An adopted vision operation cannot be imported again",
            ));
        }
        acquired.require_issued_receipt(receipt).await?;
        let result = self
            .import_acquired_owned_with_vision(
                acquired,
                receipt,
                &spec.primary_model,
                Some(spec.clone()),
            )
            .await?;
        if result.success {
            Ok(result)
        } else {
            Err(PumasError::ImportFailed {
                message: result
                    .error
                    .unwrap_or_else(|| "Acquired vision import was refused".into()),
            })
        }
    }

    /// Read-only exact publication proof; never transfer or republish a pair.
    pub async fn reconcile_acquired_gguf_vision(
        &self,
        acquired: &AcquiredArtifactUse,
        receipt: &AcquisitionConsumerReceipt,
        spec: &AcquiredGgufVisionSpec,
        model_id: &str,
    ) -> Result<ModelImportResult> {
        Self::validate_acquired_vision_intent(acquired, receipt, spec)?;
        self.observe_acquired_output(acquired, receipt, &spec.primary_model, model_id)
            .await
    }
}
