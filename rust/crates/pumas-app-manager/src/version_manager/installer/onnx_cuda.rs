//! The NVIDIA CUDA runtime and cuDNN libraries of an ONNX Runtime CUDA build.
//!
//! A `+cuda12` or `+cuda13` ONNX Runtime build loads NVIDIA's CUDA runtime,
//! cuBLAS, cuFFT, cuRAND and cuDNN when its CUDA execution provider
//! initializes. Pumas installs them with that ONNX Runtime version, from the
//! wheels NVIDIA publishes on PyPI, through the same acquisition as the
//! ONNX Runtime archive: every wheel is downloaded resumably and verified
//! against the SHA-256 its index published before anything is extracted.
//!
//! Nothing is pinned. Which libraries a build needs, and which releases are
//! compatible, come from the metadata Microsoft publishes for the matching
//! `onnxruntime-gpu` Python package: its `cuda` and `cudnn` extras list the
//! NVIDIA packages and version ranges that build was made for. Microsoft
//! publishes one CUDA major version of each release on PyPI and the other on
//! its own package feed (`onnxruntime-cuda-12` / `onnxruntime-cuda-13`), so
//! both are read, and only metadata whose CUDA runtime requirement names the
//! build's CUDA major version is used. Pumas then takes the newest release of
//! each package, and of the NVIDIA packages they require in turn, that
//! satisfies every range and has a wheel for the host. When no metadata for
//! the build exists, installation is refused with that reason instead of
//! guessing a set.
//!
//! The selection made for an install attempt is kept with that attempt, so a
//! retried or resumed install downloads the same files even after NVIDIA
//! publishes newer ones. Only the shared libraries (and license files) of each
//! wheel are kept, in `cuda/lib` of the installed version; the ONNX host's
//! library search path points there. Removing the version removes them.

use super::python_index::{
    best_wheel, normalize_name, platform_versions, requires_dist, IndexFile, PackageIndex,
    PyVersion, Requirement, WheelPlatform,
};
use super::{path_exists_sync, ExtractionBudget};
use pumas_library::acquisition::{
    ArtifactFile, ArtifactManifest, ArtifactRevisionEvidence, ArtifactSourceIdentity,
    FileVerificationRequirement, RevisionStrength, Sha256Evidence,
};
use pumas_library::config::InstallationConfig;
use pumas_library::onnx_runtime_distribution as distribution;
use pumas_library::{PumasError, Result};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs::File;
use std::path::Path;
use tracing::{debug, info};

/// PyPI's JSON simple index, where NVIDIA publishes its library wheels and
/// Microsoft publishes one CUDA major version of `onnxruntime-gpu`.
pub(crate) const PYPI_SIMPLE_INDEX: &str = "https://pypi.org/simple/";
/// Microsoft's package feed for the `onnxruntime-gpu` builds of one CUDA major
/// version (`{major}`), as the ONNX Runtime install documentation names it.
pub(crate) const ORT_CUDA_FEED_TEMPLATE: &str =
    "https://aiinfra.pkgs.visualstudio.com/PublicPackages/_packaging/onnxruntime-cuda-{major}/pypi/simple/";
const ORT_GPU_PROJECT: &str = "onnxruntime-gpu";
/// The `onnxruntime-gpu` extras that add the CUDA runtime and cuDNN.
const ORT_CUDA_EXTRAS: [&str; 2] = ["cuda", "cudnn"];
const CUDA_RUNTIME_PROJECT: &str = "nvidia-cuda-runtime";
/// Only NVIDIA library packages are followed; a requirement on anything else
/// is a Python dependency that no shared library needs.
const NVIDIA_PROJECT_PREFIX: &str = "nvidia-";
const MAX_RESOLUTION_ROUNDS: usize = 64;
const INDEX_REQUEST_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(120);
/// The selection an install attempt downloads, kept in its workspace.
pub(super) const SELECTION_FILE: &str = "cuda-wheels.json";
const SELECTION_SCHEMA: u32 = 1;
const RECORD_SCHEMA: u32 = 1;

