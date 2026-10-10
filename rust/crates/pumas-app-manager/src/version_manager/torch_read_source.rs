//! Selected installed-byte custody, without native/runtime qualification.
//! Linux managed CPython only. The optional NativeLibraries cohort retains
//! source-pinned system-loader/library bytes. Byte provenance and custody do
//! not attest dynamic loader behavior, enforced reads or production ASR.

use super::installer::{self, StagedFilesManifest, TorchVersionsLock};
use super::managed_depot_lease::ManagedDepotLease;
use super::*;
use pumas_library::runtime_read_source::{
    RetainedRuntimeReadSource, RuntimeReadFile, RuntimeReadRole, RuntimeReadRoot,
};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::fs::File;
use std::io::{self, Read};

impl VersionManager {
    /// Prepare a source-pinned native cohort and retain all selected runtime
    /// bytes. This does not grant audio admission or qualify native execution.
    /// The owned native directory disappears only after its final custody owner.
    /// The manager owns preparation through nested stages and joins it at shutdown.
    /// Do not hold a caller-side Torch lifecycle lease while awaiting this method.
    pub async fn prepare_torch_audio_runtime_bytes(
        &self,
        tag: &str,
    ) -> Result<Arc<RetainedRuntimeReadSource>> {
        let tag = tag.to_owned();
        self.owned_runtime_preparation(move |manager| async move {
            manager.prepare_torch_audio_runtime_bytes_owned(&tag).await
        })
        .await
    }

    /// Prepare the exact active tag observed by a serving request. A changed
    /// selection refuses under the owned lifecycle lock; it never retargets.
    pub async fn prepare_active_torch_audio_runtime_bytes(
        &self,
        expected_tag: &str,
    ) -> Result<Arc<RetainedRuntimeReadSource>> {
        let tag = expected_tag.to_owned();
        self.owned_runtime_preparation(move |manager| async move {
            if manager.get_active_version().await?.as_deref() != Some(tag.as_str()) {
                return Err(refused("Active Torch runtime selection changed"));
            }
            manager.prepare_torch_audio_runtime_bytes_owned(&tag).await
        })
        .await
    }

    async fn prepare_torch_audio_runtime_bytes_owned(
        &self,
        tag: &str,
    ) -> Result<Arc<RetainedRuntimeReadSource>> {
        // Validate the existing selection before downloading any additional
        // public artifact. Its read leases remain held until recapture finishes.
        let existing = self.retain_torch_runtime_bytes_owned(tag).await?;
        let recipe_source = existing.clone();
        let recipe_path = self.versions_dir().join(tag).join("runtime.json");
        tokio::task::spawn_blocking(move || -> Result<()> {
            let metadata = serde_json::from_slice(&bounded_json(&recipe_path, 1024 * 1024)?)
                .map_err(|error| refused(error.to_string()))?;
            super::audio_runtime_recipe::validate(&metadata, &recipe_source)
                .map_err(PumasError::from)
        })
        .await
        .map_err(|error| refused(format!("Audio recipe validation task failed: {error}")))??;
        let native = super::audio_native_cohort::prepare(&self.launcher_root)
            .await
            .map_err(PumasError::from)?;
        let versions = self.versions_dir();
        let lock = TorchVersionsLock::try_acquire_read(&versions).map_err(PumasError::from)?;
        let runtime = versions.join(tag);
        let launcher = self.launcher_root.clone();
        let selected = tokio::task::spawn_blocking(move || {
            capture_installed_with_native(&launcher, &runtime, lock, Some(native))
        })
        .await
        .map_err(|error| refused(format!("Native runtime capture task failed: {error}")))??;
        drop(existing);
        Ok(selected)
    }
    /// Retain actual interpreter, dependency and embedded sidecar selections.
    /// This does not grant serving availability or complete execution proof.
    /// Manager-owned lifecycle custody survives cancellation of the result waiter.
    /// Do not hold a caller-side Torch lifecycle lease while awaiting this method.
    pub async fn retain_torch_runtime_bytes(
        &self,
        tag: &str,
    ) -> Result<Arc<RetainedRuntimeReadSource>> {
        let tag = tag.to_owned();
        self.owned_runtime_preparation(move |manager| async move {
            manager.retain_torch_runtime_bytes_owned(&tag).await
        })
        .await
    }

