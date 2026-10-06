//! Private CPU runtime validation/publication from a live installed packet.
//! Catalog settlement remains unchanged. No cold reconstruction or caller adoption.
use super::*;

#[derive(Clone)]
pub(in super::super::super) enum ProviderApproval {
    Managed {
        identity: crate::version_manager::managed_python::ManagedPythonIdentity,
        sha256: String,
    },
    #[cfg(test)]
    Fixture { path: PathBuf, sha256: String },
}

impl ProviderApproval {
    fn validate(&self, target: &AcceptedTorchTarget) -> Result<serde_json::Value> {
        let observation: serde_json::Value = serde_json::from_str(&target.observation)
            .map_err(|_| failed("Invalid retained provider target"))?;
        let (path, expected, proof) = match self {
            Self::Managed { identity, sha256 } => {
                if observation["target"]["python"] != identity.version {
                    return Err(failed(
                        "Managed provider version differs from approved consumer",
                    ));
                }
                (
                    &identity.executable,
                    sha256,
                    managed_python_identity_record(identity, sha256),
                )
            }
            #[cfg(test)]
            Self::Fixture { path, sha256 } => (
                path,
                sha256,
                serde_json::json!({"kind":"existing-local-fixture-provider", "path":path, "sha256":sha256}),
            ),
        };
        if !path.is_absolute() || expected.len() != 64 || torch_interpreter_hash(path)? != *expected
        {
            return Err(failed("Approved provider executable changed"));
        }
        Ok(proof)
    }
}

pub(in super::super::super) struct RuntimeLifecycleContext<'a> {
    pub(in super::super::super) selection: SelectionContext<'a>,
    pub(in super::super::super) provider: ProviderApproval,
    pub(in super::super::super) tag: &'a str,
    pub(in super::super::super) release: &'a GitHubRelease,
    pub(in super::super::super) adapter: &'a str,
    // Explicit approved fixture source bodies, never available in production.
    #[cfg(test)]
    pub(in super::super::super) fixture_sources: Vec<(String, Vec<u8>)>,
    #[cfg(test)]
    pub(in super::super::super) fault: PublicationFault,
    #[cfg(test)]
    pub(in super::super::super) publication_pause: Option<Arc<PublicationPause>>,
}

#[cfg(test)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub(in super::super::super) enum PublicationFault {
    None,
    BeforeRename,
    AfterRename,
    AfterMetadata,
    MovedMember,
    MovedRecord,
    ForeignDirectory,
}

