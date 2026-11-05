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

/// Opens a physical store that this process has just released.
///
/// Other tests in this executable spawn child processes. A child forked while
/// this test still held the store's lock descriptor keeps an inherited copy,
/// and so the lock, until it execs. Only that refusal is retried, for a bounded
/// time; any other error, or a lock still held at the deadline, is returned.
/// This never changes production acquisition or recovery authority.
// Compiled only where a caller is.
#[cfg(any(
    target_os = "linux",
    feature = "s3",
    all(target_os = "macos", feature = "test-support")
))]
pub async fn after_fork_release<T, F, Fut>(mut acquire: F) -> pumas_library::Result<T>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = pumas_library::Result<T>>,
{
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
        match acquire().await {
            Err(pumas_library::PumasError::InvalidParams { message })
                if message.starts_with(
                    "Pumas library instance is already running for physical store",
                ) && tokio::time::Instant::now() < deadline =>
            {
                tokio::time::sleep(std::time::Duration::from_millis(5)).await;
            }
            result => return result,
        }
    }
}
