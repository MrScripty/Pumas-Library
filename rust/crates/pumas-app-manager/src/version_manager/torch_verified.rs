//! Torch runtime combinations Pumas has verified, kept as evidence.
//!
//! A record says which Torch release, build, Python and adapters worked
//! together, with which package versions, and when. It never constrains an
//! installation: packages are resolved when the user installs, so a
//! selection matches a record only when its resolved packages still carry the
//! recorded versions.

use super::TorchArtifact;
use serde::Deserialize;
use std::collections::BTreeMap;
use std::sync::LazyLock;

/// One verified combination, in the install dialog's selection terms.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TorchVerifiedRuntime {
    /// Release tag, such as `v2.9.1`.
    pub tag: String,
    pub build: String,
    /// Managed Python choice, such as `python3.12`.
    pub python: String,
    pub adapters: Vec<String>,
    /// ISO date of the verification.
    pub verified_on: String,
    /// What the verification exercised.
    pub scope: String,
    /// Source commit the verification ran against.
    pub source_commit: String,
    /// Direct package versions present when it was verified.
    pub packages: Vec<TorchVerifiedPackage>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TorchVerifiedPackage {
    pub name: String,
    pub version: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RecordFile {
    schema_version: u32,
    records: Vec<Record>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Record {
    torch: String,
    build: String,
    python: String,
    platform: String,
    adapters: Vec<String>,
    verified_on: String,
    source_commit: String,
    scope: String,
    packages: BTreeMap<String, String>,
}

const HOST_PLATFORM: &str = if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
    "linux-x86_64"
} else if cfg!(all(target_os = "windows", target_arch = "x86_64")) {
    "windows-x86_64"
} else if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
    "macos-arm64"
} else {
    "unsupported"
};

static RECORDS: LazyLock<Vec<TorchVerifiedRuntime>> = LazyLock::new(|| {
    parse(
        include_str!("../../../../../torch-server/runtime/verified-runtimes.json"),
        HOST_PLATFORM,
    )
    .expect("source-controlled verified Torch runtime records")
});

fn parse(source: &str, platform: &str) -> Result<Vec<TorchVerifiedRuntime>, String> {
    let file: RecordFile = serde_json::from_str(source).map_err(|error| error.to_string())?;
    if file.schema_version != 1 {
        return Err(format!(
            "unsupported verified runtime schema {}",
            file.schema_version
        ));
    }
    Ok(file
        .records
        .into_iter()
        .filter(|record| record.platform == platform)
        .map(|record| TorchVerifiedRuntime {
            tag: format!("v{}", record.torch),
            build: record.build,
            python: format!("python{}", record.python),
            adapters: record.adapters,
            verified_on: record.verified_on,
            scope: record.scope,
            source_commit: record.source_commit,
            packages: record
                .packages
                .into_iter()
                .map(|(name, version)| TorchVerifiedPackage { name, version })
                .collect(),
        })
        .collect())
}

/// Verified combinations for this host's platform.
pub(crate) fn verified_torch_runtimes() -> Vec<TorchVerifiedRuntime> {
    RECORDS.clone()
}

/// Whether a resolved or installed runtime is a verified combination: the
/// selection matches a record and every recorded package it contains has the
/// recorded version. Torch must be among the artifacts.
pub(crate) fn is_verified(
    tag: &str,
    build: &str,
    python: &str,
    adapter: &str,
    artifacts: &[TorchArtifact],
) -> bool {
    matches(&RECORDS, tag, build, python, adapter, artifacts)
}

fn matches(
    records: &[TorchVerifiedRuntime],
    tag: &str,
    build: &str,
    python: &str,
    adapter: &str,
    artifacts: &[TorchArtifact],
) -> bool {
    let version_of = |name: &str| {
        artifacts
            .iter()
            .find(|artifact| artifact.name.eq_ignore_ascii_case(name))
            .map(|artifact| artifact.version.as_str())
    };
    version_of("torch").is_some()
        && records.iter().any(|record| {
            record.tag == tag
                && record.build == build
                && record.python == python
                && record.adapters.iter().any(|recorded| recorded == adapter)
                && record.packages.iter().all(|package| {
                    version_of(&package.name).is_none_or(|version| version == package.version)
                })
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn artifact(name: &str, version: &str) -> TorchArtifact {
        TorchArtifact {
            name: name.into(),
            version: version.into(),
            url: format!("https://files.pythonhosted.org/packages/{name}.whl"),
            sha256: "a".repeat(64),
        }
    }

    #[test]
    fn the_source_record_parses_for_every_platform() {
        let source = include_str!("../../../../../torch-server/runtime/verified-runtimes.json");
        let linux = parse(source, "linux-x86_64").unwrap();
        assert_eq!(linux[0].tag, "v2.9.1");
        assert_eq!(linux[0].python, "python3.12");
        assert!(parse(source, "macos-arm64").unwrap().is_empty());
        assert!(parse(
            &source.replace("\"schema_version\": 1", "\"schema_version\": 2"),
            "linux-x86_64"
        )
        .is_err());
    }

    #[test]
    fn a_selection_is_verified_only_with_the_recorded_package_versions() {
        let source = include_str!("../../../../../torch-server/runtime/verified-runtimes.json");
        let records = parse(source, "linux-x86_64").unwrap();
        let recorded = [
            artifact("torch", "2.9.1+cu130"),
            artifact("diffusers", "0.37.0"),
            artifact("Pillow", "12.1.0"),
            artifact("filelock", "3.20.0"),
        ];
        let check = |tag, build, python, adapter, artifacts: &[TorchArtifact]| {
            matches(&records, tag, build, python, adapter, artifacts)
        };
        assert!(check("v2.9.1", "cu130", "python3.12", "flux2", &recorded));
        assert!(check(
            "v2.9.1",
            "cu130",
            "python3.12",
            "nunchaku",
            &recorded
        ));
        assert!(!check("v2.9.1", "cu130", "python3.12", "none", &recorded));
        assert!(!check("v2.9.1", "cpu", "python3.12", "flux2", &recorded));
        assert!(!check("v2.9.1", "cu130", "python3.13", "flux2", &recorded));
        assert!(!check("v2.10.0", "cu130", "python3.12", "flux2", &recorded));

        let mut newer = recorded.to_vec();
        newer[2] = artifact("pillow", "13.0.0");
        assert!(!check("v2.9.1", "cu130", "python3.12", "flux2", &newer));
        assert!(!check(
            "v2.9.1",
            "cu130",
            "python3.12",
            "flux2",
            &recorded[1..]
        ));
    }
}
