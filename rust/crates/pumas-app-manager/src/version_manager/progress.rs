//! Installation progress tracking.
//!
//! Tracks installation progress with stages, package weights,
//! and persistence for recovery.

use pumas_library::models::{InstallationProgress, InstallationProgressItem, InstallationStage};
use pumas_library::network::{
    ActivityUpdate, CopyableSource, MeasurementBasis, MeasurementCoverage, NetworkActivityRegistry,
    OperationId, TransferDirection, TransferId, TransferState,
};
use pumas_library::{PumasError, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Instant;
use tokio::fs;
use tracing::{debug, warn};

/// Progress update sent through channels.
#[derive(Debug, Clone)]
pub enum ProgressUpdate {
    /// Stage changed.
    StageChanged {
        stage: InstallationStage,
        message: String,
    },
    /// Download progress.
    Download {
        downloaded_bytes: u64,
        total_bytes: Option<u64>,
        speed_bytes_per_sec: Option<f64>,
    },
    /// Extraction progress.
    Extract { progress_percent: f32 },
    /// Virtual environment creation.
    Venv { message: String },
    /// Dependency installation progress.
    Dependency {
        package: String,
        completed_count: u32,
        total_count: Option<u32>,
        package_size: Option<u64>,
    },
    /// Setup progress.
    Setup { message: String },
    /// Overall progress update.
    Overall {
        progress_percent: f32,
        message: String,
    },
    /// Error occurred.
    Error { message: String },
    /// Installation completed.
    Completed { success: bool },
}

/// Package weight configuration for progress calculation.
pub struct PackageWeights;

impl PackageWeights {
    /// Default weight for unknown packages.
    pub const DEFAULT_WEIGHT: u32 = 1;

    /// Known package weights based on download size/complexity.
    pub fn get_weight(package_name: &str) -> u32 {
        match package_name.to_lowercase().as_str() {
            // Heavy packages
            "torch" | "pytorch" => 15,
            "torchvision" => 5,
            "tensorflow" | "tensorflow-gpu" => 12,
            "jax" | "jaxlib" => 8,
            "triton" => 6,

            // Medium packages
            "opencv-python" | "opencv-python-headless" => 4,
            "transformers" => 3,
            "diffusers" => 3,
            "accelerate" => 2,
            "safetensors" => 2,
            "onnxruntime" | "onnxruntime-gpu" => 4,
            "xformers" => 5,
            "numpy" => 2,
            "scipy" => 3,
            "pandas" => 2,
            "pillow" => 2,

            // Light packages
            "tqdm" | "requests" | "aiohttp" | "httpx" => 1,
            _ => Self::DEFAULT_WEIGHT,
        }
    }

    /// Calculate total weight for a list of packages.
    pub fn total_weight(packages: &[String]) -> u32 {
        packages.iter().map(|p| Self::get_weight(p)).sum()
    }
}

/// Installation progress tracker with persistence.
pub struct InstallationProgressTracker {
    /// Cache directory for state persistence.
    cache_dir: PathBuf,
    /// Current state file name.
    state_filename: String,
    /// Current installation state.
    state: Mutex<Option<InstallationProgressState>>,
    /// Transfer observations shared by the library's network producers.
    network_activity: NetworkActivityRegistry,
}

/// Internal state for tracking.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct InstallationProgressState {
    tag: String,
    started_at: String,
    stage: InstallationStage,
    stage_progress: f32,
    overall_progress: f32,
    current_item: Option<String>,
    #[serde(default)]
    download_source_url: Option<String>,
    #[serde(default)]
    download_active: bool,
    #[serde(default)]
    download_measurement_available: Option<bool>,
    download_speed: Option<f64>,
    eta_seconds: Option<f64>,
    total_size: Option<u64>,
    downloaded_bytes: u64,
    dependency_count: Option<u32>,
    completed_dependencies: u32,
    completed_items: Vec<InstallationProgressItem>,
    error: Option<String>,
    completed_at: Option<String>,
    success: Option<bool>,
    log_path: Option<String>,
    total_weight: u32,
    completed_weight: u32,
    pid: Option<u32>,
}

impl InstallationProgressTracker {
    /// Create a new progress tracker.
    pub fn new(cache_dir: PathBuf) -> Self {
        Self {
            cache_dir,
            state_filename: "installation-state.json".to_string(),
            state: Mutex::new(None),
            network_activity: NetworkActivityRegistry::new(),
        }
    }

