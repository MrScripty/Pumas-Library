//! High-level process management.

use super::detection::{DetectedProcess, ProcessDetector, ProcessSource};
use super::launcher::{LaunchConfig, LaunchResult, ProcessLauncher};
use crate::error::{PumasError, Result};
use crate::system::{ProcessResources, ResourceTracker};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, RwLock};
use std::fs;
use tracing::{debug, error, info, warn};

/// Process with resource information.
#[derive(Debug, Clone)]
pub struct ProcessInfo {
    /// Process ID.
    pub pid: u32,
    /// How the process was detected.
    pub source: ProcessSource,
    /// Version tag (if identified).
    pub tag: Option<String>,
    /// Command line (if available).
    pub cmdline: Option<String>,
    /// CPU usage percentage.
    pub cpu_usage: f32,
    /// RAM memory usage in GB.
    pub ram_memory: f32,
    /// GPU memory usage in GB.
    pub gpu_memory: f32,
}

/// Process manager for ComfyUI and other managed applications.
pub struct ProcessManager {
    /// Root directory (launcher root or app root).
    root_dir: PathBuf,
    /// Process detector.
    detector: Arc<RwLock<ProcessDetector>>,
    /// Resource tracker.
    resource_tracker: Arc<ResourceTracker>,
    /// Last launch log path (exclusive access only).
    last_launch_log: Arc<Mutex<Option<PathBuf>>>,
    /// Last launch error message (exclusive access only).
    last_launch_error: Arc<Mutex<Option<String>>>,
}

impl ProcessManager {
    /// Create a new process manager.
    ///
    /// # Arguments
    ///
    /// * `root_dir` - Root directory for the application
    /// * `version_paths` - Optional map of version tags to directories
    pub fn new(
        root_dir: impl AsRef<Path>,
        version_paths: Option<HashMap<String, PathBuf>>,
    ) -> Result<Self> {
        let root_dir = root_dir.as_ref().to_path_buf();

        Ok(Self {
            root_dir: root_dir.clone(),
            detector: Arc::new(RwLock::new(ProcessDetector::new(
                &root_dir,
                version_paths.unwrap_or_default(),
            ))),
            resource_tracker: Arc::new(ResourceTracker::default()),
            last_launch_log: Arc::new(Mutex::new(None)),
            last_launch_error: Arc::new(Mutex::new(None)),
        })
    }

    /// Update the known version paths.
    pub fn set_version_paths(&self, version_paths: HashMap<String, PathBuf>) {
        info!(
            "set_version_paths called with {} entries: {:?}",
            version_paths.len(),
            version_paths.keys().collect::<Vec<_>>()
        );
        let mut detector = self.detector.write().unwrap();
        detector.set_version_paths(version_paths);
    }

    /// Check if any managed process is running.
    pub fn is_running(&self) -> bool {
        let detector = self.detector.read().unwrap();
        let processes = detector.detect_processes();
        let running = !processes.is_empty();
        info!(
            "ProcessManager.is_running: {} (found {} processes)",
            running,
            processes.len()
        );
        running
    }

    /// Get all running processes with resource information.
    pub fn get_processes_with_resources(&self) -> Vec<ProcessInfo> {
        let detector = self.detector.read().unwrap();
        let processes = detector.detect_processes();

        processes
            .into_iter()
            .map(|proc| {
                let resources = self
                    .resource_tracker
                    .get_process_resources(proc.pid, false)
                    .unwrap_or_default();

                ProcessInfo {
                    pid: proc.pid,
                    source: proc.source,
                    tag: proc.tag,
                    cmdline: proc.cmdline,
                    cpu_usage: resources.cpu,
                    ram_memory: resources.ram_memory,
                    gpu_memory: resources.gpu_memory,
                }
            })
            .collect()
    }

    /// Get running processes without resource information (faster).
    pub fn get_running_processes(&self) -> Vec<DetectedProcess> {
        let detector = self.detector.read().unwrap();
        detector.detect_processes()
    }

    /// Launch a version.
    ///
    /// # Arguments
    ///
    /// * `tag` - Version tag to launch
    /// * `version_dir` - Path to the version directory
    /// * `log_dir` - Optional directory for log files
    pub fn launch_version(
        &self,
        tag: &str,
        version_dir: &Path,
        log_dir: Option<&Path>,
    ) -> LaunchResult {
        // Clear previous error
        {
            let mut error = self.last_launch_error.lock().unwrap();
            *error = None;
        }

        // Determine log file path
        let log_file = log_dir.map(|dir| {
            let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
            dir.join(format!("comfyui_{}_{}.log", tag, timestamp))
        });

        // Build launch config
        let mut config = LaunchConfig::new(tag, version_dir);
        if let Some(ref log_path) = log_file {
            config = config.with_log_file(log_path);
        }

        // Launch
        let result = match ProcessLauncher::launch(&config) {
            Ok(r) => r,
            Err(e) => {
                let error_msg = format!("Launch error: {}", e);
                error!("{}", error_msg);

                let mut last_error = self.last_launch_error.lock().unwrap();
                *last_error = Some(error_msg.clone());

                return LaunchResult {
                    success: false,
                    process: None,
                    log_path: log_file,
                    error: Some(error_msg),
                    ready: false,
                };
            }
        };

        // Update state
        if result.success {
            let mut log = self.last_launch_log.lock().unwrap();
            *log = result.log_path.clone();
        } else if let Some(ref error) = result.error {
            let mut last_error = self.last_launch_error.lock().unwrap();
            *last_error = Some(error.clone());
        }

        result
    }

