//! The Torch sidecar shipped with the running Pumas.
//!
//! An installed Torch runtime supplies only the interpreter and packages. The
//! sidecar code (`serve.py`, its route modules, `loaders/`, and the
//! install-time resolver, probe and compatibility check) is compiled into this
//! binary, so every distribution carries it. It is materialized once per
//! content identity under `launcher-data/torch-sidecar/<pumas version>-<digest>/`
//! and every runtime, of any version, runs from that directory. Upgrading Pumas
//! changes the identity, so each installed runtime runs the new sidecar with no
//! reinstall.
//!
//! The directory is a disposable cache derived from this binary: it is
//! published whole by a non-replacing rename, never modified in place, compared
//! with the embedded bytes before every use, and replaced when it differs.
//! Sidecars run with `python -B` and keep their state in their runtime and
//! profile directories, so concurrent Torch instances never write next to the
//! shared scripts. Directories of other identities (earlier Pumas builds) are
//! pruned when the current one is prepared.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock, PoisonError};
use std::time::{Duration, Instant, SystemTime};

use serde::Deserialize;
use sha2::{Digest, Sha256};

use crate::{PumasError, Result};

/// Directory under `launcher-data` holding materialized sidecars.
pub const TORCH_SIDECAR_DIR: &str = "torch-sidecar";
/// The sidecar's entry script.
pub const TORCH_SIDECAR_ENTRY: &str = "serve.py";
/// The stdlib-only script that checks a runtime before launch.
const COMPATIBILITY_SCRIPT: &str = "runtime_compat.py";
/// A Python startup plus metadata lookups; generous for slow disks.
const COMPATIBILITY_TIMEOUT: Duration = Duration::from_secs(60);
/// Bound on what the check may print; its report is a few hundred bytes.
const COMPATIBILITY_OUTPUT_LIMIT: u64 = 64 * 1024;
/// A staging directory this old belongs to no running publication.
const ABANDONED_STAGING_AGE: Duration = Duration::from_secs(60 * 60);
const STAGING_PREFIX: &str = ".staging-";
const REPLACED_PREFIX: &str = ".replaced-";

/// Every file of the sidecar, by its path relative to the sidecar root.
/// `torch-server/tests/test_embedded_runtime_imports.py` reads this table.
const SIDECAR_FILES: &[(&str, &str)] = &[
    ("LICENSE", include_str!("../../../../../LICENSE")),
    (
        "serve.py",
        include_str!("../../../../../torch-server/serve.py"),
    ),
    (
        "runtime_compat.py",
        include_str!("../../../../../torch-server/runtime_compat.py"),
    ),
    (
        "resolve_runtime.py",
        include_str!("../../../../../torch-server/resolve_runtime.py"),
    ),
    (
        "probe_runtime.py",
        include_str!("../../../../../torch-server/probe_runtime.py"),
    ),
    (
        "nunchaku_compat.py",
        include_str!("../../../../../torch-server/nunchaku_compat.py"),
    ),
    (
        "control_api.py",
        include_str!("../../../../../torch-server/control_api.py"),
    ),
    (
        "device_manager.py",
        include_str!("../../../../../torch-server/device_manager.py"),
    ),
    (
        "diffusion.py",
        include_str!("../../../../../torch-server/diffusion.py"),
    ),
    (
        "flux2.py",
        include_str!("../../../../../torch-server/flux2.py"),
    ),
    (
        "image_api.py",
        include_str!("../../../../../torch-server/image_api.py"),
    ),
    (
        "audio_api.py",
        include_str!("../../../../../torch-server/audio_api.py"),
    ),
    (
        "model_manager.py",
        include_str!("../../../../../torch-server/model_manager.py"),
    ),
    (
        "audio_input.py",
        include_str!("../../../../../torch-server/audio_input.py"),
    ),
    (
        "audio_contract.py",
        include_str!("../../../../../torch-server/audio_contract.py"),
    ),
    (
        "text_api.py",
        include_str!("../../../../../torch-server/text_api.py"),
    ),
    (
        "loaders/text_encoder_loader.py",
        include_str!("../../../../../torch-server/loaders/text_encoder_loader.py"),
    ),
    (
        "openai_api.py",
        include_str!("../../../../../torch-server/openai_api.py"),
    ),
    (
        "validation.py",
        include_str!("../../../../../torch-server/validation.py"),
    ),
    (
        "loaders/__init__.py",
        include_str!("../../../../../torch-server/loaders/__init__.py"),
    ),
    (
        "loaders/cohere_asr_loader.py",
        include_str!("../../../../../torch-server/loaders/cohere_asr_loader.py"),
    ),
    (
        "loaders/dllm_loader.py",
        include_str!("../../../../../torch-server/loaders/dllm_loader.py"),
    ),
    (
        "loaders/safetensors_loader.py",
        include_str!("../../../../../torch-server/loaders/safetensors_loader.py"),
    ),
    (
        "loaders/sherry_loader.py",
        include_str!("../../../../../torch-server/loaders/sherry_loader.py"),
    ),
];

