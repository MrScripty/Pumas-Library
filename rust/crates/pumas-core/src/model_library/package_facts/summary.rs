use crate::models::{
    PackageFactStatus, ProcessorComponentFacts, ProcessorComponentKind, ResolvedModelPackageFacts,
    ResolvedModelPackageFactsSummary,
};

pub(crate) fn package_facts_summary(
    facts: &ResolvedModelPackageFacts,
) -> ResolvedModelPackageFactsSummary {
    ResolvedModelPackageFactsSummary::from(facts)
}

impl From<&ResolvedModelPackageFacts> for ResolvedModelPackageFactsSummary {
    fn from(facts: &ResolvedModelPackageFacts) -> Self {
        Self {
            package_facts_contract_version: facts.package_facts_contract_version,
            model_ref: facts.model_ref.clone(),
            artifact_kind: facts.artifact.artifact_kind,
            entry_path: facts.artifact.entry_path.clone(),
            storage_kind: facts.artifact.storage_kind,
            validation_state: facts.artifact.validation_state,
            task: facts.task.clone(),
            backend_hints: facts.backend_hints.clone(),
            requires_custom_code: facts.custom_code.requires_custom_code,
            config_status: facts
                .transformers
                .as_ref()
                .map(|evidence| evidence.config_status)
                .unwrap_or(PackageFactStatus::Uninspected),
            tokenizer_status: component_status(
                &facts.components,
                &[
                    ProcessorComponentKind::Tokenizer,
                    ProcessorComponentKind::TokenizerConfig,
                    ProcessorComponentKind::SpecialTokensMap,
                ],
            ),
            processor_status: component_status(
                &facts.components,
                &[
                    ProcessorComponentKind::Processor,
                    ProcessorComponentKind::Preprocessor,
                    ProcessorComponentKind::ImageProcessor,
                    ProcessorComponentKind::VideoProcessor,
                    ProcessorComponentKind::AudioFeatureExtractor,
                    ProcessorComponentKind::FeatureExtractor,
                ],
            ),
            generation_config_status: facts
                .transformers
                .as_ref()
                .map(|evidence| evidence.generation_config_status)
                .unwrap_or(PackageFactStatus::Uninspected),
            generation_defaults_status: facts.generation_defaults.status,
            image_generation_family_evidence: facts
                .diffusers
                .as_ref()
                .map(|evidence| evidence.family_evidence.clone())
                .unwrap_or_default(),
            diffusers_pipeline_class: facts
                .diffusers
                .as_ref()
                .and_then(|evidence| evidence.pipeline_class.clone()),
            gguf_architecture: facts
                .gguf
                .as_ref()
                .and_then(|evidence| evidence.architecture.clone()),
            diagnostic_codes: facts
                .diagnostics
                .iter()
                .chain(facts.generation_defaults.diagnostics.iter())
                .map(|diagnostic| diagnostic.code.clone())
                .collect(),
        }
    }
}

fn component_status(
    components: &[ProcessorComponentFacts],
    kinds: &[ProcessorComponentKind],
) -> PackageFactStatus {
    components
        .iter()
        .filter(|component| kinds.contains(&component.kind))
        .map(|component| component.status)
        .find(|status| *status != PackageFactStatus::Uninspected)
        .unwrap_or(PackageFactStatus::Uninspected)
}
