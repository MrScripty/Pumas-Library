//! Source-fixed candidate recipe identity, never production qualification.
//! Expected values came from the official managed CPU installation cohort.
//! Actual selected bytes are captured/hashed before this comparison; presenting
//! these values in JSON without those owned bytes cannot satisfy the check.

use pumas_library::runtime_read_source::{RetainedRuntimeReadSource, RuntimeReadRole};
use serde::Deserialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::io;

const PIN: &str = include_str!("audio_native_cohort/runtime-recipe.json");
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
            return Err(refused());
        }
    }
    for field in ["distribution", "provider"] {
        if metadata["managed_python"][field] != pin["managed_python"][field] {
            return Err(refused());
        }
    }
    if metadata["managed_python"]["executable"]["sha256"] != pin["interpreter_executable_sha256"]
        || artifacts(&metadata["artifacts"])? != artifacts(&pin["artifacts"])?
    {
        return Err(refused());
    }
    for (role, key) in [
        (RuntimeReadRole::Interpreter, "interpreter_selection"),
        (RuntimeReadRole::Dependencies, "dependency_selection"),
    ] {
        let members = retained
            .manifest()
            .filter(|(selected, _)| *selected == role)
            .map(|(_, member)| (member.path(), member.size(), member.sha256()))
            .collect::<Vec<_>>();
        if Some(members.len() as u64) != pin[key]["members"].as_u64()
            || Some(selection_digest(members).as_str()) != pin[key]["sha256"].as_str()
        {
            return Err(refused());
        }
    }
    // Re-observe the held namespaces and actual member bytes after matching.
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
