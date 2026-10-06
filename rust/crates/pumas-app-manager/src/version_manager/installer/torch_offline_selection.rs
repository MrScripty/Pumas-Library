//! Dormant public offline solver. Selected evidence does not install or settle
//! another acquisition. Only a live accepted catalog can enter this boundary.
use super::*;
use std::collections::HashSet;

// This slice qualifies the already measured official Linux tooling only. There
// is no ambient PATH lookup, automatic tool download or production provisioner.
const QUALIFIED_UV_SHA256: &str =
    "abdc39eab8b4ad341dca91f3823a23a343fae94bdb22ebdd9e91694415206f2f";

fn selection_bytes(path: &Path, maximum: usize) -> Result<Vec<u8>> {
    let metadata = std::fs::symlink_metadata(path).map_err(PumasError::from)?;
    if !metadata.file_type().is_file() || metadata.len() > maximum as u64 {
        return Err(failed("Selection input missing, linked or oversized"));
    }
    let mut bytes = Vec::new();
    std::io::Read::take(
        File::open(path).map_err(PumasError::from)?,
        maximum as u64 + 1,
    )
    .read_to_end(&mut bytes)
    .map_err(PumasError::from)?;
    if bytes.len() > maximum || bytes.len() as u64 != metadata.len() {
        return Err(failed("Selection input changed or exceeded byte budget"));
    }
    Ok(bytes)
}

fn validate_selection_provenance(runtime: &Path, provenance: &[(PathBuf, Vec<u8>)]) -> Result<()> {
    for (path, expected) in provenance {
        validate_torch_owned_path(runtime, path)?;
        // Retained inputs were admitted under MAX_EVIDENCE. Use their exact
        // length here, including the bounded reader's one-byte growth sentinel;
        // a blocking comparison must never allocate the replacement's full size.
        if expected.len() > MAX_EVIDENCE || selection_bytes(path, expected.len())? != *expected {
            return Err(failed("Owned offline projection changed during selection"));
        }
    }
    Ok(())
}

fn selection_hash(path: &Path, maximum: u64) -> Result<String> {
    let metadata = std::fs::symlink_metadata(path).map_err(PumasError::from)?;
    if !metadata.file_type().is_file() || metadata.len() > maximum {
        return Err(failed("Selection hash input missing, linked or oversized"));
    }
    let mut digest = Sha256::new();
    let bytes = std::io::copy(
        &mut std::io::Read::take(File::open(path).map_err(PumasError::from)?, maximum + 1),
        &mut digest,
    )
    .map_err(PumasError::from)?;
    if bytes > maximum || bytes != metadata.len() {
        return Err(failed(
            "Selection hash input changed or exceeded byte budget",
        ));
    }
    Ok(format!("{:x}", digest.finalize()))
}

pub(super) struct QualifiedOfflineSolver {
    pub(super) path: PathBuf,
}