    // Registered under the same admission lock as manager shutdown. A caller
    // owns only its result waiter; the manager owns every nested blocking stage.
    async fn owned_runtime_preparation<T, F, Fut>(&self, operation: F) -> Result<T>
    where
        T: Send + 'static,
        F: FnOnce(VersionManager) -> Fut + Send + 'static,
        Fut: std::future::Future<Output = Result<T>> + Send + 'static,
    {
        if self.app_id != AppId::Torch {
            return Err(refused(
                "Runtime byte preparation requires the Torch manager",
            ));
        }
        let receive = {
            let _admission = self.installing_tag.lock().await;
            let mut registered = self
                .installation_tasks
                .lock()
                .map_err(|_| refused("Installation task registry poisoned"))?;
            if self.torch_shutting_down.load(Ordering::SeqCst) {
                return Err(refused("Version manager is shutting down"));
            }
            registered.harvest_finished();
            let manager = self.clone();
            let (send, receive) = tokio::sync::oneshot::channel();
            registered.tasks.push(tokio::spawn(async move {
                let _lifecycle = manager.lifecycle_lock.clone().lock_owned().await;
                // Keep the manager's original state/root owners through the
                // result handoff, even if the preparation future consumes a clone.
                let result = operation(manager.clone()).await;
                let _ = send.send(result);
                // Ordinary source refusal is returned to the caller, not a
                // cleanup failure. A panic stays in the registered JoinHandle.
                Ok(())
            }));
            receive
        };
        receive
            .await
            .map_err(|_| refused("Runtime byte preparation worker lost"))?
    }

    async fn retain_torch_runtime_bytes_owned(
        &self,
        tag: &str,
    ) -> Result<Arc<RetainedRuntimeReadSource>> {
        if self.app_id != AppId::Torch
            || tag.is_empty()
            || tag.len() > 200
            || !tag
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'+'))
            || tag == "."
            || tag == ".."
        {
            return Err(refused("Invalid Torch runtime selection"));
        }
        let versions = self.versions_dir();
        let lock = TorchVersionsLock::try_acquire_read(&versions).map_err(PumasError::from)?;
        {
            let mut state = self.state.write().await;
            state.refresh_with_lock(&lock).await?;
            if !state
                .get_installed_tags()
                .iter()
                .any(|installed| installed == tag)
            {
                return Err(refused("Torch runtime is not installed"));
            }
        }
        let runtime = versions.join(tag);
        let launcher = self.launcher_root.clone();
        tokio::task::spawn_blocking(move || capture_installed(&launcher, &runtime, lock))
            .await
            .map_err(|error| refused(format!("Runtime byte capture task failed: {error}")))?
    }
}

fn refused(message: impl Into<String>) -> PumasError {
    PumasError::Config {
        message: message.into(),
    }
}

fn strict_directory(path: &Path) -> io::Result<File> {
    if std::fs::canonicalize(path)? != path {
        return Err(io::Error::other(
            "runtime root contains a link or noncanonical parent",
        ));
    }
    let mut options = std::fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options
            .custom_flags(nix::libc::O_NOFOLLOW | nix::libc::O_DIRECTORY | nix::libc::O_NONBLOCK);
    }
    let file = options.open(path)?;
    if !file.metadata()?.is_dir() {
        return Err(io::Error::other("runtime root is not a directory"));
    }
    Ok(file)
}

fn bounded_json(path: &Path, maximum: u64) -> Result<Vec<u8>> {
    let metadata = std::fs::symlink_metadata(path).map_err(PumasError::from)?;
    if !metadata.is_file() || metadata.file_type().is_symlink() || metadata.len() > maximum {
        return Err(refused("Runtime metadata is missing, linked or oversized"));
    }
    std::fs::read(path).map_err(PumasError::from)
}

fn file_manifest(path: &Path, relative: String) -> Result<RuntimeReadFile> {
    let mut file = File::open(path).map_err(PumasError::from)?;
    let mut sha = Sha256::new();
    let mut size = 0u64;
    let mut buffer = [0; 65536];
    loop {
        let count = file.read(&mut buffer).map_err(PumasError::from)?;
        if count == 0 {
            break;
        }
        size = size
            .checked_add(count as u64)
            .ok_or_else(|| refused("Runtime byte length overflow"))?;
        sha.update(&buffer[..count]);
    }
    RuntimeReadFile::new(relative, size, format!("{:x}", sha.finalize())).map_err(PumasError::from)
}

// These namespaces are never executable/read inputs to the owned -I -S -B
// worker. They remain present and identity-tracked, but get no content grant.
// This is source-fixed selection, not a caller-controlled ignore list.
fn inert_import_member(path: &str) -> bool {
    path.split('/').any(|name| {
        name == "__pycache__"
            || name.ends_with(".pyc")
            || name.ends_with(".pyo")
            || name.ends_with(".pth")
            || matches!(name, "sitecustomize.py" | "usercustomize.py")
    })
}

