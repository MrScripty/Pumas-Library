//! High-level process management.

use super::detection::{DetectedProcess, ProcessDetector, ProcessSource};
use super::launcher::{BinaryLaunchConfig, LaunchConfig, LaunchResult, ProcessLauncher};
use crate::error::Result;
use crate::system::{ProcessResources, ResourceTracker};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, RwLock};
use tracing::{debug, error, info, warn};

/// Process with resource information.
#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Default)]
struct CachedProcessStatus {
    running: bool,
}

/// Process manager for ComfyUI and other managed applications.
#[derive(Clone)]
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
    /// Cached ComfyUI liveness from startup, launch, stop, or explicit refresh.
    comfyui_status: Arc<Mutex<CachedProcessStatus>>,
    /// Cached Ollama liveness from startup, launch, stop, or explicit refresh.
    ollama_status: Arc<Mutex<CachedProcessStatus>>,
    /// Cached Torch liveness from startup, launch, stop, or explicit refresh.
    torch_status: Arc<Mutex<CachedProcessStatus>>,
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
        let detector = ProcessDetector::new(&root_dir, version_paths.unwrap_or_default());

        let comfyui_status = CachedProcessStatus {
            running: detector.is_any_running(),
        };
        let ollama_status = CachedProcessStatus {
            running: Self::detect_ollama_running(&root_dir),
        };
        let torch_status = CachedProcessStatus {
            running: Self::detect_torch_running(&root_dir),
        };

        Ok(Self {
            root_dir: root_dir.clone(),
            detector: Arc::new(RwLock::new(detector)),
            resource_tracker: Arc::new(ResourceTracker::default()),
            last_launch_log: Arc::new(Mutex::new(None)),
            last_launch_error: Arc::new(Mutex::new(None)),
            comfyui_status: Arc::new(Mutex::new(comfyui_status)),
            ollama_status: Arc::new(Mutex::new(ollama_status)),
            torch_status: Arc::new(Mutex::new(torch_status)),
        })
    }

    /// Update the known version paths.
    pub fn set_version_paths(&self, version_paths: HashMap<String, PathBuf>) {
        {
            let mut detector = self.detector.write().unwrap();
            detector.set_version_paths(version_paths);
        }
        self.refresh_comfyui_running();
    }

    /// Return cached ComfyUI liveness.
    pub fn is_running(&self) -> bool {
        self.comfyui_status.lock().unwrap().running
    }

    /// Explicitly refresh ComfyUI liveness by detecting managed processes.
    pub fn refresh_comfyui_running(&self) -> bool {
        let running = {
            let detector = self.detector.read().unwrap();
            detector.is_any_running()
        };
        self.set_comfyui_status(running);
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
            self.set_comfyui_status(true);
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

        info!("stop_all: detected {} processes to stop", processes.len());

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
        let browser_windows =
            ProcessLauncher::stop_processes_by_pattern("--app=http://127.0.0.1", 500)?;
        if browser_windows > 0 {
            info!("Stopped {} browser app windows", browser_windows);
        }

        if stopped_any {
            self.set_comfyui_status(false);
        } else {
            self.refresh_comfyui_running();
        }

        info!("stop_all completed, stopped_any={}", stopped_any);
        Ok(stopped_any)
    }

    /// Launch an Ollama binary version.
    ///
    /// # Arguments
    ///
    /// * `tag` - Version tag to launch
    /// * `version_dir` - Path to the version directory containing the ollama binary
    /// * `log_dir` - Optional directory for log files
    pub fn launch_ollama(
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
            dir.join(format!("ollama_{}_{}.log", tag, timestamp))
        });

        // Build launch config
        let mut config = BinaryLaunchConfig::ollama(tag, version_dir);
        if let Some(ref log_path) = log_file {
            config = config.with_log_file(log_path);
        }

        // Launch
        let result = match ProcessLauncher::launch_binary(&config) {
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
            self.set_ollama_status(true);
        } else if let Some(ref error) = result.error {
            let mut last_error = self.last_launch_error.lock().unwrap();
            *last_error = Some(error.clone());
        }

        result
    }

    /// Stop Ollama processes.
    ///
    /// Looks for ollama.pid files in the ollama-versions directory and stops those processes.
    pub fn stop_ollama(&self) -> Result<bool> {
        let timeout_ms = 2000;
        let mut stopped_any = false;

        // Scan for PID files in ollama-versions directory
        let versions_dir = self.root_dir.join("ollama-versions");
        info!("Scanning for Ollama PID files in: {:?}", versions_dir);

        if versions_dir.exists() {
            if let Ok(entries) = fs::read_dir(&versions_dir) {
                for entry in entries.flatten() {
                    let pid_file = entry.path().join("ollama.pid");
                    if pid_file.exists() {
                        // Read PID from file
                        if let Ok(pid_str) = fs::read_to_string(&pid_file) {
                            if let Ok(pid) = pid_str.trim().parse::<u32>() {
                                info!("Stopping Ollama process {} from {:?}", pid, pid_file);
                                if ProcessLauncher::stop_process(pid, timeout_ms)? {
                                    stopped_any = true;
                                }
                                // Remove PID file
                                if let Err(e) = ProcessLauncher::remove_pid_file(&pid_file) {
                                    warn!("Failed to remove PID file {:?}: {}", pid_file, e);
                                } else {
                                    info!("Removed Ollama PID file: {:?}", pid_file);
                                }
                            }
                        }
                    }
                }
            }
        }

        // Also cleanup any orphaned Ollama processes by pattern
        let orphaned = ProcessLauncher::stop_processes_by_pattern("ollama serve", timeout_ms)?;
        if orphaned > 0 {
            info!("Stopped {} orphaned ollama processes", orphaned);
            stopped_any = true;
        }

        if stopped_any {
            self.set_ollama_status(false);
        } else {
            self.refresh_ollama_running();
        }

        info!("stop_ollama completed, stopped_any={}", stopped_any);
        Ok(stopped_any)
    }

    /// Return cached Ollama liveness.
    ///
    /// This is intentionally a non-scanning read. Expensive process-table
    /// fallback detection happens only at startup or explicit refresh points.
    pub fn is_ollama_running(&self) -> bool {
        self.ollama_status.lock().unwrap().running
    }

    /// Explicitly refresh Ollama liveness by looking for PID files or running processes.
    pub fn refresh_ollama_running(&self) -> bool {
        let running = Self::detect_ollama_running(&self.root_dir);
        self.set_ollama_status(running);
        running
    }

    fn set_ollama_status(&self, running: bool) {
        let mut status = self.ollama_status.lock().unwrap();
        status.running = running;
    }

    fn detect_ollama_running(root_dir: &Path) -> bool {
        // Check for PID files in ollama-versions directory
        let versions_dir = root_dir.join("ollama-versions");
        if versions_dir.exists() {
            if let Ok(entries) = fs::read_dir(&versions_dir) {
                for entry in entries.flatten() {
                    let pid_file = entry.path().join("ollama.pid");
                    if pid_file.exists() {
                        // Read PID and check if process is alive
                        if let Ok(pid_str) = fs::read_to_string(&pid_file) {
                            if let Ok(pid) = pid_str.trim().parse::<u32>() {
                                if crate::platform::is_process_alive(pid) {
                                    return true;
                                }
                            }
                        }
                    }
                }
            }
        }

        // Fallback: check for running ollama process by pattern
        let processes = crate::platform::find_processes_by_cmdline("ollama");
        for (_pid, cmdline) in &processes {
            if cmdline.contains("serve") {
                return true;
            }
        }

        false
    }

    fn set_comfyui_status(&self, running: bool) {
        let mut status = self.comfyui_status.lock().unwrap();
        status.running = running;
    }

    /// Launch the Torch inference server.
    ///
    /// # Arguments
    ///
    /// * `tag` - Version tag to launch
    /// * `version_dir` - Path to the version directory containing the torch server
    /// * `log_dir` - Optional directory for log files
    pub fn launch_torch(
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
            dir.join(format!("torch_{}_{}.log", tag, timestamp))
        });

        // Build launch config
        let mut config = BinaryLaunchConfig::torch(tag, version_dir);
        if let Some(ref log_path) = log_file {
            config = config.with_log_file(log_path);
        }

        // Launch
        let result = match ProcessLauncher::launch_binary(&config) {
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
            self.set_torch_status(true);
        } else if let Some(ref error) = result.error {
            let mut last_error = self.last_launch_error.lock().unwrap();
            *last_error = Some(error.clone());
        }

        result
    }

    /// Stop Torch server processes.
    ///
    /// Looks for torch.pid files in the torch-versions directory and stops those processes.
    pub fn stop_torch(&self) -> Result<bool> {
        let timeout_ms = 2000;
        let mut stopped_any = false;

        // Scan for PID files in torch-versions directory
        let versions_dir = self.root_dir.join("torch-versions");
        info!("Scanning for Torch PID files in: {:?}", versions_dir);

        if versions_dir.exists() {
            if let Ok(entries) = fs::read_dir(&versions_dir) {
                for entry in entries.flatten() {
                    let pid_file = entry.path().join("torch.pid");
                    if pid_file.exists() {
                        if let Ok(pid_str) = fs::read_to_string(&pid_file) {
                            if let Ok(pid) = pid_str.trim().parse::<u32>() {
                                info!("Stopping Torch process {} from {:?}", pid, pid_file);
                                if ProcessLauncher::stop_process(pid, timeout_ms)? {
                                    stopped_any = true;
                                }
                                if let Err(e) = ProcessLauncher::remove_pid_file(&pid_file) {
                                    warn!("Failed to remove PID file {:?}: {}", pid_file, e);
                                } else {
                                    info!("Removed Torch PID file: {:?}", pid_file);
                                }
                            }
                        }
                    }
                }
            }
        }

        // Also cleanup any orphaned torch serve processes by pattern
        let orphaned = ProcessLauncher::stop_processes_by_pattern("serve.py", timeout_ms)?;
        if orphaned > 0 {
            info!("Stopped {} orphaned torch server processes", orphaned);
            stopped_any = true;
        }

        if stopped_any {
            self.set_torch_status(false);
        } else {
            self.refresh_torch_running();
        }

        info!("stop_torch completed, stopped_any={}", stopped_any);
        Ok(stopped_any)
    }

    /// Return cached Torch liveness.
    pub fn is_torch_running(&self) -> bool {
        self.torch_status.lock().unwrap().running
    }

    /// Explicitly refresh Torch liveness by checking known PID files.
    pub fn refresh_torch_running(&self) -> bool {
        let running = Self::detect_torch_running(&self.root_dir);
        self.set_torch_status(running);
        running
    }

    fn set_torch_status(&self, running: bool) {
        let mut status = self.torch_status.lock().unwrap();
        status.running = running;
    }

    fn detect_torch_running(root_dir: &Path) -> bool {
        // Check for PID files in torch-versions directory
        let versions_dir = root_dir.join("torch-versions");
        if versions_dir.exists() {
            if let Ok(entries) = fs::read_dir(&versions_dir) {
                for entry in entries.flatten() {
                    let pid_file = entry.path().join("torch.pid");
                    if pid_file.exists() {
                        if let Ok(pid_str) = fs::read_to_string(&pid_file) {
                            if let Ok(pid) = pid_str.trim().parse::<u32>() {
                                if crate::platform::is_process_alive(pid) {
                                    return true;
                                }
                            }
                        }
                    }
                }
            }
        }

        false
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

    /// Aggregate resources for running Ollama processes.
    pub fn aggregate_ollama_resources(&self) -> Option<ProcessResources> {
        let versions_dir = self.root_dir.join("ollama-versions");
        debug!("aggregate_ollama_resources: checking {:?}", versions_dir);
        if !versions_dir.exists() {
            debug!("aggregate_ollama_resources: versions_dir does not exist");
            return None;
        }

        let mut total_cpu = 0.0f32;
        let mut total_ram = 0.0f32;
        let mut total_gpu = 0.0f32;
        let mut found_any = false;

        // Scan for PID files in ollama-versions directory
        if let Ok(entries) = fs::read_dir(&versions_dir) {
            for entry in entries.flatten() {
                let pid_file = entry.path().join("ollama.pid");
                debug!(
                    "aggregate_ollama_resources: checking pid_file {:?}, exists={}",
                    pid_file,
                    pid_file.exists()
                );
                if pid_file.exists() {
                    if let Ok(pid_str) = fs::read_to_string(&pid_file) {
                        if let Ok(pid) = pid_str.trim().parse::<u32>() {
                            debug!("aggregate_ollama_resources: found PID {}", pid);
                            let alive = crate::platform::is_process_alive(pid);
                            debug!("aggregate_ollama_resources: PID {} alive={}", pid, alive);
                            if alive {
                                // Process is alive, get its resources
                                match self.resource_tracker.get_process_resources(pid, true) {
                                    Ok(resources) => {
                                        debug!("aggregate_ollama_resources: PID {} resources: cpu={}, ram={}, gpu={}",
                                            pid, resources.cpu, resources.ram_memory, resources.gpu_memory);
                                        total_cpu += resources.cpu;
                                        total_ram += resources.ram_memory;
                                        total_gpu += resources.gpu_memory;
                                        found_any = true;
                                    }
                                    Err(e) => {
                                        warn!("aggregate_ollama_resources: failed to get resources for PID {}: {}", pid, e);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        debug!(
            "aggregate_ollama_resources: found_any={}, total_ram={}, total_gpu={}",
            found_any, total_ram, total_gpu
        );
        if !found_any {
            return None;
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

    #[test]
    fn ollama_liveness_read_uses_cache_until_explicit_refresh() {
        let temp_dir = TempDir::new().unwrap();
        let manager = ProcessManager::new(temp_dir.path(), None).unwrap();
        let initial_running = manager.is_ollama_running();
        let version_dir = temp_dir.path().join("ollama-versions").join("test");
        fs::create_dir_all(&version_dir).unwrap();
        fs::write(
            version_dir.join("ollama.pid"),
            std::process::id().to_string(),
        )
        .unwrap();

        assert_eq!(manager.is_ollama_running(), initial_running);

        assert!(manager.refresh_ollama_running());
        assert!(manager.is_ollama_running());
    }

    #[test]
    fn comfyui_liveness_read_uses_cache_until_explicit_refresh() {
        let temp_dir = TempDir::new().unwrap();
        let manager = ProcessManager::new(temp_dir.path(), None).unwrap();
        let initial_running = manager.is_running();
        fs::write(
            temp_dir.path().join("comfyui.pid"),
            std::process::id().to_string(),
        )
        .unwrap();

        assert_eq!(manager.is_running(), initial_running);

        assert!(manager.refresh_comfyui_running());
        assert!(manager.is_running());
    }

    #[test]
    fn torch_liveness_read_uses_cache_until_explicit_refresh() {
        let temp_dir = TempDir::new().unwrap();
        let manager = ProcessManager::new(temp_dir.path(), None).unwrap();
        let initial_running = manager.is_torch_running();
        let version_dir = temp_dir.path().join("torch-versions").join("test");
        fs::create_dir_all(&version_dir).unwrap();
        fs::write(
            version_dir.join("torch.pid"),
            std::process::id().to_string(),
        )
        .unwrap();

        assert_eq!(manager.is_torch_running(), initial_running);

        assert!(manager.refresh_torch_running());
        assert!(manager.is_torch_running());
    }
}
