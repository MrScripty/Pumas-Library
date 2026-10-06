//! Request-bound finite catalog snapshots. No resolver or installer adoption.
use super::*;
use pumas_library::acquisition::{AcquisitionPhase, AcquisitionService};
use std::io::Read;

const CATALOG_OWNER: &str = "runtime.torch.catalog";
const MAX_REQUEST: usize = 10 * 1024 * 1024;
const MAX_EVIDENCE: usize = 2 * 1024 * 1024;
const DEADLINE: Duration = Duration::from_secs(120);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CatalogFailure {
    Incomplete,
    Refused,
}

// Only the trusted caller can assemble this request from original selection,
// original profile requirements and independently produced target evidence.
// No request/Complete Deserialize implementation and no imported completion API.
#[derive(Clone)]
struct CatalogRequest {
    wire: serde_json::Value,
    target: AcceptedTorchTarget,
}

impl CatalogRequest {
    #[allow(clippy::too_many_arguments)]
    fn from_observed(
        selection: &str,
        release: &str,
        build: &str,
        roots: &[String],
        constraints: &[String],
        direct_roots: serde_json::Value,
        observations: serde_json::Value,
        produced: ProducedTorchTarget,
    ) -> Result<Self> {
        let observation: serde_json::Value = serde_json::from_str(&produced.observation)
            .map_err(|_| failed("Invalid owned catalog target observation"))?;
        let sha256 = target_observation_digest(&observation)?;
        Ok(Self {
            wire: serde_json::json!({
                "schema":"pumas.wheel-catalog-request.v1", "selection":selection,
                "release":release, "build":build, "roots":roots, "constraints":constraints,
                "direct_roots":direct_roots, "observations":observations,
                "target_observation_sha256":sha256,
            }),
            target: AcceptedTorchTarget {
                observation: produced.observation,
                sha256,
                interpreter_sha256: produced.interpreter_sha256,
                producer_path: Some(produced.path),
            },
        })
    }
}

#[derive(Deserialize, Serialize, Clone)]
#[serde(deny_unknown_fields)]
struct CatalogCandidate {
    id: String,
    repository: String,
    name: String,
    version: String,
    filename: String,
    url: String,
    sha256: String,
    size: u64,
    yanked: serde_json::Value,
    requires_python_hint: Option<String>,
}

#[derive(Deserialize, Clone)]
#[serde(deny_unknown_fields)]
struct CatalogPlan {
    schema: String,
    request_sha256: String,
    target_observation_sha256: String,
    candidates: Vec<CatalogCandidate>,
    observations: serde_json::Value,
    exclusions: serde_json::Value,
}

// A catalog snapshot receipt, not installation evidence or reusable Using access.
// Retain owner grants after settlement for private read-only selection. No API
// reconstructs an AcquiredArtifactUse or stale Using lease from this receipt.
struct CompleteCatalog {
    receipt: AcquisitionConsumerReceipt,
    evidence: serde_json::Value,
    _request: serde_json::Value,
    _target: AcceptedTorchTarget,
    _grant: ReservedDirectory,
    _stage: Arc<TorchPendingStage>,
}

struct CatalogContext<'a> {
    installer: &'a VersionInstaller,
    consumer: &'a AcquisitionConsumer,
    runtime: &'a Path,
    python: &'a Path,
    stage: Arc<TorchPendingStage>,
    grant: ReservedDirectory,
    log: &'a Path,
    progress: &'a mpsc::Sender<ProgressUpdate>,
    #[cfg(test)]
    fixture_origin: Option<String>,
}

async fn open_catalog_consumer(
    service: Arc<AcquisitionService>,
) -> Result<Arc<AcquisitionConsumer>> {
    let consumer = Arc::new(service.open_consumer(CATALOG_OWNER)?);
    let store = service.store().clone();
    let owner = consumer.owner().to_owned();
    let records = consumer
        .run_blocking("inspect retained Torch catalog snapshots", move || {
            store.acquisitions()
        })
        .await?;
    if records.values().any(|row| {
        row.demand.consumer == owner
            && !matches!(
                row.phase,
                AcquisitionPhase::Adopted { .. } | AcquisitionPhase::Withdrawn
            )
    }) {
        return Err(failed(
            "Unsettled Torch catalog snapshot requires owner recovery",
        ));
    }
    Ok(consumer)
}

