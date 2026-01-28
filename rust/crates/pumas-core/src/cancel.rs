//! Unified cancellation token for async operations.
//!
//! This module provides a reusable `CancellationToken` that can be shared across
//! async tasks for cooperative cancellation. It replaces the 6+ duplicated
//! `Arc<AtomicBool>` patterns throughout the codebase.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// A cancellation token for cooperative cancellation of async operations.
///
/// This token can be cloned and shared across tasks. When `cancel()` is called
/// on any clone, all clones will observe the cancellation.
///
/// # Example
///
/// ```
/// use pumas_core::cancel::CancellationToken;
///
/// let token = CancellationToken::new();
/// let token_clone = token.clone();
///
/// // In async task
/// // while !token.is_cancelled() {
/// //     // do work
/// // }
///
/// // Cancel from another task
/// token_clone.cancel();
/// assert!(token.is_cancelled());
/// ```
#[derive(Debug, Clone, Default)]
pub struct CancellationToken {
    cancelled: Arc<AtomicBool>,
}

impl CancellationToken {
    /// Create a new cancellation token.
    pub fn new() -> Self {
        Self {
            cancelled: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Request cancellation.
    ///
    /// All clones of this token will observe the cancellation.
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
    }

    /// Check if cancellation has been requested.
    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
    }

    /// Reset the token for reuse.
    ///
    /// This clears the cancellation state, allowing the token to be reused.
    /// Use with caution - ensure no tasks are still checking this token.
    pub fn reset(&self) {
        self.cancelled.store(false, Ordering::SeqCst);
    }

    /// Create a child token that shares cancellation state with this token.
    ///
    /// Cancelling either the parent or child will cancel both.
    pub fn child_token(&self) -> Self {
        Self {
            cancelled: self.cancelled.clone(),
        }
    }

    /// Check cancellation and return an error if cancelled.
    ///
    /// This is a convenience method for use in loops or async operations
    /// that need to check cancellation and return early.
    pub fn check(&self) -> Result<(), CancelledError> {
        if self.is_cancelled() {
            Err(CancelledError)
        } else {
            Ok(())
        }
    }
}

/// Error returned when an operation is cancelled.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CancelledError;

impl std::fmt::Display for CancelledError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Operation was cancelled")
    }
}

impl std::error::Error for CancelledError {}

/// Extension trait for converting CancelledError to PumasError.
impl From<CancelledError> for crate::error::PumasError {
    fn from(_: CancelledError) -> Self {
        crate::error::PumasError::DownloadCancelled
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_token_not_cancelled() {
        let token = CancellationToken::new();
        assert!(!token.is_cancelled());
    }

    #[test]
    fn test_cancel() {
        let token = CancellationToken::new();
        token.cancel();
        assert!(token.is_cancelled());
    }

    #[test]
    fn test_reset() {
        let token = CancellationToken::new();
        token.cancel();
        assert!(token.is_cancelled());

        token.reset();
        assert!(!token.is_cancelled());
    }

    #[test]
    fn test_clone_shares_state() {
        let token1 = CancellationToken::new();
        let token2 = token1.clone();

        assert!(!token1.is_cancelled());
        assert!(!token2.is_cancelled());

        token1.cancel();

        assert!(token1.is_cancelled());
        assert!(token2.is_cancelled());
    }

    #[test]
    fn test_child_token() {
        let parent = CancellationToken::new();
        let child = parent.child_token();

        child.cancel();

        assert!(parent.is_cancelled());
        assert!(child.is_cancelled());
    }

    #[test]
    fn test_check_not_cancelled() {
        let token = CancellationToken::new();
        assert!(token.check().is_ok());
    }

    #[test]
    fn test_check_cancelled() {
        let token = CancellationToken::new();
        token.cancel();
        assert!(token.check().is_err());
    }

    #[test]
    fn test_default() {
        let token = CancellationToken::default();
        assert!(!token.is_cancelled());
    }

    #[test]
    fn test_cancelled_error_display() {
        let err = CancelledError;
        assert_eq!(err.to_string(), "Operation was cancelled");
    }
}
