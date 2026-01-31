//! Process manager factory for creating app-specific managers.

use super::traits::AppProcessManager;
use pumas_library::plugins::{InstallationType, PluginConfig, PluginLoader};
use pumas_library::Result;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

/// Factory for creating app process managers.
///
/// Creates the appropriate process manager based on the app's installation type
/// as defined in its plugin configuration.
pub struct ProcessManagerFactory {
    /// Launcher root directory.
    launcher_root: PathBuf,
    /// Plugin loader for getting app configurations.
    plugin_loader: Arc<PluginLoader>,
    /// Cached process managers by app ID.
    managers: std::sync::RwLock<HashMap<String, Arc<dyn AppProcessManager>>>,
}

impl ProcessManagerFactory {
    /// Create a new process manager factory.
    pub fn new(launcher_root: PathBuf, plugin_loader: Arc<PluginLoader>) -> Self {
        Self {
            launcher_root,
            plugin_loader,
            managers: std::sync::RwLock::new(HashMap::new()),
        }
    }

    /// Get or create a process manager for the specified app.
    ///
    /// Returns None if the app is not supported or the plugin config is missing.
    pub fn get_manager(&self, app_id: &str) -> Option<Arc<dyn AppProcessManager>> {
        // Check cache first
        {
            if let Ok(managers) = self.managers.read() {
                if let Some(manager) = managers.get(app_id) {
                    return Some(manager.clone());
                }
            }
        }

        // Get plugin config
        let plugin = self.plugin_loader.get(app_id)?;

        // Create manager based on installation type
        let manager: Arc<dyn AppProcessManager> = match plugin.installation_type {
            InstallationType::Binary => {
                Arc::new(BinaryProcessManager::new(
                    self.launcher_root.clone(),
                    plugin.clone(),
                ))
            }
            InstallationType::PythonVenv => {
                Arc::new(PythonProcessManager::new(
                    self.launcher_root.clone(),
                    plugin.clone(),
                ))
            }
            InstallationType::Docker => {
                // Docker support can be added later
                return None;
            }
        };

        // Cache the manager
        if let Ok(mut managers) = self.managers.write() {
            managers.insert(app_id.to_string(), manager.clone());
        }

        Some(manager)
    }

    /// Check if a manager exists for the given app.
    pub fn has_manager(&self, app_id: &str) -> bool {
        self.plugin_loader.exists(app_id)
    }

    /// Get the plugin loader.
    pub fn plugin_loader(&self) -> &Arc<PluginLoader> {
        &self.plugin_loader
    }
}

/// Process manager for binary-based apps (Ollama, etc.).
struct BinaryProcessManager {
    launcher_root: PathBuf,
    plugin: PluginConfig,
}

impl BinaryProcessManager {
    fn new(launcher_root: PathBuf, plugin: PluginConfig) -> Self {
        Self {
            launcher_root,
            plugin,
        }
    }

    fn versions_dir(&self) -> PathBuf {
        self.launcher_root.join(format!("{}-versions", self.plugin.id))
    }

    fn binary_name(&self) -> String {
        if cfg!(windows) {
            format!("{}.exe", self.plugin.id)
        } else {
            self.plugin.id.clone()
        }
    }
}

#[async_trait::async_trait]
impl AppProcessManager for BinaryProcessManager {
    fn app_id(&self) -> &str {
        &self.plugin.id
    }

    async fn launch(&self, version_tag: &str) -> Result<super::ProcessHandle> {
        use pumas_library::PumasError;
        use std::process::Command;

        let version_path = self.version_path(version_tag);
        let binary_path = version_path.join(self.binary_name());

        if !binary_path.exists() {
            return Ok(super::ProcessHandle {
                success: false,
                log_file: None,
                error: Some(format!("Binary not found: {}", binary_path.display())),
                ready: false,
            });
        }

        // Create log file
        let logs_dir = self.launcher_root.join("launcher-data").join("logs");
        std::fs::create_dir_all(&logs_dir).ok();
        let log_file = logs_dir.join(format!("{}.log", self.plugin.id));

        // Launch the binary
        let log_file_clone = log_file.clone();
        let result = tokio::task::spawn_blocking(move || {
            let log = std::fs::File::create(&log_file_clone)?;
            Command::new(&binary_path)
                .arg("serve")
                .stdout(log.try_clone()?)
                .stderr(log)
                .spawn()
        }).await;

        match result {
            Ok(Ok(_child)) => Ok(super::ProcessHandle {
                success: true,
                log_file: Some(log_file),
                error: None,
                ready: true,
            }),
            Ok(Err(e)) => Ok(super::ProcessHandle {
                success: false,
                log_file: Some(log_file),
                error: Some(format!("Failed to launch: {}", e)),
                ready: false,
            }),
            Err(e) => Err(PumasError::LaunchFailed {
                app: self.plugin.id.clone(),
                message: format!("Task join error: {}", e),
            }),
        }
    }

