//! Non-admitting ownership preparation for an installed native audio runtime.
//!
//! Capture retains actual installed role capabilities and their cooperative
//! mutation leases together with the exact prepared model allocation. Equality
//! with embedded sidecar bytes establishes only selected Pumas code identity.
//! This is not a trusted recipe, complete interpreter/native read closure,
//! containment proof, or production execution grant.

use crate::model_library::artifact_use::PreparedArtifactUse;
use crate::runtime_read_source::{RetainedRuntimeReadSource, RuntimeReadRole};
use crate::{PumasError, Result};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::path::Path;
use std::sync::Arc;

use super::{AudioCustodyError, AudioRuntimeOwner, Qualification};
use cap_std::fs::Dir;

// The required native worker code selection, matching owned_worker.REQUIRED_CODE.
// Additional installed files remain retained by their role; they do not become
// a qualified executable closure through this check.
macro_rules! code {
    ($name:literal) => {
        (
            $name,
            include_bytes!(concat!("../../../../../../torch-server/", $name)).as_slice(),
        )
    };
}
const REQUIRED_CODE: &[(&str, &[u8])] = &[
    code!("owned_worker.py"),
    code!("owned_audio.py"),
    code!("model_manager.py"),
    code!("device_manager.py"),
    code!("private_owned_channel.py"),
    code!("owned_model_operations.py"),
    code!("speech_binding.py"),
    code!("speech_operations.py"),
    code!("native_speech_result.py"),
    code!("audio_input.py"),
    code!("audio_contract.py"),
    code!("loaders/__init__.py"),
    code!("loaders/cohere_asr_loader.py"),
    code!("loaders/owned_cohere_source.py"),
];

/// Unmet execution requirements, not caller-settable approval flags.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum InstalledAudioQualificationGap {
    TrustedInterpreterAndDependencyRecipe,
    CompleteNativeLoaderReadClosure,
    CompleteModelReadContainment,
    PinnedNativeExecutionAndLifecycle,
}

const GAPS: &[InstalledAudioQualificationGap] = &[
    InstalledAudioQualificationGap::TrustedInterpreterAndDependencyRecipe,
    InstalledAudioQualificationGap::CompleteNativeLoaderReadClosure,
    InstalledAudioQualificationGap::CompleteModelReadContainment,
    InstalledAudioQualificationGap::PinnedNativeExecutionAndLifecycle,
];

/// Opaque actual ownership; neither a wire manifest nor a qualification token.
/// No Clone/Deserialize or caller-selected qualification is provided.
pub(crate) struct InstalledAudioRuntimeCandidate {
    installed: Arc<RetainedRuntimeReadSource>,
    interpreter_member: String,
    selected: Arc<PreparedArtifactUse>,
    selected_members: BTreeSet<String>,
    selected_manifest_sha256: String,
}

impl std::fmt::Debug for InstalledAudioRuntimeCandidate {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("InstalledAudioRuntimeCandidate")
            .field("selected_members", &self.selected_members.len())
            .field("qualification_gaps", &GAPS)
            .finish_non_exhaustive()
    }
}

