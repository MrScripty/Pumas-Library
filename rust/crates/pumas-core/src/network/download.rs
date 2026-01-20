//! Download manager with progress tracking and cancellation support.
//!
//! Provides:
//! - Download with progress callbacks
//! - Cancellation support
//! - Retry logic for transient failures
//! - Atomic file operations (temp file → final)

use crate::config::NetworkConfig;
use crate::network::client::HttpClient;
use crate::network::retry::{retry_async, RetryConfig};
use crate::{PumasError, Result};
use futures::StreamExt;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use tracing::{debug, info};

/// Progress information for a download.
#[derive(Debug, Clone)]
pub struct DownloadProgress {
    /// Bytes downloaded so far.
    pub bytes_downloaded: u64,
    /// Total bytes (if known).
    pub total_bytes: Option<u64>,
    /// Download speed in bytes per second.
    pub speed_bytes_per_sec: f64,
    /// Percentage complete (0-100).
    pub percent: Option<f64>,
    /// Estimated time remaining in seconds.
    pub eta_seconds: Option<f64>,
}

impl DownloadProgress {
    fn new(bytes_downloaded: u64, total_bytes: Option<u64>, speed: f64) -> Self {
        let percent = total_bytes.map(|total| {
            if total > 0 {
                (bytes_downloaded as f64 / total as f64) * 100.0
            } else {
                0.0
            }
        });

        let eta_seconds = total_bytes.and_then(|total| {
            if speed > 0.0 && bytes_downloaded < total {
                Some((total - bytes_downloaded) as f64 / speed)
            } else {
                None
            }
        });

        Self {
            bytes_downloaded,
            total_bytes,
            speed_bytes_per_sec: speed,
            percent,
            eta_seconds,
        }
    }
}

/// Download manager for file downloads.
pub struct DownloadManager {
    http: Arc<HttpClient>,
    /// Whether cancellation has been requested.
    cancelled: AtomicBool,
    /// Last error encountered.
    last_error: std::sync::RwLock<Option<String>>,
    /// Whether the last error was retryable.
    last_error_retryable: AtomicBool,
    /// Progress update interval.
    progress_interval: Duration,
    /// Chunk size for reading.
    chunk_size: usize,
    /// Temp file suffix.
    temp_suffix: String,
}

impl DownloadManager {
    /// Create a new download manager.
    pub fn new() -> Result<Self> {
        let http = HttpClient::new()?;
        Ok(Self {
            http: Arc::new(http),
            cancelled: AtomicBool::new(false),
            last_error: std::sync::RwLock::new(None),
            last_error_retryable: AtomicBool::new(false),
            progress_interval: NetworkConfig::DOWNLOAD_PROGRESS_INTERVAL,
            chunk_size: NetworkConfig::DOWNLOAD_CHUNK_SIZE,
            temp_suffix: NetworkConfig::DOWNLOAD_TEMP_SUFFIX.to_string(),
        })
    }

    /// Create a download manager with custom HTTP client.
    pub fn with_client(http: Arc<HttpClient>) -> Self {
        Self {
            http,
            cancelled: AtomicBool::new(false),
            last_error: std::sync::RwLock::new(None),
            last_error_retryable: AtomicBool::new(false),
            progress_interval: NetworkConfig::DOWNLOAD_PROGRESS_INTERVAL,
            chunk_size: NetworkConfig::DOWNLOAD_CHUNK_SIZE,
            temp_suffix: NetworkConfig::DOWNLOAD_TEMP_SUFFIX.to_string(),
        }
    }