impl QualifiedOfflineSolver {
    fn validate(&self) -> Result<()> {
        if !cfg!(all(target_os = "linux", target_arch = "x86_64"))
            || !self.path.is_absolute()
            || !std::fs::symlink_metadata(&self.path)
                .map_err(PumasError::from)?
                .file_type()
                .is_file()
            || std::fs::metadata(&self.path)
                .map_err(PumasError::from)?
                .len()
                != 47_993_144
            || selection_hash(&self.path, 47_993_144)? != QUALIFIED_UV_SHA256
        {
            return Err(failed("Offline solver is not the qualified executable"));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SelectionFailure {
    Incomplete,
    Refused,
    SolverFailed,
}

// No Deserialize, cold opener, install permission or imported acceptance API.
pub(super) struct SelectedWheelPacket {
    pub(super) evidence: serde_json::Value,
    pub(super) catalog: Arc<CompleteCatalog>,
}

pub(super) struct SelectionRefusal {
    pub(super) kind: SelectionFailure,
    pub(super) _catalog: Arc<CompleteCatalog>,
}

pub(super) struct SelectionContext<'a> {
    pub(super) installer: &'a VersionInstaller,
    pub(super) consumer: &'a AcquisitionConsumer,
    pub(super) runtime: &'a Path,
    pub(super) python: &'a Path,
    pub(super) solver: QualifiedOfflineSolver,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Projection {
    schema: String,
    request_sha256: String,
    catalog_sha256: String,
    target_observation_sha256: String,
    python: String,
    platform: String,
    interpreter: String,
    roots: Vec<String>,
    constraints: Vec<String>,
    direct_roots: serde_json::Value,
    find_links: Vec<PathBuf>,
    candidates: serde_json::Value,
}

fn fence(catalog: &CompleteCatalog, runtime: &Path, python: &Path, wheels: &Path) -> Result<()> {
    catalog._grant.validate()?;
    validate_torch_target_evidence(runtime, &catalog._target, python)?;
    validate_torch_provenance(
        runtime,
        &[
            (
                runtime.join("catalog-request.json"),
                serde_json::to_vec(&catalog._request)
                    .map_err(|_| failed("Invalid retained catalog request"))?,
            ),
            (
                runtime.join("catalog-approved-target.json"),
                catalog._target.observation.as_bytes().to_vec(),
            ),
        ],
    )?;
    if read_bounded(&runtime.join("catalog-evidence.json"), MAX_EVIDENCE)? != catalog.evidence
        || catalog.receipt.payload != catalog.evidence
    {
        return Err(failed("Accepted catalog evidence changed"));
    }
    let expected: HashSet<_> = catalog
        .receipt
        .manifest
        .files()
        .iter()
        .map(|f| {
            Path::new(f.logical_path())
                .components()
                .next()
                .unwrap()
                .as_os_str()
                .to_owned()
        })
        .collect();
    let actual = std::fs::read_dir(wheels)
        .map_err(PumasError::from)?
        .map(|entry| entry.map(|e| e.file_name()))
        .collect::<std::io::Result<HashSet<_>>>()
        .map_err(PumasError::from)?;
    if actual != expected {
        return Err(failed("Granted catalog namespace changed"));
    }
    for file in catalog.receipt.manifest.files() {
        let path = wheels.join(file.logical_path());
        let parent = path
            .parent()
            .ok_or_else(|| failed("Missing catalog input parent"))?;
        let metadata = std::fs::symlink_metadata(&path).map_err(PumasError::from)?;
        if !std::fs::symlink_metadata(parent)
            .map_err(PumasError::from)?
            .file_type()
            .is_dir()
            || std::fs::read_dir(parent).map_err(PumasError::from)?.count() != 1
            || !metadata.file_type().is_file()
            || Some(metadata.len()) != file.expected_size()
            || !catalog.evidence["candidates"]
                .as_array()
                .is_some_and(|rows| {
                    rows.iter().any(|row| {
                        row["id"]
                            .as_str()
                            .zip(row["filename"].as_str())
                            .is_some_and(|(id, filename)| {
                                format!("{id}/{filename}") == file.logical_path()
                            })
                            && row["sha256"]
                                == selection_hash(&path, metadata.len()).unwrap_or_default()
                    })
                })
        {
            return Err(failed(
                "Granted catalog input changed after accepted inspection",
            ));
        }
    }
    Ok(())
}

impl SelectionContext<'_> {
    // The managed child itself retains the whole catalog grant and pending
    // runtime through cleanup if timeout/cancellation/abandonment drops waiting.
    async fn child(
        &self,
        mut command: Command,
        catalog: Arc<CompleteCatalog>,
        directory: &Path,
    ) -> Result<std::process::ExitStatus> {
        self.installer.check_cancelled()?;
        command
            .env_clear()
            .env("PATH", "/usr/bin:/bin")
            .env("LANG", "C.UTF-8")
            .env("HOME", directory.join("home"))
            .env("XDG_CONFIG_HOME", directory.join("home"))
            .env("XDG_CACHE_HOME", directory.join("cache"))
            .env("UV_CACHE_DIR", directory.join("cache"))
            .env("TMPDIR", directory.join("tmp"))
            .env("TMP", directory.join("tmp"))
            .env("TEMP", directory.join("tmp"))
            .current_dir(directory)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        let custody = self.installer.torch_cleanup.new_child_slot()?;
        let mut child = pumas_library::platform::managed_child::ManagedChild::spawn(
            command.as_std_mut(),
            custody.clone(),
        )
        .map_err(|_| failed("Offline selection child could not start"))?;
        child.attach_cleanup_lease(catalog);
        loop {
            let observed = child.observe_exit();
            let cancelled = self.installer.cancel_flag.load(Ordering::SeqCst);
            if cancelled || !matches!(&observed, Ok(None)) {
                drop(child);
                self.installer
                    .torch_cleanup
                    .drain_child_slot(&custody)
                    .await
                    .map_err(|_| failed("Offline selection child cleanup pending"))?;
                self.installer.check_cancelled()?;
                return observed
                    .map_err(PumasError::from)?
                    .ok_or_else(|| failed("Missing offline selection child status"));
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    }

    async fn effect<T: Send + 'static>(
        &self,
        catalog: Arc<CompleteCatalog>,
        name: &'static str,
        work: impl FnOnce(&CompleteCatalog) -> Result<T> + Send + 'static,
    ) -> Result<T> {
        self.consumer
            .run_blocking(name, move || Ok(work(&catalog)))
            .await?
    }

    fn checker(&self, directory: &Path, wheels: &Path, checking: bool) -> Command {
        let mut command = Command::new(self.python);
        command
            .arg("-I")
            .arg(self.runtime.join("offline_wheel_selection.py"))
            .arg("--request")
            .arg(self.runtime.join("catalog-request.json"))
            .arg("--observation")
            .arg(self.runtime.join("catalog-approved-target.json"))
            .arg("--catalog")
            .arg(self.runtime.join("catalog-evidence.json"))
            .arg("--wheels")
            .arg(wheels)
            .arg("--directory")
            .arg(directory);
        if checking {
            command.arg("--check");
        }
        command
    }

    async fn select_inner(
        &self,
        catalog: Arc<CompleteCatalog>,
    ) -> std::result::Result<serde_json::Value, SelectionFailure> {
        if self.consumer.owner() != CATALOG_OWNER {
            return Err(SelectionFailure::Refused);
        }
        let directory = self.runtime.join("offline-selection");
        let wheels = self
            .installer
            .versions_dir()
            .join(&catalog._grant.workspace_identity().relative_target);
        let runtime = self.runtime.to_owned();
        let python = self.python.to_owned();
        let path = directory.clone();
        let solver = QualifiedOfflineSolver {
            path: self.solver.path.clone(),
        };
        let versions = self.installer.versions_dir();
        let acquired = wheels.clone();
        self.effect(
            catalog.clone(),
            "admit offline catalog selection",
            move |c| {
                fence(c, &runtime, &python, &acquired)?;
                if !c._grant.binding().matches_root(&versions)? {
                    return Err(failed("Selected catalog belongs to another owner root"));
                }
                solver.validate()?;
                std::fs::create_dir(&path).map_err(PumasError::from)?;
                for leaf in ["home", "cache", "tmp"] {
                    std::fs::create_dir(path.join(leaf)).map_err(PumasError::from)?;
                }
                Ok(())
            },
        )
        .await
        .map_err(|_| SelectionFailure::Refused)?;
        let status = self
            .child(
                self.checker(&directory, &wheels, false),
                catalog.clone(),
                &directory,
            )
            .await
            .map_err(|_| SelectionFailure::Incomplete)?;
        if !status.success() {
            return Err(if status.code() == Some(20) {
                SelectionFailure::Incomplete
            } else {
                SelectionFailure::Refused
            });
        }
        let runtime = self.runtime.to_owned();
        let python = self.python.to_owned();
        let path = directory.clone();
        let solver = QualifiedOfflineSolver {
            path: self.solver.path.clone(),
        };
        let root = wheels.clone();
        let projection_root = root.clone();
        let (projection, retained) = self
            .effect(catalog.clone(), "fence offline solver inputs", move |c| {
                fence(c, &runtime, &python, &projection_root)?;
                solver.validate()?;
                let value = read_bounded(&path.join("projection.json"), MAX_EVIDENCE)?;
                let p: Projection = serde_json::from_value(value.clone())
                    .map_err(|_| failed("Invalid offline projection"))?;
                let observation: serde_json::Value = serde_json::from_str(&c._target.observation)
                    .map_err(|_| failed("Invalid target"))?;
                if p.schema != "pumas.offline-wheel-projection.v1"
                    || p.request_sha256 != target_observation_digest(&c._request)?
                    || p.catalog_sha256 != target_observation_digest(&c.evidence)?
                    || p.target_observation_sha256 != c._target.sha256
                    || Path::new(&p.interpreter) != python
                    || p.python != observation["target"]["python"]
                    || p.find_links.len() > 128
                    || p.find_links.iter().any(|link| {
                        !c.evidence["candidates"].as_array().is_some_and(|rows| {
                            rows.iter().any(|row| {
                                projection_root.join(row["id"].as_str().unwrap_or("")) == *link
                            })
                        })
                    })
                    || !matches!(p.platform.as_str(), "x86_64-pc-windows-msvc")
                        && !p.platform.starts_with("x86_64-manylinux_2_")
                {
                    return Err(failed(
                        "Offline projection differs from accepted catalog target",
                    ));
                }
                // The trusted checker owns semantic validation; retain its exact
                // inputs for the final fence after the public solver and checker.
                let mut retained = Vec::new();
                for leaf in ["projection.json", "roots.in", "constraints.in"] {
                    let file = path.join(leaf);
                    let m = std::fs::symlink_metadata(&file).map_err(PumasError::from)?;
                    if !m.file_type().is_file() || m.len() > MAX_EVIDENCE as u64 {
                        return Err(failed("Invalid bounded solver input"));
                    }
                    retained.push((file.clone(), selection_bytes(&file, MAX_EVIDENCE)?));
                }
                Ok((p, retained))
            })
            .await
            .map_err(|_| SelectionFailure::Refused)?;
        let mut command = Command::new(&self.solver.path);
        command
            .args([
                "--no-config",
                "--no-cache",
                "--offline",
                "--no-python-downloads",
                "--no-managed-python",
                "--color",
                "never",
                "pip",
                "compile",
            ])
            .arg(directory.join("roots.in"))
            .arg("--constraints")
            .arg(directory.join("constraints.in"))
            .args([
                "--no-index",
                "--no-build",
                "--no-sources",
                "--keyring-provider",
                "disabled",
                "--python",
            ])
            .arg(self.python)
            .arg("--python-version")
            .arg(&projection.python)
            .arg("--python-platform")
            .arg(&projection.platform)
            .args([
                "--format",
                "pylock.toml",
                "--generate-hashes",
                "--no-header",
                "--no-annotate",
                "-o",
            ])
            .arg(directory.join("pylock.toml"));
        for link in &projection.find_links {
            command.arg("--find-links").arg(link);
        }
        // These fields are deliberately retained only as trusted checker data;
        // they cannot become options or additional solver sources.
        let _ = (
            &projection.roots,
            &projection.constraints,
            &projection.direct_roots,
            &projection.candidates,
        );
        let status = self
            .child(command, catalog.clone(), &directory)
            .await
            .map_err(|_| SelectionFailure::SolverFailed)?;
        if !status.success() {
            return Err(SelectionFailure::SolverFailed);
        }
        let status = self
            .child(
                self.checker(&directory, &wheels, true),
                catalog.clone(),
                &directory,
            )
            .await
            .map_err(|_| SelectionFailure::Incomplete)?;
        if !status.success() {
            return Err(if status.code() == Some(20) {
                SelectionFailure::Incomplete
            } else {
                SelectionFailure::Refused
            });
        }
        let runtime = self.runtime.to_owned();
        let python = self.python.to_owned();
        let solver = QualifiedOfflineSolver {
            path: self.solver.path.clone(),
        };
        self.effect(
            catalog,
            "validate final selected packet and executable",
            move |c| {
                fence(c, &runtime, &python, &root)?;
                solver.validate()?;
                validate_selection_provenance(&runtime, &retained)?;
                let mut packet = read_bounded(&directory.join("selected.json"), MAX_EVIDENCE)?;
                let selected = packet["selected"]
                    .as_array()
                    .ok_or_else(|| failed("Missing selected files"))?;
                if packet["schema"] != "pumas.selected-wheel-packet.v1"
                    || packet["request_sha256"] != projection.request_sha256
                    || packet["catalog_sha256"] != projection.catalog_sha256
                    || packet["target_observation_sha256"] != c._target.sha256
                    || packet["projection_sha256"]
                        != target_observation_digest(&read_bounded(
                            &directory.join("projection.json"),
                            MAX_EVIDENCE,
                        )?)?
                    || packet["lock_sha256"]
                        != selection_hash(&directory.join("pylock.toml"), MAX_EVIDENCE as u64)?
                    || selected.is_empty()
                    || selected.len() > 128
                {
                    return Err(failed("Selected proof binding differs"));
                }
                let candidates = c.evidence["candidates"]
                    .as_array()
                    .ok_or_else(|| failed("Missing catalog candidates"))?;
                let mut ids = HashSet::new();
                for item in selected {
                    let mut identity = item.clone();
                    identity
                        .as_object_mut()
                        .ok_or_else(|| failed("Invalid selected identity"))?
                        .remove("local");
                    if !ids.insert(
                        item["id"]
                            .as_str()
                            .ok_or_else(|| failed("Invalid selected id"))?
                            .to_owned(),
                    ) || !candidates.contains(&identity)
                        || item["local"]
                            != root
                                .join(item["id"].as_str().unwrap())
                                .join(item["filename"].as_str().unwrap())
                                .to_string_lossy()
                                .as_ref()
                        || selection_hash(Path::new(
                            item["local"]
                                .as_str()
                                .ok_or_else(|| failed("Missing selected local input"))?,
                        ), 64 * 1024 * 1024)? != item["sha256"]
                    {
                        return Err(failed("Selected packet escaped acquired catalog identity"));
                    }
                }
                packet.as_object_mut().ok_or_else(|| failed("Invalid selected packet"))?.insert(
                    "qualified_solver".into(), serde_json::json!({"kind":"uv","version":"0.12.23","sha256":QUALIFIED_UV_SHA256}));
                Ok(packet)
            },
        )
        .await
        .map_err(|_| SelectionFailure::Refused)
    }
}

impl CompleteCatalog {
    pub(super) async fn select(
        self,
        context: SelectionContext<'_>,
    ) -> std::result::Result<SelectedWheelPacket, SelectionRefusal> {
        let catalog = Arc::new(self);
        let result = tokio::time::timeout(DEADLINE, context.select_inner(catalog.clone()))
            .await
            .unwrap_or(Err(SelectionFailure::Incomplete));
        match result {
            Ok(evidence) => Ok(SelectedWheelPacket { evidence, catalog }),
            Err(kind) => Err(SelectionRefusal {
                kind,
                _catalog: catalog,
            }),
        }
    }
}