/// Subdirectories of the sidecar root.
const SIDECAR_DIRECTORIES: &[&str] = &["loaders"];

/// Serializes publication within this process. One Pumas owns a launcher root
/// at a time; the non-replacing rename keeps any other writer from corrupting
/// a published directory.
static PUBLICATION: Mutex<()> = Mutex::new(());

/// The sidecar's content identity: this Pumas version plus a digest of every
/// embedded file, so a rebuild with different sidecar bytes never reuses a
/// directory.
pub fn torch_sidecar_id() -> &'static str {
    static ID: OnceLock<String> = OnceLock::new();
    ID.get_or_init(|| {
        let mut digest = Sha256::new();
        for (name, contents) in SIDECAR_FILES {
            digest.update(name.as_bytes());
            digest.update([0]);
            digest.update((contents.len() as u64).to_le_bytes());
            digest.update(contents.as_bytes());
        }
        let digest = hex::encode(digest.finalize());
        format!("{}-{}", env!("CARGO_PKG_VERSION"), &digest[..16])
    })
}

/// The directory this Pumas runs its Torch sidecar from.
pub fn torch_sidecar_dir(launcher_root: &Path) -> PathBuf {
    sidecar_parent(launcher_root).join(torch_sidecar_id())
}

/// The entry script every Torch launch from this Pumas runs.
pub fn torch_sidecar_entry(launcher_root: &Path) -> PathBuf {
    torch_sidecar_dir(launcher_root).join(TORCH_SIDECAR_ENTRY)
}

fn sidecar_parent(launcher_root: &Path) -> PathBuf {
    launcher_root.join("launcher-data").join(TORCH_SIDECAR_DIR)
}

/// Make the sidecar directory match the embedded sidecar and return it.
///
/// Blocking: call from a blocking context.
pub fn materialize_torch_sidecar(launcher_root: &Path) -> Result<PathBuf> {
    let _publication = PUBLICATION.lock().unwrap_or_else(PoisonError::into_inner);
    let parent = sidecar_parent(launcher_root);
    std::fs::create_dir_all(&parent).map_err(|e| PumasError::io_with_path(e, &parent))?;
    let target = parent.join(torch_sidecar_id());
    if !matches_embedded(&target)? {
        set_aside(&parent, &target)?;
        publish(&parent, &target)?;
    }
    prune(&parent);
    Ok(target)
}

