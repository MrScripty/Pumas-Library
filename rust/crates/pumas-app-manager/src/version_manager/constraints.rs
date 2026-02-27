//! Dependency constraints management.
//!
//! Resolves unpinned dependencies to specific versions based on release dates
//! for reproducible installations.

use chrono::{DateTime, Utc};
use pumas_library::{PumasError, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;
use tracing::{debug, info, warn};

/// Manages dependency constraints for reproducible installations.
pub struct ConstraintsManager {
    /// Directory for storing constraints files.
    constraints_dir: PathBuf,
    /// Cache of PyPI package versions (package name -> version -> upload date).
    pypi_cache: Mutex<HashMap<String, HashMap<String, DateTime<Utc>>>>,
    /// Cache of built constraints (tag -> package -> pinned version).
    constraints_cache: Mutex<HashMap<String, HashMap<String, String>>>,
}

/// Cached constraints file data.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ConstraintsCacheFile {
    #[serde(flatten)]
    constraints: HashMap<String, HashMap<String, String>>,
}

impl ConstraintsManager {
    /// Create a new constraints manager.
    pub fn new(constraints_dir: PathBuf) -> Self {
        let manager = Self {
            constraints_dir,
            pypi_cache: Mutex::new(HashMap::new()),
            constraints_cache: Mutex::new(HashMap::new()),
        };
        manager.load_cache();
        manager
    }

    /// Load constraints cache from disk.
    fn load_cache(&self) {
        let cache_path = self.constraints_dir.join("constraints-cache.json");
        if cache_path.exists() {
            if let Ok(content) = std::fs::read_to_string(&cache_path) {
                if let Ok(cache) = serde_json::from_str::<ConstraintsCacheFile>(&content) {
                    *self.constraints_cache.lock().unwrap() = cache.constraints;
                    debug!(
                        "Loaded constraints cache with {} entries",
                        self.constraints_cache.lock().unwrap().len()
                    );
                }
            }
        }
    }

    /// Save constraints cache to disk.
    fn save_cache(&self) -> Result<()> {
        std::fs::create_dir_all(&self.constraints_dir).map_err(|e| PumasError::Io {
            message: format!("Failed to create constraints directory: {}", e),
            path: Some(self.constraints_dir.clone()),
            source: Some(e),
        })?;

        let cache_path = self.constraints_dir.join("constraints-cache.json");
        let cache = ConstraintsCacheFile {
            constraints: self.constraints_cache.lock().unwrap().clone(),
        };

        let json = serde_json::to_string_pretty(&cache)?;
        std::fs::write(&cache_path, json).map_err(|e| PumasError::Io {
            message: format!("Failed to write constraints cache: {}", e),
            path: Some(cache_path),
            source: Some(e),
        })?;

        Ok(())
    }

    /// Get or build a constraints file for a tag.
    pub fn get_constraints_file(&self, tag: &str) -> Result<Option<PathBuf>> {
        let constraints_path = self
            .constraints_dir
            .join(format!("{}.txt", self.safe_filename(tag)));

        // Check if file already exists
        if constraints_path.exists() {
            return Ok(Some(constraints_path));
        }

        // Check if we have cached constraints
        if self.constraints_cache.lock().unwrap().contains_key(tag) {
            // Write from cache
            if let Err(e) = self.write_constraints_from_cache(tag, &constraints_path) {
                warn!("Failed to write constraints from cache: {}", e);
            } else {
                return Ok(Some(constraints_path));
            }
        }

        // No constraints available (would need to build with release info)
        Ok(None)
    }

    /// Build constraints for a tag from requirements.
    pub async fn build_constraints(
        &self,
        tag: &str,
        requirements_content: &str,
        release_date: Option<DateTime<Utc>>,
    ) -> Result<PathBuf> {
        info!(
            "Building constraints for {} (release date: {:?})",
            tag, release_date
        );

        let constraints_path = self
            .constraints_dir
            .join(format!("{}.txt", self.safe_filename(tag)));

        // Ensure directory exists
        std::fs::create_dir_all(&self.constraints_dir).map_err(|e| PumasError::Io {
            message: format!("Failed to create constraints directory: {}", e),
            path: Some(self.constraints_dir.clone()),
            source: Some(e),
        })?;

        // Parse requirements and resolve versions
        let mut constraints = HashMap::new();
        let mut constraints_content = String::new();

        for line in requirements_content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') || line.starts_with('-') {
                continue;
            }