impl InstalledAudioRuntimeCandidate {
    /// Common conditional construction. Policy is private source code, never
    /// a callback, request field, package flag or submitted manifest digest.
    pub(super) fn into_runtime_owner(
        self,
        selected: &Arc<PreparedArtifactUse>,
        policy: InstalledAudioPolicy,
    ) -> std::result::Result<Arc<AudioRuntimeOwner>, AudioCustodyError> {
        if !self.owns_selected(selected) {
            return Err(AudioCustodyError::StaleIdentity);
        }
        self.validate()
            .map_err(|_| AudioCustodyError::Unavailable)?;
        if !policy
            .accepts(&self)
            .map_err(|_| AudioCustodyError::Unavailable)?
        {
            return Err(AudioCustodyError::UnqualifiedRuntime);
        }
        let code = REQUIRED_CODE
            .iter()
            .map(|(name, _)| (*name).to_owned())
            .collect();
        let source = Dir::from_std_file(
            self.installed
                .clone_root(RuntimeReadRole::Sidecar)
                .map_err(|_| AudioCustodyError::Unavailable)?,
        );
        let mut owner = AudioRuntimeOwner::snapshot_directory(source, None, &code)
            .map_err(|_| AudioCustodyError::Unavailable)?;
        if owner.members.len() != REQUIRED_CODE.len()
            || REQUIRED_CODE.iter().any(|(name, bytes)| {
                !owner.members.iter().any(|member| {
                    member.path == *name
                        && member.size == bytes.len() as u64
                        && member.sha256 == hex::encode(Sha256::digest(bytes))
                })
            })
        {
            return Err(AudioCustodyError::Unavailable);
        }
        // No strong model Arc enters the runtime owner. The registry's load
        // admission/slot retains it through validated unload or child drain.
        owner.qualification = Qualification::Installed {
            model_read_set: self.selected_members,
            selected: Arc::downgrade(&self.selected),
        };
        owner.installed_interpreter = Some(self.interpreter_member);
        owner.installed_bytes = Some(self.installed);
        owner
            .validate_source()
            .map_err(|_| AudioCustodyError::Unavailable)?;
        selected
            .validate_read_source()
            .map_err(|_| AudioCustodyError::Unavailable)?;
        Ok(Arc::new(owner))
    }

    /// Blocking ownership preparation before child or provider effects.
    /// The installer must supply captured role capabilities, not locators or
    /// submitted hashes. A member selector cannot authorize its execution.
    pub(crate) fn capture(
        installed: Arc<RetainedRuntimeReadSource>,
        interpreter_member: &str,
        selected: Arc<PreparedArtifactUse>,
    ) -> Result<Self> {
        installed.validate()?;
        selected.validate_read_source()?;
        let roles: BTreeSet<_> = installed.manifest().map(|(role, _)| role).collect();
        let required = BTreeSet::from([
            RuntimeReadRole::Interpreter,
            RuntimeReadRole::Dependencies,
            RuntimeReadRole::Sidecar,
        ]);
        if !required.is_subset(&roles)
            || roles
                .iter()
                .any(|role| !required.contains(role) && *role != RuntimeReadRole::NativeLibraries)
        {
            return Err(refusal(
                "installed audio candidate requires all retained runtime roles",
            ));
        }
        // clone_member resolves only inside the retained role and re-reads the
        // member's actual identity/bytes. It never reopens an ambient locator.
        let interpreter =
            installed.clone_member(RuntimeReadRole::Interpreter, interpreter_member)?;
        if interpreter.metadata()?.len() == 0 {
            return Err(refusal("installed audio interpreter member is empty"));
        }
        for (name, embedded) in REQUIRED_CODE {
            let member = installed
                .manifest()
                .find(|(role, member)| *role == RuntimeReadRole::Sidecar && member.path() == *name)
                .map(|(_, member)| member)
                .ok_or_else(|| refusal("installed audio sidecar member is missing"))?;
            if member.size() != embedded.len() as u64
                || member.sha256() != hex::encode(Sha256::digest(embedded))
            {
                return Err(refusal(
                    "installed audio sidecar differs from embedded worker code",
                ));
            }
        }
        // These hooks are refused by the native bootstrap too. No import or
        // executable probe is performed to discover whether they take effect.
        if installed.manifest().any(|(_, member)| {
            let path = Path::new(member.path());
            matches!(
                path.extension().and_then(|part| part.to_str()),
                Some("pth" | "pyc" | "pyo")
            ) || matches!(
                path.file_name().and_then(|part| part.to_str()),
                Some("sitecustomize.py" | "usercustomize.py")
            )
        }) {
            return Err(refusal(
                "installed audio candidate contains unsupported import hooks or bytecode",
            ));
        }
        let candidate = Self {
            selected_members: selected
                .manifest()
                .map(|member| member.relative_path.clone())
                .collect(),
            selected_manifest_sha256: selected.manifest_sha256().to_owned(),
            installed,
            interpreter_member: interpreter_member.to_owned(),
            selected,
        };
        candidate.validate()?;
        Ok(candidate)
    }

