//! High-level process management.

use super::launcher::{BinaryLaunchConfig, LaunchResult, ProcessLauncher};
use crate::error::{PumasError, Result};
use crate::platform::{
    self,
    process_identity::{self, IdentityCheck, PidRecord},
    RecordedStop,
};
use crate::system::{ProcessResources, ResourceTracker};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Child;
use std::sync::{Arc, Mutex};
use std::thread;
use tracing::{debug, error, info, warn};

const OLLAMA_VERSIONS_DIR: &str = "ollama-versions";
const OLLAMA_PID_FILE: &str = "ollama.pid";
const TORCH_VERSIONS_DIR: &str = "torch-versions";
const TORCH_PID_FILE: &str = "torch.pid";
const LEGACY_STOP_TIMEOUT_MS: u64 = 2000;

/// Consecutive observation failures after which an exit observer stops
/// retrying. The child stays in custody, so an explicit stop can still reap it.
#[cfg(target_os = "linux")]
const OBSERVER_FAILURE_LIMIT: u32 = 20;
#[cfg(target_os = "linux")]
const OBSERVER_INTERVAL: std::time::Duration = std::time::Duration::from_millis(100);
#[cfg(target_os = "linux")]
const OBSERVER_MAX_BACKOFF: std::time::Duration = std::time::Duration::from_secs(5);

/// A recorded process a legacy stop left alone, and why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeStopSkip {
    /// The recorded PID, when the PID file named one.
    pub pid: Option<u32>,
    /// The PID file that named the process.
    pub pid_file: PathBuf,
    /// Why the process was not stopped.
    pub reason: String,
}

/// The outcome of a legacy runtime stop.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RuntimeStopReport {
    /// PIDs that were running under Pumas custody and have stopped.
    pub stopped: Vec<u32>,
    /// Recorded processes left alone, each with a reason.
    pub skipped: Vec<RuntimeStopSkip>,
}

impl RuntimeStopReport {
    /// Whether any process stopped.
    pub fn stopped_any(&self) -> bool {
        !self.stopped.is_empty()
    }

    fn skip(&mut self, pid: Option<u32>, pid_file: &Path, reason: String) {
        warn!(
            "Legacy stop skipped {:?} (PID {:?}): {}",
            pid_file, pid, reason
        );
        self.skipped.push(RuntimeStopSkip {
            pid,
            pid_file: pid_file.to_path_buf(),
            reason,
        });
    }
}

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

#[cfg(target_os = "linux")]
struct ObservedChild {
    app: &'static str,
    child: Mutex<Option<Child>>,
}

#[cfg(target_os = "linux")]
impl ObservedChild {
    fn stop(&self, app: &str, timeout_ms: u64) -> Result<bool> {
        if self.app != app {
            return Ok(false);
        }
        let mut slot = self.child.lock().unwrap();
        let Some(child) = slot.as_mut() else {
            return Ok(true);
        };
        let stopped = ProcessLauncher::stop_owned_process(child, timeout_ms)?;
        if stopped {
            slot.take();
        }
        Ok(stopped)
    }

    fn observe_and_drain(&self) -> Result<bool> {
        let mut slot = self.child.lock().unwrap();
        let Some(child) = slot.as_mut() else {
            return Ok(true);
        };
        if crate::platform::linux_group::observe_exit(child.id())?.is_none() {
            return Ok(false);
        }
        let pid = nix::unistd::Pid::from_raw(i32::try_from(child.id()).unwrap());
        let group = nix::unistd::getpgid(Some(pid)).map_err(|error| {
            PumasError::Other(format!(
                "Identifying observed {} process group {pid}: {error}",
                self.app
            ))
        })?;
        let drained = if group == pid {
            ProcessLauncher::stop_owned_process(child, 2000)?
        } else {
            child.wait().map(|_| true)?
        };
        if drained {
            slot.take();
        }
        Ok(drained)
    }
}

/// Process manager for inference runtimes.
#[derive(Clone)]
pub struct ProcessManager {
    store_lifetime: crate::platform::store_lifetime::StoreLifetime,
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
    /// One actual Child/reaper owner shared by observation and stop. Completed
    /// slots prevent known managed PID files falling back to numeric signals.
    #[cfg(target_os = "linux")]
    owned_children: Arc<Mutex<HashMap<u32, Arc<ObservedChild>>>>,
}

