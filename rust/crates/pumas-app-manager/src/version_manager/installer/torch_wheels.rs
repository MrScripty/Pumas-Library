//! Torch wheels fetched through the shared, resumable acquisition owner.
//!
//! The Torch resolver decides versions and publishes each wheel's official URL
//! and index SHA-256. This module downloads that exact set with the same
//! acquisition lifecycle llama.cpp archives use: transfers retry and resume
//! from retained bytes, and every file is verified against its SHA-256 before
//! the installer may read it. The verified use lease stays held while pip
//! installs from the local files, then settles.
//!
//! Retention: a workspace is named by its exact wheel set. A failed transfer
//! keeps its partial bytes so the next install resumes them; a failed install
//! keeps its verified wheels for the next attempt. Success and cancellation
//! remove the workspace. Once an attempt knows its whole wheel closure, it
//! removes settled workspaces it does not use; unfinished transfers stay.

use super::*;
use pumas_library::acquisition::{
    AcquiredArtifactUse, AcquisitionHost, AcquisitionPhase, AcquisitionRecord, ArtifactFile,
    ArtifactManifest, ArtifactRevisionEvidence, ArtifactSourceIdentity,
    FileVerificationRequirement, HttpAttemptHost, RevisionStrength, Sha256Evidence,
};
use tokio::sync::oneshot;

/// Acquisition consumer scope that owns Torch runtime wheels.
pub(crate) const TORCH_WHEEL_CONSUMER: &str = "runtime.torch";
const WHEEL_SET_PREFIX: &str = "set-";
const WHEEL_SET_DIGEST_CHARS: usize = 32;
const DEMAND_PREFIX: &str = "wheels";
const PROGRESS_INTERVAL: std::time::Duration = std::time::Duration::from_millis(250);

/// A wheel connection that delivers no bytes for this long is treated as
/// lost, so the acquisition retries and resumes from the bytes it has. It
/// bounds silence, not transfer time: a slow link that keeps sending finishes.
pub(super) const WHEEL_TRANSFER_STALL_TIMEOUT: std::time::Duration =
    std::time::Duration::from_secs(60);

/// The HTTP client for wheel transfers. `stall` is the longest silence a
/// connection may show before the attempt fails and is retried.
pub(super) fn wheel_http_client(stall: std::time::Duration) -> reqwest::Result<reqwest::Client> {
    reqwest::Client::builder()
        .connect_timeout(InstallationConfig::URL_FETCH_TIMEOUT)
        .read_timeout(stall)
        .user_agent("pumas-library")
        .build()
}

/// One wheel the resolver locked, with the SHA-256 its index published.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct LockedWheel {
    pub(super) name: String,
    pub(super) url: String,
    pub(super) sha256: String,
    pub(super) filename: String,
}

impl LockedWheel {
    /// A refusal names why this wheel cannot use resumable acquisition.
    pub(super) fn new(name: &str, url: &str, sha256: &str) -> std::result::Result<Self, String> {
        if sha256.len() != 64 || !sha256.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(format!(
                "{name} has no SHA-256 to verify and resume its download"
            ));
        }
        let filename = wheel_filename(url)
            .ok_or_else(|| format!("{name} has no plain wheel file name in its URL"))?;
        Ok(Self {
            name: name.to_owned(),
            url: url.to_owned(),
            sha256: sha256.to_ascii_lowercase(),
            filename,
        })
    }
}

/// The decoded wheel file name the last URL path segment names.
pub(super) fn wheel_filename(url: &str) -> Option<String> {
    let url = reqwest::Url::parse(url).ok()?;
    let segment = url.path_segments()?.next_back()?;
    let name = String::from_utf8(percent_decode(segment)?).ok()?;
    (name.len() <= 255
        && name.ends_with(".whl")
        && !name.starts_with('.')
        && !name
            .chars()
            .any(|character| character.is_control() || matches!(character, '/' | '\\' | ':')))
    .then_some(name)
}