            // Parse package spec
            if let Some((package, spec)) = self.parse_requirement_line(line) {
                // If already pinned, use as-is
                if spec.starts_with("==") {
                    let version = spec.trim_start_matches("==").to_string();
                    constraints.insert(package.clone(), version.clone());
                    constraints_content.push_str(&format!("{}=={}\n", package, version));
                } else if !spec.is_empty() {
                    // Need to resolve version
                    if let Some(version) = self.resolve_version(&package, &spec, release_date).await
                    {
                        constraints.insert(package.clone(), version.clone());
                        constraints_content.push_str(&format!("{}=={}\n", package, version));
                    }
                }
            }
        }

        // Write constraints file
        std::fs::write(&constraints_path, &constraints_content).map_err(|e| PumasError::Io {
            message: format!("Failed to write constraints file: {}", e),
            path: Some(constraints_path.clone()),
            source: Some(e),
        })?;

        // Update cache
        self.constraints_cache
            .lock()
            .unwrap()
            .insert(tag.to_string(), constraints);
        let _ = self.save_cache();

        info!(
            "Built constraints for {} with {} packages",
            tag,
            self.constraints_cache
                .lock()
                .unwrap()
                .get(tag)
                .map(|c| c.len())
                .unwrap_or(0)
        );

        Ok(constraints_path)
    }

    /// Parse a requirement line into (package, spec).
    fn parse_requirement_line(&self, line: &str) -> Option<(String, String)> {
        // Handle various formats:
        // package
        // package==1.0.0
        // package>=1.0.0
        // package[extra]>=1.0.0
        // package>=1.0.0; python_version >= "3.8"

        let line = line.split(';').next()?.trim();

        // Find version specifier start
        let spec_start =
            line.find(|c: char| c == '=' || c == '>' || c == '<' || c == '~' || c == '!');

        if let Some(idx) = spec_start {
            let package = line[..idx].trim();
            let spec = line[idx..].trim();

            // Remove extras
            let package = package.split('[').next()?.trim();

            Some((package.to_string(), spec.to_string()))
        } else {
            // No version spec
            let package = line.split('[').next()?.trim();
            Some((package.to_string(), String::new()))
        }
    }

    /// Resolve a version for a package given a spec and release date.
    async fn resolve_version(
        &self,
        package: &str,
        spec: &str,
        release_date: Option<DateTime<Utc>>,
    ) -> Option<String> {
        // Fetch versions from PyPI
        let versions = self.fetch_pypi_versions(package).await.ok()?;

        if versions.is_empty() {
            return None;
        }

        // Parse spec
        let spec_set = self.parse_version_spec(spec);

        // Filter versions that match spec and are before release date
        let mut matching: Vec<_> = versions
            .iter()
            .filter(|(version, upload_date)| {
                // Check version matches spec
                if !self.version_matches_spec(version, &spec_set) {
                    return false;
                }

                // Check upload date is before release date
                if let Some(release) = release_date {
                    if **upload_date > release {
                        return false;
                    }
                }

                true
            })
            .collect();

        // Sort by version (descending) and take newest
        matching.sort_by(|a, b| self.compare_versions(&b.0, &a.0));
        matching.first().map(|(v, _)| v.to_string())
    }

    /// Fetch available versions from PyPI.
    async fn fetch_pypi_versions(&self, package: &str) -> Result<HashMap<String, DateTime<Utc>>> {
        // Check cache first
        {
            let cache = self.pypi_cache.lock().unwrap();
            if let Some(versions) = cache.get(package) {
                return Ok(versions.clone());
            }
        }

        // Fetch from PyPI
        let url = format!("https://pypi.org/pypi/{}/json", package);
        let client = reqwest::Client::new();

        let response = client
            .get(&url)
            .send()
            .await
            .map_err(|e| PumasError::Network {
                message: format!("Failed to fetch PyPI info for {}: {}", package, e),
                cause: Some(e.to_string()),
            })?;

        if !response.status().is_success() {
            return Err(PumasError::Network {
                message: format!("PyPI returned {} for {}", response.status(), package),
                cause: None,
            });
        }

        let data: PyPIResponse = response.json().await.map_err(|e| PumasError::Network {
            message: format!("Failed to parse PyPI response: {}", e),
            cause: Some(e.to_string()),
        })?;

        // Build versions map
        let mut versions = HashMap::new();
        for (version, releases) in data.releases {
            if let Some(release) = releases.first() {
                if let Ok(upload_time) = DateTime::parse_from_rfc3339(&release.upload_time_iso_8601)
                {
                    versions.insert(version, upload_time.with_timezone(&Utc));
                }
            }
        }

        // Cache result
        {
            let mut cache = self.pypi_cache.lock().unwrap();
            cache.insert(package.to_string(), versions.clone());
        }

        Ok(versions)
    }

    /// Parse a version specifier string into individual specs.
    fn parse_version_spec(&self, spec: &str) -> Vec<(String, String)> {
        // Handle specs like ">=1.0,<2.0" or ">=1.0"
        let mut specs = Vec::new();

        for part in spec.split(',') {
            let part = part.trim();
            if part.is_empty() {
                continue;
            }

            // Extract operator and version
            let ops = ["===", "~=", "!=", ">=", "<=", "==", ">", "<"];
            for op in ops {
                if part.starts_with(op) {
                    let version = part[op.len()..].trim().to_string();
                    specs.push((op.to_string(), version));
                    break;
                }
            }
        }

        specs
    }

    /// Check if a version matches a spec set.
    fn version_matches_spec(&self, version: &str, specs: &[(String, String)]) -> bool {
        for (op, spec_version) in specs {
            let matches = match op.as_str() {
                ">=" => self.compare_versions(version, spec_version) >= std::cmp::Ordering::Equal,
                "<=" => self.compare_versions(version, spec_version) <= std::cmp::Ordering::Equal,
                ">" => self.compare_versions(version, spec_version) == std::cmp::Ordering::Greater,
                "<" => self.compare_versions(version, spec_version) == std::cmp::Ordering::Less,
                "==" => version == spec_version,
                "!=" => version != spec_version,
                "~=" => {
                    // Compatible release
                    let prefix = self.get_version_prefix(spec_version);
                    version.starts_with(&prefix)
                }
                _ => true,
            };

            if !matches {
                return false;
            }
        }

        true
    }

    /// Compare two version strings.
    fn compare_versions(&self, a: &str, b: &str) -> std::cmp::Ordering {
        let a_parts: Vec<u64> = a
            .split('.')
            .filter_map(|p| {
                p.chars()
                    .take_while(|c| c.is_ascii_digit())
                    .collect::<String>()
                    .parse()
                    .ok()
            })
            .collect();

        let b_parts: Vec<u64> = b
            .split('.')
            .filter_map(|p| {
                p.chars()
                    .take_while(|c| c.is_ascii_digit())
                    .collect::<String>()
                    .parse()
                    .ok()
            })
            .collect();

        for (a_part, b_part) in a_parts.iter().zip(b_parts.iter()) {
            match a_part.cmp(b_part) {
                std::cmp::Ordering::Equal => continue,
                other => return other,
            }
        }

        a_parts.len().cmp(&b_parts.len())
    }

    /// Get version prefix for compatible release matching.
    fn get_version_prefix(&self, version: &str) -> String {
        let parts: Vec<_> = version.split('.').collect();
        if parts.len() >= 2 {
            format!("{}.{}", parts[0], parts[1])
        } else {
            version.to_string()
        }
    }

    /// Write constraints from cache to a file.
    fn write_constraints_from_cache(&self, tag: &str, path: &PathBuf) -> Result<()> {
        let cache = self.constraints_cache.lock().unwrap();
        if let Some(constraints) = cache.get(tag) {
            let mut content = String::new();
            for (package, version) in constraints {
                content.push_str(&format!("{}=={}\n", package, version));
            }

            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).ok();
            }

            std::fs::write(path, content).map_err(|e| PumasError::Io {
                message: format!("Failed to write constraints: {}", e),
                path: Some(path.clone()),
                source: Some(e),
            })?;
        }

        Ok(())
    }

    /// Create a safe filename from a tag.
    fn safe_filename(&self, tag: &str) -> String {
        tag.chars()
            .map(|c| {
                if c.is_alphanumeric() || c == '-' || c == '_' || c == '.' {
                    c
                } else {
                    '-'
                }
            })
            .collect()
    }
}