    /// Request cancellation of the current download.
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
    }

    /// Reset cancellation flag.
    pub fn reset_cancel(&self) {
        self.cancelled.store(false, Ordering::SeqCst);
    }

    /// Check if cancellation was requested.
    pub fn was_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
    }

    /// Get the last error message.
    pub fn last_error(&self) -> Option<String> {
        self.last_error.read().unwrap().clone()
    }

    /// Check if the last error was retryable.
    pub fn was_last_error_retryable(&self) -> bool {
        self.last_error_retryable.load(Ordering::SeqCst)
    }

    /// Download a file with progress reporting.
    ///
    /// # Arguments
    ///
    /// * `url` - URL to download from
    /// * `destination` - Path to save the file
    /// * `progress_tx` - Optional channel for progress updates
    ///
    /// # Returns
    ///
    /// Total bytes downloaded on success
    pub async fn download(
        &self,
        url: &str,
        destination: &Path,
        progress_tx: Option<mpsc::Sender<DownloadProgress>>,
    ) -> Result<u64> {
        self.reset_cancel();
        self.clear_error();

        // Ensure parent directory exists
        if let Some(parent) = destination.parent() {
            if !parent.exists() {
                std::fs::create_dir_all(parent).map_err(|e| PumasError::Io {
                    message: format!("Failed to create directory: {}", e),
                    path: Some(parent.to_path_buf()),
                    source: Some(e),
                })?;
            }
        }

        let temp_path = PathBuf::from(format!(
            "{}{}",
            destination.display(),
            self.temp_suffix
        ));

        // Perform download to temp file
        let result = self
            .do_download(url, &temp_path, progress_tx.clone())
            .await;

        match result {
            Ok(bytes) => {
                // Atomic move from temp to final destination
                std::fs::rename(&temp_path, destination).map_err(|e| {
                    let _ = std::fs::remove_file(&temp_path);
                    PumasError::Io {
                        message: format!("Failed to move download to final destination: {}", e),
                        path: Some(destination.to_path_buf()),
                        source: Some(e),
                    }
                })?;

                info!("Downloaded {} bytes to {}", bytes, destination.display());
                Ok(bytes)
            }
            Err(e) => {
                // Cleanup temp file on error
                let _ = std::fs::remove_file(&temp_path);
                Err(e)
            }
        }
    }

    /// Download a file with retry logic.
    pub async fn download_with_retry(
        &self,
        url: &str,
        destination: &Path,
        max_retries: u32,
        progress_tx: Option<mpsc::Sender<DownloadProgress>>,
    ) -> Result<u64> {
        let retry_config = RetryConfig::new()
            .with_max_attempts(max_retries)
            .with_base_delay(Duration::from_secs(2));

        let (result, stats) = retry_async(
            &retry_config,
            || self.download(url, destination, progress_tx.clone()),
            |e| e.is_retryable() && !self.was_cancelled(),
        )
        .await;

        if stats.attempts > 1 {
            debug!(
                "Download succeeded after {} attempts (total delay: {:?})",
                stats.attempts, stats.total_delay
            );
        }

        result
    }

    // Internal methods

    fn clear_error(&self) {
        *self.last_error.write().unwrap() = None;
        self.last_error_retryable.store(false, Ordering::SeqCst);
    }

    fn set_error(&self, message: String, retryable: bool) {
        *self.last_error.write().unwrap() = Some(message);
        self.last_error_retryable.store(retryable, Ordering::SeqCst);
    }

    async fn do_download(
        &self,
        url: &str,
        temp_path: &Path,
        progress_tx: Option<mpsc::Sender<DownloadProgress>>,
    ) -> Result<u64> {
        let response = self.http.get(url).await?;
        let status = response.status();

        if !status.is_success() {
            let retryable = HttpClient::is_retryable_status(status);
            let message = format!("Download failed with status {}", status);
            self.set_error(message.clone(), retryable);

            return Err(PumasError::DownloadFailed {
                url: url.to_string(),
                message,
            });
        }

        let total_bytes = response.content_length();
        let mut file = std::fs::File::create(temp_path).map_err(|e| PumasError::Io {
            message: format!("Failed to create temp file: {}", e),
            path: Some(temp_path.to_path_buf()),
            source: Some(e),
        })?;

        let mut bytes_downloaded: u64 = 0;
        let mut last_progress_update = Instant::now();
        let mut speed_tracker = SpeedTracker::new();
        let mut stream = response.bytes_stream();

        // Send initial progress
        if let Some(ref tx) = progress_tx {
            let _ = tx
                .send(DownloadProgress::new(0, total_bytes, 0.0))
                .await;
        }

        while let Some(chunk_result) = stream.next().await {
            // Check for cancellation
            if self.cancelled.load(Ordering::SeqCst) {
                return Err(PumasError::DownloadCancelled);
            }

            let chunk = chunk_result.map_err(|e| {
                let message = format!("Error reading download stream: {}", e);
                self.set_error(message.clone(), true);
                PumasError::Network {
                    message,
                    source: Some(e),
                }
            })?;

            file.write_all(&chunk).map_err(|e| PumasError::Io {
                message: format!("Failed to write to temp file: {}", e),
                path: Some(temp_path.to_path_buf()),
                source: Some(e),
            })?;

            bytes_downloaded += chunk.len() as u64;
            speed_tracker.record(chunk.len() as u64);

            // Send progress updates at intervals
            if last_progress_update.elapsed() >= self.progress_interval {
                if let Some(ref tx) = progress_tx {
                    let speed = speed_tracker.speed();
                    let progress = DownloadProgress::new(bytes_downloaded, total_bytes, speed);
                    let _ = tx.send(progress).await;
                }
                last_progress_update = Instant::now();
            }
        }

        // Ensure data is flushed to disk
        file.flush().map_err(|e| PumasError::Io {
            message: format!("Failed to flush temp file: {}", e),
            path: Some(temp_path.to_path_buf()),
            source: Some(e),
        })?;

        // Send final progress
        if let Some(ref tx) = progress_tx {
            let speed = speed_tracker.speed();
            let progress = DownloadProgress::new(bytes_downloaded, total_bytes, speed);
            let _ = tx.send(progress).await;
        }

        Ok(bytes_downloaded)
    }
}