fn percent_decode(text: &str) -> Option<Vec<u8>> {
    let bytes = text.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            let pair = bytes.get(index + 1..index + 3)?;
            if !pair.iter().all(u8::is_ascii_hexdigit) {
                return None;
            }
            decoded.push(u8::from_str_radix(std::str::from_utf8(pair).ok()?, 16).ok()?);
            index += 3;
        } else {
            decoded.push(bytes[index]);
            index += 1;
        }
    }
    Some(decoded)
}

/// The workspace name and exact manifest for one wheel set. The name is
/// derived from every file name and SHA-256, so a different resolution never
/// shares (or blocks) another set's retained bytes.
fn wheel_set(wheels: &[LockedWheel]) -> std::result::Result<(String, ArtifactManifest), String> {
    if wheels.is_empty() {
        return Err("the resolution selected no wheels".into());
    }
    let mut entries: Vec<String> = wheels
        .iter()
        .map(|wheel| format!("{}\0{}\n", wheel.filename, wheel.sha256))
        .collect();
    entries.sort();
    let digest = format!("{:x}", Sha256::digest(entries.concat().as_bytes()));
    let key = format!("{WHEEL_SET_PREFIX}{}", &digest[..WHEEL_SET_DIGEST_CHARS]);
    let invalid = |error: pumas_library::acquisition::ManifestValidationError| {
        format!("the wheel set is not a valid acquisition selection: {error}")
    };
    let source = ArtifactSourceIdentity::new(
        "python-wheels",
        "pumas.torch.runtime",
        ArtifactRevisionEvidence::new("pumas.wheel_set.sha256", digest, RevisionStrength::Weak)
            .map_err(invalid)?,
    )
    .map_err(invalid)?;
    let files = wheels
        .iter()
        .map(|wheel| {
            ArtifactFile::new(
                wheel.filename.clone(),
                format!("{}@sha256:{}", wheel.name, wheel.sha256),
                None,
                Some(
                    Sha256Evidence::new("python.package_index.sha256", wheel.sha256.clone())
                        .map_err(invalid)?,
                ),
                FileVerificationRequirement::Sha256,
            )
            .map_err(invalid)
        })
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok((key, ArtifactManifest::new(source, files).map_err(invalid)?))
}

fn demand_generation(operation: &str, key: &str) -> Option<u64> {
    operation
        .strip_prefix(DEMAND_PREFIX)?
        .strip_prefix(':')?
        .strip_prefix(key)?
        .strip_prefix(':')?
        .parse()
        .ok()
}

fn is_settled(record: &AcquisitionRecord) -> bool {
    matches!(
        record.phase,
        AcquisitionPhase::Adopted { .. } | AcquisitionPhase::Withdrawn
    )
}

/// What the installer did with a held wheel set.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum WheelUse {
    /// The installation read the wheels; settle the use and remove them.
    Consumed,
    /// The attempt failed; release the use and keep the verified wheels so the
    /// next attempt reuses them without downloading.
    Retain,
    /// The attempt was cancelled or superseded; release the use and remove them.
    Discard,
}

/// Shared acquisition authority for one Torch install attempt.
pub(super) struct TorchWheelAcquisition {
    pub(super) consumer: Arc<AcquisitionConsumer>,
    pub(super) root: PathBuf,
    /// The Torch versions lock held by this attempt; it serializes every
    /// writer of the wheel root.
    pub(super) lease: Arc<dyn Send + Sync>,
    pub(super) cancel_flag: Arc<AtomicBool>,
    pub(super) shutdown_flag: Arc<AtomicBool>,
    pub(super) progress: Arc<RwLock<InstallationProgressTracker>>,
    pub(super) log_path: PathBuf,
    pub(super) client: reqwest::Client,
    pub(super) retry: AcquisitionRetryPolicy,
}

