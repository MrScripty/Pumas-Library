//! Confined command construction from the original admitted runtime/model owner.
//! Shipping policy remains closed. Paths below name inherited capabilities, not
//! caller locators; this layer cannot qualify a recipe or publish availability.

use super::{AudioRuntimeOwner, Qualification};
use crate::model_library::artifact_use::PreparedArtifactUse;
use crate::platform::audio_read_boundary::{AudioReadBoundary, AudioReadGrant};
use crate::platform::managed_child::{ManagedChild, ManagedChildCustodySlot};
use crate::runtime_read_source::RuntimeReadRole;
use std::io;
use std::os::fd::AsRawFd;
use std::process::{ChildStderr, Command};
use std::sync::Arc;

const LOADER: &str = "usr/lib/x86_64-linux-gnu/ld-linux-x86-64.so.2";
const LIBRARIES: &str = "usr/lib/x86_64-linux-gnu";

/// The session must consume diagnostics while binding its private protocol.
/// Dropping the child parks unresolved custody in its original custody slot.
#[must_use]
pub(crate) struct InstalledAudioChild {
    pub(crate) child: ManagedChild,
    pub(crate) diagnostics: ChildStderr,
}
struct SelectedChildLease {
    _runtime: Arc<AudioRuntimeOwner>,
    _model: Arc<PreparedArtifactUse>,
}
fn refused(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::PermissionDenied, message)
}
fn capability(file: &std::fs::File) -> String {
    format!("/proc/self/fd/{}", file.as_raw_fd())
}

impl AudioRuntimeOwner {
    /// The production session's only installed spawn primitive. Qualification
    /// must already belong to this opaque source-owned runtime, for this exact
    /// prepared allocation. No JSON/flag/handshake can supply either owner.
    pub(crate) fn spawn_installed_child(
        self: &Arc<Self>,
        selected: Arc<PreparedArtifactUse>,
        custody: Arc<ManagedChildCustodySlot>,
    ) -> io::Result<InstalledAudioChild> {
        if !matches!(self.qualification, Qualification::Installed { .. })
            || !self.permits_selected(&selected)
        {
            return Err(refused(
                "installed child lacks original runtime/model qualification",
            ));
        }
        AudioReadBoundary::supported_abi()?;
        self.validate_source().map_err(io::Error::other)?;
        selected.validate_read_source().map_err(io::Error::other)?;
        let installed = self
            .installed_bytes
            .as_ref()
            .ok_or_else(|| refused("installed child has no retained runtime bytes"))?;
        let interpreter_name = self
            .installed_interpreter
            .as_deref()
            .ok_or_else(|| refused("installed child has no selected interpreter"))?;
        let interpreter = installed.clone_member(RuntimeReadRole::Interpreter, interpreter_name)?;
        let loader = installed.clone_member(RuntimeReadRole::NativeLibraries, LOADER)?;
        let native = installed.clone_root(RuntimeReadRole::NativeLibraries)?;
        let packages = installed.clone_root(RuntimeReadRole::Dependencies)?;
        let code = self.clone_read_source_directory()?.into_std_file();
        let model = selected.clone_read_source_directory()?;
        let cache_root = capability(&packages);
        let mut command = Command::new(capability(&loader));
        command
            .args(["--inhibit-cache", "--library-path"])
            .arg(format!("{}/{LIBRARIES}", capability(&native)))
            .arg("--argv0")
            .arg(capability(&interpreter))
            .arg(capability(&interpreter))
            .args(["-I", "-S", "-B", "-X", "utf8"])
            .arg(format!("{}/owned_worker.py", capability(&code)))
            .arg("--code-root-fd")
            .arg(code.as_raw_fd().to_string())
            .arg("--packages-root-fd")
            .arg(packages.as_raw_fd().to_string())
            .arg("--model-root-fd")
            .arg(model.as_raw_fd().to_string())
            // Labels are derived only from the original prepared allocation.
            // The child re-hashes held bytes for correlation, never admission.
            .arg(format!("--model-id={}", selected.model_id()))
            .arg(format!(
                "--selected-artifact-id={}",
                selected.selected_artifact_id()
            ));
        // Sidecar originals are not execution inputs: only their independently
        // copied and revalidated code snapshot below gets content grants.
        let directories = installed
            .directory_manifest()
            .filter(|(role, _)| *role != RuntimeReadRole::Sidecar)
            .map(|(role, name)| {
                installed
                    .clone_directory(role, name)
                    .and_then(AudioReadGrant::directory)
            });
        let files = installed
            .manifest()
            .filter(|(role, _)| *role != RuntimeReadRole::Sidecar)
            .map(|(role, member)| {
                installed
                    .clone_member(role, member.path())
                    .and_then(|file| {
                        AudioReadGrant::file(
                            file,
                            role == RuntimeReadRole::NativeLibraries && member.path() == LOADER,
                        )
                    })
            });
        let code_directories =
            std::iter::once(self.copied_root.try_clone().map(|dir| dir.into_std_file()))
                .chain(
                    self.directories
                        .iter()
                        .map(|(_, dir)| dir.try_clone().map(|dir| dir.into_std_file())),
                )
                .map(|file| file.and_then(AudioReadGrant::directory));
        let code_files = self.members.iter().map(|member| {
            member
                .file
                .try_clone()
                .and_then(|file| AudioReadGrant::file(file, false))
        });
        let model_directory = std::iter::once(
            selected
                .clone_read_source_directory()
                .and_then(AudioReadGrant::directory),
        );
        let model_files = selected
            .clone_read_source_members()
            .map(|file| file.and_then(|file| AudioReadGrant::file(file, false)));
        let boundary = AudioReadBoundary::prepare(
            directories
                .chain(files)
                .chain(code_directories)
                .chain(code_files)
                .chain(model_directory)
                .chain(model_files)
                .chain(std::iter::once(AudioReadGrant::kernel_entropy())),
            vec![interpreter, loader, native, packages, code, model],
        )?;
        // Rule setup can take time for a large installed tree. Re-check actual
        // selected identities/bytes before any process or provider effect.
        self.validate_source().map_err(io::Error::other)?;
        selected.validate_read_source().map_err(io::Error::other)?;
        boundary.confine_command(&mut command);
        // Eager inference does not get a writable compiler cache. The held
        // directory satisfies import-time discovery while writes stay denied.
        command.env("TORCHINDUCTOR_CACHE_DIR", cache_root);
        let lease = Arc::new(SelectedChildLease {
            _runtime: self.clone(),
            _model: selected,
        });
        let mut child = ManagedChild::spawn(&mut command, custody)?;
        // No fallible operation or readiness publication precedes attachment.
        child.attach_cleanup_lease(lease);
        let diagnostics = child.take_private_stderr()?;
        Ok(InstalledAudioChild { child, diagnostics })
    }
}
