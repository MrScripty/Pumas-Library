//! Package-facts extraction helpers used by the `ModelLibrary` facade.
//!
//! This module owns bounded package inspection and projection. Public Pumas APIs
//! continue to live on `ModelLibrary`; helpers here stay crate-private so the
//! extraction contract can evolve with the versioned DTOs.

pub(crate) mod artifact;
pub(crate) mod context;
pub(crate) mod diffusers;
pub(crate) mod generation;
pub(crate) mod gguf;
pub(crate) mod manifest;
pub(crate) mod summary;
pub(crate) mod transformers;

pub(crate) use artifact::{
    companion_artifacts, package_artifact_kind, package_class_references, package_component_facts,
};
pub(crate) use context::PackageInspectionContext;
pub(crate) use diffusers::diffusers_package_evidence;
pub(crate) use generation::generation_default_facts;
pub(crate) use gguf::{gguf_package_evidence, invalid_gguf_package_evidence};
pub(crate) use summary::package_facts_summary;
pub(crate) use transformers::{
    auto_map_sources_from_config, backend_hint_facts, custom_generate_dependency_manifests,
    custom_generate_sources, merge_string_lists, transformers_package_evidence,
};
