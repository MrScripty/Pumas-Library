//! Version metadata types.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Information about an installed version.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct VersionInfo {
    pub path: String,
    #[serde(default)]
    pub installed_date: Option<String>,
    #[serde(default)]
    pub python_version: Option<String>,
    #[serde(default)]
    pub git_commit: Option<String>,
    #[serde(default)]
    pub release_tag: Option<String>,
    #[serde(default)]
    pub release_date: Option<String>,
    #[serde(default)]
    pub release_notes: Option<String>,
    #[serde(default)]
    pub download_url: Option<String>,
    #[serde(default)]
    pub size: Option<u64>,
    #[serde(default)]
    pub requirements_hash: Option<String>,
    #[serde(default)]
    pub dependencies_installed: Option<bool>,
}

/// Root metadata structure for versions.json.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct VersionsMetadata {
    /// Map of tag -> version info
    #[serde(default)]
    pub installed: HashMap<String, VersionInfo>,
    #[serde(default)]
    pub last_selected_version: Option<String>,
    #[serde(default)]
    pub default_version: Option<String>,
}

/// Version status for a specific installed version.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionStatus {
    pub is_active: bool,
    pub dependencies: DependencyStatus,
}

/// Status of version dependencies.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DependencyStatus {
    #[serde(default)]
    pub installed: Vec<String>,
    #[serde(default)]
    pub missing: Vec<String>,
    #[serde(default)]
    pub requirements_file: Option<String>,
}

/// Installation progress stage.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum InstallationStage {
    Download,
    Extract,
    Venv,
    Dependencies,
    Setup,
}

impl InstallationStage {
    /// Get the weight of this stage for progress calculation.
    pub fn weight(&self) -> f32 {
        match self {
            InstallationStage::Download => 0.15,
            InstallationStage::Extract => 0.05,
            InstallationStage::Venv => 0.05,
            InstallationStage::Dependencies => 0.70,
            InstallationStage::Setup => 0.05,
        }
    }

    /// Get the cumulative weight up to and including this stage.
    pub fn cumulative_weight(&self) -> f32 {
        match self {
            InstallationStage::Download => 0.15,
            InstallationStage::Extract => 0.20,
            InstallationStage::Venv => 0.25,
            InstallationStage::Dependencies => 0.95,
            InstallationStage::Setup => 1.00,
        }
    }
}

/// Progress item for tracking completed work.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallationProgressItem {
    pub name: String,
    #[serde(rename = "type")]
    pub item_type: String,
    pub size: Option<u64>,
    pub completed_at: String,
}

/// Installation progress information.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct InstallationProgress {
    #[serde(default)]
    pub tag: Option<String>,
    #[serde(default)]
    pub started_at: Option<String>,
    #[serde(default)]
    pub stage: Option<InstallationStage>,
    #[serde(default)]
    pub stage_progress: Option<f32>,
    #[serde(default)]
    pub overall_progress: Option<f32>,
    #[serde(default)]
    pub current_item: Option<String>,
    #[serde(default)]
    pub download_speed: Option<f64>,
    #[serde(default)]
    pub eta_seconds: Option<f64>,
    #[serde(default)]
    pub total_size: Option<u64>,
    #[serde(default)]
    pub downloaded_bytes: Option<u64>,
    #[serde(default)]
    pub dependency_count: Option<u32>,
    #[serde(default)]
    pub completed_dependencies: Option<u32>,
    #[serde(default)]
    pub completed_items: Option<Vec<InstallationProgressItem>>,
    #[serde(default)]
    pub error: Option<String>,
    #[serde(default)]
    pub completed_at: Option<String>,
    #[serde(default)]
    pub success: Option<bool>,
    #[serde(default)]
    pub log_path: Option<String>,
}

impl InstallationProgress {
    /// Create a new progress tracker for a version.
    pub fn new(tag: &str) -> Self {
        Self {
            tag: Some(tag.to_string()),
            started_at: Some(chrono::Utc::now().to_rfc3339()),
            stage: Some(InstallationStage::Download),
            stage_progress: Some(0.0),
            overall_progress: Some(0.0),
            ..Default::default()
        }
    }

    /// Mark the installation as completed successfully.
    pub fn complete_success(&mut self) {
        self.completed_at = Some(chrono::Utc::now().to_rfc3339());
        self.success = Some(true);
        self.overall_progress = Some(100.0);
        self.stage_progress = Some(100.0);
    }

    /// Mark the installation as failed.
    pub fn complete_error(&mut self, error: &str) {
        self.completed_at = Some(chrono::Utc::now().to_rfc3339());
        self.success = Some(false);
        self.error = Some(error.to_string());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_installation_stage_weights() {
        let total: f32 = [
            InstallationStage::Download,
            InstallationStage::Extract,
            InstallationStage::Venv,
            InstallationStage::Dependencies,
            InstallationStage::Setup,
        ]
        .iter()
        .map(|s| s.weight())
        .sum();

        assert!((total - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_version_info_serialization() {
        let info = VersionInfo {
            path: "/path/to/version".into(),
            installed_date: Some("2024-01-01T00:00:00Z".into()),
            release_tag: Some("v1.0.0".into()),
            ..Default::default()
        };

        let json = serde_json::to_string(&info).unwrap();
        assert!(json.contains("installedDate"));
        assert!(json.contains("releaseTag"));
    }
}