// Only the 42 observed generated entrypoints and their RECORDs vary with the
// installer staging path. They are never imports or runtime program inputs.
// Keep full installer validation and namespace identity, but grant no content
// read for these exact source-fixed names. All other metadata remains selected.
fn inert_dependency_member(path: &str) -> bool {
    static NAMES: std::sync::LazyLock<Vec<String>> = std::sync::LazyLock::new(|| {
        let pin: serde_json::Value = serde_json::from_str(
            pumas_library::runtime_read_source::AUDIO_RUNTIME_CANDIDATE_RECIPE,
        )
        .expect("source-fixed audio recipe JSON");
        serde_json::from_value(pin["dependency_inert_members"].clone())
            .expect("source-fixed inert dependency names")
    });
    inert_import_member(path) || NAMES.iter().any(|name| name == path)
}

fn tree_manifest(
    root: &Path,
    excluded: &[String],
    aliases: bool,
) -> Result<(Vec<RuntimeReadFile>, Vec<String>)> {
    let mut members = Vec::new();
    let mut omissions = excluded.to_vec();
    let walker = walkdir::WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .filter_entry(|entry| {
            entry.path() == root
                || !excluded
                    .iter()
                    .any(|name| entry.path().starts_with(root.join(name)))
        });
    let mut entries_seen = 0usize;
    for entry in walker {
        let entry = entry.map_err(|error| refused(error.to_string()))?;
        if entry.path() == root {
            continue;
        }
        entries_seen += 1;
        if entries_seen > 200_000 {
            return Err(refused("Runtime selection exceeds namespace bound"));
        }
        let relative = entry
            .path()
            .strip_prefix(root)
            .map_err(|_| refused("Runtime member escaped root"))?
            .to_str()
            .ok_or_else(|| refused("Runtime member is not UTF-8"))?
            .replace(std::path::MAIN_SEPARATOR, "/");
        if relative.len() > 1024 {
            return Err(refused("Runtime member path exceeds bound"));
        }
        if entry.file_type().is_symlink() {
            let target = std::fs::canonicalize(entry.path()).map_err(PumasError::from)?;
            // UV creates this exact minor-version directory alias alongside
            // the pinned full distribution. Do not follow it or select bytes
            // through it; retain the link identity as an omission. Every other
            // directory alias and every external alias remains refused.
            let fixed_directory_alias = relative == "cpython-3.12-linux-x86_64-gnu"
                && target == root.join("cpython-3.12.14-linux-x86_64-gnu")
                && target.is_dir();
            if !aliases || !target.starts_with(root) || !(target.is_file() || fixed_directory_alias)
            {
                return Err(refused("Unsupported external or directory runtime alias"));
            }
            omissions.push(relative);
        } else if entry.file_type().is_file() {
            if inert_import_member(&relative) {
                omissions.push(relative);
            } else {
                members.push(file_manifest(entry.path(), relative)?);
            }
        } else if !entry.file_type().is_dir() {
            return Err(refused("Runtime selection contains a special file"));
        }
        if members.len() + omissions.len() > 200_000 {
            return Err(refused("Runtime selection exceeds member bound"));
        }
    }
    Ok((members, omissions))
}

fn capture_installed(
    launcher: &Path,
    runtime: &Path,
    lock: TorchVersionsLock,
) -> Result<Arc<RetainedRuntimeReadSource>> {
    capture_installed_with_native(launcher, runtime, lock, None)
}

