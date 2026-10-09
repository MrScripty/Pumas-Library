//! Assembly-only immutable component bundles. No interpreter/import authority.

use super::*;
use pumas_library::acquisition::AcquisitionLocalRequest;
use pumas_library::runtime_read_source::{RetainedRuntimeReadSource, RuntimeReadRole};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct TorchComponentFile {
    pub path: String,
    pub size: u64,
    pub sha256: String,
}

/// Reviewed byte closure, not semantic dependency resolution or execution proof.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TorchComponentManifest {
    pub distribution: String,
    pub version: String,
    pub wheel_name: String,
    pub wheel_tag: String,
    pub python: String,
    pub platform: String,
    pub archive_files: Vec<TorchComponentFile>,
    pub replace_base_members: Vec<String>,
    pub final_dependency_files: Vec<TorchComponentFile>,
}

/// A selected registered base plus exact owner-held local wheel input.
/// No caller-selected output path or positive startup witness is accepted.
pub struct TorchComponentAssemblyRequest {
    pub base_tag: String,
    pub expected_base_manifest_sha256: String,
    pub component: TorchComponentManifest,
    pub input: AcquisitionLocalRequest,
}

pub struct TorchComponentAssembly {
    pub revision_tag: String,
    pub manifest_sha256: String,
    pub progress: mpsc::Receiver<ProgressUpdate>,
}

/// Owner-issued byte selection. Private fields prevent caller reconstruction.
/// Clone/retain this actual association through registered native work and
/// process retirement. It is not registry pinning or initialization/import proof.
#[derive(Clone)]
pub struct TorchComponentSelection {
    revision_tag: String,
    manifest_sha256: String,
    interpreter_depot_manifest_sha256: String,
    source: Arc<RetainedRuntimeReadSource>,
}

impl TorchComponentSelection {
    pub fn revision_tag(&self) -> &str {
        &self.revision_tag
    }
    pub fn manifest_sha256(&self) -> &str {
        &self.manifest_sha256
    }
    pub fn interpreter_depot_manifest_sha256(&self) -> &str {
        &self.interpreter_depot_manifest_sha256
    }
    /// Fixed immutable policy disposition; never an executable readiness claim.
    pub fn qualification(&self) -> &'static str {
        "assembled_unqualified"
    }
    pub fn retained_source(&self) -> &Arc<RetainedRuntimeReadSource> {
        &self.source
    }

    /// Blocking validation of captured bytes and their immutable association.
    /// Run under consumer effect custody, retaining this selection itself.
    pub fn validate(&self) -> Result<()> {
        self.source.validate().map_err(PumasError::from)?;
        if torch_interpreter_depot_manifest_sha256(&self.source)?
            != self.interpreter_depot_manifest_sha256
        {
            return Err(refusal("Owner-issued interpreter byte association changed"));
        }
        let mut input = self
            .source
            .clone_member(RuntimeReadRole::Sidecar, "component-manifest.json")
            .map_err(PumasError::from)?;
        let mut sha = Sha256::new();
        let mut buffer = [0_u8; 65536];
        use std::io::Read;
        loop {
            let count = input.read(&mut buffer).map_err(PumasError::from)?;
            if count == 0 {
                break;
            }
            sha.update(&buffer[..count]);
        }
        if format!("{:x}", sha.finalize()) != self.manifest_sha256
            || self.revision_tag != format!("torch-component-{}", self.manifest_sha256)
        {
            return Err(refusal("Owner-issued component association changed"));
        }
        Ok(())
    }
}

pub(crate) struct TorchComponentPlan {
    pub(crate) base_tag: String,
    pub(crate) base: Arc<RetainedRuntimeReadSource>,
    pub(crate) component: TorchComponentManifest,
    pub(crate) revision_tag: String,
    pub(crate) manifest_sha256: String,
    pub(crate) identity: serde_json::Value,
    pub(crate) input: std::sync::Mutex<Option<AcquisitionLocalRequest>>,
}

fn refusal(message: &str) -> PumasError {
    PumasError::Validation {
        field: "torch.component_assembly".into(),
        message: message.into(),
    }
}

