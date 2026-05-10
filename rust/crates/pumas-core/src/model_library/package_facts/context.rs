use crate::error::Result;
use crate::index::ModelDependencyBindingRecord;
use crate::model_library::package_facts::manifest::PackageInspectionManifest;
use crate::model_library::types::ModelMetadata;
use crate::models::{ModelExecutionDescriptor, PumasModelRef, PUMAS_MODEL_REF_CONTRACT_VERSION};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub(crate) struct PackageInspectionContext {
    model_id: String,
    model_dir: PathBuf,
    descriptor: ModelExecutionDescriptor,
    metadata: ModelMetadata,
    dependency_bindings: Vec<ModelDependencyBindingRecord>,
    manifest: PackageInspectionManifest,
    selected_artifact_id: Option<String>,
    selected_artifact_path: Option<String>,
}

impl PackageInspectionContext {
    pub(crate) async fn build(
        model_id: String,
        model_dir: PathBuf,
        descriptor: ModelExecutionDescriptor,
        metadata: ModelMetadata,
        dependency_bindings: Vec<ModelDependencyBindingRecord>,
    ) -> Result<Self> {
        let manifest = PackageInspectionManifest::build(&model_dir, &metadata).await?;
        let selected_artifact_path = Some(descriptor.entry_path.clone());
        let selected_artifact_id = metadata.selected_artifact_id.clone().and_then(|value| {
            let trimmed = value.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        });

        Ok(Self {
            model_id,
            model_dir,
            descriptor,
            metadata,
            dependency_bindings,
            manifest,
            selected_artifact_id,
            selected_artifact_path,
        })
    }

    pub(crate) fn model_id(&self) -> &str {
        &self.model_id
    }

    pub(crate) fn model_dir(&self) -> &Path {
        &self.model_dir
    }

    pub(crate) fn descriptor(&self) -> &ModelExecutionDescriptor {
        &self.descriptor
    }

    pub(crate) fn metadata(&self) -> &ModelMetadata {
        &self.metadata
    }

    pub(crate) fn selected_artifact_id(&self) -> Option<&str> {
        self.selected_artifact_id.as_deref()
    }

    pub(crate) fn cache_selected_artifact_id(&self) -> &str {
        self.selected_artifact_id.as_deref().unwrap_or("")
    }

    pub(crate) fn selected_files(&self) -> &[String] {
        self.manifest.selected_files()
    }

    pub(crate) async fn source_fingerprint(&self) -> Result<String> {
        self.manifest
            .source_fingerprint(
                &self.model_dir,
                &self.descriptor,
                &self.metadata,
                &self.dependency_bindings,
            )
            .await
    }

    pub(crate) fn model_ref(&self) -> PumasModelRef {
        PumasModelRef {
            model_ref_contract_version: PUMAS_MODEL_REF_CONTRACT_VERSION,
            model_id: self.model_id.clone(),
            revision: None,
            selected_artifact_id: self.selected_artifact_id.clone(),
            selected_artifact_path: self.selected_artifact_path.clone(),
            migration_diagnostics: Vec::new(),
        }
    }
}