/// PyPI API response structure.
#[derive(Debug, Deserialize)]
struct PyPIResponse {
    releases: HashMap<String, Vec<PyPIRelease>>,
}

/// PyPI release info.
#[derive(Debug, Deserialize)]
struct PyPIRelease {
    upload_time_iso_8601: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_test_manager() -> (ConstraintsManager, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let manager = ConstraintsManager::new(temp_dir.path().to_path_buf());
        (manager, temp_dir)
    }

    #[test]
    fn test_parse_requirement_line() {
        let (manager, _temp) = create_test_manager();

        // Simple package
        let result = manager.parse_requirement_line("numpy");
        assert_eq!(result, Some(("numpy".to_string(), "".to_string())));

        // Pinned version
        let result = manager.parse_requirement_line("numpy==1.24.0");
        assert_eq!(result, Some(("numpy".to_string(), "==1.24.0".to_string())));

        // Range version
        let result = manager.parse_requirement_line("torch>=2.0.0");
        assert_eq!(result, Some(("torch".to_string(), ">=2.0.0".to_string())));

        // With extras
        let result = manager.parse_requirement_line("pillow[webp]>=9.0");
        assert_eq!(result, Some(("pillow".to_string(), ">=9.0".to_string())));

        // With environment marker
        let result = manager.parse_requirement_line("numpy>=1.0; python_version >= \"3.8\"");
        assert_eq!(result, Some(("numpy".to_string(), ">=1.0".to_string())));
    }

