//! Version launching with health checks.
//!
//! Handles launching ComfyUI instances and detecting when they're ready.

use crate::config::{AppId, InstallationConfig};
use crate::version_manager::LaunchResult;
use crate::{PumasError, Result};
use chrono::Utc;
use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;
use tokio::process::{Child, Command};
use tracing::{debug, error, info, warn};

/// Handles launching version instances.
pub struct VersionLauncher {
    /// Root directory for launcher.
    launcher_root: PathBuf,
    /// Application ID.
    app_id: AppId,
    /// Logs directory.
    logs_dir: PathBuf,
}

impl VersionLauncher {
    /// Create a new version launcher.
    pub fn new(launcher_root: PathBuf, app_id: AppId, logs_dir: PathBuf) -> Self {
        Self {
            launcher_root,
            app_id,
            logs_dir,
        }
    }

    /// Get the version directory path.
    fn version_path(&self, tag: &str) -> PathBuf {
        self.launcher_root
            .join(self.app_id.versions_dir_name())
            .join(tag)
    }

    /// Get the venv python path for a version.
    fn venv_python(&self, tag: &str) -> PathBuf {
        self.version_path(tag).join("venv").join("bin").join("python")
    }

    /// Launch a version.
    pub async fn launch_version(
        &self,
        tag: &str,
        extra_args: Option<Vec<String>>,
    ) -> Result<LaunchResult> {
        let version_path = self.version_path(tag);
        if !version_path.exists() {
            return Err(PumasError::VersionNotFound {
                tag: tag.to_string(),
            });
        }

        let venv_python = self.venv_python(tag);
        if !venv_python.exists() {
            return Ok(LaunchResult {
                success: false,
                log_file: None,
                error: Some("Virtual environment not found".to_string()),
                ready: None,
            });
        }

        // Create log file
        std::fs::create_dir_all(&self.logs_dir).ok();
        let log_file = self.logs_dir.join(format!(
            "launch-{}-{}.log",
            self.slugify_tag(tag),
            Utc::now().format("%Y%m%d-%H%M%S")
        ));

        info!("Launching {} from {}", tag, version_path.display());

        match self.app_id {
            AppId::ComfyUI => self.launch_comfyui(tag, &version_path, &log_file, extra_args).await,
            AppId::Ollama => self.launch_ollama(tag, &version_path, &log_file, extra_args).await,
            _ => Err(PumasError::Other(format!(
                "Launch not implemented for {:?}",
                self.app_id
            ))),
        }
    }

    /// Launch ComfyUI.
    async fn launch_comfyui(
        &self,
        tag: &str,
        version_path: &PathBuf,
        log_file: &PathBuf,
        extra_args: Option<Vec<String>>,
    ) -> Result<LaunchResult> {
        let venv_python = self.venv_python(tag);
        let main_py = version_path.join("main.py");

        if !main_py.exists() {
            return Ok(LaunchResult {
                success: false,
                log_file: Some(log_file.clone()),
                error: Some("main.py not found".to_string()),
                ready: None,
            });
        }

        // Build command
        let mut args = vec!["main.py".to_string(), "--enable-manager".to_string()];
        if let Some(extra) = extra_args {
            args.extend(extra);
        }

        // Create log file handle
        let log_output = std::fs::File::create(log_file).map_err(|e| PumasError::Io {
            message: format!("Failed to create log file: {}", e),
            path: Some(log_file.clone()),
            source: Some(e),
        })?;

        // Spawn process
        let child = Command::new(&venv_python)
            .args(&args)
            .current_dir(version_path)
            .env("SKIP_BROWSER", "1")
            .stdout(Stdio::from(log_output.try_clone().map_err(|e| PumasError::Io {
                message: format!("Failed to clone log handle: {}", e),
                path: Some(log_file.clone()),
                source: Some(e),
            })?))
            .stderr(Stdio::from(log_output))
            // Start in new process group for clean termination
            .process_group(0)
            .spawn()
            .map_err(|e| PumasError::Other(format!("Failed to spawn ComfyUI: {}", e)))?;

        let pid = child.id();
        info!("ComfyUI started with PID {:?}", pid);

        // Write PID file
        if let Some(pid) = pid {
            let pid_file = version_path.join("comfyui.pid");
            std::fs::write(&pid_file, pid.to_string()).ok();
        }

        // Wait for server to be ready
        let server_url = "http://127.0.0.1:8188";
        let (ready, error) = self.wait_for_server_ready(server_url, child, 90).await;

        Ok(LaunchResult {
            success: ready,
            log_file: Some(log_file.clone()),
            error,
            ready: Some(ready),
        })
    }