fn refused(message: impl Into<String>) -> PumasError {
    PumasError::InstallationFailed {
        message: message.into(),
    }
}

/// Where CUDA library metadata and wheels are read from.
#[derive(Clone, Debug)]
pub(crate) struct CudaWheelSources {
    /// Indexes searched, in order, for the `onnxruntime-gpu` metadata of a
    /// release. `{major}` is replaced by the build's CUDA major version.
    pub(crate) requirement_indexes: Vec<String>,
    /// The index NVIDIA's library wheels are taken from.
    pub(crate) wheel_index: String,
    pub(crate) client: Option<reqwest::Client>,
}

impl CudaWheelSources {
    pub(crate) fn official() -> Self {
        Self {
            requirement_indexes: vec![
                PYPI_SIMPLE_INDEX.to_owned(),
                ORT_CUDA_FEED_TEMPLATE.to_owned(),
            ],
            wheel_index: PYPI_SIMPLE_INDEX.to_owned(),
            client: None,
        }
    }

    fn client(&self) -> Result<reqwest::Client> {
        if let Some(client) = &self.client {
            return Ok(client.clone());
        }
        reqwest::Client::builder()
            .connect_timeout(InstallationConfig::URL_FETCH_TIMEOUT)
            .timeout(INDEX_REQUEST_TIMEOUT)
            .user_agent("pumas-library")
            .build()
            .map_err(|error| PumasError::Network {
                message: "Failed to create the package index HTTP client".into(),
                cause: Some(error.to_string()),
            })
    }
}

/// The `onnxruntime-gpu` metadata a selection's requirements came from.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RequirementsSource {
    pub(super) index: String,
    pub(super) project: String,
    pub(super) version: String,
    pub(super) wheel: String,
    /// The requirements its `cuda` and `cudnn` extras add.
    pub(super) requires: Vec<String>,
}

/// One NVIDIA wheel chosen for an install, with its index SHA-256.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SelectedWheel {
    pub(super) project: String,
    pub(super) version: String,
    pub(super) filename: String,
    pub(super) url: String,
    pub(super) sha256: String,
    pub(super) size: Option<u64>,
}

/// The CUDA library wheels one ONNX Runtime CUDA install downloads.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct CudaWheelSelection {
    pub(super) schema_version: u32,
    pub(super) tag: String,
    pub(super) platform: String,
    pub(super) cuda_major: u32,
    pub(super) requirements: RequirementsSource,
    pub(super) wheels: Vec<SelectedWheel>,
}

impl CudaWheelSelection {
    /// Whether this selection was made for `tag` on `platform`.
    pub(super) fn serves(&self, tag: &str, platform: WheelPlatform, cuda_major: u32) -> bool {
        self.schema_version == SELECTION_SCHEMA
            && self.tag == tag
            && self.platform == platform.id()
            && self.cuda_major == cuda_major
            && !self.wheels.is_empty()
            && self.wheels.iter().all(|wheel| valid_sha256(&wheel.sha256))
    }
}