    #[test]
    fn test_compare_versions() {
        let (manager, _temp) = create_test_manager();

        assert_eq!(
            manager.compare_versions("1.0.0", "1.0.0"),
            std::cmp::Ordering::Equal
        );
        assert_eq!(
            manager.compare_versions("2.0.0", "1.0.0"),
            std::cmp::Ordering::Greater
        );
        assert_eq!(
            manager.compare_versions("1.0.0", "2.0.0"),
            std::cmp::Ordering::Less
        );
        assert_eq!(
            manager.compare_versions("1.10.0", "1.9.0"),
            std::cmp::Ordering::Greater
        );
        assert_eq!(
            manager.compare_versions("1.0.0", "1.0"),
            std::cmp::Ordering::Greater
        );
    }

    #[test]
    fn test_version_matches_spec() {
        let (manager, _temp) = create_test_manager();

        let specs = vec![(">=".to_string(), "1.0.0".to_string())];
        assert!(manager.version_matches_spec("1.0.0", &specs));
        assert!(manager.version_matches_spec("2.0.0", &specs));
        assert!(!manager.version_matches_spec("0.9.0", &specs));

        let specs = vec![
            (">=".to_string(), "1.0.0".to_string()),
            ("<".to_string(), "2.0.0".to_string()),
        ];
        assert!(manager.version_matches_spec("1.5.0", &specs));
        assert!(!manager.version_matches_spec("2.0.0", &specs));
        assert!(!manager.version_matches_spec("0.5.0", &specs));
    }

    #[test]
    fn test_safe_filename() {
        let (manager, _temp) = create_test_manager();

        assert_eq!(manager.safe_filename("v1.0.0"), "v1.0.0");
        assert_eq!(manager.safe_filename("v1.0.0-beta"), "v1.0.0-beta");
        assert_eq!(manager.safe_filename("v1.0.0/rc1"), "v1.0.0-rc1");
    }
}