impl ProcessManager {
    pub(crate) fn with_store_lifetime(
        mut self,
        lifetime: crate::platform::store_lifetime::StoreLifetime,
    ) -> Self {
        self.store_lifetime = lifetime;
        self
    }

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
            store_lifetime: Default::default(),
            root_dir: root_dir.clone(),
            resource_tracker: Arc::new(ResourceTracker::default()),
            last_launch_log: Arc::new(Mutex::new(None)),
            last_launch_error: Arc::new(Mutex::new(None)),
            ollama_status: Arc::new(Mutex::new(ollama_status)),
            torch_status: Arc::new(Mutex::new(torch_status)),
            #[cfg(target_os = "linux")]
            owned_children: Arc::new(Mutex::new(HashMap::new())),
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
                self.observe_child_exit(self.ollama_status.clone(), "ollama", generation, child);
            }
        } else if let Some(ref error) = result.error {
            let mut last_error = self.last_launch_error.lock().unwrap();
            *last_error = Some(error.clone());
        }

        result
    }

    /// Stop Ollama processes Pumas launched.
    ///
    /// Returns whether any process stopped. See [`Self::stop_ollama_report`]
    /// for what was skipped and why.
    pub fn stop_ollama(&self) -> Result<bool> {
        self.stop_ollama_report().map(|report| report.stopped_any())
    }

    /// Stop Ollama processes Pumas launched and report what was skipped.
    ///
    /// Only processes with proven custody are signalled: a child this manager
    /// still holds, or a PID record whose recorded identity matches the live
    /// process. Anything else (an Ollama the user started, a reused PID, or a
    /// PID file without an identity record) is left running and reported as
    /// skipped with a reason.
    pub fn stop_ollama_report(&self) -> Result<RuntimeStopReport> {
        let report = self.stop_recorded("ollama", OLLAMA_VERSIONS_DIR, OLLAMA_PID_FILE)?;
        self.refresh_ollama_running();
        info!(
            "stop_ollama completed: stopped={}, skipped={}",
            report.stopped.len(),
            report.skipped.len()
        );
        Ok(report)
    }

    /// Return cached Ollama liveness.
    ///
    /// This is intentionally a non-scanning read. Record verification happens
    /// only at startup or explicit refresh points.
    pub fn is_ollama_running(&self) -> bool {
        self.ollama_status.lock().unwrap().running
    }

    /// Explicitly refresh Ollama liveness from verified PID records.
    pub fn refresh_ollama_running(&self) -> bool {
        let running = Self::detect_ollama_running(&self.root_dir);
        self.set_ollama_status(running);
        running
    }

    fn set_ollama_status(&self, running: bool) -> u64 {
        Self::set_cached_status(&self.ollama_status, running)
    }

    /// Ollama counts as running only when a PID record proves a live process
    /// Pumas launched. An Ollama started outside Pumas is not ours to report.
    fn detect_ollama_running(root_dir: &Path) -> bool {
        !Self::live_recorded_pids(root_dir, OLLAMA_VERSIONS_DIR, OLLAMA_PID_FILE).is_empty()
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

        // The runtime supplies the interpreter; the sidecar is this Pumas's.
        // A runtime whose packages cannot run it is refused with the reasons.
        let entry = match crate::runtime_profiles::prepare_torch_sidecar_blocking(
            &self.root_dir,
            version_dir,
        ) {
            Ok(entry) => entry,
            Err(error) => {
                let message = error.to_string();
                error!("{}", message);
                *self.last_launch_error.lock().unwrap() = Some(message.clone());
                return LaunchResult {
                    success: false,
                    process: None,
                    log_path: None,
                    error: Some(message),
                    ready: false,
                };
            }
        };

        // Build launch config
        let mut config = BinaryLaunchConfig::torch(tag, version_dir, entry);
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
                self.observe_child_exit(self.torch_status.clone(), "torch", generation, child);
            }
        } else if let Some(ref error) = result.error {
            let mut last_error = self.last_launch_error.lock().unwrap();
            *last_error = Some(error.clone());
        }

        result
    }

    /// Stop Torch server processes Pumas launched.
    ///
    /// Returns whether any process stopped. See [`Self::stop_torch_report`].
    pub fn stop_torch(&self) -> Result<bool> {
        self.stop_torch_report().map(|report| report.stopped_any())
    }

    /// Stop Torch server processes Pumas launched and report what was skipped.
    ///
    /// Custody rules match [`Self::stop_ollama_report`]: an unrelated
    /// `serve.py` is never signalled. Runtime-profile Torch processes belong
    /// to the runtime-profile owner and are not touched by this legacy stop.
    pub fn stop_torch_report(&self) -> Result<RuntimeStopReport> {
        if legacy_torch_requires_owned_api() {
            return Err(PumasError::LaunchFailed {
                app: "torch".to_owned(),
                message: LEGACY_TORCH_REJECTION.to_owned(),
            });
        }
        let report = self.stop_recorded("torch", TORCH_VERSIONS_DIR, TORCH_PID_FILE)?;
        self.refresh_torch_running();
        info!(
            "stop_torch completed: stopped={}, skipped={}",
            report.stopped.len(),
            report.skipped.len()
        );
        Ok(report)
    }

    /// Return cached Torch liveness.
    pub fn is_torch_running(&self) -> bool {
        self.torch_status.lock().unwrap().running
    }

    /// Explicitly refresh Torch liveness from verified PID records.
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
        !Self::live_recorded_pids(root_dir, TORCH_VERSIONS_DIR, TORCH_PID_FILE).is_empty()
    }

    fn set_cached_status(status: &Arc<Mutex<CachedProcessStatus>>, running: bool) -> u64 {
        let mut status = status.lock().unwrap();
        status.running = running;
        status.generation = status.generation.saturating_add(1);
        status.generation
    }

    /// Every `<root>/<versions_dir>/*/<file_name>` that exists.
    fn recorded_pid_files(root_dir: &Path, versions_dir: &str, file_name: &str) -> Vec<PathBuf> {
        let Ok(entries) = fs::read_dir(root_dir.join(versions_dir)) else {
            return Vec::new();
        };
        entries
            .flatten()
            .map(|entry| entry.path().join(file_name))
            .filter(|pid_file| pid_file.exists())
            .collect()
    }

    /// PIDs whose recorded identity matches a live process.
    fn live_recorded_pids(root_dir: &Path, versions_dir: &str, file_name: &str) -> Vec<u32> {
        Self::recorded_pid_files(root_dir, versions_dir, file_name)
            .into_iter()
            .filter_map(|pid_file| match process_identity::read_record(&pid_file) {
                Ok(Some(PidRecord::Verified(identity)))
                    if process_identity::verify(&identity) == IdentityCheck::Alive =>
                {
                    Some(identity.pid)
                }
                _ => None,
            })
            .collect()
    }

    /// Stop every recorded process of one app that Pumas can prove it owns.
    fn stop_recorded(
        &self,
        app: &'static str,
        versions_dir: &str,
        file_name: &str,
    ) -> Result<RuntimeStopReport> {
        let mut report = RuntimeStopReport::default();
        for pid_file in Self::recorded_pid_files(&self.root_dir, versions_dir, file_name) {
            let record = match process_identity::read_record(&pid_file) {
                Ok(Some(record)) => record,
                Ok(None) => continue,
                Err(error) => {
                    report.skip(
                        None,
                        &pid_file,
                        format!("the PID record is unreadable: {error}"),
                    );
                    continue;
                }
            };
            self.stop_record(app, &pid_file, record, &mut report)?;
        }
        Ok(report)
    }

    fn stop_record(
        &self,
        app: &'static str,
        pid_file: &Path,
        record: PidRecord,
        report: &mut RuntimeStopReport,
    ) -> Result<()> {
        #[cfg(target_os = "linux")]
        if let Some(pid) = record.pid() {
            let owned = self.owned_children.lock().unwrap().get(&pid).cloned();
            if let Some(owned) = owned {
                // This manager holds the Child itself: the strongest custody.
                if owned.app != app {
                    report.skip(
                        Some(pid),
                        pid_file,
                        format!("process {pid} is a {} runtime, not {app}", owned.app),
                    );
                } else if owned.stop(app, LEGACY_STOP_TIMEOUT_MS)? {
                    report.stopped.push(pid);
                    Self::discard_record(pid_file);
                } else {
                    report.skip(
                        Some(pid),
                        pid_file,
                        format!(
                            "process {pid} did not finish stopping within \
                             {LEGACY_STOP_TIMEOUT_MS} ms; it remains in Pumas custody"
                        ),
                    );
                }
                return Ok(());
            }
        }
        match record {
            PidRecord::Verified(identity) => {
                let pid = identity.pid;
                info!("Stopping recorded {app} process {pid} from {:?}", pid_file);
                match platform::terminate_recorded_process(&identity, LEGACY_STOP_TIMEOUT_MS) {
                    Ok(RecordedStop::Stopped) => {
                        report.stopped.push(pid);
                        Self::discard_record(pid_file);
                    }
                    Ok(RecordedStop::AlreadyGone { reason }) => {
                        info!("Discarding stale {app} PID record {:?}: {reason}", pid_file);
                        Self::discard_record(pid_file);
                    }
                    Ok(RecordedStop::Refused { reason }) => {
                        report.skip(Some(pid), pid_file, reason);
                    }
                    Err(error) => {
                        report.skip(Some(pid), pid_file, error.to_string());
                    }
                }
            }
            PidRecord::Unverified { pid, reason } => {
                // Never signal a PID Pumas cannot prove it launched. The file
                // can never be proven later either, so discard it.
                Self::discard_record(pid_file);
                report.skip(
                    pid,
                    pid_file,
                    format!(
                        "skipped: not owned by Pumas ({reason}); the PID file was discarded \
                         and the process, if any, was left running"
                    ),
                );
            }
        }
        Ok(())
    }

    fn discard_record(pid_file: &Path) {
        if let Err(error) = ProcessLauncher::remove_pid_file(pid_file) {
            warn!("Failed to remove PID record {:?}: {}", pid_file, error);
        }
    }

    #[cfg(target_os = "linux")]
    #[cfg(test)]
    fn stop_known_process(&self, app: &str, pid: u32, timeout_ms: u64) -> Result<bool> {
        let owned = self.owned_children.lock().unwrap().get(&pid).cloned();
        match owned {
            Some(owned) => owned.stop(app, timeout_ms),
            None => Ok(false),
        }
    }

    fn observe_child_exit(
        &self,
        status_cache: Arc<Mutex<CachedProcessStatus>>,
        label: &'static str,
        generation: u64,
        child: Child,
    ) {
        let pid = child.id();
        #[cfg(target_os = "linux")]
        let owned = {
            let owned = Arc::new(ObservedChild {
                app: label,
                child: Mutex::new(Some(child)),
            });
            self.owned_children
                .lock()
                .unwrap()
                .insert(pid, owned.clone());
            owned
        };
        let thread_name = format!("pumas-{label}-wait");
        let store_lifetime = self.store_lifetime.clone();
        if let Err(error) = thread::Builder::new().name(thread_name).spawn(move || {
            let _store_lifetime = store_lifetime;
            #[cfg(target_os = "linux")]
            {
                let mut failures = 0u32;
                let mut delay = OBSERVER_INTERVAL;
                loop {
                    match owned.observe_and_drain() {
                        Ok(true) => break,
                        Ok(false) => {
                            failures = 0;
                            delay = OBSERVER_INTERVAL;
                        }
                        Err(error) => {
                            failures += 1;
                            if failures >= OBSERVER_FAILURE_LIMIT {
                                // Leave liveness and custody as they are: the
                                // exit is unobserved, not proven.
                                error!(
                                    "giving up observing {label} process {pid} after {failures} \
                                     consecutive failures: {error}; an explicit stop can still reap it"
                                );
                                return;
                            }
                            warn!("failed observing/draining {label} process {pid}: {error}");
                            delay = (delay * 2).min(OBSERVER_MAX_BACKOFF);
                        }
                    }
                    thread::sleep(delay);
                }
            }
            #[cfg(not(target_os = "linux"))]
            {
                let mut child = child;
                match child.wait() {
                    Ok(status) => info!("{label} process {pid} exited with {status}"),
                    Err(error) => warn!("failed waiting for {label} process {pid}: {error}"),
                }
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

    /// Aggregate resources for running Ollama processes Pumas launched.
    pub fn aggregate_ollama_resources(&self) -> Option<ProcessResources> {
        let mut total_cpu = 0.0f32;
        let mut total_ram = 0.0f32;
        let mut total_gpu = 0.0f32;
        let mut found_any = false;

        for pid in Self::live_recorded_pids(&self.root_dir, OLLAMA_VERSIONS_DIR, OLLAMA_PID_FILE) {
            match self.resource_tracker.get_process_resources(pid, true) {
                Ok(resources) => {
                    debug!(
                        "aggregate_ollama_resources: PID {} resources: cpu={}, ram={}, gpu={}",
                        pid, resources.cpu, resources.ram_memory, resources.gpu_memory
                    );
                    total_cpu += resources.cpu;
                    total_ram += resources.ram_memory;
                    total_gpu += resources.gpu_memory;
                    found_any = true;
                }
                Err(e) => {
                    warn!(
                        "aggregate_ollama_resources: failed to get resources for PID {}: {}",
                        pid, e
                    );
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
        assert!(!manager.is_ollama_running());
        let version_dir = temp_dir.path().join("ollama-versions").join("test");
        fs::create_dir_all(&version_dir).unwrap();
        let pid_file = version_dir.join("ollama.pid");
        // A bare PID proves nothing, even when that PID is alive.
        fs::write(&pid_file, std::process::id().to_string()).unwrap();
        assert!(!manager.refresh_ollama_running());
        process_identity::write_record(
            &pid_file,
            &process_identity::capture(std::process::id()).unwrap(),
        )
        .unwrap();

        assert!(!manager.is_ollama_running());

        assert!(manager.refresh_ollama_running());
        assert!(manager.is_ollama_running());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn torch_liveness_read_uses_cache_until_explicit_refresh() {
        let temp_dir = TempDir::new().unwrap();
        let manager = ProcessManager::new(temp_dir.path(), None).unwrap();
        assert!(!manager.is_torch_running());
        let version_dir = temp_dir.path().join("torch-versions").join("test");
        fs::create_dir_all(&version_dir).unwrap();
        let pid_file = version_dir.join("torch.pid");
        // A bare PID proves nothing, even when that PID is alive.
        fs::write(&pid_file, std::process::id().to_string()).unwrap();
        assert!(!manager.refresh_torch_running());
        process_identity::write_record(
            &pid_file,
            &process_identity::capture(std::process::id()).unwrap(),
        )
        .unwrap();

        assert!(!manager.is_torch_running());

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

        manager.observe_child_exit(
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

    #[cfg(all(target_os = "linux", not(target_env = "uclibc")))]
    struct ObserverFixture(Arc<ObservedChild>);

    #[cfg(all(target_os = "linux", not(target_env = "uclibc")))]
    impl Drop for ObserverFixture {
        fn drop(&mut self) {
            // Fixtures have no descendants. Retain actual Child custody for
            // cleanup even if an assertion fails before the observer reaps it.
            let mut slot = self.0.child.lock().unwrap();
            if let Some(mut child) = slot.take() {
                if crate::platform::linux_group::observe_exit(child.id()).is_ok() {
                    let _ = child.kill();
                    let _ = child.wait();
                }
            }
        }
    }

    #[cfg(all(target_os = "linux", not(target_env = "uclibc")))]
    #[test]
    fn already_exited_group_observer_reaps_child_and_clears_liveness() {
        use crate::platform::linux_group;
        use nix::sys::wait::{waitpid, WaitPidFlag};
        use nix::unistd::{getpgid, Pid};
        use std::os::unix::process::CommandExt;

        let temp = TempDir::new().unwrap();
        let manager = ProcessManager::new(temp.path(), None).unwrap();
        let child = Command::new("sh")
            .args(["-c", "exit 7"])
            .process_group(0)
            .spawn()
            .unwrap();
        let pid = child.id();
        let group = Pid::from_raw(i32::try_from(pid).unwrap());
        let mut fixture = ObserverFixture(Arc::new(ObservedChild {
            app: "ollama",
            child: Mutex::new(Some(child)),
        }));
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while linux_group::observe_exit(pid).unwrap().is_none() {
            assert!(
                std::time::Instant::now() < deadline,
                "fixture terminal status"
            );
            thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(
            linux_group::observe_exit(pid).unwrap().unwrap().code(),
            Some(7)
        );
        assert!(!linux_group::group_has_live_members(group.as_raw()).unwrap());
        // No live members does not mean the group identity vanished: the
        // waitable leader still pins it until the manager performs Child::wait.
        assert_eq!(getpgid(Some(group)).unwrap(), group);
        let generation = manager.set_ollama_status(true);
        let child = fixture.0.child.lock().unwrap().take().unwrap();
        manager.observe_child_exit(manager.ollama_status.clone(), "ollama", generation, child);
        fixture.0 = manager
            .owned_children
            .lock()
            .unwrap()
            .get(&pid)
            .unwrap()
            .clone();
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while manager.is_ollama_running() && std::time::Instant::now() < deadline {
            thread::sleep(Duration::from_millis(10));
        }
        assert!(!manager.is_ollama_running());
        assert_eq!(
            manager.ollama_status.lock().unwrap().generation,
            generation + 1
        );
        assert!(fixture.0.child.lock().unwrap().is_none());
        assert_eq!(
            waitpid(group, Some(WaitPidFlag::WNOHANG)),
            Err(nix::errno::Errno::ECHILD)
        );
        assert!(manager.stop_known_process("ollama", pid, 100).unwrap());
    }

    #[cfg(all(target_os = "linux", not(target_env = "uclibc")))]
    #[test]
    fn running_group_observer_retains_child_until_owned_stop_reaps_it() {
        use crate::platform::linux_group;
        use nix::sys::wait::{waitpid, WaitPidFlag};
        use nix::unistd::Pid;
        use std::os::unix::process::CommandExt;

        let child = Command::new("sleep")
            .arg("60")
            .process_group(0)
            .spawn()
            .unwrap();
        let pid = child.id();
        let group = Pid::from_raw(i32::try_from(pid).unwrap());
        let fixture = ObserverFixture(Arc::new(ObservedChild {
            app: "ollama",
            child: Mutex::new(Some(child)),
        }));
        assert!(!fixture.0.observe_and_drain().unwrap());
        assert!(fixture.0.child.lock().unwrap().is_some());
        assert!(!fixture.0.stop("torch", 100).unwrap());
        assert!(linux_group::group_has_live_members(group.as_raw()).unwrap());
        assert!(fixture.0.stop("ollama", 100).unwrap());
        assert!(fixture.0.child.lock().unwrap().is_none());
        assert_eq!(
            waitpid(group, Some(WaitPidFlag::WNOHANG)),
            Err(nix::errno::Errno::ECHILD)
        );
        assert!(fixture.0.stop("ollama", 100).unwrap());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn ollama_stop_and_real_exit_observer_share_child_custody() {
        assert_stop_with_exit_observer("ollama", false);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn torch_stop_and_real_exit_observer_share_child_custody() {
        assert_stop_with_exit_observer("torch", false);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn recorded_ollama_stop_uses_the_same_owned_child_slot() {
        assert_stop_with_exit_observer("ollama", true);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn recorded_torch_stop_uses_the_same_owned_child_slot() {
        assert_stop_with_exit_observer("torch", true);
    }

    #[cfg(target_os = "linux")]
    fn assert_stop_with_exit_observer(label: &'static str, recorded_stop: bool) {
        use crate::platform::linux_group;
        use std::os::unix::fs::PermissionsExt;

        struct ReleaseWorker {
            release: PathBuf,
            group: i32,
        }
        impl Drop for ReleaseWorker {
            fn drop(&mut self) {
                let _ = fs::write(&self.release, b"release");
                let until = std::time::Instant::now() + Duration::from_secs(3);
                while !linux_group::group_has_live_members(self.group).is_ok_and(|live| !live)
                    && std::time::Instant::now() < until
                {
                    std::thread::sleep(Duration::from_millis(10));
                }
            }
        }

        let temp = TempDir::new().unwrap();
        let manager = ProcessManager::new(temp.path(), None).unwrap();
        let versions = if label == "ollama" {
            OLLAMA_VERSIONS_DIR
        } else {
            TORCH_VERSIONS_DIR
        };
        let version_dir = temp.path().join(versions).join("owned-fixture");
        fs::create_dir_all(&version_dir).unwrap();
        let binary = version_dir.join("owned-fixture");
        fs::write(
            &binary,
            "#!/bin/bash\nbash -c 'trap \"\" TERM; echo $$ > worker.pid; for i in $(seq 1 300); do [ -f release ] && break; sleep .01; done' &\nwait\n",
        ).unwrap();
        fs::set_permissions(&binary, fs::Permissions::from_mode(0o700)).unwrap();
        let mut config = if label == "ollama" {
            BinaryLaunchConfig::ollama("owned-fixture", &version_dir)
        } else {
            BinaryLaunchConfig::torch("owned-fixture", &version_dir, "serve.py")
        };
        config.binary_path = binary;
        config.command = None;
        config.extra_args = Vec::new();
        config.health_check_url = None;
        let mut launched = ProcessLauncher::launch_binary(&config).unwrap();
        assert!(launched.success);
        assert!(matches!(
            process_identity::read_record(&config.pid_file).unwrap(),
            Some(PidRecord::Verified(_))
        ));
        let child = launched.process.take().unwrap();
        let group = i32::try_from(child.id()).unwrap();
        let release = ReleaseWorker {
            release: version_dir.join("release"),
            group,
        };
        let until = std::time::Instant::now() + Duration::from_secs(3);
        while !version_dir.join("worker.pid").exists() {
            assert!(
                std::time::Instant::now() < until,
                "fixture worker readiness"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
        let status = if label == "ollama" {
            manager.ollama_status.clone()
        } else {
            manager.torch_status.clone()
        };
        let generation = ProcessManager::set_cached_status(&status, true);
        manager.observe_child_exit(status, label, generation, child);
        let pid = u32::try_from(group).unwrap();
        let other_app = if label == "ollama" { "torch" } else { "ollama" };
        assert!(!manager.stop_known_process(other_app, pid, 200).unwrap());
        assert!(linux_group::group_has_live_members(group).unwrap());
        let stopping = manager.clone();
        let stopped = if recorded_stop {
            let report = if label == "ollama" {
                stopping.stop_ollama_report()
            } else {
                stopping.stop_torch_report()
            };
            report.map(|report| report.stopped == vec![pid] && report.skipped.is_empty())
        } else {
            stopping.stop_known_process(label, pid, 200)
        };
        let live_before_release = linux_group::group_has_live_members(group).unwrap();
        // Only our cooperative worker sees this private release file. Never
        // signal a numeric PGID after the old observer may have reaped its pin.
        drop(release);
        assert!(
            stopped.unwrap(),
            "managed stop must retain exclusive child custody"
        );
        assert!(
            !live_before_release,
            "managed stop must drain the TERM-ignoring worker"
        );
        if recorded_stop {
            assert!(!config.pid_file.exists());
            assert!(!process_identity::identity_path(&config.pid_file).exists());
        }
        // The completed slot remains recognizable without signalling a stale
        // numeric PID after the observer or stopper performed the sole reap.
        assert!(stopping.stop_known_process(label, pid, 200).unwrap());
    }

    /// Start an unrelated process whose command line looks like a runtime.
    #[cfg(unix)]
    fn impostor(argv0: &str) -> std::process::Child {
        Command::new("bash")
            .args(["-c", &format!("exec -a '{argv0}' sleep 30")])
            .spawn()
            .unwrap()
    }

    #[cfg(unix)]
    fn wait_for_cmdline(child: &std::process::Child, needle: &str) {
        #[cfg(target_os = "linux")]
        {
            let path = format!("/proc/{}/cmdline", child.id());
            let until = std::time::Instant::now() + Duration::from_secs(5);
            while !fs::read(&path)
                .map(|bytes| String::from_utf8_lossy(&bytes).contains(needle))
                .unwrap_or(false)
            {
                assert!(std::time::Instant::now() < until, "impostor exec");
                std::thread::sleep(Duration::from_millis(10));
            }
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = (child, needle);
            std::thread::sleep(Duration::from_millis(200));
        }
    }

    /// Stop with records that never prove custody of `impostor`, and assert
    /// it survives every one of them.
    #[cfg(unix)]
    fn assert_impostor_survives(
        versions_dir: &str,
        pid_file_name: &str,
        argv0: &str,
        stop: impl Fn(&ProcessManager) -> RuntimeStopReport,
    ) {
        let temp = TempDir::new().unwrap();
        let manager = ProcessManager::new(temp.path(), None).unwrap();
        let mut unrelated = impostor(argv0);
        wait_for_cmdline(&unrelated, argv0.split(' ').next().unwrap());
        let live = process_identity::capture(unrelated.id()).unwrap();
        let alive = |child: &mut std::process::Child| child.try_wait().unwrap().is_none();

        // No record at all: the old substring sweep would have killed it.
        let report = stop(&manager);
        assert!(report.stopped.is_empty() && report.skipped.is_empty());
        assert!(alive(&mut unrelated), "no record must mean no signal");

        let version_dir = temp.path().join(versions_dir).join("v1");
        fs::create_dir_all(&version_dir).unwrap();
        let pid_file = version_dir.join(pid_file_name);

        // A bare PID (older Pumas, or a stale file) is unverified.
        fs::write(&pid_file, unrelated.id().to_string()).unwrap();
        let report = stop(&manager);
        assert!(report.stopped.is_empty());
        assert_eq!(report.skipped.len(), 1);
        assert!(report.skipped[0].reason.contains("not owned by Pumas"));
        assert!(
            alive(&mut unrelated),
            "an unverified PID must not be signalled"
        );
        assert!(!pid_file.exists(), "the unverifiable PID file is discarded");

        // A record whose PID was reused by the unrelated process.
        let reused = process_identity::ProcessIdentity {
            start_time: live.start_time.wrapping_sub(1),
            ..live.clone()
        };
        process_identity::write_record(&pid_file, &reused).unwrap();
        let report = stop(&manager);
        assert!(report.stopped.is_empty() && report.skipped.is_empty());
        assert!(alive(&mut unrelated), "a reused PID must not be signalled");
        assert!(!pid_file.exists(), "a stale record is discarded");

        // A record that cannot be proven either way is refused and kept.
        let other_runtime = process_identity::ProcessIdentity {
            executable: PathBuf::from("/nonexistent/runtime"),
            ..live
        };
        process_identity::write_record(&pid_file, &other_runtime).unwrap();
        let report = stop(&manager);
        assert!(report.stopped.is_empty());
        assert_eq!(report.skipped.len(), 1);
        assert!(
            alive(&mut unrelated),
            "an unproven PID must not be signalled"
        );
        assert!(pid_file.exists());

        unrelated.kill().unwrap();
        unrelated.wait().unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn unrelated_ollama_serve_survives_legacy_stop() {
        assert_impostor_survives(
            OLLAMA_VERSIONS_DIR,
            OLLAMA_PID_FILE,
            "ollama serve",
            |manager| manager.stop_ollama_report().unwrap(),
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn unrelated_serve_py_survives_legacy_torch_stop() {
        assert_impostor_survives(
            TORCH_VERSIONS_DIR,
            TORCH_PID_FILE,
            "python serve.py",
            |manager| manager.stop_torch_report().unwrap(),
        );
    }

    /// A runtime launched by an earlier Pumas process is stopped through its
    /// recorded identity alone, without any in-memory child custody.
    #[cfg(unix)]
    #[test]
    fn recorded_runtime_from_a_previous_session_still_stops() {
        let temp = TempDir::new().unwrap();
        let version_dir = temp.path().join(OLLAMA_VERSIONS_DIR).join("v1");
        fs::create_dir_all(&version_dir).unwrap();
        let pid_file = version_dir.join(OLLAMA_PID_FILE);
        let mut command = Command::new("sleep");
        command.arg("30");
        platform::configure_detached_command(&mut command);
        let mut runtime = command.spawn().unwrap();
        process_identity::write_record(
            &pid_file,
            &process_identity::capture(runtime.id()).unwrap(),
        )
        .unwrap();

        let manager = ProcessManager::new(temp.path(), None).unwrap();
        assert!(manager.is_ollama_running());
        let report = manager.stop_ollama_report().unwrap();
        let exited = runtime.try_wait().unwrap().is_some() || {
            std::thread::sleep(Duration::from_millis(200));
            runtime.try_wait().unwrap().is_some()
        };
        if !exited {
            let _ = runtime.kill();
        }
        let _ = runtime.wait();
        assert_eq!(report.stopped, vec![runtime.id()]);
        assert!(report.skipped.is_empty());
        assert!(exited, "the recorded runtime must stop");
        assert!(!pid_file.exists());
        assert!(!manager.is_ollama_running());
    }
}
