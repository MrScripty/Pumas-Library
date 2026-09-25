//! Private, pinned uv bootstrap for Pumas-managed standard CPython.
//!
//! This provider is staged for the manager's automatic candidate-selection flow.
//! Its archive and download cache live under the caller-owned root; each invocation
//! verifies the pinned archive and checks the private uv binary against its contents.

use futures::StreamExt;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::io::{Cursor, Read, Write};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;
use tokio::process::Command;

use super::installer::TorchCleanupTasks;

const UV_VERSION: &str = "0.12.18";
const UV_BASE_URL: &str = "https://releases.astral.sh/github/uv/releases/download/0.12.18/";
const MAX_ARCHIVE_BYTES: usize = 40 * 1024 * 1024;
const MAX_BINARY_BYTES: u64 = 100 * 1024 * 1024;
const MAX_CATALOG_BYTES: usize = 1024 * 1024;
const MAX_CATALOG_RECORDS: usize = 256;
const MIN_CPYTHON_MINOR: u32 = 10;
const BOOTSTRAP_TIMEOUT: Duration = Duration::from_secs(120);
const INSTALL_TIMEOUT: Duration = Duration::from_secs(300);
const QUERY_TIMEOUT: Duration = Duration::from_secs(20);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ManagedPythonFailureKind {
    UnsupportedTarget,
    InvalidMinor,
    ArtifactIntegrity,
    Inconclusive,
}

#[derive(Debug)]
pub(super) struct ManagedPythonFailure {
    #[allow(dead_code)] // The manager will retain typed failure categories when wiring outcomes.
    pub kind: ManagedPythonFailureKind,
    pub message: &'static str,
}

impl ManagedPythonFailure {
    fn new(kind: ManagedPythonFailureKind, message: &'static str) -> Self {
        Self { kind, message }
    }
}

type ProviderResult<T> = Result<T, ManagedPythonFailure>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum NativeTarget {
    LinuxX8664,
    WindowsX8664,
    MacosArm64,
}