pub(crate) fn component_revision(tag: &str) -> bool {
    tag.strip_prefix("torch-component-").is_some_and(|digest| {
        digest.len() == 64
            && digest
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    })
}

pub(crate) fn safe_member(path: &str) -> bool {
    !path.is_empty()
        && path.len() <= 1024
        && path.split('/').all(|part| {
            !part.is_empty()
                && part != "."
                && part != ".."
                && part
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"._+-".contains(&b))
        })
}

fn canonical_files(files: &mut [TorchComponentFile]) -> Result<()> {
    if files.is_empty() || files.len() > 200_000 {
        return Err(refusal("Component file manifest is empty or oversized"));
    }
    let mut names = BTreeSet::new();
    let mut total = 0_u64;
    for file in files.iter_mut() {
        if !safe_member(&file.path)
            || file.size > 2 * 1024 * 1024 * 1024
            || file.sha256.len() != 64
            || !file.sha256.bytes().all(|b| b.is_ascii_hexdigit())
            || !names.insert(file.path.to_ascii_lowercase())
            || file.path.ends_with(".pth")
            || file.path.ends_with(".pyc")
            || file.path.ends_with(".pyo")
            || file.path.split('/').any(|part| part.ends_with(".data"))
            || matches!(
                file.path.rsplit('/').next(),
                Some("sitecustomize.py" | "usercustomize.py")
            )
        {
            return Err(refusal("Invalid, aliased or unsupported component member"));
        }
        file.sha256.make_ascii_lowercase();
        total = total
            .checked_add(file.size)
            .ok_or_else(|| refusal("Component size overflow"))?;
    }
    if total > 4 * 1024 * 1024 * 1024 {
        return Err(refusal("Component expanded byte closure exceeds bound"));
    }
    let mut directories = BTreeSet::new();
    for file in files.iter() {
        for (offset, _) in file.path.match_indices('/') {
            let parent = &file.path[..offset];
            directories.insert(parent.to_owned());
            if names.contains(&parent.to_ascii_lowercase()) {
                return Err(refusal("Component file and directory names overlap"));
            }
        }
    }
    if names.len() + directories.len() > 200_000 {
        return Err(refusal(
            "Component file and directory namespace exceeds bound",
        ));
    }
    files.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(())
}

/// Domain-separated digest of the actual captured closed selections.
pub fn torch_runtime_byte_manifest_sha256(source: &RetainedRuntimeReadSource) -> Result<String> {
    let mut members: Vec<_> = source
        .manifest()
        .map(|(role, file)| {
            let role = match role {
                RuntimeReadRole::Interpreter => "interpreter",
                RuntimeReadRole::Dependencies => "dependencies",
                RuntimeReadRole::Sidecar => "sidecar",
            };
            (role, file.path(), file.size(), file.sha256())
        })
        .collect();
    members.sort_unstable();
    let bytes = serde_json::to_vec(
        &serde_json::json!({"domain":"pumas.torch.selected-byte-manifest.v1","members":members}),
    )
    .map_err(|_| refusal("Cannot encode captured base identity"))?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
}

/// Digest of sorted (path, size, SHA256) tuples in the actual Interpreter role.
/// This selected depot byte manifest excludes omitted paths and system libraries.
pub fn torch_interpreter_depot_manifest_sha256(
    source: &RetainedRuntimeReadSource,
) -> Result<String> {
    let mut members: Vec<_> = source
        .manifest()
        .filter(|(role, _)| *role == RuntimeReadRole::Interpreter)
        .map(|(_, file)| (file.path(), file.size(), file.sha256()))
        .collect();
    if members.is_empty() {
        return Err(refusal("Managed interpreter selection is absent"));
    }
    members.sort_unstable();
    let bytes = serde_json::to_vec(&serde_json::json!({
        "domain":"pumas.torch.selected-interpreter-manifest.v1", "members": members
    }))
    .map_err(|_| refusal("Cannot encode captured interpreter identity"))?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
}