    /// Stop all running ComfyUI processes.
    pub fn stop_all(&self) -> Result<bool> {
        let detector = self.detector.read().unwrap();
        let processes = detector.detect_processes();
        let timeout_ms = 2000; // 2 second grace period

        info!(
            "stop_all: detected {} processes to stop",
            processes.len()
        );

        let mut stopped_any = false;

        // Stop each detected process
        for proc in &processes {
            info!(
                "Stopping process {} (tag={}, source={:?}, pid_file={:?})",
                proc.pid,
                proc.tag.as_deref().unwrap_or("unknown"),
                proc.source,
                proc.pid_file
            );

            // Stop the process using platform module
            let stop_result = ProcessLauncher::stop_process(proc.pid, timeout_ms)?;
            info!("stop_process({}) returned: {}", proc.pid, stop_result);
            if stop_result {
                stopped_any = true;
            }

            // Remove PID file if present
            if let Some(ref pid_file) = proc.pid_file {
                info!("Removing PID file: {:?}", pid_file);
                if let Err(e) = ProcessLauncher::remove_pid_file(pid_file) {
                    warn!("Failed to remove PID file {:?}: {}", pid_file, e);
                } else {
                    info!("Successfully removed PID file: {:?}", pid_file);
                }
            } else {
                warn!(
                    "Process {} has NO pid_file (detected via {:?}) - cannot remove PID file!",
                    proc.pid, proc.source
                );
            }
        }

        // Also scan for PID files in comfyui-versions directory
        // This catches cases where version_paths is not populated
        let versions_dir = self.root_dir.join("comfyui-versions");
        info!("Scanning for orphaned PID files in: {:?}", versions_dir);
        if versions_dir.exists() {
            if let Ok(entries) = fs::read_dir(&versions_dir) {
                for entry in entries.flatten() {
                    let pid_file = entry.path().join("comfyui.pid");
                    if pid_file.exists() {
                        warn!(
                            "Found orphaned PID file (not tracked by version_paths): {:?}",
                            pid_file
                        );
                        if let Err(e) = fs::remove_file(&pid_file) {
                            warn!("Failed to remove orphaned PID file {:?}: {}", pid_file, e);
                        } else {
                            info!("Removed orphaned PID file: {:?}", pid_file);
                        }
                    }
                }
            }
        }

        // Cleanup any orphaned ComfyUI processes (cross-platform)
        let orphaned = ProcessLauncher::stop_processes_by_pattern("comfyui-versions", timeout_ms)?;
        if orphaned > 0 {
            info!("Stopped {} orphaned comfyui processes", orphaned);
            stopped_any = true;
        }

        // Also cleanup browser windows running ComfyUI app mode (cross-platform)
        let browser_windows = ProcessLauncher::stop_processes_by_pattern("--app=http://127.0.0.1", 500)?;
        if browser_windows > 0 {
            info!("Stopped {} browser app windows", browser_windows);
        }

        info!("stop_all completed, stopped_any={}", stopped_any);
        Ok(stopped_any)
    }

    /// Get the last launch log path.
    pub fn last_launch_log(&self) -> Option<PathBuf> {
        self.last_launch_log.lock().unwrap().clone()
    }

    /// Get the last launch error message.
    pub fn last_launch_error(&self) -> Option<String> {
        self.last_launch_error.lock().unwrap().clone()
    }

    /// Get the resource tracker.
    pub fn resource_tracker(&self) -> &Arc<ResourceTracker> {
        &self.resource_tracker
    }

    /// Aggregate resources for all running processes of an app type.
    pub fn aggregate_app_resources(&self) -> Option<ProcessResources> {
        let processes = self.get_processes_with_resources();

        if processes.is_empty() {
            return None;
        }

        let mut total_cpu = 0.0f32;
        let mut total_ram = 0.0f32;
        let mut total_gpu = 0.0f32;

        for proc in &processes {
            total_cpu += proc.cpu_usage;
            total_ram += proc.ram_memory;
            total_gpu += proc.gpu_memory;
        }

        Some(ProcessResources {
            cpu: (total_cpu * 10.0).round() / 10.0,
            ram_memory: (total_ram * 100.0).round() / 100.0,
            gpu_memory: (total_gpu * 100.0).round() / 100.0,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_process_manager_creation() {
        let temp_dir = TempDir::new().unwrap();
        let manager = ProcessManager::new(temp_dir.path(), None).unwrap();

        // Should have no processes running in a temp dir
        assert!(!manager.is_running());
        assert!(manager.get_running_processes().is_empty());
    }

    #[test]
    fn test_set_version_paths() {
        let temp_dir = TempDir::new().unwrap();
        let manager = ProcessManager::new(temp_dir.path(), None).unwrap();

        let mut paths = HashMap::new();
        paths.insert(
            "v1.0.0".to_string(),
            temp_dir.path().join("versions").join("v1.0.0"),
        );

        manager.set_version_paths(paths);

        // Should still work after setting paths
        assert!(!manager.is_running());
    }

    #[test]
    fn test_last_launch_state() {
        let temp_dir = TempDir::new().unwrap();
        let manager = ProcessManager::new(temp_dir.path(), None).unwrap();

        // Initially should be None
        assert!(manager.last_launch_log().is_none());
        assert!(manager.last_launch_error().is_none());
    }
}
