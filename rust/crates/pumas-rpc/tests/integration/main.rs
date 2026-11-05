//! pumas-rpc integration tests, built as one test executable.
//!
//! Each file was its own executable that linked the whole backend; one
//! executable links it once.

mod hf_cached_search_tests;
mod integration_tests;
mod intent_integration_tests;
mod local_enumeration_cli;
mod s3_conditional_bundle_process;
mod s3_conditional_process;