impl NativeTarget {
    fn triple(self) -> &'static str {
        match self {
            Self::LinuxX8664 => "x86_64-unknown-linux-gnu",
            Self::WindowsX8664 => "x86_64-pc-windows-msvc",
            Self::MacosArm64 => "aarch64-apple-darwin",
        }
    }

    fn current() -> ProviderResult<Self> {
        if cfg!(all(
            target_os = "linux",
            target_arch = "x86_64",
            target_env = "gnu"
        )) {
            Ok(Self::LinuxX8664)
        } else if cfg!(all(
            target_os = "windows",
            target_arch = "x86_64",
            target_env = "msvc"
        )) {
            Ok(Self::WindowsX8664)
        } else if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
            Ok(Self::MacosArm64)
        } else {
            Err(ManagedPythonFailure::new(
                ManagedPythonFailureKind::UnsupportedTarget,
                "No pinned managed uv artifact exists for this target",
            ))
        }
    }

    fn pin(self) -> UvPin {
        match self {
            Self::LinuxX8664 => UvPin {
                archive: "uv-x86_64-unknown-linux-gnu.tar.gz",
                sha256: "89eadd7c76fc063887959510d5ba0ab1264dfd5f1143b925ddb73021a40acf16",
                binary: "uv",
                os: "linux",
                arch: "x86_64",
                libc: Some("gnu"),
            },
            Self::WindowsX8664 => UvPin {
                archive: "uv-x86_64-pc-windows-msvc.zip",
                sha256: "cae6a3bc25239f83dffb467a4b180508d9da23986c04639ebfa44e43e6a84bff",
                binary: "uv.exe",
                os: "windows",
                arch: "x86_64",
                libc: Some("none"),
            },
            Self::MacosArm64 => UvPin {
                archive: "uv-aarch64-apple-darwin.tar.gz",
                sha256: "cf40e0c6a202190ccd9e0406dcfdd5b2d6668a9a5c779b17948963df32aafe5b",
                binary: "uv",
                os: "macos",
                arch: "aarch64",
                libc: Some("none"),
            },
        }
    }

    fn accepts_observed(self, value: &ObservedPython, minor: &str) -> bool {
        let platform_matches = match self {
            Self::LinuxX8664 => {
                value.platform == "linux" && value.machine.eq_ignore_ascii_case("x86_64")
            }
            Self::WindowsX8664 => {
                value.platform == "win32"
                    && (value.machine.eq_ignore_ascii_case("amd64")
                        || value.machine.eq_ignore_ascii_case("x86_64"))
            }
            Self::MacosArm64 => {
                value.platform == "darwin" && value.machine.eq_ignore_ascii_case("arm64")
            }
        };
        platform_matches
            && value.implementation == "cpython"
            && value.releaselevel == "final"
            && value.bits == 64
            && value.version.starts_with(&format!("{minor}."))
            && stable_version(&value.version).is_some()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct UvPin {
    archive: &'static str,
    sha256: &'static str,
    binary: &'static str,
    os: &'static str,
    arch: &'static str,
    libc: Option<&'static str>,
}

impl UvPin {
    fn url(self) -> String {
        format!("{UV_BASE_URL}{}", self.archive)
    }

    fn valid(self) -> bool {
        if ![
            NativeTarget::LinuxX8664.pin(),
            NativeTarget::WindowsX8664.pin(),
            NativeTarget::MacosArm64.pin(),
        ]
        .contains(&self)
        {
            return false;
        }
        let Ok(url) = reqwest::Url::parse(&self.url()) else {
            return false;
        };
        url.scheme() == "https"
            && url.host_str() == Some("releases.astral.sh")
            && url.port().is_none()
            && url.username().is_empty()
            && url.password().is_none()
            && url.query().is_none()
            && url.fragment().is_none()
            && url.path() == format!("/github/uv/releases/download/{UV_VERSION}/{}", self.archive)
            && self.sha256.len() == 64
            && self.sha256.bytes().all(|byte| byte.is_ascii_hexdigit())
    }

    fn verify_archive(self, bytes: &[u8]) -> bool {
        self.valid()
            && bytes.len() <= MAX_ARCHIVE_BYTES
            && format!("{:x}", Sha256::digest(bytes)) == self.sha256
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct ManagedPythonCandidate {
    pub minor: String,
    pub version: String,
    pub catalog_key: String,
    pub source_url: String,
}

#[derive(Clone, Debug)]
pub(super) struct ManagedPythonInterpreter {
    pub minor: String,
    pub version: String,
    pub catalog_key: String,
    pub source_url: String,
    pub executable: PathBuf,
}

#[derive(Clone, Debug)]
pub(crate) struct ManagedPythonIdentity {
    pub python: String,
    pub version: String,
    pub catalog_key: String,
    pub source_url: String,
    pub executable: PathBuf,
    pub target_triple: String,
    pub uv_version: String,
    pub uv_archive_sha256: String,
}

impl ManagedPythonIdentity {
    fn from_install(installed: ManagedPythonInterpreter, target: NativeTarget) -> Self {
        Self {
            python: format!("python{}", installed.minor),
            version: installed.version,
            catalog_key: installed.catalog_key,
            source_url: installed.source_url,
            executable: installed.executable,
            target_triple: target.triple().into(),
            uv_version: UV_VERSION.into(),
            uv_archive_sha256: target.pin().sha256.into(),
        }
    }
}

pub(super) async fn ensure_managed_torch_interpreter(
    root: impl AsRef<Path>,
    cleanup: &Arc<TorchCleanupTasks>,
    minor: &str,
) -> ProviderResult<ManagedPythonIdentity> {
    let provider = ManagedPythonProvider::new(root, cleanup.clone())?;
    let installed = provider.ensure_minor(minor).await?;
    Ok(ManagedPythonIdentity::from_install(
        installed,
        provider.target,
    ))
}

pub(super) async fn list_managed_torch_candidates(
    root: impl AsRef<Path>,
    cleanup: &Arc<TorchCleanupTasks>,
) -> ProviderResult<Vec<ManagedPythonCandidate>> {
    ManagedPythonProvider::new(root, cleanup.clone())?
        .stable_candidates()
        .await
}

pub(super) struct ManagedPythonProvider {
    root: PathBuf,
    target: NativeTarget,
    cleanup: Arc<TorchCleanupTasks>,
}

impl ManagedPythonProvider {
    pub fn new(root: impl AsRef<Path>, cleanup: Arc<TorchCleanupTasks>) -> ProviderResult<Self> {
        let target = NativeTarget::current()?;
        std::fs::create_dir_all(root.as_ref()).map_err(|_| {
            ManagedPythonFailure::new(
                ManagedPythonFailureKind::Inconclusive,
                "Managed Python root could not be created",
            )
        })?;
        let root = std::fs::canonicalize(root.as_ref()).map_err(|_| {
            ManagedPythonFailure::new(
                ManagedPythonFailureKind::Inconclusive,
                "Managed Python root could not be resolved",
            )
        })?;
        private_directory(&root, "python")?;
        private_directory(&root, "tmp")?;
        Ok(Self {
            root,
            target,
            cleanup,
        })
    }

    /// Read the pinned uv catalog without probing a host interpreter.
    pub async fn stable_candidates(&self) -> ProviderResult<Vec<ManagedPythonCandidate>> {
        let staged = self.stage_uv().await?;
        self.read_catalog(&staged).await
    }

    async fn read_catalog(&self, staged: &StagedUv) -> ProviderResult<Vec<ManagedPythonCandidate>> {
        let mut command = self.uv_command(&staged.binary, &staged.cache, &self.root.join("python"));
        command.args([
            "python",
            "list",
            "--managed-python",
            "--only-downloads",
            "--show-urls",
            "--output-format",
            "json",
        ]);
        let output = self
            .run_bounded(command, QUERY_TIMEOUT, MAX_CATALOG_BYTES)
            .await?;
        parse_catalog(&output, self.target)
    }

    /// Install one stable standard CPython minor into the private depot.
    pub async fn ensure_minor(&self, minor: &str) -> ProviderResult<ManagedPythonInterpreter> {
        if stable_minor(minor).is_none() {
            return Err(ManagedPythonFailure::new(
                ManagedPythonFailureKind::InvalidMinor,
                "Select a stable CPython minor at or above 3.10",
            ));
        }
        let staged = self.stage_uv().await?;
        let selected = self
            .read_catalog(&staged)
            .await?
            .into_iter()
            .find(|candidate| candidate.minor == minor)
            .ok_or_else(|| {
                ManagedPythonFailure::new(
                    ManagedPythonFailureKind::Inconclusive,
                    "Requested CPython minor was absent from the pinned uv catalog",
                )
            })?;
        let depot = private_install_depot(&self.root, self.target.pin(), &selected)?;

        let mut install = self.uv_command(&staged.binary, &staged.cache, &depot);
        install.args([
            "python",
            "install",
            "--managed-python",
            "--no-bin",
            "--no-registry",
        ]);
        install.arg(&selected.version);
        self.run_bounded(install, INSTALL_TIMEOUT, 4096).await?;

        let mut find = self.uv_command(&staged.binary, &staged.cache, &depot);
        find.args([
            "python",
            "find",
            "--managed-python",
            "--no-python-downloads",
        ]);
        find.arg(&selected.version);
        let output = self.run_bounded(find, QUERY_TIMEOUT, 4096).await?;
        let path = std::str::from_utf8(&output)
            .ok()
            .and_then(|text| {
                let text = text.trim();
                (!text.is_empty() && !text.contains(['\r', '\n'])).then_some(PathBuf::from(text))
            })
            .ok_or_else(|| {
                ManagedPythonFailure::new(
                    ManagedPythonFailureKind::Inconclusive,
                    "Managed interpreter location was not reported",
                )
            })?;
        let executable = canonical_interpreter_in_depot(&path, &depot)?;
        let mut probe = Command::new(&executable);
        self.private_environment(&mut probe, &staged.cache, &depot);
        probe.args(["-I", "-c", PYTHON_IDENTITY_PROBE]);
        let observed = self.run_bounded(probe, QUERY_TIMEOUT, 4096).await?;
        let observed: ObservedPython = serde_json::from_slice(&observed).map_err(|_| {
            ManagedPythonFailure::new(
                ManagedPythonFailureKind::Inconclusive,
                "Managed interpreter identity was malformed",
            )
        })?;
        if !self.target.accepts_observed(&observed, minor) {
            return Err(ManagedPythonFailure::new(
                ManagedPythonFailureKind::ArtifactIntegrity,
                "Managed interpreter identity did not match the requested target",
            ));
        }
        if observed.version != selected.version {
            return Err(ManagedPythonFailure::new(
                ManagedPythonFailureKind::ArtifactIntegrity,
                "Managed interpreter version differed from the selected catalog release",
            ));
        }
        let reported = std::fs::canonicalize(&observed.path).map_err(|_| {
            ManagedPythonFailure::new(
                ManagedPythonFailureKind::ArtifactIntegrity,
                "Managed interpreter reported an invalid executable",
            )
        })?;
        if reported != executable {
            return Err(ManagedPythonFailure::new(
                ManagedPythonFailureKind::ArtifactIntegrity,
                "Managed interpreter reported a different executable",
            ));
        }
        Ok(ManagedPythonInterpreter {
            minor: minor.into(),
            version: observed.version,
            catalog_key: selected.catalog_key,
            source_url: selected.source_url,
            executable,
        })
    }

    fn uv_command(&self, binary: &Path, cache: &Path, depot: &Path) -> Command {
        let mut command = Command::new(binary);
        self.private_environment(&mut command, cache, depot);
        command.arg("--no-config").arg("--cache-dir").arg(cache);
        command
    }

    fn private_environment(&self, command: &mut Command, cache: &Path, depot: &Path) {
        command.env_clear();
        command.current_dir(&self.root);
        command.env("UV_CACHE_DIR", cache);
        command.env("UV_PYTHON_INSTALL_DIR", depot);
        command.env("UV_PYTHON_INSTALL_BIN", "0");
        command.env("UV_PYTHON_INSTALL_REGISTRY", "0");
        command.env("UV_NO_CONFIG", "1");
        command.env("UV_NO_SYSTEM_CONFIG", "1");
        command.env("UV_MANAGED_PYTHON", "1");
        command.env("UV_NO_MODIFY_PATH", "1");
        let temporary = self.root.join("tmp");
        command.env("TMPDIR", &temporary);
        command.env("TEMP", &temporary);
        command.env("TMP", &temporary);
        #[cfg(windows)]
        for key in ["SystemRoot", "WINDIR"] {
            if let Some(value) = std::env::var_os(key) {
                command.env(key, value);
            }
        }
    }

    async fn run_bounded(
        &self,
        command: Command,
        deadline: Duration,
        max_output: usize,
    ) -> ProviderResult<Vec<u8>> {
        run_bounded(&self.root, &self.cleanup, command, deadline, max_output).await
    }

    async fn stage_uv(&self) -> ProviderResult<StagedUv> {
        let pin = self.target.pin();
        if !pin.valid() {
            return Err(ManagedPythonFailure::new(
                ManagedPythonFailureKind::ArtifactIntegrity,
                "Pinned uv artifact metadata is invalid",
            ));
        }
        let bootstrap = private_directory(&self.root, &uv_bootstrap_directory_name(pin))?;
        let cache = private_directory(&self.root, &uv_cache_directory_name(pin))?;
        let archive_path = bootstrap.join(pin.archive);
        let archive = if archive_path.exists() {
            if std::fs::symlink_metadata(&archive_path)
                .is_ok_and(|meta| meta.file_type().is_symlink())
            {
                return Err(ManagedPythonFailure::new(
                    ManagedPythonFailureKind::ArtifactIntegrity,
                    "Pinned uv archive was a link",
                ));
            }
            if std::fs::metadata(&archive_path)
                .is_ok_and(|meta| meta.len() > MAX_ARCHIVE_BYTES as u64)
            {
                return Err(ManagedPythonFailure::new(
                    ManagedPythonFailureKind::ArtifactIntegrity,
                    "Pinned uv archive exceeded its size bound",
                ));
            }
            let bytes = tokio::fs::read(&archive_path).await.map_err(|_| {
                ManagedPythonFailure::new(
                    ManagedPythonFailureKind::Inconclusive,
                    "Pinned uv archive could not be read",
                )
            })?;
            if !pin.verify_archive(&bytes) {
                return Err(ManagedPythonFailure::new(
                    ManagedPythonFailureKind::ArtifactIntegrity,
                    "Pinned uv archive failed SHA-256 verification",
                ));
            }
            bytes
        } else {
            let bytes = download_archive(pin).await?;
            let temporary = tempfile::Builder::new()
                .prefix("uv-download-")
                .tempdir_in(&bootstrap)
                .map_err(|_| {
                    ManagedPythonFailure::new(
                        ManagedPythonFailureKind::Inconclusive,
                        "Private uv staging could not be created",
                    )
                })?;
            let staged_archive = temporary.path().join(pin.archive);
            tokio::fs::write(&staged_archive, &bytes)
                .await
                .map_err(|_| {
                    ManagedPythonFailure::new(
                        ManagedPythonFailureKind::Inconclusive,
                        "Pinned uv archive could not be staged",
                    )
                })?;
            match std::fs::rename(&staged_archive, &archive_path) {
                Ok(()) => {}
                Err(_) if archive_path.exists() => {
                    let existing = tokio::fs::read(&archive_path).await.map_err(|_| {
                        ManagedPythonFailure::new(
                            ManagedPythonFailureKind::Inconclusive,
                            "Concurrent uv archive could not be read",
                        )
                    })?;
                    if !pin.verify_archive(&existing) {
                        return Err(ManagedPythonFailure::new(
                            ManagedPythonFailureKind::ArtifactIntegrity,
                            "Concurrent uv archive failed SHA-256 verification",
                        ));
                    }
                }
                Err(_) => {
                    return Err(ManagedPythonFailure::new(
                        ManagedPythonFailureKind::Inconclusive,
                        "Pinned uv archive could not be published",
                    ))
                }
            }
            bytes
        };
        let temporary = tempfile::Builder::new()
            .prefix("uv-extract-")
            .tempdir_in(&bootstrap)
            .map_err(|_| {
                ManagedPythonFailure::new(
                    ManagedPythonFailureKind::Inconclusive,
                    "Private uv execution directory could not be created",
                )
            })?;
        let extracted = temporary.path().join(pin.binary);
        extract_binary(pin, &archive, &extracted)?;
        let binary = bootstrap.join(pin.binary);
        if binary.exists() {
            if std::fs::symlink_metadata(&binary).is_ok_and(|meta| meta.file_type().is_symlink()) {
                return Err(ManagedPythonFailure::new(
                    ManagedPythonFailureKind::ArtifactIntegrity,
                    "Private uv binary was a link",
                ));
            }
            if !same_binary(&extracted, &binary)? {
                return Err(ManagedPythonFailure::new(
                    ManagedPythonFailureKind::ArtifactIntegrity,
                    "Private uv binary differed from its pinned archive",
                ));
            }
        } else {
            match std::fs::rename(&extracted, &binary) {
                Ok(()) => {}
                Err(_) if binary.exists() && same_binary(&extracted, &binary)? => {}
                Err(_) => {
                    return Err(ManagedPythonFailure::new(
                        ManagedPythonFailureKind::Inconclusive,
                        "Private uv binary could not be published",
                    ))
                }
            }
        }
        Ok(StagedUv { binary, cache })
    }
}

fn uv_bootstrap_directory_name(pin: UvPin) -> String {
    format!("uv-bootstrap-{UV_VERSION}-{}", pin.sha256)
}

fn uv_cache_directory_name(pin: UvPin) -> String {
    format!("uv-cache-{UV_VERSION}-{}", pin.sha256)
}

fn private_directory(root: &Path, name: &str) -> ProviderResult<PathBuf> {
    let path = root.join(name);
    std::fs::create_dir_all(&path).map_err(|_| {
        ManagedPythonFailure::new(
            ManagedPythonFailureKind::Inconclusive,
            "Private managed Python directory could not be created",
        )
    })?;
    let resolved = std::fs::canonicalize(&path).map_err(|_| {
        ManagedPythonFailure::new(
            ManagedPythonFailureKind::Inconclusive,
            "Private managed Python directory could not be resolved",
        )
    })?;
    if !resolved.starts_with(root) || resolved == root {
        return Err(ManagedPythonFailure::new(
            ManagedPythonFailureKind::ArtifactIntegrity,
            "Private managed Python directory escaped its root",
        ));
    }
    Ok(resolved)
}

fn private_install_depot(
    root: &Path,
    pin: UvPin,
    selected: &ManagedPythonCandidate,
) -> ProviderResult<PathBuf> {
    let parent = private_directory(root, "python")?;
    let mut digest = Sha256::new();
    for component in [
        UV_VERSION,
        pin.archive,
        pin.sha256,
        selected.version.as_str(),
        selected.catalog_key.as_str(),
        selected.source_url.as_str(),
    ] {
        digest.update((component.len() as u64).to_le_bytes());
        digest.update(component.as_bytes());
    }
    private_directory(&parent, &format!("{:x}", digest.finalize()))
}

fn canonical_interpreter_in_depot(path: &Path, depot: &Path) -> ProviderResult<PathBuf> {
    let executable = std::fs::canonicalize(path).map_err(|_| {
        ManagedPythonFailure::new(
            ManagedPythonFailureKind::Inconclusive,
            "Managed interpreter location could not be resolved",
        )
    })?;
    if !executable.starts_with(depot) || executable == depot {
        return Err(ManagedPythonFailure::new(
            ManagedPythonFailureKind::ArtifactIntegrity,
            "Interpreter is outside the selected managed Python depot",
        ));
    }
    Ok(executable)
}

struct StagedUv {
    binary: PathBuf,
    cache: PathBuf,
}

fn same_binary(expected: &Path, current: &Path) -> ProviderResult<bool> {
    let expected_len = std::fs::metadata(expected)
        .map_err(|_| {
            ManagedPythonFailure::new(
                ManagedPythonFailureKind::Inconclusive,
                "Extracted uv binary could not be inspected",
            )
        })?
        .len();
    let current_len = std::fs::metadata(current)
        .map_err(|_| {
            ManagedPythonFailure::new(
                ManagedPythonFailureKind::Inconclusive,
                "Private uv binary could not be inspected",
            )
        })?
        .len();
    if expected_len == 0 || expected_len > MAX_BINARY_BYTES || current_len != expected_len {
        return Ok(false);
    }
    let expected = std::fs::read(expected).map_err(|_| {
        ManagedPythonFailure::new(
            ManagedPythonFailureKind::Inconclusive,
            "Extracted uv binary could not be read",
        )
    })?;
    let current = std::fs::read(current).map_err(|_| {
        ManagedPythonFailure::new(
            ManagedPythonFailureKind::Inconclusive,
            "Private uv binary could not be read",
        )
    })?;
    Ok(Sha256::digest(expected) == Sha256::digest(current))
}

async fn download_archive(pin: UvPin) -> ProviderResult<Vec<u8>> {
    let client = reqwest::Client::builder()
        .timeout(BOOTSTRAP_TIMEOUT)
        .redirect(reqwest::redirect::Policy::limited(5))
        .build()
        .map_err(|_| {
            ManagedPythonFailure::new(
                ManagedPythonFailureKind::Inconclusive,
                "Private uv downloader could not start",
            )
        })?;
    let response = client.get(pin.url()).send().await.map_err(|_| {
        ManagedPythonFailure::new(
            ManagedPythonFailureKind::Inconclusive,
            "Pinned uv archive download was inconclusive",
        )
    })?;
    if !response.status().is_success() {
        return Err(ManagedPythonFailure::new(
            ManagedPythonFailureKind::Inconclusive,
            "Pinned uv archive was unavailable",
        ));
    }
    let mut bytes = Vec::new();
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|_| {
            ManagedPythonFailure::new(
                ManagedPythonFailureKind::Inconclusive,
                "Pinned uv archive download was interrupted",
            )
        })?;
        if bytes.len().saturating_add(chunk.len()) > MAX_ARCHIVE_BYTES {
            return Err(ManagedPythonFailure::new(
                ManagedPythonFailureKind::ArtifactIntegrity,
                "Pinned uv archive exceeded its size bound",
            ));
        }
        bytes.extend_from_slice(&chunk);
    }
    if !pin.verify_archive(&bytes) {
        return Err(ManagedPythonFailure::new(
            ManagedPythonFailureKind::ArtifactIntegrity,
            "Pinned uv archive failed SHA-256 verification",
        ));
    }
    Ok(bytes)
}

fn extract_binary(pin: UvPin, archive: &[u8], output: &Path) -> ProviderResult<()> {
    let expected = format!(
        "{}/{}",
        pin.archive
            .trim_end_matches(".tar.gz")
            .trim_end_matches(".zip"),
        pin.binary
    );
    let mut found = false;
    if pin.archive.ends_with(".zip") {
        let mut zip = zip::ZipArchive::new(Cursor::new(archive)).map_err(|_| {
            ManagedPythonFailure::new(
                ManagedPythonFailureKind::ArtifactIntegrity,
                "Pinned uv ZIP archive was invalid",
            )
        })?;
        for index in 0..zip.len() {
            let mut entry = zip.by_index(index).map_err(|_| {
                ManagedPythonFailure::new(
                    ManagedPythonFailureKind::ArtifactIntegrity,
                    "Pinned uv ZIP entry was invalid",
                )
            })?;
            if entry.name() != expected && entry.name() != pin.binary {
                continue;
            }
            if found || !entry.is_file() {
                return Err(ManagedPythonFailure::new(
                    ManagedPythonFailureKind::ArtifactIntegrity,
                    "Pinned uv archive had an invalid binary entry",
                ));
            }
            write_bounded_binary(&mut entry, output)?;
            found = true;
        }
    } else {
        let decoder = flate2::read::GzDecoder::new(archive);
        let mut tar = tar::Archive::new(decoder);
        for entry in tar.entries().map_err(|_| {
            ManagedPythonFailure::new(
                ManagedPythonFailureKind::ArtifactIntegrity,
                "Pinned uv TAR archive was invalid",
            )
        })? {
            let mut entry = entry.map_err(|_| {
                ManagedPythonFailure::new(
                    ManagedPythonFailureKind::ArtifactIntegrity,
                    "Pinned uv TAR entry was invalid",
                )
            })?;
            let path = entry.path().map_err(|_| {
                ManagedPythonFailure::new(
                    ManagedPythonFailureKind::ArtifactIntegrity,
                    "Pinned uv TAR path was invalid",
                )
            })?;
            if path != Path::new(&expected) && path != Path::new(pin.binary) {
                continue;
            }
            if found || !entry.header().entry_type().is_file() {
                return Err(ManagedPythonFailure::new(
                    ManagedPythonFailureKind::ArtifactIntegrity,
                    "Pinned uv archive had an invalid binary entry",
                ));
            }
            write_bounded_binary(&mut entry, output)?;
            found = true;
        }
    }
    if !found {
        return Err(ManagedPythonFailure::new(
            ManagedPythonFailureKind::ArtifactIntegrity,
            "Pinned uv binary was missing from its archive",
        ));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(output, std::fs::Permissions::from_mode(0o700)).map_err(|_| {
            ManagedPythonFailure::new(
                ManagedPythonFailureKind::Inconclusive,
                "Pinned uv binary permissions could not be set",
            )
        })?;
    }
    Ok(())
}

fn write_bounded_binary(input: &mut impl Read, output: &Path) -> ProviderResult<()> {
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output)
        .map_err(|_| {
            ManagedPythonFailure::new(
                ManagedPythonFailureKind::Inconclusive,
                "Pinned uv binary could not be staged",
            )
        })?;
    let count = std::io::copy(&mut input.take(MAX_BINARY_BYTES + 1), &mut file).map_err(|_| {
        ManagedPythonFailure::new(
            ManagedPythonFailureKind::ArtifactIntegrity,
            "Pinned uv binary could not be read",
        )
    })?;
    file.flush().map_err(|_| {
        ManagedPythonFailure::new(
            ManagedPythonFailureKind::Inconclusive,
            "Pinned uv binary could not be flushed",
        )
    })?;
    if count == 0 || count > MAX_BINARY_BYTES {
        return Err(ManagedPythonFailure::new(
            ManagedPythonFailureKind::ArtifactIntegrity,
            "Pinned uv binary exceeded its size bound",
        ));
    }
    Ok(())
}