fn capture_installed_with_native(
    launcher: &Path,
    runtime: &Path,
    lock: TorchVersionsLock,
    native: Option<RuntimeReadRoot>,
) -> Result<Arc<RetainedRuntimeReadSource>> {
    if !cfg!(target_os = "linux") {
        return Err(refused(
            "Installed runtime byte custody supports Linux only",
        ));
    }
    let sidecar_dir = strict_directory(runtime).map_err(PumasError::from)?;
    let recipe: serde_json::Value =
        serde_json::from_slice(&bounded_json(&runtime.join("runtime.json"), 1024 * 1024)?)
            .map_err(|error| refused(error.to_string()))?;
    let minor = recipe["python"]
        .as_str()
        .and_then(|value| value.strip_prefix("python"))
        .ok_or_else(|| refused("Missing managed Python minor"))?;
    let components: Vec<_> = minor.split('.').collect();
    if components.len() != 2
        || components[0] != "3"
        || components[1].is_empty()
        || !components[1].bytes().all(|b| b.is_ascii_digit())
    {
        return Err(refused("Unsupported Python minor"));
    }
    let executable = PathBuf::from(
        recipe["managed_python"]["executable"]["path"]
            .as_str()
            .ok_or_else(|| refused("Legacy runtime has no managed interpreter"))?,
    );
    let depot_parent = launcher.join("launcher-data/managed-python/python");
    let relative = executable
        .strip_prefix(&depot_parent)
        .map_err(|_| refused("External interpreter cannot be retained"))?;
    let depot_name = relative
        .components()
        .next()
        .and_then(|part| part.as_os_str().to_str())
        .ok_or_else(|| refused("Missing managed depot identity"))?;
    if depot_name.len() != 64 || !depot_name.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(refused("Unsupported managed depot identity"));
    }
    let depot = depot_parent.join(depot_name);
    let depot_lease = Arc::new(ManagedDepotLease::read(&depot).map_err(PumasError::from)?);
    let interpreter_dir = strict_directory(&depot).map_err(PumasError::from)?;
    if std::fs::canonicalize(&executable).map_err(PumasError::from)? != executable
        || !executable.is_file()
        || std::fs::canonicalize(runtime.join("venv/bin/python")).map_err(PumasError::from)?
            != executable
    {
        return Err(refused(
            "Installed venv does not select the owned interpreter",
        ));
    }
    let interpreter = file_manifest(&executable, "executable".into())?;
    if recipe["managed_python"]["executable"]["sha256"].as_str() != Some(interpreter.sha256()) {
        return Err(refused("Managed interpreter bytes changed"));
    }
    let packages = runtime
        .join("venv/lib")
        .join(format!("python{minor}"))
        .join("site-packages");
    let packages_dir = strict_directory(&packages).map_err(PumasError::from)?;
    let installed: StagedFilesManifest = serde_json::from_slice(&bounded_json(
        &runtime.join("installed-files.json"),
        64 * 1024 * 1024,
    )?)
    .map_err(|error| refused(error.to_string()))?;
    installer::validate_staged_files(&packages, &installed)?;
    let dependency_omissions = installed
        .files
        .iter()
        .filter(|file| inert_dependency_member(&file.path))
        .map(|file| file.path.clone())
        .collect::<Vec<_>>();
    let dependencies = installed
        .files
        .into_iter()
        .filter(|file| !inert_dependency_member(&file.path))
        .map(|file| RuntimeReadFile::new(file.path, file.size, file.sha256))
        .collect::<io::Result<Vec<_>>>()
        .map_err(PumasError::from)?;
    let expected = tempfile::tempdir().map_err(PumasError::from)?;
    installer::write_embedded_torch_runtime(expected.path())?;
    let (embedded, _) = tree_manifest(expected.path(), &[], false)?;
    let code_names: BTreeSet<_> = embedded
        .iter()
        .filter(|member| member.path().ends_with(".py"))
        .map(|member| member.path().to_owned())
        .collect();
    for member in embedded
        .iter()
        .filter(|member| member.path().ends_with(".py"))
    {
        let installed = runtime.join(member.path());
        // Direct/resolved installers remove this installer-only script. The
        // bundled installer retains it, and then its exact bytes still matter.
        if member.path() == "validate_runtime.py"
            && matches!(std::fs::symlink_metadata(&installed), Err(error) if error.kind() == io::ErrorKind::NotFound)
        {
            continue;
        }
        if file_manifest(&installed, member.path().into())? != *member {
            return Err(refused(
                "Installed sidecar differs from embedded selected code",
            ));
        }
    }
    let (sidecar, sidecar_omissions) = tree_manifest(runtime, &["venv".into()], false)?;
    if sidecar
        .iter()
        .any(|member| member.path().ends_with(".py") && !code_names.contains(member.path()))
    {
        return Err(refused(
            "Installed sidecar contains unreported executable Python",
        ));
    }
    let (interpreter_members, interpreter_omissions) = tree_manifest(&depot, &[], true)?;
    let shared_lock = Arc::new(lock);
    let mut selections = vec![
        RuntimeReadRoot::new(
            RuntimeReadRole::Interpreter,
            interpreter_dir,
            interpreter_members,
            interpreter_omissions,
            depot_lease,
        ),
        RuntimeReadRoot::new(
            RuntimeReadRole::Dependencies,
            packages_dir,
            dependencies,
            dependency_omissions,
            shared_lock.clone(),
        ),
        RuntimeReadRoot::new(
            RuntimeReadRole::Sidecar,
            sidecar_dir,
            sidecar,
            sidecar_omissions,
            shared_lock,
        ),
    ]
    .into_iter()
    .collect::<io::Result<Vec<_>>>()
    .map_err(PumasError::from)?;
    if let Some(native) = native {
        selections.push(native);
    }
    RetainedRuntimeReadSource::capture(selections).map_err(PumasError::from)
}

#[cfg(all(test, target_os = "linux"))]
#[path = "torch_read_source/tests.rs"]
mod tests;