fn read_bounded(path: &Path, maximum: usize) -> Result<serde_json::Value> {
    let metadata = std::fs::symlink_metadata(path).map_err(PumasError::from)?;
    if !metadata.file_type().is_file() || metadata.len() > maximum as u64 {
        return Err(failed("Catalog output missing, linked or oversized"));
    }
    let file = File::open(path).map_err(PumasError::from)?;
    let mut bytes = Vec::new();
    std::io::Read::take(file, maximum as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(PumasError::from)?;
    if bytes.len() > maximum {
        return Err(failed("Catalog output exceeded byte budget"));
    }
    serde_json::from_slice(&bytes).map_err(|_| failed("Invalid owned catalog output"))
}

fn write_torch_new_provenance(path: &Path, bytes: &[u8]) -> Result<()> {
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(PumasError::from)?;
    std::io::Write::write_all(&mut file, bytes).map_err(PumasError::from)
}

// An expected request refusal is data, not a failed supervisor task. The shared
// scope still owns/joins the complete blocking effect, including on abandonment.
async fn catalog_effect<T: Send + 'static>(
    consumer: &AcquisitionConsumer,
    stage: Arc<TorchPendingStage>,
    name: &'static str,
    work: impl FnOnce() -> Result<T> + Send + 'static,
) -> Result<T> {
    consumer
        .run_blocking(name, move || {
            let _stage = stage;
            Ok(work())
        })
        .await?
}

fn catalog_manifest(plan: &CatalogPlan) -> Result<ArtifactManifest> {
    if plan.candidates.is_empty() || plan.candidates.len() > 128 {
        return Err(failed("Catalog has no bounded acquisition set"));
    }
    let source = ArtifactSourceIdentity::new(
        "python-wheels",
        "torch.catalog-snapshot",
        ArtifactRevisionEvidence::new(
            "torch.catalog.request.sha256",
            &plan.request_sha256,
            RevisionStrength::Immutable,
        )
        .map_err(|_| failed("Invalid catalog revision"))?,
    )
    .map_err(|_| failed("Invalid catalog source"))?;
    let files = plan
        .candidates
        .iter()
        .map(|c| {
            if c.id.len() != 64
                || !c.id.bytes().all(|b| b.is_ascii_hexdigit())
                || torch_wheel_filename(&c.url)? != c.filename
                || c.size == 0
            {
                return Err(failed("Invalid catalog candidate identity"));
            }
            ArtifactFile::new(
                format!("{}/{}", c.id, c.filename),
                c.url.clone(),
                Some(c.size),
                Some(
                    Sha256Evidence::new("torch.catalog.simple.sha256", &c.sha256)
                        .map_err(|_| failed("Invalid catalog digest"))?,
                ),
                FileVerificationRequirement::Sha256,
            )
            .map_err(|_| failed("Invalid catalog manifest"))
        })
        .collect::<Result<Vec<_>>>()?;
    ArtifactManifest::new(source, files).map_err(|_| failed("Invalid catalog namespace"))
}

