//! Helpers shared by the integration test modules.

#[cfg(feature = "s3")]
pub mod store_refusal;

/// The libtest name of `name` in the module that called this, for running
/// one test of this executable in a child process with `--exact`.
///
/// Pass `module_path!()`. It starts with the test target's crate name, which
/// libtest names leave out.
pub fn exact_test_name(module_path: &str, name: &str) -> String {
    let module = module_path
        .split_once("::")
        .map(|(_, module)| module)
        .expect("a test module path below the crate root");
    format!("{module}::{name}")
}