#[cfg(test)]
#[derive(Default)]
pub(in super::super::super) struct PublicationPause {
    pub(in super::super::super) entered: AtomicBool,
    released: std::sync::Mutex<bool>,
    ready: std::sync::Condvar,
}
#[cfg(test)]
impl PublicationPause {
    fn wait(&self) {
        let mut released = self.released.lock().unwrap();
        self.entered.store(true, Ordering::SeqCst);
        while !*released {
            released = self.ready.wait(released).unwrap();
        }
    }
    pub(in super::super::super) fn release(&self) {
        *self.released.lock().unwrap() = true;
        self.ready.notify_all();
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(in super::super::super) enum RuntimeFailure {
    Refused,
    RecoveryRequired,
    CommittedWithoutAck,
}

pub(in super::super::super) struct RuntimeRefusal {
    pub(in super::super::super) kind: RuntimeFailure,
    pub(in super::super::super) _installed: Arc<InstalledSelectedPacket>,
    _validated: Option<Arc<ValidatedRuntime>>,
}

pub(in super::super::super) struct PublishedSelectedRuntime {
    pub(in super::super::super) path: PathBuf,
    pub(in super::super::super) record: serde_json::Value,
    _validated: Arc<ValidatedRuntime>,
}

struct ValidatedRuntime {
    installed: Arc<InstalledSelectedPacket>,
    provider: ProviderApproval,
    provider_record: serde_json::Value,
    original: PathBuf,
    python_relative: PathBuf,
    packages_relative: PathBuf,
    provenance: Vec<(PathBuf, Vec<u8>)>,
    metadata: InstalledVersionMetadata,
    probe: serde_json::Value,
}

fn relocated_provenance(
    original: &Path,
    runtime: &Path,
    inputs: &[(PathBuf, Vec<u8>)],
) -> Result<Vec<(PathBuf, Vec<u8>)>> {
    inputs
        .iter()
        .map(|(path, bytes)| {
            Ok((
                runtime.join(
                    path.strip_prefix(original)
                        .map_err(|_| failed("Runtime proof escaped original stage"))?,
                ),
                bytes.clone(),
            ))
        })
        .collect()
}

fn member_fence(packages: &Path, manifest: &StagedFilesManifest) -> Result<()> {
    // Bootstrap pip/setuptools ownership remains separate. Only the previously
    // accepted RECORD members may authorize the selected payload after moving.
    for item in &manifest.files {
        let path = packages.join(&item.path);
        validate_torch_owned_path(packages, &path)?;
        if std::fs::symlink_metadata(&path)
            .map_err(PumasError::from)?
            .len()
            != item.size
            || selection_hash(&path, item.size)? != item.sha256
        {
            return Err(failed(
                "Selected installed member changed after movement or probe",
            ));
        }
    }
    Ok(())
}

fn source_namespace_fence(runtime: &Path, sources: &[(PathBuf, Vec<u8>)]) -> Result<()> {
    let expected: HashSet<_> = sources
        .iter()
        .filter(|(p, _)| p.extension().is_some_and(|x| x == "py"))
        .map(|(p, _)| p.strip_prefix(runtime).map(Path::to_owned))
        .collect::<std::result::Result<_, _>>()
        .map_err(|_| failed("Runtime source escaped"))?;
    let mut actual = HashSet::new();
    for directory in [runtime, &runtime.join("loaders")] {
        for entry in std::fs::read_dir(directory).map_err(PumasError::from)? {
            let entry = entry.map_err(PumasError::from)?;
            if entry.path().extension().is_some_and(|x| x == "py") {
                actual.insert(entry.path().strip_prefix(runtime).unwrap().to_owned());
            }
        }
    }
    if actual != expected {
        return Err(failed(
            "Runtime source namespace differs from owned embedding",
        ));
    }
    Ok(())
}

impl ValidatedRuntime {
    fn fence(&self, runtime: &Path, versions: &Path) -> Result<()> {
        let packet = &self.installed._packet;
        let catalog = &packet.catalog;
        if catalog._stage.path().join("runtime") != self.original
            || !catalog._grant.binding().matches_root(versions)?
        {
            return Err(failed("Runtime publication belongs to another owner"));
        }
        super::super::fence_at(
            catalog,
            &self.original,
            runtime,
            &runtime.join(&self.python_relative),
            &versions.join(&catalog._grant.workspace_identity().relative_target),
        )?;
        if self.provider.validate(&catalog._target)? != self.provider_record {
            return Err(failed("Runtime provider proof differs"));
        }
        for inputs in [
            &packet.provenance,
            &self.installed._provenance,
            &self.provenance,
        ] {
            validate_bounded_provenance(
                runtime,
                &relocated_provenance(&self.original, runtime, inputs)?,
                MAX_INSTALLED_PROOF,
            )?;
        }
        source_namespace_fence(
            runtime,
            &relocated_provenance(&self.original, runtime, &self.provenance)?,
        )?;
        let cache = runtime.join("selected-consumption/probe-cache");
        validate_torch_owned_path(runtime, &cache)?;
        if std::fs::read_dir(&cache)
            .map_err(PumasError::from)?
            .next()
            .is_some()
        {
            return Err(failed("Private probe cache changed"));
        }
        let manifest: StagedFilesManifest =
            serde_json::from_slice(&self.installed._provenance[2].1)
                .map_err(|_| failed("Invalid retained installed manifest"))?;
        member_fence(&runtime.join(&self.packages_relative), &manifest)
    }
}

impl RuntimeLifecycleContext<'_> {
    async fn validate(
        self,
        installed: Arc<InstalledSelectedPacket>,
    ) -> Result<Arc<ValidatedRuntime>> {
        let selection = &self.selection;
        let runtime = selection.runtime.to_owned();
        let python = selection.python.to_owned();
        let versions = selection.installer.versions_dir();
        let tag = self.tag.to_owned();
        let admitted = installed.clone();
        let provider = self.provider.clone();
        let provider_record = selection
            .effect(
                installed._packet.catalog.clone(),
                "admit selected runtime provider",
                move |catalog| {
                    let _lease = admitted;
                    provider.validate(&catalog._target)
                },
            )
            .await?;
        let observation: serde_json::Value =
            serde_json::from_str(&installed._packet.catalog._target.observation)
                .map_err(|_| failed("Invalid original runtime observation"))?;
        let minor = observation["target"]["markers"]["python_version"]
            .as_str()
            .ok_or_else(|| failed("Missing runtime Python version"))?
            .to_owned();
        if tag
            != format!(
                "v{}",
                installed._packet.catalog._request["release"]
                    .as_str()
                    .unwrap_or_default()
            )
            || self.release.tag_name != tag
            || stable_torch_tag(&tag).is_none()
            || installed._packet.catalog._request["build"] != "cpu"
            || self.adapter != "none"
            || self
                .release
                .body
                .as_ref()
                .is_some_and(|s| s.len() > 64 * 1024)
        {
            return Err(failed(
                "Runtime profile is outside the private CPU lifecycle qualification",
            ));
        }
        let selected = installed._packet.evidence["selected"]
            .as_array()
            .ok_or_else(|| failed("Missing selected runtime artifacts"))?;
        let torch = selected
            .iter()
            .find(|row| row["name"] == "torch")
            .ok_or_else(|| failed("Missing selected Torch identity"))?;
        let profile = serde_json::json!({"schema":"pumas.selected-runtime-profile.v1",
            "torch":torch["version"], "adapter":self.adapter, "artifacts":selected,
            "selected_packet_sha256":target_observation_digest(&installed._packet.evidence)?,
            "target_observation_sha256":installed._packet.catalog._target.sha256,
            "provider":provider_record});
        let recipe = serde_json::json!({"recipe_id":"pumas-selected-cpu-runtime-v1", "protocol":SUPPORTED_TORCH_PROTOCOL,
            "capabilities":[TORCH_IMAGE_GENERATION_CAPABILITY], "python":minor, "platform":"linux-x86_64",
            "qualification":"private-staged-cpu-lifecycle", "selected_packet_sha256":profile["selected_packet_sha256"],
            "managed_python":provider_record});
        let metadata = InstalledVersionMetadata {
            path: tag.clone(),
            release_tag: tag,
            installed_date: Utc::now().to_rfc3339(),
            python_version: Some(format!(
                "Python {}",
                observation["target"]["python"].as_str().unwrap_or_default()
            )),
            release_date: Some(self.release.published_at.clone()),
            release_notes: self.release.body.clone(),
            download_url: torch["url"].as_str().map(str::to_owned),
            dependencies_installed: Some(true),
            ..Default::default()
        };
        let mut sources: Vec<_> = embedded_torch_runtime_files()
            .into_iter()
            .map(|(name, bytes)| (name.to_owned(), bytes.as_bytes().to_vec()))
            .collect();
        #[cfg(test)]
        for (name, bytes) in self.fixture_sources {
            if let Some(source) = sources.iter_mut().find(|(n, _)| *n == name) {
                source.1 = bytes;
            } else {
                return Err(failed("Unknown approved fixture runtime source"));
            }
        }
        sources.push((
            "packaging-tooling.zip".into(),
            include_bytes!("../../../../../../torch-server/tooling/packaging.zip").to_vec(),
        ));
        let retained = installed.clone();
        let root = runtime.clone();
        let executable = python.clone();
        let owner_root = versions.clone();
        let minor_copy = minor.clone();
        let profile_bytes =
            serde_json::to_vec(&profile).map_err(|_| failed("Invalid runtime profile"))?;
        let recipe_bytes =
            serde_json::to_vec(&recipe).map_err(|_| failed("Invalid runtime recipe"))?;
        let expected_profile = profile_bytes.clone();
        let provenance = selection
            .effect(
                installed._packet.catalog.clone(),
                "validate and move selected runtime packages",
                move |_| {
                    input_fence(
                        &retained._packet,
                        &root,
                        &executable,
                        &owner_root.join(
                            &retained
                                ._packet
                                .catalog
                                ._grant
                                .workspace_identity()
                                .relative_target,
                        ),
                    )?;
                    let mut provenance: Vec<_> = sources
                        .into_iter()
                        .map(|(name, bytes)| (root.join(name), bytes))
                        .collect();
                    validate_bounded_provenance(&root, &provenance, MAX_EVIDENCE)?;
                    source_namespace_fence(&root, &provenance)?;
                    std::fs::create_dir(root.join("selected-consumption/probe-cache"))
                        .map_err(PumasError::from)?;
                    provenance.push((
                        root.join("selected-consumption/packet.json"),
                        serde_json::to_vec(&retained._packet.evidence)
                            .map_err(|_| failed("Invalid original selected packet"))?,
                    ));

                    validate_bounded_provenance(&root, &retained._provenance, MAX_INSTALLED_PROOF)?;
                    let manifest: StagedFilesManifest =
                        serde_json::from_slice(&retained._provenance[2].1)
                            .map_err(|_| failed("Invalid selected installed proof"))?;
                    installed_fence(&retained.packages, &manifest)?;
                    if profile_bytes.len() > MAX_EVIDENCE {
                        return Err(failed("Runtime profile exceeds budget"));
                    }
                    write_torch_new_provenance(
                        &root.join("selected-runtime-profile.json"),
                        &profile_bytes,
                    )?;
                    std::fs::write(root.join("runtime.json"), &recipe_bytes)
                        .map_err(PumasError::from)?;
                    provenance.push((root.join("selected-runtime-profile.json"), profile_bytes));
                    provenance.push((root.join("runtime.json"), recipe_bytes));
                    move_verified_packages(&retained.packages, &root, &minor_copy)?;
                    member_fence(&torch_site_packages(&root, &minor_copy), &manifest)?;
                    Ok(provenance)
                },
            )
            .await?;
        let mut probe = Command::new(&python);
        probe
            .args(["-I", "-B", "-X"])
            .arg(format!(
                "pycache_prefix={}",
                runtime.join("selected-consumption/probe-cache").display()
            ))
            .arg(runtime.join("probe_runtime.py"))
            .arg("--selected-profile");
        if !selection
            .child(
                probe,
                installed.clone(),
                &runtime.join("selected-consumption"),
            )
            .await?
            .success()
        {
            return Err(failed("Selected runtime probe refused"));
        }
        let checked = installed.clone();
        let root = runtime.clone();
        let consumer_python = python.clone();
        let profile_expected = profile.clone();
        let (probe, proof_path, proof) = selection
            .effect(
                installed._packet.catalog.clone(),
                "validate actual selected runtime probe",
                move |_| {
                    let proof_path = root.join("probe-results.json");
                    let proof = selection_bytes(&proof_path, MAX_EVIDENCE)?;
                    let probe: serde_json::Value = serde_json::from_slice(&proof)
                        .map_err(|_| failed("Invalid runtime probe result"))?;
                    if probe["core_status"] != "passed"
                        || probe["environment"] != profile_expected
                        || probe["context"]["runtime_profile_sha256"]
                            != format!("{:x}", Sha256::digest(&expected_profile))
                        || probe["context"]["executable"]
                            != consumer_python.to_string_lossy().as_ref()
                        || probe["context"]["interpreter_sha256"]
                            != checked._packet.catalog._target.interpreter_sha256
                    {
                        return Err(failed(
                            "Runtime probe identity differs from accepted profile",
                        ));
                    }
                    for artifact in checked._packet.evidence["selected"].as_array().unwrap() {
                        if probe["context"]["installed_distributions"]
                            [artifact["name"].as_str().unwrap_or_default()]
                            != artifact["version"]
                        {
                            return Err(failed("Runtime probe installed distribution differs"));
                        }
                    }
                    Ok((probe, proof_path, proof))
                },
            )
            .await?;
        let mut provenance = provenance;
        provenance.push((proof_path, proof));
        let validated = ValidatedRuntime {
            installed,
            provider: self.provider,
            provider_record,
            original: runtime.clone(),
            python_relative: python
                .strip_prefix(&runtime)
                .map_err(|_| failed("Runtime interpreter escaped"))?
                .to_owned(),
            packages_relative: torch_site_packages(&runtime, &minor)
                .strip_prefix(&runtime)
                .unwrap()
                .to_owned(),
            provenance,
            metadata,
            probe,
        };
        let validated = Arc::new(validated);
        let check = validated.clone();
        selection
            .effect(
                validated.installed._packet.catalog.clone(),
                "final post-probe selected runtime fence",
                move |_| check.fence(&check.original, &versions),
            )
            .await?;
        Ok(validated)
    }
}

impl InstalledSelectedPacket {
    pub(in super::super::super) async fn publish(
        self,
        context: RuntimeLifecycleContext<'_>,
    ) -> std::result::Result<PublishedSelectedRuntime, RuntimeRefusal> {
        let installed = Arc::new(self);
        let consumer = context.selection.consumer;
        let metadata_manager = context.selection.installer.metadata_manager.clone();
        let versions = context.selection.installer.versions_dir();
        let control = context.selection.installer.torch_control.clone();
        let cancelled = context.selection.installer.cancel_flag.clone();
        #[cfg(test)]
        let fault = context.fault;
        #[cfg(test)]
        let publication_pause = context.publication_pause.clone();
        let result = tokio::time::timeout(DEADLINE, context.validate(installed.clone())).await;
        let validated = match result {
            Ok(Ok(v)) => v,
            _ => {
                return Err(RuntimeRefusal {
                    kind: RuntimeFailure::Refused,
                    _installed: installed,
                    _validated: None,
                })
            }
        };
        let lease = validated.clone();
        let result = consumer.run_blocking("publish owned selected runtime", move || Ok((|| {
            lease.fence(&lease.original, &versions).map_err(|_| RuntimeFailure::Refused)?;
            let destination = versions.join(&lease.metadata.path);
            let pending = versions.join(format!(".torch-pending-publish-{}", lease.metadata.path));
            if destination.symlink_metadata().is_ok() || pending.symlink_metadata().is_ok()
                || metadata_manager.get_installed_version(&lease.metadata.path, Some(AppId::Torch)).map_err(|_| RuntimeFailure::Refused)?.is_some()
                || cancelled.load(Ordering::SeqCst) || !control.try_begin_publication() {
                return Err(RuntimeFailure::Refused);
            }
            let directory = torch_directory_identity(&lease.original).map_err(|_| RuntimeFailure::Refused)?;
            let record = serde_json::json!({"schema":"pumas.selected-runtime-install.v1", "directory":directory,
                "metadata":lease.metadata, "catalog_receipt_sha256":target_observation_digest(&serde_json::to_value(&lease.installed._packet.catalog.receipt).map_err(|_| RuntimeFailure::Refused)?).map_err(|_| RuntimeFailure::Refused)?,
                "catalog_acquisition_id":lease.installed._packet.catalog.receipt.acquisition_id,
                "local_installation":lease.installed.evidence, "provider":lease.provider_record, "probe":lease.probe});
            let record_bytes = serde_json::to_vec_pretty(&record).map_err(|_| RuntimeFailure::Refused)?;
            if record_bytes.len() > MAX_EVIDENCE { return Err(RuntimeFailure::Refused); }
            let marker = serde_json::json!({"owner":"selected runtime metadata pending", "directory":directory,
                "source_stage":lease.original.parent().and_then(Path::file_name).and_then(|s| s.to_str()), "record_sha256":format!("{:x}",Sha256::digest(&record_bytes))});
            pumas_library::metadata::atomic_write_json(&lease.original.join("selected-runtime-install.json"), &record, false).map_err(|_| RuntimeFailure::RecoveryRequired)?;
            pumas_library::metadata::atomic_write_json(&pending, &marker, false).map_err(|_| RuntimeFailure::RecoveryRequired)?;
            sync_native_directory(&versions).map_err(|_| RuntimeFailure::RecoveryRequired)?;
            let publication = (|| -> Result<()> {
                #[cfg(test)]
                if fault == PublicationFault::BeforeRename { return Err(failed("Controlled pre-rename refusal")); }
                pumas_library::platform::filesystem::rename_directory_noreplace(&lease.original, &destination).map_err(PumasError::from)?;
                sync_native_directory(lease.original.parent().unwrap())?;
                sync_native_directory(&versions)?;
                #[cfg(test)]
                if let Some(pause) = publication_pause { pause.wait(); }
                #[cfg(test)]
                match fault {
                    PublicationFault::AfterRename => return Err(failed("Controlled pre-metadata refusal")),
                    PublicationFault::MovedMember => std::fs::write(destination.join(&lease.packages_relative).join("torch.py"), b"changed after public move").map_err(PumasError::from)?,
                    PublicationFault::MovedRecord => std::fs::write(destination.join("selected-runtime-install.json"), b"changed publication proof").map_err(PumasError::from)?,
                    PublicationFault::ForeignDirectory => {
                        std::fs::rename(&destination, versions.join("fixture-moved-original")).map_err(PumasError::from)?;
                        std::fs::create_dir(&destination).map_err(PumasError::from)?;
                        std::fs::write(destination.join("foreign-keeper"), b"foreign directory retained").map_err(PumasError::from)?;
                    }
                    _ => {}
                }
                if torch_directory_identity(&destination)? != directory
                    || selection_bytes(&destination.join("selected-runtime-install.json"), record_bytes.len())? != record_bytes {
                    return Err(failed("Published directory or installation record changed"));
                }
                lease.fence(&destination, &versions)?;
                metadata_manager.update_installed_version(&lease.metadata.path, lease.metadata.clone(), Some(AppId::Torch))?;
                #[cfg(test)]
                if fault == PublicationFault::AfterMetadata { return Err(failed("Controlled lost publication acknowledgment")); }
                let current = metadata_manager.get_installed_version(&lease.metadata.path, Some(AppId::Torch))?
                    .ok_or_else(|| failed("Published metadata missing"))?;
                if !metadata_matches(&current, &lease.metadata) { return Err(failed("Published metadata identity changed")); }
                lease.fence(&destination, &versions)?;
                if torch_directory_identity(&destination)? != directory
                    || selection_bytes(&destination.join("selected-runtime-install.json"), record_bytes.len())? != record_bytes {
                    return Err(failed("Published runtime changed before acknowledgment"));
                }
                std::fs::remove_file(&pending).map_err(PumasError::from)?;
                sync_native_directory(&versions)
            })();
            if publication.is_err() {
                let current = metadata_manager.get_installed_version(&lease.metadata.path, Some(AppId::Torch));
                if current.as_ref().is_ok_and(|m| m.as_ref().is_some_and(|m| metadata_matches(m, &lease.metadata))) {
                    return Err(RuntimeFailure::CommittedWithoutAck);
                }
                if current.is_ok_and(|m| m.is_none()) {
                    if destination.symlink_metadata().is_ok() {
                        if torch_directory_identity(&destination).ok() != Some(directory) { return Err(RuntimeFailure::RecoveryRequired); }
                        std::fs::remove_dir_all(&destination).map_err(|_| RuntimeFailure::RecoveryRequired)?;
                    }
                    std::fs::remove_file(&pending).map_err(|_| RuntimeFailure::RecoveryRequired)?;
                    sync_native_directory(&versions).map_err(|_| RuntimeFailure::RecoveryRequired)?;
                    return Err(RuntimeFailure::Refused);
                }
                return Err(RuntimeFailure::RecoveryRequired);
            }
            Ok((destination, record))
        })())).await;
        match result {
            Ok(Ok((path, record))) => Ok(PublishedSelectedRuntime {
                path,
                record,
                _validated: validated,
            }),
            Ok(Err(kind)) => Err(RuntimeRefusal {
                kind,
                _installed: installed,
                _validated: Some(validated),
            }),
            Err(_) => Err(RuntimeRefusal {
                kind: RuntimeFailure::RecoveryRequired,
                _installed: installed,
                _validated: Some(validated),
            }),
        }
    }
}