fn validate_catalog_source_plan(plan: &CatalogPlan, request: &CatalogRequest) -> Result<()> {
    let mut total = 0_u64;
    let build = request.wire["build"]
        .as_str()
        .ok_or_else(|| failed("Missing catalog build"))?;
    let observation: serde_json::Value = serde_json::from_str(&request.target.observation)
        .map_err(|_| failed("Invalid catalog target"))?;
    let mac_cpu = observation["target"]["os"] == "macos" && build == "cpu";
    let observations = request.wire["observations"]
        .as_array()
        .ok_or_else(|| failed("Missing catalog observations"))?;
    for candidate in &plan.candidates {
        total = total
            .checked_add(candidate.size)
            .ok_or_else(|| failed("Catalog byte budget overflow"))?;
        let url =
            reqwest::Url::parse(&candidate.url).map_err(|_| failed("Invalid catalog source"))?;
        let torch = matches!(candidate.name.as_str(), "torch" | "torchvision");
        let pytorch = matches!(
            url.host_str(),
            Some("download.pytorch.org" | "download-r2.pytorch.org")
        ) && url.path().starts_with("/whl/");
        let pypi = url.host_str() == Some("files.pythonhosted.org")
            && url.path().starts_with("/packages/");
        if !safe_torch_download_source(&candidate.url)
            || url.port_or_known_default() != Some(443)
            || !(if torch {
                (pytorch && url.path().starts_with(&format!("/whl/{build}/"))) || (pypi && mac_cpu)
            } else {
                pytorch || pypi
            })
        {
            return Err(failed("Catalog source exceeds caller authority"));
        }
        let mut identity =
            serde_json::to_value(candidate).map_err(|_| failed("Invalid catalog candidate"))?;
        identity
            .as_object_mut()
            .ok_or_else(|| failed("Invalid catalog candidate"))?
            .remove("id");
        if target_observation_digest(&identity)? != candidate.id {
            return Err(failed("Catalog artifact identity changed"));
        }
        let matched = observations.iter().any(|o| {
            if o["project"] != candidate.name
                || o["repository"] != candidate.repository
                || o["status"] != 200
            {
                return false;
            }
            let Some(body) = o["body"].as_str() else {
                return false;
            };
            let Ok(body) = serde_json::from_str::<serde_json::Value>(body) else {
                return false;
            };
            body["files"].as_array().is_some_and(|rows| {
                rows.iter().any(|row| {
                    row["url"] == candidate.url
                        && row["filename"] == candidate.filename
                        && row["hashes"]["sha256"] == candidate.sha256
                        && row["size"] == candidate.size
                        && row
                            .get("requires-python")
                            .unwrap_or(&serde_json::Value::Null)
                            == &serde_json::json!(candidate.requires_python_hint)
                        && row.get("yanked").unwrap_or(&serde_json::Value::Bool(false))
                            == &candidate.yanked
                })
            })
        });
        if !matched {
            return Err(failed(
                "Catalog candidate lacks original observation authority",
            ));
        }
    }
    if total > 64 * 1024 * 1024 {
        return Err(failed("Catalog byte budget exceeded"));
    }
    Ok(())
}