    /// Create a new progress tracker and clear stale completed state asynchronously.
    pub async fn new_with_stale_cleanup(cache_dir: PathBuf) -> Self {
        let tracker = Self::new(cache_dir);
        tracker.clear_stale_state_file_async().await;
        tracker
    }

    /// Clear the state file if it contains a completed installation.
    /// This prevents stale "completed" states from persisting across restarts.
    async fn clear_stale_state_file_async(&self) {
        let state_path = self.cache_dir.join(&self.state_filename);
        let exists: bool = fs::try_exists(&state_path).await.unwrap_or_default();
        if !exists {
            return;
        }

        match fs::read_to_string(&state_path).await {
            Ok(json) => {
                if let Ok(state) = serde_json::from_str::<InstallationProgressState>(&json) {
                    if state.completed_at.is_some() {
                        debug!(
                            "Clearing stale completed installation state for {} from previous session",
                            state.tag
                        );
                        let _ = fs::remove_file(&state_path).await;
                    }
                }
            }
            Err(_) => {
                // If we can't read the file, remove it to be safe
                let _ = fs::remove_file(&state_path).await;
            }
        }
    }

    /// Start tracking a new installation.
    pub fn start_installation(
        &mut self,
        tag: &str,
        total_size: Option<u64>,
        dependency_count: Option<u32>,
        log_path: Option<&str>,
    ) {
        self.clear_network_activity();
        let state = InstallationProgressState {
            tag: tag.to_string(),
            started_at: chrono::Utc::now().to_rfc3339(),
            stage: InstallationStage::Download,
            stage_progress: 0.0,
            overall_progress: 0.0,
            current_item: None,
            download_source_url: None,
            download_active: false,
            download_measurement_available: None,
            download_speed: None,
            eta_seconds: None,
            total_size,
            downloaded_bytes: 0,
            dependency_count,
            completed_dependencies: 0,
            completed_items: Vec::new(),
            error: None,
            completed_at: None,
            success: None,
            log_path: log_path.map(String::from),
            total_weight: 0,
            completed_weight: 0,
            pid: None,
        };

        *self.state.lock().unwrap() = Some(state);
        let _ = self.persist_state();
        debug!("Started tracking installation for {}", tag);
    }

    /// Update the current stage.
    pub fn update_stage(
        &mut self,
        stage: InstallationStage,
        progress: f32,
        current_item: Option<&str>,
    ) {
        self.clear_network_activity();
        let mut guard = self.state.lock().unwrap();
        if let Some(ref mut state) = *guard {
            state.stage = stage;
            state.stage_progress = progress.clamp(0.0, 100.0);
            state.current_item = current_item.map(String::from);
            state.download_source_url = None;
            state.download_active = false;
            state.download_measurement_available = None;
            state.download_speed = None;
            state.overall_progress = self.calculate_overall_progress(state);
        }
        drop(guard);
        let _ = self.persist_state();
    }

    /// Update download progress.
    pub fn update_download_progress(
        &mut self,
        downloaded_bytes: u64,
        total_bytes: Option<u64>,
        speed_bytes_per_sec: Option<f64>,
    ) {
        let active = total_bytes.is_none_or(|total| downloaded_bytes < total);
        let measured_speed = self
            .record_network_activity(
                "installer-download",
                None,
                active,
                downloaded_bytes,
                total_bytes,
            )
            .or(speed_bytes_per_sec);
        let mut guard = self.state.lock().unwrap();
        if let Some(ref mut state) = *guard {
            state.downloaded_bytes = downloaded_bytes;
            state.download_source_url = None;
            state.download_active = active;
            state.download_measurement_available = Some(true);
            if let Some(total) = total_bytes {
                state.total_size = Some(total);
                state.stage_progress = (downloaded_bytes as f32 / total as f32) * 100.0;
            }
            state.download_speed = active.then_some(measured_speed).flatten();

            // Calculate ETA
            if let (Some(speed), Some(total)) = (measured_speed, state.total_size) {
                if speed > 0.0 && downloaded_bytes < total {
                    let remaining = total - downloaded_bytes;
                    state.eta_seconds = Some(remaining as f64 / speed);
                }
            }

            state.overall_progress = self.calculate_overall_progress(state);
        }
        drop(guard);
        let _ = self.persist_state();
    }