    /// Revalidation observes actual custody; it does not discharge any gap.
    pub(crate) fn validate(&self) -> Result<()> {
        self.installed.validate()?;
        self.selected.validate_read_source()?;
        self.installed
            .clone_member(RuntimeReadRole::Interpreter, &self.interpreter_member)?;
        if self.selected.manifest_sha256() != self.selected_manifest_sha256
            || self
                .selected
                .manifest()
                .map(|member| member.relative_path.clone())
                .collect::<BTreeSet<_>>()
                != self.selected_members
        {
            return Err(refusal("installed audio selected read set changed"));
        }
        Ok(())
    }

    /// Equal names, paths, content or manifests cannot retarget the owner.
    pub(crate) fn owns_selected(&self, selected: &Arc<PreparedArtifactUse>) -> bool {
        Arc::ptr_eq(&self.selected, selected)
    }

    pub(crate) fn qualification_gaps(&self) -> &'static [InstalledAudioQualificationGap] {
        GAPS
    }
}

/// Closed source-owned policy selector. The shipping catalog has no qualified
/// recipe. Adding a shipping variant requires complete execution qualification;
/// captured bytes, an ELF graph or a successful import are insufficient.
pub(super) enum InstalledAudioPolicy {
    Unavailable,
    #[cfg(test)]
    FixedControlledFixture,
}

impl InstalledAudioPolicy {
    pub(super) fn shipping() -> Self {
        Self::Unavailable
    }

    #[cfg(test)]
    pub(super) fn fixed_fixture() -> Self {
        Self::FixedControlledFixture
    }

    fn accepts(&self, candidate: &InstalledAudioRuntimeCandidate) -> Result<bool> {
        match self {
            Self::Unavailable => {
                let _ = candidate;
                Ok(false)
            }
            #[cfg(test)]
            Self::FixedControlledFixture => fixed_fixture_matches(candidate),
        }
    }
}

// This positive policy authorizes ONLY source-fixed dummy-byte unit fixtures.
// It is deliberately absent for feature="test-support", and gives no native
// interpreter/model/ASR, production handshake or execution qualification.
#[cfg(test)]
fn fixed_fixture_matches(candidate: &InstalledAudioRuntimeCandidate) -> Result<bool> {
    const INTERPRETER: (&str, &[u8]) = ("bin/python3.12", b"not an executable interpreter");
    const DEPENDENCY: (&str, &[u8]) = ("controlled_dependency.py", b"CONTROLLED = True\n");
    const MODEL: &[(&str, &[u8])] = &[
        (
            "config.json",
            br#"{"model_type":"cohere_asr","architectures":["CohereAsrForConditionalGeneration"]}"#,
        ),
        (
            "model.safetensors",
            b"synthetic unparsed weights; no native execution",
        ),
        ("preprocessor_config.json", b"{}"),
        ("tokenizer.json", b"{}"),
        ("tokenizer_config.json", b"{}"),
    ];
    if candidate.interpreter_member != INTERPRETER.0 {
        return Ok(false);
    }
    let expected = [
        (
            RuntimeReadRole::Interpreter,
            std::slice::from_ref(&INTERPRETER),
        ),
        (
            RuntimeReadRole::Dependencies,
            std::slice::from_ref(&DEPENDENCY),
        ),
        (RuntimeReadRole::Sidecar, REQUIRED_CODE),
    ];
    for (role, members) in expected {
        let actual: Vec<_> = candidate
            .installed
            .manifest()
            .filter(|(current, _)| *current == role)
            .map(|(_, member)| member)
            .collect();
        if actual.len() != members.len()
            || members.iter().any(|(name, bytes)| {
                !actual.iter().any(|member| {
                    member.path() == *name
                        && member.size() == bytes.len() as u64
                        && member.sha256() == hex::encode(Sha256::digest(bytes))
                })
            })
        {
            return Ok(false);
        }
    }
    let actual: Vec<_> = candidate.selected.manifest().collect();
    Ok(actual.len() == MODEL.len()
        && MODEL.iter().all(|(name, bytes)| {
            actual.iter().any(|member| {
                member.relative_path == *name
                    && member.size_bytes == bytes.len() as u64
                    && member.sha256 == hex::encode(Sha256::digest(bytes))
            })
        }))
}

fn refusal(message: &str) -> PumasError {
    PumasError::Config {
        message: message.into(),
    }
}

#[cfg(all(test, target_os = "linux"))]
#[path = "installed_tests.rs"]
mod tests;