    /// Launch Ollama.
    async fn launch_ollama(
        &self,
        _tag: &str,
        version_path: &PathBuf,
        log_file: &PathBuf,
        extra_args: Option<Vec<String>>,
    ) -> Result<LaunchResult> {
        let ollama_bin = version_path.join("ollama");

        if !ollama_bin.exists() {
            return Ok(LaunchResult {
                success: false,
                log_file: Some(log_file.clone()),
                error: Some("ollama binary not found".to_string()),
                ready: None,
            });
        }

        // Build command
        let mut args = vec!["serve".to_string()];
        if let Some(extra) = extra_args {
            args.extend(extra);
        }

        // Create log file handle
        let log_output = std::fs::File::create(log_file).map_err(|e| PumasError::Io {
            message: format!("Failed to create log file: {}", e),
            path: Some(log_file.clone()),
            source: Some(e),
        })?;

        // Spawn process
        let child = Command::new(&ollama_bin)
            .args(&args)
            .current_dir(version_path)
            .stdout(Stdio::from(log_output.try_clone().map_err(|e| PumasError::Io {
                message: format!("Failed to clone log handle: {}", e),
                path: Some(log_file.clone()),
                source: Some(e),
            })?))
            .stderr(Stdio::from(log_output))
            .process_group(0)
            .spawn()
            .map_err(|e| PumasError::Other(format!("Failed to spawn Ollama: {}", e)))?;

        let pid = child.id();
        info!("Ollama started with PID {:?}", pid);

        // Wait for server to be ready
        let server_url = "http://127.0.0.1:11434";
        let (ready, error) = self.wait_for_server_ready(server_url, child, 30).await;

        Ok(LaunchResult {
            success: ready,
            log_file: Some(log_file.clone()),
            error,
            ready: Some(ready),
        })
    }

    /// Wait for a server to become ready.
    async fn wait_for_server_ready(
        &self,
        url: &str,
        mut child: Child,
        timeout_secs: u64,
    ) -> (bool, Option<String>) {
        let client = reqwest::Client::builder()
            .timeout(InstallationConfig::URL_QUICK_CHECK_TIMEOUT)
            .build()
            .ok();

        let start = std::time::Instant::now();
        let timeout = Duration::from_secs(timeout_secs);
        let mut delay = Duration::from_millis(500);
        let max_delay = Duration::from_secs(5);

        while start.elapsed() < timeout {
            // Check if process is still alive
            match child.try_wait() {
                Ok(Some(status)) => {
                    error!("Process exited with status {}", status);
                    return (false, Some(format!("Process exited with status {}", status)));
                }
                Ok(None) => {
                    // Still running, continue
                }
                Err(e) => {
                    error!("Failed to check process status: {}", e);
                    return (false, Some(format!("Failed to check process: {}", e)));
                }
            }

            // Try to connect
            if let Some(ref client) = client {
                match client.get(url).send().await {
                    Ok(response) if response.status().is_success() => {
                        info!("Server ready at {} after {:?}", url, start.elapsed());
                        return (true, None);
                    }
                    Ok(response) => {
                        debug!("Server returned {}, still starting...", response.status());
                    }
                    Err(e) => {
                        debug!("Connection attempt failed: {}", e);
                    }
                }
            }

            // Wait with exponential backoff
            tokio::time::sleep(delay).await;
            delay = (delay * 2).min(max_delay);
        }

        warn!("Server did not become ready within {} seconds", timeout_secs);
        (false, Some(format!("Server did not become ready within {} seconds", timeout_secs)))
    }

