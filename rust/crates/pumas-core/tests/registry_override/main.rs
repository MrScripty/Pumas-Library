//! pumas-library tests that point the process-wide registry location at a
//! per-test database.
//!
//! `PUMAS_REGISTRY_DB_PATH` is read whenever the registry opens, so these
//! tests run in their own executable: a test elsewhere must never open a
//! registry another test chose. Within this executable every override is
//! serialized by `support::RegistryOverride`.

mod support;

mod api_tests;
mod intent_api_tests;
mod intent_desired_state_tests;
