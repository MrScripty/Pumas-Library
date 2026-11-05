//! Dependency management for Python versions.
//!
//! Handles checking and installing dependencies using pip/uv.

use crate::version_manager::constraints::ConstraintsManager;
use crate::version_manager::progress::ProgressUpdate;
use pumas_library::config::AppId;
use pumas_library::models::DependencyStatus;
use pumas_library::{PumasError, Result};
use regex::Regex;
use std::collections::HashSet;
use std::path::Path;
use std::path::PathBuf;
use std::process::Stdio;
use tokio::fs;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::mpsc;
use tracing::{debug, info, warn};

async fn path_exists(path: &Path) -> Result<bool> {
    fs::try_exists(path)
        .await
        .map_err(|err| PumasError::io_with_path(err, path))
}

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

    /// Get the version directory path; the tag must be one directory name.
    fn version_path(&self, tag: &str) -> Result<PathBuf> {
        super::validate_version_tag(tag)?;
        Ok(self
            .launcher_root
            .join(self.app_id.versions_dir_name())
            .join(tag))
    }

    /// Get the venv python path for a version.
    fn venv_python(&self, tag: &str) -> Result<PathBuf> {
        Ok(pumas_library::platform::paths::venv_python(
            &self.version_path(tag)?,
        ))
    }

    async fn read_requirements_file(&self, requirements_path: &Path) -> Result<String> {
        fs::read_to_string(requirements_path)
            .await
            .map_err(|e| PumasError::Io {
                message: format!("Failed to read requirements.txt: {}", e),
                path: Some(requirements_path.to_path_buf()),
                source: Some(e),
            })
    }

    /// Check dependencies for a version.
    pub async fn check_dependencies(&self, tag: &str) -> Result<DependencyStatus> {
        let version_path = self.version_path(tag)?;
        if !path_exists(&version_path).await? {
            return Err(PumasError::VersionNotFound {
                tag: tag.to_string(),
            });
        }

        let venv_python = self.venv_python(tag)?;
        if !path_exists(&venv_python).await? {
            return Ok(DependencyStatus {
                installed: vec![],
                missing: vec!["Virtual environment not created".to_string()],
                requirements_file: None,
            });
        }

        let requirements_path = version_path.join("requirements.txt");
        if !path_exists(&requirements_path).await? {
            return Ok(DependencyStatus {
                installed: vec![],
                missing: vec![],
                requirements_file: None,
            });
        }

        // Read requirements
        let requirements_content = self.read_requirements_file(&requirements_path).await?;

        let required = self.parse_requirements(&requirements_content);

        // Get installed packages
        let installed = self.get_installed_packages(tag).await?;

        // Find missing packages
        let installed_set: HashSet<_> = installed
            .iter()
            .map(|s| self.canonicalize_name(s))
            .collect();
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
                .split(['=', '>', '<', '[', ';', '@'])
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
        let venv_python = self.venv_python(tag)?;

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
        if self.app_id == AppId::Torch {
            return Err(PumasError::DependencyFailed {
                message: "Torch dependencies belong to its validated runtime bundle. Install another runtime version through version management; in-place dependency repair is unsupported.".to_string(),
            });
        }
        let version_path = self.version_path(tag)?;
        if !path_exists(&version_path).await? {
            return Err(PumasError::VersionNotFound {
                tag: tag.to_string(),
            });
        }

        // Nothing is created for a version that declares no Python
        // dependencies, such as a native Ollama or llama.cpp runtime.
        let requirements_path = version_path.join("requirements.txt");
        if !path_exists(&requirements_path).await? {
            info!("No requirements.txt found for {}", tag);
            return Ok(true);
        }

        // Dependencies go only into the version's own environment. Pumas
        // never builds one from whatever `python3` is first on PATH.
        if !path_exists(&self.venv_python(tag)?).await? {
            return Err(PumasError::DependencyFailed {
                message: format!(
                    "{tag} has no Python environment of its own; Pumas does not create one from a system interpreter"
                ),
            });
        }

        // Parse requirements for progress tracking
        let requirements_content = self.read_requirements_file(&requirements_path).await?;
        let packages = self.parse_requirements(&requirements_content);
        let hash_locked = requirements_content.contains("--hash=");

        // Try to get or build constraints file
        let constraints_path = constraints_manager
            .get_constraints_file(tag)
            .await
            .ok()
            .flatten();

        // Install with progress tracking
        self.install_with_progress(
            tag,
            &requirements_path,
            constraints_path.as_ref(),
            &packages,
            hash_locked,
            progress_tx,
        )
        .await
    }

    /// Install dependencies with progress tracking.
    ///
    /// pip runs isolated (`python -I`, `pip --isolated`), so neither the
    /// user's site packages, `PIP_*` environment nor pip configuration can
    /// redirect the install. A hash-locked requirements file is installed
    /// with `--require-hashes`, and then nothing unhashed is added to it.
    async fn install_with_progress(
        &self,
        tag: &str,
        requirements_path: &PathBuf,
        constraints_path: Option<&PathBuf>,
        packages: &[String],
        hash_locked: bool,
        progress_tx: Option<mpsc::Sender<ProgressUpdate>>,
    ) -> Result<bool> {
        let venv_python = self.venv_python(tag)?;
        let version_path = self.version_path(tag)?;

        info!("Installing {} packages for {}", packages.len(), tag);

        // Build command
        fs::create_dir_all(&self.pip_cache_dir).await.ok();
        let mut cmd = tokio::process::Command::new(&venv_python);
        cmd.args(["-I", "-m", "pip", "--isolated", "install", "--cache-dir"])
            .arg(&self.pip_cache_dir)
            .arg("-r")
            .arg(requirements_path);

        // Add constraints if available
        if let Some(constraints) = constraints_path {
            cmd.args(["-c"]).arg(constraints);
        }

        if hash_locked {
            cmd.arg("--require-hashes");
        } else {
            // Add global packages
            cmd.args(["setproctitle"]);
        }
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
        let status = child
            .wait()
            .await
            .map_err(|e| PumasError::InstallationFailed {
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
            return Err(PumasError::DependencyFailed {
                message: format!("pip install failed with status {}", status),
            });
        }

        info!("Dependencies installed successfully for {}", tag);
        Ok(true)
    }

    /// Get the Python version for a venv.
    pub async fn get_python_version(&self, tag: &str) -> Result<Option<String>> {
        let venv_python = self.venv_python(tag)?;
        if !path_exists(&venv_python).await? {
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
            AppId::Torch,
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

    #[test]
    fn locked_direct_reference_is_a_package_name() {
        let (manager, _temp) = create_test_manager();
        assert_eq!(
            manager.parse_requirements(
                "nunchaku @ https://example.test/nunchaku.whl \\\n    --hash=sha256:1234\n"
            ),
            vec!["nunchaku"]
        );
    }

    #[tokio::test]
    async fn torch_dependency_repair_cannot_mutate_an_installed_runtime() {
        let (manager, temp) = create_test_manager();
        let version = manager.version_path("candidate").unwrap();
        fs::create_dir_all(&version).await.unwrap();
        fs::write(version.join("requirements.txt"), "torch==2.9.1")
            .await
            .unwrap();
        let constraints = ConstraintsManager::new_with_cache(temp.path().join("constraints")).await;
        let result = manager
            .install_dependencies("candidate", &constraints, None)
            .await;
        assert!(matches!(result, Err(PumasError::DependencyFailed { .. })));
        assert!(!version.join("venv").exists());
        assert_eq!(
            fs::read_to_string(version.join("requirements.txt"))
                .await
                .unwrap(),
            "torch==2.9.1"
        );
    }

    #[tokio::test]
    async fn test_check_dependencies_returns_version_not_found_for_missing_version() {
        let (manager, _temp) = create_test_manager();

        let result = manager.check_dependencies("missing-version").await;
        assert!(matches!(result, Err(PumasError::VersionNotFound { .. })));
    }

    #[tokio::test]
    async fn test_get_python_version_returns_none_without_venv() {
        let (manager, _temp) = create_test_manager();

        let version = manager.get_python_version("missing-version").await.unwrap();
        assert_eq!(version, None);
    }

    fn native_manager(temp: &TempDir) -> DependencyManager {
        DependencyManager::new(
            temp.path().to_path_buf(),
            AppId::Ollama,
            temp.path().join("pip-cache"),
        )
    }

    #[tokio::test]
    async fn a_tag_that_leaves_the_versions_directory_is_refused() {
        let temp = TempDir::new().unwrap();
        let manager = native_manager(&temp);
        // A directory the escaping tag would otherwise reach.
        let outside = temp.path().join("x");
        fs::create_dir_all(&outside).await.unwrap();
        fs::write(outside.join("requirements.txt"), "requests\n")
            .await
            .unwrap();
        let constraints = ConstraintsManager::new(temp.path().join("constraints"));
        for tag in ["../x", "..", ".", "", "a/b", "a\\b", "/abs"] {
            assert!(
                matches!(
                    manager.install_dependencies(tag, &constraints, None).await,
                    Err(PumasError::Validation { field, .. }) if field == "tag"
                ),
                "{tag:?}"
            );
            assert!(manager.check_dependencies(tag).await.is_err(), "{tag:?}");
        }
        assert!(!outside.join("venv").exists());
    }

    #[tokio::test]
    async fn a_version_without_requirements_gets_no_environment() {
        let temp = TempDir::new().unwrap();
        let manager = native_manager(&temp);
        let version = manager.version_path("v0.5.1").unwrap();
        fs::create_dir_all(&version).await.unwrap();
        let constraints = ConstraintsManager::new(temp.path().join("constraints"));

        assert!(manager
            .install_dependencies("v0.5.1", &constraints, None)
            .await
            .unwrap());
        assert!(!version.join("venv").exists());
        assert!(!temp.path().join("pip-cache").exists());
    }

    #[tokio::test]
    async fn requirements_without_an_owned_environment_are_refused_not_built_from_path_python() {
        let temp = TempDir::new().unwrap();
        let manager = native_manager(&temp);
        let version = manager.version_path("v0.5.1").unwrap();
        fs::create_dir_all(&version).await.unwrap();
        fs::write(version.join("requirements.txt"), "requests\n")
            .await
            .unwrap();
        let constraints = ConstraintsManager::new(temp.path().join("constraints"));

        let error = manager
            .install_dependencies("v0.5.1", &constraints, None)
            .await
            .unwrap_err()
            .to_string();
        assert!(
            error.contains("no Python environment of its own"),
            "{error}"
        );
        assert!(!version.join("venv").exists());
    }
}
