//! High-level process management.

use super::launcher::{BinaryLaunchConfig, LaunchResult, ProcessLauncher};
use crate::error::{PumasError, Result};
use crate::system::{ProcessResources, ResourceTracker};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Child;
use std::sync::{Arc, Mutex};
use std::thread;
use tracing::{debug, error, info, warn};

const LEGACY_TORCH_REJECTION: &str =
    "Legacy Torch process control is unavailable on this platform; use owned Torch runtime-profile APIs";

fn legacy_torch_requires_owned_api() -> bool {
    !cfg!(target_os = "linux")
}

#[derive(Debug, Clone, Default)]
struct CachedProcessStatus {
    running: bool,
    generation: u64,
}

/// Process manager for inference runtimes.
#[derive(Clone)]
pub struct ProcessManager {
    /// Root directory (launcher root or app root).
    root_dir: PathBuf,
    /// Resource tracker.
    resource_tracker: Arc<ResourceTracker>,
    /// Last launch log path (exclusive access only).
    last_launch_log: Arc<Mutex<Option<PathBuf>>>,
    /// Last launch error message (exclusive access only).
    last_launch_error: Arc<Mutex<Option<String>>>,
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
        _version_paths: Option<HashMap<String, PathBuf>>,
    ) -> Result<Self> {
        let root_dir = root_dir.as_ref().to_path_buf();
        let ollama_status = CachedProcessStatus {
            running: Self::detect_ollama_running(&root_dir),
            generation: 0,
        };
        let torch_status = CachedProcessStatus {
            running: Self::detect_torch_running(&root_dir),
            generation: 0,
        };

        Ok(Self {
            root_dir: root_dir.clone(),
            resource_tracker: Arc::new(ResourceTracker::default()),
            last_launch_log: Arc::new(Mutex::new(None)),
            last_launch_error: Arc::new(Mutex::new(None)),
            ollama_status: Arc::new(Mutex::new(ollama_status)),
            torch_status: Arc::new(Mutex::new(torch_status)),
        })
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
        let mut result = match ProcessLauncher::launch_binary(&config) {
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
            let generation = self.set_ollama_status(true);
            if let Some(child) = result.process.take() {
                Self::observe_child_exit(self.ollama_status.clone(), "ollama", generation, child);
            }
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

    fn set_ollama_status(&self, running: bool) -> u64 {
        Self::set_cached_status(&self.ollama_status, running)
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
        if legacy_torch_requires_owned_api() {
            let message = LEGACY_TORCH_REJECTION.to_owned();
            *self.last_launch_error.lock().unwrap() = Some(message.clone());
            return LaunchResult {
                success: false,
                process: None,
                log_path: None,
                error: Some(message),
                ready: false,
            };
        }
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
        let mut result = match ProcessLauncher::launch_binary(&config) {
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
            let generation = self.set_torch_status(true);
            if let Some(child) = result.process.take() {
                Self::observe_child_exit(self.torch_status.clone(), "torch", generation, child);
            }
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
        if legacy_torch_requires_owned_api() {
            return Err(PumasError::LaunchFailed {
                app: "torch".to_owned(),
                message: LEGACY_TORCH_REJECTION.to_owned(),
            });
        }
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

    fn set_torch_status(&self, running: bool) -> u64 {
        Self::set_cached_status(&self.torch_status, running)
    }

    fn detect_torch_running(root_dir: &Path) -> bool {
        if legacy_torch_requires_owned_api() {
            return false;
        }
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

    fn set_cached_status(status: &Arc<Mutex<CachedProcessStatus>>, running: bool) -> u64 {
        let mut status = status.lock().unwrap();
        status.running = running;
        status.generation = status.generation.saturating_add(1);
        status.generation
    }

    fn observe_child_exit(
        status_cache: Arc<Mutex<CachedProcessStatus>>,
        label: &'static str,
        generation: u64,
        mut child: Child,
    ) {
        let pid = child.id();
        let thread_name = format!("pumas-{label}-wait");
        if let Err(error) = thread::Builder::new().name(thread_name).spawn(move || {
            match child.wait() {
                Ok(status) => info!("{label} process {pid} exited with {status}"),
                Err(error) => warn!("failed waiting for {label} process {pid}: {error}"),
            }

            let mut cached = status_cache.lock().unwrap();
            if cached.generation == generation {
                cached.running = false;
                cached.generation = cached.generation.saturating_add(1);
            }
        }) {
            warn!("failed to spawn {label} process wait observer: {error}");
        }
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

    #[cfg(unix)]
    use std::process::Command;
    #[cfg(unix)]
    use std::time::Duration;

    #[test]
    fn test_process_manager_creation() {
        let temp_dir = TempDir::new().unwrap();
        let manager = ProcessManager::new(temp_dir.path(), None).unwrap();
        assert!(manager.last_launch_log().is_none());
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

    #[cfg(target_os = "linux")]
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

    #[cfg(target_os = "linux")]
    #[test]
    fn linux_legacy_torch_path_remains_enabled() {
        assert!(!legacy_torch_requires_owned_api());
    }

    #[cfg(not(target_os = "linux"))]
    #[test]
    fn non_linux_legacy_torch_control_rejects_before_pid_or_process_actions() {
        assert!(legacy_torch_requires_owned_api());
        let temp_dir = TempDir::new().unwrap();
        let version_dir = temp_dir.path().join("torch-versions").join("test");
        fs::create_dir_all(&version_dir).unwrap();
        let pid_file = version_dir.join("torch.pid");
        fs::write(&pid_file, "4294967295").unwrap();
        let manager = ProcessManager::new(temp_dir.path(), None).unwrap();
        assert!(!manager.is_torch_running());
        assert!(!manager.refresh_torch_running());

        let launched = manager.launch_torch("test", &version_dir, None);
        assert!(!launched.success);
        assert!(launched.process.is_none());
        assert!(launched.log_path.is_none());
        assert!(!launched.ready);
        assert!(launched
            .error
            .as_deref()
            .unwrap()
            .contains("owned Torch runtime-profile APIs"));
        assert_eq!(manager.last_launch_error(), launched.error);

        let stopped = manager.stop_torch().unwrap_err();
        assert!(stopped
            .to_string()
            .contains("owned Torch runtime-profile APIs"));
        assert!(pid_file.exists());
    }

    #[cfg(unix)]
    #[test]
    fn child_exit_observer_clears_matching_liveness_generation() {
        let temp_dir = TempDir::new().unwrap();
        let manager = ProcessManager::new(temp_dir.path(), None).unwrap();
        let generation = manager.set_ollama_status(true);
        let child = Command::new("sh").arg("-c").arg("exit 0").spawn().unwrap();

        ProcessManager::observe_child_exit(
            manager.ollama_status.clone(),
            "ollama-test",
            generation,
            child,
        );

        let deadline = std::time::Instant::now() + Duration::from_secs(1);
        while manager.is_ollama_running() && std::time::Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(10));
        }

        assert!(!manager.is_ollama_running());
    }
}