impl TorchWheelAcquisition {
    /// Download and verify `wheels`, then hold their verified use until
    /// [`HeldWheels::finish`]. `Ok(Err(reason))` means shared acquisition
    /// cannot serve this set and nothing was transferred; the caller keeps
    /// the existing pip download and reports the reason. A transfer failure is
    /// an error, with partial bytes kept so a retry resumes them.
    ///
    /// With `collect_settled`, settled workspaces of other sets are removed
    /// first. Pass it only once the attempt's whole wheel closure is known, so
    /// a set the attempt is about to reuse is never removed.
    pub(super) async fn hold(
        &self,
        wheels: Vec<LockedWheel>,
        collect_settled: bool,
    ) -> Result<std::result::Result<HeldWheels, String>> {
        // One canonical order for the workspace key, manifest and sources, so
        // the same set in another order resumes the same durable demand.
        let mut wheels = wheels;
        wheels.sort_by(|left, right| {
            (&left.filename, &left.sha256).cmp(&(&right.filename, &right.sha256))
        });
        let (key, manifest) = match wheel_set(&wheels) {
            Ok(selection) => selection,
            Err(reason) => return Ok(Err(reason)),
        };
        let root = self.root.clone();
        let lease = self.lease.clone();
        let relative = key.clone();
        let grant = self
            .consumer
            .run_blocking("reserve Torch wheel workspace", move || {
                let directory = root.join(&relative);
                match std::fs::create_dir(&directory) {
                    Ok(()) => {}
                    Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                    Err(error) => return Err(PumasError::io_with_path(error, &directory)),
                }
                ReservedDirectory::capture(&root, Path::new(&relative), lease, || Ok(()))
            })
            .await?;
        let workspace_identity = grant.workspace_identity().clone();
        let records = self.consumer.retained_acquisitions().await?;
        if collect_settled {
            self.collect_garbage(&records, &key).await;
        }

        let mut on_workspace: Vec<_> = records
            .into_iter()
            .filter(|record| record.workspace == workspace_identity)
            .collect();
        for record in on_workspace
            .iter()
            .filter(|record| matches!(record.phase, AcquisitionPhase::Using { .. }))
        {
            if let Err(error) = self.release_interrupted_use(record, &grant).await {
                return Ok(Err(format!(
                    "a wheel use left by an interrupted install could not be released: {error}"
                )));
            }
        }
        if on_workspace
            .iter()
            .any(|record| matches!(record.phase, AcquisitionPhase::Using { .. }))
        {
            on_workspace = self
                .consumer
                .retained_acquisitions()
                .await?
                .into_iter()
                .filter(|record| record.workspace == workspace_identity)
                .collect();
        }
        let resumable = on_workspace.iter().find(|record| {
            matches!(
                record.phase,
                AcquisitionPhase::Transferring | AcquisitionPhase::FilesReady
            )
        });
        let demand = match resumable {
            Some(record) if record.manifest == manifest => record.demand.clone(),
            Some(_) => {
                return Ok(Err(
                    "the wheel workspace holds an unfinished download of another selection".into(),
                ))
            }
            None => {
                let generation = on_workspace
                    .iter()
                    .filter_map(|record| demand_generation(&record.demand.operation, &key))
                    .max()
                    .unwrap_or(0)
                    + 1;
                AcquisitionDemand {
                    consumer: self.consumer.owner().to_owned(),
                    operation: format!("{DEMAND_PREFIX}:{key}:{generation}"),
                }
            }
        };
        let workspace = grant.acquisition_workspace()?;
        let directory = self.root.join(&key);
        let host = TorchWheelHost {
            sources: wheels.iter().map(|wheel| wheel.url.clone()).collect(),
            current: 0,
            cancel_flag: self.cancel_flag.clone(),
            shutdown_flag: self.shutdown_flag.clone(),
            progress: self.progress.clone(),
            log_path: self.log_path.clone(),
            cancelled_grant: Some(grant.clone()),
            last_report: None,
        };
        let request = AcquisitionHttpRequest {
            demand,
            manifest,
            workspace,
            sources: wheels
                .iter()
                .map(|wheel| AcquisitionHttpSource {
                    url: wheel.url.clone(),
                    authorization: None,
                })
                .collect(),
            retry: self.retry.clone(),
        };
        let (ready_tx, ready_rx) = oneshot::channel::<()>();
        let (done_tx, done_rx) = oneshot::channel::<WheelUse>();
        let released = Arc::new(AtomicBool::new(false));
        let released_by_use = released.clone();
        let discard_grant = grant.clone();
        let consumer = self.consumer.clone();
        let client = self.client.clone();
        // The task is joined by `HeldWheels::finish`, or here when transfer
        // ends before handoff. Consumer shutdown drains it in every case.
        let task = tokio::spawn(async move {
            consumer
                .acquire_http(
                    request,
                    client,
                    Box::new(host),
                    move |use_set: AcquiredArtifactUse| async move {
                        let _ = ready_tx.send(());
                        // A dropped holder is an abandoned attempt: keep the inputs.
                        let outcome = done_rx.await.unwrap_or(WheelUse::Retain);
                        let cleanup: Box<dyn FnOnce() -> Result<()> + Send> = match outcome {
                            WheelUse::Consumed => {
                                return Ok((
                                    (),
                                    serde_json::json!({
                                        "schema_version": 1,
                                        "use": "torch-runtime-installation",
                                    }),
                                ))
                            }
                            // The installer's output lives in its disposable
                            // stage, which is never published after a failed
                            // or cancelled attempt; only the inputs remain.
                            WheelUse::Retain => Box::new(|| Ok(())),
                            WheelUse::Discard => Box::new(move || discard_grant.clear_contents()),
                        };
                        use_set.withdraw_after_cleanup(cleanup).await?;
                        released_by_use.store(true, Ordering::SeqCst);
                        Err(PumasError::Other("Torch wheel use released".into()))
                    },
                    |(), _receipt| async { Ok(()) },
                )
                .await
        });
        match ready_rx.await {
            Ok(()) => Ok(Ok(HeldWheels {
                directory,
                grant: Some(grant),
                consumer: self.consumer.clone(),
                done: Some(done_tx),
                released,
                task: Some(task),
            })),
            Err(_) => {
                let outcome = task.await.map_err(|error| {
                    PumasError::Other(format!("Torch wheel acquisition task failed: {error}"))
                })?;
                let error = match outcome {
                    Err(error) => error,
                    Ok(()) => {
                        return Err(PumasError::Other(
                            "Torch wheel acquisition ended without handing off its files".into(),
                        ))
                    }
                };
                let shutting_down = self.shutdown_flag.load(Ordering::SeqCst);
                if shutting_down || self.cancel_flag.load(Ordering::SeqCst) {
                    // A user cancel discards; shutdown pauses and keeps bytes.
                    if !shutting_down && matches!(error, PumasError::DownloadCancelled) {
                        remove_workspace(&self.consumer, grant).await;
                    }
                    return Err(VersionInstaller::cancellation_error());
                }
                Err(PumasError::InstallationFailed {
                    message: format!(
                        "Torch wheel download failed; downloaded bytes are kept so the next \
                         install resumes them: {error}"
                    ),
                })
            }
        }
    }