/// Whether `target` holds exactly the embedded files and nothing else. An
/// extra file would be importable by the sidecar, so it is a difference.
fn matches_embedded(target: &Path) -> Result<bool> {
    match std::fs::symlink_metadata(target) {
        Ok(metadata) if metadata.is_dir() => {}
        Ok(_) => return Ok(false),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(PumasError::io_with_path(error, target)),
    }
    let mut seen = 0;
    let mut pending = vec![(target.to_path_buf(), String::new())];
    while let Some((directory, prefix)) = pending.pop() {
        let entries =
            std::fs::read_dir(&directory).map_err(|e| PumasError::io_with_path(e, &directory))?;
        for entry in entries {
            let entry = entry.map_err(|e| PumasError::io_with_path(e, &directory))?;
            let Some(name) = entry
                .file_name()
                .to_str()
                .map(|name| format!("{prefix}{name}"))
            else {
                return Ok(false);
            };
            let kind = entry
                .file_type()
                .map_err(|e| PumasError::io_with_path(e, entry.path()))?;
            if kind.is_dir() && SIDECAR_DIRECTORIES.contains(&name.as_str()) {
                pending.push((entry.path(), format!("{name}/")));
                continue;
            }
            let expected = SIDECAR_FILES
                .iter()
                .find_map(|(file, contents)| (*file == name).then_some(contents));
            let Some(expected) = expected.filter(|_| kind.is_file()) else {
                return Ok(false);
            };
            let path = entry.path();
            let actual = std::fs::read(&path).map_err(|e| PumasError::io_with_path(e, &path))?;
            if actual != expected.as_bytes() {
                return Ok(false);
            }
            seen += 1;
        }
    }
    Ok(seen == SIDECAR_FILES.len())
}

/// Move a differing `target` out of the way; it is pruned afterwards.
fn set_aside(parent: &Path, target: &Path) -> Result<()> {
    if std::fs::symlink_metadata(target).is_err() {
        return Ok(());
    }
    let aside = parent.join(format!(
        "{REPLACED_PREFIX}{}-{}",
        torch_sidecar_id(),
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    ));
    tracing::warn!(
        path = %target.display(),
        "Torch sidecar directory differs from this Pumas build; replacing it"
    );
    std::fs::rename(target, &aside).map_err(|e| PumasError::io_with_path(e, target))
}

/// Write the embedded sidecar into a staging directory and publish it whole.
fn publish(parent: &Path, target: &Path) -> Result<()> {
    let staging = tempfile::Builder::new()
        .prefix(STAGING_PREFIX)
        .tempdir_in(parent)
        .map_err(|e| PumasError::io_with_path(e, parent))?;
    for directory in SIDECAR_DIRECTORIES {
        let path = staging.path().join(directory);
        std::fs::create_dir(&path).map_err(|e| PumasError::io_with_path(e, &path))?;
    }
    for (name, contents) in SIDECAR_FILES {
        let path = staging.path().join(name);
        std::fs::write(&path, contents).map_err(|e| PumasError::io_with_path(e, &path))?;
    }
    match crate::platform::filesystem::rename_directory_noreplace(staging.path(), target) {
        Ok(()) => Ok(()),
        // Another writer published first; accept it only if it is ours.
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            if matches_embedded(target)? {
                Ok(())
            } else {
                Err(PumasError::Other(format!(
                    "Torch sidecar directory {} was replaced by different files while Pumas published it",
                    target.display()
                )))
            }
        }
        Err(error) => Err(PumasError::io_with_path(error, target)),
    }
}

/// Remove sidecars of other identities and abandoned publication leftovers.
/// Best effort: a directory still open elsewhere is retried on a later launch.
fn prune(parent: &Path) {
    let Ok(entries) = std::fs::read_dir(parent) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name == torch_sidecar_id() {
            continue;
        }
        let abandoned = || {
            entry
                .metadata()
                .and_then(|metadata| metadata.modified())
                .ok()
                .and_then(|modified| modified.elapsed().ok())
                .is_some_and(|age| age >= ABANDONED_STAGING_AGE)
        };
        if name.starts_with(STAGING_PREFIX) && !abandoned() {
            continue;
        }
        let path = entry.path();
        let removed = match entry.file_type() {
            Ok(kind) if kind.is_dir() => std::fs::remove_dir_all(&path),
            Ok(_) => std::fs::remove_file(&path),
            Err(error) => Err(error),
        };
        if let Err(error) = removed {
            tracing::debug!(%error, path = %path.display(), "Could not prune a Torch sidecar directory");
        }
    }
}

#[derive(Deserialize)]
struct CompatibilityReport {
    problems: Vec<String>,
}