    /// Stop a running version.
    pub async fn stop_version(&self, tag: &str) -> Result<bool> {
        let version_path = self.version_path(tag);
        let pid_file = version_path.join("comfyui.pid");

        if !pid_file.exists() {
            return Ok(false);
        }

        let pid_str = std::fs::read_to_string(&pid_file).map_err(|e| PumasError::Io {
            message: format!("Failed to read PID file: {}", e),
            path: Some(pid_file.clone()),
            source: Some(e),
        })?;

        let pid: i32 = pid_str.trim().parse().map_err(|_| PumasError::Other(
            format!("Invalid PID in file: {}", pid_str)
        ))?;

        info!("Stopping process with PID {}", pid);

        // Send SIGTERM
        #[cfg(unix)]
        {
            use nix::sys::signal::{kill, Signal};
            use nix::unistd::Pid;

            // Try to kill the process group
            let pgid = Pid::from_raw(-pid); // Negative PID = process group
            match kill(pgid, Signal::SIGTERM) {
                Ok(_) => {
                    debug!("Sent SIGTERM to process group {}", pid);
                }
                Err(_) => {
                    // Try individual process
                    let process_pid = Pid::from_raw(pid);
                    if let Err(e) = kill(process_pid, Signal::SIGTERM) {
                        warn!("Failed to send SIGTERM: {}", e);
                    }
                }
            }

            // Wait a bit
            tokio::time::sleep(Duration::from_millis(500)).await;

            // Send SIGKILL if still running
            let process_pid = Pid::from_raw(pid);
            if let Ok(()) = kill(process_pid, Signal::SIGKILL) {
                debug!("Sent SIGKILL to process {}", pid);
            }
        }

        // Remove PID file
        std::fs::remove_file(&pid_file).ok();

        info!("Process {} stopped", pid);
        Ok(true)
    }

    /// Check if a version is running.
    pub async fn is_version_running(&self, tag: &str) -> bool {
        let version_path = self.version_path(tag);
        let pid_file = version_path.join("comfyui.pid");

        if !pid_file.exists() {
            return false;
        }

        if let Ok(pid_str) = std::fs::read_to_string(&pid_file) {
            if let Ok(pid) = pid_str.trim().parse::<i32>() {
                #[cfg(unix)]
                {
                    use nix::sys::signal::kill;
                    use nix::unistd::Pid;

                    // Check if process exists by sending signal 0 (None = signal 0)
                    let process_pid = Pid::from_raw(pid);
                    return kill(process_pid, None).is_ok();
                }

                #[cfg(not(unix))]
                {
                    // On non-Unix, just assume it's running if PID file exists
                    return true;
                }
            }
        }

        false
    }