    async fn stop(&self) -> Result<bool> {
        // Find and kill process by name or PID file
        let pid_file = self.launcher_root.join(format!("{}.pid", self.plugin.id));
        if pid_file.exists() {
            if let Ok(pid_str) = std::fs::read_to_string(&pid_file) {
                if let Ok(pid) = pid_str.trim().parse::<i32>() {
                    #[cfg(unix)]
                    {
                        use nix::sys::signal::{self, Signal};
                        use nix::unistd::Pid;
                        let _ = signal::kill(Pid::from_raw(pid), Signal::SIGTERM);
                    }
                    #[cfg(windows)]
                    {
                        let _ = std::process::Command::new("taskkill")
                            .args(["/PID", &pid.to_string(), "/F"])
                            .output();
                    }
                    std::fs::remove_file(&pid_file).ok();
                    return Ok(true);
                }
            }
        }
        Ok(false)
    }

    async fn is_running(&self) -> bool {
        // Check if we can connect to the health endpoint
        if let Some(ref conn) = self.plugin.connection {
            if let Some(ref health) = conn.health_endpoint {
                let url = format!("{}://localhost:{}{}", conn.protocol, conn.default_port, health);
                if let Ok(resp) = reqwest::Client::new()
                    .get(&url)
                    .timeout(std::time::Duration::from_secs(2))
                    .send()
                    .await
                {
                    return resp.status().is_success();
                }
            }
        }
        false
    }

    async fn get_status(&self) -> Option<super::ProcessStatus> {
        if !self.is_running().await {
            return None;
        }

        Some(super::ProcessStatus {
            port: self.plugin.connection.as_ref().map(|c| c.default_port),
            healthy: true,
            ..Default::default()
        })
    }

    async fn get_logs(&self, lines: usize) -> Vec<String> {
        let log_file = self.launcher_root
            .join("launcher-data")
            .join("logs")
            .join(format!("{}.log", self.plugin.id));

        if let Ok(content) = std::fs::read_to_string(&log_file) {
            content.lines().rev().take(lines).map(String::from).collect()
        } else {
            vec![]
        }
    }

    fn version_path(&self, version_tag: &str) -> PathBuf {
        self.versions_dir().join(version_tag)
    }

    async fn is_version_installed(&self, version_tag: &str) -> bool {
        let version_path = self.version_path(version_tag);
        let binary_path = version_path.join(self.binary_name());
        binary_path.exists()
    }
}

/// Process manager for Python venv-based apps (ComfyUI, etc.).
struct PythonProcessManager {
    launcher_root: PathBuf,
    plugin: PluginConfig,
}

impl PythonProcessManager {
    fn new(launcher_root: PathBuf, plugin: PluginConfig) -> Self {
        Self {
            launcher_root,
            plugin,
        }
    }

    fn versions_dir(&self) -> PathBuf {
        self.launcher_root.join(format!("{}-versions", self.plugin.id))
    }
}

#[async_trait::async_trait]
impl AppProcessManager for PythonProcessManager {
    fn app_id(&self) -> &str {
        &self.plugin.id
    }

