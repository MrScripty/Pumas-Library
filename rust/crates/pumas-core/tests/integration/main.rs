//! pumas-library integration tests, built as one test executable.
//!
//! Each file was its own executable that linked the whole library; one
//! executable links it once. Tests that re-run this executable as a child
//! name themselves with `support::exact_test_name`. Tests that change
//! process-wide registry configuration live in `registry_override`.

mod support;

mod acquired_gguf_vision;
mod acquired_model_bridge;
mod acquired_onnx_package;
mod artifact_acquisition;
mod artifact_load_target_contract_fixtures;
mod catalog_owner_recovery;
mod copied_publication_freshness;
mod dependency_requirements_contract_fixtures;
mod intent_desired_crash_tests;
mod intent_ipc_tests;
mod link_health;
mod local_cohere_import;
mod local_owner_retention;
mod local_start_authority;
mod metadata_artifact_tests;
mod model_ref_contract_fixtures;
mod package_facts_contract_fixtures;
mod package_facts_resolution;
mod pending_reservation_recovery;
mod physical_mapper_lifetime;
mod physical_owner_lifetime;
mod runtime_profile_contract_fixtures;
mod s3_acquisition;
mod s3_conditional_bundle_workflow;
mod s3_conditional_model_workflow;
mod s3_model_bridge;
mod s3_model_workflow;
mod s3_reader;
mod s3_sharded_cohort;
