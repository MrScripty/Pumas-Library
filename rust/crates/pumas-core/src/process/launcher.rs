//! Process launching functionality.

use crate::error::{PumasError, Result};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};
use tracing::{debug, error, info, warn};

/// Configuration for launching a process.
#[derive(Debug, Clone)]
pub struct LaunchConfig {
    /// Version tag being launched.
    pub tag: String,
    /// Path to the version directory.
    pub version_dir: PathBuf,
    /// Path to the Python executable (in venv).
    pub python_path: PathBuf,
    /// Path to main.py.
    pub main_py: PathBuf,
    /// Additional arguments to pass.
    pub extra_args: Vec<String>,
    /// Environment variables to set.
    pub env_vars: HashMap<String, String>,
    /// Path to write the PID file.
    pub pid_file: PathBuf,
    /// Path to write stdout/stderr logs.
    pub log_file: Option<PathBuf>,
    /// Timeout for server readiness check.
    pub ready_timeout: Duration,
    /// URL to check for server readiness.
    pub health_check_url: Option<String>,
}

impl LaunchConfig {
    /// Create a new launch config with sensible defaults.
    pub fn new(tag: impl Into<String>, version_dir: impl AsRef<Path>) -> Self {
        let version_dir = version_dir.as_ref().to_path_buf();
        let venv_python = version_dir.join("venv").join("bin").join("python");
        let main_py = version_dir.join("main.py");
        let pid_file = version_dir.join("comfyui.pid");

        Self {
            tag: tag.into(),
            version_dir: version_dir.clone(),
            python_path: venv_python,
            main_py,
            extra_args: vec!["--enable-manager".to_string()],
            env_vars: HashMap::new(),
            pid_file,
            log_file: None,
            ready_timeout: Duration::from_secs(60),
            health_check_url: Some("http://127.0.0.1:8188".to_string()),
        }
    }

    /// Set extra arguments.
    pub fn with_extra_args(mut self, args: Vec<String>) -> Self {
        self.extra_args = args;
        self
    }

    /// Add an extra argument.
    pub fn with_arg(mut self, arg: impl Into<String>) -> Self {
        self.extra_args.push(arg.into());
        self
    }

    /// Set environment variables.
    pub fn with_env(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.env_vars.insert(key.into(), value.into());
        self
    }

    /// Set the log file path.
    pub fn with_log_file(mut self, path: impl AsRef<Path>) -> Self {
        self.log_file = Some(path.as_ref().to_path_buf());
        self
    }

    /// Set the ready timeout.
    pub fn with_ready_timeout(mut self, timeout: Duration) -> Self {
        self.ready_timeout = timeout;
        self
    }

    /// Set the health check URL.
    pub fn with_health_check_url(mut self, url: impl Into<String>) -> Self {
        self.health_check_url = Some(url.into());
        self
    }
}

/// Result of launching a process.
#[derive(Debug)]
pub struct LaunchResult {
    /// Whether the launch was successful.
    pub success: bool,
    /// The child process (if launched successfully).
    pub process: Option<Child>,
    /// Path to the log file.
    pub log_path: Option<PathBuf>,
    /// Error message (if failed).
    pub error: Option<String>,
    /// Whether the server is ready (passed health check).
    pub ready: bool,
}

/// Process launcher for managed applications.
pub struct ProcessLauncher;

impl ProcessLauncher {
    /// Launch a process with the given configuration.
    pub fn launch(config: &LaunchConfig) -> Result<LaunchResult> {
        // Validate prerequisites
        if !config.python_path.exists() {
            return Ok(LaunchResult {
                success: false,
                process: None,
                log_path: None,
                error: Some(format!(
                    "Python executable not found: {}",
                    config.python_path.display()
                )),
                ready: false,
            });
        }

        if !config.main_py.exists() {
            return Ok(LaunchResult {
                success: false,
                process: None,
                log_path: None,
                error: Some(format!(
                    "main.py not found: {}",
                    config.main_py.display()
                )),
                ready: false,
            });
        }

        // Build command
        let mut cmd = Command::new(&config.python_path);
        cmd.arg(&config.main_py);
        cmd.args(&config.extra_args);
        cmd.current_dir(&config.version_dir);

        // Set environment variables
        for (key, value) in &config.env_vars {
            cmd.env(key, value);
        }

        // Set up stdio
        let log_path = config.log_file.clone();
        if let Some(ref log_file) = log_path {
            // Ensure parent directory exists
            if let Some(parent) = log_file.parent() {
                fs::create_dir_all(parent).ok();
            }

            // Open log file for writing
            let file = fs::File::create(log_file).map_err(|e| PumasError::Io {
                message: "create log file".to_string(),
                path: Some(log_file.clone()),
                source: Some(e),
            })?;

            cmd.stdout(Stdio::from(file.try_clone().unwrap()));
            cmd.stderr(Stdio::from(file));
        } else {
            cmd.stdout(Stdio::null());
            cmd.stderr(Stdio::null());
        }

        // Spawn the process
        info!("Launching {} from {}", config.tag, config.version_dir.display());

        let child = match cmd.spawn() {
            Ok(c) => c,
            Err(e) => {
                error!("Failed to spawn process: {}", e);
                return Ok(LaunchResult {
                    success: false,
                    process: None,
                    log_path,
                    error: Some(format!("Failed to spawn process: {}", e)),
                    ready: false,
                });
            }
        };

        let pid = child.id();

        // Write PID file
        if let Err(e) = fs::write(&config.pid_file, pid.to_string()) {
            warn!("Failed to write PID file: {}", e);
        }

        info!("Launched process with PID {}", pid);

        // Check for readiness (if health check URL is configured)
        let ready = if let Some(ref url) = config.health_check_url {
            Self::wait_for_ready(url, config.ready_timeout)
        } else {
            true
        };

        Ok(LaunchResult {
            success: true,
            process: Some(child),
            log_path,
            error: None,
            ready,
        })
    }