    /// Record network transfer details without changing stage completion.
    pub fn update_network_transfer(
        &mut self,
        source_url: Option<&str>,
        active: bool,
        downloaded_bytes: u64,
        total_bytes: Option<u64>,
        speed_bytes_per_sec: Option<f64>,
        measurement_available: bool,
    ) {
        let transfer_key = source_url.map_or_else(
            || "torch-pip-download".to_owned(),
            |source| format!("torch-wheel:{source}"),
        );
        let measured_speed = self
            .record_network_activity(
                &transfer_key,
                source_url,
                active,
                downloaded_bytes,
                total_bytes,
            )
            .or(speed_bytes_per_sec);
        let mut guard = self.state.lock().unwrap();
        if let Some(ref mut state) = *guard {
            state.download_source_url = source_url.map(str::to_owned);
            state.download_active = active;
            state.download_measurement_available = Some(measurement_available);
            state.downloaded_bytes = downloaded_bytes;
            state.total_size = total_bytes;
            state.download_speed = active
                .then_some(measured_speed)
                .flatten()
                .filter(|speed| speed.is_finite() && *speed >= 0.0);
        }
        drop(guard);
        let _ = self.persist_state();
    }

    fn record_network_activity(
        &self,
        transfer_key: &str,
        source_url: Option<&str>,
        active: bool,
        downloaded_bytes: u64,
        total_bytes: Option<u64>,
    ) -> Option<f64> {
        let (operation_id, transfer_id) = {
            let guard = self.state.lock().unwrap();
            let state = guard.as_ref()?;
            let operation_id = OperationId(format!("{}:{}", state.tag, state.started_at));
            let transfer_id = TransferId(format!("{}:{transfer_key}", operation_id.0));
            (operation_id, transfer_id)
        };
        let activity = self
            .network_activity
            .update(ActivityUpdate {
                operation_id,
                transfer_id: transfer_id.clone(),
                direction: TransferDirection::Download,
                source: Some(CopyableSource::new(source_url, None)),
                expected_bytes: total_bytes,
                cumulative_bytes: Some(downloaded_bytes),
                state: if active {
                    TransferState::Active
                } else {
                    TransferState::Completed
                },
                basis: MeasurementBasis::Payload,
                coverage: MeasurementCoverage::Complete,
                sampled_at: Instant::now(),
            })
            .ok()?;
        if !active {
            self.network_activity.remove(&transfer_id);
        }
        activity.rate_bytes_per_second
    }

    fn clear_network_activity(&self) {
        let operation_id = {
            let guard = self.state.lock().unwrap();
            guard
                .as_ref()
                .map(|state| OperationId(format!("{}:{}", state.tag, state.started_at)))
        };
        if let Some(operation_id) = operation_id {
            for activity in self.network_activity.for_operation(&operation_id) {
                self.network_activity.remove(&activity.transfer_id);
            }
        }
    }

    /// Update dependency installation progress.
    pub fn update_dependency_progress(
        &mut self,
        current_package: &str,
        completed_count: u32,
        total_count: Option<u32>,
        _package_size: Option<u64>,
    ) {
        let mut guard = self.state.lock().unwrap();
        if let Some(ref mut state) = *guard {
            state.current_item = Some(current_package.to_string());
            state.completed_dependencies = completed_count;
            if let Some(total) = total_count {
                state.dependency_count = Some(total);
            }

            // Calculate stage progress based on weights
            if state.total_weight > 0 {
                state.stage_progress =
                    (state.completed_weight as f32 / state.total_weight as f32) * 100.0;
            } else if let Some(total) = state.dependency_count {
                if total > 0 {
                    state.stage_progress = (completed_count as f32 / total as f32) * 100.0;
                }
            }

            state.overall_progress = self.calculate_overall_progress(state);
        }
        drop(guard);
        let _ = self.persist_state();
    }

    /// Set dependency weights for accurate progress tracking.
    pub fn set_dependency_weights(&mut self, packages: &[String]) {
        let mut guard = self.state.lock().unwrap();
        if let Some(ref mut state) = *guard {
            state.total_weight = PackageWeights::total_weight(packages);
            state.completed_weight = 0;
        }
        drop(guard);
        let _ = self.persist_state();
    }

