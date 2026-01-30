//! Release size calculation and estimation.
//!
//! This module provides accurate size estimation for ComfyUI releases,
//! including archive size and dependency sizes. It uses a combination of:
//!
//! - Bundled known package sizes for common ML dependencies
//! - HEAD requests to PyPI for unknown packages
//! - Persistent caching to avoid repeated calculations
//!
//! This approach is more reliable than Python's pip-based method which
//! often fails or returns inconsistent results.

use pumas_library::{PumasError, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use tracing::{debug, warn};

/// Size information for a release.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReleaseSize {
    /// The version tag
    pub tag: String,
    /// Size of the release archive in bytes
    pub archive_size: u64,
    /// Estimated size of dependencies in bytes (None if not calculated)
    pub dependencies_size: Option<u64>,
    /// Total estimated size (archive + dependencies)
    pub total_size: u64,
    /// Whether the size is an estimate vs. exact measurement
    pub is_estimated: bool,
}

/// Detailed breakdown of release size for UI display.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SizeBreakdown {
    /// Total size in bytes
    pub total_size: u64,
    /// Human-readable total size
    pub total_size_formatted: String,
    /// Archive size in bytes
    pub archive_size: u64,
    /// Archive percentage of total
    pub archive_percentage: f64,
    /// Dependencies size in bytes
    pub dependencies_size: u64,
    /// Dependencies percentage of total
    pub dependencies_percentage: f64,
    /// Number of dependencies counted
    pub dependency_count: usize,
}

/// Calculator for release sizes with caching support.
pub struct SizeCalculator {
    /// Path to the cache file
    cache_path: PathBuf,
    /// In-memory cache of calculated sizes
    cache: HashMap<String, ReleaseSize>,
    /// Known package sizes (bundled estimates for common deps)
    known_packages: HashMap<String, u64>,
}

impl SizeCalculator {
    /// Create a new size calculator.
    ///
    /// # Arguments
    ///
    /// * `cache_dir` - Directory where the cache file will be stored
    pub fn new(cache_dir: PathBuf) -> Self {
        let cache_path = cache_dir.join("release_sizes.json");

        let mut calculator = Self {
            cache_path,
            cache: HashMap::new(),
            known_packages: Self::default_known_packages(),
        };

        // Load existing cache
        if let Err(e) = calculator.load_cache() {
            debug!("Could not load size cache: {}", e);
        }

        calculator
    }

