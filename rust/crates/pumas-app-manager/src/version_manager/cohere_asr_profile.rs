//! Compatibility floors for the `cohere-asr` adapter. Installs resolve current
//! package versions; these checks only refuse a resolution that cannot run
//! Transformers' native Cohere ASR implementation.
use super::TorchArtifact;
use pumas_library::{PumasError, Result};

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
];

/// Native `CohereAsrForConditionalGeneration` support starts here.
const TRANSFORMERS_FLOOR: (u64, u64, u64) = (5, 4, 0);
const ACCELERATE_FLOOR: (u64, u64, u64) = (1, 1, 0);
const TORCH_FLOOR: (u64, u64, u64) = (2, 4, 0);

fn reject(message: &str) -> PumasError {
    PumasError::InstallationFailed {
        message: message.into(),
    }
}

/// The release triple of a PEP 440 version (`5.10.0.dev0` and `2.14.0+cpu`
/// compare as `5.10.0` and `2.14.0`).
fn release(value: &str) -> Option<(u64, u64, u64)> {
    let mut parts = value.split(['+', '-']).next()?.split('.').map(|part| {
        let digits: String = part.chars().take_while(char::is_ascii_digit).collect();
        digits.parse::<u64>().ok()
    });
    let major = parts.next()??;
    let minor = parts.next().flatten().unwrap_or(0);
    let patch = parts.next().flatten().unwrap_or(0);
    Some((major, minor, patch))
}

pub(super) fn validate_selection(torch: &str) -> Result<()> {
    let selected = release(torch).ok_or_else(|| reject("Invalid cohere-asr Torch version"))?;
    if selected < TORCH_FLOOR {
        return Err(reject("cohere-asr requires Torch >=2.4.0"));
    }
    Ok(())
}

pub(super) fn validate(torch: &str, artifacts: &[TorchArtifact]) -> Result<()> {
    validate_selection(torch)?;
    let get = |name: &str| -> Result<&str> {
        let mut matches = artifacts.iter().filter(|artifact| artifact.name == name);
        let artifact = matches.next().ok_or_else(|| {
            reject(&format!(
                "cohere-asr resolution omitted the required {name} package"
            ))
        })?;
        if matches.next().is_some() {
            return Err(reject("Duplicate cohere-asr dependency"));
        }
        Ok(&artifact.version)
    };
    for name in REQUIRED {
        get(name)?;
    }
    let at_least = |name: &str, floor: (u64, u64, u64), message: &str| -> Result<()> {
        let version =
            release(get(name)?).ok_or_else(|| reject("Invalid cohere-asr dependency version"))?;
        if version < floor {
            return Err(reject(message));
        }
        Ok(())
    };
    at_least(
        "transformers",
        TRANSFORMERS_FLOOR,
        "cohere-asr requires Transformers >=5.4.0",
    )?;
    at_least(
        "accelerate",
        ACCELERATE_FLOOR,
        "cohere-asr requires Accelerate >=1.1.0",
    )?;
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
                    _ => "1.0.0",
                }
                .into(),
                url: "https://files.pythonhosted.org/packages/fixture.whl".into(),
                sha256: "a".repeat(64),
            })
            .collect()
    }
    fn with(name: &str, version: &str) -> Vec<TorchArtifact> {
        let mut entries = fixture();
        entries
            .iter_mut()
            .find(|entry| entry.name == name)
            .unwrap()
            .version = version.into();
        entries
    }
    #[test]
    fn cohere_asr_accepts_current_versions_above_the_floors() {
        for selected in ["2.4.0", "2.10.0", "2.14.0+cpu"] {
            assert!(validate(selected, &fixture()).is_ok());
        }
        assert!(validate("2.3.1", &fixture()).is_err());
        for current in ["5.4.0", "5.4.1", "5.10.0", "6.0.0.dev0"] {
            assert!(
                validate("2.10.0", &with("transformers", current)).is_ok(),
                "{current}"
            );
        }
        for current in ["1.0.0", "2.0.0", "0.23.1"] {
            assert!(validate("2.10.0", &with("huggingface-hub", current)).is_ok());
            assert!(validate("2.10.0", &with("tokenizers", current)).is_ok());
        }
    }
    #[test]
    fn cohere_asr_refuses_missing_duplicate_or_too_old_dependencies() {
        for name in REQUIRED {
            let mut entries = fixture();
            entries.retain(|entry| entry.name != *name);
            let error = validate("2.10.0", &entries).unwrap_err().to_string();
            assert!(error.contains(name), "{error}");
        }
        assert!(validate("2.10.0", &with("transformers", "4.57.6")).is_err());
        assert!(validate("2.10.0", &with("accelerate", "1.0.0")).is_err());
        let mut entries = fixture();
        entries.push(entries[0].clone());
        assert!(validate("2.10.0", &entries).is_err());
    }
}
