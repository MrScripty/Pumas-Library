//! File system watcher for model library changes.
//!
//! Watches the model library directory for changes and triggers
//! index rebuilds when files are added, modified, or removed.

use crate::config::NetworkConfig;
use crate::error::Result;
use notify::{RecommendedWatcher, RecursiveMode};
use notify_debouncer_mini::{new_debouncer, Debouncer};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tracing::{debug, info, warn};

/// Model file extensions that should trigger a rebuild.
const MODEL_EXTENSIONS: &[&str] = &[
    "gguf",
    "safetensors",
    "pt",
    "pth",
    "ckpt",
    "bin",
    "onnx",
    "json", // metadata.json changes
];

/// Callback type for when model library changes are detected.
pub type ChangeCallback = Box<dyn Fn(Vec<PathBuf>) + Send + Sync + 'static>;

/// File system watcher for the model library.
///
/// Watches for file changes and triggers callbacks when model files
/// are added, modified, or removed.
pub struct ModelLibraryWatcher {
    /// The debounced file watcher
    _debouncer: Debouncer<RecommendedWatcher>,
    /// Channel to stop the watcher
    stop_tx: mpsc::Sender<()>,
}

impl ModelLibraryWatcher {
    /// Create a new model library watcher.
    ///
    /// # Arguments
    ///
    /// * `library_root` - Root directory of the model library to watch
    /// * `debounce_duration` - How long to wait after changes before triggering callback
    /// * `on_change` - Callback invoked with deduplicated changed paths
    pub fn new(
        library_root: impl AsRef<Path>,
        debounce_duration: Duration,
        on_change: ChangeCallback,
    ) -> Result<Self> {
        let library_root = library_root.as_ref().to_path_buf();
        let (stop_tx, mut stop_rx) = mpsc::channel::<()>(1);

        // Create a channel for debounced events
        let (event_tx, event_rx) = std::sync::mpsc::channel();

        // Create the debounced watcher
        let mut debouncer = new_debouncer(debounce_duration, event_tx).map_err(|e| {
            crate::error::PumasError::Other(format!("Failed to create file watcher: {}", e))
        })?;

        // Start watching the library root
        debouncer
            .watcher()
            .watch(&library_root, RecursiveMode::Recursive)
            .map_err(|e| {
                crate::error::PumasError::Other(format!("Failed to watch directory: {}", e))
            })?;

        info!("Started watching model library at {:?}", library_root);

        // Spawn a task to handle debounced events
        let on_change = Arc::new(on_change);
        let on_change_clone = Arc::clone(&on_change);

        std::thread::spawn(move || {
            loop {
                // Check for stop signal (non-blocking)
                match stop_rx.try_recv() {
                    Ok(()) | Err(tokio::sync::mpsc::error::TryRecvError::Disconnected) => {
                        debug!("File watcher stopping");
                        break;
                    }
                    Err(tokio::sync::mpsc::error::TryRecvError::Empty) => {}
                }

                // Check for file events with timeout
                match event_rx.recv_timeout(NetworkConfig::FILE_WATCHER_DEBOUNCE) {
                    Ok(result) => {
                        if let Ok(events) = result {
                            // Filter and coalesce relevant paths.
                            let mut relevant_paths: Vec<PathBuf> = events
                                .iter()
                                .filter_map(|event| {
                                    if is_relevant_change(&event.path) {
                                        Some(event.path.clone())
                                    } else {
                                        None
                                    }
                                })
                                .collect();

                            if !relevant_paths.is_empty() {
                                relevant_paths.sort();
                                relevant_paths.dedup();
                                debug!("Detected relevant model library changes");
                                on_change_clone(relevant_paths);
                            }
                        }
                    }
                    Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                        // No events, continue
                    }
                    Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                        warn!("File watcher channel disconnected");
                        break;
                    }
                }
            }
        });

        Ok(Self {
            _debouncer: debouncer,
            stop_tx,
        })
    }

    /// Stop the file watcher.
    pub async fn stop(&self) {
        let _ = self.stop_tx.send(()).await;
    }
}

/// Check if a path change is relevant (model file or metadata).
fn is_relevant_change(path: &Path) -> bool {
    // Check file extension
    if let Some(ext) = path.extension() {
        let ext_str = ext.to_string_lossy().to_lowercase();
        if MODEL_EXTENSIONS.contains(&ext_str.as_str()) {
            return true;
        }
    }

    // Also check for directory changes (model directory added/removed)
    if path.is_dir() {
        return true;
    }

    // Deleted model directories often arrive as non-existent paths without extension.
    if !path.exists() && path.extension().is_none() {
        return true;
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_relevant_change() {
        assert!(is_relevant_change(Path::new("/models/test.safetensors")));
        assert!(is_relevant_change(Path::new("/models/test.gguf")));
        assert!(is_relevant_change(Path::new("/models/metadata.json")));
        assert!(is_relevant_change(Path::new(
            "/models/llm/family/model_without_extension"
        )));
        assert!(!is_relevant_change(Path::new("/models/readme.md")));
        assert!(!is_relevant_change(Path::new("/models/test.txt")));
    }
}