    /// Mark a package as completed.
    pub fn complete_package(&mut self, package_name: &str) {
        let mut guard = self.state.lock().unwrap();
        if let Some(ref mut state) = *guard {
            state.completed_weight += PackageWeights::get_weight(package_name);
            state.completed_dependencies += 1;

            // Add to completed items
            state.completed_items.push(InstallationProgressItem {
                name: package_name.to_string(),
                item_type: "package".to_string(),
                size: None,
                completed_at: chrono::Utc::now().to_rfc3339(),
            });

            // Update stage progress
            if state.total_weight > 0 {
                state.stage_progress =
                    (state.completed_weight as f32 / state.total_weight as f32) * 100.0;
            }

            state.overall_progress = self.calculate_overall_progress(state);
        }
        drop(guard);
        let _ = self.persist_state();
    }

    /// Add a completed item (archive, package, etc.).
    pub fn add_completed_item(&mut self, item_name: &str, item_type: &str, size: Option<u64>) {
        let mut guard = self.state.lock().unwrap();
        if let Some(ref mut state) = *guard {
            state.completed_items.push(InstallationProgressItem {
                name: item_name.to_string(),
                item_type: item_type.to_string(),
                size,
                completed_at: chrono::Utc::now().to_rfc3339(),
            });
        }
        drop(guard);
        let _ = self.persist_state();
    }

    /// Set PID for process tracking.
    pub fn set_pid(&mut self, pid: u32) {
        let mut guard = self.state.lock().unwrap();
        if let Some(ref mut state) = *guard {
            state.pid = Some(pid);
        }
        drop(guard);
        let _ = self.persist_state();
    }

    /// Set an error.
    pub fn set_error(&mut self, error_msg: &str) {
        let mut guard = self.state.lock().unwrap();
        if let Some(ref mut state) = *guard {
            state.error = Some(error_msg.to_string());
        }
        drop(guard);
        let _ = self.persist_state();
    }

    /// Mark installation as completed.
    pub fn complete_installation(&mut self, success: bool) {
        self.clear_network_activity();
        let mut guard = self.state.lock().unwrap();
        if let Some(ref mut state) = *guard {
            state.completed_at = Some(chrono::Utc::now().to_rfc3339());
            state.success = Some(success);
            state.download_active = false;
            state.download_measurement_available = None;
            state.download_speed = None;
            if success {
                state.stage_progress = 100.0;
                state.overall_progress = 100.0;
            }
        }
        drop(guard);
        let _ = self.persist_state();
        debug!("Installation completed: success={}", success);
    }

    /// Get the current state as InstallationProgress.
    pub fn get_current_state(&self) -> Option<InstallationProgress> {
        let guard = self.state.lock().unwrap();
        guard.as_ref().map(|state| InstallationProgress {
            tag: Some(state.tag.clone()),
            started_at: Some(state.started_at.clone()),
            stage: Some(state.stage),
            stage_progress: Some(state.stage_progress),
            overall_progress: Some(state.overall_progress),
            current_item: state.current_item.clone(),
            download_source_url: state.download_source_url.clone(),
            download_active: state.download_active,
            download_measurement_available: state.download_measurement_available,
            download_speed: state.download_speed,
            eta_seconds: state.eta_seconds,
            total_size: state.total_size,
            downloaded_bytes: Some(state.downloaded_bytes),
            dependency_count: state.dependency_count,
            completed_dependencies: Some(state.completed_dependencies),
            completed_items: Some(state.completed_items.clone()),
            error: state.error.clone(),
            completed_at: state.completed_at.clone(),
            success: state.success,
            log_path: state.log_path.clone(),
        })
    }

    /// Clear the current state.
    pub fn clear(&mut self) {
        *self.state.lock().unwrap() = None;
        let state_path = self.cache_dir.join(&self.state_filename);
        if state_path.exists() {
            let _ = std::fs::remove_file(&state_path);
        }
    }

    /// Clear state only if installation is completed.
    /// Used to clean up after frontend has had time to poll final status.
    pub fn clear_completed_state(&mut self) {
        let should_clear = {
            let guard = self.state.lock().unwrap();
            guard
                .as_ref()
                .map(|s| s.completed_at.is_some())
                .unwrap_or(false)
        };
        if should_clear {
            debug!("Clearing completed installation state");
            self.clear();
        }
    }

