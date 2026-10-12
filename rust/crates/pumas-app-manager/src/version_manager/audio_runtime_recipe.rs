//! Source-fixed candidate recipe identity, never production qualification.
//! Expected values came from the official managed CPU installation cohort.
//! Actual selected bytes are captured/hashed before this comparison; presenting
//! these values in JSON without those owned bytes cannot satisfy the check.

use pumas_library::runtime_read_source::{RetainedRuntimeReadSource, RuntimeReadRole};
use serde::Deserialize;
use serde_json::Value;
#[cfg(test)]
use sha2::{Digest, Sha256};
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
fn mismatch(field: &str) -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        format!("selected runtime differs from source-pinned audio candidate recipe ({field})"),
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

pub(super) fn validate_managed_python(metadata: &Value) -> io::Result<()> {
    let pin: Value = serde_json::from_str(PIN).map_err(|_| refused())?;
    for field in ["distribution", "provider"] {
        if metadata[field] != pin["managed_python"][field] {
            return Err(mismatch(&format!("managed_python.{field}")));
        }
    }
    if metadata["executable"]["sha256"] != pin["interpreter_executable_sha256"] {
        return Err(mismatch("managed_python.executable.sha256"));
    }
    Ok(())
}

pub(super) fn validate_artifacts(selected: &Value) -> io::Result<()> {
    let pin: Value = serde_json::from_str(PIN).map_err(|_| refused())?;
    let selected = artifacts(selected)?;
    for (selected, expected) in selected.iter().zip(artifacts(&pin["artifacts"])?) {
        if selected != &expected {
            // Only source-owned names enter the diagnostic, never observed URLs
            // or paths from the installed record.
            return Err(mismatch(&format!("artifacts.{}", expected.name)));
        }
    }
    Ok(())
}

/// Keep the same named dependency roots and pip-installed metadata while
/// limiting qualification acquisition to the existing reviewed wheel cohort.
/// Direct-URL requirements for every transitive package would add REQUESTED
/// and direct_url.json records, changing the pinned installed-byte selection.
pub(super) fn candidate_links() -> io::Result<String> {
    let pin: Value = serde_json::from_str(PIN).map_err(|_| refused())?;
    let mut links = String::from("<!doctype html>\n");
    for artifact in artifacts(&pin["artifacts"])? {
        let url = artifact
            .url
            .replace('&', "&amp;")
            .replace('"', "&quot;")
            .replace('<', "&lt;")
            .replace('>', "&gt;");
        links.push_str(&format!(
            "<a href=\"{url}#sha256={}\">{}</a>\n",
            artifact.sha256, artifact.name
        ));
    }
    Ok(links)
}
#[cfg(test)]
fn selection_digest(mut members: Vec<(&str, u64, &str)>) -> String {
    members.sort();
    let mut digest = Sha256::new();
    digest.update(b"pumas-audio-runtime-recipe-v1\0");
    for (name, size, sha256) in members {
        digest.update((name.len() as u64).to_be_bytes());
        digest.update(name.as_bytes());
        digest.update(size.to_be_bytes());
        digest.update(sha256.as_bytes());
    }
    format!("{:x}", digest.finalize())
}
pub(super) fn validate(metadata: &Value, retained: &RetainedRuntimeReadSource) -> io::Result<()> {
    let pin: Value = serde_json::from_str(PIN).map_err(|_| refused())?;
    for field in ["python", "adapter", "build"] {
        if metadata[field] != pin[field] {
            return Err(mismatch(field));
        }
    }
    validate_managed_python(&metadata["managed_python"])?;
    validate_artifacts(&metadata["artifacts"])?;
    for role in [RuntimeReadRole::Interpreter, RuntimeReadRole::Dependencies] {
        pumas_library::runtime_read_source::validate_audio_candidate_read_role(retained, role)?;
    }
    // Re-observe the held namespaces and actual member bytes after matching.
    retained.validate()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn candidate_links_only_advertise_the_exact_hashed_source_cohort() {
        let pin: Value = serde_json::from_str(PIN).unwrap();
        let links = candidate_links().unwrap();
        assert_eq!(links.lines().count(), 69);
        for artifact in artifacts(&pin["artifacts"]).unwrap() {
            assert_eq!(
                links
                    .matches(&format!("{}#sha256={}", artifact.url, artifact.sha256))
                    .count(),
                1
            );
        }
    }

    #[test]
    fn selected_artifacts_must_equal_the_whole_recipe_without_disclosing_observed_values() {
        let pin: Value = serde_json::from_str(PIN).unwrap();
        let original = pin["artifacts"].clone();
        validate_artifacts(&original).unwrap();
        let mut reversed = original.clone();
        reversed.as_array_mut().unwrap().reverse();
        validate_artifacts(&reversed).unwrap();
        for field in ["name", "version", "url", "sha256"] {
            let mut changed = original.clone();
            changed[0][field] = Value::String("private-observed-value".into());
            let error = validate_artifacts(&changed).unwrap_err().to_string();
            assert!(error.contains("artifacts."), "{field}: {error}");
            assert!(!error.contains("private-observed-value"));
        }
        let mut missing = original.clone();
        missing.as_array_mut().unwrap().pop();
        assert!(validate_artifacts(&missing).is_err());
        let mut extra = original.clone();
        extra.as_array_mut().unwrap().push(serde_json::json!({
            "name": "unpinned", "version": "1", "url": "https://private.invalid/secret", "sha256": "0".repeat(64)
        }));
        assert!(validate_artifacts(&extra).is_err());
        let mut duplicate = original.clone();
        duplicate[1] = duplicate[0].clone();
        assert!(validate_artifacts(&duplicate).is_err());
    }

    #[test]
    fn managed_candidate_identity_refuses_drift_before_wheel_acquisition() {
        let pin: Value = serde_json::from_str(PIN).unwrap();
        let mut identity = pin["managed_python"].clone();
        identity["executable"] =
            serde_json::json!({"sha256": pin["interpreter_executable_sha256"]});
        validate_managed_python(&identity).unwrap();
        for field in ["distribution", "provider", "executable"] {
            let mut changed = identity.clone();
            changed[field] = Value::String("private-observed-value".into());
            let error = validate_managed_python(&changed).unwrap_err().to_string();
            assert!(error.contains(&format!("managed_python.{field}")));
            assert!(!error.contains("private-observed-value"));
        }
    }

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
    #[test]
    fn selected_digest_is_order_independent_and_field_framed() {
        let first = vec![("a", 1, "sha-a"), ("bc", 2, "sha-b")];
        let reverse = vec![("bc", 2, "sha-b"), ("a", 1, "sha-a")];
        assert_eq!(selection_digest(first.clone()), selection_digest(reverse));
        assert_ne!(
            selection_digest(first),
            selection_digest(vec![("ab", 1, "sha-a"), ("c", 2, "sha-b")])
        );
    }
}