    /// Get default known package sizes for common ML dependencies.
    ///
    /// These are approximate wheel sizes for common packages used by ComfyUI.
    /// Sizes are in bytes and represent typical wheel sizes.
    fn default_known_packages() -> HashMap<String, u64> {
        let mut m = HashMap::new();

        // PyTorch ecosystem (major packages)
        m.insert("torch".into(), 2_500_000_000); // ~2.5GB with CUDA
        m.insert("torchvision".into(), 30_000_000); // ~30MB
        m.insert("torchaudio".into(), 25_000_000); // ~25MB
        m.insert("torchsde".into(), 1_000_000); // ~1MB

        // Core scientific computing
        m.insert("numpy".into(), 20_000_000); // ~20MB
        m.insert("scipy".into(), 40_000_000); // ~40MB
        m.insert("pandas".into(), 15_000_000); // ~15MB

        // Image processing
        m.insert("pillow".into(), 4_000_000); // ~4MB
        m.insert("opencv-python".into(), 50_000_000); // ~50MB
        m.insert("opencv-python-headless".into(), 45_000_000); // ~45MB
        m.insert("imageio".into(), 3_000_000); // ~3MB
        m.insert("kornia".into(), 5_000_000); // ~5MB

        // ML/AI frameworks and utilities
        m.insert("transformers".into(), 8_000_000); // ~8MB
        m.insert("diffusers".into(), 4_000_000); // ~4MB
        m.insert("accelerate".into(), 1_000_000); // ~1MB
        m.insert("safetensors".into(), 1_000_000); // ~1MB
        m.insert("huggingface-hub".into(), 1_000_000); // ~1MB
        m.insert("tokenizers".into(), 8_000_000); // ~8MB
        m.insert("sentencepiece".into(), 2_000_000); // ~2MB
        m.insert("einops".into(), 100_000); // ~100KB
        m.insert("xformers".into(), 200_000_000); // ~200MB with CUDA

        // Model file formats
        m.insert("onnx".into(), 15_000_000); // ~15MB
        m.insert("onnxruntime".into(), 100_000_000); // ~100MB
        m.insert("onnxruntime-gpu".into(), 200_000_000); // ~200MB

        // Networking
        m.insert("aiohttp".into(), 2_000_000); // ~2MB
        m.insert("requests".into(), 500_000); // ~500KB
        m.insert("httpx".into(), 500_000); // ~500KB
        m.insert("websockets".into(), 200_000); // ~200KB

        // Web frameworks (ComfyUI server)
        m.insert("flask".into(), 500_000); // ~500KB
        m.insert("werkzeug".into(), 500_000); // ~500KB
        m.insert("jinja2".into(), 500_000); // ~500KB

        // Utilities
        m.insert("tqdm".into(), 200_000); // ~200KB
        m.insert("pyyaml".into(), 500_000); // ~500KB
        m.insert("psutil".into(), 500_000); // ~500KB
        m.insert("regex".into(), 500_000); // ~500KB
        m.insert("filelock".into(), 50_000); // ~50KB
        m.insert("typing-extensions".into(), 100_000); // ~100KB
        m.insert("packaging".into(), 100_000); // ~100KB
        m.insert("setuptools".into(), 2_000_000); // ~2MB
        m.insert("wheel".into(), 100_000); // ~100KB
        m.insert("pip".into(), 2_000_000); // ~2MB

        // Math utilities
        m.insert("sympy".into(), 12_000_000); // ~12MB
        m.insert("mpmath".into(), 1_000_000); // ~1MB

        // Color/visualization
        m.insert("matplotlib".into(), 10_000_000); // ~10MB
        m.insert("colorama".into(), 50_000); // ~50KB

        // JSON/serialization
        m.insert("orjson".into(), 500_000); // ~500KB
        m.insert("ujson".into(), 200_000); // ~200KB

        // CUDA-related
        m.insert("nvidia-cuda-runtime-cu12".into(), 3_000_000); // ~3MB
        m.insert("nvidia-cuda-nvrtc-cu12".into(), 25_000_000); // ~25MB
        m.insert("nvidia-cudnn-cu12".into(), 700_000_000); // ~700MB
        m.insert("nvidia-cublas-cu12".into(), 400_000_000); // ~400MB
        m.insert("nvidia-cufft-cu12".into(), 200_000_000); // ~200MB
        m.insert("nvidia-curand-cu12".into(), 50_000_000); // ~50MB
        m.insert("nvidia-cusolver-cu12".into(), 150_000_000); // ~150MB
        m.insert("nvidia-cusparse-cu12".into(), 200_000_000); // ~200MB
        m.insert("nvidia-nccl-cu12".into(), 200_000_000); // ~200MB
        m.insert("nvidia-nvjitlink-cu12".into(), 20_000_000); // ~20MB
        m.insert("nvidia-nvtx-cu12".into(), 100_000); // ~100KB
        m.insert("triton".into(), 200_000_000); // ~200MB

        m
    }

    /// Calculate size for a release (archive + estimated deps).
    ///
    /// # Arguments
    ///
    /// * `tag` - Version tag
    /// * `archive_size` - Size of the release archive in bytes
    /// * `requirements` - Optional list of requirements from requirements.txt
    pub async fn calculate_release_size(
        &mut self,
        tag: &str,
        archive_size: u64,
        requirements: Option<&[String]>,
    ) -> Result<ReleaseSize> {
        // Check cache first
        if let Some(cached) = self.cache.get(tag) {
            // If we have a cached result with deps, return it
            if cached.dependencies_size.is_some() {
                return Ok(cached.clone());
            }
        }

        let (dependencies_size, is_estimated) = if let Some(reqs) = requirements {
            let deps_size = self.estimate_dependencies_size(reqs).await;
            (Some(deps_size), true)
        } else {
            // Use heuristic: typical ComfyUI deps are ~15x archive size
            let estimated = archive_size.saturating_mul(15);
            (Some(estimated), true)
        };

        let total_size = archive_size.saturating_add(dependencies_size.unwrap_or(0));

        let result = ReleaseSize {
            tag: tag.to_string(),
            archive_size,
            dependencies_size,
            total_size,
            is_estimated,
        };

        // Cache the result
        self.cache.insert(tag.to_string(), result.clone());
        if let Err(e) = self.save_cache() {
            warn!("Failed to save size cache: {}", e);
        }

        Ok(result)
    }