    /// Generate a run script for a version.
    pub fn generate_run_script(&self, tag: &str) -> Result<PathBuf> {
        let version_path = self.version_path(tag);
        let slug = self.slugify_tag(tag);
        let script_path = version_path.join(format!("run_{}.sh", slug));

        let venv_python = self.venv_python(tag);
        let profiles_dir = self.launcher_root.join("launcher-data").join("profiles").join(&slug);

        let script_content = format!(
            r#"#!/bin/bash
# Run script for {tag}
# Generated by Pumas Library

SCRIPT_DIR="$(cd "$(dirname "${{BASH_SOURCE[0]}}")" && pwd)"
VENV_PYTHON="{venv_python}"
PID_FILE="$SCRIPT_DIR/comfyui.pid"
SERVER_URL="http://127.0.0.1:8188"
PROFILE_DIR="{profiles_dir}"

# Stop any existing instance
if [ -f "$PID_FILE" ]; then
    OLD_PID=$(cat "$PID_FILE")
    if kill -0 "$OLD_PID" 2>/dev/null; then
        echo "Stopping existing instance (PID: $OLD_PID)..."
        kill -TERM "$OLD_PID" 2>/dev/null
        sleep 1
        kill -KILL "$OLD_PID" 2>/dev/null || true
    fi
    rm -f "$PID_FILE"
fi

# Close existing browser window if wmctrl is available
if command -v wmctrl &> /dev/null; then
    wmctrl -c "ComfyUI" 2>/dev/null || true
fi

# Start ComfyUI
cd "$SCRIPT_DIR"
export SKIP_BROWSER=1
"$VENV_PYTHON" main.py --enable-manager "$@" &
echo $! > "$PID_FILE"

# Wait for server and open browser
echo "Waiting for server to start..."
for i in {{1..30}}; do
    if curl -s "$SERVER_URL" > /dev/null 2>&1; then
        echo "Server ready!"

        # Open in browser
        if command -v brave-browser &> /dev/null; then
            brave-browser --app="$SERVER_URL" --new-window --user-data-dir="$PROFILE_DIR" &
        elif command -v xdg-open &> /dev/null; then
            xdg-open "$SERVER_URL" &
        fi

        exit 0
    fi
    sleep 1
done

echo "Warning: Server did not become ready in 30 seconds"
"#,
            tag = tag,
            venv_python = venv_python.display(),
            profiles_dir = profiles_dir.display(),
        );

        std::fs::write(&script_path, script_content).map_err(|e| PumasError::Io {
            message: format!("Failed to write run script: {}", e),
            path: Some(script_path.clone()),
            source: Some(e),
        })?;

        // Make executable
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(&script_path)
                .map_err(|e| PumasError::Io {
                    message: format!("Failed to get script permissions: {}", e),
                    path: Some(script_path.clone()),
                    source: Some(e),
                })?
                .permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(&script_path, perms).map_err(|e| PumasError::Io {
                message: format!("Failed to set script permissions: {}", e),
                path: Some(script_path.clone()),
                source: Some(e),
            })?;
        }

        info!("Generated run script: {}", script_path.display());
        Ok(script_path)
    }

    /// Tail the last N lines from a log file.
    pub fn tail_log(&self, log_file: &PathBuf, lines: usize) -> Result<Vec<String>> {
        if !log_file.exists() {
            return Ok(vec![]);
        }

        let content = std::fs::read_to_string(log_file).map_err(|e| PumasError::Io {
            message: format!("Failed to read log file: {}", e),
            path: Some(log_file.clone()),
            source: Some(e),
        })?;

        let all_lines: Vec<_> = content.lines().map(String::from).collect();
        let start = all_lines.len().saturating_sub(lines);
        Ok(all_lines[start..].to_vec())
    }

    /// Create a slug from a tag.
    fn slugify_tag(&self, tag: &str) -> String {
        tag.chars()
            .filter(|c| c.is_alphanumeric() || *c == '-' || *c == '_')
            .collect::<String>()
            .to_lowercase()
            .trim_start_matches('v')
            .to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_test_launcher() -> (VersionLauncher, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let launcher = VersionLauncher::new(
            temp_dir.path().to_path_buf(),
            AppId::ComfyUI,
            temp_dir.path().join("logs"),
        );
        (launcher, temp_dir)
    }

    #[test]
    fn test_slugify_tag() {
        let (launcher, _temp) = create_test_launcher();

        assert_eq!(launcher.slugify_tag("v1.0.0"), "100");
        assert_eq!(launcher.slugify_tag("v1.0.0-beta"), "100-beta");
        assert_eq!(launcher.slugify_tag("1.0.0"), "100");
    }

    #[test]
    fn test_version_path() {
        let (launcher, temp) = create_test_launcher();

        let path = launcher.version_path("v1.0.0");
        assert_eq!(path, temp.path().join("comfyui-versions/v1.0.0"));
    }

    #[tokio::test]
    async fn test_is_version_running_no_pid_file() {
        let (launcher, temp) = create_test_launcher();

        // Create version directory but no PID file
        std::fs::create_dir_all(temp.path().join("comfyui-versions/v1.0.0")).unwrap();

        assert!(!launcher.is_version_running("v1.0.0").await);
    }
}
