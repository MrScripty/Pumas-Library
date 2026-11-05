//! Version launching with health checks.
//!
//! Handles launching managed inference runtime versions and readiness checks.

use crate::version_manager::LaunchResult;
use chrono::Utc;
use pumas_library::config::{AppId, InstallationConfig};
use pumas_library::{PumasError, Result};
use std::path::Path;
use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;
use tokio::fs;
use tokio::process::{Child, Command};
use tracing::{debug, error, info, warn};

async fn path_exists(path: &Path) -> Result<bool> {
    fs::try_exists(path)
        .await
        .map_err(|err| PumasError::io_with_path(err, path))
}

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
    fn version_path(&self, tag: &str) -> Result<PathBuf> {
        super::validate_version_tag(tag)?;
        Ok(self
            .launcher_root
            .join(self.app_id.versions_dir_name())
            .join(tag))
    }

    /// Launch a version.
    pub async fn launch_version(
        &self,
        tag: &str,
        extra_args: Option<Vec<String>>,
    ) -> Result<LaunchResult> {
        let version_path = self.version_path(tag)?;
        if !path_exists(&version_path).await? {
            return Err(PumasError::VersionNotFound {
                tag: tag.to_string(),
            });
        }

        // Create log file
        fs::create_dir_all(&self.logs_dir).await.ok();
        let log_file = self.logs_dir.join(format!(
            "launch-{}-{}.log",
            self.slugify_tag(tag),
            Utc::now().format("%Y%m%d-%H%M%S")
        ));

        info!("Launching {} from {}", tag, version_path.display());

        match self.app_id {
            AppId::Ollama => {
                self.launch_ollama(tag, &version_path, &log_file, extra_args)
                    .await
            }
            _ => Err(PumasError::Other(format!(
                "Launch not implemented for {:?}",
                self.app_id
            ))),
        }
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

        if !path_exists(&ollama_bin).await? {
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
        let log_output = fs::File::create(log_file)
            .await
            .map_err(|e| PumasError::Io {
                message: format!("Failed to create log file: {}", e),
                path: Some(log_file.clone()),
                source: Some(e),
            })?
            .into_std()
            .await;

        // Spawn process
        let mut cmd = Command::new(&ollama_bin);
        cmd.args(&args)
            .current_dir(version_path)
            .stdout(Stdio::from(log_output.try_clone().map_err(|e| {
                PumasError::Io {
                    message: format!("Failed to clone log handle: {}", e),
                    path: Some(log_file.clone()),
                    source: Some(e),
                }
            })?))
            .stderr(Stdio::from(log_output));
        // Unix: start in new process group for clean termination
        #[cfg(unix)]
        cmd.process_group(0);
        let child = cmd
            .spawn()
            .map_err(|e| PumasError::Other(format!("Failed to spawn Ollama: {}", e)))?;

        let pid = child.id();
        info!("Ollama started with PID {:?}", pid);

        // Wait for server to be ready
        let server_url = AppId::Ollama.default_base_url();
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
                    return (
                        false,
                        Some(format!("Process exited with status {}", status)),
                    );
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

        warn!(
            "Server did not become ready within {} seconds",
            timeout_secs
        );
        (
            false,
            Some(format!(
                "Server did not become ready within {} seconds",
                timeout_secs
            )),
        )
    }

    /// Stop a running version.
    ///
    /// Only a process whose recorded identity (see
    /// [`process_identity`](pumas_library::platform::process_identity))
    /// matches the live process is signalled. A bare PID file without an
    /// identity record is never signalled: it is discarded and the refusal
    /// is returned with its reason. Returns `Ok(false)` when no recorded
    /// process was running.
    pub async fn stop_version(&self, tag: &str) -> Result<bool> {
        let pid_file = self.version_path(tag)?.join("ollama.pid");
        tokio::task::spawn_blocking(move || stop_recorded_version(&pid_file))
            .await
            .map_err(|error| PumasError::Other(format!("Stop task failed: {error}")))?
    }

    /// Check if a version is running.
    ///
    /// True only when the PID file's recorded identity matches a live
    /// process; a bare or reused PID does not count as running.
    pub async fn is_version_running(&self, tag: &str) -> bool {
        let Ok(version_path) = self.version_path(tag) else {
            return false;
        };
        let pid_file = version_path.join("ollama.pid");
        tokio::task::spawn_blocking(move || recorded_process_alive(&pid_file))
            .await
            .unwrap_or(false)
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

/// Grace period between SIGTERM and SIGKILL for a recorded runtime.
const RECORDED_STOP_TIMEOUT_MS: u64 = 5_000;

fn recorded_process_alive(pid_file: &Path) -> bool {
    use pumas_library::platform::process_identity::{
        read_record, verify, IdentityCheck, PidRecord,
    };
    matches!(
        read_record(pid_file),
        Ok(Some(PidRecord::Verified(identity))) if verify(&identity) == IdentityCheck::Alive
    )
}

fn discard_pid_record(pid_file: &Path) {
    if let Err(error) = pumas_library::platform::process_identity::remove_record(pid_file) {
        warn!("Failed to remove PID record {:?}: {}", pid_file, error);
    }
}

fn stop_recorded_version(pid_file: &Path) -> Result<bool> {
    use pumas_library::platform::process_identity::{read_record, PidRecord};
    use pumas_library::platform::{terminate_recorded_process, RecordedStop};

    let record =
        read_record(pid_file).map_err(|error| PumasError::io_with_path(error, pid_file))?;
    match record {
        None => Ok(false),
        Some(PidRecord::Verified(identity)) => {
            info!(
                "Stopping recorded process {} from {:?}",
                identity.pid, pid_file
            );
            match terminate_recorded_process(&identity, RECORDED_STOP_TIMEOUT_MS)? {
                RecordedStop::Stopped => {
                    discard_pid_record(pid_file);
                    info!("Process {} stopped", identity.pid);
                    Ok(true)
                }
                RecordedStop::AlreadyGone { reason } => {
                    info!("Discarding stale PID record {:?}: {reason}", pid_file);
                    discard_pid_record(pid_file);
                    Ok(false)
                }
                RecordedStop::Refused { reason } => Err(PumasError::Other(format!(
                    "Refused to stop process {}: {reason}",
                    identity.pid
                ))),
            }
        }
        Some(PidRecord::Unverified { pid, reason }) => {
            // Never signal a PID Pumas cannot prove it launched. The file can
            // never be proven later either, so discard it.
            discard_pid_record(pid_file);
            let pid = pid.map_or_else(|| "unknown".to_owned(), |pid| pid.to_string());
            Err(PumasError::Other(format!(
                "Refused to stop process {pid}: not owned by Pumas ({reason}); the PID \
                 file was discarded and the process, if any, was left running"
            )))
        }
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
            AppId::Ollama,
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

        let path = launcher.version_path("v1.0.0").unwrap();
        assert!(launcher.version_path("../x").is_err());
        assert_eq!(path, temp.path().join("ollama-versions/v1.0.0"));
    }

    fn ollama_pid_file(launcher: &VersionLauncher, tag: &str) -> PathBuf {
        let dir = launcher.version_path(tag).unwrap();
        std::fs::create_dir_all(&dir).unwrap();
        dir.join("ollama.pid")
    }

    #[tokio::test]
    async fn bare_pid_file_is_never_signalled_or_reported_running() {
        let (launcher, _temp) = create_test_launcher();
        let pid_file = ollama_pid_file(&launcher, "v1.0.0");
        // A live process (this test) named only by a bare PID file.
        std::fs::write(&pid_file, std::process::id().to_string()).unwrap();

        assert!(!launcher.is_version_running("v1.0.0").await);
        let error = launcher.stop_version("v1.0.0").await.unwrap_err();
        assert!(error.to_string().contains("not owned by Pumas"), "{error}");
        // Discarded, and this process is evidently still alive.
        assert!(!pid_file.exists());
    }

    #[tokio::test]
    async fn reused_pid_with_foreign_identity_is_not_signalled() {
        use pumas_library::platform::process_identity::{capture, write_record};
        let (launcher, _temp) = create_test_launcher();
        let pid_file = ollama_pid_file(&launcher, "v1.0.0");
        let mut identity = capture(std::process::id()).unwrap();
        // The PID is live but its start time no longer matches the record.
        identity.start_time = identity.start_time.wrapping_add(1);
        write_record(&pid_file, &identity).unwrap();

        assert!(!launcher.is_version_running("v1.0.0").await);
        assert!(!launcher.stop_version("v1.0.0").await.unwrap());
        assert!(!pid_file.exists());
    }

    #[cfg(target_os = "linux")]
    #[tokio::test]
    async fn verified_record_is_reported_running_and_stopped() {
        use pumas_library::platform::process_identity::{capture, write_record};
        let (launcher, _temp) = create_test_launcher();
        let pid_file = ollama_pid_file(&launcher, "v1.0.0");
        let mut child = std::process::Command::new("sleep")
            .arg("30")
            .spawn()
            .unwrap();
        write_record(&pid_file, &capture(child.id()).unwrap()).unwrap();

        assert!(launcher.is_version_running("v1.0.0").await);
        assert!(launcher.stop_version("v1.0.0").await.unwrap());
        assert!(!pid_file.exists());
        let status = child.wait().unwrap();
        assert!(!status.success());
        assert!(!launcher.is_version_running("v1.0.0").await);
    }

    #[tokio::test]
    async fn missing_pid_file_reports_not_running() {
        let (launcher, _temp) = create_test_launcher();
        assert!(!launcher.is_version_running("v1.0.0").await);
        assert!(!launcher.stop_version("v1.0.0").await.unwrap());
    }

    #[tokio::test]
    async fn test_is_version_running_no_pid_file() {
        let (launcher, temp) = create_test_launcher();

        // Create version directory but no PID file
        std::fs::create_dir_all(temp.path().join("ollama-versions/v1.0.0")).unwrap();

        assert!(!launcher.is_version_running("v1.0.0").await);
    }
}