impl TorchComponentPlan {
    pub(crate) fn new(
        mut request: TorchComponentAssemblyRequest,
        base: Arc<RetainedRuntimeReadSource>,
    ) -> Result<Self> {
        let observed = torch_runtime_byte_manifest_sha256(&base)?;
        if request.expected_base_manifest_sha256 != observed
            || request.component.python != "python3.12"
            || request.component.platform != "linux-x86_64"
            || !safe_member(&request.component.distribution)
            || request.component.distribution.contains('/')
            || !safe_member(&request.component.version)
            || request.component.version.contains('/')
            || !safe_member(&request.component.wheel_name)
            || request.component.wheel_name.contains('/')
            || !request
                .component
                .wheel_name
                .ends_with(&format!("-{}.whl", request.component.wheel_tag))
            || !["cp39-abi3-linux_x86_64", "cp312-cp312-linux_x86_64"]
                .contains(&request.component.wheel_tag.as_str())
            || request.input.manifest.files().len() != 1
            || request.input.sources.len() != 1
        {
            return Err(refusal(
                "Base identity, cohort or exact wheel input mismatch",
            ));
        }
        let wheel = &request.input.manifest.files()[0];
        if wheel.logical_path() != request.component.wheel_name
            || wheel
                .expected_size()
                .is_none_or(|size| size == 0 || size > 2 * 1024 * 1024 * 1024)
            || wheel.expected_sha256().is_none()
        {
            return Err(refusal(
                "Wheel requires bounded exact size and SHA256 evidence",
            ));
        }
        canonical_files(&mut request.component.archive_files)?;
        canonical_files(&mut request.component.final_dependency_files)?;
        request.component.replace_base_members.sort();
        let mut replacements = BTreeSet::new();
        let dependencies: BTreeSet<_> = base
            .manifest()
            .filter(|(role, _)| *role == RuntimeReadRole::Dependencies)
            .map(|(_, file)| file.path().to_owned())
            .collect();
        for path in &request.component.replace_base_members {
            if !safe_member(path)
                || !dependencies.contains(path)
                || !replacements.insert(path.clone())
            {
                return Err(refusal(
                    "Replacement must name each selected base member exactly once",
                ));
            }
        }
        let identity = serde_json::json!({"domain":"pumas.torch.component-assembly.v1", "policy_version":1,
            "base_tag":request.base_tag,"base_manifest_sha256":observed,
            "interpreter_depot_manifest_sha256":torch_interpreter_depot_manifest_sha256(&base)?,
            "qualification":"assembled_unqualified",
            "launch_restrictions":["no_runnable_interpreter_entry","no_initialized_import_provenance"],
            "component":request.component,
            "input_manifest":request.input.manifest});
        let encoded = serde_json::to_vec(&identity)
            .map_err(|_| refusal("Cannot encode component identity"))?;
        if encoded.len() > 64 * 1024 * 1024 {
            return Err(refusal("Component identity exceeds retained reader bound"));
        }
        let manifest_sha256 = format!("{:x}", Sha256::digest(encoded));
        let revision_tag = format!("torch-component-{manifest_sha256}");
        Ok(Self {
            base_tag: request.base_tag,
            base,
            component: request.component,
            revision_tag,
            manifest_sha256,
            identity,
            input: std::sync::Mutex::new(Some(request.input)),
        })
    }
}

impl VersionManager {
    /// Select only a registered immutable assembly with real owner byte custody.
    /// No execution witness is created; existing startup paths remain refused.
    pub async fn select_torch_component_revision(
        &self,
        revision_tag: &str,
    ) -> Result<TorchComponentSelection> {
        if !component_revision(revision_tag) {
            return Err(refusal("Invalid component revision identity"));
        }
        let source = self.retain_torch_runtime_bytes(revision_tag).await?;
        let selection = TorchComponentSelection {
            revision_tag: revision_tag.into(),
            manifest_sha256: revision_tag["torch-component-".len()..].into(),
            interpreter_depot_manifest_sha256: torch_interpreter_depot_manifest_sha256(&source)?,
            source,
        };
        Ok(selection)
    }
    /// Match caller declarations against a real owner selection before native effects.
    /// Successful matching retains the same unqualified disposition.
    pub async fn select_torch_component_revision_matching(
        &self,
        revision_tag: &str,
        expected_manifest_sha256: &str,
        expected_interpreter_depot_manifest_sha256: &str,
    ) -> Result<TorchComponentSelection> {
        for digest in [
            expected_manifest_sha256,
            expected_interpreter_depot_manifest_sha256,
        ] {
            if digest.len() != 64
                || !digest
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            {
                return Err(refusal("Expected selection digest is not canonical SHA256"));
            }
        }
        if revision_tag != format!("torch-component-{expected_manifest_sha256}") {
            return Err(refusal("Expected revision and manifest identity mismatch"));
        }
        let selection = self.select_torch_component_revision(revision_tag).await?;
        if selection.interpreter_depot_manifest_sha256()
            != expected_interpreter_depot_manifest_sha256
        {
            return Err(refusal("Expected interpreter depot byte manifest mismatch"));
        }
        Ok(selection)
    }

