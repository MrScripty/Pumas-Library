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

/// Whether a listed build can be installed on this host, as far as the
/// listing knows. Only builds whose installability depends on more than their
/// archive (ONNX Runtime CUDA builds, which also need NVIDIA's libraries)
/// carry one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "contract-schema", derive(schemars::JsonSchema))]
pub enum InstallAvailability {
    /// The lookup is still running; list the versions again for its result.
    Checking,
    /// The build installs.
    Available,
    /// The build cannot install; `unavailable_reason` says why.
    Unavailable,
    /// The lookup could not reach the package indexes. Installing still
    /// checks, and decides.
    Unknown,
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
    /// Why this listed build cannot be installed on this host, when the
    /// listing knows it before an install is attempted. Computed per listing;
    /// never part of the publisher's release data.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unavailable_reason: Option<String>,
    /// Whether this build installs, for builds where the listing checks it.
    /// Computed per listing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub install_availability: Option<InstallAvailability>,
}

impl GitHubRelease {
    /// Whether this is a pre-release: the publisher flagged it, or its tag is
    /// a semantic version with a pre-release part (`v0.21.3-rc0`). Some
    /// projects publish release candidates without the flag.
    pub fn is_prerelease(&self) -> bool {
        self.prerelease
            || semver::Version::parse(self.tag_name.strip_prefix('v').unwrap_or(&self.tag_name))
                .is_ok_and(|version| !version.pre.is_empty())
    }
}

/// Version release info as returned to frontend.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "contract-schema", derive(schemars::JsonSchema))]
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
    /// Why this build cannot be installed on this host, or `null` when it can
    /// (or the backend cannot tell yet). An unavailable build is listed so the
    /// user sees the reason and the builds that install instead.
    #[serde(default)]
    pub unavailable_reason: Option<String>,
    /// Whether this build installs, for builds the backend checks before an
    /// install (ONNX Runtime CUDA builds); `null` for every other build.
    #[serde(default)]
    pub install_availability: Option<InstallAvailability>,
}

/// Release asset for frontend.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "contract-schema", derive(schemars::JsonSchema))]
pub struct VersionReleaseAsset {
    pub name: String,
    pub size: u64,
    pub download_url: String,
}

impl From<GitHubRelease> for VersionReleaseInfo {
    fn from(release: GitHubRelease) -> Self {
        let prerelease = release.is_prerelease();
        Self {
            tag_name: release.tag_name,
            name: release.name,
            published_at: release.published_at,
            prerelease,
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
            unavailable_reason: release.unavailable_reason,
            install_availability: release.install_availability,
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
            unavailable_reason: None,
            install_availability: None,
        };

        let info: VersionReleaseInfo = release.clone().into();
        assert_eq!(info.tag_name, "v1.0.0");
        assert_eq!(info.assets.len(), 1);
        assert_eq!(info.total_size, Some(2048));
        assert!(!info.prerelease);

        let candidate = GitHubRelease {
            tag_name: "v0.21.3-rc0".into(),
            ..release.clone()
        };
        assert!(candidate.is_prerelease());
        assert!(VersionReleaseInfo::from(candidate).prerelease);
        for (tag, flagged, expected) in [
            ("0.21.3-rc.1", false, true),
            ("v1.0.0+build.5", false, false),
            ("b12129", false, false),
            ("b12129", true, true),
            ("v2.9.1", true, true),
        ] {
            let release = GitHubRelease {
                tag_name: tag.into(),
                prerelease: flagged,
                ..release.clone()
            };
            assert_eq!(release.is_prerelease(), expected, "{tag}");
        }
    }
}