fn valid_sha256(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

/// The CUDA major version a set of `onnxruntime-gpu` extra requirements was
/// built for, read from its CUDA runtime requirement: the `-cuNN` suffix of
/// `nvidia-cuda-runtime-cuNN`, or the release `nvidia-cuda-runtime`'s range
/// starts from (`~=13.0`).
fn targeted_cuda_major(requirements: &[Requirement]) -> Option<u32> {
    requirements.iter().find_map(|requirement| {
        let suffix = requirement.name.strip_prefix(CUDA_RUNTIME_PROJECT)?;
        if let Some(major) = suffix.strip_prefix("-cu") {
            return major.parse().ok();
        }
        if !suffix.is_empty() {
            return None;
        }
        requirement
            .specifiers
            .iter()
            .find_map(|specifier| specifier.lower_bound())
            .and_then(|version| u32::try_from(version.major()).ok())
    })
}

fn is_cuda_runtime_project(name: &str) -> bool {
    name == CUDA_RUNTIME_PROJECT
        || name
            .strip_prefix(CUDA_RUNTIME_PROJECT)
            .and_then(|suffix| suffix.strip_prefix("-cu"))
            .is_some_and(|major| !major.is_empty() && major.bytes().all(|b| b.is_ascii_digit()))
}

struct Chosen {
    version: PyVersion,
    file: IndexFile,
    requires: Vec<Requirement>,
    extras: BTreeSet<String>,
}

#[derive(Default)]
struct Wanted {
    requirements: Vec<Requirement>,
    extras: BTreeSet<String>,
    required_by: BTreeSet<String>,
    /// The chosen NVIDIA packages among `required_by`, by project name.
    required_by_packages: BTreeSet<String>,
}

/// Resolve the CUDA library wheels for ONNX Runtime `release`'s CUDA
/// `cuda_major` build on `platform`. Every refusal names what is missing.
pub(super) async fn resolve(
    sources: &CudaWheelSources,
    tag: &str,
    release: &semver::Version,
    cuda_major: u32,
    platform: WheelPlatform,
) -> Result<CudaWheelSelection> {
    let client = sources.client()?;
    let release_version = PyVersion::parse(&release.to_string()).ok_or_else(|| {
        refused(format!(
            "ONNX Runtime {release} is not a Python package version"
        ))
    })?;
    let extras: BTreeSet<String> = ORT_CUDA_EXTRAS
        .iter()
        .map(|extra| (*extra).to_owned())
        .collect();

    // Find metadata of this release that was built for this CUDA major.
    let mut looked = Vec::new();
    let mut found = None;
    for template in &sources.requirement_indexes {
        let index = PackageIndex::new(
            &template.replace("{major}", &cuda_major.to_string()),
            client.clone(),
        )?;
        let Some(files) = index.files(ORT_GPU_PROJECT).await? else {
            looked.push(format!("{} has no {ORT_GPU_PROJECT}", index.display()));
            continue;
        };
        let Some(file) = best_wheel(&files, &release_version, platform) else {
            looked.push(format!(
                "{} has no {ORT_GPU_PROJECT} {release} wheel for {}",
                index.display(),
                platform.id()
            ));
            continue;
        };
        let metadata = index.core_metadata(file).await?;
        let added: Vec<Requirement> = requires_dist(&metadata)
            .map_err(|why| refused(format!("{} metadata: {why}", file.filename)))?
            .into_iter()
            .filter(|requirement| requirement.added_by_extras(platform, &extras))
            .collect();
        match targeted_cuda_major(&added) {
            Some(major) if major == cuda_major => {
                found = Some((index, file.clone(), added));
                break;
            }
            Some(major) => looked.push(format!(
                "{ORT_GPU_PROJECT} {release} on {} is the CUDA {major} build",
                index.display()
            )),
            None => looked.push(format!(
                "{ORT_GPU_PROJECT} {release} on {} names no CUDA runtime in its `cuda` extra",
                index.display()
            )),
        }
    }
    let Some((requirement_index, requirement_file, roots)) = found else {
        return Err(refused(format!(
            "ONNX Runtime {release} has no published CUDA {cuda_major} package metadata ({}). \
             Pumas reads which CUDA runtime and cuDNN libraries a build needs from that \
             metadata and does not guess them; install another build or release of ONNX \
             Runtime",
            looked.join("; ")
        )));
    };
    info!(
        "ONNX Runtime {release} CUDA {cuda_major} requirements from {} ({})",
        requirement_index.display(),
        requirement_file.filename
    );

    // Resolve the NVIDIA packages and the NVIDIA packages they require.
    let wheel_index = PackageIndex::new(&sources.wheel_index, client)?;
    let mut listings: HashMap<String, Vec<IndexFile>> = HashMap::new();
    let mut chosen: BTreeMap<String, Chosen> = BTreeMap::new();
    // Releases set aside because what they require cannot be satisfied: the
    // resolver then tries an older release of that package.
    let mut excluded: HashMap<String, Vec<PyVersion>> = HashMap::new();
    let mut settled = false;
    'rounds: for _ in 0..MAX_RESOLUTION_ROUNDS {
        let mut wanted: BTreeMap<String, Wanted> = BTreeMap::new();
        let mut want = |requirement: &Requirement, by: &str, by_package: Option<&str>| {
            if !requirement.name.starts_with(NVIDIA_PROJECT_PREFIX) {
                debug!(
                    "Not following {} (required by {by}): not an NVIDIA library",
                    requirement.text
                );
                return;
            }
            let entry = wanted.entry(requirement.name.clone()).or_default();
            entry.requirements.push(requirement.clone());
            entry.extras.extend(requirement.extras.iter().cloned());
            entry.required_by.insert(by.to_owned());
            if let Some(package) = by_package {
                entry.required_by_packages.insert(package.to_owned());
            }
        };
        let root_name = format!("{ORT_GPU_PROJECT} {release}");
        for requirement in &roots {
            want(requirement, &root_name, None);
        }
        for (name, package) in &chosen {
            for requirement in &package.requires {
                if requirement.applies(platform, &package.extras) {
                    want(
                        requirement,
                        &format!("{name} {}", package.version.as_str()),
                        Some(name),
                    );
                }
            }
        }
        let before = chosen.len();
        chosen.retain(|name, _| wanted.contains_key(name));
        let mut changed = chosen.len() != before;
        for (name, wanted) in wanted {
            if let Some(package) = chosen.get_mut(&name) {
                if wanted
                    .requirements
                    .iter()
                    .all(|requirement| requirement.allows(&package.version))
                {
                    if package.extras != wanted.extras {
                        package.extras = wanted.extras;
                        changed = true;
                    }
                    continue;
                }
            }
            if !listings.contains_key(&name) {
                let files = wheel_index.files(&name).await?.ok_or_else(|| {
                    refused(format!(
                        "{} has no project {name}, which {} requires",
                        wheel_index.display(),
                        wanted
                            .required_by
                            .iter()
                            .cloned()
                            .collect::<Vec<_>>()
                            .join(", ")
                    ))
                })?;
                listings.insert(name.clone(), files);
            }
            let files = &listings[&name];
            let ranges = || {
                wanted
                    .requirements
                    .iter()
                    .map(Requirement::constraint)
                    .collect::<Vec<_>>()
                    .join(", ")
            };
            let set_aside = excluded.get(&name);
            let version = platform_versions(files, platform)
                .into_iter()
                .find(|version| {
                    set_aside.is_none_or(|versions| !versions.contains(version))
                        && wanted
                            .requirements
                            .iter()
                            .all(|requirement| requirement.allows(version))
                });
            let Some(version) = version else {
                if !wanted.required_by_packages.is_empty() {
                    // Try older releases of the packages that asked for this.
                    for requirer in &wanted.required_by_packages {
                        if let Some(package) = chosen.remove(requirer) {
                            excluded
                                .entry(requirer.clone())
                                .or_default()
                                .push(package.version);
                        }
                    }
                    continue 'rounds;
                }
                return Err(refused(format!(
                    "No release of {name} on {} has a {} wheel that satisfies {} \
                         (required by {}), so the CUDA {cuda_major} libraries of ONNX Runtime \
                         {release} cannot be installed; install another build or release of \
                         ONNX Runtime",
                    wheel_index.display(),
                    platform.id(),
                    ranges(),
                    wanted
                        .required_by
                        .iter()
                        .cloned()
                        .collect::<Vec<_>>()
                        .join(", ")
                )));
            };
            let file = best_wheel(files, &version, platform)
                .cloned()
                .ok_or_else(|| {
                    refused(format!("{name} {} has no usable wheel", version.as_str()))
                })?;
            if !file.sha256.as_deref().is_some_and(valid_sha256) {
                return Err(refused(format!(
                    "{} publishes no SHA-256 for {}, so Pumas cannot verify or resume it",
                    wheel_index.display(),
                    file.filename
                )));
            }
            let metadata = wheel_index.core_metadata(&file).await?;
            let requires = requires_dist(&metadata)
                .map_err(|why| refused(format!("{} metadata: {why}", file.filename)))?;
            chosen.insert(
                name,
                Chosen {
                    version,
                    file,
                    requires,
                    extras: wanted.extras,
                },
            );
            changed = true;
        }
        if !changed {
            settled = true;
            break;
        }
    }
    if !settled {
        return Err(refused(format!(
            "The CUDA {cuda_major} library requirements of ONNX Runtime {release} did not settle \
             on one set of NVIDIA packages"
        )));
    }

    // The resolved CUDA runtime must be the build's CUDA major version.
    let runtime = chosen
        .iter()
        .find(|(name, _)| is_cuda_runtime_project(name))
        .ok_or_else(|| {
            refused(format!(
                "The CUDA {cuda_major} requirements of ONNX Runtime {release} resolved no CUDA \
                 runtime package"
            ))
        })?;
    if runtime.1.version.major() != u64::from(cuda_major) {
        return Err(refused(format!(
            "The CUDA {cuda_major} requirements of ONNX Runtime {release} resolved {} {}, which \
             is not a CUDA {cuda_major} runtime",
            runtime.0,
            runtime.1.version.as_str()
        )));
    }

    let mut wheels: Vec<SelectedWheel> = chosen
        .into_iter()
        .map(|(project, package)| SelectedWheel {
            project,
            version: package.version.as_str().to_owned(),
            filename: package.file.filename.clone(),
            url: package.file.url.to_string(),
            sha256: package.file.sha256.clone().unwrap_or_default(),
            size: package.file.size,
        })
        .collect();
    wheels.sort_by(|left, right| left.filename.cmp(&right.filename));
    Ok(CudaWheelSelection {
        schema_version: SELECTION_SCHEMA,
        tag: tag.to_owned(),
        platform: platform.id().to_owned(),
        cuda_major,
        requirements: RequirementsSource {
            index: requirement_index.display(),
            project: ORT_GPU_PROJECT.to_owned(),
            version: release.to_string(),
            wheel: requirement_file.filename,
            requires: roots
                .iter()
                .map(|requirement| requirement.text.clone())
                .collect(),
        },
        wheels,
    })
}