    /// An interrupted install cannot have published output: its stage is
    /// disposable and reclaimed before any new Torch install. A receipted use
    /// is settled; an unreceipted one is withdrawn, keeping its verified inputs.
    async fn release_interrupted_use(
        &self,
        record: &AcquisitionRecord,
        grant: &ReservedDirectory,
    ) -> Result<()> {
        let workspace = grant.acquisition_workspace()?;
        let receipt_consumer = self.consumer.clone();
        let retained = record.clone();
        let receipt = self
            .consumer
            .run_blocking("read retained Torch wheel receipt", move || {
                receipt_consumer.completion_receipt(&retained)
            })
            .await?;
        if receipt.is_some() {
            self.consumer
                .reconcile(
                    record.demand.clone(),
                    record.manifest.clone(),
                    workspace,
                    |_receipt, _use_set| async { Ok(()) },
                )
                .await?;
        } else {
            self.consumer
                .withdraw_unreceipted_use(
                    record.demand.clone(),
                    record.manifest.clone(),
                    workspace,
                    || Ok(()),
                )
                .await?;
        }
        Ok(())
    }

    /// Remove settled workspaces this attempt does not use. Unfinished
    /// transfers and held uses stay; a failure is reported and skipped.
    async fn collect_garbage(&self, records: &[AcquisitionRecord], keep: &str) {
        let root = self.root.clone();
        let lease = self.lease.clone();
        let keep = keep.to_owned();
        let records = records.to_vec();
        let result = self
            .consumer
            .run_blocking("remove settled Torch wheel workspaces", move || {
                let mut failures = Vec::new();
                let entries = std::fs::read_dir(&root)
                    .map_err(|error| PumasError::io_with_path(error, &root))?;
                for entry in entries {
                    let entry = entry.map_err(|error| PumasError::io_with_path(error, &root))?;
                    let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
                        continue;
                    };
                    let owned_name = name.strip_prefix(WHEEL_SET_PREFIX).is_some_and(|digest| {
                        digest.len() == WHEEL_SET_DIGEST_CHARS
                            && digest.bytes().all(|byte| byte.is_ascii_hexdigit())
                    });
                    if !owned_name
                        || name == keep
                        || !entry.file_type().is_ok_and(|kind| kind.is_dir())
                    {
                        continue;
                    }
                    let removal = (|| {
                        let identity = AcquisitionWorkspace::identity_for_reserved_directory(
                            &root,
                            Path::new(&name),
                        )?;
                        if records
                            .iter()
                            .any(|record| record.workspace == identity && !is_settled(record))
                        {
                            return Ok(());
                        }
                        let grant = ReservedDirectory::capture(
                            &root,
                            Path::new(&name),
                            lease.clone(),
                            || Ok(()),
                        )?;
                        grant.clear_contents()?;
                        grant.remove_empty()
                    })();
                    if let Err(error) = removal {
                        failures.push(format!("{name}: {error}"));
                    }
                }
                Ok(failures)
            })
            .await;
        match result {
            Ok(failures) if failures.is_empty() => {}
            Ok(failures) => warn!(
                failures = %failures.join("; "),
                "Settled Torch wheel workspaces were retained"
            ),
            Err(error) => warn!(%error, "Torch wheel workspace cleanup was skipped"),
        }
    }
}

