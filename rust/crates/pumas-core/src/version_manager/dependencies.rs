//! Dependency management for Python versions.
//!
//! Handles checking and installing dependencies using pip/uv.

use crate::config::AppId;
use crate::models::DependencyStatus;
use crate::version_manager::constraints::ConstraintsManager;
use crate::version_manager::progress::ProgressUpdate;
use crate::{PumasError, Result};
use regex::Regex;
use std::collections::HashSet;
use std::path::PathBuf;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::mpsc;
use tracing::{debug, info, warn};

/// Manages Python dependencies for versions.
pub struct DependencyManager {
    /// Root directory for launcher.
    launcher_root: PathBuf,
    /// Application ID.
    app_id: AppId,
    /// Pip cache directory.
    pip_cache_dir: PathBuf,
}

impl DependencyManager {
    /// Create a new dependency manager.
    pub fn new(launcher_root: PathBuf, app_id: AppId, pip_cache_dir: PathBuf) -> Self {
        Self {
            launcher_root,
            app_id,
            pip_cache_dir,
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

    /// Check if a venv exists for a version.
    fn has_venv(&self, tag: &str) -> bool {
        self.venv_python(tag).exists()
    }

    /// Check dependencies for a version.
    pub async fn check_dependencies(&self, tag: &str) -> Result<DependencyStatus> {
        let version_path = self.version_path(tag);
        if !version_path.exists() {
            return Err(PumasError::VersionNotFound {
                tag: tag.to_string(),
            });
        }

        let venv_python = self.venv_python(tag);
        if !venv_python.exists() {
            return Ok(DependencyStatus {
                installed: vec![],
                missing: vec!["Virtual environment not created".to_string()],
                requirements_file: None,
            });
        }

        let requirements_path = version_path.join("requirements.txt");
        if !requirements_path.exists() {
            return Ok(DependencyStatus {
                installed: vec![],
                missing: vec![],
                requirements_file: None,
            });
        }

        // Read requirements
        let requirements_content =
            std::fs::read_to_string(&requirements_path).map_err(|e| PumasError::Io {
                message: format!("Failed to read requirements.txt: {}", e),
                path: Some(requirements_path.clone()),
                source: Some(e),
            })?;

        let required = self.parse_requirements(&requirements_content);

        // Get installed packages
        let installed = self.get_installed_packages(tag).await?;

        // Find missing packages
        let installed_set: HashSet<_> = installed.iter().map(|s| self.canonicalize_name(s)).collect();
        let missing: Vec<_> = required
            .iter()
            .filter(|r| !installed_set.contains(&self.canonicalize_name(r)))
            .cloned()
            .collect();

        Ok(DependencyStatus {
            installed,
            missing,
            requirements_file: Some("requirements.txt".to_string()),
        })
    }

    /// Parse requirements from requirements.txt content.
    fn parse_requirements(&self, content: &str) -> Vec<String> {
        let mut packages = Vec::new();
        let mut in_non_essential = false;

        for line in content.lines() {
            let line = line.trim();

            // Skip empty lines and comments
            if line.is_empty() || line.starts_with('#') {
                // Check for non-essential section marker
                if line.to_lowercase().contains("non essential") {
                    in_non_essential = true;
                }
                continue;
            }

            // Skip non-essential dependencies
            if in_non_essential {
                continue;
            }

            // Skip -r includes and other pip options
            if line.starts_with('-') {
                continue;
            }

            // Extract package name (before any version specifier)
            let package_name = line
                .split(|c| c == '=' || c == '>' || c == '<' || c == '[' || c == ';')
                .next()
                .map(|s| s.trim())
                .unwrap_or("");

            if !package_name.is_empty() {
                packages.push(package_name.to_string());
            }
        }

        packages
    }

    /// Get installed packages using pip list.
    async fn get_installed_packages(&self, tag: &str) -> Result<Vec<String>> {
        let venv_python = self.venv_python(tag);

        // Try JSON format first
        let output = tokio::process::Command::new(&venv_python)
            .args(["-m", "pip", "list", "--format=json"])
            .output()
            .await
            .map_err(|e| PumasError::Other(format!("Failed to run pip list: {}", e)))?;

        if output.status.success() {
            let json_str = String::from_utf8_lossy(&output.stdout);
            if let Ok(packages) = serde_json::from_str::<Vec<PipPackage>>(&json_str) {
                return Ok(packages.into_iter().map(|p| p.name).collect());
            }
        }

        // Fall back to freeze format
        let output = tokio::process::Command::new(&venv_python)
            .args(["-m", "pip", "list", "--format=freeze"])
            .output()
            .await
            .map_err(|e| PumasError::Other(format!("Failed to run pip list: {}", e)))?;

        if output.status.success() {
            let freeze_str = String::from_utf8_lossy(&output.stdout);
            let packages: Vec<_> = freeze_str
                .lines()
                .filter_map(|line| {
                    let name = line.split("==").next()?.trim();
                    if !name.is_empty() {
                        Some(name.to_string())
                    } else {
                        None
                    }
                })
                .collect();
            return Ok(packages);
        }

        warn!("Failed to get installed packages for {}", tag);
        Ok(vec![])
    }

    /// Canonicalize a package name (lowercase, replace _ and - with -)
    fn canonicalize_name(&self, name: &str) -> String {
        name.to_lowercase().replace('_', "-")
    }

    /// Install dependencies for a version.
    pub async fn install_dependencies(
        &self,
        tag: &str,
        constraints_manager: &ConstraintsManager,
        progress_tx: Option<mpsc::Sender<ProgressUpdate>>,
    ) -> Result<bool> {
        let version_path = self.version_path(tag);
        if !version_path.exists() {
            return Err(PumasError::VersionNotFound {
                tag: tag.to_string(),
            });
        }

        // Create venv if needed
        if !self.has_venv(tag) {
            self.create_venv(tag).await?;
        }

        let requirements_path = version_path.join("requirements.txt");
        if !requirements_path.exists() {
            info!("No requirements.txt found for {}", tag);
            return Ok(true);
        }

        // Parse requirements for progress tracking
        let requirements_content = std::fs::read_to_string(&requirements_path)?;
        let packages = self.parse_requirements(&requirements_content);

        // Try to get or build constraints file
        let constraints_path = constraints_manager.get_constraints_file(tag).ok().flatten();

        // Install with progress tracking
        self.install_with_progress(
            tag,
            &requirements_path,
            constraints_path.as_ref(),
            &packages,
            progress_tx,
        )
        .await
    }

    /// Create a virtual environment.
    async fn create_venv(&self, tag: &str) -> Result<()> {
        let version_path = self.version_path(tag);
        let venv_dir = version_path.join("venv");

        info!("Creating virtual environment for {}", tag);

        let output = tokio::process::Command::new("python3")
            .args(["-m", "venv", venv_dir.to_string_lossy().as_ref()])
            .current_dir(&version_path)
            .output()
            .await
            .map_err(|e| PumasError::InstallationFailed {
                message: format!("Failed to create venv: {}", e),
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(PumasError::InstallationFailed {
                message: format!("Venv creation failed: {}", stderr),
            });
        }

        // Ensure pip is available
        let venv_python = self.venv_python(tag);
        let _ = tokio::process::Command::new(&venv_python)
            .args(["-m", "ensurepip", "--upgrade"])
            .output()
            .await;

        let _ = tokio::process::Command::new(&venv_python)
            .args(["-m", "pip", "install", "--upgrade", "pip"])
            .output()
            .await;

        Ok(())
    }

    /// Install dependencies with progress tracking.
    async fn install_with_progress(
        &self,
        tag: &str,
        requirements_path: &PathBuf,
        constraints_path: Option<&PathBuf>,
        packages: &[String],
        progress_tx: Option<mpsc::Sender<ProgressUpdate>>,
    ) -> Result<bool> {
        let venv_python = self.venv_python(tag);
        let version_path = self.version_path(tag);

        info!("Installing {} packages for {}", packages.len(), tag);

        // Build command
        let mut cmd = tokio::process::Command::new(&venv_python);
        cmd.args(["-m", "pip", "install", "-r"])
            .arg(requirements_path);

        // Add constraints if available
        if let Some(constraints) = constraints_path {
            cmd.args(["-c"]).arg(constraints);
        }

        // Add global packages
        cmd.args(["setproctitle"]);

        // Set environment
        std::fs::create_dir_all(&self.pip_cache_dir).ok();
        cmd.env("PIP_CACHE_DIR", &self.pip_cache_dir);
        cmd.current_dir(&version_path);

        // Set up for streaming output
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        // Start process
        let mut child = cmd.spawn().map_err(|e| PumasError::InstallationFailed {
            message: format!("Failed to start pip: {}", e),
        })?;

        // Track progress by parsing output
        let stdout = child.stdout.take();
        let stderr = child.stderr.take();

        let collecting_re = Regex::new(r"(?i)collecting\s+([a-zA-Z0-9_-]+)").unwrap();
        let downloading_re = Regex::new(r"(?i)downloading\s+([^\s]+)\s*\(([^)]+)\)").unwrap();
        let installed_re = Regex::new(r"(?i)successfully installed").unwrap();

        let mut completed_count = 0u32;
        let total_count = packages.len() as u32;

        // Process stdout
        if let Some(stdout) = stdout {
            let reader = BufReader::new(stdout);
            let mut lines = reader.lines();

            while let Ok(Some(line)) = lines.next_line().await {
                debug!("pip: {}", line);

                // Check for package being collected
                if let Some(caps) = collecting_re.captures(&line) {
                    let current_package = caps.get(1).unwrap().as_str().to_string();
                    if let Some(ref tx) = progress_tx {
                        let _ = tx
                            .send(ProgressUpdate::Dependency {
                                package: current_package,
                                completed_count,
                                total_count: Some(total_count),
                                package_size: None,
                            })
                            .await;
                    }
                }

                // Check for download with size
                if let Some(caps) = downloading_re.captures(&line) {
                    let _url = caps.get(1).unwrap().as_str();
                    let _size = caps.get(2).unwrap().as_str();
                }

                // Check for completion
                if installed_re.is_match(&line) {
                    completed_count = total_count;
                }
            }
        }

        // Wait for process
        let status = child.wait().await.map_err(|e| PumasError::InstallationFailed {
            message: format!("Failed to wait for pip: {}", e),
        })?;

        // Read any remaining stderr
        if let Some(stderr) = stderr {
            let reader = BufReader::new(stderr);
            let mut lines = reader.lines();
            while let Ok(Some(line)) = lines.next_line().await {
                if line.contains("ERROR") || line.contains("error") {
                    warn!("pip stderr: {}", line);
                }
            }
        }

        if !status.success() {
            return Err(PumasError::DependencyInstallFailed {
                message: format!("pip install failed with status {}", status),
            });
        }

        info!("Dependencies installed successfully for {}", tag);
        Ok(true)
    }

    /// Get the Python version for a venv.
    pub async fn get_python_version(&self, tag: &str) -> Result<Option<String>> {
        let venv_python = self.venv_python(tag);
        if !venv_python.exists() {
            return Ok(None);
        }

        let output = tokio::process::Command::new(&venv_python)
            .args(["--version"])
            .output()
            .await
            .map_err(|e| PumasError::Other(format!("Failed to get Python version: {}", e)))?;

        if output.status.success() {
            let version = String::from_utf8_lossy(&output.stdout).trim().to_string();
            Ok(Some(version))
        } else {
            Ok(None)
        }
    }
}

/// Pip package info from JSON output.
#[derive(serde::Deserialize)]
struct PipPackage {
    name: String,
    #[allow(dead_code)]
    version: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_test_manager() -> (DependencyManager, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let manager = DependencyManager::new(
            temp_dir.path().to_path_buf(),
            AppId::ComfyUI,
            temp_dir.path().join("pip-cache"),
        );
        (manager, temp_dir)
    }

    #[test]
    fn test_parse_requirements() {
        let (manager, _temp) = create_test_manager();

        let content = r#"
# Main dependencies
torch>=2.0.0
numpy==1.24.0
pillow[webp]
requests

# Non essential dependencies
# These are optional
optional-package
"#;

        let packages = manager.parse_requirements(content);
        assert!(packages.contains(&"torch".to_string()));
        assert!(packages.contains(&"numpy".to_string()));
        assert!(packages.contains(&"pillow".to_string()));
        assert!(packages.contains(&"requests".to_string()));
        assert!(!packages.contains(&"optional-package".to_string()));
    }

    #[test]
    fn test_canonicalize_name() {
        let (manager, _temp) = create_test_manager();

        assert_eq!(manager.canonicalize_name("PyTorch"), "pytorch");
        assert_eq!(manager.canonicalize_name("scikit_learn"), "scikit-learn");
        assert_eq!(manager.canonicalize_name("PIL"), "pil");
    }
}