/// The selection kept with an install attempt, if one is there.
pub(super) fn read_selection(path: &Path) -> Result<Option<CudaWheelSelection>> {
    if !path_exists_sync(path)? {
        return Ok(None);
    }
    pumas_library::metadata::atomic_read_json(path)
}

/// Keep `selection` with its install attempt.
pub(super) fn write_selection(path: &Path, selection: &CudaWheelSelection) -> Result<()> {
    pumas_library::metadata::atomic_write_json(path, selection, false)
}

/// The acquisition manifest of an ONNX Runtime CUDA install: the release
/// archive first, then every wheel, each with the SHA-256 its publisher gave.
pub(super) fn composite_manifest(
    archive: &ArtifactManifest,
    selection: &CudaWheelSelection,
) -> Result<ArtifactManifest> {
    let invalid = |error: pumas_library::acquisition::ManifestValidationError| {
        refused(format!(
            "The ONNX Runtime CUDA download set is not a valid selection: {error}"
        ))
    };
    let archive_file = archive
        .files()
        .first()
        .ok_or_else(|| refused("The ONNX Runtime archive selection has no file"))?;
    let mut set = Sha256::new();
    set.update(archive.source().revision().value().as_bytes());
    set.update(b"\0");
    set.update(
        archive_file
            .expected_sha256()
            .map(|digest| digest.value())
            .unwrap_or_default()
            .as_bytes(),
    );
    for wheel in &selection.wheels {
        set.update(b"\n");
        set.update(wheel.filename.as_bytes());
        set.update(b"\0");
        set.update(wheel.sha256.as_bytes());
    }
    let source = ArtifactSourceIdentity::new(
        "onnx-runtime",
        archive.source().source_id(),
        ArtifactRevisionEvidence::new(
            "pumas.onnx_runtime.cuda_set.sha256",
            format!("{:x}", set.finalize()),
            RevisionStrength::Weak,
        )
        .map_err(invalid)?,
    )
    .map_err(invalid)?;
    let mut files = vec![archive_file.clone()];
    for wheel in &selection.wheels {
        files.push(
            ArtifactFile::new(
                wheel.filename.clone(),
                format!("python-index:{}:{}", wheel.project, wheel.filename),
                wheel.size,
                Some(
                    Sha256Evidence::new("python.package_index.sha256", wheel.sha256.clone())
                        .map_err(invalid)?,
                ),
                FileVerificationRequirement::Sha256,
            )
            .map_err(invalid)?,
        );
    }
    ArtifactManifest::new(source, files).map_err(invalid)
}