async fn remove_workspace(consumer: &AcquisitionConsumer, grant: ReservedDirectory) {
    let result = consumer
        .run_blocking("remove Torch wheel workspace", move || {
            grant.clear_contents()?;
            grant.remove_empty()
        })
        .await;
    if let Err(error) = result {
        warn!(%error, "Torch wheel workspace was retained for later cleanup");
    }
}

/// A verified wheel set whose use lease is held until [`Self::finish`].
pub(super) struct HeldWheels {
    directory: PathBuf,
    grant: Option<ReservedDirectory>,
    consumer: Arc<AcquisitionConsumer>,
    done: Option<oneshot::Sender<WheelUse>>,
    released: Arc<AtomicBool>,
    task: Option<tokio::task::JoinHandle<Result<()>>>,
}

impl HeldWheels {
    /// The directory holding every verified wheel of this set by file name.
    pub(super) fn directory(&self) -> &Path {
        &self.directory
    }

    pub(super) fn file(&self, wheel: &LockedWheel) -> PathBuf {
        self.directory.join(&wheel.filename)
    }

    /// Settle or release the held use, then apply the retention policy.
    pub(super) async fn finish(mut self, outcome: WheelUse) -> Result<()> {
        if let Some(done) = self.done.take() {
            let _ = done.send(outcome);
        }
        let task = self
            .task
            .take()
            .ok_or_else(|| PumasError::Other("Torch wheel use was already settled".into()))?;
        let result = task.await.map_err(|error| {
            PumasError::Other(format!("Torch wheel acquisition task failed: {error}"))
        })?;
        match (outcome, result) {
            (WheelUse::Consumed, Ok(())) => {}
            (WheelUse::Retain | WheelUse::Discard, Err(_))
                if self.released.load(Ordering::SeqCst) => {}
            (_, Err(error)) => return Err(error),
            (_, Ok(())) => {
                return Err(PumasError::Other(
                    "Torch wheel use settled instead of being released".into(),
                ))
            }
        }
        if outcome != WheelUse::Retain {
            if let Some(grant) = self.grant.take() {
                remove_workspace(&self.consumer, grant).await;
            }
        }
        Ok(())
    }
}

