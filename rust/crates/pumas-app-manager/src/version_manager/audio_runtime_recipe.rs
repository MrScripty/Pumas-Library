//! Source-fixed candidate recipe identity, never production qualification.
//! Expected values came from the official managed CPU installation cohort.
//! Acquisition provenance is retained separately from installed-file ownership.

use pumas_library::runtime_read_source::RetainedRuntimeReadSource;
use serde::Deserialize;
use serde_json::Value;
use std::io;

const PIN: &str = pumas_library::runtime_read_source::AUDIO_RUNTIME_CANDIDATE_RECIPE;
#[derive(Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(deny_unknown_fields)]
struct Artifact {
    name: String,
    version: String,
    url: String,
    sha256: String,
}
fn refused() -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        "selected runtime differs from source-pinned audio candidate recipe",
    )
}
fn artifacts(value: &Value) -> io::Result<Vec<Artifact>> {
    let mut artifacts: Vec<Artifact> =
        serde_json::from_value(value.clone()).map_err(|_| refused())?;
    artifacts.sort();
    if artifacts.len() != 68
        || artifacts
            .windows(2)
            .any(|pair| pair[0].name == pair[1].name)
    {
        return Err(refused());
    }
    Ok(artifacts)
}
// Only this source-fixed Linux candidate has an exact dependency recipe.
// Other supported Torch selections retain their existing resolution policy.
fn install_recipe(
    version: &str,
    build: &str,
    minor: &str,
    adapter: &str,
) -> io::Result<Option<Vec<Artifact>>> {
    if !cfg!(all(
        target_os = "linux",
        target_arch = "x86_64",
        target_env = "gnu"
    )) {
        return Ok(None);
    }
    let pin: Value = serde_json::from_str(PIN).map_err(|_| refused())?;
    if pin["python"] != format!("python{minor}")
        || pin["build"] != build
        || pin["adapter"] != adapter
    {
        return Ok(None);
    }
    let expected = artifacts(&pin["artifacts"])?;
    if !expected
        .iter()
        .any(|a| a.name == "torch" && a.version == format!("{version}+{build}"))
    {
        return Ok(None);
    }
    Ok(Some(expected))
}

/// Constrain the normal index-based install without changing wheel provenance
/// metadata by turning dependencies into direct URL requirements.
pub(super) fn install_constraints(
    version: &str,
    build: &str,
    minor: &str,
    adapter: &str,
) -> io::Result<Option<String>> {
    let Some(expected) = install_recipe(version, build, minor, adapter)? else {
        return Ok(None);
    };
    let mut constraints = String::new();
    for artifact in expected {
        if artifact.name.is_empty()
            || artifact.version.is_empty()
            || !artifact
                .name
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"-.".contains(&b))
            || !artifact
                .version
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b".+!-".contains(&b))
        {
            return Err(refused());
        }
        constraints.push_str(&format!("{}=={}\n", artifact.name, artifact.version));
    }
    Ok(Some(constraints))
}

/// Gate staged package publication independently of pip's constraint handling.
pub(super) fn validate_install_artifacts(
    version: &str,
    build: &str,
    minor: &str,
    adapter: &str,
    selected: &Value,
) -> io::Result<()> {
    if let Some(expected) = install_recipe(version, build, minor, adapter)? {
        if artifacts(selected)? != expected {
            return Err(refused());
        }
    }
    Ok(())
}

pub(super) fn validate(metadata: &Value, retained: &RetainedRuntimeReadSource) -> io::Result<()> {
    let pin: Value = serde_json::from_str(PIN).map_err(|_| refused())?;
    for field in ["python", "adapter", "build"] {
        if metadata[field] != pin[field] {
            return Err(refused());
        }
    }
    for field in ["distribution", "provider"] {
        if metadata["managed_python"][field] != pin["managed_python"][field] {
            return Err(refused());
        }
    }
    if artifacts(&metadata["artifacts"])? != artifacts(&pin["artifacts"])? {
        return Err(refused());
    }
    // Re-observe held namespaces and physical ownership, not content checksums.
    retained.validate()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_source_artifacts_refuse_duplicate_or_changed_entries() {
        let pin: Value = serde_json::from_str(PIN).unwrap();
        let original = artifacts(&pin["artifacts"]).unwrap();
        let mut changed = pin["artifacts"].clone();
        changed[0]["sha256"] = Value::String("0".repeat(64));
        assert!(artifacts(&changed).unwrap() != original);
        changed[1] = changed[0].clone();
        assert!(artifacts(&changed).is_err());
    }
    #[cfg(all(target_os = "linux", target_arch = "x86_64", target_env = "gnu"))]
    #[test]
    fn constraints_come_from_all_source_artifacts_and_only_apply_to_candidate() {
        let constraints = install_constraints("2.10.0", "cpu", "3.12", "cohere-asr")
            .unwrap()
            .unwrap();
        assert_eq!(constraints.lines().count(), 68);
        assert!(constraints.lines().any(|line| line == "numpy==2.5.3"));
        assert!(constraints.lines().any(|line| line == "narwhals==2.26.0"));
        assert!(constraints.lines().any(|line| line == "torch==2.10.0+cpu"));
        let pin: Value = serde_json::from_str(PIN).unwrap();
        for artifact in pin["artifacts"].as_array().unwrap() {
            let line = format!(
                "{}=={}",
                artifact["name"].as_str().unwrap(),
                artifact["version"].as_str().unwrap()
            );
            assert_eq!(
                constraints.lines().filter(|actual| *actual == line).count(),
                1
            );
        }
        assert!(!constraints.contains('@'));
        for (version, build, minor, adapter) in [
            ("2.14.0", "cpu", "3.12", "cohere-asr"),
            ("2.10.0", "cu128", "3.12", "cohere-asr"),
            ("2.10.0", "cpu", "3.13", "cohere-asr"),
            ("2.10.0", "cpu", "3.12", "none"),
        ] {
            assert!(install_constraints(version, build, minor, adapter)
                .unwrap()
                .is_none());
        }
    }
}
