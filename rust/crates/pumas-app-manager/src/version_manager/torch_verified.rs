//! Torch runtime combinations Pumas has verified, kept as evidence.
//!
//! A record says which Torch release, build, Python and optional extras worked
//! together, with which package versions, and when. Records name adapters
//! from before every install carried all general modalities: a `nunchaku`
//! record is the Nunchaku extra, and any other record (such as `flux2`) is a
//! general install whose recorded packages were verified. It does not constrain an
//! installation unless the user opts into its versions: packages are
//! otherwise resolved when the user installs. "Verified" is a version match:
//! a selection matches a record only when its resolved packages include every
//! recorded package at the recorded version. It is not a hash or provenance
//! claim; install integrity comes from the per-install hash lock.

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
    /// Optional extras the record verified, derived from `adapters`.
    pub extras: Vec<String>,
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
            extras: record_extras(&record.adapters),
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

/// The optional extras a record's legacy adapter names stand for.
fn record_extras(adapters: &[String]) -> Vec<String> {
    if adapters.iter().any(|adapter| adapter == "nunchaku") {
        vec!["nunchaku".to_owned()]
    } else {
        Vec::new()
    }
}

/// Verified combinations for this host's platform.
pub(crate) fn verified_torch_runtimes() -> Vec<TorchVerifiedRuntime> {
    RECORDS.clone()
}

/// pip constraints pinning the recorded package versions of the verified
/// combination that matches this exact selection, or `None` when no record
/// matches it.
pub(crate) fn verified_constraints(
    tag: &str,
    build: &str,
    python: &str,
    extras: &[String],
) -> Option<String> {
    constraints_for(&RECORDS, tag, build, python, extras)
}

fn constraints_for(
    records: &[TorchVerifiedRuntime],
    tag: &str,
    build: &str,
    python: &str,
    extras: &[String],
) -> Option<String> {
    let record = records.iter().find(|record| {
        record.tag == tag
            && record.build == build
            && record.python == python
            && record.extras == extras
    })?;
    Some(
        record
            .packages
            .iter()
            .map(|package| format!("{}=={}\n", package.name, package.version))
            .collect(),
    )
}

/// Whether a resolved or installed runtime is a verified combination: a
/// version match. The selection matches a record, and every recorded package
/// is among the artifacts at the recorded version; a recorded package that is
/// absent fails the match.
pub(crate) fn is_verified(
    tag: &str,
    build: &str,
    python: &str,
    extras: &[String],
    artifacts: &[TorchArtifact],
) -> bool {
    matches(&RECORDS, tag, build, python, extras, artifacts)
}

fn matches(
    records: &[TorchVerifiedRuntime],
    tag: &str,
    build: &str,
    python: &str,
    extras: &[String],
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
                && record.extras == extras
                && record
                    .packages
                    .iter()
                    .all(|package| version_of(&package.name) == Some(package.version.as_str()))
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

    /// Every package a record names, at its recorded version, plus an
    /// unrecorded transitive dependency that a match ignores.
    fn installed_as_recorded(record: &TorchVerifiedRuntime) -> Vec<TorchArtifact> {
        record
            .packages
            .iter()
            .map(|package| artifact(&package.name, &package.version))
            .chain([artifact("filelock", "3.20.0")])
            .collect()
    }

    #[test]
    fn a_selection_is_verified_only_as_a_version_match_of_every_recorded_package() {
        let source = include_str!("../../../../../torch-server/runtime/verified-runtimes.json");
        let records = parse(source, "linux-x86_64").unwrap();
        let record_for = |adapter: &str| {
            records
                .iter()
                .find(|record| record.adapters.iter().any(|recorded| recorded == adapter))
                .unwrap()
        };
        let flux2 = installed_as_recorded(record_for("flux2"));
        let nunchaku = installed_as_recorded(record_for("nunchaku"));
        let general: &[String] = &[];
        let with_nunchaku = &["nunchaku".to_owned()];
        let check = |tag, build, python, extras: &[String], artifacts: &[TorchArtifact]| {
            matches(&records, tag, build, python, extras, artifacts)
        };
        // A general install (no extras) is checked against the former Flux 2
        // record, whose image packages every install now carries.
        assert!(check("v2.9.1", "cu130", "python3.12", general, &flux2));
        assert!(check(
            "v2.9.1",
            "cu130",
            "python3.12",
            with_nunchaku,
            &nunchaku
        ));
        // A general install has no Nunchaku wheel, so it cannot match that record.
        assert!(!check(
            "v2.9.1",
            "cu130",
            "python3.12",
            with_nunchaku,
            &flux2
        ));
        assert!(!check("v2.9.1", "cpu", "python3.12", general, &flux2));
        assert!(!check("v2.9.1", "cu130", "python3.13", general, &flux2));
        assert!(!check("v2.10.0", "cu130", "python3.12", general, &flux2));

        let mut newer = flux2.clone();
        let pillow = newer.iter().position(|item| item.name == "pillow").unwrap();
        newer[pillow] = artifact("Pillow", "13.0.0");
        assert!(!check("v2.9.1", "cu130", "python3.12", general, &newer));

        // An absent recorded package fails the match.
        let mut missing = flux2.clone();
        missing.retain(|item| item.name != "diffusers");
        assert!(!check("v2.9.1", "cu130", "python3.12", general, &missing));
        assert_eq!(record_for("nunchaku").extras, vec!["nunchaku".to_owned()]);
        assert!(record_for("flux2").extras.is_empty());
    }

    #[test]
    fn verified_constraints_pin_exactly_the_matching_record() {
        let source = include_str!("../../../../../torch-server/runtime/verified-runtimes.json");
        let records = parse(source, "linux-x86_64").unwrap();
        let nunchaku = &["nunchaku".to_owned()];
        let constraints =
            constraints_for(&records, "v2.9.1", "cu130", "python3.12", nunchaku).unwrap();
        assert!(constraints.contains("torch==2.9.1+cu130\n"));
        assert!(constraints.contains("nunchaku==1.2.0+torch2.9\n"));
        assert!(
            !constraints_for(&records, "v2.9.1", "cu130", "python3.12", &[])
                .unwrap()
                .contains("nunchaku")
        );
        assert!(constraints_for(&records, "v2.9.1", "auto", "python3.12", &[]).is_none());
        assert!(constraints_for(&records, "v2.9.1", "cu130", "python3.13", &[]).is_none());
    }
}