/// Refuse, with every reason, a runtime whose interpreter or packages cannot
/// run this sidecar. Runs the sidecar's stdlib-only check under the runtime's
/// interpreter, isolated and without writing bytecode.
///
/// Blocking: call from a blocking context.
pub fn check_torch_runtime_compatibility(python: &Path, sidecar_dir: &Path) -> Result<()> {
    let refuse = |message: String| PumasError::LaunchFailed {
        app: "torch".into(),
        message,
    };
    let mut command = std::process::Command::new(python);
    crate::platform::child_environment::apply_owned_child_environment(&mut command);
    command
        .args(["-I", "-B"])
        .arg(sidecar_dir.join(COMPATIBILITY_SCRIPT))
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    let mut child = command.spawn().map_err(|error| {
        refuse(format!(
            "the Torch runtime interpreter {} cannot start: {error}",
            python.display()
        ))
    })?;
    let read = |stream: Option<Box<dyn Read + Send>>| {
        std::thread::spawn(move || {
            let mut output = Vec::new();
            if let Some(stream) = stream {
                let _ = stream
                    .take(COMPATIBILITY_OUTPUT_LIMIT)
                    .read_to_end(&mut output);
            }
            output
        })
    };
    let stdout = read(child.stdout.take().map(|s| Box::new(s) as _));
    let stderr = read(child.stderr.take().map(|s| Box::new(s) as _));
    let deadline = Instant::now() + COMPATIBILITY_TIMEOUT;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(25)),
            Ok(None) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(refuse(format!(
                    "the Torch runtime compatibility check did not finish within {} seconds",
                    COMPATIBILITY_TIMEOUT.as_secs()
                )));
            }
            Err(error) => {
                let _ = child.kill();
                return Err(refuse(format!(
                    "the Torch runtime compatibility check could not be observed: {error}"
                )));
            }
        }
    };
    let stdout = stdout.join().unwrap_or_default();
    let stderr = stderr.join().unwrap_or_default();
    let report = status
        .success()
        .then(|| serde_json::from_slice::<CompatibilityReport>(&stdout).ok())
        .flatten();
    let Some(report) = report else {
        let detail = String::from_utf8_lossy(&stderr);
        let detail = detail
            .trim()
            .lines()
            .last()
            .unwrap_or("no diagnostic output");
        return Err(refuse(format!(
            "the Torch runtime compatibility check failed ({status}): {detail}"
        )));
    };
    if report.problems.is_empty() {
        return Ok(());
    }
    Err(refuse(format!(
        "this Torch runtime cannot run the sidecar shipped with Pumas {}: {}. \
         Remove and reinstall this Torch version, or install another, to get packages it supports",
        env!("CARGO_PKG_VERSION"),
        report.problems.join("; ")
    )))
}

/// Materialize this Pumas's sidecar and check that the runtime in
/// `version_dir` can run it. Returns the entry script to launch.
///
/// Blocking: call from a blocking context.
pub fn prepare_torch_sidecar_blocking(launcher_root: &Path, version_dir: &Path) -> Result<PathBuf> {
    let sidecar = materialize_torch_sidecar(launcher_root)?;
    check_torch_runtime_compatibility(&crate::platform::paths::venv_python(version_dir), &sidecar)?;
    Ok(sidecar.join(TORCH_SIDECAR_ENTRY))
}

/// [`prepare_torch_sidecar_blocking`] on the blocking pool.
pub async fn prepare_torch_sidecar(launcher_root: &Path, version_dir: &Path) -> Result<PathBuf> {
    let launcher_root = launcher_root.to_path_buf();
    let version_dir = version_dir.to_path_buf();
    tokio::task::spawn_blocking(move || {
        prepare_torch_sidecar_blocking(&launcher_root, &version_dir)
    })
    .await
    .map_err(|error| PumasError::Other(format!("Torch sidecar preparation task failed: {error}")))?
}

/// [`materialize_torch_sidecar`] on the blocking pool.
pub async fn materialize_torch_sidecar_async(launcher_root: &Path) -> Result<PathBuf> {
    let launcher_root = launcher_root.to_path_buf();
    tokio::task::spawn_blocking(move || materialize_torch_sidecar(&launcher_root))
        .await
        .map_err(|error| {
            PumasError::Other(format!(
                "Torch sidecar materialization task failed: {error}"
            ))
        })?
}

#[cfg(test)]
#[path = "torch_sidecar_tests.rs"]
mod tests;
