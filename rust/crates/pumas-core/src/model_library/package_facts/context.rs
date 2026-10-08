use crate::error::Result;
use crate::index::ModelDependencyBindingRecord;
use crate::model_library::package_facts::manifest::PackageInspectionManifest;
use crate::model_library::types::ModelMetadata;
use crate::models::{
    ModelExecutionDescriptor, PackageInspectionManifest as ContractPackageInspectionManifest,
    PumasModelRef, PUMAS_MODEL_REF_CONTRACT_VERSION,
};
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
    artifact_kind: crate::models::PackageArtifactKind,
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
        let artifact_kind = super::artifact::package_artifact_kind(
            &model_dir,
            &metadata,
            manifest.selected_files(),
        )
        .await?;
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
            artifact_kind,
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

    pub(crate) fn manifest(&self) -> &PackageInspectionManifest {
        &self.manifest
    }

    pub(crate) fn inspection_manifest(&self) -> ContractPackageInspectionManifest {
        self.manifest.to_contract()
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
            revision: self.metadata.upstream_revision.clone(),
            selected_artifact_id: self.selected_artifact_id.clone(),
            selected_artifact_path: self.selected_artifact_path.clone(),
            migration_diagnostics: Vec::new(),
        }
    }

    pub(crate) fn facts_are_coherent(
        &self,
        facts: &crate::models::ResolvedModelPackageFacts,
    ) -> bool {
        facts.package_facts_contract_version == crate::models::PACKAGE_FACTS_CONTRACT_VERSION
            && facts.model_ref == self.model_ref()
            && facts.artifact.artifact_kind == self.artifact_kind
            && facts.artifact.entry_path == self.descriptor.entry_path
            && facts.artifact.storage_kind == self.descriptor.storage_kind
            && facts.artifact.validation_state == self.descriptor.validation_state
            && facts.artifact.selected_files == self.selected_files()
            && facts.inspection_manifest.as_ref() == Some(&self.inspection_manifest())
            && (self.artifact_kind != crate::models::PackageArtifactKind::HfCompatibleDirectory
                || facts.transformers.is_some())
            && facts.transformers.as_ref().is_none_or(|evidence| {
                evidence.source_revision == self.metadata.upstream_revision
                    && evidence.source_repo_id
                        == self.metadata.repo_id.clone().or_else(|| {
                            self.metadata
                                .huggingface_evidence
                                .as_ref()
                                .and_then(|evidence| evidence.repo_id.clone())
                        })
            })
    }

    pub(crate) fn summary_is_coherent(
        &self,
        summary: &crate::models::ResolvedModelPackageFactsSummary,
    ) -> bool {
        summary.package_facts_contract_version == crate::models::PACKAGE_FACTS_CONTRACT_VERSION
            && summary.model_ref == self.model_ref()
            && summary.artifact_kind == self.artifact_kind
            && summary.entry_path == self.descriptor.entry_path
            && summary.storage_kind == self.descriptor.storage_kind
            && summary.validation_state == self.descriptor.validation_state
    }
}