impl Default for DownloadManager {
    fn default() -> Self {
        Self::new().expect("Failed to create default DownloadManager")
    }
}

/// Simple speed tracker for download progress.
struct SpeedTracker {
    start_time: Instant,
    total_bytes: u64,
    window_start: Instant,
    window_bytes: u64,
}

impl SpeedTracker {
    fn new() -> Self {
        let now = Instant::now();
        Self {
            start_time: now,
            total_bytes: 0,
            window_start: now,
            window_bytes: 0,
        }
    }

    fn record(&mut self, bytes: u64) {
        self.total_bytes += bytes;
        self.window_bytes += bytes;

        // Reset window every second
        if self.window_start.elapsed() >= Duration::from_secs(1) {
            self.window_start = Instant::now();
            self.window_bytes = 0;
        }
    }

    fn speed(&self) -> f64 {
        let elapsed = self.start_time.elapsed().as_secs_f64();
        if elapsed > 0.0 {
            self.total_bytes as f64 / elapsed
        } else {
            0.0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_download_progress() {
        let progress = DownloadProgress::new(50, Some(100), 10.0);
        assert_eq!(progress.bytes_downloaded, 50);
        assert_eq!(progress.total_bytes, Some(100));
        assert_eq!(progress.percent, Some(50.0));
        assert_eq!(progress.eta_seconds, Some(5.0)); // 50 remaining / 10 speed
    }

    #[test]
    fn test_download_progress_unknown_total() {
        let progress = DownloadProgress::new(50, None, 10.0);
        assert_eq!(progress.bytes_downloaded, 50);
        assert_eq!(progress.total_bytes, None);
        assert_eq!(progress.percent, None);
        assert_eq!(progress.eta_seconds, None);
    }

    #[test]
    fn test_speed_tracker() {
        let mut tracker = SpeedTracker::new();
        tracker.record(1000);
        tracker.record(1000);

        // Speed should be > 0
        assert!(tracker.speed() > 0.0);
    }

    #[tokio::test]
    async fn test_download_manager_creation() {
        let manager = DownloadManager::new().unwrap();
        assert!(!manager.was_cancelled());
        assert!(manager.last_error().is_none());
    }

    #[tokio::test]
    async fn test_download_manager_cancel() {
        let manager = DownloadManager::new().unwrap();
        assert!(!manager.was_cancelled());

        manager.cancel();
        assert!(manager.was_cancelled());

        manager.reset_cancel();
        assert!(!manager.was_cancelled());
    }
}
