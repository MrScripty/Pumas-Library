//! Dependency compatibility checks, never native execution authority.
use super::TorchArtifact;
use pumas_library::{PumasError, Result};
use semver::Version;

const REQUIRED: &[&str] = &[
    "transformers",
    "accelerate",
    "huggingface-hub",
    "tokenizers",
    "librosa",
    "soxr",
    "soundfile",
    "sentencepiece",
    "protobuf",
    "numpy",
    "scipy",
    "numba",
    "llvmlite",
];

fn reject(message: &str) -> PumasError {
    PumasError::InstallationFailed {
        message: message.into(),
    }
}

pub(super) fn validate_selection(torch: &str) -> Result<()> {
    let selected = Version::parse(torch).map_err(|_| reject("Invalid cohere-asr Torch version"))?;
    if selected < Version::new(2, 4, 0) {
        return Err(reject("cohere-asr requires selected Torch >=2.4.0"));
    }
    Ok(())
}

pub(super) fn validate(torch: &str, artifacts: &[TorchArtifact]) -> Result<()> {
    validate_selection(torch)?;
    let parse = |value: &str| {
        Version::parse(value).map_err(|_| reject("Invalid cohere-asr dependency version"))
    };
    let get = |name: &str| -> Result<&str> {
        let mut matches = artifacts.iter().filter(|artifact| artifact.name == name);
        let artifact = matches
            .next()
            .ok_or_else(|| reject("cohere-asr omitted a required native dependency"))?;
        if matches.next().is_some() {
            return Err(reject("Duplicate cohere-asr dependency"));
        }
        Ok(&artifact.version)
    };
    for name in REQUIRED {
        get(name)?;
    }
    if get("transformers")? != "5.4.0" {
        return Err(reject(
            "cohere-asr requires native Transformers exactly 5.4.0",
        ));
    }
    if parse(get("accelerate")?)? < Version::new(1, 1, 0) {
        return Err(reject("cohere-asr requires Accelerate >=1.1.0"));
    }
    let hub = parse(get("huggingface-hub")?)?;
    if hub < Version::new(1, 5, 0) || hub >= Version::new(2, 0, 0) {
        return Err(reject("cohere-asr requires huggingface-hub >=1.5.0,<2.0"));
    }
    let tokens = parse(get("tokenizers")?)?;
    if tokens < Version::new(0, 22, 0) || tokens > Version::new(0, 23, 0) {
        return Err(reject("cohere-asr requires tokenizers >=0.22.0,<=0.23.0"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> Vec<TorchArtifact> {
        REQUIRED
            .iter()
            .map(|name| TorchArtifact {
                name: (*name).into(),
                version: match *name {
                    "transformers" => "5.4.0",
                    "accelerate" => "1.12.0",
                    "huggingface-hub" => "1.5.0",
                    "tokenizers" => "0.22.2",
                    _ => "1.0.0",
                }
                .into(),
                url: "https://files.pythonhosted.org/packages/fixture.whl".into(),
                sha256: "a".repeat(64),
            })
            .collect()
    }
    #[test]
    fn cohere_asr_profile_preserves_selected_torch_above_native_floor() {
        for selected in ["2.4.0", "2.9.1", "2.10.0", "2.14.0"] {
            assert!(validate(selected, &fixture()).is_ok());
        }
        assert!(validate("2.3.1", &fixture()).is_err());
    }
    #[test]
    fn cohere_asr_profile_refuses_missing_wrong_or_duplicate_dependencies() {
        for name in REQUIRED {
            let mut entries = fixture();
            entries.retain(|entry| entry.name != *name);
            assert!(validate("2.10.0", &entries).is_err());
        }
        for (name, wrong) in [
            ("transformers", "4.57.6"),
            ("transformers", "5.4.1"),
            ("accelerate", "1.0.0"),
            ("huggingface-hub", "2.0.0"),
            ("tokenizers", "0.23.1"),
        ] {
            let mut entries = fixture();
            entries
                .iter_mut()
                .find(|entry| entry.name == name)
                .unwrap()
                .version = wrong.into();
            assert!(validate("2.10.0", &entries).is_err());
        }
        let mut entries = fixture();
        entries.push(entries[0].clone());
        assert!(validate("2.10.0", &entries).is_err());
    }
}
