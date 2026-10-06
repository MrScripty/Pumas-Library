//! Dormant local installation proof. No runtime publication or receipt mutation.
use super::*;

const MAX_INSTALLED_PROOF: usize = 32 * 1024 * 1024;
const MAX_INSTALLED_BYTES: u64 = 64 * 1024 * 1024;

pub(in super::super) struct InstalledSelectedPacket {
    pub(in super::super) evidence: serde_json::Value,
    pub(in super::super) packages: PathBuf,
    pub(in super::super) _packet: Arc<SelectedWheelPacket>,
    _provenance: Vec<(PathBuf, Vec<u8>)>,
}

pub(in super::super) struct ConsumptionRefusal {
    pub(in super::super) _packet: Arc<SelectedWheelPacket>,
}

fn input_fence(
    packet: &SelectedWheelPacket,
    runtime: &Path,
    python: &Path,
    wheels: &Path,
) -> Result<()> {
    if packet.catalog._stage.path().join("runtime") != runtime {
        return Err(failed("Selected packet belongs to another pending stage"));
    }
    fence(&packet.catalog, runtime, python, wheels)?;
    validate_selection_provenance(runtime, &packet.provenance)
}

// The installed manifest is already checked against every actual RECORD by the
// single Python owner. Recheck every member with bounded streaming reads after
// that child exits, so its successful exit is never sufficient acceptance.
fn installed_fence(packages: &Path, manifest: &StagedFilesManifest) -> Result<()> {
    if manifest.files.is_empty() || manifest.files.len() > 200_000 {
        return Err(failed("Selected installed manifest is empty or oversized"));
    }
    let mut expected = std::collections::HashMap::new();
    let mut total = 0_u64;
    for item in &manifest.files {
        let path = Path::new(&item.path);
        total = total
            .checked_add(item.size)
            .ok_or_else(|| failed("Installed byte budget exceeded"))?;
        if item.path.len() > 1024
            || item.path.contains('\\')
            || path
                .components()
                .any(|part| !matches!(part, std::path::Component::Normal(_)))
            || item.sha256.len() != 64
            || !item.sha256.bytes().all(|b| b.is_ascii_hexdigit())
            || total > MAX_INSTALLED_BYTES
            || expected.insert(item.path.as_str(), item).is_some()
        {
            return Err(failed(
                "Selected installed manifest exceeds its namespace or byte budget",
            ));
        }
    }
    let mut seen = 0;
    for entry in walkdir::WalkDir::new(packages).follow_links(false) {
        let entry = entry.map_err(|_| failed("Cannot inspect selected installed members"))?;
        if entry.file_type().is_dir() {
            continue;
        }
        if !entry.file_type().is_file() {
            return Err(failed("Installed member is linked or special"));
        }
        let relative = entry
            .path()
            .strip_prefix(packages)
            .map_err(|_| failed("Installed member escaped"))?
            .to_str()
            .ok_or_else(|| failed("Invalid installed member namespace"))?
            .replace(std::path::MAIN_SEPARATOR, "/");
        let item = expected
            .get(relative.as_str())
            .ok_or_else(|| failed("Unreported installed member"))?;
        if std::fs::symlink_metadata(entry.path())
            .map_err(PumasError::from)?
            .len()
            != item.size
            || selection_hash(entry.path(), item.size)? != item.sha256
        {
            return Err(failed("Installed member changed after RECORD inspection"));
        }
        seen += 1;
    }
    if seen != expected.len() {
        return Err(failed("Missing installed member"));
    }
    Ok(())
}