    /// Clear completed state without blocking the async runtime on file removal.
    pub async fn clear_completed_state_async(&mut self) {
        let should_clear = {
            let guard = self.state.lock().unwrap();
            guard
                .as_ref()
                .map(|s| s.completed_at.is_some())
                .unwrap_or(false)
        };
        if should_clear {
            debug!("Clearing completed installation state");
            *self.state.lock().unwrap() = None;
            let state_path = self.cache_dir.join(&self.state_filename);
            if fs::try_exists(&state_path).await.unwrap_or(false) {
                let _ = fs::remove_file(&state_path).await;
            }
        }
    }

    /// Calculate overall progress based on stage weights.
    fn calculate_overall_progress(&self, state: &InstallationProgressState) -> f32 {
        let stage_weight = state.stage.weight();
        let stage_start = match state.stage {
            InstallationStage::Resolving => 0.0,
            InstallationStage::Download => InstallationStage::Resolving.cumulative_weight(),
            InstallationStage::Extract => InstallationStage::Download.cumulative_weight(),
            InstallationStage::Venv => InstallationStage::Extract.cumulative_weight(),
            InstallationStage::Dependencies => InstallationStage::Venv.cumulative_weight(),
            InstallationStage::Setup => InstallationStage::Dependencies.cumulative_weight(),
        };

        let stage_contribution = (state.stage_progress / 100.0) * stage_weight;
        ((stage_start + stage_contribution) * 100.0).clamp(0.0, 100.0)
    }

    fn snapshot_persist_payload(&self) -> Result<Option<(PathBuf, String)>> {
        let guard = self.state.lock().unwrap();
        let Some(state) = guard.as_ref() else {
            return Ok(None);
        };

        let state_path = self.cache_dir.join(&self.state_filename);
        let json = serde_json::to_string_pretty(state)?;
        Ok(Some((state_path, json)))
    }

    async fn persist_payload_async(state_path: PathBuf, json: String) {
        if let Some(parent) = state_path.parent() {
            if let Err(err) = fs::create_dir_all(parent).await {
                warn!("Failed to create progress cache directory: {}", err);
                return;
            }
        }

        if let Err(err) = fs::write(&state_path, json).await {
            warn!(
                "Failed to write progress state {}: {}",
                state_path.display(),
                err
            );
        }
    }

    fn persist_payload_blocking(state_path: &PathBuf, json: &str) -> Result<()> {
        if let Some(parent) = state_path.parent() {
            if !parent.exists() {
                std::fs::create_dir_all(parent).map_err(|e| PumasError::Io {
                    message: format!("Failed to create cache directory: {}", e),
                    path: Some(parent.to_path_buf()),
                    source: Some(e),
                })?;
            }
        }

        std::fs::write(state_path, json).map_err(|e| PumasError::Io {
            message: format!("Failed to write progress state: {}", e),
            path: Some(state_path.clone()),
            source: Some(e),
        })?;

        Ok(())
    }

    /// Persist state to disk.
    fn persist_state(&self) -> Result<()> {
        let Some((state_path, json)) = self.snapshot_persist_payload()? else {
            return Ok(());
        };

        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            handle.spawn(Self::persist_payload_async(state_path, json));
            return Ok(());
        }

