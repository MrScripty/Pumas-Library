//! Local selected-byte identity authored only inside copied-import publication.
//! No repository revision, publisher digest or native model qualification is inferred.
use crate::model_library::artifact_use::{
    validate_cohere_descriptors, OPTIONAL_MEMBERS, REQUIRED_MEMBERS,
};
use crate::model_library::DownloadRecoveryDestination;
use crate::models::{AssetValidationState, ModelFileInfo, ModelMetadata, StorageKind};
use crate::{PumasError, Result};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::path::Path;

pub(super) fn apply_metadata(
    stage: &DownloadRecoveryDestination,
    target: &Path,
    files: &[ModelFileInfo],
    metadata: &mut ModelMetadata,
) -> Result<()> {
    validate_cohere_descriptors(|name| stage.open_import_file(name).map_err(Into::into))?;
    let names: BTreeSet<_> = files.iter().map(|file| file.name.as_str()).collect();
    if names.len() != files.len()
        || !REQUIRED_MEMBERS.iter().all(|name| names.contains(name))
        || !names
            .iter()
            .all(|name| REQUIRED_MEMBERS.contains(name) || OPTIONAL_MEMBERS.contains(name))
    {
        return Err(invalid(
            "Local Cohere copied member set is incomplete or unsupported",
        ));
    }
    let mut digest = Sha256::new();
    digest.update(b"pumas-local-cohere-selection-v1\0");
    // The staged copy owner supplies observed size/content identities, never caller hashes.
    for name in &names {
        let file = files
            .iter()
            .find(|file| file.name == *name)
            .expect("member came from files");
        let size = file
            .size
            .ok_or_else(|| invalid("Copied member size is unavailable"))?;
        let hash = hex::decode(
            file.sha256
                .as_deref()
                .ok_or_else(|| invalid("Copied member hash is unavailable"))?,
        )
        .map_err(|_| invalid("Copied member hash is invalid"))?;
        if hash.len() != 32 {
            return Err(invalid("Copied member hash is invalid"));
        }
        digest.update((name.len() as u64).to_le_bytes());
        digest.update(name.as_bytes());
        digest.update(size.to_le_bytes());
        digest.update(hash);
    }
    let selected: Vec<_> = names.into_iter().map(str::to_owned).collect();
    metadata.selected_artifact_id = Some(format!(
        "local-cohere-sha256:{}",
        hex::encode(digest.finalize())
    ));
    metadata.selected_artifact_files = Some(selected.clone());
    metadata.expected_files = Some(selected);
    metadata.entry_path = Some(target.display().to_string());
    metadata.storage_kind = Some(StorageKind::LibraryOwned);
    // Validates the closed package/descriptor structure, not the tensor payload or inference.
    metadata.validation_state = Some(AssetValidationState::Valid);
    metadata.config_model_type = Some("cohere_asr".into());
    metadata.architecture_family = Some("cohere_asr".into());
    metadata.task_type_primary = Some("speech-to-text".into());
    metadata.input_modalities = Some(vec!["audio".into()]);
    metadata.output_modalities = Some(vec!["text".into()]);
    metadata.task_classification_source = Some("explicit-local-cohere-import".into());
    metadata.task_classification_confidence = Some(0.0);
    metadata.metadata_needs_review = Some(true);
    metadata.review_reasons = Some(vec!["local-model-native-qualification-required".into()]);
    metadata.pending_online_lookup = Some(false);
    metadata.repo_id = None;
    metadata.upstream_revision = None;
    metadata.huggingface_evidence = None;
    metadata.pipeline_tag = None;
    Ok(())
}

fn invalid(message: &str) -> PumasError {
    PumasError::Validation {
        field: "import.local_cohere".into(),
        message: message.into(),
    }
}