    /// Assemble and register an immutable byte bundle. Startup remains refused.
    /// Dropping progress does not cancel work; await shutdown_installations.
    pub async fn assemble_torch_component_revision(
        &self,
        request: TorchComponentAssemblyRequest,
    ) -> Result<TorchComponentAssembly> {
        if self.app_id != AppId::Torch || !cfg!(all(target_os = "linux", target_arch = "x86_64")) {
            return Err(refusal("Component assembly requires Linux x86_64 Torch"));
        }
        let consumer = self.acquisition_consumer.clone().ok_or_else(|| {
            refusal("Component assembly requires the shared acquisition consumer")
        })?;
        if request.input.demand.consumer != consumer.owner() {
            return Err(refusal(
                "Component input belongs to another acquisition owner",
            ));
        }
        let release = Self::torch_release_for_install(&request.base_tag)?;
        let base = self.retain_torch_runtime_bytes(&request.base_tag).await?;
        let plan = TorchComponentPlan::new(request, base)?;
        let revision_tag = plan.revision_tag.clone();
        let manifest_sha256 = plan.manifest_sha256.clone();
        let guard = self.install_lock.clone().lock_owned().await;
        let mut installing = self.installing_tag.lock().await;
        let mut registered = self
            .installation_tasks
            .lock()
            .map_err(|_| refusal("Installation task registry poisoned"))?;
        registered.harvest_finished();
        if self.torch_shutting_down.load(Ordering::SeqCst) {
            return Err(refusal("Version manager is shutting down"));
        }
        self.cancel_flag.store(false, Ordering::SeqCst);
        self.torch_control.start();
        *installing = Some(revision_tag.clone());
        let (tx, rx) = mpsc::channel(32);
        let installer = VersionInstaller::new(
            self.launcher_root.clone(),
            AppId::Torch,
            self.metadata_manager.clone(),
            self.progress_tracker.clone(),
            self.cancel_flag.clone(),
        )
        .with_torch_control(self.torch_control.clone())
        .with_torch_cleanup(self.torch_cleanup.clone())
        .with_shutdown_flag(self.torch_shutting_down.clone())
        .with_acquisition_consumer(Some(consumer));
        #[cfg(test)]
        let installer = if let Some(pause) = &self.torch_publication_pause {
            installer.with_torch_publication_pause(pause.clone())
        } else {
            installer
        };
        let manager = self.clone();
        let tag = revision_tag.clone();
        registered.tasks.push(tokio::spawn(async move {
            let _guard = guard;
            let result = installer.install_version_with_torch_input(&tag,&release,tx.clone(),Some(installer::TorchInstallInput::Component(Box::new(plan)))).await;
            *manager.installing_tag.lock().await = None;
            if result.is_ok() {
                if let Err(error) = manager.state.write().await.refresh().await {
                    warn!(%error,"Component publication succeeded; state cache refresh failed");
                }
            } else if let Err(error) = &result {
                let mut tracker = manager.progress_tracker.write().await;
                tracker.set_error(&error.to_string());
                tracker.complete_installation(false);
            }
            let terminal = result.as_ref().map(|_| ()).map_err(ToString::to_string);
            tokio::select! {
                _ = tx.send(match result {Ok(())=>ProgressUpdate::Completed{success:true},Err(e)=>ProgressUpdate::Error{message:e.to_string()}}) => {},
                _ = wait_for_install_cancel(manager.torch_shutting_down.clone()) => {},
            }
            terminal
        }));
        Ok(TorchComponentAssembly {
            revision_tag,
            manifest_sha256,
            progress: rx,
        })
    }
}