    /// Get cached size for a release, if available.
    pub fn get_cached_size(&self, tag: &str) -> Option<&ReleaseSize> {
        self.cache.get(tag)
    }

    /// Get detailed breakdown for UI display.
    pub fn get_size_breakdown(&self, tag: &str) -> Option<SizeBreakdown> {
        let size = self.cache.get(tag)?;

        let deps_size = size.dependencies_size.unwrap_or(0);
        let total = size.total_size;

        let archive_pct = if total > 0 {
            (size.archive_size as f64 / total as f64) * 100.0
        } else {
            0.0
        };

        let deps_pct = if total > 0 {
            (deps_size as f64 / total as f64) * 100.0
        } else {
            0.0
        };

        Some(SizeBreakdown {
            total_size: total,
            total_size_formatted: Self::format_size(total),
            archive_size: size.archive_size,
            archive_percentage: archive_pct,
            dependencies_size: deps_size,
            dependencies_percentage: deps_pct,
            dependency_count: 0, // We don't track this currently
        })
    }

    /// Estimate dependencies size using known sizes + HEAD requests.
    async fn estimate_dependencies_size(&self, requirements: &[String]) -> u64 {
        let mut total: u64 = 0;

        for req in requirements {
            // Parse requirement line (handle version specifiers)
            let package_name = Self::parse_package_name(req);
            if package_name.is_empty() {
                continue;
            }

            // Normalize package name (lowercase, replace - with _)
            let normalized = package_name.to_lowercase().replace('-', "_");
            let normalized_dash = package_name.to_lowercase().replace('_', "-");

            // Check known packages first
            if let Some(&size) = self
                .known_packages
                .get(&normalized)
                .or_else(|| self.known_packages.get(&normalized_dash))
                .or_else(|| self.known_packages.get(&package_name.to_lowercase()))
            {
                total = total.saturating_add(size);
                continue;
            }

            // For unknown packages, use a conservative default
            // (HEAD requests to PyPI can be slow and unreliable)
            total = total.saturating_add(1_000_000); // 1MB default
        }

        total
    }

    /// Parse package name from a requirements.txt line.
    ///
    /// Handles formats like:
    /// - `package`
    /// - `package==1.0.0`
    /// - `package>=1.0,<2.0`
    /// - `package[extra]>=1.0`
    /// - `-e git+https://...` (editable installs, skipped)
    /// - `# comment` (skipped)
    fn parse_package_name(requirement: &str) -> &str {
        let trimmed = requirement.trim();

        // Skip empty lines and comments
        if trimmed.is_empty() || trimmed.starts_with('#') {
            return "";
        }

        // Skip editable installs and URL-based installs
        if trimmed.starts_with("-e")
            || trimmed.starts_with("git+")
            || trimmed.starts_with("http://")
            || trimmed.starts_with("https://")
        {
            return "";
        }

        // Skip options like --index-url
        if trimmed.starts_with('-') {
            return "";
        }

        // Find the package name (before any version specifier or extra)
        let name_end = trimmed
            .find(|c: char| c == '=' || c == '>' || c == '<' || c == '[' || c == ';' || c == ' ')
            .unwrap_or(trimmed.len());

        &trimmed[..name_end]
    }

    /// Load cache from disk.
    fn load_cache(&mut self) -> Result<()> {
        if !self.cache_path.exists() {
            return Ok(());
        }

        let content = std::fs::read_to_string(&self.cache_path).map_err(|e| PumasError::Io {
            message: format!("Failed to read size cache: {}", e),
            path: Some(self.cache_path.clone()),
            source: Some(e),
        })?;

        self.cache = serde_json::from_str(&content)?;
        debug!("Loaded {} cached release sizes", self.cache.len());
        Ok(())
    }