#[derive(serde::Serialize)]
struct InstalledLibrary {
    name: String,
    sha256: String,
}

#[derive(serde::Serialize)]
struct InstalledPackage {
    project: String,
    version: String,
    wheel: String,
    wheel_sha256: String,
    libraries: Vec<InstalledLibrary>,
}

/// `cuda/libraries.json`: what filled `cuda/lib`, and from where.
#[derive(serde::Serialize)]
struct CudaLibrariesRecord<'a> {
    schema_version: u32,
    tag: &'a str,
    platform: &'a str,
    cuda_major: u32,
    requirements: &'a RequirementsSource,
    packages: Vec<InstalledPackage>,
}

fn hash_file(path: &Path) -> Result<String> {
    let mut file = File::open(path).map_err(|error| PumasError::io_with_path(error, path))?;
    let mut digest = Sha256::new();
    std::io::copy(&mut file, &mut digest).map_err(|error| PumasError::io_with_path(error, path))?;
    Ok(format!("{:x}", digest.finalize()))
}

/// Copy one zip entry to `destination`, held to its declared size.
fn copy_entry(
    entry: &mut zip::read::ZipFile<'_, File>,
    destination: &Path,
    index: usize,
) -> Result<()> {
    let declared = entry.size();
    let mut output =
        File::create(destination).map_err(|error| PumasError::io_with_path(error, destination))?;
    let written = std::io::copy(
        &mut std::io::Read::take(&mut *entry, declared.saturating_add(1)),
        &mut output,
    )
    .map_err(|error| PumasError::io_with_path(error, destination))?;
    if written > declared {
        return Err(refused(format!(
            "Wheel entry {index} expands beyond its declared size of {declared} bytes"
        )));
    }
    output
        .sync_all()
        .map_err(|error| PumasError::io_with_path(error, destination))
}

