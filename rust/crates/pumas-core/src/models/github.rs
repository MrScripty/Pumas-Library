//! GitHub release metadata types.

use serde::{Deserialize, Serialize};

/// GitHub release asset information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitHubAsset {
    pub name: String,
    pub size: u64,
    #[serde(rename = "browser_download_url")]
    pub download_url: String,
    #[serde(default)]
    pub content_type: Option<String>,
}

/// GitHub release information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitHubRelease {
    pub tag_name: String,
    pub name: String,
    pub published_at: String,
    #[serde(default)]
    pub body: Option<String>,
    #[serde(default)]
    pub tarball_url: Option<String>,
    #[serde(default)]
    pub zipball_url: Option<String>,
    #[serde(default)]
    pub prerelease: bool,
    #[serde(default)]
    pub assets: Vec<GitHubAsset>,
    pub html_url: String,
    // Size information (computed/cached)
    #[serde(default)]
    pub total_size: Option<u64>,
    #[serde(default)]
    pub archive_size: Option<u64>,
    #[serde(default)]
    pub dependencies_size: Option<u64>,
}

/// Version release info as returned to frontend.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionReleaseInfo {
    pub tag_name: String,
    pub name: String,
    pub published_at: String,
    #[serde(default)]
    pub prerelease: bool,
    #[serde(default)]
    pub body: Option<String>,
    pub html_url: String,
    #[serde(default)]
    pub assets: Vec<VersionReleaseAsset>,
    #[serde(default)]
    pub total_size: Option<u64>,
    #[serde(default)]
    pub archive_size: Option<u64>,
    #[serde(default)]
    pub dependencies_size: Option<u64>,
    #[serde(default)]
    pub installing: Option<bool>,
}

/// Release asset for frontend.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionReleaseAsset {
    pub name: String,
    pub size: u64,
    pub download_url: String,
}

impl From<GitHubRelease> for VersionReleaseInfo {
    fn from(release: GitHubRelease) -> Self {
        Self {
            tag_name: release.tag_name,
            name: release.name,
            published_at: release.published_at,
            prerelease: release.prerelease,
            body: release.body,
            html_url: release.html_url,
            assets: release
                .assets
                .into_iter()
                .map(|a| VersionReleaseAsset {
                    name: a.name,
                    size: a.size,
                    download_url: a.download_url,
                })
                .collect(),
            total_size: release.total_size,
            archive_size: release.archive_size,
            dependencies_size: release.dependencies_size,
            installing: None,
        }
    }
}

/// Cached GitHub releases.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitHubReleasesCache {
    pub last_fetched: String,
    pub ttl: u64,
    pub releases: Vec<GitHubRelease>,
}

/// GitHub cache status for frontend.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CacheStatus {
    pub has_cache: bool,
    pub is_valid: bool,
    pub is_fetching: bool,
    #[serde(default)]
    pub age_seconds: Option<u64>,
    #[serde(default)]
    pub last_fetched: Option<String>,
    #[serde(default)]
    pub releases_count: Option<u32>,
}

/// Release size breakdown information.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReleaseSizeBreakdown {
    #[serde(default)]
    pub archive_size: Option<u64>,
    #[serde(default)]
    pub dependencies_size: Option<u64>,
    #[serde(default)]
    pub total_size: Option<u64>,
    #[serde(default)]
    pub dependency_details: Option<Vec<DependencySizeInfo>>,
}

/// Size information for a single dependency.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DependencySizeInfo {
    pub name: String,
    pub version: String,
    pub size: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_github_release_to_version_info() {
        let release = GitHubRelease {
            tag_name: "v1.0.0".into(),
            name: "Release 1.0.0".into(),
            published_at: "2024-01-01T00:00:00Z".into(),
            body: Some("Release notes".into()),
            tarball_url: None,
            zipball_url: None,
            prerelease: false,
            assets: vec![GitHubAsset {
                name: "source.zip".into(),
                size: 1024,
                download_url: "https://example.com/source.zip".into(),
                content_type: None,
            }],
            html_url: "https://github.com/test/repo/releases/v1.0.0".into(),
            total_size: Some(2048),
            archive_size: Some(1024),
            dependencies_size: Some(1024),
        };

        let info: VersionReleaseInfo = release.into();
        assert_eq!(info.tag_name, "v1.0.0");
        assert_eq!(info.assets.len(), 1);
        assert_eq!(info.total_size, Some(2048));
    }
}