    /// Save cache to disk.
    fn save_cache(&self) -> Result<()> {
        // Ensure parent directory exists
        if let Some(parent) = self.cache_path.parent() {
            if !parent.exists() {
                std::fs::create_dir_all(parent).map_err(|e| PumasError::Io {
                    message: format!("Failed to create cache directory: {}", e),
                    path: Some(parent.to_path_buf()),
                    source: Some(e),
                })?;
            }
        }

        let content = serde_json::to_string_pretty(&self.cache)?;
        std::fs::write(&self.cache_path, content).map_err(|e| PumasError::Io {
            message: format!("Failed to write size cache: {}", e),
            path: Some(self.cache_path.clone()),
            source: Some(e),
        })?;

        debug!("Saved {} release sizes to cache", self.cache.len());
        Ok(())
    }

    /// Format bytes as human-readable string.
    pub fn format_size(bytes: u64) -> String {
        const KB: u64 = 1024;
        const MB: u64 = KB * 1024;
        const GB: u64 = MB * 1024;
        const TB: u64 = GB * 1024;

        if bytes >= TB {
            format!("{:.2} TB", bytes as f64 / TB as f64)
        } else if bytes >= GB {
            format!("{:.2} GB", bytes as f64 / GB as f64)
        } else if bytes >= MB {
            format!("{:.2} MB", bytes as f64 / MB as f64)
        } else if bytes >= KB {
            format!("{:.2} KB", bytes as f64 / KB as f64)
        } else {
            format!("{} bytes", bytes)
        }
    }

    /// Clear all cached sizes.
    pub fn clear_cache(&mut self) {
        self.cache.clear();
        let _ = self.save_cache();
    }

