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
/// No Clone/Deserialize or conversion to AudioRuntimeOwner is provided.
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
        if roles
            != BTreeSet::from([
                RuntimeReadRole::Interpreter,
                RuntimeReadRole::Dependencies,
                RuntimeReadRole::Sidecar,
            ])
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

fn refusal(message: &str) -> PumasError {
    PumasError::Config {
        message: message.into(),
    }
}

#[cfg(all(test, target_os = "linux"))]
#[path = "installed_tests.rs"]
mod tests;