impl SelectionContext<'_> {
    async fn consume_inner(
        &self,
        packet: Arc<SelectedWheelPacket>,
    ) -> Result<InstalledSelectedPacket> {
        if self.consumer.owner() != CATALOG_OWNER {
            return Err(failed("Wrong selected consumer owner"));
        }
        let directory = self.runtime.join("selected-consumption");
        let packages = directory.join("packages");
        let output = directory.join("proof");
        let wheels = self
            .installer
            .versions_dir()
            .join(&packet.catalog._grant.workspace_identity().relative_target);
        let runtime = self.runtime.to_owned();
        let python = self.python.to_owned();
        let root = wheels.clone();
        let work = directory.clone();
        let admitted = packet.clone();
        let versions = self.installer.versions_dir();
        let packet_bytes =
            serde_json::to_vec(&packet.evidence).map_err(|_| failed("Invalid selected packet"))?;
        let retained_packet = packet_bytes.clone();
        self.effect(
            packet.catalog.clone(),
            "admit selected local consumption",
            move |c| {
                input_fence(&admitted, &runtime, &python, &root)?;
                if !c._grant.binding().matches_root(&versions)? || packet_bytes.len() > MAX_EVIDENCE
                {
                    return Err(failed("Selected consumption owner or packet differs"));
                }
                std::fs::create_dir(&work).map_err(PumasError::from)?;
                for leaf in ["home", "cache", "tmp"] {
                    std::fs::create_dir(work.join(leaf)).map_err(PumasError::from)?;
                }
                std::fs::write(work.join("packet.json"), packet_bytes).map_err(PumasError::from)
            },
        )
        .await?;
        let mut command = Command::new(self.python);
        command
            .arg("-I")
            .arg(self.runtime.join("consume_selected_wheels.py"));
        for (name, path) in [
            ("request", self.runtime.join("catalog-request.json")),
            (
                "observation",
                self.runtime.join("catalog-approved-target.json"),
            ),
            ("catalog", self.runtime.join("catalog-evidence.json")),
            ("wheels", wheels.clone()),
            ("directory", self.runtime.join("offline-selection")),
            ("packet", directory.join("packet.json")),
            ("target", packages.clone()),
            ("output", output.clone()),
        ] {
            command.arg(format!("--{name}")).arg(path);
        }
        let mut verification = Command::new(command.as_std().get_program());
        verification
            .args(command.as_std().get_args())
            .arg("--verify-only");
        if !self
            .child(command, packet.clone(), &directory)
            .await?
            .success()
        {
            return Err(failed("Selected local installation child refused"));
        }
        let proof_root = output.clone();
        let output_lease = packet.clone();
        let retained = self
            .effect(
                packet.catalog.clone(),
                "retain actual selected installation output",
                move |_| {
                    let _lease = output_lease;
                    [
                        "local-pip-report.json",
                        "local-requirements.txt",
                        "installed-files.json",
                    ]
                    .into_iter()
                    .map(|name| {
                        let path = proof_root.join(name);
                        let cap = if name == "installed-files.json" {
                            MAX_INSTALLED_PROOF
                        } else {
                            MAX_EVIDENCE
                        };
                        selection_bytes(&path, cap).map(|bytes| (path, bytes))
                    })
                    .collect::<Result<Vec<_>>>()
                },
            )
            .await?;
        if !self
            .child(verification, packet.clone(), &directory)
            .await?
            .success()
        {
            return Err(failed("Selected installation proof child refused"));
        }
        let runtime = self.runtime.to_owned();
        let python = self.python.to_owned();
        let accepted = packet.clone();
        let installed = packages.clone();
        let proof = self
            .effect(
                packet.catalog.clone(),
                "fence selected installation proof",
                move |_| {
                    input_fence(&accepted, &runtime, &python, &wheels)?;
                    validate_selection_provenance(
                        &runtime,
                        &[(directory.join("packet.json"), retained_packet)],
                    )?;
                    for (path, expected) in &retained {
                        validate_torch_owned_path(&runtime, path)?;
                        if selection_bytes(path, expected.len())? != *expected {
                            return Err(failed(
                                "Selected installation output changed after verification",
                            ));
                        }
                    }
                    validate_torch_owned_path(&runtime, &installed)?;
                    let manifest_bytes = &retained[2].1;
                    let manifest: StagedFilesManifest = serde_json::from_slice(manifest_bytes)
                        .map_err(|_| failed("Invalid installed selected manifest"))?;
                    installed_fence(&installed, &manifest)?;
                    let report_bytes = &retained[0].1;
                    let requirements = &retained[1].1;
                    input_fence(&accepted, &runtime, &python, &wheels)?;
                    // Retain checked output bytes as proof, not a new acquisition receipt.
                    let evidence = serde_json::json!({"schema":"pumas.selected-local-install.v1",
                "selected_packet_sha256": target_observation_digest(&accepted.evidence)?,
                "catalog_request_sha256": accepted.evidence["request_sha256"],
                "target_observation_sha256": accepted.catalog._target.sha256,
                "interpreter_sha256": accepted.catalog._target.interpreter_sha256,
                "installation_report_sha256":format!("{:x}",Sha256::digest(report_bytes)),
                "installed_manifest_sha256":format!("{:x}",Sha256::digest(manifest_bytes)),
                "local_requirements_sha256":format!("{:x}",Sha256::digest(requirements)),
                "installed_files":manifest.files.len()});
                    Ok((evidence, retained))
                },
            )
            .await?;
        Ok(InstalledSelectedPacket {
            evidence: proof.0,
            packages,
            _packet: packet,
            _provenance: proof.1,
        })
    }
}

impl SelectedWheelPacket {
    pub(in super::super) async fn consume(
        self,
        context: SelectionContext<'_>,
    ) -> std::result::Result<InstalledSelectedPacket, ConsumptionRefusal> {
        let packet = Arc::new(self);
        let result = tokio::time::timeout(DEADLINE, context.consume_inner(packet.clone())).await;
        match result {
            Ok(Ok(installed)) => Ok(installed),
            _ => Err(ConsumptionRefusal { _packet: packet }),
        }
    }
}