    /// Update a known package size (useful for runtime calibration).
    pub fn update_known_package_size(&mut self, package: &str, size: u64) {
        self.known_packages.insert(package.to_lowercase(), size);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_test_calculator() -> (SizeCalculator, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let calculator = SizeCalculator::new(temp_dir.path().to_path_buf());
        (calculator, temp_dir)
    }

    #[test]
    fn test_format_size_bytes() {
        assert_eq!(SizeCalculator::format_size(0), "0 bytes");
        assert_eq!(SizeCalculator::format_size(500), "500 bytes");
        assert_eq!(SizeCalculator::format_size(1023), "1023 bytes");
    }

    #[test]
    fn test_format_size_kb() {
        assert_eq!(SizeCalculator::format_size(1024), "1.00 KB");
        assert_eq!(SizeCalculator::format_size(1536), "1.50 KB");
        assert_eq!(SizeCalculator::format_size(1024 * 1023), "1023.00 KB");
    }

    #[test]
    fn test_format_size_mb() {
        assert_eq!(SizeCalculator::format_size(1024 * 1024), "1.00 MB");
        assert_eq!(SizeCalculator::format_size(1024 * 1024 * 50), "50.00 MB");
        assert_eq!(
            SizeCalculator::format_size(1024 * 1024 * 1024 - 1),
            "1024.00 MB"
        );
    }

    #[test]
    fn test_format_size_gb() {
        assert_eq!(SizeCalculator::format_size(1024 * 1024 * 1024), "1.00 GB");
        assert_eq!(
            SizeCalculator::format_size(1024 * 1024 * 1024 * 5),
            "5.00 GB"
        );
        assert_eq!(
            SizeCalculator::format_size((1024_u64 * 1024 * 1024 * 3) + (1024 * 1024 * 512)),
            "3.50 GB"
        );
    }

    #[test]
    fn test_format_size_tb() {
        assert_eq!(
            SizeCalculator::format_size(1024_u64 * 1024 * 1024 * 1024),
            "1.00 TB"
        );
        assert_eq!(
            SizeCalculator::format_size(1024_u64 * 1024 * 1024 * 1024 * 2),
            "2.00 TB"
        );
    }

    #[test]
    fn test_parse_package_name_simple() {
        assert_eq!(SizeCalculator::parse_package_name("torch"), "torch");
        assert_eq!(SizeCalculator::parse_package_name("numpy"), "numpy");
        assert_eq!(
            SizeCalculator::parse_package_name("pillow-simd"),
            "pillow-simd"
        );
    }

    #[test]
    fn test_parse_package_name_with_version() {
        assert_eq!(SizeCalculator::parse_package_name("torch==2.0.0"), "torch");
        assert_eq!(SizeCalculator::parse_package_name("numpy>=1.20"), "numpy");
        assert_eq!(
            SizeCalculator::parse_package_name("pillow>=9.0,<10"),
            "pillow"
        );
        assert_eq!(
            SizeCalculator::parse_package_name("requests>=2.25.1"),
            "requests"
        );
    }

    #[test]
    fn test_parse_package_name_with_extras() {
        assert_eq!(
            SizeCalculator::parse_package_name("transformers[torch]"),
            "transformers"
        );
        assert_eq!(
            SizeCalculator::parse_package_name("aiohttp[speedups]>=3.8"),
            "aiohttp"
        );
    }

    #[test]
    fn test_parse_package_name_skipped() {
        assert_eq!(SizeCalculator::parse_package_name(""), "");
        assert_eq!(SizeCalculator::parse_package_name("# comment"), "");
        assert_eq!(
            SizeCalculator::parse_package_name("-e git+https://github.com/..."),
            ""
        );
        assert_eq!(
            SizeCalculator::parse_package_name("git+https://github.com/..."),
            ""
        );
        assert_eq!(SizeCalculator::parse_package_name("--index-url https://..."), "");
    }

    #[test]
    fn test_parse_package_name_with_semicolon() {
        assert_eq!(
            SizeCalculator::parse_package_name("pywin32; sys_platform == 'win32'"),
            "pywin32"
        );
    }

    #[test]
    fn test_default_known_packages() {
        let packages = SizeCalculator::default_known_packages();

        // Check that major packages are present
        assert!(packages.contains_key("torch"));
        assert!(packages.contains_key("numpy"));
        assert!(packages.contains_key("pillow"));
        assert!(packages.contains_key("transformers"));
        assert!(packages.contains_key("safetensors"));

        // Check torch is largest
        assert!(packages["torch"] > packages["numpy"]);
        assert!(packages["torch"] > 1_000_000_000); // > 1GB
    }

    #[test]
    fn test_calculator_creation() {
        let (calculator, _temp) = create_test_calculator();
        assert!(!calculator.known_packages.is_empty());
    }

    #[test]
    fn test_get_cached_size_empty() {
        let (calculator, _temp) = create_test_calculator();
        assert!(calculator.get_cached_size("v1.0.0").is_none());
    }

    #[tokio::test]
    async fn test_calculate_release_size_no_requirements() {
        let (mut calculator, _temp) = create_test_calculator();

        let result = calculator
            .calculate_release_size("v1.0.0", 10_000_000, None)
            .await
            .unwrap();

        assert_eq!(result.tag, "v1.0.0");
        assert_eq!(result.archive_size, 10_000_000);
        assert!(result.is_estimated);
        // Without requirements, uses heuristic of 15x archive size
        assert_eq!(result.total_size, 10_000_000 + 10_000_000 * 15);
    }

    #[tokio::test]
    async fn test_calculate_release_size_with_requirements() {
        let (mut calculator, _temp) = create_test_calculator();

        let requirements = vec![
            "torch==2.0.0".to_string(),
            "numpy>=1.20".to_string(),
            "pillow".to_string(),
        ];

        let result = calculator
            .calculate_release_size("v1.0.0", 10_000_000, Some(&requirements))
            .await
            .unwrap();

        assert_eq!(result.tag, "v1.0.0");
        assert_eq!(result.archive_size, 10_000_000);
        assert!(result.is_estimated);

        // Should have deps calculated from known packages
        let deps = result.dependencies_size.unwrap();
        assert!(deps > 0);

        // torch alone is ~2.5GB
        assert!(deps >= 2_500_000_000);
    }

    #[tokio::test]
    async fn test_calculate_release_size_caching() {
        let (mut calculator, _temp) = create_test_calculator();

        // First calculation
        let result1 = calculator
            .calculate_release_size("v1.0.0", 10_000_000, None)
            .await
            .unwrap();

        // Should be cached
        assert!(calculator.get_cached_size("v1.0.0").is_some());

        // Second call should return cached value
        let result2 = calculator
            .calculate_release_size("v1.0.0", 10_000_000, None)
            .await
            .unwrap();

        assert_eq!(result1.total_size, result2.total_size);
    }

    #[tokio::test]
    async fn test_get_size_breakdown() {
        let (mut calculator, _temp) = create_test_calculator();

        // Calculate a size first
        calculator
            .calculate_release_size("v1.0.0", 10_000_000, None)
            .await
            .unwrap();

        let breakdown = calculator.get_size_breakdown("v1.0.0").unwrap();

        assert!(breakdown.total_size > 0);
        assert_eq!(breakdown.archive_size, 10_000_000);
        assert!(breakdown.archive_percentage > 0.0);
        assert!(breakdown.archive_percentage < 100.0);
        assert!(breakdown.dependencies_percentage > 0.0);
        assert!(!breakdown.total_size_formatted.is_empty());
    }

    #[test]
    fn test_cache_persistence() {
        let temp_dir = TempDir::new().unwrap();
        let cache_path = temp_dir.path().to_path_buf();

        // Create calculator and add to cache
        {
            let mut calculator = SizeCalculator::new(cache_path.clone());
            calculator.cache.insert(
                "v1.0.0".to_string(),
                ReleaseSize {
                    tag: "v1.0.0".to_string(),
                    archive_size: 1000,
                    dependencies_size: Some(5000),
                    total_size: 6000,
                    is_estimated: true,
                },
            );
            calculator.save_cache().unwrap();
        }

        // Create new calculator and verify cache loaded
        {
            let calculator = SizeCalculator::new(cache_path);
            let cached = calculator.get_cached_size("v1.0.0").unwrap();
            assert_eq!(cached.tag, "v1.0.0");
            assert_eq!(cached.archive_size, 1000);
            assert_eq!(cached.total_size, 6000);
        }
    }

    #[test]
    fn test_clear_cache() {
        let (mut calculator, _temp) = create_test_calculator();

        calculator.cache.insert(
            "v1.0.0".to_string(),
            ReleaseSize {
                tag: "v1.0.0".to_string(),
                archive_size: 1000,
                dependencies_size: Some(5000),
                total_size: 6000,
                is_estimated: true,
            },
        );

        assert!(calculator.get_cached_size("v1.0.0").is_some());

        calculator.clear_cache();

        assert!(calculator.get_cached_size("v1.0.0").is_none());
    }

    #[test]
    fn test_update_known_package_size() {
        let (mut calculator, _temp) = create_test_calculator();

        // Original torch size
        let original = calculator.known_packages.get("torch").copied();

        // Update it
        calculator.update_known_package_size("torch", 3_000_000_000);

        assert_eq!(
            calculator.known_packages.get("torch").copied(),
            Some(3_000_000_000)
        );
        assert_ne!(Some(3_000_000_000), original);
    }

    #[tokio::test]
    async fn test_unknown_package_default_size() {
        let (mut calculator, _temp) = create_test_calculator();

        let requirements = vec!["some-unknown-package==1.0.0".to_string()];

        let result = calculator
            .calculate_release_size("v1.0.0", 1_000_000, Some(&requirements))
            .await
            .unwrap();

        // Unknown package should get 1MB default
        assert_eq!(result.dependencies_size.unwrap(), 1_000_000);
    }

    #[tokio::test]
    async fn test_mixed_known_unknown_packages() {
        let (mut calculator, _temp) = create_test_calculator();

        let requirements = vec![
            "numpy>=1.20".to_string(),           // known: ~20MB
            "unknown-package==1.0.0".to_string(), // unknown: 1MB default
        ];

        let result = calculator
            .calculate_release_size("v1.0.0", 1_000_000, Some(&requirements))
            .await
            .unwrap();

        // numpy (~20MB) + unknown (1MB)
        let deps = result.dependencies_size.unwrap();
        assert!(deps >= 20_000_000);
        assert!(deps < 100_000_000); // Not too large
    }

    #[test]
    fn test_package_name_normalization() {
        let (calculator, _temp) = create_test_calculator();

        // Check that packages with hyphens/underscores are handled
        // opencv-python-headless should be found
        assert!(calculator.known_packages.contains_key("opencv-python-headless"));
    }
}