    async fn launch(&self, version_tag: &str) -> Result<super::ProcessHandle> {
        // Python apps have more complex launch requirements
        // For now, delegate to existing VersionManager
        // This is a placeholder that can be expanded
        let version_path = self.version_path(version_tag);

        if !version_path.exists() {
            return Ok(super::ProcessHandle {
                success: false,
                log_file: None,
                error: Some(format!("Version not installed: {}", version_tag)),
                ready: false,
            });
        }

        let python_config = self.plugin.python_config.as_ref();
        let entry_point = python_config
            .map(|c| c.entry_point.as_str())
            .unwrap_or("main.py");

        let venv_python = version_path.join(".venv").join("bin").join("python");
        let entry_script = version_path.join(entry_point);

        if !venv_python.exists() {
            return Ok(super::ProcessHandle {
                success: false,
                log_file: None,
                error: Some("Virtual environment not set up".to_string()),
                ready: false,
            });
        }

        // Create log file
        let logs_dir = self.launcher_root.join("launcher-data").join("logs");
        std::fs::create_dir_all(&logs_dir).ok();
        let log_file = logs_dir.join(format!("{}.log", self.plugin.id));

        let log_file_clone = log_file.clone();
        let result = tokio::task::spawn_blocking(move || {
            let log = std::fs::File::create(&log_file_clone)?;
            std::process::Command::new(venv_python)
                .arg(entry_script)
                .current_dir(version_path)
                .stdout(log.try_clone()?)
                .stderr(log)
                .spawn()
        }).await;

        match result {
            Ok(Ok(_child)) => Ok(super::ProcessHandle {
                success: true,
                log_file: Some(log_file),
                error: None,
                ready: true,
            }),
            Ok(Err(e)) => Ok(super::ProcessHandle {
                success: false,
                log_file: Some(log_file),
                error: Some(format!("Failed to launch: {}", e)),
                ready: false,
            }),
            Err(e) => Err(pumas_library::PumasError::LaunchFailed {
                app: self.plugin.id.clone(),
                message: format!("Task join error: {}", e),
            }),
        }
    }

    async fn stop(&self) -> Result<bool> {
        // Find and kill by PID file
        let pid_file = self.launcher_root.join(format!("{}.pid", self.plugin.id));
        if pid_file.exists() {
            if let Ok(pid_str) = std::fs::read_to_string(&pid_file) {
                if let Ok(pid) = pid_str.trim().parse::<i32>() {
                    #[cfg(unix)]
                    {
                        use nix::sys::signal::{self, Signal};
                        use nix::unistd::Pid;
                        let _ = signal::kill(Pid::from_raw(pid), Signal::SIGTERM);
                    }
                    std::fs::remove_file(&pid_file).ok();
                    return Ok(true);
                }
            }
        }
        Ok(false)
    }

    async fn is_running(&self) -> bool {
        // Check health endpoint if configured
        if let Some(ref conn) = self.plugin.connection {
            if let Some(ref health) = conn.health_endpoint {
                let url = format!("{}://localhost:{}{}", conn.protocol, conn.default_port, health);
                if let Ok(resp) = reqwest::Client::new()
                    .get(&url)
                    .timeout(std::time::Duration::from_secs(2))
                    .send()
                    .await
                {
                    return resp.status().is_success();
                }
            }
        }
        false
    }

    async fn get_status(&self) -> Option<super::ProcessStatus> {
        if !self.is_running().await {
            return None;
        }

        Some(super::ProcessStatus {
            port: self.plugin.connection.as_ref().map(|c| c.default_port),
            healthy: true,
            ..Default::default()
        })
    }

    async fn get_logs(&self, lines: usize) -> Vec<String> {
        let log_file = self.launcher_root
            .join("launcher-data")
            .join("logs")
            .join(format!("{}.log", self.plugin.id));

        if let Ok(content) = std::fs::read_to_string(&log_file) {
            content.lines().rev().take(lines).map(String::from).collect()
        } else {
            vec![]
        }
    }

    fn version_path(&self, version_tag: &str) -> PathBuf {
        self.versions_dir().join(version_tag)
    }

    async fn is_version_installed(&self, version_tag: &str) -> bool {
        let version_path = self.version_path(version_tag);
        version_path.exists()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_binary_manager_version_path() {
        let temp = TempDir::new().unwrap();
        let plugin = PluginConfig {
            id: "test-app".to_string(),
            display_name: "Test App".to_string(),
            description: String::new(),
            icon: None,
            github_repo: None,
            installation_type: InstallationType::Binary,
            capabilities: Default::default(),
            connection: None,
            version_filter: None,
            model_compatibility: None,
            python_config: None,
            api: Default::default(),
            panel_layout: vec![],
            sidebar_priority: 100,
            enabled_by_default: true,
        };

        let manager = BinaryProcessManager::new(temp.path().to_path_buf(), plugin);
        let path = manager.version_path("v1.0.0");
        assert!(path.ends_with("test-app-versions/v1.0.0"));
    }
}