async fn run_bounded(
    root: &Path,
    cleanup: &TorchCleanupTasks,
    mut command: Command,
    deadline: Duration,
    max_output: usize,
) -> ProviderResult<Vec<u8>> {
    let temporary = private_directory(root, "tmp")?;
    let workspace = Arc::new(
        tempfile::Builder::new()
            .prefix("managed-command-")
            .tempdir_in(temporary)
            .map_err(|_| {
                ManagedPythonFailure::new(
                    ManagedPythonFailureKind::Inconclusive,
                    "Private provider command workspace could not be created",
                )
            })?,
    );
    let output_path = workspace.path().join("stdout");
    let stdout = std::fs::File::create(&output_path).map_err(|_| {
        ManagedPythonFailure::new(
            ManagedPythonFailureKind::Inconclusive,
            "Private provider command output could not be created",
        )
    })?;
    command.stdout(Stdio::from(stdout)).stderr(Stdio::null());
    let custody = cleanup.new_child_slot().map_err(|_| {
        ManagedPythonFailure::new(
            ManagedPythonFailureKind::Inconclusive,
            "Managed provider process admission is closed",
        )
    })?;
    let mut child = pumas_library::platform::managed_child::ManagedChild::spawn(
        command.as_std_mut(),
        custody.clone(),
    )
    .map_err(|_| {
        ManagedPythonFailure::new(
            ManagedPythonFailureKind::Inconclusive,
            "Managed provider process could not start",
        )
    })?;
    child.attach_cleanup_lease(workspace.clone());
    let until = tokio::time::Instant::now() + deadline;
    let result = loop {
        match std::fs::metadata(&output_path) {
            Ok(metadata) if metadata.len() > max_output as u64 => {
                break Err(ManagedPythonFailure::new(
                    ManagedPythonFailureKind::Inconclusive,
                    "Managed provider output exceeded its bound",
                ));
            }
            Ok(_) => {}
            Err(_) => {
                break Err(ManagedPythonFailure::new(
                    ManagedPythonFailureKind::Inconclusive,
                    "Managed provider output could not be inspected",
                ));
            }
        }
        match child.observe_exit() {
            Ok(Some(status)) => break Ok(status),
            Ok(None) => {}
            Err(_) => {
                break Err(ManagedPythonFailure::new(
                    ManagedPythonFailureKind::Inconclusive,
                    "Managed provider process could not be observed",
                ));
            }
        }
        if tokio::time::Instant::now() >= until {
            break Err(ManagedPythonFailure::new(
                ManagedPythonFailureKind::Inconclusive,
                "Managed provider process timed out",
            ));
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    };
    drop(child);
    cleanup.drain_child_slot(&custody).await.map_err(|_| {
        ManagedPythonFailure::new(
            ManagedPythonFailureKind::Inconclusive,
            "Managed provider process cleanup is pending",
        )
    })?;
    let status = result?;
    if !status.success() {
        return Err(ManagedPythonFailure::new(
            ManagedPythonFailureKind::Inconclusive,
            "Managed provider process did not complete",
        ));
    }
    let output = tokio::fs::read(&output_path).await.map_err(|_| {
        ManagedPythonFailure::new(
            ManagedPythonFailureKind::Inconclusive,
            "Managed provider output could not be read",
        )
    })?;
    if output.len() > max_output {
        return Err(ManagedPythonFailure::new(
            ManagedPythonFailureKind::Inconclusive,
            "Managed provider output exceeded its bound",
        ));
    }
    Ok(output)
}

fn stable_minor(value: &str) -> Option<u32> {
    let (major, minor) = value.split_once('.')?;
    (major == "3"
        && !minor.is_empty()
        && minor.bytes().all(|b| b.is_ascii_digit())
        && !minor.starts_with('0'))
    .then(|| minor.parse::<u32>().ok())
    .flatten()
    .filter(|minor| *minor >= MIN_CPYTHON_MINOR)
}

fn stable_version(value: &str) -> Option<(u32, u32, u32)> {
    let mut parts = value.split('.');
    let (major, minor, patch) = (parts.next()?, parts.next()?, parts.next()?);
    if parts.next().is_some()
        || [major, minor, patch]
            .iter()
            .any(|part| part.is_empty() || !part.bytes().all(|b| b.is_ascii_digit()))
    {
        return None;
    }
    Some((
        major.parse().ok()?,
        minor.parse().ok()?,
        patch.parse().ok()?,
    ))
}

#[derive(Deserialize)]
struct CatalogVersionParts {
    major: u32,
    minor: u32,
    patch: u32,
}

#[derive(Deserialize)]
struct CatalogRecord {
    key: String,
    version: String,
    version_parts: CatalogVersionParts,
    url: String,
    os: String,
    variant: String,
    implementation: String,
    arch: String,
    libc: Option<String>,
}

fn parse_catalog(
    bytes: &[u8],
    target: NativeTarget,
) -> ProviderResult<Vec<ManagedPythonCandidate>> {
    if bytes.len() > MAX_CATALOG_BYTES {
        return Err(ManagedPythonFailure::new(
            ManagedPythonFailureKind::Inconclusive,
            "Managed Python catalog exceeded its bound",
        ));
    }
    let records: Vec<CatalogRecord> = serde_json::from_slice(bytes).map_err(|_| {
        ManagedPythonFailure::new(
            ManagedPythonFailureKind::Inconclusive,
            "Managed Python catalog was malformed",
        )
    })?;
    if records.is_empty() || records.len() > MAX_CATALOG_RECORDS {
        return Err(ManagedPythonFailure::new(
            ManagedPythonFailureKind::Inconclusive,
            "Managed Python catalog was empty or incomplete",
        ));
    }
    let pin = target.pin();
    let mut by_minor = BTreeMap::<u32, ManagedPythonCandidate>::new();
    for record in records {
        if record.implementation != "cpython"
            || record.variant != "default"
            || record.os != pin.os
            || record.arch != pin.arch
            || record.libc.as_deref() != pin.libc
        {
            continue;
        }
        let Some((major, minor, patch)) = stable_version(&record.version) else {
            continue;
        };
        if major != 3 || minor < MIN_CPYTHON_MINOR {
            continue;
        }
        if (major, minor, patch)
            != (
                record.version_parts.major,
                record.version_parts.minor,
                record.version_parts.patch,
            )
            || record.key.len() > 160
            || record.key.chars().any(char::is_control)
            || !valid_python_source(&record.url)
        {
            return Err(ManagedPythonFailure::new(
                ManagedPythonFailureKind::Inconclusive,
                "Managed Python catalog identity was invalid",
            ));
        }
        let candidate = ManagedPythonCandidate {
            minor: format!("3.{minor}"),
            version: record.version,
            catalog_key: record.key,
            source_url: record.url,
        };
        if by_minor.get(&minor).is_none_or(|current| {
            stable_version(&candidate.version) > stable_version(&current.version)
        }) {
            by_minor.insert(minor, candidate);
        }
    }
    if by_minor.is_empty() {
        return Err(ManagedPythonFailure::new(
            ManagedPythonFailureKind::Inconclusive,
            "Managed Python catalog had no stable native CPython",
        ));
    }
    Ok(by_minor
        .into_iter()
        .rev()
        .map(|(_, candidate)| candidate)
        .collect())
}

fn valid_python_source(value: &str) -> bool {
    let Ok(url) = reqwest::Url::parse(value) else {
        return false;
    };
    url.scheme() == "https"
        && url.host_str() == Some("releases.astral.sh")
        && url.port().is_none()
        && url.username().is_empty()
        && url.password().is_none()
        && url.query().is_none()
        && url.fragment().is_none()
        && url
            .path()
            .starts_with("/github/python-build-standalone/releases/download/")
}

const PYTHON_IDENTITY_PROBE: &str = "import json,platform,struct,sys; print(json.dumps({'path':sys.executable,'version':platform.python_version(),'platform':sys.platform,'machine':platform.machine(),'implementation':sys.implementation.name,'releaselevel':sys.version_info.releaselevel,'bits':struct.calcsize('P')*8}))";

#[derive(Deserialize)]
struct ObservedPython {
    path: String,
    version: String,
    platform: String,
    machine: String,
    implementation: String,
    releaselevel: String,
    bits: u8,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_shipped_targets_have_distinct_official_uv_pins() {
        let pins = [
            NativeTarget::LinuxX8664.pin(),
            NativeTarget::WindowsX8664.pin(),
            NativeTarget::MacosArm64.pin(),
        ];
        for pin in pins {
            assert!(pin.valid());
            assert!(!pin.verify_archive(b"tampered"));
        }
        assert_ne!(pins[0].sha256, pins[1].sha256);
        assert_ne!(pins[1].sha256, pins[2].sha256);
        assert!(pins[0].url().contains("uv-x86_64-unknown-linux-gnu.tar.gz"));
        assert!(pins[1].url().contains("uv-x86_64-pc-windows-msvc.zip"));
        assert!(pins[2].url().contains("uv-aarch64-apple-darwin.tar.gz"));
        let mut wrong_archive = pins[0];
        wrong_archive.archive = "uv-unapproved-linux.tar.gz";
        assert!(!wrong_archive.valid());
        let mut wrong_hash = pins[1];
        wrong_hash.sha256 = pins[0].sha256;
        assert!(!wrong_hash.valid());
    }

    #[test]
    fn uv_bootstrap_and_cache_paths_are_scoped_to_exact_provider_pin() {
        let current = NativeTarget::LinuxX8664.pin();
        let mut changed_hash = current;
        changed_hash.sha256 = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
        assert_ne!(
            uv_bootstrap_directory_name(current),
            uv_bootstrap_directory_name(changed_hash)
        );
        assert_ne!(
            uv_cache_directory_name(current),
            uv_cache_directory_name(changed_hash)
        );
        assert!(uv_bootstrap_directory_name(current).contains(UV_VERSION));
        assert!(uv_cache_directory_name(current).contains(UV_VERSION));
    }

    #[test]
    fn managed_identity_retains_selected_release_and_exact_provider_pin() {
        let source_url = "https://releases.astral.sh/github/python-build-standalone/releases/download/20260901/cpython.tar.gz";
        let installed = ManagedPythonInterpreter {
            minor: "3.14".into(),
            version: "3.14.2".into(),
            catalog_key: "cpython-3.14.2-linux-x86_64-gnu".into(),
            source_url: source_url.into(),
            executable: PathBuf::from("/private/python/3.14/bin/python3.14"),
        };
        let identity = ManagedPythonIdentity::from_install(installed, NativeTarget::LinuxX8664);
        assert_eq!(identity.python, "python3.14");
        assert_eq!(identity.version, "3.14.2");
        assert_eq!(identity.catalog_key, "cpython-3.14.2-linux-x86_64-gnu");
        assert_eq!(identity.source_url, source_url);
        assert_eq!(identity.target_triple, "x86_64-unknown-linux-gnu");
        assert_eq!(identity.uv_version, UV_VERSION);
        assert_eq!(
            identity.uv_archive_sha256,
            NativeTarget::LinuxX8664.pin().sha256
        );
        assert!(identity.executable.is_absolute());
    }

    #[test]
    fn selected_catalog_identity_owns_a_distinct_interpreter_depot() {
        let root = tempfile::tempdir().unwrap();
        let root = std::fs::canonicalize(root.path()).unwrap();
        let pin = NativeTarget::LinuxX8664.pin();
        let selected = ManagedPythonCandidate {
            minor: "3.14".into(),
            version: "3.14.2".into(),
            catalog_key: "cpython-3.14.2-linux-x86_64-gnu".into(),
            source_url: "https://github.com/astral-sh/python-build-standalone/releases/download/one/cpython.tar.gz".into(),
        };
        let first = private_install_depot(&root, pin, &selected).unwrap();
        assert_eq!(private_install_depot(&root, pin, &selected).unwrap(), first);
        let mut changed_key = selected.clone();
        changed_key.catalog_key.push_str("-repacked");
        let second = private_install_depot(&root, pin, &changed_key).unwrap();
        let mut changed_source = selected.clone();
        changed_source.source_url = changed_source.source_url.replace("/one/", "/two/");
        let third = private_install_depot(&root, pin, &changed_source).unwrap();
        assert_ne!(first, second);
        assert_ne!(first, third);
        assert_ne!(second, third);

        let executable = first.join(if cfg!(windows) {
            "python.exe"
        } else {
            "python"
        });
        std::fs::write(&executable, b"fixture").unwrap();
        assert_eq!(
            canonical_interpreter_in_depot(&executable, &first).unwrap(),
            executable
        );
        assert_eq!(
            canonical_interpreter_in_depot(&executable, &second)
                .unwrap_err()
                .kind,
            ManagedPythonFailureKind::ArtifactIntegrity
        );
    }

    #[test]
    fn stable_minor_rejects_prereleases_and_noncanonical_requests() {
        assert_eq!(stable_minor("3.10"), Some(10));
        assert_eq!(stable_minor("3.14"), Some(14));
        assert_eq!(stable_minor("3.100"), Some(100));
        for value in [
            "3.9",
            "3.0",
            "3.14.0",
            "3.14rc1",
            "3.014",
            "2.14",
            "3.-1",
            "3.14+free",
            "python3.14",
        ] {
            assert_eq!(stable_minor(value), None, "{value}");
        }
        assert_eq!(stable_version("3.14.2"), Some((3, 14, 2)));
        assert_eq!(stable_version("3.15.0a1"), None);
        assert!(valid_python_source("https://releases.astral.sh/github/python-build-standalone/releases/download/20260901/cpython.tar.gz"));
        assert!(!valid_python_source("https://evil.example/github/python-build-standalone/releases/download/20260901/cpython.tar.gz"));
        assert!(!valid_python_source("https://github.com/astral-sh/python-build-standalone/releases/download/20260901/cpython.tar.gz"));
        assert!(!valid_python_source("https://releases.astral.sh/github/other-project/releases/download/20260901/cpython.tar.gz"));
        assert!(!valid_python_source("https://releases.astral.sh/github/python-build-standalone/releases/download/20260901/cpython.tar.gz?mirror=1"));
    }

    #[tokio::test]
    async fn ensure_minor_rejects_below_runtime_floor_before_bootstrap() {
        if NativeTarget::current().is_err() {
            return;
        }
        let root = tempfile::tempdir().unwrap();
        let cleanup = Arc::new(TorchCleanupTasks::default());
        let provider = ManagedPythonProvider::new(root.path(), cleanup).unwrap();
        let failure = provider.ensure_minor("3.9").await.unwrap_err();
        assert_eq!(failure.kind, ManagedPythonFailureKind::InvalidMinor);
        assert!(!root
            .path()
            .join(uv_bootstrap_directory_name(
                NativeTarget::current().unwrap().pin()
            ))
            .exists());
    }

    #[cfg(target_os = "linux")]
    async fn wait_for_process_group_marker(path: &Path) -> i32 {
        tokio::time::timeout(Duration::from_secs(3), async {
            loop {
                if let Ok(marker) = std::fs::read_to_string(path) {
                    if let Ok(pid) = marker.trim().parse::<i32>() {
                        return pid;
                    }
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("provider command never started")
    }

    #[cfg(target_os = "linux")]
    fn sleeping_tree_command(marker: &Path) -> Command {
        let mut command = Command::new("sh");
        command
            .args(["-c", "echo $$ > \"$1\"; sleep 30 & wait", "sh"])
            .arg(marker);
        command
    }

    #[cfg(target_os = "linux")]
    #[tokio::test]
    async fn cancelled_provider_command_is_drained_by_manager_shutdown() {
        let root = tempfile::tempdir().unwrap();
        let cleanup = Arc::new(TorchCleanupTasks::default());
        let provider = Arc::new(ManagedPythonProvider::new(root.path(), cleanup.clone()).unwrap());
        let marker = root.path().join("cancelled-group");
        let command = sleeping_tree_command(&marker);
        let running = tokio::spawn(async move {
            provider
                .run_bounded(command, Duration::from_secs(30), 4096)
                .await
        });
        let group = wait_for_process_group_marker(&marker).await;
        running.abort();
        assert!(running.await.unwrap_err().is_cancelled());
        cleanup.close();
        cleanup.drain().await.unwrap();
        cleanup.drain_child_slots().await.unwrap();
        assert!(!pumas_library::platform::linux_group::group_has_live_members(group).unwrap());
    }

    #[cfg(target_os = "linux")]
    #[tokio::test]
    async fn timed_out_provider_command_drains_tree_and_closed_owner_rejects_spawn() {
        let root = tempfile::tempdir().unwrap();
        let cleanup = Arc::new(TorchCleanupTasks::default());
        let provider = ManagedPythonProvider::new(root.path(), cleanup.clone()).unwrap();
        let marker = root.path().join("timed-out-group");
        let failure = provider
            .run_bounded(
                sleeping_tree_command(&marker),
                Duration::from_millis(200),
                4096,
            )
            .await
            .unwrap_err();
        assert_eq!(failure.message, "Managed provider process timed out");
        let group = wait_for_process_group_marker(&marker).await;
        cleanup.close();
        cleanup.drain().await.unwrap();
        cleanup.drain_child_slots().await.unwrap();
        assert!(!pumas_library::platform::linux_group::group_has_live_members(group).unwrap());

        let rejected_marker = root.path().join("rejected-group");
        let failure = provider
            .run_bounded(
                sleeping_tree_command(&rejected_marker),
                Duration::from_secs(1),
                4096,
            )
            .await
            .unwrap_err();
        assert_eq!(
            failure.message,
            "Managed provider process admission is closed"
        );
        assert!(!rejected_marker.exists());
    }

    #[test]
    fn catalog_rejects_malformed_and_sorts_native_stable_minors() {
        let source = "https://releases.astral.sh/github/python-build-standalone/releases/download/20260901/cpython.tar.gz";
        let records = serde_json::json!([
            {"key":"cpython-3.9.25-linux-x86_64-gnu","version":"3.9.25","version_parts":{"major":3,"minor":9,"patch":25},"url":source,"os":"linux","variant":"default","implementation":"cpython","arch":"x86_64","libc":"gnu"},
            {"key":"cpython-3.10.19-linux-x86_64-gnu","version":"3.10.19","version_parts":{"major":3,"minor":10,"patch":19},"url":source,"os":"linux","variant":"default","implementation":"cpython","arch":"x86_64","libc":"gnu"},
            {"key":"cpython-3.13.7-linux-x86_64-gnu","version":"3.13.7","version_parts":{"major":3,"minor":13,"patch":7},"url":source,"os":"linux","variant":"default","implementation":"cpython","arch":"x86_64","libc":"gnu"},
            {"key":"cpython-3.15.0a1-linux-x86_64-gnu","version":"3.15.0a1","version_parts":{"major":3,"minor":15,"patch":0},"url":source,"os":"linux","variant":"default","implementation":"cpython","arch":"x86_64","libc":"gnu"},
            {"key":"cpython-3.14.2-linux-x86_64-gnu","version":"3.14.2","version_parts":{"major":3,"minor":14,"patch":2},"url":source,"os":"linux","variant":"default","implementation":"cpython","arch":"x86_64","libc":"gnu"},
            {"key":"cpython-3.14.1-linux-x86_64-gnu","version":"3.14.1","version_parts":{"major":3,"minor":14,"patch":1},"url":source,"os":"linux","variant":"default","implementation":"cpython","arch":"x86_64","libc":"gnu"}
        ]);
        let candidates = parse_catalog(
            &serde_json::to_vec(&records).unwrap(),
            NativeTarget::LinuxX8664,
        )
        .unwrap();
        assert_eq!(
            candidates
                .iter()
                .map(|item| item.minor.as_str())
                .collect::<Vec<_>>(),
            ["3.14", "3.13", "3.10"]
        );
        assert_eq!(candidates[0].version, "3.14.2");
        assert_eq!(
            parse_catalog(b"{}", NativeTarget::LinuxX8664)
                .unwrap_err()
                .kind,
            ManagedPythonFailureKind::Inconclusive
        );
        assert_eq!(
            parse_catalog(b"[]", NativeTarget::LinuxX8664)
                .unwrap_err()
                .kind,
            ManagedPythonFailureKind::Inconclusive
        );
    }

    #[test]
    fn catalog_keeps_native_windows_and_macos_records_with_none_libc() {
        let source = "https://releases.astral.sh/github/python-build-standalone/releases/download/20260901/cpython.tar.gz";
        let records = serde_json::json!([
            {"key":"cpython-3.14.2-windows-x86_64-none","version":"3.14.2","version_parts":{"major":3,"minor":14,"patch":2},"url":source,"os":"windows","variant":"default","implementation":"cpython","arch":"x86_64","libc":"none"},
            {"key":"cpython-3.13.7-macos-aarch64-none","version":"3.13.7","version_parts":{"major":3,"minor":13,"patch":7},"url":source,"os":"macos","variant":"default","implementation":"cpython","arch":"aarch64","libc":"none"},
            {"key":"cpython-3.12.11-macos-x86_64-none","version":"3.12.11","version_parts":{"major":3,"minor":12,"patch":11},"url":source,"os":"macos","variant":"default","implementation":"cpython","arch":"x86_64","libc":"none"},
            {"key":"cpython-3.11.12-linux-x86_64-gnu","version":"3.11.12","version_parts":{"major":3,"minor":11,"patch":12},"url":source,"os":"linux","variant":"default","implementation":"cpython","arch":"x86_64","libc":"gnu"}
        ]);
        let bytes = serde_json::to_vec(&records).unwrap();
        let windows = parse_catalog(&bytes, NativeTarget::WindowsX8664).unwrap();
        assert_eq!(windows.len(), 1);
        assert_eq!(windows[0].catalog_key, "cpython-3.14.2-windows-x86_64-none");
        let macos = parse_catalog(&bytes, NativeTarget::MacosArm64).unwrap();
        assert_eq!(macos.len(), 1);
        assert_eq!(macos[0].catalog_key, "cpython-3.13.7-macos-aarch64-none");
    }

    #[test]
    fn observed_identity_rejects_wrong_minor_platform_or_prerelease() {
        let mut observed = ObservedPython {
            path: "/managed/python".into(),
            version: "3.14.2".into(),
            platform: "linux".into(),
            machine: "x86_64".into(),
            implementation: "cpython".into(),
            releaselevel: "final".into(),
            bits: 64,
        };
        assert!(NativeTarget::LinuxX8664.accepts_observed(&observed, "3.14"));
        assert!(!NativeTarget::LinuxX8664.accepts_observed(&observed, "3.13"));
        observed.releaselevel = "candidate".into();
        assert!(!NativeTarget::LinuxX8664.accepts_observed(&observed, "3.14"));
        observed.releaselevel = "final".into();
        observed.machine = "arm64".into();
        assert!(!NativeTarget::LinuxX8664.accepts_observed(&observed, "3.14"));
        observed.platform = "darwin".into();
        assert!(NativeTarget::MacosArm64.accepts_observed(&observed, "3.14"));
    }

    #[cfg(unix)]
    #[test]
    fn private_provider_directory_rejects_symlink_escape() {
        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let cache_directory = uv_cache_directory_name(NativeTarget::LinuxX8664.pin());
        std::os::unix::fs::symlink(outside.path(), root.path().join(&cache_directory)).unwrap();
        let error = private_directory(root.path(), &cache_directory).unwrap_err();
        assert_eq!(error.kind, ManagedPythonFailureKind::ArtifactIntegrity);
    }
}