struct TorchWheelHost {
    sources: Vec<String>,
    current: usize,
    cancel_flag: Arc<AtomicBool>,
    shutdown_flag: Arc<AtomicBool>,
    progress: Arc<RwLock<InstallationProgressTracker>>,
    log_path: PathBuf,
    cancelled_grant: Option<ReservedDirectory>,
    last_report: Option<Instant>,
}

impl TorchWheelHost {
    fn source(&self) -> Option<&str> {
        self.sources.get(self.current).map(String::as_str)
    }

    fn log(&self, line: &str) {
        let written = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.log_path)
            .and_then(|mut log| {
                std::io::Write::write_all(&mut log, format!("{line}\n").as_bytes())
            });
        if let Err(error) = written {
            warn!(%error, "Torch wheel download note was not written to the install log");
        }
    }
}

#[async_trait::async_trait]
impl HttpAttemptHost for TorchWheelHost {
    async fn pause_requested(&self) {
        while !self.cancel_flag.load(Ordering::SeqCst) && !self.shutdown_flag.load(Ordering::SeqCst)
        {
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
    }

    /// Shutdown pauses: the transfer stays resumable for the next start.
    fn pause_requested_now(&self) -> bool {
        self.shutdown_flag.load(Ordering::SeqCst)
    }

    /// Shutdown also sets the cancel flag; it takes precedence so the
    /// transfer pauses and keeps its bytes instead of discarding them.
    fn cancel_requested(&self) -> bool {
        self.cancel_flag.load(Ordering::SeqCst) && !self.shutdown_flag.load(Ordering::SeqCst)
    }

    async fn record_progress(&mut self, downloaded_for_file: u64) -> Result<()> {
        if self
            .last_report
            .is_some_and(|reported| reported.elapsed() < PROGRESS_INTERVAL)
        {
            return Ok(());
        }
        self.last_report = Some(Instant::now());
        let source = self.source().map(str::to_owned);
        self.progress.write().await.update_network_transfer(
            source.as_deref(),
            true,
            downloaded_for_file,
            None,
            None,
            true,
        );
        Ok(())
    }
}

#[async_trait::async_trait]
impl AcquisitionHost for TorchWheelHost {
    fn cancelled_transfer_cleanup(&mut self) -> Option<Box<dyn FnOnce() -> Result<()> + Send>> {
        self.cancelled_grant.take().map(|grant| {
            Box::new(move || grant.clear_contents()) as Box<dyn FnOnce() -> Result<()> + Send>
        })
    }

    fn file_started(&mut self, index: usize) {
        self.current = index;
        self.last_report = None;
    }

    fn file_acquired(&mut self, _index: usize, bytes: u64) {
        let source = self.source().map(str::to_owned);
        if let Ok(mut progress) = self.progress.try_write() {
            progress.update_network_transfer(
                source.as_deref(),
                false,
                bytes,
                Some(bytes),
                None,
                true,
            );
        }
    }

    async fn retry(
        &mut self,
        attempt: u32,
        _delay: Option<std::time::Duration>,
        error: Option<&str>,
    ) -> Result<()> {
        if let Some(error) = error {
            self.log(&format!(
                "Torch wheel download attempt {attempt} for {} failed: {error}; \
                 retrying from the bytes already received",
                self.source().unwrap_or("a wheel")
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "torch_wheels_tests.rs"]
mod tests;