impl CatalogContext<'_> {
    async fn inspect_command(
        &self,
        request_path: &Path,
        output: &Path,
        plan: Option<&Path>,
        inputs: Option<Arc<AcquiredArtifactUse>>,
    ) -> Result<CatalogFailureOrSuccess> {
        let mut command = Command::new(self.python);
        command
            .args(["-I"])
            .arg(self.runtime.join("wheel_catalog_owner.py"))
            .arg("--request")
            .arg(request_path)
            .arg("--observation")
            .arg(self.runtime.join("catalog-approved-target.json"))
            .arg("--output")
            .arg(output);
        if let Some(plan) = plan {
            self.grant.validate()?;
            command.arg("--plan").arg(plan).arg("--wheels").arg(
                self.installer
                    .versions_dir()
                    .join(&self.grant.workspace_identity().relative_target),
            );
        }
        let status = self
            .installer
            .run_runtime_command_status_with_custody(
                command,
                self.log,
                "Inspecting bounded wheel catalog",
                self.progress,
                Some(TorchChildLease {
                    stage: self.stage.clone(),
                    _inputs: inputs,
                }),
                None,
            )
            .await?;
        Ok(if status.success() {
            CatalogFailureOrSuccess::Success
        } else if status.code() == Some(20) {
            CatalogFailureOrSuccess::Incomplete
        } else {
            CatalogFailureOrSuccess::Refused
        })
    }

    async fn acquire(
        self,
        request: CatalogRequest,
    ) -> std::result::Result<CompleteCatalog, CatalogFailure> {
        tokio::time::timeout(DEADLINE, self.acquire_inner(request))
            .await
            .map_err(|_| CatalogFailure::Incomplete)?
    }

    async fn acquire_inner(
        self,
        request: CatalogRequest,
    ) -> std::result::Result<CompleteCatalog, CatalogFailure> {
        if self.consumer.owner() != CATALOG_OWNER {
            return Err(CatalogFailure::Refused);
        }
        let request_path = self.runtime.join("catalog-request.json");
        let plan_path = self.runtime.join("catalog-plan.json");
        let proof_path = self.runtime.join("catalog-evidence.json");
        let runtime = self.runtime.to_owned();
        let python = self.python.to_owned();
        let target = request.target.clone();
        let wire = serde_json::to_vec(&request.wire).map_err(|_| CatalogFailure::Refused)?;
        if wire.len() > MAX_REQUEST {
            return Err(CatalogFailure::Incomplete);
        }
        let request_sha256 =
            target_observation_digest(&request.wire).map_err(|_| CatalogFailure::Refused)?;
        let request_file = request_path.clone();
        let approved_path = self.runtime.join("catalog-approved-target.json");
        let stage = self.stage.clone();
        let grant = self.grant.clone();
        let versions = self.installer.versions_dir();
        catalog_effect(
            self.consumer,
            self.stage.clone(),
            "retain catalog request and approval",
            move || {
                let _stage = stage;
                grant.validate()?;
                if !grant.binding().matches_root(&versions)? {
                    return Err(failed("Catalog workspace belongs to another owner root"));
                }
                validate_torch_target_evidence(&runtime, &target, &python)?;
                write_torch_new_provenance(&request_file, &wire)?;
                write_torch_new_provenance(&approved_path, target.observation.as_bytes())
            },
        )
        .await
        .map_err(|_| CatalogFailure::Refused)?;
        match self
            .inspect_command(&request_path, &plan_path, None, None)
            .await
            .map_err(|_| CatalogFailure::Incomplete)?
        {
            CatalogFailureOrSuccess::Incomplete => return Err(CatalogFailure::Incomplete),
            CatalogFailureOrSuccess::Refused => return Err(CatalogFailure::Refused),
            CatalogFailureOrSuccess::Success => {}
        }
        let path = plan_path.clone();
        let plan_value = catalog_effect(
            self.consumer,
            self.stage.clone(),
            "read catalog acquisition plan",
            move || read_bounded(&path, MAX_EVIDENCE),
        )
        .await
        .map_err(|_| CatalogFailure::Refused)?;
        let plan: CatalogPlan =
            serde_json::from_value(plan_value.clone()).map_err(|_| CatalogFailure::Refused)?;
        if plan.schema != "pumas.wheel-catalog-plan.v1"
            || plan.request_sha256 != request_sha256
            || plan.target_observation_sha256 != request.target.sha256
        {
            return Err(CatalogFailure::Refused);
        }
        let source_plan = plan.clone();
        let source_request = request.clone();
        let runtime = self.runtime.to_owned();
        let python = self.python.to_owned();
        let target = request.target.clone();
        let retained_request = request_path.clone();
        let retained_plan = plan_path.clone();
        let expected_plan = plan_value.clone();
        let wire = serde_json::to_vec(&request.wire).map_err(|_| CatalogFailure::Refused)?;
        catalog_effect(
            self.consumer,
            self.stage.clone(),
            "fence catalog before payload acquisition",
            move || {
                validate_catalog_source_plan(&source_plan, &source_request)?;
                validate_torch_target_evidence(&runtime, &target, &python)?;
                validate_torch_provenance(
                    &runtime,
                    &[
                        (retained_request, wire),
                        (
                            runtime.join("catalog-approved-target.json"),
                            target.observation.as_bytes().to_vec(),
                        ),
                    ],
                )?;
                if read_bounded(&retained_plan, MAX_EVIDENCE)? != expected_plan {
                    return Err(failed("Catalog plan changed"));
                }
                Ok(())
            },
        )
        .await
        .map_err(|_| CatalogFailure::Refused)?;
        let manifest = catalog_manifest(&plan).map_err(|_| CatalogFailure::Incomplete)?;
        let sources: Vec<_> = plan
            .candidates
            .iter()
            .map(|c| AcquisitionHttpSource {
                url: c.url.clone(),
                authorization: None,
            })
            .collect();
        #[cfg(test)]
        let sources = {
            let mut sources = sources;
            if let Some(origin) = &self.fixture_origin {
                for (source, candidate) in sources.iter_mut().zip(&plan.candidates) {
                    source.url = format!("{origin}/{}/{}", candidate.id, candidate.filename);
                }
            }
            sources
        };
        let acquisition = AcquisitionHttpRequest {
            demand: AcquisitionDemand {
                consumer: self.consumer.owner().to_owned(),
                operation: request_sha256.clone(),
            },
            manifest: manifest.clone(),
            workspace: self
                .grant
                .acquisition_workspace()
                .map_err(|_| CatalogFailure::Refused)?,
            sources,
            retry: AcquisitionRetryPolicy {
                attempts: Some(1),
                elapsed: DEADLINE,
                backoff: RetryConfig::new(),
            },
        };
        // No redirects/retries/ambient proxy can extend the approved snapshot.
        let client = reqwest::Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|_| CatalogFailure::Refused)?;
        let mut completion = None;
        let mut inspection_failure = CatalogFailure::Incomplete;
        let context = &self;
        let request = &request;
        let manifest = &manifest;
        let plan_value = &plan_value;
        let request_path = &request_path;
        let plan_path = &plan_path;
        let proof_path = &proof_path;
        let failure = &mut inspection_failure;
        let completion_slot = &mut completion;
        let result = with_verified_torch_wheels(
            self.consumer,
            acquisition,
            client,
            Box::new(TorchWheelHost {
                cancel: self.installer.cancel_flag.clone(),
                shutdown: Arc::new(AtomicBool::new(false)),
                progress: self.installer.progress_tracker.clone(),
            }),
            |inputs| async move {
                if &inputs.record().manifest != manifest {
                    return Err(failed("Catalog use identity differs"));
                }
                for i in 0..manifest.files().len() {
                    drop(inputs.open_file(i).await?);
                }
                match context
                    .inspect_command(
                        request_path,
                        proof_path,
                        Some(plan_path),
                        Some(inputs.clone()),
                    )
                    .await?
                {
                    CatalogFailureOrSuccess::Incomplete => {
                        *failure = CatalogFailure::Incomplete;
                        return Err(failed("Catalog incomplete"));
                    }
                    CatalogFailureOrSuccess::Refused => {
                        *failure = CatalogFailure::Refused;
                        return Err(failed("Catalog refused"));
                    }
                    CatalogFailureOrSuccess::Success => {}
                }
                *failure = CatalogFailure::Refused;
                for i in 0..manifest.files().len() {
                    drop(inputs.open_file(i).await?);
                }
                let runtime = context.runtime.to_owned();
                let python = context.python.to_owned();
                let target = request.target.clone();
                let path = proof_path.to_owned();
                let expected_plan = plan_value.to_owned();
                let request_file = request_path.to_owned();
                let wire = serde_json::to_vec(&request.wire)
                    .map_err(|_| failed("Invalid catalog request"))?;
                let stage = context.stage.clone();
                let proof = inputs
                    .run_blocking("validate final catalog snapshot", move || {
                        let _stage = stage;
                        Ok((|| {
                            validate_torch_target_evidence(&runtime, &target, &python)?;
                            validate_torch_provenance(
                                &runtime,
                                &[
                                    (request_file, wire),
                                    (
                                        runtime.join("catalog-approved-target.json"),
                                        target.observation.as_bytes().to_vec(),
                                    ),
                                ],
                            )?;
                            let proof = read_bounded(&path, MAX_EVIDENCE)?;
                            if proof["schema"] != "pumas.wheel-catalog-evidence.v1"
                                || [
                                    "request_sha256",
                                    "target_observation_sha256",
                                    "candidates",
                                    "observations",
                                    "exclusions",
                                ]
                                .iter()
                                .any(|field| proof[field] != expected_plan[field])
                            {
                                return Err(failed(
                                    "Catalog evidence differs from approved acquisition",
                                ));
                            }
                            Ok(proof)
                        })())
                    })
                    .await??;
                Ok((context.runtime.to_owned(), proof))
            },
            |_, receipt| async move {
                *completion_slot = Some(receipt);
                Ok(())
            },
        )
        .await;
        result.map_err(|_| inspection_failure)?;
        let receipt = completion.ok_or(CatalogFailure::Incomplete)?;
        Ok(CompleteCatalog {
            evidence: receipt.payload.clone(),
            receipt,
            _request: request.wire.clone(),
            _target: request.target.clone(),
            _grant: self.grant,
            _stage: self.stage,
        })
    }
}

enum CatalogFailureOrSuccess {
    Success,
    Incomplete,
    Refused,
}

#[path = "torch_offline_selection.rs"]
mod offline_selection;

#[cfg(all(test, target_os = "linux"))]
#[path = "torch_catalog_owner_tests.rs"]
mod tests;