fn is_link(entry: &zip::read::ZipFile<'_, File>) -> bool {
    entry
        .unix_mode()
        .is_some_and(|mode| mode & 0o170_000 == 0o120_000)
}

/// Extract the shared libraries and license files of `wheels` (opened in
/// selection order) into `stage/cuda`, and record them in
/// `stage/cuda/libraries.json`.
pub(super) fn extract_libraries(
    wheels: Vec<File>,
    selection: &CudaWheelSelection,
    stage: &Path,
    platform: WheelPlatform,
) -> Result<()> {
    if wheels.len() != selection.wheels.len() {
        return Err(refused(
            "The verified CUDA wheel set does not match its selection",
        ));
    }
    let library_dir = distribution::join_relative(stage, distribution::CUDA_LIBRARY_DIR);
    let licenses_dir = stage.join("cuda").join("licenses");
    std::fs::create_dir_all(&library_dir)
        .map_err(|error| PumasError::io_with_path(error, &library_dir))?;
    let mut owners: HashMap<String, String> = HashMap::new();
    let mut packages = Vec::with_capacity(wheels.len());
    for (file, wheel) in wheels.into_iter().zip(&selection.wheels) {
        let mut budget = ExtractionBudget::for_archive(&file, stage)?;
        let mut archive = zip::ZipArchive::new(file).map_err(|error| {
            refused(format!("{} is not a valid wheel: {error}", wheel.filename))
        })?;
        let mut libraries = Vec::new();
        for index in 0..archive.len() {
            let mut entry = archive.by_index(index).map_err(|error| {
                refused(format!(
                    "Could not read entry {index} of {}: {error}",
                    wheel.filename
                ))
            })?;
            if entry.is_dir() {
                continue;
            }
            let path = entry.enclosed_name().ok_or_else(|| {
                refused(format!(
                    "{} has entry {index} outside the wheel",
                    wheel.filename
                ))
            })?;
            let components: Vec<String> = path
                .components()
                .map(|component| component.as_os_str().to_string_lossy().into_owned())
                .collect();
            let (Some(first), Some(name)) = (components.first(), components.last()) else {
                continue;
            };
            let destination = if first == "nvidia" && platform.is_shared_library(name) {
                library_dir.join(name)
            } else if first.ends_with(".dist-info")
                && name.to_ascii_lowercase().starts_with("license")
            {
                let directory = licenses_dir.join(&wheel.project);
                std::fs::create_dir_all(&directory)
                    .map_err(|error| PumasError::io_with_path(error, &directory))?;
                directory.join(name)
            } else {
                continue;
            };
            if is_link(&entry) {
                return Err(refused(format!(
                    "{} ships {name} as a link; Pumas installs only regular library files",
                    wheel.filename
                )));
            }
            budget.admit(entry.size())?;
            let is_library = destination.parent() == Some(library_dir.as_path());
            let lookup = if cfg!(windows) || platform == WheelPlatform::WindowsX64 {
                name.to_ascii_lowercase()
            } else {
                name.clone()
            };
            if is_library && owners.contains_key(&lookup) {
                // Two packages may ship the same library; it must be the same file.
                let partial = library_dir.join(format!(".{name}.partial"));
                copy_entry(&mut entry, &partial, index)?;
                let same = hash_file(&partial)? == hash_file(&destination)?;
                std::fs::remove_file(&partial)
                    .map_err(|error| PumasError::io_with_path(error, &partial))?;
                if !same {
                    return Err(refused(format!(
                        "{} and {} ship different {name} libraries",
                        owners[&lookup], wheel.project
                    )));
                }
                continue;
            }
            if path_exists_sync(&destination)? {
                continue;
            }
            copy_entry(&mut entry, &destination, index)?;
            if is_library {
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    std::fs::set_permissions(&destination, std::fs::Permissions::from_mode(0o755))
                        .map_err(|error| PumasError::io_with_path(error, &destination))?;
                }
                owners.insert(lookup, wheel.project.clone());
                libraries.push(InstalledLibrary {
                    name: name.clone(),
                    sha256: hash_file(&destination)?,
                });
            }
        }
        libraries.sort_by(|left, right| left.name.cmp(&right.name));
        packages.push(InstalledPackage {
            project: wheel.project.clone(),
            version: wheel.version.clone(),
            wheel: wheel.filename.clone(),
            wheel_sha256: wheel.sha256.clone(),
            libraries,
        });
    }
    if owners.is_empty() {
        return Err(refused(format!(
            "The CUDA {} wheels of {} contain no {} shared libraries",
            selection.cuda_major,
            selection.tag,
            platform.id()
        )));
    }
    let record = CudaLibrariesRecord {
        schema_version: RECORD_SCHEMA,
        tag: &selection.tag,
        platform: &selection.platform,
        cuda_major: selection.cuda_major,
        requirements: &selection.requirements,
        packages,
    };
    let record_path = distribution::join_relative(stage, distribution::CUDA_LIBRARIES_RECORD);
    let bytes = serde_json::to_vec_pretty(&record)?;
    std::fs::write(&record_path, bytes)
        .map_err(|error| PumasError::io_with_path(error, &record_path))
}

/// The normalized project names of a selection, for messages.
pub(super) fn describe(selection: &CudaWheelSelection) -> String {
    selection
        .wheels
        .iter()
        .map(|wheel| format!("{} {}", normalize_name(&wheel.project), wheel.version))
        .collect::<Vec<_>>()
        .join(", ")
}