    /// Wait for the server to become ready.
    fn wait_for_ready(url: &str, timeout: Duration) -> bool {
        let start = Instant::now();
        let check_interval = Duration::from_millis(500);

        info!("Waiting for server at {} to become ready...", url);

        while start.elapsed() < timeout {
            if Self::check_health(url) {
                info!("Server is ready");
                return true;
            }
            std::thread::sleep(check_interval);
        }

        warn!("Server did not become ready within {:?}", timeout);
        false
    }

    /// Check if the server is responding.
    fn check_health(url: &str) -> bool {
        // Simple TCP connect check
        if let Some(host_port) = url.strip_prefix("http://") {
            let addr = host_port.split('/').next().unwrap_or(host_port);
            match std::net::TcpStream::connect_timeout(
                &addr.parse().unwrap_or_else(|_| "127.0.0.1:8188".parse().unwrap()),
                Duration::from_secs(1),
            ) {
                Ok(_) => true,
                Err(_) => false,
            }
        } else {
            false
        }
    }

    /// Stop a process by PID.
    ///
    /// Sends SIGTERM first, then SIGKILL after a grace period.
    #[cfg(unix)]
    pub fn stop_process(pid: u32, grace_period: Duration) -> Result<bool> {
        use nix::sys::signal::{kill, Signal};
        use nix::unistd::Pid;

        let nix_pid = Pid::from_raw(pid as i32);

        // Send SIGTERM
        info!("Sending SIGTERM to process {}", pid);
        if let Err(e) = kill(nix_pid, Signal::SIGTERM) {
            if e == nix::errno::Errno::ESRCH {
                debug!("Process {} already exited", pid);
                return Ok(true);
            }
            warn!("Failed to send SIGTERM to {}: {}", pid, e);
        }

        // Wait for graceful shutdown
        std::thread::sleep(grace_period);

        // Check if still running
        if kill(nix_pid, None).is_ok() {
            // Still running - send SIGKILL
            info!("Process {} still running, sending SIGKILL", pid);
            if let Err(e) = kill(nix_pid, Signal::SIGKILL) {
                if e != nix::errno::Errno::ESRCH {
                    warn!("Failed to send SIGKILL to {}: {}", pid, e);
                }
            }
        }

        Ok(true)
    }

    #[cfg(windows)]
    pub fn stop_process(pid: u32, _grace_period: Duration) -> Result<bool> {
        // On Windows, use taskkill
        let output = Command::new("taskkill")
            .args(["/PID", &pid.to_string(), "/F"])
            .output();

        match output {
            Ok(o) if o.status.success() => Ok(true),
            Ok(o) => {
                let stderr = String::from_utf8_lossy(&o.stderr);
                warn!("taskkill failed: {}", stderr);
                Ok(false)
            }
            Err(e) => {
                warn!("Failed to run taskkill: {}", e);
                Ok(false)
            }
        }
    }

    /// Remove a PID file.
    pub fn remove_pid_file(pid_file: &Path) -> Result<()> {
        if pid_file.exists() {
            fs::remove_file(pid_file).map_err(|e| PumasError::Io {
                message: "remove PID file".to_string(),
                path: Some(pid_file.to_path_buf()),
                source: Some(e),
            })?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_launch_config_creation() {
        let temp_dir = TempDir::new().unwrap();
        let version_dir = temp_dir.path().join("v1.0.0");

        let config = LaunchConfig::new("v1.0.0", &version_dir);

        assert_eq!(config.tag, "v1.0.0");
        assert_eq!(config.version_dir, version_dir);
        assert!(config.extra_args.contains(&"--enable-manager".to_string()));
    }

    #[test]
    fn test_launch_config_builder() {
        let temp_dir = TempDir::new().unwrap();
        let version_dir = temp_dir.path().join("v1.0.0");
        let log_file = temp_dir.path().join("comfyui.log");

        let config = LaunchConfig::new("v1.0.0", &version_dir)
            .with_arg("--port=8189")
            .with_env("CUDA_VISIBLE_DEVICES", "0")
            .with_log_file(&log_file)
            .with_ready_timeout(Duration::from_secs(30));

        assert!(config.extra_args.contains(&"--port=8189".to_string()));
        assert_eq!(config.env_vars.get("CUDA_VISIBLE_DEVICES"), Some(&"0".to_string()));
        assert_eq!(config.log_file, Some(log_file));
        assert_eq!(config.ready_timeout, Duration::from_secs(30));
    }

    #[test]
    fn test_launch_missing_python() {
        let temp_dir = TempDir::new().unwrap();
        let version_dir = temp_dir.path().join("v1.0.0");
        fs::create_dir_all(&version_dir).unwrap();

        let config = LaunchConfig::new("v1.0.0", &version_dir);
        let result = ProcessLauncher::launch(&config).unwrap();

        assert!(!result.success);
        assert!(result.error.is_some());
        assert!(result.error.unwrap().contains("Python executable not found"));
    }
}