        Self::persist_payload_blocking(&state_path, &json)?;
        Ok(())
    }

    /// Load state from disk (for recovery).
    pub fn load_from_disk(&mut self) -> Option<InstallationProgress> {
        let state_path = self.cache_dir.join(&self.state_filename);
        if !state_path.exists() {
            return None;
        }

        match std::fs::read_to_string(&state_path) {
            Ok(json) => match serde_json::from_str::<InstallationProgressState>(&json) {
                Ok(state) => {
                    let progress = InstallationProgress {
                        tag: Some(state.tag.clone()),
                        started_at: Some(state.started_at.clone()),
                        stage: Some(state.stage),
                        stage_progress: Some(state.stage_progress),
                        overall_progress: Some(state.overall_progress),
                        current_item: state.current_item.clone(),
                        download_source_url: state.download_source_url.clone(),
                        download_active: state.download_active,
                        download_measurement_available: state.download_measurement_available,
                        download_speed: state.download_speed,
                        eta_seconds: state.eta_seconds,
                        total_size: state.total_size,
                        downloaded_bytes: Some(state.downloaded_bytes),
                        dependency_count: state.dependency_count,
                        completed_dependencies: Some(state.completed_dependencies),
                        completed_items: Some(state.completed_items.clone()),
                        error: state.error.clone(),
                        completed_at: state.completed_at.clone(),
                        success: state.success,
                        log_path: state.log_path.clone(),
                    };
                    *self.state.lock().unwrap() = Some(state);
                    Some(progress)
                }
                Err(e) => {
                    warn!("Failed to parse progress state: {}", e);
                    None
                }
            },
            Err(e) => {
                warn!("Failed to read progress state: {}", e);
                None
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_package_weights() {
        assert_eq!(PackageWeights::get_weight("torch"), 15);
        assert_eq!(PackageWeights::get_weight("numpy"), 2);
        assert_eq!(PackageWeights::get_weight("unknown-pkg"), 1);

        let packages = vec![
            "torch".to_string(),
            "numpy".to_string(),
            "requests".to_string(),
        ];
        assert_eq!(PackageWeights::total_weight(&packages), 15 + 2 + 1);
    }

    #[test]
    fn test_progress_tracker_start() {
        let temp_dir = TempDir::new().unwrap();
        let mut tracker = InstallationProgressTracker::new(temp_dir.path().to_path_buf());

        tracker.start_installation("v1.0.0", Some(1000), Some(10), None);

        let state = tracker.get_current_state().unwrap();
        assert_eq!(state.tag, Some("v1.0.0".to_string()));
        assert_eq!(state.stage, Some(InstallationStage::Download));
        assert_eq!(state.overall_progress, Some(0.0));
    }

    #[test]
    fn test_progress_tracker_update_stage() {
        let temp_dir = TempDir::new().unwrap();
        let mut tracker = InstallationProgressTracker::new(temp_dir.path().to_path_buf());

        tracker.start_installation("v1.0.0", None, None, None);
        tracker.update_stage(InstallationStage::Extract, 50.0, Some("archive.zip"));

        let state = tracker.get_current_state().unwrap();
        assert_eq!(state.stage, Some(InstallationStage::Extract));
        assert_eq!(state.stage_progress, Some(50.0));
        assert_eq!(state.current_item, Some("archive.zip".to_string()));
    }

    #[test]
    fn test_progress_tracker_download_progress() {
        let temp_dir = TempDir::new().unwrap();
        let mut tracker = InstallationProgressTracker::new(temp_dir.path().to_path_buf());

        tracker.start_installation("v1.0.0", Some(1000), None, None);
        tracker.update_download_progress(500, Some(1000), Some(100.0));

        let state = tracker.get_current_state().unwrap();
        assert_eq!(state.downloaded_bytes, Some(500));
        assert_eq!(state.download_speed, Some(100.0));
        assert!(state.eta_seconds.is_some());
    }

    #[test]
    fn network_transfer_preserves_source_and_speed_without_advancing_stage() {
        let temp_dir = TempDir::new().unwrap();
        let mut tracker = InstallationProgressTracker::new(temp_dir.path().to_path_buf());
        tracker.start_installation("v2.14.0", None, None, None);
        tracker.update_stage(
            InstallationStage::Setup,
            0.0,
            Some("Resolving Torch packages"),
        );
        tracker.update_network_transfer(
            Some("https://download.pytorch.org/whl/cu134/torch-2.14.0.whl"),
            true,
            1024,
            Some(4096),
            Some(512.0),
            true,
        );

        let state = tracker.get_current_state().unwrap();
        assert_eq!(state.stage, Some(InstallationStage::Setup));
        assert_eq!(state.stage_progress, Some(0.0));
        assert!(state.download_active);
        assert_eq!(
            state.download_source_url.as_deref(),
            Some("https://download.pytorch.org/whl/cu134/torch-2.14.0.whl")
        );
        assert_eq!(state.downloaded_bytes, Some(1024));
        assert_eq!(state.total_size, Some(4096));
        assert_eq!(state.download_speed, Some(512.0));
        assert_eq!(state.download_measurement_available, Some(true));
    }

    #[test]
    fn stage_change_clears_previous_network_source_and_speed() {
        let temp_dir = TempDir::new().unwrap();
        let mut tracker = InstallationProgressTracker::new(temp_dir.path().to_path_buf());
        tracker.start_installation("v2.14.0", None, None, None);
        tracker.update_network_transfer(
            Some("https://files.pythonhosted.org/packages/pkg-1.0.whl"),
            true,
            20,
            Some(40),
            Some(10.0),
            true,
        );
        tracker.update_stage(
            InstallationStage::Setup,
            0.0,
            Some("Checking installed Torch"),
        );

        let state = tracker.get_current_state().unwrap();
        assert_eq!(state.download_source_url, None);
        assert!(!state.download_active);
        assert_eq!(state.download_speed, None);
        assert_eq!(state.download_measurement_available, None);
    }

    #[test]
    fn completed_transfer_and_installation_clear_active_rate() {
        let temp_dir = TempDir::new().unwrap();
        let mut tracker = InstallationProgressTracker::new(temp_dir.path().to_path_buf());
        tracker.start_installation("v2.14.0", None, None, None);
        tracker.update_network_transfer(
            Some("https://files.pythonhosted.org/packages/pkg-1.0.whl"),
            true,
            20,
            Some(40),
            Some(10.0),
            true,
        );
        tracker.update_network_transfer(
            Some("https://files.pythonhosted.org/packages/pkg-1.0.whl"),
            false,
            40,
            Some(40),
            None,
            true,
        );
        let transfer = tracker.get_current_state().unwrap();
        assert!(!transfer.download_active);
        assert_eq!(transfer.download_speed, None);

        tracker.update_network_transfer(
            Some("https://files.pythonhosted.org/packages/pkg-1.0.whl"),
            true,
            20,
            Some(40),
            Some(10.0),
            true,
        );
        tracker.complete_installation(false);
        let terminal = tracker.get_current_state().unwrap();
        assert!(!terminal.download_active);
        assert_eq!(terminal.download_speed, None);
    }

    #[test]
    fn network_activity_measures_downloads_without_exposing_unapproved_sources() {
        let temp_dir = TempDir::new().unwrap();
        let mut tracker = InstallationProgressTracker::new(temp_dir.path().to_path_buf());
        tracker.start_installation("v2.14.0", None, None, None);
        tracker.update_network_transfer(None, true, 0, Some(40), None, true);
        tracker.update_network_transfer(None, true, 40, Some(40), None, true);

        let state = tracker.get_current_state().unwrap();
        assert_eq!(state.download_source_url, None);
        assert!(state
            .download_speed
            .is_some_and(|speed| speed.is_finite() && speed > 0.0));
    }

    #[test]
    fn test_progress_tracker_complete_package() {
        let temp_dir = TempDir::new().unwrap();
        let mut tracker = InstallationProgressTracker::new(temp_dir.path().to_path_buf());

        tracker.start_installation("v1.0.0", None, Some(3), None);
        tracker.set_dependency_weights(&["torch".to_string(), "numpy".to_string()]);

        tracker.update_stage(InstallationStage::Dependencies, 0.0, None);
        tracker.complete_package("torch");

        let state = tracker.get_current_state().unwrap();
        assert_eq!(state.completed_dependencies, Some(1));
        assert!(state.completed_items.as_ref().unwrap().len() == 1);
    }

    #[test]
    fn test_progress_tracker_persistence() {
        let temp_dir = TempDir::new().unwrap();

        // Create and save state
        {
            let mut tracker = InstallationProgressTracker::new(temp_dir.path().to_path_buf());
            tracker.start_installation("v1.0.0", Some(1000), Some(10), None);
        }

        // Load from disk
        {
            let mut tracker = InstallationProgressTracker::new(temp_dir.path().to_path_buf());
            let loaded = tracker.load_from_disk();
            assert!(loaded.is_some());
            assert_eq!(loaded.unwrap().tag, Some("v1.0.0".to_string()));
        }
    }

    #[test]
    fn test_overall_progress_calculation() {
        let temp_dir = TempDir::new().unwrap();
        let mut tracker = InstallationProgressTracker::new(temp_dir.path().to_path_buf());

        tracker.start_installation("v1.0.0", None, None, None);

        // Download complete (15% weight)
        tracker.update_stage(InstallationStage::Download, 100.0, None);
        let state = tracker.get_current_state().unwrap();
        assert!((state.overall_progress.unwrap() - 15.0).abs() < 0.1);

        // Extract complete (+5% = 20%)
        tracker.update_stage(InstallationStage::Extract, 100.0, None);
        let state = tracker.get_current_state().unwrap();
        assert!((state.overall_progress.unwrap() - 20.0).abs() < 0.1);

        // Dependencies at 50% (25% + 35% = 60%)
        tracker.update_stage(InstallationStage::Dependencies, 50.0, None);
        let state = tracker.get_current_state().unwrap();
        assert!((state.overall_progress.unwrap() - 60.0).abs() < 0.1);
    }
}
