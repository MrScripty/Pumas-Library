//! Durable acquisition custody shared by source adapters and consumers.
use super::http::{
    open_http_artifact, stream_http_artifact, HttpArtifactSink, HttpAttemptHost, HttpBodyOutcome,
    HttpResumeEvidence,
};
use super::store::AcquisitionStore;
use super::task_custody::{TaskContext, TaskCustodyOwner};
use super::workspace::{
    write_chunk, AcquisitionWorkspace, PartialPrefix, VerifiedFile, WorkspaceCheckpointOwner,
    WorkspaceIdentity,
};
use super::ArtifactManifest;
use crate::{PumasError, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::future::Future;
use std::io::{self, Write};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use uuid::Uuid;

// New consumer bindings reserve space for the copied-output receipt: embedding
// at most doubles pretty JSON indentation; namespace entries include physical
// identities, sizes and hashes. The model publication schema contract test
// checks these reserves against the real version-2 receipt and its 16 MiB limit.
const CONSUMER_DOCUMENT_MAX_BYTES: usize = 4 * 1024 * 1024;
const CONSUMER_NAMESPACE_MAX_BYTES: usize = 2 * 1024 * 1024;
const CONSUMER_NAMESPACE_ENTRY_BYTES: usize = 512;

struct JsonSize {
    bytes: usize,
    limit: usize,
}

impl Write for JsonSize {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let next = self
            .bytes
            .checked_add(bytes.len())
            .filter(|next| *next <= self.limit)
            .ok_or_else(|| io::Error::other("Consumer JSON exceeds its byte budget"))?;
        self.bytes = next;
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn consumer_json_size<T: Serialize + ?Sized>(
    value: &T,
    limit: usize,
    field: &str,
) -> Result<usize> {
    let mut size = JsonSize { bytes: 0, limit };
    serde_json::to_writer_pretty(&mut size, value).map_err(|error| PumasError::Validation {
        field: field.into(),
        message: format!("Consumer serialization must fit {limit} bytes: {error}"),
    })?;
    Ok(size.bytes)
}

fn require_consumer_manifest_bound(manifest: &ArtifactManifest) -> Result<()> {
    consumer_json_size(
        manifest,
        CONSUMER_DOCUMENT_MAX_BYTES,
        "acquisition.consumer_document_size",
    )?;
    let mut parents = BTreeSet::new();
    let mut namespace_bytes = 0_usize;
    let mut charge = |path: &str| -> Result<()> {
        let name_bytes = consumer_json_size(
            path,
            CONSUMER_NAMESPACE_MAX_BYTES,
            "acquisition.consumer_namespace_size",
        )?;
        namespace_bytes = namespace_bytes
            .checked_add(name_bytes)
            .and_then(|bytes| bytes.checked_add(CONSUMER_NAMESPACE_ENTRY_BYTES))
            .filter(|bytes| *bytes <= CONSUMER_NAMESPACE_MAX_BYTES)
            .ok_or_else(|| PumasError::Validation {
                field: "acquisition.consumer_namespace_size".into(),
                message: "Consumer payload namespace exceeds its serialized proof budget".into(),
            })?;
        Ok(())
    };
    for file in manifest.files() {
        charge(file.logical_path())?;
        for (separator, _) in file.logical_path().match_indices('/') {
            let parent = &file.logical_path()[..separator];
            if parents.insert(parent) {
                charge(parent)?;
            }
        }
    }
    Ok(())
}

/// Consumer identity and exact demand operation; neither authorizes file access.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AcquisitionDemand {
    pub consumer: String,
    pub operation: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
pub enum AcquisitionPhase {
    Transferring,
    FilesReady,
    Using {
        #[serde(with = "uuid_wire")]
        lease: Uuid,
    },
    Adopted {
        #[serde(with = "uuid_wire")]
        lease: Uuid,
    },
    Withdrawn,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AcquisitionRecord {
    #[serde(with = "uuid_wire")]
    pub id: Uuid,
    pub demand: AcquisitionDemand,
    pub manifest: ArtifactManifest,
    pub workspace: WorkspaceIdentity,
    pub phase: AcquisitionPhase,
    pub files: Vec<VerifiedFile>,
}

/// Versioned consumer-owned completion data, paired with the exact acquisition
/// record and use lease that authorized publication.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AcquisitionConsumerReceipt {
    pub receipt_kind: String,
    pub receipt_version: u32,
    pub owner: String,
    pub acquisition_id: String,
    pub use_lease: String,
    pub demand: AcquisitionDemand,
    pub manifest: ArtifactManifest,
    pub workspace: WorkspaceIdentity,
    pub verified_files: Vec<VerifiedFile>,
    pub payload: Value,
}

impl AcquisitionConsumerReceipt {
    const KIND: &'static str = "pumas.consumer-completion";

    fn for_record(record: &AcquisitionRecord, lease: Uuid, payload: Value) -> Result<Self> {
        let receipt = Self {
            receipt_kind: Self::KIND.into(),
            receipt_version: 1,
            owner: record.demand.consumer.clone(),
            acquisition_id: record.id.to_string(),
            use_lease: lease.to_string(),
            demand: record.demand.clone(),
            manifest: record.manifest.clone(),
            workspace: record.workspace.clone(),
            verified_files: record.files.clone(),
            payload,
        };
        receipt.validate_for_record(record, lease)?;
        Ok(receipt)
    }

    pub(crate) fn validate_for_record(
        &self,
        record: &AcquisitionRecord,
        lease: Uuid,
    ) -> Result<()> {
        if self.receipt_kind != Self::KIND
            || self.receipt_version != 1
            || self.owner != record.demand.consumer
            || self.acquisition_id != record.id.to_string()
            || self.use_lease != lease.to_string()
            || self.demand != record.demand
            || self.manifest != record.manifest
            || self.workspace != record.workspace
            || self.verified_files != record.files
        {
            return Err(invalid(
                "Consumer completion receipt does not bind the exact acquisition use",
            ));
        }
        Ok(())
    }
}

/// Ephemeral location and optional authorization for one manifest file. These
/// values are used for the request only and are never persisted.
#[derive(Clone, Default)]
pub struct AcquisitionHttpSource {
    pub url: String,
    pub authorization: Option<String>,
}

/// Request data for the shared HTTP acquisition lifecycle.
#[derive(Clone)]
pub struct AcquisitionHttpRequest {
    pub demand: AcquisitionDemand,
    pub manifest: ArtifactManifest,
    pub workspace: AcquisitionWorkspace,
    pub sources: Vec<AcquisitionHttpSource>,
    pub retry: AcquisitionRetryPolicy,
}

/// One exact versioned S3 selection; access remains ephemeral. Retry budgets
/// must have positive finite attempt and elapsed limits.
#[cfg(feature = "s3")]
pub struct AcquisitionS3Request {
    pub demand: AcquisitionDemand,
    pub selection: super::S3ObjectSelection,
    pub workspace: AcquisitionWorkspace,
    pub retry: AcquisitionRetryPolicy,
}

/// A complete explicit pinned S3 set. Retry limits apply to each object's
/// transfer; all members verify before the shared owner grants consumer use.
#[cfg(feature = "s3")]
pub struct AcquisitionS3ManifestRequest {
    pub demand: AcquisitionDemand,
    pub selection: super::S3ManifestSelection,
    pub workspace: AcquisitionWorkspace,
    pub retry: AcquisitionRetryPolicy,
}

enum AcquisitionSource {
    Http {
        client: super::AcquisitionHttpClient,
        source: AcquisitionHttpSource,
    },
    #[cfg(feature = "s3")]
    S3(super::S3ObjectSelection),
}

impl AcquisitionSource {
    fn request_identity(&self) -> String {
        match self {
            Self::Http { source, .. } => source.url.clone(),
            #[cfg(feature = "s3")]
            Self::S3(selection) => selection.acquisition_identity(),
        }
    }

    fn transfer_deadline(
        &self,
        _retry: &AcquisitionRetryPolicy,
    ) -> Result<Option<tokio::time::Instant>> {
        match self {
            Self::Http { .. } => http_transfer_deadline(_retry.elapsed),
            #[cfg(feature = "s3")]
            Self::S3(_) => tokio::time::Instant::now()
                .checked_add(_retry.elapsed)
                .map(Some)
                .ok_or_else(|| invalid("S3 elapsed budget exceeds the supported clock range")),
        }
    }

    async fn open(
        &self,
        manifest: &ArtifactManifest,
        file_index: usize,
        resume: u64,
        continuation: Option<&HttpResumeEvidence>,
        _deadline: Option<tokio::time::Instant>,
    ) -> Result<super::http::HttpArtifactResponse> {
        match self {
            Self::Http { client, source } => {
                let request = open_http_artifact(
                    client,
                    &source.url,
                    manifest,
                    file_index,
                    resume,
                    source.authorization.as_deref(),
                    continuation,
                );
                if let Some(deadline) = _deadline {
                    tokio::select! {
                        biased;
                        _ = http_budget_elapsed(deadline) => Err(http_budget_timeout()),
                        response = request => response,
                    }
                } else {
                    request.await
                }
            }
            #[cfg(feature = "s3")]
            Self::S3(selection) => {
                selection
                    .open_acquisition(resume, continuation, _deadline)
                    .await
            }
        }
    }
}

// Zero is the existing opt-out used by the native consumer. A positive
// budget starts once per selected file and is never reset by a retry. Local
// verification and registered effect drainage remain awaited independently.
fn http_transfer_deadline(elapsed: Duration) -> Result<Option<tokio::time::Instant>> {
    if elapsed.is_zero() {
        return Ok(None);
    }
    tokio::time::Instant::now()
        .checked_add(elapsed)
        .map(Some)
        .ok_or_else(|| invalid("HTTP elapsed budget exceeds the supported clock range"))
}

// Sleep may yield to Tokio cooperative scheduling even when its timer has
// expired. Check the clock on every poll before touching a source/body future.
async fn http_budget_elapsed(deadline: tokio::time::Instant) {
    let sleep = tokio::time::sleep_until(deadline);
    tokio::pin!(sleep);
    futures::future::poll_fn(|cx| {
        if tokio::time::Instant::now() >= deadline {
            std::task::Poll::Ready(())
        } else {
            sleep.as_mut().poll(cx)
        }
    })
    .await
}

fn http_budget_timeout() -> PumasError {
    PumasError::Network {
        message: "HTTP acquisition attempt exceeded its operation budget".into(),
        cause: None,
    }
}

struct AcquisitionRequest {
    demand: AcquisitionDemand,
    manifest: ArtifactManifest,
    workspace: AcquisitionWorkspace,
    sources: Vec<AcquisitionSource>,
    retry: AcquisitionRetryPolicy,
}

fn invalid(message: &str) -> PumasError {
    PumasError::Validation {
        field: "acquisition.custody".into(),
        message: message.into(),
    }
}

impl AcquisitionRecord {
    pub(crate) fn validate(&self, id: Uuid) -> Result<()> {
        if id != self.id
            || id.is_nil()
            || self.demand.consumer.is_empty()
            || self.demand.operation.is_empty()
            || self.workspace.root_identity.is_empty()
            || self.workspace.relative_target.is_empty()
        {
            return Err(invalid(
                "Acquisition identity is incomplete or inconsistent",
            ));
        }
        if matches!(
            self.phase,
            AcquisitionPhase::FilesReady
                | AcquisitionPhase::Using { .. }
                | AcquisitionPhase::Adopted { .. }
        ) {
            if self.files.len() != self.manifest.files().len() {
                return Err(invalid("Verified-file set is incomplete"));
            }
            for (receipt, selected) in self.files.iter().zip(self.manifest.files()) {
                if receipt.path != selected.logical_path()
                    || selected
                        .expected_size()
                        .is_some_and(|size| size != receipt.bytes)
                    || receipt.sha256.len() != 64
                    || !receipt
                        .sha256
                        .bytes()
                        .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
                    || selected
                        .expected_sha256()
                        .is_some_and(|digest| digest.value() != receipt.sha256)
                {
                    return Err(invalid("Verified-file receipt contradicts selection"));
                }
            }
        } else if !self.files.is_empty() {
            return Err(invalid(
                "Unverified custody cannot contain a verified-file set",
            ));
        }
        if matches!(self.phase, AcquisitionPhase::Using { lease } | AcquisitionPhase::Adopted { lease } if lease.is_nil())
        {
            return Err(invalid("Consumer lease identity is invalid"));
        }
        Ok(())
    }
}

/// Exact operation handle; it is not a filesystem capability.
#[derive(Clone)]
pub(crate) struct AcquisitionOperation {
    record: AcquisitionRecord,
    context: TaskContext,
}

impl AcquisitionOperation {
    pub(crate) fn is_adopted(&self) -> bool {
        matches!(self.record.phase, AcquisitionPhase::Adopted { .. })
    }
}

#[derive(Clone)]
pub(crate) enum AcquisitionReconciliation {
    Using,
    Adopted(Uuid),
}

/// Pins the held workspace while a consumer uses the verified set. Dropping an
/// unacknowledged lease leaves durable `Using` custody for explicit reconciliation.
pub(crate) struct AcquisitionUseLease {
    operation: AcquisitionOperation,
    workspace: AcquisitionWorkspace,
    lease: Uuid,
    context: TaskContext,
}

impl AcquisitionUseLease {
    pub(crate) fn record(&self) -> &AcquisitionRecord {
        &self.operation.record
    }
}

/// Cold observation of a retained `Using` lease. It proves only that the
/// historic selection and file receipts still match under the reopened
/// workspace; it cannot resume importer effects or renew the lease.
pub(crate) struct AcquisitionReopenProof {
    record: AcquisitionRecord,
    _workspace: AcquisitionWorkspace,
}

impl AcquisitionReopenProof {
    pub(crate) fn record(&self) -> &AcquisitionRecord {
        &self.record
    }
}

/// Source-neutral, exact runtime proofs. A retained row or workspace locator
/// cannot construct either proof or reopen unresolved consumer use.
pub(crate) struct AcquisitionUseProof(AcquisitionProof);
pub(crate) struct AcquisitionTransferProof(AcquisitionProof);

pub(crate) struct AcquisitionProof {
    context: TaskContext,
    store: Arc<AcquisitionStore>,
    record: AcquisitionRecord,
    workspace: AcquisitionWorkspace,
}

impl AcquisitionUseProof {
    pub(crate) fn into_proof(self) -> AcquisitionProof {
        self.0
    }
}
impl AcquisitionTransferProof {
    pub(crate) fn into_proof(self) -> AcquisitionProof {
        self.0
    }
}

impl AcquisitionProof {
    pub(crate) fn record(&self) -> &AcquisitionRecord {
        &self.record
    }

    pub(crate) fn validate_current(
        &self,
        store: &Arc<AcquisitionStore>,
        current: &std::collections::BTreeMap<Uuid, AcquisitionRecord>,
    ) -> Result<()> {
        if !self
            .context
            .is_current_role(super::task_custody::TaskRole::Worker)
            || !Arc::ptr_eq(&self.store, store)
            || current.get(&self.record.id) != Some(&self.record)
            || self.workspace.identity() != &self.record.workspace
        {
            return Err(invalid(
                "Acquisition effect proof is stale or belongs to another store/workspace",
            ));
        }
        self.validate_binding()
    }

    pub(crate) fn verify_receipts(&self) -> Result<()> {
        self.workspace
            .verify_receipts(&self.record.manifest, &self.record.files)
    }

    pub(crate) fn validate_binding(&self) -> Result<()> {
        self.workspace.validate()
    }
}

#[derive(Clone)]
pub struct AcquisitionRetryPolicy {
    pub attempts: Option<u32>,
    /// Per-file source-wait budget across headers, body, retries and backoff.
    /// HTTP zero preserves the legacy opt-out; S3 requires a positive value.
    /// Registered filesystem effects still drain after expiry, and consumer
    /// verification/publication/cleanup are not given this transfer deadline.
    pub elapsed: Duration,
    pub backoff: crate::network::RetryConfig,
}

#[async_trait::async_trait]
pub trait AcquisitionHost: HttpAttemptHost {
    /// Optional observation only: an exact manifest member is about to acquire.
    /// Default no-op preserves existing hosts and transfer/publication policy.
    fn file_started(&mut self, _index: usize) {}
    /// The member's verified staging acquisition returned these bytes. Complete
    /// set verification and consumer publication still follow separately.
    fn file_acquired(&mut self, _index: usize, _bytes: u64) {}

    async fn retry(
        &mut self,
        attempt: u32,
        delay: Option<Duration>,
        error: Option<&str>,
    ) -> Result<()>;
}

/// Verified manifest file use held inside one current acquisition worker.
/// The workspace capability and exact `Using` lease remain held until the
/// consumer callback returns and its completion receipt is atomically settled.
pub struct AcquiredArtifactUse {
    store: Arc<AcquisitionStore>,
    context: TaskContext,
    lease: AcquisitionUseLease,
}

impl AcquiredArtifactUse {
    /// A model publisher must observe the exact issued receipt before effects.
    /// Neither a serialized receipt nor a matching in-memory value grants use.
    pub(crate) async fn require_issued_receipt(
        &self,
        receipt: &AcquisitionConsumerReceipt,
    ) -> Result<()> {
        let store = self.store.clone();
        let expected = self.lease.record().clone();
        let receipt = receipt.clone();
        owned(
            &self.context,
            "require issued model consumer receipt",
            move || {
                let current = store
                    .acquisitions()?
                    .remove(&expected.id)
                    .ok_or_else(|| invalid("Model acquisition custody disappeared"))?;
                if current != expected
                    || store.consumer_receipt(expected.id)?.as_ref() != Some(&receipt)
                {
                    return Err(invalid(
                        "Model publication requires the exact current issued consumer receipt",
                    ));
                }
                Ok(())
            },
        )
        .await
    }

    /// Join registered effects, then durably revoke and reclaim consumer output.
    /// Only successful cleanup permits exact, receipt-free Using withdrawal.
    /// Cleanup must revoke this attempt before reclaiming it. Cleanup errors retain
    /// Using; withdrawal publication failures propagate durability uncertainty.
    pub async fn withdraw_after_cleanup(
        self,
        cleanup: impl FnOnce() -> Result<()> + Send + 'static,
    ) -> Result<()> {
        match self.context.drain_blocking().await {
            Ok(0) => {}
            result => {
                return Err(PumasError::Other(format!(
                    "Consumer effects unsettled before cancellation cleanup: {result:?}"
                )))
            }
        }
        let store = self.store.clone();
        let expected = self.lease.record().clone();
        owned(
            &self.context,
            "check exact unreceipted consumer use",
            move || store.require_unreceipted_use(&expected),
        )
        .await?;
        owned(
            &self.context,
            "revoke and clean cancelled consumer output",
            cleanup,
        )
        .await?;
        let store = self.store.clone();
        let expected = self.lease.record().clone();
        owned(
            &self.context,
            "withdraw exact cancelled consumer use",
            move || store.withdraw_unreceipted_use(&expected),
        )
        .await
    }

    pub fn record(&self) -> &AcquisitionRecord {
        self.lease.record()
    }

    /// Open a file only after rechecking the receipt through the held directory
    /// capability. The returned descriptor is read-only and no-follow.
    pub async fn open_file(&self, file_index: usize) -> Result<std::fs::File> {
        let record = self.lease.record();
        let selected = record
            .manifest
            .files()
            .get(file_index)
            .ok_or_else(|| invalid("Selected file is unavailable"))?
            .clone();
        let verified = record
            .files
            .get(file_index)
            .ok_or_else(|| invalid("Verified file receipt is unavailable"))?
            .clone();
        let workspace = self.lease.workspace.clone();
        owned(&self.context, "open verified consumer input", move || {
            workspace.open_verified_readonly(&selected, &verified)
        })
        .await
    }

    /// Run blocking consumer effects under the same custody worker. Dropping a
    /// waiter cannot detach the blocking effect from acquisition shutdown.
    pub async fn run_blocking<T: Send + 'static>(
        &self,
        name: &'static str,
        work: impl FnOnce() -> Result<T> + Send + 'static,
    ) -> Result<T> {
        let workspace = self.lease.workspace.clone();
        owned(&self.context, name, move || {
            // The effect can outlive its waiter and use handle. Keep the
            // execution reservation until this registered closure returns.
            let _held_workspace = workspace;
            work()
        })
        .await
    }
}

/// One durable lifecycle/store owner and the existing shared task supervisor.
/// Source access is ephemeral and is never written to the durable manifest.
pub struct AcquisitionService {
    store: Arc<AcquisitionStore>,
    supervisor: Arc<TaskCustodyOwner>,
    checkpoints: Arc<Mutex<BTreeMap<(Uuid, usize), WarmCheckpoint>>>,
}

/// Runtime evidence only: never decoded from persisted progress or filenames.
#[derive(Clone)]
struct WarmCheckpoint {
    record: AcquisitionRecord,
    owner: WorkspaceCheckpointOwner,
    request_url: String,
    resource: String,
    etag: String,
    total: Option<u64>,
    prefix: PartialPrefix,
}

const CHECKPOINT_METADATA_LIMIT: usize = 64 * 1024;

/// Measure the complete variable metadata before retaining any owned copies.
#[derive(Serialize)]
struct WarmCheckpointMetadata<'a> {
    record: &'a AcquisitionRecord,
    request_url: &'a str,
    resource: &'a str,
    etag: &'a str,
    total: Option<u64>,
    prefix: &'a PartialPrefix,
}

struct CappedMetadataWriter {
    remaining: usize,
}

impl std::io::Write for CappedMetadataWriter {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        if bytes.len() > self.remaining {
            return Err(std::io::Error::other("Checkpoint metadata limit exceeded"));
        }
        self.remaining -= bytes.len();
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

fn checkpoint_metadata_fits(metadata: &impl Serialize) -> bool {
    serde_json::to_writer(
        CappedMetadataWriter {
            remaining: CHECKPOINT_METADATA_LIMIT,
        },
        metadata,
    )
    .is_ok()
}

impl AcquisitionService {
    pub fn new(store: Arc<AcquisitionStore>) -> Self {
        Self {
            store,
            supervisor: Arc::new(TaskCustodyOwner::new()),
            checkpoints: Arc::new(Mutex::new(BTreeMap::new())),
        }
    }

    /// Construct the shared owner with explicit finite admission limits.
    /// Every consumer and HF scope shares these budgets. Saturation fails
    /// synchronously; terminal control work has separate blocking capacity.
    pub fn with_capacity(
        store: Arc<AcquisitionStore>,
        capacity: super::AcquisitionCapacity,
    ) -> Result<Self> {
        Ok(Self {
            store,
            supervisor: Arc::new(TaskCustodyOwner::with_capacity(capacity)?),
            checkpoints: Arc::new(Mutex::new(BTreeMap::new())),
        })
    }

    pub(crate) fn with_store(&self, store: Arc<AcquisitionStore>) -> Self {
        Self {
            store,
            supervisor: self.supervisor.clone(),
            checkpoints: self.checkpoints.clone(),
        }
    }

    fn lookup_checkpoint(&self, key: (Uuid, usize)) -> Result<Option<WarmCheckpoint>> {
        let mut checkpoints = self
            .checkpoints
            .lock()
            .map_err(|_| invalid("Checkpoint owner poisoned"))?;
        checkpoints.retain(|_, checkpoint| checkpoint.owner.is_live());
        Ok(checkpoints.get(&key).cloned())
    }

    fn retain_checkpoint(
        &self,
        key: (Uuid, usize),
        workspace: &AcquisitionWorkspace,
        metadata: WarmCheckpointMetadata<'_>,
    ) -> Result<bool> {
        let mut checkpoints = self
            .checkpoints
            .lock()
            .map_err(|_| invalid("Checkpoint owner poisoned"))?;
        checkpoints.retain(|_, checkpoint| checkpoint.owner.is_live());
        if (!checkpoints.contains_key(&key)
            && checkpoints.len() >= self.supervisor.checkpoint_limit())
            || !checkpoint_metadata_fits(&metadata)
        {
            return Ok(false);
        }
        checkpoints.insert(
            key,
            WarmCheckpoint {
                record: metadata.record.clone(),
                owner: workspace.checkpoint_owner(),
                request_url: metadata.request_url.to_owned(),
                resource: metadata.resource.to_owned(),
                etag: metadata.etag.to_owned(),
                total: metadata.total,
                prefix: metadata.prefix.clone(),
            },
        );
        Ok(true)
    }

    fn forget_checkpoint(&self, key: (Uuid, usize)) -> Result<()> {
        self.checkpoints
            .lock()
            .map_err(|_| invalid("Checkpoint owner poisoned"))?
            .remove(&key);
        Ok(())
    }

    pub fn store(&self) -> &Arc<AcquisitionStore> {
        &self.store
    }

    /// Open a narrow consumer scope on this service's one existing supervisor.
    /// The returned consumer never creates a second store or task owner.
    pub fn open_consumer(
        self: &Arc<Self>,
        owner: impl Into<String>,
    ) -> Result<AcquisitionConsumer> {
        let owner = owner.into();
        if owner.trim().is_empty() {
            return Err(invalid("Consumer identity is empty"));
        }
        let scope = self.supervisor.open_scope(|| async { Ok(()) })?;
        Ok(AcquisitionConsumer {
            service: self.clone(),
            scope,
            owner,
        })
    }

    /// Read a consumer-owned receipt without granting filesystem or settlement
    /// authority. Reopen callers must still validate their own durable outputs.
    pub fn consumer_receipt(&self, id: Uuid) -> Result<Option<AcquisitionConsumerReceipt>> {
        self.store.consumer_receipt(id)
    }
    pub(crate) fn supervisor(&self) -> Arc<TaskCustodyOwner> {
        self.supervisor.clone()
    }

    async fn settle_consumer_use(
        &self,
        context: &TaskContext,
        expected: &AcquisitionRecord,
        lease: Uuid,
        payload: Value,
    ) -> Result<()> {
        let receipt = AcquisitionConsumerReceipt::for_record(expected, lease, payload)?;
        let store = self.store.clone();
        let expected = expected.clone();
        owned(
            context,
            "settle exact consumer completion receipt",
            move || store.settle_consumer_use(&expected, lease, receipt),
        )
        .await
    }

    async fn issue_consumer_receipt(
        &self,
        context: &TaskContext,
        expected: &AcquisitionRecord,
        lease: Uuid,
        receipt: AcquisitionConsumerReceipt,
    ) -> Result<()> {
        let store = self.store.clone();
        let expected = expected.clone();
        owned(context, "issue consumer completion receipt", move || {
            consumer_json_size(
                &receipt,
                CONSUMER_DOCUMENT_MAX_BYTES,
                "acquisition.consumer_document_size",
            )?;
            store.issue_consumer_receipt(&expected, lease, &receipt)
        })
        .await
    }

    async fn settle_consumer_receipt(
        &self,
        context: &TaskContext,
        expected: &AcquisitionRecord,
        lease: Uuid,
        receipt: AcquisitionConsumerReceipt,
    ) -> Result<()> {
        let store = self.store.clone();
        let expected = expected.clone();
        owned(context, "settle issued consumer receipt", move || {
            store.settle_consumer_receipt(&expected, lease, &receipt)
        })
        .await
    }

    /// Global closure after every consumer has stopped admitting work. Narrow
    /// consumer shutdown must close that consumer's scope first.
    pub async fn shutdown(self: &Arc<Self>) -> Result<()> {
        self.supervisor.request_shutdown().wait().await?;
        self.checkpoints
            .lock()
            .map_err(|_| invalid("Checkpoint owner poisoned"))?
            .clear();
        Ok(())
    }

    pub(crate) fn use_proof(
        &self,
        context: &TaskContext,
        lease: &AcquisitionUseLease,
    ) -> Result<AcquisitionUseProof> {
        if !context.shares_scope(&lease.context)
            || !context.generation().matches(lease.context.generation())
            || !context.is_current_role(super::task_custody::TaskRole::Worker)
            || !matches!(lease.operation.record.phase, AcquisitionPhase::Using { lease: token } if token == lease.lease)
        {
            return Err(invalid(
                "Consumer effect requires its current verified use lease",
            ));
        }
        Ok(AcquisitionUseProof(AcquisitionProof {
            context: context.clone(),
            store: self.store.clone(),
            record: lease.operation.record.clone(),
            workspace: lease.workspace.clone(),
        }))
    }

    pub(crate) fn transfer_proof(
        &self,
        context: &TaskContext,
        operation: &AcquisitionOperation,
        workspace: &AcquisitionWorkspace,
    ) -> Result<AcquisitionTransferProof> {
        if !context.shares_scope(&operation.context)
            || !context.generation().matches(operation.context.generation())
            || !context.is_current_role(super::task_custody::TaskRole::Worker)
            || operation.record.phase != AcquisitionPhase::Transferring
        {
            return Err(invalid(
                "Transfer effect requires its current transferring operation",
            ));
        }
        Ok(AcquisitionTransferProof(AcquisitionProof {
            context: context.clone(),
            store: self.store.clone(),
            record: operation.record.clone(),
            workspace: workspace.clone(),
        }))
    }

    pub(crate) async fn require_schema(&self, context: &TaskContext) -> Result<()> {
        let store = self.store.clone();
        owned(
            context,
            "validate acquisition schema eligibility",
            move || store.require_acquisition_schema(),
        )
        .await
    }

    /// The owner calls this only after observing exact-operation cleanup and
    /// its registered effects. Unknown/Pending cleanup cannot reach this seam.
    pub(crate) async fn withdraw(
        &self,
        context: &TaskContext,
        demand: AcquisitionDemand,
        workspace: WorkspaceIdentity,
    ) -> Result<()> {
        self.checkpoints
            .lock()
            .map_err(|_| invalid("Checkpoint owner poisoned"))?
            .retain(|_, checkpoint| {
                checkpoint.record.demand != demand || checkpoint.record.workspace != workspace
            });
        let store = self.store.clone();
        owned(context, "withdraw cleaned acquisition demand", move || {
            store.update_acquisitions_if_changed(|records| {
                if let Some(record) = records.values_mut().find(|record| record.demand == demand) {
                    if record.workspace != workspace {
                        return Err(invalid("Withdrawal workspace does not match exact demand"));
                    }
                    if !matches!(
                        record.phase,
                        AcquisitionPhase::Adopted { .. } | AcquisitionPhase::Withdrawn
                    ) {
                        record.phase = AcquisitionPhase::Withdrawn;
                        record.files.clear();
                    }
                }
                Ok(())
            })
        })
        .await
    }

    pub(crate) async fn begin(
        &self,
        context: &TaskContext,
        demand: AcquisitionDemand,
        manifest: ArtifactManifest,
        workspace: WorkspaceIdentity,
        reconciliation: Option<AcquisitionReconciliation>,
    ) -> Result<AcquisitionOperation> {
        let store = self.store.clone();
        let origin = context.clone();
        owned(context, "admit durable acquisition", move || {
            store.update_acquisitions(|records| {
                if let Some(existing) = records.values().find(|record| record.demand == demand) {
                    if existing.manifest != manifest || existing.workspace != workspace { return Err(invalid("Exact acquisition demand changed selection or workspace")); }
                    if matches!(existing.phase, AcquisitionPhase::Using { .. }) {
                        return Err(PumasError::Validation {
                            field: "acquisition.consumer_recovery_required".into(),
                            message: "Retained consumer use requires its authoritative exact result; input custody alone cannot authorize repeating import".into(),
                        });
                    }
                    let reconciling = matches!((&existing.phase, &reconciliation),
                        (AcquisitionPhase::Adopted { lease }, Some(AcquisitionReconciliation::Adopted(expected))) if lease == expected);
                    if !matches!(existing.phase, AcquisitionPhase::Transferring | AcquisitionPhase::FilesReady) && !reconciling {
                        return Err(invalid("Acquisition demand has unresolved or terminal consumer custody"));
                    }
                    return Ok(AcquisitionOperation { record: existing.clone(), context: origin.clone() });
                }
                if records.values().any(|record| record.workspace == workspace && !matches!(record.phase, AcquisitionPhase::Adopted { .. } | AcquisitionPhase::Withdrawn)) {
                    return Err(PumasError::DownloadRootBusy);
                }
                let record = AcquisitionRecord { id: Uuid::new_v4(), demand, manifest, workspace, phase: AcquisitionPhase::Transferring, files: Vec::new() };
                record.validate(record.id)?;
                records.insert(record.id, record.clone());
                Ok(AcquisitionOperation { record, context: origin.clone() })
            })
        }).await
    }

    pub(crate) async fn verified_existing_file(
        &self,
        context: &TaskContext,
        operation: &AcquisitionOperation,
        workspace: &AcquisitionWorkspace,
        file_index: usize,
    ) -> Result<Option<u64>> {
        self.forget_checkpoint((operation.record.id, file_index))?;
        let selected = operation
            .record
            .manifest
            .files()
            .get(file_index)
            .ok_or_else(|| invalid("Selected file is unavailable"))?
            .clone();
        let grant = workspace.clone();
        owned(context, "inspect reusable acquisition file", move || {
            grant.prepare_file(&selected)?;
            let Some(size) = grant.file_len(selected.logical_path(), false)? else {
                return Ok(None);
            };
            if selected
                .expected_size()
                .is_some_and(|expected| expected != size)
            {
                return Err(invalid(
                    "Existing selected file size conflicts with selection",
                ));
            }
            if selected.expected_sha256().is_none() {
                return Ok(None);
            }
            let verified = grant.verify_file(&selected, false)?;
            grant.remove_part(selected.logical_path())?;
            Ok(Some(verified.bytes))
        })
        .await
    }

    /// The only byte attempt/retry/file-promotion path. The host projects
    /// progress and cancellation; it never writes artifact bytes or retries.
    #[allow(clippy::too_many_arguments)]
    pub(crate) async fn acquire_file(
        &self,
        context: &TaskContext,
        operation: &AcquisitionOperation,
        workspace: &AcquisitionWorkspace,
        file_index: usize,
        client: &super::AcquisitionHttpClient,
        url: &str,
        authorization: Option<&str>,
        retry: &AcquisitionRetryPolicy,
        host: &mut dyn AcquisitionHost,
    ) -> Result<u64> {
        self.acquire_source_file(
            context,
            operation,
            workspace,
            file_index,
            &AcquisitionSource::Http {
                client: client.clone(),
                source: AcquisitionHttpSource {
                    url: url.to_owned(),
                    authorization: authorization.map(str::to_owned),
                },
            },
            retry,
            host,
        )
        .await
    }

    #[allow(clippy::too_many_arguments)]
    async fn acquire_source_file(
        &self,
        context: &TaskContext,
        operation: &AcquisitionOperation,
        workspace: &AcquisitionWorkspace,
        file_index: usize,
        source: &AcquisitionSource,
        retry: &AcquisitionRetryPolicy,
        host: &mut dyn AcquisitionHost,
    ) -> Result<u64> {
        let url = source.request_identity();
        let transfer_deadline = source.transfer_deadline(retry)?;
        if &operation.record.workspace != workspace.identity() {
            return Err(invalid("Workspace grant does not match acquisition"));
        }
        let file = operation
            .record
            .manifest
            .files()
            .get(file_index)
            .ok_or_else(|| invalid("Selected file is unavailable"))?
            .clone();
        let prepare = workspace.clone();
        let selected = file.clone();
        owned(context, "prepare acquisition file parent", move || {
            prepare.prepare_file(&selected)
        })
        .await?;
        if matches!(
            operation.record.phase,
            AcquisitionPhase::FilesReady
                | AcquisitionPhase::Using { .. }
                | AcquisitionPhase::Adopted { .. }
        ) {
            let verify = workspace.clone();
            let manifest = operation.record.manifest.clone();
            let receipts = operation.record.files.clone();
            owned(context, "verify reopened acquisition receipts", move || {
                verify.verify_receipts(&manifest, &receipts)
            })
            .await?;
            return Ok(operation.record.files[file_index].bytes);
        }
        let inspect = workspace.clone();
        let path = file.logical_path().to_owned();
        let existing = owned(context, "inspect acquisition final file", move || {
            inspect.file_len(&path, false)
        })
        .await?;
        let compare_existing = match existing {
            Some(size) => {
                if file
                    .expected_size()
                    .is_some_and(|expected| expected != size)
                {
                    return Err(invalid(
                        "Existing selected file size conflicts with selection",
                    ));
                }
                if file.expected_sha256().is_some() {
                    let verify = workspace.clone();
                    let selected = file.clone();
                    let receipt = owned(context, "verify existing acquisition file", move || {
                        verify.verify_file(&selected, false)
                    })
                    .await?;
                    return Ok(receipt.bytes);
                }
                if operation.record.manifest.source().revision().strength()
                    != super::RevisionStrength::Immutable
                {
                    return Err(invalid(
                        "Existing file has no digest or custody receipt for its mutable source",
                    ));
                }
                true
            }
            None => false,
        };
        let key = (operation.record.id, file_index);
        let started = Instant::now();
        let mut attempt = 0_u32;
        loop {
            if transfer_deadline.is_some_and(|deadline| deadline <= tokio::time::Instant::now()) {
                return Err(match source {
                    AcquisitionSource::Http { .. } => {
                        if host.cancel_requested() {
                            self.forget_checkpoint(key)?;
                            return Err(PumasError::DownloadCancelled);
                        }
                        if host.pause_requested_now() {
                            return Err(PumasError::DownloadPaused);
                        }
                        PumasError::DownloadFailed {
                            url: "artifact source".into(),
                            message: "HTTP acquisition elapsed retry budget exhausted".into(),
                        }
                    }
                    #[cfg(feature = "s3")]
                    AcquisitionSource::S3(_) => PumasError::DownloadFailed {
                        url: "S3 selected object".into(),
                        message: "S3 acquisition elapsed retry budget exhausted".into(),
                    },
                });
            }
            attempt = attempt
                .checked_add(1)
                .ok_or_else(|| invalid("Retry counter exhausted"))?;
            host.retry(attempt, None, None).await?;
            if host.cancel_requested() {
                self.forget_checkpoint(key)?;
                return Err(PumasError::DownloadCancelled);
            }
            if host.pause_requested_now() {
                return Err(PumasError::DownloadPaused);
            }
            let inspect = workspace.clone();
            let path = file.logical_path().to_owned();
            let physical_length = owned(context, "inspect acquisition partial file", move || {
                inspect.file_len(&path, true)
            })
            .await?
            .unwrap_or(0);
            // A publisher digest can independently verify a complete partial.
            // Immutable revision and length alone cannot establish completion.
            if !compare_existing
                && physical_length > 0
                && file.expected_size() == Some(physical_length)
                && file.expected_sha256().is_some()
            {
                self.forget_checkpoint(key)?;
                let publish = workspace.clone();
                let selected = file.clone();
                return owned(
                    context,
                    "publish digest-verified acquisition partial",
                    move || {
                        publish
                            .publish_part(&selected, false)
                            .map(|receipt| receipt.bytes)
                    },
                )
                .await;
            }
            let checkpoint = self.lookup_checkpoint(key)?;
            let checkpoint = checkpoint.filter(|checkpoint| {
                checkpoint.record == operation.record
                    && checkpoint.owner.matches(workspace)
                    && checkpoint.request_url == url
                    && operation.record.manifest.permits_resume(file_index)
                    && !(compare_existing && attempt == 1)
                    && checkpoint.prefix.bytes > 0
                    && checkpoint.prefix.bytes == physical_length
                    && checkpoint
                        .total
                        .or(file.expected_size())
                        .is_none_or(|total| checkpoint.prefix.bytes < total)
            });
            let mut checked_part = None;
            let mut resume = 0;
            let mut continuation = None;
            if let Some(checkpoint) = checkpoint {
                let check = workspace.clone();
                let path = file.logical_path().to_owned();
                let prefix = checkpoint.prefix.clone();
                match owned(context, "verify live acquisition prefix", move || {
                    check.open_checkpoint_part(&path, &prefix)
                })
                .await
                {
                    Ok(checked) => {
                        resume = checkpoint.prefix.bytes;
                        continuation = Some(HttpResumeEvidence {
                            resource: checkpoint.resource.clone(),
                            etag: checkpoint.etag.clone(),
                            total: checkpoint.total,
                        });
                        checked_part = Some((checked, checkpoint.prefix));
                    }
                    Err(PumasError::Validation { .. }) => self.forget_checkpoint(key)?,
                    Err(error) => {
                        self.forget_checkpoint(key)?;
                        return Err(error);
                    }
                }
            } else {
                self.forget_checkpoint(key)?;
            }
            let response = tokio::select! {
                biased;
                _ = host.pause_requested() => return Err(if host.cancel_requested() {
                    self.forget_checkpoint(key)?;
                    PumasError::DownloadCancelled
                } else {
                    PumasError::DownloadPaused
                }),
                response = source.open(&operation.record.manifest, file_index, resume, continuation.as_ref(), transfer_deadline) => response,
            };
            let outcome = match response {
                Ok(mut response) => {
                    let resource = std::mem::take(&mut response.resource);
                    let etag = response.strong_etag.take();
                    let total = response.total_size;
                    self.forget_checkpoint(key)?;
                    let open = workspace.clone();
                    let path = file.logical_path().to_owned();
                    let append = response.resumed;
                    let (handle, hash) = owned(
                        context,
                        "open checked acquisition partial file",
                        move || {
                            if append {
                                let ((mut handle, _), prefix) = checked_part.ok_or_else(|| {
                                    invalid("Continuation has no checked descriptor")
                                })?;
                                let hash =
                                    open.validate_checkpoint_part(&path, &mut handle, &prefix)?;
                                Ok((handle, hash))
                            } else {
                                Ok((open.open_part(&path, false)?, Sha256::new()))
                            }
                        },
                    )
                    .await?;
                    let mut sink = AcquisitionSink {
                        file: Some(handle),
                        context,
                        hash,
                    };
                    let stream = stream_http_artifact(response, resume, &mut sink, host);
                    let outcome = match (source, transfer_deadline) {
                        (AcquisitionSource::Http { .. }, Some(deadline)) => {
                            // Tokio timeout_at polls the wrapped future first.
                            // Check expiry before polling any new byte effect.
                            let outcome = tokio::select! {
                                biased;
                                _ = http_budget_elapsed(deadline) => None,
                                outcome = stream => Some(outcome),
                            };
                            match outcome {
                                Some(outcome) => outcome,
                                None if host.cancel_requested() => Ok(HttpBodyOutcome::Cancelled),
                                None if host.pause_requested_now() => Ok(HttpBodyOutcome::Paused),
                                None => Err(http_budget_timeout()),
                            }
                        }
                        _ => stream.await,
                    };
                    if sink.file.is_none() {
                        // A source deadline can cancel the write waiter. Join the
                        // registered descriptor effect before any retry truncates it.
                        match context.drain_blocking().await {
                            Ok(0) => {},
                            result => return Err(PumasError::Other(format!(
                                "Acquisition write effects unsettled after interruption: {result:?}"))),
                        }
                    }
                    if !matches!(outcome, Ok(HttpBodyOutcome::Cancelled)) && sink.file.is_some() {
                        sink.flush().await?;
                        let mut handle = sink
                            .file
                            .take()
                            .ok_or_else(|| invalid("Partial file effect is unfinished"))?;
                        let expected = hex::encode(sink.hash.clone().finalize());
                        let check = workspace.clone();
                        let path = file.logical_path().to_owned();
                        let prefix =
                            owned(context, "verify streamed acquisition prefix", move || {
                                let (prefix, _) = check.observe_part(&path, &mut handle)?;
                                if prefix.sha256 != expected {
                                    return Err(invalid(
                                        "Acquisition prefix differs from streamed bytes",
                                    ));
                                }
                                Ok(prefix)
                            })
                            .await?;
                        if let Some(etag) = etag {
                            if prefix.bytes > 0 && total.is_none_or(|total| prefix.bytes < total) {
                                self.retain_checkpoint(
                                    key,
                                    workspace,
                                    WarmCheckpointMetadata {
                                        record: &operation.record,
                                        request_url: &url,
                                        resource: &resource,
                                        etag: &etag,
                                        total,
                                        prefix: &prefix,
                                    },
                                )?;
                            }
                        }
                    }
                    outcome
                }
                Err(error) => Err(error),
            };
            match outcome {
                Ok(HttpBodyOutcome::Complete { .. }) => {
                    self.forget_checkpoint(key)?;
                    if host.cancel_requested() {
                        return Err(PumasError::DownloadCancelled);
                    }
                    if host.pause_requested_now() {
                        return Err(PumasError::DownloadPaused);
                    }
                    let publish = workspace.clone();
                    let selected = file.clone();
                    return owned(context, "publish verified acquisition file", move || {
                        publish
                            .publish_part(&selected, compare_existing)
                            .map(|receipt| receipt.bytes)
                    })
                    .await;
                }
                Ok(HttpBodyOutcome::Paused) => return Err(PumasError::DownloadPaused),
                Ok(HttpBodyOutcome::Cancelled) => {
                    self.forget_checkpoint(key)?;
                    return Err(PumasError::DownloadCancelled);
                }
                Err(error) => {
                    if !error.is_retryable() || host.cancel_requested() {
                        self.forget_checkpoint(key)?;
                        return Err(error);
                    }
                    if retry.attempts.is_some_and(|limit| attempt >= limit)
                        || (retry.elapsed > Duration::ZERO && started.elapsed() >= retry.elapsed)
                    {
                        return Err(PumasError::DownloadFailed { url: "artifact source".into(), message: format!("Acquisition retry budget exhausted after {attempt} attempts: {error}") });
                    }
                    let delay = retry.backoff.calculate_delay(attempt.saturating_sub(1));
                    let delay = if let Some(deadline) = transfer_deadline {
                        delay.min(deadline.saturating_duration_since(tokio::time::Instant::now()))
                    } else {
                        delay
                    };
                    host.retry(attempt, Some(delay), Some(&error.to_string()))
                        .await?;
                    tokio::select! {
                        biased;
                        _ = host.pause_requested() => return Err(if host.cancel_requested() {
                            self.forget_checkpoint(key)?;
                            PumasError::DownloadCancelled
                        } else {
                            PumasError::DownloadPaused
                        }),
                        _ = tokio::time::sleep(delay) => {},
                    }
                }
            }
        }
    }

    /// Reopening `Using` requires an exact consumer reconciliation lease. The
    /// consumer must first revalidate its durable admission under root custody;
    /// the store token by itself never authorizes replay of consumer effects.
    pub(crate) async fn reconciliation_lease(
        &self,
        context: &TaskContext,
        demand: &AcquisitionDemand,
    ) -> Result<Option<AcquisitionReconciliation>> {
        let store = self.store.clone();
        let demand = demand.clone();
        owned(
            context,
            "observe acquisition reconciliation custody",
            move || {
                Ok(store
                    .acquisitions()?
                    .values()
                    .find(|record| record.demand == demand)
                    .and_then(|record| match record.phase {
                        AcquisitionPhase::Using { .. } => Some(AcquisitionReconciliation::Using),
                        AcquisitionPhase::Adopted { lease } => {
                            Some(AcquisitionReconciliation::Adopted(lease))
                        }
                        _ => None,
                    }))
            },
        )
        .await
    }

    /// Reopen the exact historical `Using` record without changing its lease
    /// or publishing state. The returned proof retains the verified workspace
    /// through caller-owned, receipt-qualified settlement.
    pub(crate) async fn reopen_using(
        &self,
        context: &TaskContext,
        demand: &AcquisitionDemand,
        manifest: &ArtifactManifest,
        workspace: &AcquisitionWorkspace,
    ) -> Result<Option<AcquisitionReopenProof>> {
        if !context.is_current_role(super::task_custody::TaskRole::Worker) {
            return Err(invalid("Cold reconciliation requires its active worker"));
        }
        let store = self.store.clone();
        let demand = demand.clone();
        let manifest = manifest.clone();
        let expected_workspace = workspace.identity().clone();
        let record = owned(context, "observe retained acquisition use", move || {
            let Some(record) = store
                .acquisitions()?
                .into_values()
                .find(|record| record.demand == demand)
            else {
                return Ok(None);
            };
            if record.workspace != expected_workspace {
                return Err(invalid(
                    "Retained acquisition does not match the reopened workspace",
                ));
            }
            match &record.phase {
                AcquisitionPhase::Using { .. } => {
                    if record.manifest != manifest {
                        return Err(invalid(
                            "Retained acquisition use does not match the reopened selection",
                        ));
                    }
                    Ok(Some(record))
                }
                AcquisitionPhase::Transferring | AcquisitionPhase::FilesReady => Ok(None),
                AcquisitionPhase::Adopted { .. } | AcquisitionPhase::Withdrawn => Err(invalid(
                    "Retained terminal acquisition cannot be replayed by a reopened consumer",
                )),
            }
        })
        .await?;
        let Some(record) = record else {
            return Ok(None);
        };
        let verify = workspace.clone();
        let manifest = record.manifest.clone();
        let files = record.files.clone();
        owned(context, "verify retained acquisition receipts", move || {
            verify.verify_receipts(&manifest, &files)
        })
        .await?;
        Ok(Some(AcquisitionReopenProof {
            record,
            _workspace: workspace.clone(),
        }))
    }

    pub(crate) async fn files_ready(
        &self,
        context: &TaskContext,
        operation: AcquisitionOperation,
        workspace: AcquisitionWorkspace,
    ) -> Result<AcquisitionUseLease> {
        self.files_ready_with_cancellation_check(context, operation, workspace, || false)
            .await
    }

    async fn files_ready_with_host<H>(
        &self,
        context: &TaskContext,
        operation: AcquisitionOperation,
        workspace: AcquisitionWorkspace,
        host: &mut H,
    ) -> Result<AcquisitionUseLease>
    where
        H: HttpAttemptHost + ?Sized,
    {
        self.files_ready_with_cancellation_check(context, operation, workspace, move || {
            host.cancel_requested()
        })
        .await
    }

    async fn files_ready_with_cancellation_check<F>(
        &self,
        context: &TaskContext,
        operation: AcquisitionOperation,
        workspace: AcquisitionWorkspace,
        cancellation_requested: F,
    ) -> Result<AcquisitionUseLease>
    where
        F: Fn() -> bool + Send,
    {
        if !context.shares_scope(&operation.context)
            || !context.generation().matches(operation.context.generation())
        {
            return Err(invalid(
                "Verified handoff belongs to another operation generation",
            ));
        }
        if cancellation_requested() {
            return Err(PumasError::DownloadCancelled);
        }
        let seal = workspace.clone();
        let manifest = operation.record.manifest.clone();
        let files = owned(context, "seal verified acquisition file set", move || {
            seal.seal(&manifest)
        })
        .await?;
        if cancellation_requested() {
            return Err(PumasError::DownloadCancelled);
        }
        let store = self.store.clone();
        let mut expected = operation.record.clone();
        if matches!(expected.phase, AcquisitionPhase::Transferring) {
            let ready = expected.clone();
            let receipts = files.clone();
            owned(context, "persist acquisition files ready", move || {
                store.update_acquisitions(|records| {
                    let record = records
                        .get_mut(&ready.id)
                        .ok_or_else(|| invalid("Acquisition custody disappeared"))?;
                    if record != &ready {
                        return Err(invalid("Acquisition readiness is stale"));
                    }
                    record.files = receipts;
                    record.phase = AcquisitionPhase::FilesReady;
                    Ok(())
                })
            })
            .await?;
            expected.files = files.clone();
            expected.phase = AcquisitionPhase::FilesReady;
        }
        if files != expected.files {
            return Err(invalid("Reopened verified-file receipts changed"));
        }
        let lease = match expected.phase {
            AcquisitionPhase::Using { lease } | AcquisitionPhase::Adopted { lease } => lease,
            AcquisitionPhase::FilesReady => {
                let store = self.store.clone();
                let lease = Uuid::new_v4();
                owned(
                    context,
                    "handoff durable verified acquisition files",
                    move || {
                        store.update_acquisitions(|records| {
                            let record = records
                                .get_mut(&expected.id)
                                .ok_or_else(|| invalid("Acquisition custody disappeared"))?;
                            if record != &expected {
                                return Err(invalid("Acquisition handoff is stale"));
                            }
                            record.phase = AcquisitionPhase::Using { lease };
                            Ok(())
                        })
                    },
                )
                .await?;
                lease
            }
            _ => return Err(invalid("Acquisition is not ready for consumer use")),
        };
        let mut record = operation.record;
        record.files = files;
        if !matches!(record.phase, AcquisitionPhase::Adopted { .. }) {
            record.phase = AcquisitionPhase::Using { lease };
        }
        Ok(AcquisitionUseLease {
            operation: AcquisitionOperation {
                record,
                context: context.clone(),
            },
            workspace,
            lease,
            context: context.clone(),
        })
    }

    /// Called only after the exact consumer operation and every nested effect
    /// have been positively observed. A path/tag probe cannot acknowledge it.
    pub(crate) async fn acknowledge(
        &self,
        context: &TaskContext,
        lease: AcquisitionUseLease,
    ) -> Result<()> {
        let verify = lease.workspace.clone();
        let store = self.store.clone();
        let id = lease.operation.record.id;
        let token = lease.lease;
        let manifest = lease.operation.record.manifest.clone();
        let read_store = self.store.clone();
        let record = owned(
            context,
            "observe exact acquisition consumer custody",
            move || {
                read_store
                    .acquisitions()?
                    .remove(&id)
                    .ok_or_else(|| invalid("Consumer custody disappeared"))
            },
        )
        .await?;
        let expected = record.clone();
        owned(context, "verify acquisition consumer receipts", move || {
            verify.verify_receipts(&manifest, &record.files)
        })
        .await?;
        owned(context, "acknowledge adopted acquisition files", move || {
            store.update_acquisitions(|records| {
                let record = records.get_mut(&id).ok_or_else(|| invalid("Consumer custody disappeared"))?;
                if record != &expected || !matches!(record.phase, AcquisitionPhase::Using { lease } | AcquisitionPhase::Adopted { lease } if lease == token) {
                    return Err(invalid("Consumer acknowledgment is stale"));
                }
                record.phase = AcquisitionPhase::Adopted { lease: token };
                Ok(())
            })
        }).await
    }
}

/// Consumer-facing view of the existing shared acquisition lifecycle.
pub struct AcquisitionConsumer {
    service: Arc<AcquisitionService>,
    scope: Arc<super::task_custody::TaskScope>,
    owner: String,
}

impl AcquisitionConsumer {
    /// Resolve explicit pins under the same bounded worker/scope authority as
    /// transfer. Source failures remain typed; no durable acquisition is admitted.
    #[cfg(feature = "s3")]
    pub(crate) async fn resolve_s3_manifest(
        &self,
        reader: super::S3Reader,
        entries: Vec<super::S3ManifestEntry>,
        demand: &AcquisitionDemand,
        retry: &AcquisitionRetryPolicy,
        host: Box<dyn AcquisitionHost>,
    ) -> Result<std::result::Result<super::S3ManifestSelection, super::S3ReaderError>> {
        self.require_s3_transfer(demand, retry)?;
        self.scope.run_worker_invocation(move |_| async move {
            if host.cancel_requested() {
                return Err(PumasError::DownloadCancelled);
            }
            tokio::select! {
                biased;
                _ = host.pause_requested() => {
                    Err(if host.cancel_requested() { PumasError::DownloadCancelled } else { PumasError::DownloadPaused })
                }
                selection = reader.select_manifest(entries) => Ok(selection),
            }
        }).await
    }

    pub fn owner(&self) -> &str {
        &self.owner
    }

    /// Read and validate this consumer's completion receipt for an exact
    /// retained record. This grants no filesystem or settlement authority;
    /// adopted consumers use it to verify their own published output before
    /// retrying local workspace cleanup.
    pub fn completion_receipt(
        &self,
        record: &AcquisitionRecord,
    ) -> Result<Option<AcquisitionConsumerReceipt>> {
        if record.demand.consumer != self.owner {
            return Err(invalid("Completion receipt belongs to another consumer"));
        }
        let lease = match &record.phase {
            AcquisitionPhase::Using { lease } | AcquisitionPhase::Adopted { lease } => *lease,
            _ => {
                return Err(invalid(
                    "Completion receipt requires a retained consumer use",
                ))
            }
        };
        let Some(receipt) = self.service.consumer_receipt(record.id)? else {
            return Ok(None);
        };
        receipt.validate_for_record(record, lease)?;
        Ok(Some(receipt))
    }

    /// Close this consumer scope and join every transfer and registered effect.
    pub async fn shutdown(&self) -> Result<()> {
        self.scope.shutdown().await
    }

    /// Run consumer recovery or cleanup through this shared scope's bounded
    /// task/effect custody. Dropping the result waiter cancels the outer task;
    /// shutdown still joins a registered blocking closure until it truly ends.
    pub async fn run_blocking<T: Send + 'static>(
        &self,
        name: &'static str,
        work: impl FnOnce() -> Result<T> + Send + 'static,
    ) -> Result<T> {
        self.scope
            .run_worker_invocation(move |context| async move { owned(&context, name, work).await })
            .await
    }

    /// Revalidate a retained consumer receipt under the supplied held
    /// workspace and exact source selection. The callback owns interpretation
    /// of its payload and must verify its durable output before returning.
    /// This method never repeats transfer or consumer effects.
    pub async fn reconcile<T, F, Fut>(
        &self,
        demand: AcquisitionDemand,
        manifest: ArtifactManifest,
        workspace: AcquisitionWorkspace,
        validate_output: F,
    ) -> Result<Option<T>>
    where
        T: Send + 'static,
        F: FnOnce(AcquisitionConsumerReceipt, AcquiredArtifactUse) -> Fut + Send + 'static,
        Fut: Future<Output = Result<T>> + Send + 'static,
    {
        if demand.consumer != self.owner {
            return Err(invalid("Consumer demand identity does not match its scope"));
        }
        let service = self.service.clone();
        self.scope
            .run_worker_invocation(move |context| async move {
                service.require_schema(&context).await?;
                let records = service.store.clone();
                let demand_lookup = demand.clone();
                let current = owned(&context, "observe consumer completion record", move || {
                    Ok(records
                        .acquisitions()?
                        .into_values()
                        .find(|record| record.demand == demand_lookup))
                })
                .await?;
                let Some(record) = current else {
                    return Ok(None);
                };
                if record.manifest != manifest || &record.workspace != workspace.identity() {
                    return Err(invalid(
                        "Retained consumer operation changed selection or workspace",
                    ));
                }
                let lease = match &record.phase {
                    AcquisitionPhase::Using { lease } | AcquisitionPhase::Adopted { lease } => {
                        *lease
                    }
                    AcquisitionPhase::Transferring | AcquisitionPhase::FilesReady => {
                        return Ok(None)
                    }
                    AcquisitionPhase::Withdrawn => {
                        return Err(invalid("Withdrawn consumer operation cannot be reconciled"))
                    }
                };
                if matches!(&record.phase, AcquisitionPhase::Using { .. }) {
                    let proof = service
                        .reopen_using(&context, &demand, &manifest, &workspace)
                        .await?
                        .ok_or_else(|| invalid("Retained consumer use disappeared"))?;
                    if proof.record() != &record {
                        return Err(invalid("Retained consumer use changed during reopen"));
                    }
                } else {
                    let verify = workspace.clone();
                    let manifest = manifest.clone();
                    let files = record.files.clone();
                    owned(
                        &context,
                        "verify adopted consumer input receipts",
                        move || verify.verify_receipts(&manifest, &files),
                    )
                    .await?;
                }
                let receipt_store = service.store.clone();
                let receipt = owned(
                    &context,
                    "read exact consumer completion receipt",
                    move || {
                        receipt_store.consumer_receipt(record.id)?.ok_or_else(|| {
                            PumasError::Validation {
                                field: "acquisition.consumer_recovery_required".into(),
                                message:
                                    "Retained consumer use has no authoritative completion receipt"
                                        .into(),
                            }
                        })
                    },
                )
                .await?;
                receipt.validate_for_record(&record, lease)?;
                let use_handle = AcquiredArtifactUse {
                    store: service.store.clone(),
                    context: context.clone(),
                    lease: AcquisitionUseLease {
                        operation: AcquisitionOperation {
                            record: record.clone(),
                            context: context.clone(),
                        },
                        workspace: workspace.clone(),
                        lease,
                        context: context.clone(),
                    },
                };
                let result = validate_output(receipt.clone(), use_handle).await?;
                match context.drain_blocking().await {
                    Ok(0) => {}
                    Ok(failures) => {
                        return Err(PumasError::Other(format!(
                            "Consumer recovery effects failed before settlement: {failures}"
                        )))
                    }
                    Err(error) => {
                        return Err(PumasError::Other(format!(
                            "Consumer recovery effect drain failed before settlement: {error}"
                        )))
                    }
                }
                service
                    .settle_consumer_use(&context, &record, lease, receipt.payload)
                    .await?;
                Ok(Some(result))
            })
            .await
    }

    /// Acquire the exact manifest using shared HTTP custody, then run the
    /// consumer's extraction/publication while the verified use lease remains
    /// held. The callback returns its owner-specific durable receipt payload.
    pub async fn acquire_http<Staged, Output, F, Fut, Publish, PublishFut>(
        &self,
        request: AcquisitionHttpRequest,
        client: impl Into<super::AcquisitionHttpClient>,
        host: Box<dyn AcquisitionHost>,
        prepare: F,
        publish: Publish,
    ) -> Result<Output>
    where
        Staged: Send + 'static,
        Output: Send + 'static,
        F: FnOnce(AcquiredArtifactUse) -> Fut + Send + 'static,
        Fut: Future<Output = Result<(Staged, Value)>> + Send + 'static,
        Publish: FnOnce(Staged, AcquisitionConsumerReceipt) -> PublishFut + Send + 'static,
        PublishFut: Future<Output = Result<Output>> + Send + 'static,
    {
        let client = client.into();
        if request.demand.consumer != self.owner {
            return Err(invalid("Consumer demand identity does not match its scope"));
        }
        if request.sources.len() != request.manifest.files().len()
            || request
                .sources
                .iter()
                .any(|source| source.url.trim().is_empty())
        {
            return Err(invalid(
                "HTTP sources must match the exact selected manifest files",
            ));
        }
        // Reject an unrepresentable positive budget before worker/store admission.
        let _ = http_transfer_deadline(request.retry.elapsed)?;
        for source in &request.sources {
            client.for_request(&source.url, source.authorization.is_some())?;
        }
        self.acquire(
            AcquisitionRequest {
                demand: request.demand,
                manifest: request.manifest,
                workspace: request.workspace,
                sources: request
                    .sources
                    .into_iter()
                    .map(|source| AcquisitionSource::Http {
                        client: client.clone(),
                        source,
                    })
                    .collect(),
                retry: request.retry,
            },
            host,
            prepare,
            publish,
        )
        .await
    }

    /// Acquire one versioned S3 object under this consumer's existing durable
    /// lifecycle, then hold its verified use through consumer publication.
    /// Caller-supplied positive finite attempt and elapsed limits bound transfer
    /// retries; registered writes are drained before retry or terminal release.
    #[cfg(feature = "s3")]
    pub async fn acquire_s3<Staged, Output, F, Fut, Publish, PublishFut>(
        &self,
        request: AcquisitionS3Request,
        host: Box<dyn AcquisitionHost>,
        prepare: F,
        publish: Publish,
    ) -> Result<Output>
    where
        Staged: Send + 'static,
        Output: Send + 'static,
        F: FnOnce(AcquiredArtifactUse) -> Fut + Send + 'static,
        Fut: Future<Output = Result<(Staged, Value)>> + Send + 'static,
        Publish: FnOnce(Staged, AcquisitionConsumerReceipt) -> PublishFut + Send + 'static,
        PublishFut: Future<Output = Result<Output>> + Send + 'static,
    {
        self.require_s3_transfer(&request.demand, &request.retry)?;
        self.acquire(
            AcquisitionRequest {
                demand: request.demand,
                manifest: request.selection.manifest().clone(),
                workspace: request.workspace,
                sources: vec![AcquisitionSource::S3(request.selection)],
                retry: request.retry,
            },
            host,
            prepare,
            publish,
        )
        .await
    }

    /// Acquire the complete explicit version-pinned set through the same store,
    /// writer, verified-file handoff and consumer receipt/settlement protocol.
    /// Retry attempts and elapsed budgets apply per object; no complete-set
    /// hard wall-clock or atomic remote-prefix snapshot is promised.
    #[cfg(feature = "s3")]
    pub async fn acquire_s3_manifest<Staged, Output, F, Fut, Publish, PublishFut>(
        &self,
        request: AcquisitionS3ManifestRequest,
        host: Box<dyn AcquisitionHost>,
        prepare: F,
        publish: Publish,
    ) -> Result<Output>
    where
        Staged: Send + 'static,
        Output: Send + 'static,
        F: FnOnce(AcquiredArtifactUse) -> Fut + Send + 'static,
        Fut: Future<Output = Result<(Staged, Value)>> + Send + 'static,
        Publish: FnOnce(Staged, AcquisitionConsumerReceipt) -> PublishFut + Send + 'static,
        PublishFut: Future<Output = Result<Output>> + Send + 'static,
    {
        self.require_s3_transfer(&request.demand, &request.retry)?;
        let (manifest, objects) = request.selection.into_parts();
        self.acquire(
            AcquisitionRequest {
                demand: request.demand,
                manifest,
                workspace: request.workspace,
                sources: objects.into_iter().map(AcquisitionSource::S3).collect(),
                retry: request.retry,
            },
            host,
            prepare,
            publish,
        )
        .await
    }

    #[cfg(feature = "s3")]
    fn require_s3_transfer(
        &self,
        demand: &AcquisitionDemand,
        retry: &AcquisitionRetryPolicy,
    ) -> Result<()> {
        if demand.consumer != self.owner {
            return Err(invalid("Consumer demand identity does not match its scope"));
        }
        if retry.attempts.is_none_or(|limit| limit == 0)
            || retry.elapsed.is_zero()
            || tokio::time::Instant::now()
                .checked_add(retry.elapsed)
                .is_none()
        {
            return Err(invalid(
                "S3 transfer requires positive finite attempt and elapsed retry budgets",
            ));
        }
        Ok(())
    }

    async fn acquire<Staged, Output, F, Fut, Publish, PublishFut>(
        &self,
        request: AcquisitionRequest,
        mut host: Box<dyn AcquisitionHost>,
        prepare: F,
        publish: Publish,
    ) -> Result<Output>
    where
        Staged: Send + 'static,
        Output: Send + 'static,
        F: FnOnce(AcquiredArtifactUse) -> Fut + Send + 'static,
        Fut: Future<Output = Result<(Staged, Value)>> + Send + 'static,
        Publish: FnOnce(Staged, AcquisitionConsumerReceipt) -> PublishFut + Send + 'static,
        PublishFut: Future<Output = Result<Output>> + Send + 'static,
    {
        let service = self.service.clone();
        self.scope
            .run_worker_invocation(move |context| async move {
                let request = owned(&context, "bound consumer admission", move || {
                    require_consumer_manifest_bound(&request.manifest)?;
                    Ok(request)
                })
                .await?;
                service.require_schema(&context).await?;
                let operation = service
                    .begin(
                        &context,
                        request.demand,
                        request.manifest.clone(),
                        request.workspace.identity().clone(),
                        None,
                    )
                    .await?;
                if operation.is_adopted() {
                    return Err(PumasError::Validation {
                        field: "acquisition.consumer_recovery_required".into(),
                        message: "An adopted consumer operation cannot be replayed".into(),
                    });
                }
                for (file_index, source) in request.sources.iter().enumerate() {
                    host.file_started(file_index);
                    let bytes = service
                        .acquire_source_file(
                            &context,
                            &operation,
                            &request.workspace,
                            file_index,
                            source,
                            &request.retry,
                            host.as_mut(),
                        )
                        .await?;
                    host.file_acquired(file_index, bytes);
                }
                let lease = service
                    .files_ready_with_host(&context, operation, request.workspace, host.as_mut())
                    .await?;
                let expected = lease.record().clone();
                let use_lease = match &expected.phase {
                    AcquisitionPhase::Using { lease } | AcquisitionPhase::Adopted { lease } => {
                        *lease
                    }
                    _ => return Err(invalid("Verified consumer handoff has no exact use lease")),
                };
                let (staged, payload) = prepare(AcquiredArtifactUse {
                    store: service.store.clone(),
                    context: context.clone(),
                    lease,
                })
                .await?;
                match context.drain_blocking().await {
                    Ok(0) => {}
                    Ok(failures) => {
                        return Err(PumasError::Other(format!(
                            "Consumer effects failed before receipt settlement: {failures}"
                        )))
                    }
                    Err(error) => {
                        return Err(PumasError::Other(format!(
                            "Consumer effect drain failed before receipt settlement: {error}"
                        )))
                    }
                }
                let receipt = AcquisitionConsumerReceipt::for_record(&expected, use_lease, payload)?;
                service
                    .issue_consumer_receipt(&context, &expected, use_lease, receipt.clone())
                    .await?;
                let result = publish(staged, receipt.clone()).await?;
                match context.drain_blocking().await {
                    Ok(0) => {}
                    Ok(failures) => {
                        return Err(PumasError::Other(format!(
                            "Consumer publication effects failed before receipt settlement: {failures}"
                        )))
                    }
                    Err(error) => {
                        return Err(PumasError::Other(format!(
                            "Consumer publication effect drain failed before receipt settlement: {error}"
                        )))
                    }
                }
                service
                    .settle_consumer_receipt(&context, &expected, use_lease, receipt)
                    .await?;
                Ok(result)
            })
            .await
    }
}

async fn owned<T: Send + 'static>(
    context: &TaskContext,
    name: &'static str,
    work: impl FnOnce() -> Result<T> + Send + 'static,
) -> Result<T> {
    context
        .run_fallible_blocking_named(name, move || match work() {
            Err(
                error @ (PumasError::Validation { .. }
                | PumasError::HashMismatch { .. }
                | PumasError::DownloadRootBusy),
            ) => Ok(Err(error)),
            Err(error) => Err(error),
            Ok(value) => Ok(Ok(value)),
        })
        .await
        .map_err(|error| match error {
            super::task_custody::BlockingTaskError::CapacityExhausted { resource } => {
                PumasError::AcquisitionCapacityExhausted { resource }
            }
            error => PumasError::Other(format!("Acquisition effect observation failed: {error}")),
        })?
        .and_then(|result| result)
}

struct AcquisitionSink<'a> {
    file: Option<std::fs::File>,
    context: &'a TaskContext,
    hash: Sha256,
}

#[async_trait::async_trait]
impl HttpArtifactSink for AcquisitionSink<'_> {
    async fn write_all(&mut self, bytes: &[u8]) -> Result<()> {
        let mut file = self
            .file
            .take()
            .ok_or_else(|| invalid("Partial file effect is unfinished"))?;
        let written = bytes.to_owned();
        let bytes = written.clone();
        self.file = Some(
            owned(self.context, "write acquisition partial file", move || {
                write_chunk(&mut file, &bytes)?;
                Ok(file)
            })
            .await?,
        );
        self.hash.update(&written);
        Ok(())
    }
    async fn flush(&mut self) -> Result<()> {
        let file = self
            .file
            .take()
            .ok_or_else(|| invalid("Partial file effect is unfinished"))?;
        self.file = Some(
            owned(self.context, "sync acquisition partial file", move || {
                file.sync_all()?;
                Ok(file)
            })
            .await?,
        );
        Ok(())
    }
}

mod uuid_wire {
    use serde::{Deserialize, Deserializer, Serializer};
    use uuid::Uuid;
    pub(super) fn serialize<S: Serializer>(value: &Uuid, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&value.to_string())
    }
    pub(super) fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Uuid, D::Error> {
        Uuid::parse_str(&String::deserialize(deserializer)?).map_err(serde::de::Error::custom)
    }
}

pub(super) mod uuid_map {
    use super::AcquisitionRecord;
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use std::collections::BTreeMap;
    use uuid::Uuid;
    pub(crate) fn serialize<S: Serializer>(
        values: &BTreeMap<Uuid, AcquisitionRecord>,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        values
            .iter()
            .map(|(id, value)| (id.to_string(), value))
            .collect::<BTreeMap<_, _>>()
            .serialize(serializer)
    }
    pub(crate) fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<BTreeMap<Uuid, AcquisitionRecord>, D::Error> {
        BTreeMap::<String, AcquisitionRecord>::deserialize(deserializer)?
            .into_iter()
            .map(|(id, record)| {
                Uuid::parse_str(&id)
                    .map(|id| (id, record))
                    .map_err(serde::de::Error::custom)
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::acquisition::{
        ArtifactFile, ArtifactRevisionEvidence, ArtifactSourceIdentity,
        FileVerificationRequirement, RevisionStrength, Sha256Evidence,
    };
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    fn checkpoint_fixture(workspace: &AcquisitionWorkspace) -> (AcquisitionRecord, PartialPrefix) {
        let mut handle = workspace.open_part("payload.bin", false).unwrap();
        std::io::Write::write_all(&mut handle, b"DATA").unwrap();
        let prefix = workspace
            .observe_part("payload.bin", &mut handle)
            .unwrap()
            .0;
        let record = AcquisitionRecord {
            id: Uuid::new_v4(),
            demand: AcquisitionDemand {
                consumer: "fixture".into(),
                operation: "paused".into(),
            },
            manifest: manifest_with_size("payload.bin", 8),
            workspace: workspace.identity().clone(),
            phase: AcquisitionPhase::Transferring,
            files: Vec::new(),
        };
        (record, prefix)
    }

    fn retain_fixture(
        service: &AcquisitionService,
        workspace: &AcquisitionWorkspace,
        record: &AcquisitionRecord,
        index: usize,
        prefix: &PartialPrefix,
        resource: &str,
    ) -> bool {
        service
            .retain_checkpoint(
                (record.id, index),
                workspace,
                WarmCheckpointMetadata {
                    record,
                    request_url: "http://fixture/artifact",
                    resource,
                    etag: "\"fixture-v1\"",
                    total: Some(8),
                    prefix,
                },
            )
            .unwrap()
    }

    fn checkpoint_service(temp: &tempfile::TempDir, workers: usize) -> AcquisitionService {
        AcquisitionService::with_capacity(
            Arc::new(AcquisitionStore::new(temp.path())),
            super::super::AcquisitionCapacity {
                workers,
                ..Default::default()
            },
        )
        .unwrap()
    }

    #[test]
    fn checkpoint_cap_counts_file_indices_and_allows_same_key_replacement() {
        let temp = tempfile::TempDir::new().unwrap();
        let workspace = workspace(temp.path());
        let (record, prefix) = checkpoint_fixture(&workspace);
        let service = checkpoint_service(&temp, 2);
        assert!(retain_fixture(
            &service, &workspace, &record, 0, &prefix, "first"
        ));
        assert!(retain_fixture(
            &service, &workspace, &record, 1, &prefix, "second"
        ));
        assert!(!retain_fixture(
            &service, &workspace, &record, 2, &prefix, "third"
        ));
        assert!(retain_fixture(
            &service,
            &workspace,
            &record,
            0,
            &prefix,
            "replacement"
        ));
        assert_eq!(service.checkpoints.lock().unwrap().len(), 2);
        assert_eq!(
            service
                .lookup_checkpoint((record.id, 0))
                .unwrap()
                .unwrap()
                .resource,
            "replacement"
        );
        assert!(service.lookup_checkpoint((record.id, 1)).unwrap().is_some());
        assert_eq!(
            std::fs::read(temp.path().join("payload.bin.part")).unwrap(),
            b"DATA"
        );
        assert!(service.store.acquisitions().unwrap().is_empty());
    }

    #[test]
    fn expired_checkpoint_owners_are_reclaimed_on_lookup_and_insertion() {
        let temp = tempfile::TempDir::new().unwrap();
        let service = checkpoint_service(&temp, 1);
        let first = workspace(temp.path());
        let (record, prefix) = checkpoint_fixture(&first);
        assert!(retain_fixture(
            &service, &first, &record, 0, &prefix, "first"
        ));
        drop(first);
        assert!(service.lookup_checkpoint((record.id, 0)).unwrap().is_none());
        assert_eq!(
            std::fs::read(temp.path().join("payload.bin.part")).unwrap(),
            b"DATA"
        );
        let second = workspace(temp.path());
        assert!(retain_fixture(
            &service, &second, &record, 0, &prefix, "second"
        ));
        drop(second);
        let third = workspace(temp.path());
        assert!(retain_fixture(
            &service, &third, &record, 1, &prefix, "third"
        ));
        assert!(service.lookup_checkpoint((record.id, 0)).unwrap().is_none());
        assert_eq!(service.checkpoints.lock().unwrap().len(), 1);
        assert_eq!(
            std::fs::read(temp.path().join("payload.bin.part")).unwrap(),
            b"DATA"
        );
    }

    #[test]
    fn oversized_checkpoint_metadata_refuses_replacement_without_owned_copies() {
        let temp = tempfile::TempDir::new().unwrap();
        let workspace = workspace(temp.path());
        let (mut record, prefix) = checkpoint_fixture(&workspace);
        let service = checkpoint_service(&temp, 1);
        assert!(retain_fixture(
            &service, &workspace, &record, 0, &prefix, "small"
        ));
        let large = "\"".repeat(CHECKPOINT_METADATA_LIMIT / 2);
        assert!(!retain_fixture(
            &service, &workspace, &record, 0, &prefix, &large
        ));
        record.demand.operation = "x".repeat(CHECKPOINT_METADATA_LIMIT);
        assert!(!retain_fixture(
            &service, &workspace, &record, 0, &prefix, "small"
        ));
        assert_eq!(
            service
                .lookup_checkpoint((record.id, 0))
                .unwrap()
                .unwrap()
                .resource,
            "small"
        );

        // A borrowed encoder aborts before serializing the tail. The retention
        // helper only clones after this complete measurement succeeds.
        struct UnvisitedTail;
        impl Serialize for UnvisitedTail {
            fn serialize<S: serde::Serializer>(
                &self,
                _: S,
            ) -> std::result::Result<S::Ok, S::Error> {
                panic!("encoding must stop at the metadata limit");
            }
        }
        assert!(!checkpoint_metadata_fits(&(&large, UnvisitedTail)));
        let mut writer = CappedMetadataWriter {
            remaining: CHECKPOINT_METADATA_LIMIT,
        };
        assert!(std::io::Write::write_all(&mut writer, &[0; CHECKPOINT_METADATA_LIMIT]).is_ok());
        assert!(std::io::Write::write_all(&mut writer, &[0]).is_err());
    }

    #[tokio::test]
    async fn checkpoint_limits_are_shared_by_stores_and_consumers() {
        let temp = tempfile::TempDir::new().unwrap();
        let workspace = workspace(temp.path());
        let (record, prefix) = checkpoint_fixture(&workspace);
        let service = Arc::new(checkpoint_service(&temp, 1));
        let other_store = tempfile::TempDir::new().unwrap();
        let rebound =
            Arc::new(service.with_store(Arc::new(AcquisitionStore::new(other_store.path()))));
        let first = service.open_consumer("first").unwrap();
        let second = rebound.open_consumer("second").unwrap();
        assert!(retain_fixture(
            &first.service,
            &workspace,
            &record,
            0,
            &prefix,
            "first"
        ));
        assert!(!retain_fixture(
            &second.service,
            &workspace,
            &record,
            1,
            &prefix,
            "second"
        ));
        assert_eq!(rebound.checkpoints.lock().unwrap().len(), 1);
        first.shutdown().await.unwrap();
        second.shutdown().await.unwrap();
        assert_eq!(service.checkpoints.lock().unwrap().len(), 1);
        service.shutdown().await.unwrap();
        assert!(rebound.checkpoints.lock().unwrap().is_empty());
    }

    #[test]
    fn concurrent_checkpoint_insertions_stay_within_shared_cap() {
        let temp = tempfile::TempDir::new().unwrap();
        let workspace = workspace(temp.path());
        let (record, prefix) = checkpoint_fixture(&workspace);
        let service = checkpoint_service(&temp, 3);
        let gate = std::sync::Barrier::new(16);
        let admitted = std::thread::scope(|scope| {
            let handles: Vec<_> = (0..16)
                .map(|index| {
                    let (service, workspace, record, prefix, gate) =
                        (&service, &workspace, &record, &prefix, &gate);
                    scope.spawn(move || {
                        gate.wait();
                        let retained =
                            retain_fixture(service, workspace, record, index, prefix, "resource");
                        assert!(service.checkpoints.lock().unwrap().len() <= 3);
                        retained
                    })
                })
                .collect();
            handles
                .into_iter()
                .map(|handle| handle.join().unwrap())
                .filter(|retained| *retained)
                .count()
        });
        assert_eq!(admitted, 3);
        assert_eq!(service.checkpoints.lock().unwrap().len(), 3);
    }

    fn manifest(path: &str) -> ArtifactManifest {
        manifest_with_size(path, 4)
    }

    fn manifest_with_size(path: &str, size: u64) -> ArtifactManifest {
        ArtifactManifest::new(
            ArtifactSourceIdentity::new(
                "fixture",
                "selected-object",
                ArtifactRevisionEvidence::new(
                    "fixture.revision",
                    "immutable-v1",
                    RevisionStrength::Immutable,
                )
                .unwrap(),
            )
            .unwrap(),
            vec![ArtifactFile::new(
                path,
                "payload",
                Some(size),
                None,
                FileVerificationRequirement::SizeAndImmutableRevision,
            )
            .unwrap()],
        )
        .unwrap()
    }

    fn workspace(path: &std::path::Path) -> AcquisitionWorkspace {
        workspace_with_identity(
            path,
            WorkspaceIdentity {
                root_identity: "fixture-physical-root".into(),
                relative_target: "staging".into(),
            },
        )
    }

    fn workspace_with_identity(
        path: &std::path::Path,
        identity: WorkspaceIdentity,
    ) -> AcquisitionWorkspace {
        let root = crate::platform::capability_fs::open_directory(path).unwrap();
        let check = root.try_clone().unwrap();
        let expected = std::fs::canonicalize(path).unwrap();
        let source = path.to_path_buf();
        AcquisitionWorkspace::from_capability(root, identity, Arc::new(()), move || {
            if std::fs::canonicalize(&source)? != expected || !check.dir_metadata()?.is_dir() {
                return Err(invalid("Fixture grant changed"));
            }
            Ok(())
        })
        .unwrap()
    }

    struct Host;
    #[async_trait::async_trait]
    impl HttpAttemptHost for Host {
        async fn pause_requested(&self) {
            std::future::pending::<()>().await;
        }
        fn pause_requested_now(&self) -> bool {
            false
        }
        fn cancel_requested(&self) -> bool {
            false
        }
        async fn record_progress(&mut self, _bytes: u64) -> Result<()> {
            Ok(())
        }
    }
    #[async_trait::async_trait]
    impl AcquisitionHost for Host {
        async fn retry(
            &mut self,
            _attempt: u32,
            _delay: Option<Duration>,
            _error: Option<&str>,
        ) -> Result<()> {
            Ok(())
        }
    }

    #[derive(Clone, Default)]
    struct RecordingHost {
        retry_notifications: Arc<Mutex<Vec<Option<String>>>>,
        progress_bytes: Arc<Mutex<Vec<u64>>>,
    }

    struct AbortOnDrop<T> {
        handle: Option<tokio::task::JoinHandle<T>>,
    }

    impl<T> AbortOnDrop<T> {
        fn new(handle: tokio::task::JoinHandle<T>) -> Self {
            Self {
                handle: Some(handle),
            }
        }

        async fn join_with_timeout(mut self, timeout: Duration) -> std::result::Result<T, ()> {
            let handle = self.handle.as_mut().ok_or(())?;
            let result = match tokio::time::timeout(timeout, &mut *handle).await {
                Ok(Ok(output)) => Ok(output),
                Ok(Err(_)) => Err(()),
                Err(_) => {
                    handle.abort();
                    let _ = tokio::time::timeout(timeout, &mut *handle).await;
                    Err(())
                }
            };
            self.handle.take();
            result
        }
    }

    impl<T> Drop for AbortOnDrop<T> {
        fn drop(&mut self) {
            if let Some(handle) = &self.handle {
                handle.abort();
            }
        }
    }

    #[async_trait::async_trait]
    impl HttpAttemptHost for RecordingHost {
        async fn pause_requested(&self) {
            std::future::pending::<()>().await;
        }
        fn pause_requested_now(&self) -> bool {
            false
        }
        fn cancel_requested(&self) -> bool {
            false
        }
        async fn record_progress(&mut self, bytes: u64) -> Result<()> {
            self.progress_bytes.lock().unwrap().push(bytes);
            Ok(())
        }
    }

    #[async_trait::async_trait]
    impl AcquisitionHost for RecordingHost {
        async fn retry(
            &mut self,
            _attempt: u32,
            _delay: Option<Duration>,
            error: Option<&str>,
        ) -> Result<()> {
            self.retry_notifications
                .lock()
                .unwrap()
                .push(error.map(str::to_owned));
            Ok(())
        }
    }

    #[derive(Clone)]
    struct ControlledHost {
        paused: Arc<AtomicBool>,
        cancelled: Arc<AtomicBool>,
        pause_wake: Arc<tokio::sync::Notify>,
        cancel_wake: Arc<tokio::sync::Notify>,
        progress: tokio::sync::watch::Sender<u64>,
    }

    struct ReservationDropProbe(Arc<AtomicBool>);

    impl Drop for ReservationDropProbe {
        fn drop(&mut self) {
            self.0.store(true, Ordering::SeqCst);
        }
    }

    impl ControlledHost {
        fn pause(&self) {
            self.paused.store(true, Ordering::Release);
            self.pause_wake.notify_waiters();
        }

        fn cancel(&self) {
            self.cancelled.store(true, Ordering::Release);
            self.cancel_wake.notify_waiters();
        }
    }

    #[async_trait::async_trait]
    impl HttpAttemptHost for ControlledHost {
        async fn pause_requested(&self) {
            loop {
                let paused = self.pause_wake.notified();
                tokio::pin!(paused);
                paused.as_mut().enable();
                let cancelled = self.cancel_wake.notified();
                tokio::pin!(cancelled);
                cancelled.as_mut().enable();
                if self.paused.load(Ordering::Acquire) || self.cancelled.load(Ordering::Acquire) {
                    return;
                }
                tokio::select! {
                    _ = paused => {},
                    _ = cancelled => {},
                }
            }
        }

        fn pause_requested_now(&self) -> bool {
            self.paused.load(Ordering::Acquire)
        }

        fn cancel_requested(&self) -> bool {
            self.cancelled.load(Ordering::Acquire)
        }

        async fn record_progress(&mut self, downloaded_for_file: u64) -> Result<()> {
            self.progress.send_replace(downloaded_for_file);
            Ok(())
        }
    }

    #[async_trait::async_trait]
    impl AcquisitionHost for ControlledHost {
        async fn retry(
            &mut self,
            _attempt: u32,
            _delay: Option<Duration>,
            _error: Option<&str>,
        ) -> Result<()> {
            Ok(())
        }
    }

    fn retry() -> AcquisitionRetryPolicy {
        AcquisitionRetryPolicy {
            attempts: Some(1),
            elapsed: Duration::from_secs(5),
            backoff: crate::network::RetryConfig::new(),
        }
    }

    #[derive(Clone, Copy, PartialEq, Eq)]
    enum HttpBudgetFixture {
        Headers,
        Body,
        Backoff,
        RetryHeaders,
        HeldWrite,
        ExpiredBeforeBody,
        Zero,
        Happy,
        Cancel,
    }

    struct HttpBudgetHost {
        controls: ControlledHost,
        backoff: Arc<AtomicBool>,
    }

    #[async_trait::async_trait]
    impl HttpAttemptHost for HttpBudgetHost {
        async fn pause_requested(&self) {
            self.controls.pause_requested().await;
        }
        fn pause_requested_now(&self) -> bool {
            self.controls.pause_requested_now()
        }
        fn cancel_requested(&self) -> bool {
            self.controls.cancel_requested()
        }
        async fn record_progress(&mut self, bytes: u64) -> Result<()> {
            self.controls.record_progress(bytes).await
        }
    }

    #[async_trait::async_trait]
    impl AcquisitionHost for HttpBudgetHost {
        async fn retry(
            &mut self,
            _attempt: u32,
            delay: Option<Duration>,
            _error: Option<&str>,
        ) -> Result<()> {
            if delay.is_some() {
                self.backoff.store(true, Ordering::SeqCst);
            }
            Ok(())
        }
    }

    async fn http_budget_fixture(mode: HttpBudgetFixture) {
        use HttpBudgetFixture::*;
        let guard = Duration::from_secs(5);
        let temp = tempfile::TempDir::new().unwrap();
        let stage = temp.path().join("stage");
        std::fs::create_dir(&stage).unwrap();
        let store = Arc::new(AcquisitionStore::new(temp.path()));
        let service = Arc::new(AcquisitionService::new(store.clone()));
        let consumer = Arc::new(service.open_consumer("fixture").unwrap());
        let reservation_dropped = Arc::new(AtomicBool::new(false));
        let grant = AcquisitionWorkspace::from_capability(
            crate::platform::capability_fs::open_directory(&stage).unwrap(),
            workspace(&stage).identity().clone(),
            Arc::new(ReservationDropProbe(reservation_dropped.clone())),
            || Ok(()),
        )
        .unwrap();
        let count = if mode == Happy { 3 } else { 1 };
        let selected = ArtifactManifest::new(
            manifest("payload.bin").source().clone(),
            (0..count)
                .map(|i| {
                    ArtifactFile::new(
                        format!("payload-{i}.bin"),
                        format!("source-{i}"),
                        Some(4),
                        Some(
                            super::super::Sha256Evidence::new(
                                "fixture.sha256",
                                hex::encode(Sha256::digest(b"DATA")),
                            )
                            .unwrap(),
                        ),
                        FileVerificationRequirement::Sha256,
                    )
                    .unwrap()
                })
                .collect(),
        )
        .unwrap();
        let expected_manifest = selected.clone();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!(
            "http://{}/fixture?seed=seed-http-budget-secret",
            listener.local_addr().unwrap()
        );
        let requests = Arc::new(AtomicUsize::new(0));
        let seen = requests.clone();
        let (stop_tx, mut stop_rx) = tokio::sync::oneshot::channel();
        let (happy_tx, mut happy_rx) = tokio::sync::mpsc::channel::<()>(1);
        let (first_response_tx, mut first_response_rx) = tokio::sync::oneshot::channel();
        let mut first_response_tx = Some(first_response_tx);
        let mut source = tokio::spawn(async move {
            loop {
                let (mut socket, _) = tokio::select! {
                    biased;
                    _ = &mut stop_rx => return Ok::<(), PumasError>(()),
                    accepted = listener.accept() => accepted?,
                };
                let mut request = [0_u8; 2048];
                let read = tokio::select! {
                    biased;
                    _ = &mut stop_rx => return Ok(()),
                    read = socket.read(&mut request) => read?,
                };
                if read == 0 {
                    continue;
                }
                let attempt = seen.fetch_add(1, Ordering::SeqCst) + 1;
                if mode == RetryHeaders && attempt == 1 {
                    tokio::select! {
                        _ = &mut stop_rx => return Ok(()),
                        _ = &mut first_response_rx => {},
                    }
                }
                if mode == Backoff || (mode == RetryHeaders && attempt == 1) {
                    socket.write_all(b"HTTP/1.1 503 Service Unavailable\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").await?;
                    continue;
                }
                if matches!(mode, Headers | RetryHeaders | Cancel) {
                    let _ = stop_rx.await;
                    return Ok(());
                }
                socket.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 4\r\nETag: \"fixture-v1\"\r\nConnection: close\r\n\r\n").await?;
                if mode == Happy {
                    tokio::select! {
                        _ = &mut stop_rx => return Ok(()),
                        release = happy_rx.recv() => {
                            if release.is_none() { return Ok(()); }
                        },
                    }
                    socket.write_all(b"DATA").await?;
                } else {
                    socket.write_all(b"DA").await?;
                    let _ = stop_rx.await;
                    // An expired attempt has closed its body; zero-budget use
                    // still receives its exact remainder after gate release.
                    let _ = socket.write_all(b"TA").await;
                    return Ok(());
                }
            }
        });
        let write_started = Arc::new(AtomicBool::new(false));
        let write_expired = Arc::new(AtomicBool::new(false));
        let (release_write_tx, release_write_rx) = std::sync::mpsc::channel();
        let release_write_rx = Mutex::new(release_write_rx);
        if matches!(mode, HeldWrite | ExpiredBeforeBody) {
            let started = write_started.clone();
            let expired = write_expired.clone();
            consumer
                .scope
                .set_blocking_observer(Some(Arc::new(move |label| {
                    if label
                        == if mode == HeldWrite {
                            "write acquisition partial file"
                        } else {
                            "open checked acquisition partial file"
                        }
                    {
                        started.store(true, Ordering::SeqCst);
                        if release_write_rx
                            .lock()
                            .unwrap()
                            .recv_timeout(Duration::from_secs(10))
                            .is_err()
                        {
                            expired.store(true, Ordering::SeqCst);
                        }
                    }
                })));
        }
        let (progress_tx, progress) = tokio::sync::watch::channel(0_u64);
        let controls = ControlledHost {
            paused: Arc::new(AtomicBool::new(false)),
            cancelled: Arc::new(AtomicBool::new(false)),
            pause_wake: Arc::new(tokio::sync::Notify::new()),
            cancel_wake: Arc::new(tokio::sync::Notify::new()),
            progress: progress_tx,
        };
        let backoff = Arc::new(AtomicBool::new(false));
        let callbacks = Arc::new(AtomicUsize::new(0));
        let prepared = callbacks.clone();
        let published = callbacks.clone();
        let mut policy = retry();
        policy.attempts = None;
        if mode == Zero {
            policy.elapsed = Duration::ZERO;
        }
        policy.backoff = crate::network::RetryConfig::new()
            .with_jitter(false)
            .with_base_delay(if mode == Backoff {
                Duration::from_secs(10)
            } else {
                Duration::ZERO
            });
        let running = consumer.clone();
        let host = HttpBudgetHost {
            controls: controls.clone(),
            backoff: backoff.clone(),
        };
        let mut transfer = tokio::spawn(async move {
            running
                .acquire_http(
                    AcquisitionHttpRequest {
                        demand: AcquisitionDemand {
                            consumer: "fixture".into(),
                            operation: "http-budget".into(),
                        },
                        manifest: selected,
                        workspace: grant,
                        sources: (0..count)
                            .map(|_| AcquisitionHttpSource {
                                url: url.clone(),
                                authorization: None,
                            })
                            .collect(),
                        retry: policy,
                    },
                    reqwest::Client::new(),
                    Box::new(host),
                    move |_| async move {
                        prepared.fetch_add(1, Ordering::SeqCst);
                        Ok(((), Value::Null))
                    },
                    move |(), _| async move {
                        published.fetch_add(1, Ordering::SeqCst);
                        Ok(())
                    },
                )
                .await
        });
        // Observe the actual request/effect before advancing Tokio's clock;
        // automatic paused-clock advancement must not race socket setup.
        let setup = tokio::time::timeout(guard, async {
            while requests.load(Ordering::SeqCst) == 0 {
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
            if mode == RetryHeaders {
                tokio::time::pause();
                tokio::time::advance(Duration::from_secs(4)).await;
                tokio::time::resume();
                let _ = first_response_tx.take().unwrap().send(());
                while requests.load(Ordering::SeqCst) < 2 {
                    tokio::time::sleep(Duration::from_millis(5)).await;
                }
            }
            while (matches!(mode, Body | Zero) && *progress.borrow() < 2)
                || (matches!(mode, HeldWrite | ExpiredBeforeBody)
                    && !write_started.load(Ordering::SeqCst))
                || (mode == Backoff && !backoff.load(Ordering::SeqCst))
            {
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await;
        let mut happy_setup = true;
        if setup.is_ok() && mode == Happy {
            for member in 1..=count {
                // Each file uses 3s of its own 5s budget. The complete set
                // takes over 9s, rejecting a whole-request deadline mutant.
                let accepted = tokio::time::timeout(guard, async {
                    while requests.load(Ordering::SeqCst) < member {
                        tokio::time::sleep(Duration::from_millis(5)).await;
                    }
                })
                .await;
                if accepted.is_err() {
                    happy_setup = false;
                    break;
                }
                tokio::time::pause();
                tokio::time::advance(Duration::from_secs(3)).await;
                tokio::time::resume();
                if happy_tx.send(()).await.is_err() {
                    happy_setup = false;
                    break;
                }
            }
        }
        let mut result = None;
        let mut effect_pending = false;
        let mut reservation_held = false;
        if setup.is_ok() && mode != Happy {
            if mode == Cancel {
                controls.cancel();
            }
            tokio::time::pause();
            tokio::time::advance(Duration::from_secs(if mode == RetryHeaders {
                2
            } else {
                6
            }))
            .await;
            tokio::time::resume();
            if matches!(mode, Zero | HeldWrite | ExpiredBeforeBody) {
                match tokio::time::timeout(Duration::from_millis(30), &mut transfer).await {
                    Ok(value) => result = Some(value),
                    Err(_) => effect_pending = true,
                }
                reservation_held = !reservation_dropped.load(Ordering::SeqCst);
            }
        }
        // Always release the owned write and source gates before assertions.
        let _ = release_write_tx.send(());
        let mut stop_tx = Some(stop_tx);
        if mode == Zero {
            let _ = stop_tx.take().unwrap().send(());
        }
        if result.is_none() {
            result = tokio::time::timeout(Duration::from_secs(1), &mut transfer)
                .await
                .ok();
        }
        let finished_in_budget = result.is_some();
        if result.is_none() {
            controls.cancel();
            result = tokio::time::timeout(guard, &mut transfer).await.ok();
            if result.is_none() {
                transfer.abort();
                let _ = transfer.await;
            }
        }
        if let Some(stop) = stop_tx {
            let _ = stop.send(());
        }
        if let Some(first) = first_response_tx {
            let _ = first.send(());
        }
        consumer.scope.set_blocking_observer(None);
        let consumer_shutdown = tokio::time::timeout(guard, consumer.shutdown()).await;
        let service_shutdown = tokio::time::timeout(guard, service.shutdown()).await;
        let source_result = match tokio::time::timeout(guard, &mut source).await {
            Ok(value) => Some(value),
            Err(_) => {
                source.abort();
                let _ = source.await;
                None
            }
        };
        if !matches!(consumer_shutdown, Ok(Ok(()))) || !matches!(service_shutdown, Ok(Ok(()))) {
            eprintln!(
                "retained undrained HTTP budget fixture: {}",
                temp.keep().display()
            );
        }
        assert!(
            happy_setup,
            "multi-file source did not finish within per-file budgets"
        );
        assert!(
            setup.is_ok(),
            "fixture never reached its requested source/effect hold"
        );
        assert!(matches!(consumer_shutdown, Ok(Ok(()))));
        assert!(matches!(service_shutdown, Ok(Ok(()))));
        assert!(matches!(source_result, Some(Ok(Ok(())))));
        assert!(
            finished_in_budget,
            "HTTP source wait outlived its explicit elapsed budget"
        );
        assert!(!write_expired.load(Ordering::SeqCst));
        let result = result
            .expect("bounded transfer must return")
            .expect("transfer task must join");
        let records = store.acquisitions().unwrap();
        assert_eq!(records.len(), 1);
        let record = records.values().next().unwrap();
        assert_eq!(record.manifest, expected_manifest);
        if matches!(mode, Happy | Zero) {
            result.unwrap();
            assert_eq!(callbacks.load(Ordering::SeqCst), 2);
            assert!(matches!(record.phase, AcquisitionPhase::Adopted { .. }));
            let receipt = store.consumer_receipt(record.id).unwrap().unwrap();
            let AcquisitionPhase::Adopted { lease } = record.phase else {
                unreachable!()
            };
            receipt.validate_for_record(record, lease).unwrap();
            assert_eq!(receipt.verified_files.len(), count);
            for i in 0..count {
                assert_eq!(
                    std::fs::read(stage.join(format!("payload-{i}.bin"))).unwrap(),
                    b"DATA"
                );
            }
        } else {
            if mode == Cancel {
                assert!(matches!(result, Err(PumasError::DownloadCancelled)));
            } else {
                assert!(
                    matches!(result, Err(PumasError::DownloadFailed { .. })),
                    "{result:?}"
                );
                let diagnostic = result.unwrap_err().to_string();
                assert!(
                    !diagnostic.contains("seed-http-budget-secret") && !diagnostic.contains("S3")
                );
            }
            assert_eq!(callbacks.load(Ordering::SeqCst), 0);
            assert!(matches!(record.phase, AcquisitionPhase::Transferring));
            assert!(record.files.is_empty());
            assert!(store.consumer_receipt(record.id).unwrap().is_none());
            assert!(!stage.join("payload-0.bin").exists());
            if mode == ExpiredBeforeBody {
                assert_eq!(
                    std::fs::metadata(stage.join("payload-0.bin.part"))
                        .unwrap()
                        .len(),
                    0,
                    "expired budget polled an already-buffered body and wrote new bytes"
                );
            }
            if matches!(mode, Body | HeldWrite) {
                assert_eq!(
                    std::fs::read(stage.join("payload-0.bin.part")).unwrap(),
                    b"DA"
                );
            }
        }
        if matches!(mode, Zero | HeldWrite | ExpiredBeforeBody) {
            assert!(
                effect_pending && reservation_held,
                "elapsed budget released live consumer custody"
            );
        }
        assert_eq!(
            requests.load(Ordering::SeqCst),
            if mode == RetryHeaders { 2 } else { count }
        );
    }

    #[tokio::test]
    async fn http_elapsed_budget_stops_stalled_headers() {
        http_budget_fixture(HttpBudgetFixture::Headers).await;
    }
    #[tokio::test]
    async fn http_elapsed_budget_stops_stalled_partial_body() {
        http_budget_fixture(HttpBudgetFixture::Body).await;
    }
    #[tokio::test]
    async fn http_elapsed_budget_caps_backoff_without_another_request() {
        http_budget_fixture(HttpBudgetFixture::Backoff).await;
    }
    #[tokio::test]
    async fn http_elapsed_budget_is_not_reset_after_a_retry() {
        http_budget_fixture(HttpBudgetFixture::RetryHeaders).await;
    }
    #[tokio::test]
    async fn http_elapsed_budget_drains_a_held_write_before_returning() {
        http_budget_fixture(HttpBudgetFixture::HeldWrite).await;
    }
    #[tokio::test]
    async fn http_elapsed_budget_does_not_write_buffered_body_after_expiry() {
        http_budget_fixture(HttpBudgetFixture::ExpiredBeforeBody).await;
    }

    #[tokio::test]
    async fn http_elapsed_budget_zero_preserves_gated_success() {
        http_budget_fixture(HttpBudgetFixture::Zero).await;
    }
    #[tokio::test]
    async fn http_elapsed_budget_preserves_exact_multifile_receipt() {
        http_budget_fixture(HttpBudgetFixture::Happy).await;
    }
    #[tokio::test]
    async fn http_elapsed_budget_preserves_cancellation() {
        http_budget_fixture(HttpBudgetFixture::Cancel).await;
    }

    #[tokio::test]
    async fn http_elapsed_budget_overflow_refuses_before_admission() {
        let guard = Duration::from_secs(5);
        let temp = tempfile::TempDir::new().unwrap();
        let state_path = temp.path().join("downloads.json");
        let stage = temp.path().join("stage");
        std::fs::create_dir(&stage).unwrap();
        let store = Arc::new(AcquisitionStore::new(temp.path()));
        let service = Arc::new(AcquisitionService::new(store.clone()));
        let consumer = service.open_consumer("fixture").unwrap();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}/overflow", listener.local_addr().unwrap());
        let requests = Arc::new(AtomicUsize::new(0));
        let seen = requests.clone();
        let (stop_tx, mut stop_rx) = tokio::sync::oneshot::channel();
        let mut source = tokio::spawn(async move {
            loop {
                let (mut socket, _) = tokio::select! {
                    biased;
                    _ = &mut stop_rx => return Ok::<(), PumasError>(()),
                    accepted = listener.accept() => accepted?,
                };
                seen.fetch_add(1, Ordering::SeqCst);
                let mut request = [0_u8; 2048];
                let read = tokio::select! {
                    _ = &mut stop_rx => return Ok(()),
                    read = socket.read(&mut request) => read?,
                };
                if read > 0 {
                    socket.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 4\r\nConnection: close\r\n\r\nDATA").await?;
                }
            }
        });
        let callbacks = Arc::new(AtomicUsize::new(0));
        let prepared = callbacks.clone();
        let published = callbacks.clone();
        let mut policy = retry();
        policy.elapsed = Duration::MAX;
        let result = tokio::time::timeout(
            guard,
            consumer.acquire_http(
                AcquisitionHttpRequest {
                    demand: AcquisitionDemand {
                        consumer: "fixture".into(),
                        operation: "overflow".into(),
                    },
                    manifest: manifest("payload.bin"),
                    workspace: workspace(&stage),
                    sources: vec![AcquisitionHttpSource {
                        url,
                        authorization: None,
                    }],
                    retry: policy,
                },
                reqwest::Client::new(),
                Box::new(Host),
                move |_| async move {
                    prepared.fetch_add(1, Ordering::SeqCst);
                    Ok(((), Value::Null))
                },
                move |(), _| async move {
                    published.fetch_add(1, Ordering::SeqCst);
                    Ok(())
                },
            ),
        )
        .await;
        let consumer_shutdown = tokio::time::timeout(guard, consumer.shutdown()).await;
        let service_shutdown = tokio::time::timeout(guard, service.shutdown()).await;
        // Keep the monitor through owner drainage and a bounded quiet window.
        tokio::time::sleep(Duration::from_millis(30)).await;
        let _ = stop_tx.send(());
        let source_result = match tokio::time::timeout(guard, &mut source).await {
            Ok(value) => Some(value),
            Err(_) => {
                source.abort();
                let _ = source.await;
                None
            }
        };
        if !matches!(consumer_shutdown, Ok(Ok(()))) || !matches!(service_shutdown, Ok(Ok(()))) {
            eprintln!(
                "retained undrained overflow fixture: {}",
                temp.keep().display()
            );
        }
        assert!(matches!(result, Ok(Err(PumasError::Validation { .. }))));
        assert!(matches!(consumer_shutdown, Ok(Ok(()))));
        assert!(matches!(service_shutdown, Ok(Ok(()))));
        assert!(matches!(source_result, Some(Ok(Ok(())))));
        assert_eq!(requests.load(Ordering::SeqCst), 0);
        assert_eq!(callbacks.load(Ordering::SeqCst), 0);
        assert!(store.acquisitions().unwrap().is_empty());
        assert_eq!(std::fs::read_dir(&stage).unwrap().count(), 0);
        assert!(!state_path.exists());
    }

    async fn serve(body: &'static [u8]) -> (String, tokio::task::JoinHandle<()>) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}/fixture", listener.local_addr().unwrap());
        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut request = [0_u8; 2048];
            let received = socket.read(&mut request).await.unwrap();
            assert!(received > 0);
            socket
                .write_all(
                    format!(
                        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        body.len()
                    )
                    .as_bytes(),
                )
                .await
                .unwrap();
            socket.write_all(body).await.unwrap();
        });
        (url, server)
    }

    async fn serve_after_gate(
        first: &'static [u8],
        remainder: &'static [u8],
    ) -> (
        String,
        tokio::task::JoinHandle<()>,
        tokio::sync::oneshot::Receiver<()>,
        tokio::sync::oneshot::Sender<()>,
    ) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}/fixture", listener.local_addr().unwrap());
        let (first_sent_sender, first_sent_receiver) = tokio::sync::oneshot::channel();
        let (continue_sender, continue_receiver) = tokio::sync::oneshot::channel();
        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut request = [0_u8; 2048];
            let received = socket.read(&mut request).await.unwrap();
            assert!(received > 0);
            let total = first.len() + remainder.len();
            socket
                .write_all(
                    format!(
                        "HTTP/1.1 200 OK\r\nContent-Length: {total}\r\nETag: \"fixture-v1\"\r\nConnection: close\r\n\r\n"
                    )
                    .as_bytes(),
                )
                .await
                .unwrap();
            socket.write_all(first).await.unwrap();
            let _ = first_sent_sender.send(());
            if continue_receiver.await.is_ok() {
                let _ = socket.write_all(remainder).await;
            }
        });
        (url, server, first_sent_receiver, continue_sender)
    }

    async fn controlled_partial_transfer_stops_writing(paused: bool) {
        let temp = tempfile::TempDir::new().unwrap();
        let stage = temp.path().join("stage");
        std::fs::create_dir(&stage).unwrap();
        let store = Arc::new(AcquisitionStore::new(temp.path()));
        let service = Arc::new(AcquisitionService::new(store.clone()));
        let consumer = Arc::new(service.open_consumer("fixture").unwrap());
        let demand = AcquisitionDemand {
            consumer: "fixture".into(),
            operation: if paused {
                "paused-transfer".into()
            } else {
                "cancelled-transfer".into()
            },
        };
        let expected_demand = demand.clone();
        let request_manifest = manifest_with_size("payload.bin", 8);
        let expected_manifest = request_manifest.clone();
        let request_workspace = workspace(&stage);
        let expected_workspace = request_workspace.identity().clone();
        let (url, server, first_sent, continue_sender) = serve_after_gate(b"DATA", b"TAIL").await;
        let mut continue_sender = Some(continue_sender);
        let (progress_sender, mut progress) = tokio::sync::watch::channel(0_u64);
        let host = ControlledHost {
            paused: Arc::new(AtomicBool::new(false)),
            cancelled: Arc::new(AtomicBool::new(false)),
            pause_wake: Arc::new(tokio::sync::Notify::new()),
            cancel_wake: Arc::new(tokio::sync::Notify::new()),
            progress: progress_sender,
        };
        let controls = host.clone();
        let transfer_consumer = consumer.clone();
        let mut transfer = tokio::spawn(async move {
            transfer_consumer
                .acquire_http(
                    AcquisitionHttpRequest {
                        demand,
                        manifest: request_manifest,
                        workspace: request_workspace,
                        sources: vec![AcquisitionHttpSource {
                            url,
                            authorization: None,
                        }],
                        retry: retry(),
                    },
                    reqwest::Client::new(),
                    Box::new(host),
                    |_| async move { Ok::<((), Value), PumasError>(((), Value::Null)) },
                    |(), _receipt| async move { Ok::<(), PumasError>(()) },
                )
                .await
        });

        let progress_result = tokio::time::timeout(Duration::from_secs(2), async {
            first_sent
                .await
                .map_err(|_| "HTTP fixture stopped before sending the first chunk")?;
            while *progress.borrow_and_update() < 4 {
                progress
                    .changed()
                    .await
                    .map_err(|_| "transfer ended before reporting the first chunk")?;
            }
            Ok::<(), &'static str>(())
        })
        .await;
        let progress_failure = match progress_result {
            Ok(Ok(())) => None,
            Ok(Err(message)) => Some(message),
            Err(_) => Some("timed out waiting for the first chunk"),
        };
        if let Some(message) = progress_failure {
            controls.cancel();
            if let Some(sender) = continue_sender.take() {
                let _ = sender.send(());
            }
            if tokio::time::timeout(Duration::from_secs(2), &mut transfer)
                .await
                .is_err()
            {
                transfer.abort();
                let _ = transfer.await;
            }
            server.abort();
            let _ = server.await;
            panic!("controlled transfer setup failed: {message}");
        }
        let before_control = store.acquisitions().unwrap();
        assert_eq!(before_control.len(), 1);
        let exact_record = before_control.values().next().unwrap().clone();
        assert_eq!(exact_record.demand, expected_demand);
        assert_eq!(exact_record.manifest, expected_manifest);
        assert_eq!(exact_record.workspace, expected_workspace);
        assert!(matches!(exact_record.phase, AcquisitionPhase::Transferring));
        assert!(exact_record.files.is_empty());

        if paused {
            controls.pause();
        } else {
            controls.cancel();
        }
        let result = match tokio::time::timeout(Duration::from_secs(2), &mut transfer).await {
            Ok(Ok(result)) => result,
            Ok(Err(error)) => {
                if let Some(sender) = continue_sender.take() {
                    let _ = sender.send(());
                }
                server.abort();
                let _ = server.await;
                panic!("controlled transfer task failed: {error}");
            }
            Err(_) => {
                controls.cancel();
                if let Some(sender) = continue_sender.take() {
                    let _ = sender.send(());
                }
                transfer.abort();
                let _ = transfer.await;
                server.abort();
                let _ = server.await;
                panic!("controlled transfer did not observe pause/cancellation");
            }
        };
        if paused {
            assert!(matches!(result, Err(PumasError::DownloadPaused)));
        } else {
            assert!(matches!(result, Err(PumasError::DownloadCancelled)));
        }
        // Only release the final source bytes after the public operation has
        // returned from either control action; they must not extend its partial.
        continue_sender.take().unwrap().send(()).unwrap();
        server.await.unwrap();

        assert_eq!(
            std::fs::read(stage.join("payload.bin.part")).unwrap(),
            b"DATA"
        );
        assert!(!stage.join("payload.bin").exists());
        let records = store.acquisitions().unwrap();
        assert_eq!(records, before_control);
        assert!(store.consumer_receipt(exact_record.id).unwrap().is_none());
        let record = records.get(&exact_record.id).unwrap();
        assert!(matches!(record.phase, AcquisitionPhase::Transferring));
        assert!(record.files.is_empty());

        consumer.shutdown().await.unwrap();
        service.shutdown().await.unwrap();
        assert!(service.checkpoints.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn host_cancellation_during_file_set_sealing_prevents_consumer_handoff() {
        let timeout = Duration::from_secs(5);
        let temp = tempfile::TempDir::new().unwrap();
        let stage = temp.path().join("stage");
        std::fs::create_dir(&stage).unwrap();
        let store = Arc::new(AcquisitionStore::new(temp.path()));
        let service = Arc::new(AcquisitionService::new(store.clone()));
        let consumer = Arc::new(service.open_consumer("fixture").unwrap());
        let reservation_released = Arc::new(AtomicBool::new(false));
        let root = crate::platform::capability_fs::open_directory(&stage).unwrap();
        let grant = AcquisitionWorkspace::from_capability(
            root,
            WorkspaceIdentity {
                root_identity: "fixture-physical-root".into(),
                relative_target: "staging".into(),
            },
            Arc::new(ReservationDropProbe(reservation_released.clone())),
            || Ok(()),
        )
        .unwrap();
        let expected_workspace = grant.identity().clone();
        let demand = AcquisitionDemand {
            consumer: "fixture".into(),
            operation: "cancel-during-file-set-seal".into(),
        };
        let expected_demand = demand.clone();
        let request_manifest = manifest("payload.bin");
        let expected_manifest = request_manifest.clone();
        let (url, server) = serve(b"DATA").await;
        let (progress_sender, _progress) = tokio::sync::watch::channel(0_u64);
        let host = ControlledHost {
            paused: Arc::new(AtomicBool::new(false)),
            cancelled: Arc::new(AtomicBool::new(false)),
            pause_wake: Arc::new(tokio::sync::Notify::new()),
            cancel_wake: Arc::new(tokio::sync::Notify::new()),
            progress: progress_sender,
        };
        let controls = host.clone();

        let (seal_started_sender, seal_started) = tokio::sync::oneshot::channel();
        let seal_started_sender = Mutex::new(Some(seal_started_sender));
        let (release_seal_sender, release_seal_receiver) = std::sync::mpsc::channel();
        let release_seal_receiver = Mutex::new(release_seal_receiver);
        let (seal_released_sender, seal_released) = tokio::sync::oneshot::channel();
        let seal_released_sender = Mutex::new(Some(seal_released_sender));
        let ready_mutation_started = Arc::new(AtomicBool::new(false));
        let ready_mutation_observer = ready_mutation_started.clone();
        let handoff_mutation_started = Arc::new(AtomicBool::new(false));
        let handoff_mutation_observer = handoff_mutation_started.clone();
        consumer
            .scope
            .set_blocking_observer(Some(Arc::new(move |label| match label {
                "seal verified acquisition file set" => {
                    if let Some(sender) = seal_started_sender.lock().unwrap().take() {
                        let _ = sender.send(());
                    }
                    let _ = release_seal_receiver.lock().unwrap().recv();
                    if let Some(sender) = seal_released_sender.lock().unwrap().take() {
                        let _ = sender.send(());
                    }
                }
                "persist acquisition files ready" => {
                    ready_mutation_observer.store(true, Ordering::SeqCst);
                }
                "handoff durable verified acquisition files" => {
                    handoff_mutation_observer.store(true, Ordering::SeqCst);
                }
                _ => {}
            })));

        let prepare_calls = Arc::new(AtomicUsize::new(0));
        let prepare_observer = prepare_calls.clone();
        let publish_calls = Arc::new(AtomicUsize::new(0));
        let publish_observer = publish_calls.clone();
        let running_consumer = consumer.clone();
        let waiter = tokio::spawn(async move {
            running_consumer
                .acquire_http(
                    AcquisitionHttpRequest {
                        demand,
                        manifest: request_manifest,
                        workspace: grant,
                        sources: vec![AcquisitionHttpSource {
                            url,
                            authorization: None,
                        }],
                        retry: retry(),
                    },
                    reqwest::Client::new(),
                    Box::new(host),
                    move |_| async move {
                        prepare_observer.fetch_add(1, Ordering::SeqCst);
                        Ok::<((), Value), PumasError>(((), Value::Null))
                    },
                    move |(), _receipt| async move {
                        publish_observer.fetch_add(1, Ordering::SeqCst);
                        Ok::<(), PumasError>(())
                    },
                )
                .await
        });

        tokio::time::timeout(timeout, seal_started)
            .await
            .expect("public consumer must reach its registered seal effect")
            .expect("seal observer must remain connected");
        let before_cancel = store.acquisitions().unwrap();
        assert_eq!(before_cancel.len(), 1);
        let record = before_cancel.values().next().unwrap().clone();
        assert_eq!(record.demand, expected_demand);
        assert_eq!(record.manifest, expected_manifest);
        assert_eq!(record.workspace, expected_workspace);
        assert_eq!(record.phase, AcquisitionPhase::Transferring);
        assert!(record.files.is_empty());
        assert!(store.consumer_receipt(record.id).unwrap().is_none());
        assert_eq!(std::fs::read(stage.join("payload.bin")).unwrap(), b"DATA");
        assert!(!stage.join("payload.bin.part").exists());
        assert!(!reservation_released.load(Ordering::SeqCst));
        assert_eq!(prepare_calls.load(Ordering::SeqCst), 0);
        assert_eq!(publish_calls.load(Ordering::SeqCst), 0);

        controls.cancel();
        assert!(!reservation_released.load(Ordering::SeqCst));
        release_seal_sender.send(()).unwrap();
        let result = tokio::time::timeout(timeout, waiter)
            .await
            .expect("cancelled public consumer must drain its seal effect")
            .expect("consumer task must not panic");
        assert!(matches!(result, Err(PumasError::DownloadCancelled)));
        tokio::time::timeout(timeout, seal_released)
            .await
            .expect("seal barrier must be released")
            .expect("seal release observer must remain connected");
        tokio::time::timeout(timeout, server)
            .await
            .expect("local HTTP source must finish")
            .expect("local HTTP source must not panic");

        assert_eq!(store.acquisitions().unwrap(), before_cancel);
        assert!(store.consumer_receipt(record.id).unwrap().is_none());
        assert_eq!(std::fs::read(stage.join("payload.bin")).unwrap(), b"DATA");
        assert!(!ready_mutation_started.load(Ordering::SeqCst));
        assert!(!handoff_mutation_started.load(Ordering::SeqCst));
        assert_eq!(prepare_calls.load(Ordering::SeqCst), 0);
        assert_eq!(publish_calls.load(Ordering::SeqCst), 0);
        assert!(reservation_released.load(Ordering::SeqCst));

        consumer.scope.set_blocking_observer(None);
        tokio::time::timeout(timeout, consumer.shutdown())
            .await
            .expect("consumer shutdown must drain")
            .unwrap();
        tokio::time::timeout(timeout, service.shutdown())
            .await
            .expect("service shutdown must drain")
            .unwrap();
    }

    #[tokio::test]
    async fn promoted_digestless_file_reopens_by_fresh_http_comparison_and_retains_using() {
        let temp = tempfile::TempDir::new().unwrap();
        let stage = temp.path().join("stage");
        std::fs::create_dir(&stage).unwrap();
        let store = Arc::new(AcquisitionStore::new(temp.path()));
        let service = Arc::new(AcquisitionService::new(store));
        let scope = service
            .supervisor()
            .open_scope(|| async { Ok(()) })
            .unwrap();
        let demand = AcquisitionDemand {
            consumer: "fixture".into(),
            operation: "exact-demand".into(),
        };
        let first_service = service.clone();
        let first_stage = stage.clone();
        let first_demand = demand.clone();
        let (url, server) = serve(b"DATA").await;
        scope
            .run_invocation(move |context| async move {
                let grant = workspace(&first_stage);
                let operation = first_service
                    .begin(
                        &context,
                        first_demand,
                        manifest("payload.bin"),
                        grant.identity().clone(),
                        None,
                    )
                    .await?;
                first_service
                    .acquire_file(
                        &context,
                        &operation,
                        &grant,
                        0,
                        &reqwest::Client::new().into(),
                        &url,
                        None,
                        &retry(),
                        &mut Host,
                    )
                    .await?;
                Ok(())
            })
            .await
            .unwrap();
        server.await.unwrap();
        scope.shutdown().await.unwrap();
        assert!(matches!(
            service
                .store
                .acquisitions()
                .unwrap()
                .values()
                .next()
                .unwrap()
                .phase,
            AcquisitionPhase::Transferring
        ));
        drop(service);

        // A promoted file has no durable receipt yet: reopen must obtain a fresh
        // immutable-source representation instead of trusting path/size/tag.
        let reopened = Arc::new(AcquisitionService::new(Arc::new(AcquisitionStore::new(
            temp.path(),
        ))));
        let scope = reopened
            .supervisor()
            .open_scope(|| async { Ok(()) })
            .unwrap();
        let owner = reopened.clone();
        let use_stage = stage.clone();
        let use_demand = demand.clone();
        let (url, server) = serve(b"DATA").await;
        scope
            .run_invocation(move |context| async move {
                let grant = workspace(&use_stage);
                let operation = owner
                    .begin(
                        &context,
                        use_demand,
                        manifest("payload.bin"),
                        grant.identity().clone(),
                        None,
                    )
                    .await?;
                owner
                    .acquire_file(
                        &context,
                        &operation,
                        &grant,
                        0,
                        &reqwest::Client::new().into(),
                        &url,
                        None,
                        &retry(),
                        &mut Host,
                    )
                    .await?;
                let lease = owner.files_ready(&context, operation, grant).await?;
                drop(lease); // Simulate loss before the consumer can acknowledge.
                Ok(())
            })
            .await
            .unwrap();
        server.await.unwrap();
        scope.shutdown().await.unwrap();
        drop(reopened);
        let owner = Arc::new(AcquisitionService::new(Arc::new(AcquisitionStore::new(
            temp.path(),
        ))));
        let scope = owner.supervisor().open_scope(|| async { Ok(()) }).unwrap();
        let current = owner.clone();
        let before = std::fs::read(temp.path().join("downloads.json")).unwrap();
        let result = scope
            .run_invocation(move |context| async move {
                let grant = workspace(&stage);
                let evidence = current.reconciliation_lease(&context, &demand).await?;
                current
                    .begin(
                        &context,
                        demand,
                        manifest("payload.bin"),
                        grant.identity().clone(),
                        evidence,
                    )
                    .await
                    .map(|_| ())
            })
            .await;
        assert!(
            matches!(result, Err(PumasError::Validation { field, .. }) if field == "acquisition.consumer_recovery_required")
        );
        assert_eq!(
            std::fs::read(temp.path().join("downloads.json")).unwrap(),
            before
        );
        scope.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn adopted_proof_is_terminal_and_exact_withdrawal_does_not_release_a_successor() {
        let temp = tempfile::TempDir::new().unwrap();
        let stage = temp.path().join("stage");
        std::fs::create_dir(&stage).unwrap();
        std::fs::write(stage.join("payload.bin"), b"DATA").unwrap();
        let owner = Arc::new(AcquisitionService::new(Arc::new(AcquisitionStore::new(
            temp.path(),
        ))));
        let scope = owner.supervisor().open_scope(|| async { Ok(()) }).unwrap();
        let service = owner.clone();
        scope
            .run_invocation(move |context| async move {
                let grant = workspace(&stage);
                let demand = AcquisitionDemand {
                    consumer: "fixture".into(),
                    operation: "first".into(),
                };
                let operation = service
                    .begin(
                        &context,
                        demand.clone(),
                        manifest("payload.bin"),
                        grant.identity().clone(),
                        None,
                    )
                    .await?;
                let lease = service
                    .files_ready(&context, operation, grant.clone())
                    .await?;
                service.acknowledge(&context, lease).await?;
                let evidence = service.reconciliation_lease(&context, &demand).await?;
                let terminal = service
                    .begin(
                        &context,
                        demand.clone(),
                        manifest("payload.bin"),
                        grant.identity().clone(),
                        evidence,
                    )
                    .await?;
                assert!(terminal.is_adopted());
                let successor = AcquisitionDemand {
                    consumer: "fixture".into(),
                    operation: "successor".into(),
                };
                service
                    .begin(
                        &context,
                        successor.clone(),
                        manifest("payload.bin"),
                        grant.identity().clone(),
                        None,
                    )
                    .await?;
                service
                    .withdraw(&context, demand, grant.identity().clone())
                    .await?;
                assert!(matches!(
                    service
                        .store
                        .acquisitions()?
                        .values()
                        .find(|record| record.demand == successor)
                        .unwrap()
                        .phase,
                    AcquisitionPhase::Transferring
                ));
                service
                    .withdraw(&context, successor, grant.identity().clone())
                    .await?;
                Ok(())
            })
            .await
            .unwrap();
        scope.shutdown().await.unwrap();
        let records = owner.store.acquisitions().unwrap();
        assert_eq!(
            records
                .values()
                .filter(|record| matches!(record.phase, AcquisitionPhase::Adopted { .. }))
                .count(),
            1
        );
        assert_eq!(
            records
                .values()
                .filter(|record| matches!(record.phase, AcquisitionPhase::Withdrawn))
                .count(),
            1
        );
    }

    #[tokio::test]
    async fn repeated_exact_withdrawal_preserves_an_active_demand_in_a_distinct_workspace() {
        let temp = tempfile::TempDir::new().unwrap();
        let first_stage = temp.path().join("first-stage");
        let second_stage = temp.path().join("second-stage");
        std::fs::create_dir(&first_stage).unwrap();
        std::fs::create_dir(&second_stage).unwrap();
        let sentinel = second_stage.join("sentinel.bin");
        std::fs::write(&sentinel, b"second workspace sentinel").unwrap();
        let sentinel_before = std::fs::read(&sentinel).unwrap();
        let owner = Arc::new(AcquisitionService::new(Arc::new(AcquisitionStore::new(
            temp.path(),
        ))));
        let scope = owner.supervisor().open_scope(|| async { Ok(()) }).unwrap();
        let service = owner.clone();
        scope
            .run_invocation(move |context| async move {
                let first_grant = workspace_with_identity(
                    &first_stage,
                    WorkspaceIdentity {
                        root_identity: "fixture-physical-root".into(),
                        relative_target: "first-stage".into(),
                    },
                );
                let second_grant = workspace_with_identity(
                    &second_stage,
                    WorkspaceIdentity {
                        root_identity: "fixture-physical-root".into(),
                        relative_target: "second-stage".into(),
                    },
                );
                assert_ne!(first_grant.identity(), second_grant.identity());
                let first_demand = AcquisitionDemand {
                    consumer: "fixture".into(),
                    operation: "first".into(),
                };
                let second_demand = AcquisitionDemand {
                    consumer: "fixture".into(),
                    operation: "second".into(),
                };
                let first = service
                    .begin(
                        &context,
                        first_demand.clone(),
                        manifest("payload.bin"),
                        first_grant.identity().clone(),
                        None,
                    )
                    .await?;
                let second = service
                    .begin(
                        &context,
                        second_demand,
                        manifest("payload.bin"),
                        second_grant.identity().clone(),
                        None,
                    )
                    .await?;
                let second_before = service.store.acquisitions()?[&second.record.id].clone();
                assert_eq!(second_before.phase, AcquisitionPhase::Transferring);
                assert_eq!(&second_before.workspace, second_grant.identity());

                service
                    .withdraw(
                        &context,
                        first_demand.clone(),
                        first_grant.identity().clone(),
                    )
                    .await?;
                let after_first = service.store.acquisitions()?;
                let withdrawn = after_first[&first.record.id].clone();
                assert_eq!(withdrawn.demand, first_demand);
                assert_eq!(&withdrawn.workspace, first_grant.identity());
                assert_eq!(withdrawn.phase, AcquisitionPhase::Withdrawn);
                assert_eq!(after_first[&second.record.id], second_before);
                assert_eq!(std::fs::read(&sentinel)?, sentinel_before);

                service
                    .withdraw(&context, first_demand, first_grant.identity().clone())
                    .await?;
                let after_repeat = service.store.acquisitions()?;
                assert_eq!(after_repeat[&first.record.id], withdrawn);
                assert_eq!(after_repeat[&second.record.id], second_before);
                assert_eq!(std::fs::read(&sentinel)?, sentinel_before);
                Ok(())
            })
            .await
            .unwrap();
        scope.shutdown().await.unwrap();
    }

    async fn withdrawal_waits_for_consumer_effect(cleanup_fails: bool) {
        let temp = tempfile::TempDir::new().unwrap();
        let stage = temp.path().join("stage");
        std::fs::create_dir(&stage).unwrap();
        let store = Arc::new(AcquisitionStore::new(temp.path()));
        let service = Arc::new(AcquisitionService::new(store.clone()));
        let consumer = Arc::new(service.open_consumer("fixture").unwrap());
        let demand = AcquisitionDemand {
            consumer: "fixture".into(),
            operation: "held-consumer-effect".into(),
        };
        let (url, server) = serve(b"DATA").await;

        let (use_sender, use_receiver) = tokio::sync::oneshot::channel();
        let (continue_sender, continue_receiver) = tokio::sync::oneshot::channel();
        let (effect_started_sender, effect_started_receiver) = tokio::sync::oneshot::channel();
        let (release_effect_sender, release_effect_receiver) = std::sync::mpsc::channel();
        let consumer_task = consumer.clone();
        let acquisition_stage = stage.clone();
        let acquisition = tokio::spawn(async move {
            consumer_task
                .acquire_http(
                    AcquisitionHttpRequest {
                        demand,
                        manifest: manifest("payload.bin"),
                        workspace: workspace(&acquisition_stage),
                        sources: vec![AcquisitionHttpSource {
                            url,
                            authorization: None,
                        }],
                        retry: retry(),
                    },
                    reqwest::Client::new(),
                    Box::new(Host),
                    move |use_handle| async move {
                        // Register an effect under this exact worker generation, then
                        // hand the use lease to the cancellation cleanup under test.
                        let effect_context = use_handle.context.clone();
                        let effect = tokio::spawn(async move {
                            owned(&effect_context, "held consumer effect", move || {
                                let _ = effect_started_sender.send(());
                                release_effect_receiver.recv().map_err(|error| {
                                    PumasError::Other(format!(
                                        "Consumer effect gate closed: {error}"
                                    ))
                                })?;
                                Ok(())
                            })
                            .await
                        });
                        effect_started_receiver.await.map_err(|_| {
                            PumasError::Other("Consumer effect did not start".into())
                        })?;
                        use_sender.send(use_handle).map_err(|_| {
                            PumasError::Other("Cancellation observer disappeared".into())
                        })?;
                        continue_receiver.await.map_err(|_| {
                            PumasError::Other(
                                "Cancellation observer did not resume consumer".into(),
                            )
                        })?;
                        effect
                            .await
                            .map_err(|error| PumasError::Other(error.to_string()))??;
                        Err::<((), Value), PumasError>(PumasError::Other(
                            "Fixture consumer cancelled after cleanup".into(),
                        ))
                    },
                    |(), _receipt| async move { Ok::<(), PumasError>(()) },
                )
                .await
        });

        let use_handle = use_receiver.await.unwrap();
        let retained = use_handle.record().clone();
        assert!(matches!(retained.phase, AcquisitionPhase::Using { .. }));
        assert_eq!(std::fs::read(stage.join("payload.bin")).unwrap(), b"DATA");

        let cleanup_started = Arc::new(AtomicBool::new(false));
        let cleanup_started_in_callback = cleanup_started.clone();
        let cleanup_path = stage.join("payload.bin");
        let withdrawal = use_handle.withdraw_after_cleanup(move || {
            cleanup_started_in_callback.store(true, Ordering::Release);
            if cleanup_fails {
                return Err(PumasError::Other("Fixture cleanup failed".into()));
            }
            std::fs::remove_file(cleanup_path)
                .map_err(|error| PumasError::Other(error.to_string()))?;
            Ok(())
        });
        tokio::pin!(withdrawal);

        // Poll the real withdrawal path while the registered blocking effect is
        // gated. The future stays alive after the timeout so it can finish later.
        assert!(
            tokio::time::timeout(Duration::from_millis(50), &mut withdrawal)
                .await
                .is_err(),
            "withdrawal completed while a registered consumer effect was blocked"
        );
        assert!(!cleanup_started.load(Ordering::Acquire));
        assert_eq!(std::fs::read(stage.join("payload.bin")).unwrap(), b"DATA");
        let during_cleanup = store.acquisitions().unwrap();
        assert_eq!(during_cleanup.get(&retained.id), Some(&retained));

        release_effect_sender.send(()).unwrap();
        let withdrawal_result = withdrawal.await;
        assert!(cleanup_started.load(Ordering::Acquire));

        let after_cleanup = store.acquisitions().unwrap();
        let after_record = after_cleanup.get(&retained.id).unwrap();
        if cleanup_fails {
            assert!(withdrawal_result.is_err());
            assert_eq!(after_record, &retained);
            assert!(matches!(after_record.phase, AcquisitionPhase::Using { .. }));
            assert_eq!(std::fs::read(stage.join("payload.bin")).unwrap(), b"DATA");
        } else {
            withdrawal_result.unwrap();
            assert!(matches!(after_record.phase, AcquisitionPhase::Withdrawn));
            assert!(after_record.files.is_empty());
            assert!(!stage.join("payload.bin").exists());
        }

        continue_sender.send(()).unwrap();
        assert!(acquisition.await.unwrap().is_err());
        server.await.unwrap();
        let shutdown = consumer.shutdown().await;
        if cleanup_fails {
            assert!(matches!(
                shutdown,
                Err(PumasError::DownloadShutdownFailed { failures: 1 })
            ));
        } else {
            shutdown.unwrap();
        }
        let paused_stage = temp.path().join("unrelated-paused");
        std::fs::create_dir(&paused_stage).unwrap();
        let paused_workspace = workspace(&paused_stage);
        let (paused_record, paused_prefix) = checkpoint_fixture(&paused_workspace);
        assert!(retain_fixture(
            &service,
            &paused_workspace,
            &paused_record,
            0,
            &paused_prefix,
            "paused"
        ));
        let shutdown = service.shutdown().await;
        if cleanup_fails {
            assert!(matches!(
                shutdown,
                Err(PumasError::DownloadShutdownFailed { failures: 1 })
            ));
        } else {
            shutdown.unwrap();
        }
        assert_eq!(
            service.checkpoints.lock().unwrap().len(),
            usize::from(cleanup_fails)
        );
        assert_eq!(
            std::fs::read(paused_stage.join("payload.bin.part")).unwrap(),
            b"DATA"
        );
    }

    #[tokio::test]
    async fn cancelled_use_waiter_holds_workspace_lock_until_registered_effect_returns() {
        let timeout = Duration::from_secs(5);
        let temp = tempfile::TempDir::new().unwrap();
        let stage = temp.path().join("stage");
        std::fs::create_dir(&stage).unwrap();
        let lock_path = temp.path().join("workspace.lock");
        let held_lock = std::fs::File::create(&lock_path).unwrap();
        fs2::FileExt::try_lock_exclusive(&held_lock).unwrap();
        let contender = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(&lock_path)
            .unwrap();
        let directory = crate::platform::capability_fs::open_directory(&stage).unwrap();
        let grant = AcquisitionWorkspace::from_capability(
            directory,
            workspace(&stage).identity().clone(),
            Arc::new(held_lock),
            || Ok(()),
        )
        .unwrap();
        let service = Arc::new(AcquisitionService::new(Arc::new(AcquisitionStore::new(
            temp.path(),
        ))));
        let consumer = Arc::new(service.open_consumer("fixture").unwrap());
        let running = consumer.clone();
        let (url, server) = serve(b"DATA").await;
        let (started, observed) = tokio::sync::oneshot::channel();
        let (release, wait) = std::sync::mpsc::channel();
        let (callback_alive, callback_dropped) = tokio::sync::oneshot::channel::<()>();
        let waiter = tokio::spawn(async move {
            running
                .acquire_http(
                    AcquisitionHttpRequest {
                        demand: AcquisitionDemand {
                            consumer: "fixture".into(),
                            operation: "cancel-gated-consumer-effect".into(),
                        },
                        manifest: manifest("payload.bin"),
                        workspace: grant,
                        sources: vec![AcquisitionHttpSource {
                            url,
                            authorization: None,
                        }],
                        retry: retry(),
                    },
                    reqwest::Client::new(),
                    Box::new(Host),
                    move |use_handle| async move {
                        let _callback_alive = callback_alive;
                        use_handle
                            .run_blocking("gated workspace effect", move || {
                                let _ = started.send(());
                                wait.recv()
                                    .map_err(|error| PumasError::Other(error.to_string()))?;
                                Ok(())
                            })
                            .await?;
                        Ok(((), serde_json::json!({ "fixture": "completed" })))
                    },
                    |(), _receipt| async { Ok(()) },
                )
                .await
        });
        tokio::time::timeout(timeout, observed)
            .await
            .unwrap()
            .unwrap();
        waiter.abort();
        let cancelled = waiter.await.unwrap_err().is_cancelled();
        // Closing this sender proves the owned callback was dropped as well as
        // its public waiter, while the registered effect is still gated.
        let callback_was_dropped = tokio::time::timeout(timeout, callback_dropped).await;
        let lock_still_held = fs2::FileExt::try_lock_exclusive(&contender).is_err();
        let drain = consumer.shutdown();
        tokio::pin!(drain);
        let drain_still_pending = futures::poll!(&mut drain).is_pending();
        // Release before asserting so a regression cannot strand a blocking
        // thread when the test runtime is torn down.
        release.send(()).unwrap();
        tokio::time::timeout(timeout, drain).await.unwrap().unwrap();
        service.shutdown().await.unwrap();
        server.await.unwrap();
        assert!(cancelled);
        assert!(matches!(callback_was_dropped, Ok(Err(_))));
        assert!(
            lock_still_held,
            "cancelled waiter released the workspace reservation"
        );
        assert!(
            drain_still_pending,
            "shutdown skipped the registered effect"
        );
        fs2::FileExt::try_lock_exclusive(&contender)
            .expect("completed effect must release the workspace reservation");
        fs2::FileExt::unlock(&contender).unwrap();
    }

    #[tokio::test]
    async fn withdrawal_waits_for_consumer_effect_before_reclaiming_files() {
        withdrawal_waits_for_consumer_effect(false).await;
    }

    #[tokio::test]
    async fn cleanup_failure_retains_using_record_and_verified_file() {
        withdrawal_waits_for_consumer_effect(true).await;
    }

    #[tokio::test]
    async fn public_withdrawal_preserves_live_use_and_cold_retained_demand() {
        let timeout = Duration::from_secs(5);
        let temp = tempfile::TempDir::new().unwrap();
        let stage_a = temp.path().join("stage-a");
        let stage_b = temp.path().join("stage-b");
        let stage_c = temp.path().join("stage-c");
        std::fs::create_dir(&stage_a).unwrap();
        std::fs::create_dir(&stage_b).unwrap();
        std::fs::create_dir(&stage_c).unwrap();
        let sentinel_a = stage_a.join("sentinel.bin");
        let sentinel_b = stage_b.join("sentinel.bin");
        let sentinel_c = stage_c.join("sentinel.bin");
        std::fs::write(&sentinel_a, b"A sentinel").unwrap();
        std::fs::write(&sentinel_b, b"B sentinel").unwrap();
        std::fs::write(&sentinel_c, b"C sentinel").unwrap();
        let sentinel_a_before = std::fs::read(&sentinel_a).unwrap();
        let sentinel_b_before = std::fs::read(&sentinel_b).unwrap();
        let sentinel_c_before = std::fs::read(&sentinel_c).unwrap();

        // Leave a real receiptless Using row, then destroy the original owner
        // and reopen the same persisted store before creating the other demands.
        let first_store = Arc::new(AcquisitionStore::new(temp.path()));
        let first_service = Arc::new(AcquisitionService::new(first_store.clone()));
        let consumer_c = first_service.open_consumer("consumer-c").unwrap();
        let demand_c = AcquisitionDemand {
            consumer: "consumer-c".into(),
            operation: "cold-retained-using-demand".into(),
        };
        let workspace_c = workspace_with_identity(
            &stage_c,
            WorkspaceIdentity {
                root_identity: "fixture-physical-root".into(),
                relative_target: "stage-c".into(),
            },
        );
        let (url_c, server_c) = serve(b"CCCC").await;
        let result_c = tokio::time::timeout(
            timeout,
            consumer_c.acquire_http(
                AcquisitionHttpRequest {
                    demand: demand_c.clone(),
                    manifest: manifest("payload.bin"),
                    workspace: workspace_c,
                    sources: vec![AcquisitionHttpSource {
                        url: url_c,
                        authorization: None,
                    }],
                    retry: retry(),
                },
                reqwest::Client::new(),
                Box::new(Host),
                |_use_handle| async move {
                    Err::<((), Value), PumasError>(PumasError::Other(
                        "injected consumer preparation failure".into(),
                    ))
                },
                |(), _receipt| async move { Ok::<(), PumasError>(()) },
            ),
        )
        .await
        .expect("cold retained fixture acquisition must finish");
        assert!(
            matches!(result_c, Err(PumasError::Other(message)) if message == "injected consumer preparation failure")
        );
        tokio::time::timeout(timeout, server_c)
            .await
            .expect("cold retained fixture source must finish")
            .expect("cold retained fixture source must not panic");
        let records_c = first_store.acquisitions().unwrap();
        assert_eq!(records_c.len(), 1);
        let cold_record = records_c
            .values()
            .find(|record| record.demand == demand_c)
            .unwrap()
            .clone();
        assert!(matches!(&cold_record.phase, AcquisitionPhase::Using { .. }));
        assert!(first_store
            .consumer_receipt(cold_record.id)
            .unwrap()
            .is_none());
        assert_eq!(std::fs::read(stage_c.join("payload.bin")).unwrap(), b"CCCC");
        tokio::time::timeout(timeout, consumer_c.shutdown())
            .await
            .expect("initial cold-fixture consumer must shut down")
            .unwrap();
        tokio::time::timeout(timeout, first_service.shutdown())
            .await
            .expect("initial cold-fixture service must shut down")
            .unwrap();
        drop(consumer_c);
        drop(first_service);
        drop(first_store);

        let store = Arc::new(AcquisitionStore::new(temp.path()));
        assert_eq!(store.acquisitions().unwrap(), records_c);
        let service = Arc::new(AcquisitionService::new(store.clone()));
        let consumer_a = Arc::new(service.open_consumer("consumer-a").unwrap());
        let consumer_b = Arc::new(service.open_consumer("consumer-b").unwrap());

        let demand_b = AcquisitionDemand {
            consumer: "consumer-b".into(),
            operation: "live-using-demand".into(),
        };
        let b_reservation_dropped = Arc::new(AtomicBool::new(false));
        let b_root = crate::platform::capability_fs::open_directory(&stage_b).unwrap();
        let b_check = b_root.try_clone().unwrap();
        let b_expected_root = std::fs::canonicalize(&stage_b).unwrap();
        let b_source_root = stage_b.clone();
        let b_grant = AcquisitionWorkspace::from_capability(
            b_root,
            WorkspaceIdentity {
                root_identity: "fixture-physical-root".into(),
                relative_target: "stage-b".into(),
            },
            Arc::new(ReservationDropProbe(b_reservation_dropped.clone())),
            move || {
                if std::fs::canonicalize(&b_source_root)? != b_expected_root
                    || !b_check.dir_metadata()?.is_dir()
                {
                    return Err(invalid("Fixture B grant changed"));
                }
                Ok(())
            },
        )
        .unwrap();
        let (url_b, server_b) = serve(b"BBBB").await;
        let (b_effect_started_sender, b_effect_started) = tokio::sync::oneshot::channel();
        let (b_effect_release_sender, b_effect_release_receiver) = std::sync::mpsc::channel();
        let (b_read_sender, b_read_result) = tokio::sync::oneshot::channel();
        let (b_callback_continue_sender, b_callback_continue_receiver) =
            tokio::sync::oneshot::channel();
        let b_consumer = consumer_b.clone();
        let b_task = tokio::spawn(async move {
            b_consumer
                .acquire_http(
                    AcquisitionHttpRequest {
                        demand: demand_b,
                        manifest: manifest("payload.bin"),
                        workspace: b_grant,
                        sources: vec![AcquisitionHttpSource {
                            url: url_b,
                            authorization: None,
                        }],
                        retry: retry(),
                    },
                    reqwest::Client::new(),
                    Box::new(Host),
                    move |use_handle| async move {
                        let expected_id = use_handle.record().id;
                        let mut verified_file = use_handle.open_file(0).await?;
                        let read_effect = use_handle.run_blocking(
                            "held verified read during unrelated withdrawal",
                            move || {
                                let _ = b_effect_started_sender.send(());
                                b_effect_release_receiver.recv().map_err(|error| {
                                    PumasError::Other(format!("B read gate closed: {error}"))
                                })?;
                                let mut bytes = Vec::new();
                                std::io::Read::read_to_end(&mut verified_file, &mut bytes)
                                    .map_err(|error| PumasError::Other(error.to_string()))?;
                                b_read_sender.send(bytes).map_err(|_| {
                                    PumasError::Other("B read observer disappeared".into())
                                })?;
                                Ok(())
                            },
                        );
                        tokio::pin!(read_effect);
                        let mut callback_continue = b_callback_continue_receiver;
                        tokio::select! {
                            result = &mut read_effect => {
                                result?;
                                callback_continue.await.map_err(|_| {
                                    PumasError::Other("B consumer callback was not released".into())
                                })?;
                            }
                            signal = &mut callback_continue => {
                                signal.map_err(|_| {
                                    PumasError::Other("B consumer callback was not released".into())
                                })?;
                                read_effect.await?;
                            }
                        }
                        assert_eq!(use_handle.record().id, expected_id);
                        Ok::<((), Value), PumasError>(((), serde_json::json!({"consumer": "B"})))
                    },
                    |(), _receipt| async move { Ok::<String, PumasError>("published-B".into()) },
                )
                .await
        });

        tokio::time::timeout(timeout, b_effect_started)
            .await
            .expect("B must enter its registered verified read")
            .expect("B read observer must remain connected");
        assert!(!b_reservation_dropped.load(Ordering::SeqCst));

        let demand_a = AcquisitionDemand {
            consumer: "consumer-a".into(),
            operation: "withdraw-while-other-use-is-live".into(),
        };
        let workspace_a = workspace_with_identity(
            &stage_a,
            WorkspaceIdentity {
                root_identity: "fixture-physical-root".into(),
                relative_target: "stage-a".into(),
            },
        );
        let (url_a, server_a) = serve(b"AAAA").await;
        let (a_use_sender, a_use_receiver) = tokio::sync::oneshot::channel();
        let (a_callback_continue_sender, a_callback_continue_receiver) =
            tokio::sync::oneshot::channel();
        let a_consumer = consumer_a.clone();
        let a_task = tokio::spawn(async move {
            a_consumer
                .acquire_http(
                    AcquisitionHttpRequest {
                        demand: demand_a,
                        manifest: manifest("payload.bin"),
                        workspace: workspace_a,
                        sources: vec![AcquisitionHttpSource {
                            url: url_a,
                            authorization: None,
                        }],
                        retry: retry(),
                    },
                    reqwest::Client::new(),
                    Box::new(Host),
                    move |use_handle| async move {
                        a_use_sender.send(use_handle).map_err(|_| {
                            PumasError::Other("A withdrawal observer disappeared".into())
                        })?;
                        a_callback_continue_receiver
                            .await
                            .map_err(|_| PumasError::Other("A callback was not released".into()))?;
                        Err::<((), Value), PumasError>(PumasError::DownloadCancelled)
                    },
                    |(), _receipt| async move { Ok::<(), PumasError>(()) },
                )
                .await
        });
        let a_use = tokio::time::timeout(timeout, a_use_receiver)
            .await
            .expect("A must reach its public Using callback")
            .expect("A use handle must remain available for withdrawal");

        let before = store.acquisitions().unwrap();
        assert_eq!(before.len(), 3);
        let record_a = before
            .values()
            .find(|record| {
                record.demand
                    == AcquisitionDemand {
                        consumer: "consumer-a".into(),
                        operation: "withdraw-while-other-use-is-live".into(),
                    }
            })
            .unwrap()
            .clone();
        let record_b = before
            .values()
            .find(|record| {
                record.demand
                    == AcquisitionDemand {
                        consumer: "consumer-b".into(),
                        operation: "live-using-demand".into(),
                    }
            })
            .unwrap()
            .clone();
        assert_eq!(before.get(&cold_record.id), Some(&cold_record));
        assert!(matches!(&record_a.phase, AcquisitionPhase::Using { .. }));
        assert!(matches!(&record_b.phase, AcquisitionPhase::Using { .. }));
        assert!(matches!(&cold_record.phase, AcquisitionPhase::Using { .. }));
        for record in [&record_a, &record_b, &cold_record] {
            assert!(store.consumer_receipt(record.id).unwrap().is_none());
        }
        assert_eq!(std::fs::read(stage_a.join("payload.bin")).unwrap(), b"AAAA");
        assert_eq!(std::fs::read(stage_b.join("payload.bin")).unwrap(), b"BBBB");
        assert_eq!(std::fs::read(stage_c.join("payload.bin")).unwrap(), b"CCCC");
        assert_eq!(std::fs::read(&sentinel_a).unwrap(), sentinel_a_before);
        assert_eq!(std::fs::read(&sentinel_b).unwrap(), sentinel_b_before);
        assert_eq!(std::fs::read(&sentinel_c).unwrap(), sentinel_c_before);

        let cleanup_path = stage_a.join("payload.bin");
        tokio::time::timeout(
            timeout,
            a_use.withdraw_after_cleanup(move || {
                std::fs::remove_file(cleanup_path)
                    .map_err(|error| PumasError::Other(error.to_string()))?;
                Ok(())
            }),
        )
        .await
        .expect("A's withdrawal must finish while B remains blocked")
        .unwrap();

        let after_a = store.acquisitions().unwrap();
        let mut expected_a = record_a.clone();
        expected_a.phase = AcquisitionPhase::Withdrawn;
        expected_a.files.clear();
        assert_eq!(after_a.get(&record_a.id), Some(&expected_a));
        assert_eq!(after_a.get(&record_b.id), Some(&record_b));
        assert_eq!(after_a.get(&cold_record.id), Some(&cold_record));
        assert!(store.consumer_receipt(record_a.id).unwrap().is_none());
        assert!(store.consumer_receipt(record_b.id).unwrap().is_none());
        assert!(store.consumer_receipt(cold_record.id).unwrap().is_none());
        assert!(!stage_a.join("payload.bin").exists());
        assert_eq!(std::fs::read(stage_b.join("payload.bin")).unwrap(), b"BBBB");
        assert_eq!(std::fs::read(stage_c.join("payload.bin")).unwrap(), b"CCCC");
        assert_eq!(std::fs::read(&sentinel_a).unwrap(), sentinel_a_before);
        assert_eq!(std::fs::read(&sentinel_b).unwrap(), sentinel_b_before);
        assert_eq!(std::fs::read(&sentinel_c).unwrap(), sentinel_c_before);
        assert!(
            !b_reservation_dropped.load(Ordering::SeqCst),
            "withdrawing A must not release B's live workspace reservation"
        );

        a_callback_continue_sender.send(()).unwrap();
        let a_result = tokio::time::timeout(timeout, a_task)
            .await
            .expect("A acquisition must finish after its callback is released")
            .expect("A acquisition task must not panic");
        assert!(matches!(a_result, Err(PumasError::DownloadCancelled)));
        tokio::time::timeout(timeout, consumer_a.shutdown())
            .await
            .expect("A consumer scope must close while B remains held")
            .unwrap();
        tokio::time::timeout(timeout, server_a)
            .await
            .expect("A local HTTP source must finish")
            .expect("A local HTTP source must not panic");
        assert!(!b_reservation_dropped.load(Ordering::SeqCst));

        b_effect_release_sender.send(()).unwrap();
        let b_read = tokio::time::timeout(timeout, b_read_result)
            .await
            .expect("B registered read must finish after release")
            .expect("B read observer must remain connected");
        assert_eq!(b_read, b"BBBB");
        b_callback_continue_sender.send(()).unwrap();
        let b_result = tokio::time::timeout(timeout, b_task)
            .await
            .expect("B consumer must settle normally")
            .expect("B acquisition task must not panic")
            .unwrap();
        assert_eq!(b_result, "published-B");
        assert!(b_reservation_dropped.load(Ordering::SeqCst));
        tokio::time::timeout(timeout, consumer_b.shutdown())
            .await
            .expect("B consumer scope must close")
            .unwrap();
        tokio::time::timeout(timeout, server_b)
            .await
            .expect("B local HTTP source must finish")
            .expect("B local HTTP source must not panic");

        let final_records = store.acquisitions().unwrap();
        assert_eq!(final_records.get(&record_a.id), Some(&expected_a));
        let mut expected_b = record_b.clone();
        if let AcquisitionPhase::Using { lease } = &record_b.phase {
            expected_b.phase = AcquisitionPhase::Adopted { lease: *lease };
        } else {
            unreachable!("B snapshot must be in Using");
        }
        assert_eq!(final_records.get(&record_b.id), Some(&expected_b));
        assert_eq!(final_records.get(&cold_record.id), Some(&cold_record));
        assert!(store.consumer_receipt(record_a.id).unwrap().is_none());
        assert!(store.consumer_receipt(record_b.id).unwrap().is_some());
        assert!(store.consumer_receipt(cold_record.id).unwrap().is_none());
        assert_eq!(std::fs::read(stage_b.join("payload.bin")).unwrap(), b"BBBB");
        assert_eq!(std::fs::read(stage_c.join("payload.bin")).unwrap(), b"CCCC");
        assert_eq!(std::fs::read(&sentinel_a).unwrap(), sentinel_a_before);
        assert_eq!(std::fs::read(&sentinel_b).unwrap(), sentinel_b_before);
        assert_eq!(std::fs::read(&sentinel_c).unwrap(), sentinel_c_before);
        tokio::time::timeout(timeout, service.shutdown())
            .await
            .expect("shared acquisition service must drain")
            .unwrap();
    }

    #[tokio::test]
    async fn explicit_access_refresh_preserves_selected_consumer_identity() {
        async fn read_headers(socket: &mut tokio::net::TcpStream) -> String {
            let mut bytes = Vec::new();
            while !bytes.ends_with(b"\r\n\r\n") {
                assert!(bytes.len() < 8192, "fixture request headers exceed bound");
                bytes.push(socket.read_u8().await.unwrap());
            }
            String::from_utf8(bytes).unwrap()
        }

        async fn write_response(
            socket: &mut tokio::net::TcpStream,
            status: &'static str,
            body: &'static [u8],
        ) {
            socket
                .write_all(
                    format!(
                        "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        body.len()
                    )
                    .as_bytes(),
                )
                .await
                .unwrap();
            socket.write_all(body).await.unwrap();
        }

        let timeout = Duration::from_secs(5);
        let temp = tempfile::TempDir::new().unwrap();
        let stage = temp.path().join("workspace");
        std::fs::create_dir(&stage).unwrap();
        let sentinel = stage.join("sentinel.bin");
        std::fs::write(&sentinel, b"preserve me").unwrap();
        let sentinel_before = std::fs::read(&sentinel).unwrap();

        let manifest_for = |bytes: &[u8]| {
            ArtifactManifest::new(
                ArtifactSourceIdentity::new(
                    "fixture",
                    "selected-object",
                    ArtifactRevisionEvidence::new(
                        "fixture.revision",
                        "immutable-v1",
                        RevisionStrength::Immutable,
                    )
                    .unwrap(),
                )
                .unwrap(),
                vec![ArtifactFile::new(
                    "payload.bin",
                    "selected-object",
                    Some(bytes.len() as u64),
                    Some(
                        Sha256Evidence::new("publisher.sha256", hex::encode(Sha256::digest(bytes)))
                            .unwrap(),
                    ),
                    FileVerificationRequirement::Sha256,
                )
                .unwrap()],
            )
            .unwrap()
        };
        let original_manifest = manifest_for(b"DATA");
        let changed_manifest = manifest_for(b"EVIL");
        let expected_digest = hex::encode(Sha256::digest(b"DATA"));
        let demand = AcquisitionDemand {
            consumer: "credential-refresh-fixture".into(),
            operation: "preserve-selected-object-on-access-refresh".into(),
        };
        let store = Arc::new(AcquisitionStore::new(temp.path()));
        let service = Arc::new(AcquisitionService::new(store.clone()));
        let consumer = service.open_consumer("credential-refresh-fixture").unwrap();
        let host = RecordingHost::default();
        let prepare_calls = Arc::new(AtomicUsize::new(0));
        let publish_calls = Arc::new(AtomicUsize::new(0));

        let old_access_token = "q1-old-access-secret";
        let old_authorization = format!("Bearer {old_access_token}");
        let old_query = "q1-old-query-secret";
        let new_access_token = "q1-new-access-secret";
        let new_authorization = format!("Bearer {new_access_token}");
        let new_query = "q1-new-query-secret";
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base_url = format!("http://{}/fixture", listener.local_addr().unwrap());
        let old_url = format!("{base_url}?token={old_query}");
        let refreshed_url = format!("{base_url}?token={new_query}");
        let request_count = Arc::new(AtomicUsize::new(0));
        let old_access_seen = Arc::new(AtomicBool::new(false));
        let refreshed_access_seen = Arc::new(AtomicBool::new(false));
        let request_count_in_server = request_count.clone();
        let old_access_seen_in_server = old_access_seen.clone();
        let refreshed_access_seen_in_server = refreshed_access_seen.clone();
        let (stop_server_sender, stop_server_receiver) = tokio::sync::oneshot::channel();
        let server = AbortOnDrop::new(tokio::spawn(async move {
            let mut stop_server_receiver = stop_server_receiver;
            let mut drain_deadline: Option<tokio::time::Instant> = None;
            loop {
                let accepted = if let Some(deadline) = drain_deadline {
                    let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
                    if remaining.is_zero() {
                        break;
                    }
                    match tokio::time::timeout(
                        remaining.min(Duration::from_millis(25)),
                        listener.accept(),
                    )
                    .await
                    {
                        Ok(Ok(accepted)) => accepted,
                        Ok(Err(error)) => panic!("fixture listener failed: {error}"),
                        Err(_) => break,
                    }
                } else {
                    tokio::select! {
                        biased;
                        accepted = listener.accept() => accepted.unwrap(),
                        _ = &mut stop_server_receiver => {
                            drain_deadline = Some(
                                tokio::time::Instant::now() + Duration::from_secs(2),
                            );
                            continue;
                        }
                    }
                };

                let (mut socket, _) = accepted;
                let count = request_count_in_server.fetch_add(1, Ordering::SeqCst) + 1;
                let request_deadline = tokio::time::Instant::now() + timeout;
                let io_deadline = drain_deadline
                    .map_or(request_deadline, |deadline| deadline.min(request_deadline));
                let read_timeout =
                    io_deadline.saturating_duration_since(tokio::time::Instant::now());
                if read_timeout.is_zero() {
                    break;
                }
                let request = tokio::time::timeout(read_timeout, read_headers(&mut socket))
                    .await
                    .expect("fixture request-header read must be bounded");
                let request_lower = request.to_ascii_lowercase();
                let is_old_access = request.contains(&format!("/fixture?token={old_query}"))
                    && request_lower.contains(&format!("authorization: bearer {old_access_token}"));
                let is_refreshed_access = request.contains(&format!("/fixture?token={new_query}"))
                    && request_lower.contains(&format!("authorization: bearer {new_access_token}"));

                let write_timeout =
                    io_deadline.saturating_duration_since(tokio::time::Instant::now());
                if write_timeout.is_zero() {
                    break;
                }

                if count == 1 {
                    old_access_seen_in_server.store(is_old_access, Ordering::SeqCst);
                    tokio::time::timeout(
                        write_timeout,
                        write_response(&mut socket, "403 Forbidden", b""),
                    )
                    .await
                    .expect("fixture denial response write must be bounded");
                } else if is_refreshed_access {
                    refreshed_access_seen_in_server.store(true, Ordering::SeqCst);
                    tokio::time::timeout(
                        write_timeout,
                        write_response(&mut socket, "200 OK", b"DATA"),
                    )
                    .await
                    .expect("fixture success response write must be bounded");
                } else {
                    // An attempted source read for the changed manifest is an
                    // observed unexpected request and must not be mistaken for
                    // the subsequent access refresh.
                    tokio::time::timeout(
                        write_timeout,
                        write_response(&mut socket, "403 Forbidden", b""),
                    )
                    .await
                    .expect("fixture refusal response write must be bounded");
                }
            }
        }));

        let make_request =
            |manifest: ArtifactManifest, url: String, authorization: &str| AcquisitionHttpRequest {
                demand: demand.clone(),
                manifest,
                workspace: workspace(&stage),
                sources: vec![AcquisitionHttpSource {
                    url,
                    authorization: Some(authorization.to_owned()),
                }],
                retry: retry(),
            };

        let prepare_on_denial = prepare_calls.clone();
        let publish_on_denial = publish_calls.clone();
        let denied_result = tokio::time::timeout(
            timeout,
            consumer.acquire_http(
                make_request(
                    original_manifest.clone(),
                    old_url.clone(),
                    &old_authorization,
                ),
                super::super::AcquisitionHttpClient::from(reqwest::Client::new())
                    .with_loopback_fixture(reqwest::Client::new()),
                Box::new(host.clone()),
                move |_use_handle| async move {
                    prepare_on_denial.fetch_add(1, Ordering::SeqCst);
                    Err::<((), Value), PumasError>(PumasError::Other(
                        "denied attempt reached consumer preparation".into(),
                    ))
                },
                move |(), _receipt| async move {
                    publish_on_denial.fetch_add(1, Ordering::SeqCst);
                    Ok::<(), PumasError>(())
                },
            ),
        )
        .await
        .expect("denied acquisition must finish");
        let denied_error = denied_result.expect_err("403 must refuse source access");
        assert!(matches!(
            &denied_error,
            PumasError::DownloadFailed { url, message }
                if url == "artifact source" && message == "HTTP 403 Forbidden"
        ));
        let denied_error_text = denied_error.to_string();
        assert!(!denied_error_text.contains(old_access_token));
        assert!(!denied_error_text.contains(old_query));

        let denied_records = store.acquisitions().unwrap();
        assert_eq!(denied_records.len(), 1);
        let denied_record = denied_records.values().next().unwrap().clone();
        assert_eq!(denied_record.demand, demand);
        assert_eq!(denied_record.manifest, original_manifest);
        assert_eq!(
            denied_record.workspace,
            workspace(&stage).identity().clone()
        );
        assert!(matches!(
            &denied_record.phase,
            AcquisitionPhase::Transferring
        ));
        assert!(denied_record.files.is_empty());
        assert!(store.consumer_receipt(denied_record.id).unwrap().is_none());
        assert!(!stage.join("payload.bin").exists());
        assert!(!stage.join("payload.bin.part").exists());
        assert_eq!(prepare_calls.load(Ordering::SeqCst), 0);
        assert_eq!(publish_calls.load(Ordering::SeqCst), 0);
        assert_eq!(request_count.load(Ordering::SeqCst), 1);
        assert!(old_access_seen.load(Ordering::SeqCst));
        {
            let notifications = host.retry_notifications.lock().unwrap();
            assert!(
                notifications.len() == 1 && notifications[0].is_none(),
                "403 must not schedule a retry after the initial attempt"
            );
        }
        assert!(host.progress_bytes.lock().unwrap().is_empty());
        let denied_json = std::fs::read_to_string(temp.path().join("downloads.json")).unwrap();
        assert!(!denied_json.contains(old_access_token));
        assert!(!denied_json.contains(old_query));

        let prepare_on_changed = prepare_calls.clone();
        let publish_on_changed = publish_calls.clone();
        let changed_result = tokio::time::timeout(
            timeout,
            consumer.acquire_http(
                make_request(changed_manifest, old_url.clone(), &old_authorization),
                super::super::AcquisitionHttpClient::from(reqwest::Client::new())
                    .with_loopback_fixture(reqwest::Client::new()),
                Box::new(host.clone()),
                move |_use_handle| async move {
                    prepare_on_changed.fetch_add(1, Ordering::SeqCst);
                    Err::<((), Value), PumasError>(PumasError::Other(
                        "changed selection reached consumer preparation".into(),
                    ))
                },
                move |(), _receipt| async move {
                    publish_on_changed.fetch_add(1, Ordering::SeqCst);
                    Ok::<(), PumasError>(())
                },
            ),
        )
        .await
        .expect("changed selection attempt must finish");
        assert!(matches!(
            &changed_result,
            Err(PumasError::Validation { field, message })
                if field == "acquisition.custody"
                    && message == "Exact acquisition demand changed selection or workspace"
        ));
        assert_eq!(store.acquisitions().unwrap(), denied_records);
        assert!(
            std::fs::read_to_string(temp.path().join("downloads.json")).unwrap() == denied_json,
            "persisted state changed during rejected selection"
        );
        assert!(!stage.join("payload.bin").exists());
        assert!(!stage.join("payload.bin.part").exists());
        assert_eq!(request_count.load(Ordering::SeqCst), 1);
        assert_eq!(prepare_calls.load(Ordering::SeqCst), 0);
        assert_eq!(publish_calls.load(Ordering::SeqCst), 0);
        {
            let notifications = host.retry_notifications.lock().unwrap();
            assert!(
                notifications.len() == 1 && notifications[0].is_none(),
                "changed selection must not start another HTTP attempt"
            );
        }

        let expected_id = denied_record.id;
        let expected_manifest = original_manifest.clone();
        let prepare_on_refresh = prepare_calls.clone();
        let publish_on_refresh = publish_calls.clone();
        let receipt_result = tokio::time::timeout(
            timeout,
            consumer.acquire_http(
                make_request(original_manifest.clone(), refreshed_url, &new_authorization),
                super::super::AcquisitionHttpClient::from(reqwest::Client::new())
                    .with_loopback_fixture(reqwest::Client::new()),
                Box::new(host.clone()),
                move |use_handle| async move {
                    prepare_on_refresh.fetch_add(1, Ordering::SeqCst);
                    let verified_file = use_handle.open_file(0).await?;
                    let bytes = use_handle
                        .run_blocking("read refreshed verified artifact", move || {
                            let mut verified_file = verified_file;
                            let mut bytes = Vec::new();
                            std::io::Read::read_to_end(&mut verified_file, &mut bytes)
                                .map_err(|error| PumasError::Other(error.to_string()))?;
                            Ok(bytes)
                        })
                        .await?;
                    Ok::<(Vec<u8>, Value), PumasError>((
                        bytes,
                        serde_json::json!({"selected_sha256": expected_digest}),
                    ))
                },
                move |bytes, receipt| async move {
                    publish_on_refresh.fetch_add(1, Ordering::SeqCst);
                    assert_eq!(bytes, b"DATA");
                    assert_eq!(receipt.acquisition_id, expected_id.to_string());
                    assert_eq!(receipt.manifest, expected_manifest);
                    Ok::<AcquisitionConsumerReceipt, PumasError>(receipt)
                },
            ),
        )
        .await
        .expect("refreshed access attempt must finish");
        let receipt = match receipt_result {
            Ok(receipt) => receipt,
            Err(_) => panic!("refreshed access must acquire the original selected bytes"),
        };

        assert!(old_access_seen.load(Ordering::SeqCst));
        assert!(refreshed_access_seen.load(Ordering::SeqCst));
        assert_eq!(prepare_calls.load(Ordering::SeqCst), 1);
        assert_eq!(publish_calls.load(Ordering::SeqCst), 1);
        {
            let notifications = host.retry_notifications.lock().unwrap();
            assert!(
                notifications.len() == 2 && notifications.iter().all(Option::is_none),
                "only the two explicit acquisition attempt starts are expected"
            );
        }
        assert!(host
            .progress_bytes
            .lock()
            .unwrap()
            .iter()
            .all(|bytes| *bytes <= 4));

        let final_records = store.acquisitions().unwrap();
        assert_eq!(final_records.len(), 1);
        let final_record = final_records.get(&expected_id).unwrap();
        assert_eq!(final_record.demand, demand);
        assert_eq!(final_record.manifest, original_manifest);
        assert_eq!(final_record.workspace, denied_record.workspace);
        assert!(matches!(
            &final_record.phase,
            AcquisitionPhase::Adopted { .. }
        ));
        assert_eq!(final_record.files.len(), 1);
        assert_eq!(final_record.files[0].path, "payload.bin");
        assert_eq!(final_record.files[0].bytes, 4);
        assert_eq!(
            final_record.files[0].sha256,
            hex::encode(Sha256::digest(b"DATA"))
        );
        assert_eq!(std::fs::read(stage.join("payload.bin")).unwrap(), b"DATA");
        assert_eq!(std::fs::read(&sentinel).unwrap(), sentinel_before);
        let stored_receipt = store
            .consumer_receipt(expected_id)
            .unwrap()
            .expect("successful consumer must publish its exact receipt");
        assert_eq!(stored_receipt, receipt);
        assert_eq!(stored_receipt.manifest, original_manifest);
        assert_eq!(stored_receipt.verified_files, final_record.files);
        assert_eq!(
            consumer.completion_receipt(final_record).unwrap(),
            Some(receipt.clone())
        );
        let receipt_json = serde_json::to_string(&receipt).unwrap();
        assert!(!receipt_json.contains(old_access_token));
        assert!(!receipt_json.contains(old_query));
        assert!(!receipt_json.contains(new_access_token));
        assert!(!receipt_json.contains(new_query));
        let final_json = std::fs::read_to_string(temp.path().join("downloads.json")).unwrap();
        assert!(!final_json.contains(old_access_token));
        assert!(!final_json.contains(old_query));
        assert!(!final_json.contains(new_access_token));
        assert!(!final_json.contains(new_query));
        assert!(host
            .retry_notifications
            .lock()
            .unwrap()
            .iter()
            .flatten()
            .all(|error| {
                !error.contains(old_access_token)
                    && !error.contains(old_query)
                    && !error.contains(new_access_token)
                    && !error.contains(new_query)
            }));

        tokio::time::timeout(timeout, consumer.shutdown())
            .await
            .expect("consumer scope must shut down")
            .unwrap();
        tokio::time::timeout(timeout, service.shutdown())
            .await
            .expect("acquisition service must shut down")
            .unwrap();
        let _ = stop_server_sender.send(());
        server
            .join_with_timeout(timeout)
            .await
            .expect("source monitor must drain or be aborted within the bound");
        assert_eq!(request_count.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn same_task_key_cancel_replacement_rejects_stale_acquisition_readiness() {
        use crate::acquisition::task_custody::{CancelTransition, TaskRole};

        let timeout = Duration::from_secs(5);
        let temp = tempfile::TempDir::new().unwrap();
        let stage = temp.path().join("stage");
        std::fs::create_dir(&stage).unwrap();
        std::fs::write(stage.join("payload.bin"), b"DATA").unwrap();
        std::fs::write(stage.join("payload.bin.part"), b"KEEP").unwrap();
        let store = Arc::new(AcquisitionStore::new(temp.path()));
        let service = Arc::new(AcquisitionService::new(store.clone()));
        let consumer = service.open_consumer("fixture").unwrap();
        let task_key = "stable-acquisition-worker-key";
        let demand = AcquisitionDemand {
            consumer: "fixture".into(),
            operation: "same-task-key-cancel-replacement".into(),
        };
        let selection = manifest("payload.bin");
        let grant = workspace(&stage);
        let service_for_worker = service.clone();
        let demand_for_worker = demand.clone();
        let selection_for_worker = selection.clone();
        let workspace_for_worker = grant.identity().clone();
        let (operation_sender, operation_receiver) =
            tokio::sync::oneshot::channel::<(TaskContext, AcquisitionOperation)>();
        let (effect_started_sender, effect_started) = tokio::sync::oneshot::channel();
        let (release_effect_sender, release_effect) = tokio::sync::oneshot::channel();
        let (effect_finished_sender, effect_finished) = tokio::sync::oneshot::channel();
        let prepared = consumer
            .scope
            .prepare(
                task_key.into(),
                TaskRole::Worker,
                move |context| async move {
                    let operation = service_for_worker
                        .begin(
                            &context,
                            demand_for_worker,
                            selection_for_worker,
                            workspace_for_worker,
                            None,
                        )
                        .await
                        .unwrap();
                    let _ = operation_sender.send((context.clone(), operation));
                    let _ = context
                        .run_fallible_async_named("held same-key predecessor", move || async move {
                            let _ = effect_started_sender.send(());
                            let _ = release_effect.await;
                            let _ = effect_finished_sender.send(());
                            Ok::<(), ()>(())
                        })
                        .await;
                },
            )
            .unwrap();
        let worker = consumer.scope.install_gated(prepared).unwrap();
        let worker_generation = worker.generation().clone();
        worker.start();
        let (worker_context, operation) = tokio::time::timeout(timeout, operation_receiver)
            .await
            .expect("worker must admit an acquisition")
            .expect("worker must retain its operation context");
        tokio::time::timeout(timeout, effect_started)
            .await
            .expect("registered predecessor effect must start")
            .expect("predecessor effect observer must remain connected");

        assert!(worker_context
            .generation()
            .matches(operation.context.generation()));
        assert!(worker_context.is_current_role(TaskRole::Worker));
        let before = store.acquisitions().unwrap();
        assert_eq!(before.len(), 1);
        let (acquisition_id, record) = before.iter().next().unwrap();
        assert_eq!(record.demand, demand);
        assert_eq!(record.manifest, selection);
        assert_eq!(record.workspace, *grant.identity());
        assert!(matches!(record.phase, AcquisitionPhase::Transferring));
        assert!(record.files.is_empty());

        let seal_observed = Arc::new(AtomicBool::new(false));
        let seal_observed_by_hook = seal_observed.clone();
        consumer
            .scope
            .set_blocking_observer(Some(Arc::new(move |label| {
                if label == "seal verified acquisition file set" {
                    seal_observed_by_hook.store(true, Ordering::SeqCst);
                }
            })));
        let (finalizer_entered_sender, mut finalizer_entered) = tokio::sync::oneshot::channel::<(
            bool,
            crate::acquisition::task_custody::TaskGeneration,
        )>();
        let (release_finalizer_sender, release_finalizer) = tokio::sync::oneshot::channel();
        let transition = consumer
            .scope
            .begin_cancel(task_key, move |context, _predecessor| async move {
                let _ = finalizer_entered_sender.send((
                    context.is_current_role(TaskRole::CancelFinalizer),
                    context.generation().clone(),
                ));
                let _ = release_finalizer.await;
            })
            .unwrap();
        let CancelTransition::Started(finalizer) = transition else {
            panic!("the active worker must be replaced by a cancellation finalizer");
        };
        let finalizer_generation = finalizer.generation().clone();
        assert!(!worker_generation.matches(&finalizer_generation));
        finalizer.start();

        let readiness = service
            .files_ready(&worker_context, operation, grant.clone())
            .await;
        assert!(matches!(
            readiness,
            Err(PumasError::Other(message))
                if message == "Acquisition effect observation failed: task generation is no longer current"
        ));
        assert!(!worker_context.is_current_role(TaskRole::Worker));
        assert!(!seal_observed.load(Ordering::SeqCst));
        assert_eq!(store.acquisitions().unwrap(), before);
        assert!(store.consumer_receipt(*acquisition_id).unwrap().is_none());
        assert_eq!(std::fs::read(stage.join("payload.bin")).unwrap(), b"DATA");
        assert_eq!(
            std::fs::read(stage.join("payload.bin.part")).unwrap(),
            b"KEEP"
        );
        assert_eq!(std::fs::read_dir(&stage).unwrap().count(), 2);
        drop(grant.open_part("payload.bin", true).unwrap());
        assert!(matches!(
            finalizer_entered.try_recv(),
            Err(tokio::sync::oneshot::error::TryRecvError::Empty)
        ));
        assert!(consumer
            .scope
            .snapshot(task_key)
            .is_some_and(|snapshot| snapshot.role == TaskRole::CancelFinalizer));

        release_effect_sender.send(()).unwrap();
        tokio::time::timeout(timeout, effect_finished)
            .await
            .expect("owned predecessor effect must finish after release")
            .expect("predecessor completion observer must remain connected");
        let (finalizer_is_current, observed_generation) =
            tokio::time::timeout(timeout, &mut finalizer_entered)
                .await
                .expect("finalizer must enter after predecessor drainage")
                .expect("finalizer entry observer must remain connected");
        assert!(finalizer_is_current);
        assert!(observed_generation.matches(&finalizer_generation));
        assert!(consumer
            .scope
            .snapshot(task_key)
            .is_some_and(|snapshot| snapshot.role == TaskRole::CancelFinalizer));
        release_finalizer_sender.send(()).unwrap();

        tokio::time::timeout(timeout, consumer.shutdown())
            .await
            .expect("consumer scope must drain")
            .unwrap();
        tokio::time::timeout(timeout, service.shutdown())
            .await
            .expect("service must drain")
            .unwrap();
        consumer.scope.set_blocking_observer(None);
        assert_eq!(store.acquisitions().unwrap(), before);
        assert!(store.consumer_receipt(*acquisition_id).unwrap().is_none());
        assert!(!seal_observed.load(Ordering::SeqCst));
        assert_eq!(std::fs::read(stage.join("payload.bin")).unwrap(), b"DATA");
        assert_eq!(
            std::fs::read(stage.join("payload.bin.part")).unwrap(),
            b"KEEP"
        );
        assert_eq!(std::fs::read_dir(&stage).unwrap().count(), 2);
    }

    #[tokio::test]
    async fn same_task_key_worker_reuse_rejects_stale_readiness_and_preserves_successor_handoff() {
        use crate::acquisition::task_custody::{TaskRole, TaskTerminal};

        let timeout = Duration::from_secs(5);
        let temp = tempfile::TempDir::new().unwrap();
        let stage = temp.path().join("stage");
        std::fs::create_dir(&stage).unwrap();
        std::fs::write(stage.join("payload.bin"), b"DATA").unwrap();
        std::fs::write(stage.join("payload.bin.part"), b"KEEP").unwrap();
        let store = Arc::new(AcquisitionStore::new(temp.path()));
        let service = Arc::new(AcquisitionService::new(store.clone()));
        let consumer = service.open_consumer("fixture").unwrap();
        let task_key = "stable-acquisition-worker-key";
        let demand = AcquisitionDemand {
            consumer: "fixture".into(),
            operation: "same-task-key-worker-reuse".into(),
        };
        let selection = manifest("payload.bin");
        let grant = workspace(&stage);
        let seal_calls = Arc::new(AtomicUsize::new(0));
        let observed_seal_calls = seal_calls.clone();
        consumer
            .scope
            .set_blocking_observer(Some(Arc::new(move |label| {
                if label == "seal verified acquisition file set" {
                    observed_seal_calls.fetch_add(1, Ordering::SeqCst);
                }
            })));

        let mut predecessor_generation = None;
        let mut successor_generation = None;
        let (release_successor_sender, release_successor_receiver) =
            tokio::sync::oneshot::channel::<()>();
        let outcome = async {
            let service_for_predecessor = service.clone();
            let demand_for_predecessor = demand.clone();
            let selection_for_predecessor = selection.clone();
            let workspace_for_predecessor = grant.identity().clone();
            let (predecessor_sender, predecessor_receiver) =
                tokio::sync::oneshot::channel::<Result<(TaskContext, AcquisitionOperation)>>();
            let prepared = consumer.scope.prepare(
                task_key.into(),
                TaskRole::Worker,
                move |context| async move {
                    let result = service_for_predecessor
                        .begin(
                            &context,
                            demand_for_predecessor,
                            selection_for_predecessor,
                            workspace_for_predecessor,
                            None,
                        )
                        .await
                        .map(|operation| (context.clone(), operation));
                    let _ = predecessor_sender.send(result);
                },
            )?;
            let worker_a = consumer.scope.install_gated(prepared).map_err(|_| {
                PumasError::Other("predecessor Worker must install under the free task key".into())
            })?;
            let generation_a = worker_a.generation().clone();
            predecessor_generation = Some(generation_a.clone());
            worker_a.start();

            let (predecessor_context, predecessor_operation) =
                tokio::time::timeout(timeout, predecessor_receiver)
                    .await
                    .map_err(|_| {
                        PumasError::Other("predecessor must admit acquisition in time".into())
                    })?
                    .map_err(|_| {
                        PumasError::Other(
                            "predecessor admission observer must remain connected".into(),
                        )
                    })??;
            let predecessor_pair_matches = predecessor_context
                .shares_scope(&predecessor_operation.context)
                && predecessor_context
                    .generation()
                    .matches(predecessor_operation.context.generation());
            let acquisition_id = predecessor_operation.record.id;
            let before = store.acquisitions()?;
            let before_record = before.get(&acquisition_id).cloned().ok_or_else(|| {
                PumasError::Other("predecessor acquisition row must be retained".into())
            })?;

            let observation_a = tokio::time::timeout(timeout, async {
                loop {
                    if let Some(observation) = consumer
                        .scope
                        .observe_finished_generation(task_key, &generation_a)
                        .await
                    {
                        break observation;
                    }
                    tokio::task::yield_now().await;
                }
            })
            .await
            .map_err(|_| PumasError::Other("predecessor generation must finish in time".into()))?;

            let service_for_successor = service.clone();
            let demand_for_successor = demand.clone();
            let selection_for_successor = selection.clone();
            let workspace_for_successor = grant.identity().clone();
            let (successor_sender, successor_receiver) =
                tokio::sync::oneshot::channel::<Result<(TaskContext, AcquisitionOperation)>>();
            let prepared = consumer.scope.prepare(
                task_key.into(),
                TaskRole::Worker,
                move |context| async move {
                    let result = service_for_successor
                        .begin(
                            &context,
                            demand_for_successor,
                            selection_for_successor,
                            workspace_for_successor,
                            None,
                        )
                        .await
                        .map(|operation| (context.clone(), operation));
                    let _ = successor_sender.send(result);
                    let _ = release_successor_receiver.await;
                },
            )?;
            let worker_b = consumer.scope.install_gated(prepared).map_err(|_| {
                PumasError::Other(
                    "successor Worker must install after predecessor observation".into(),
                )
            })?;
            let generation_b = worker_b.generation().clone();
            successor_generation = Some(generation_b.clone());
            worker_b.start();

            let (successor_context, successor_operation) =
                tokio::time::timeout(timeout, successor_receiver)
                    .await
                    .map_err(|_| {
                        PumasError::Other("successor must admit acquisition in time".into())
                    })?
                    .map_err(|_| {
                        PumasError::Other(
                            "successor admission observer must remain connected".into(),
                        )
                    })??;
            let successor_pair_matches = successor_context
                .shares_scope(&successor_operation.context)
                && successor_context
                    .generation()
                    .matches(successor_operation.context.generation());
            let successor_is_current = successor_context.is_current_role(TaskRole::Worker);
            let successor_has_fresh_generation = !generation_a.matches(&generation_b);

            let stale_readiness = tokio::time::timeout(
                timeout,
                service.files_ready(&predecessor_context, predecessor_operation, grant.clone()),
            )
            .await
            .map_err(|_| PumasError::Other("stale readiness must reject in time".into()))?;
            let stale_outcome = match stale_readiness {
                Err(error) => Err(error),
                Ok(lease) => {
                    drop(lease);
                    Ok(())
                }
            };
            let after_stale = store.acquisitions()?;
            let receipt_after_stale = service.consumer_receipt(acquisition_id)?;
            let final_after_stale = std::fs::read(stage.join("payload.bin"))
                .map_err(|error| PumasError::Other(error.to_string()))?;
            let partial_after_stale = std::fs::read(stage.join("payload.bin.part"))
                .map_err(|error| PumasError::Other(error.to_string()))?;
            let entries_after_stale = std::fs::read_dir(&stage)
                .map_err(|error| PumasError::Other(error.to_string()))?
                .count();
            let partial_appendable = match grant.open_part("payload.bin", true) {
                Ok(partial) => {
                    drop(partial);
                    true
                }
                Err(_) => false,
            };
            let seals_after_stale = seal_calls.load(Ordering::SeqCst);

            let successor_readiness = tokio::time::timeout(
                timeout,
                service.files_ready(&successor_context, successor_operation, grant.clone()),
            )
            .await
            .map_err(|_| PumasError::Other("current readiness must finish in time".into()))?;
            let successor_result = successor_readiness.map(|lease| {
                let record = lease.record().clone();
                drop(lease);
                record
            });
            let after_successor = store.acquisitions()?;
            let receipt_after_successor = service.consumer_receipt(acquisition_id)?;
            let final_after_successor = std::fs::read(stage.join("payload.bin"))
                .map_err(|error| PumasError::Other(error.to_string()))?;
            let partial_after_successor = std::fs::read(stage.join("payload.bin.part"))
                .map_err(|error| PumasError::Other(error.to_string()))?;
            let entries_after_successor = std::fs::read_dir(&stage)
                .map_err(|error| PumasError::Other(error.to_string()))?
                .count();
            let seals_after_successor = seal_calls.load(Ordering::SeqCst);

            Ok::<_, PumasError>((
                observation_a,
                predecessor_pair_matches,
                acquisition_id,
                before,
                before_record,
                successor_pair_matches,
                successor_is_current,
                successor_has_fresh_generation,
                stale_outcome,
                after_stale,
                receipt_after_stale,
                final_after_stale,
                partial_after_stale,
                entries_after_stale,
                partial_appendable,
                seals_after_stale,
                successor_result,
                after_successor,
                receipt_after_successor,
                final_after_successor,
                partial_after_successor,
                entries_after_successor,
                seals_after_successor,
            ))
        }
        .await;

        // Releasing the held successor is also the unwind path if an earlier
        // bounded step returned an error.
        let _ = release_successor_sender.send(());
        let successor_observations = match (&predecessor_generation, &successor_generation) {
            (Some(generation_a), Some(generation_b)) => tokio::time::timeout(timeout, async {
                loop {
                    if consumer.scope.snapshot(task_key).is_some_and(|snapshot| {
                        snapshot.role == TaskRole::Worker && snapshot.finished
                    }) {
                        break;
                    }
                    tokio::task::yield_now().await;
                }
                let old = consumer
                    .scope
                    .observe_finished_generation(task_key, generation_a)
                    .await;
                let current = consumer
                    .scope
                    .observe_finished_generation(task_key, generation_b)
                    .await;
                (old, current)
            })
            .await
            .ok(),
            _ => None,
        };
        let consumer_drain = tokio::time::timeout(timeout, consumer.shutdown()).await;
        let service_drain = tokio::time::timeout(timeout, service.shutdown()).await;
        consumer.scope.set_blocking_observer(None);
        consumer_drain.expect("consumer scope must drain").unwrap();
        service_drain
            .expect("acquisition service must drain")
            .unwrap();

        let (
            observation_a,
            predecessor_pair_matches,
            acquisition_id,
            before,
            before_record,
            successor_pair_matches,
            successor_is_current,
            successor_has_fresh_generation,
            stale_outcome,
            after_stale,
            receipt_after_stale,
            final_after_stale,
            partial_after_stale,
            entries_after_stale,
            partial_appendable,
            seals_after_stale,
            successor_result,
            after_successor,
            receipt_after_successor,
            final_after_successor,
            partial_after_successor,
            entries_after_successor,
            seals_after_successor,
        ) = outcome.expect("same-key Worker generations must complete");

        let generation_a = predecessor_generation
            .as_ref()
            .expect("predecessor generation must be installed");
        let generation_b = successor_generation
            .as_ref()
            .expect("successor generation must be installed");
        assert!(predecessor_pair_matches);
        assert!(observation_a.generation.matches(generation_a));
        assert_eq!(observation_a.role, TaskRole::Worker);
        assert_eq!(observation_a.terminal, TaskTerminal::Completed);
        assert_eq!(observation_a.nested_failures, 0);
        assert!(successor_has_fresh_generation);
        assert!(successor_pair_matches);
        assert!(successor_is_current);
        assert!(matches!(
            &before_record.phase,
            AcquisitionPhase::Transferring
        ));
        assert_eq!(before_record.demand, demand);
        assert_eq!(before_record.manifest, selection);
        assert_eq!(before_record.workspace, *grant.identity());
        assert!(before_record.files.is_empty());
        assert_eq!(before.len(), 1);
        assert_eq!(after_stale, before);
        assert!(matches!(
            stale_outcome,
            Err(PumasError::Other(message))
                if message == "Acquisition effect observation failed: task generation is no longer current"
        ));
        assert!(receipt_after_stale.is_none());
        assert_eq!(final_after_stale, b"DATA");
        assert_eq!(partial_after_stale, b"KEEP");
        assert_eq!(entries_after_stale, 2);
        assert!(partial_appendable);
        assert_eq!(seals_after_stale, 0);

        let successor_record = match successor_result {
            Ok(record) => record,
            Err(_) => panic!("current successor readiness must hand off the acquisition"),
        };
        assert_eq!(successor_record.id, acquisition_id);
        assert_eq!(successor_record.demand, demand);
        assert_eq!(successor_record.manifest, selection);
        assert_eq!(successor_record.workspace, *grant.identity());
        assert!(matches!(
            &successor_record.phase,
            AcquisitionPhase::Using { .. }
        ));
        assert_eq!(successor_record.files.len(), 1);
        assert_eq!(successor_record.files[0].path, "payload.bin");
        assert_eq!(successor_record.files[0].bytes, 4);
        assert_eq!(
            successor_record.files[0].sha256,
            hex::encode(Sha256::digest(b"DATA"))
        );
        assert_eq!(after_successor.len(), 1);
        assert_eq!(
            after_successor.get(&acquisition_id),
            Some(&successor_record)
        );
        assert!(receipt_after_successor.is_none());
        assert_eq!(final_after_successor, b"DATA");
        assert_eq!(partial_after_successor, b"KEEP");
        assert_eq!(entries_after_successor, 2);
        assert_eq!(seals_after_successor, 1);
        let (stale_lookup, successor_observation) =
            successor_observations.expect("successor generation must finish within the bound");
        assert!(
            stale_lookup.is_none(),
            "observing the old generation must not consume its finished successor"
        );
        let successor_observation =
            successor_observation.expect("the exact successor generation must remain observable");
        assert!(successor_observation.generation.matches(generation_b));
        assert_eq!(successor_observation.role, TaskRole::Worker);
        assert_eq!(successor_observation.terminal, TaskTerminal::Completed);
        assert_eq!(successor_observation.nested_failures, 0);
    }

    #[tokio::test]
    async fn stale_worker_generation_cannot_seal_or_handoff_acquisition() {
        use crate::acquisition::task_custody::TaskRole;

        let timeout = Duration::from_secs(5);
        let temp = tempfile::TempDir::new().unwrap();
        let stage = temp.path().join("stage");
        std::fs::create_dir(&stage).unwrap();
        std::fs::write(stage.join("payload.bin"), b"DATA").unwrap();
        std::fs::write(stage.join("payload.bin.part"), b"KEEP").unwrap();
        let store = Arc::new(AcquisitionStore::new(temp.path()));
        let service = Arc::new(AcquisitionService::new(store.clone()));
        let consumer = service.open_consumer("fixture").unwrap();
        let grant = workspace(&stage);
        let demand = AcquisitionDemand {
            consumer: "fixture".into(),
            operation: "stale-generation-handoff".into(),
        };
        let selection = manifest("payload.bin");
        let first_service = service.clone();
        let first_demand = demand.clone();
        let first_selection = selection.clone();
        let first_workspace = grant.identity().clone();
        let outcome = tokio::time::timeout(timeout, async {
            let stale = consumer
                .scope
                .run_worker_invocation(move |context| async move {
                    first_service
                        .begin(
                            &context,
                            first_demand,
                            first_selection,
                            first_workspace,
                            None,
                        )
                        .await
                })
                .await?;
            let original = stale.record.clone();
            let before = store.acquisitions()?;
            let successor_service = service.clone();
            let successor_workspace = grant.clone();
            let observation = consumer
                .scope
                .run_worker_invocation(move |context| async move {
                    let same_scope = context.shares_scope(&stale.context);
                    let same_generation = context.generation().matches(stale.context.generation());
                    let successor_current = context.is_current_role(TaskRole::Worker);
                    let rejection = successor_service
                        .files_ready(&context, stale, successor_workspace)
                        .await;
                    Ok((same_scope, same_generation, successor_current, rejection))
                })
                .await?;
            Ok::<_, PumasError>((original, before, observation))
        })
        .await;
        // Drain even if an invocation failed/timed out before reporting any
        // assertion, so the workspace and durable snapshot cannot race effects.
        let consumer_drain = tokio::time::timeout(timeout, consumer.shutdown()).await;
        let service_drain = tokio::time::timeout(timeout, service.shutdown()).await;
        consumer_drain.expect("consumer must drain").unwrap();
        service_drain.expect("service must drain").unwrap();
        let (original, before, (same_scope, same_generation, successor_current, rejection)) =
            outcome.expect("worker generations must complete").unwrap();
        assert!(same_scope, "the successor must use the same consumer scope");
        assert!(
            !same_generation,
            "the successor must use a fresh generation"
        );
        assert!(successor_current);
        assert!(
            matches!(rejection, Err(PumasError::Validation { field, message })
            if field == "acquisition.custody"
                && message == "Verified handoff belongs to another operation generation")
        );
        assert_eq!(before.len(), 1);
        assert_eq!(original.demand, demand);
        assert_eq!(original.manifest, selection);
        assert_eq!(original.workspace, *grant.identity());
        assert!(matches!(original.phase, AcquisitionPhase::Transferring));
        assert!(original.files.is_empty());
        assert_eq!(store.acquisitions().unwrap(), before);
        assert!(service.consumer_receipt(original.id).unwrap().is_none());
        // Opening an existing partial for append does not alter its bytes and
        // also proves seal() never made the shared workspace read-only.
        drop(
            grant
                .open_part("payload.bin", true)
                .expect("workspace must remain unsealed"),
        );
        assert_eq!(std::fs::read(stage.join("payload.bin")).unwrap(), b"DATA");
        assert_eq!(
            std::fs::read(stage.join("payload.bin.part")).unwrap(),
            b"KEEP"
        );
        assert_eq!(std::fs::read_dir(&stage).unwrap().count(), 2);
    }

    #[tokio::test]
    async fn concurrent_stale_worker_handoff_cannot_mutate_durable_acquisition() {
        use crate::acquisition::task_custody::TaskRole;

        let timeout = Duration::from_secs(5);
        let temp = tempfile::TempDir::new().unwrap();
        let stage = temp.path().join("stage");
        std::fs::create_dir(&stage).unwrap();
        std::fs::write(stage.join("payload.bin"), b"DATA").unwrap();
        std::fs::write(stage.join("payload.bin.part"), b"KEEP").unwrap();
        let store = Arc::new(AcquisitionStore::new(temp.path()));
        let service = Arc::new(AcquisitionService::new(store.clone()));
        let consumer = Arc::new(service.open_consumer("fixture").unwrap());
        let demand = AcquisitionDemand {
            consumer: "fixture".into(),
            operation: "concurrent-stale-handoff".into(),
        };
        let selection = manifest("payload.bin");
        let grant = workspace(&stage);

        // Gate every ordering step: the stale worker parks after admission
        // while its successor seals and hands off the same demand, then the
        // stale worker attempts its own handoff with its still-live context.
        let (stale_begun_sender, stale_begun) = tokio::sync::oneshot::channel::<()>();
        let (release_sender, release_receiver) = tokio::sync::oneshot::channel::<()>();
        let stale_consumer = consumer.clone();
        let stale_service = service.clone();
        let stale_demand = demand.clone();
        let stale_selection = selection.clone();
        let stale_grant = grant.clone();
        let stale = tokio::spawn(async move {
            stale_consumer
                .scope
                .run_worker_invocation(move |context| async move {
                    let operation = stale_service
                        .begin(
                            &context,
                            stale_demand,
                            stale_selection,
                            stale_grant.identity().clone(),
                            None,
                        )
                        .await?;
                    let operation_id = operation.record.id;
                    let _ = stale_begun_sender.send(());
                    release_receiver.await.map_err(|_| {
                        PumasError::Other("Successor did not release the stale worker".into())
                    })?;
                    // The stale generation is still installed, so its own
                    // seal/handoff passes custody and must fail only at the
                    // durable store comparison.
                    let current = context.is_current_role(TaskRole::Worker);
                    let workspace = stale_grant.clone();
                    let rejection = stale_service
                        .files_ready(&context, operation, workspace)
                        .await
                        .err();
                    Ok::<_, PumasError>((operation_id, current, rejection))
                })
                .await
        });
        let successor_consumer = consumer.clone();
        let successor_service = service.clone();
        let successor_demand = demand.clone();
        let successor_selection = selection.clone();
        let successor_grant = grant.clone();
        let successor = tokio::spawn(async move {
            successor_consumer
                .scope
                .run_worker_invocation(move |context| async move {
                    stale_begun.await.map_err(|_| {
                        PumasError::Other("Stale worker never reached admission".into())
                    })?;
                    let operation = successor_service
                        .begin(
                            &context,
                            successor_demand,
                            successor_selection,
                            successor_grant.identity().clone(),
                            None,
                        )
                        .await?;
                    let lease = successor_service
                        .files_ready(&context, operation, successor_grant)
                        .await?;
                    Ok::<_, PumasError>(lease.record().clone())
                })
                .await
        });

        let outcome = tokio::time::timeout(timeout, async {
            let successor_record = successor
                .await
                .map_err(|error| PumasError::Other(error.to_string()))??;
            // Observe the winner's durable state while the stale worker is
            // still parked; its later attempt must leave all of this unchanged.
            let winner = store.acquisitions()?;
            let winner_final = std::fs::read(stage.join("payload.bin"))
                .map_err(|error| PumasError::Other(error.to_string()))?;
            let winner_partial = std::fs::read(stage.join("payload.bin.part"))
                .map_err(|error| PumasError::Other(error.to_string()))?;
            let winner_entries = std::fs::read_dir(&stage)
                .map_err(|error| PumasError::Other(error.to_string()))?
                .count();
            release_sender.send(()).map_err(|_| {
                PumasError::Other("Stale worker disappeared before its handoff".into())
            })?;
            let (stale_id, stale_current, rejection) = stale
                .await
                .map_err(|error| PumasError::Other(error.to_string()))??;
            Ok::<_, PumasError>((
                successor_record,
                winner,
                winner_final,
                winner_partial,
                winner_entries,
                stale_id,
                stale_current,
                rejection,
            ))
        })
        .await;
        // Drain even if an invocation failed/timed out before reporting any
        // assertion, so the workspace and durable snapshot cannot race effects.
        let consumer_drain = tokio::time::timeout(timeout, consumer.shutdown()).await;
        let service_drain = tokio::time::timeout(timeout, service.shutdown()).await;
        consumer_drain.expect("consumer must drain").unwrap();
        service_drain.expect("service must drain").unwrap();
        let (
            successor_record,
            winner,
            winner_final,
            winner_partial,
            winner_entries,
            stale_id,
            stale_current,
            rejection,
        ) = outcome.expect("worker generations must complete").unwrap();

        assert_eq!(stale_id, successor_record.id);
        assert_eq!(successor_record.demand, demand);
        assert_eq!(successor_record.manifest, selection);
        assert_eq!(successor_record.workspace, *grant.identity());
        assert!(matches!(
            successor_record.phase,
            AcquisitionPhase::Using { .. }
        ));
        assert_eq!(successor_record.files.len(), 1);
        assert!(
            stale_current,
            "the stale generation must still be an installed worker at its handoff"
        );
        assert!(
            matches!(rejection, Some(PumasError::Validation { ref field, ref message })
            if field == "acquisition.custody"
                && message == "Acquisition readiness is stale")
        );
        // The losing seal/handoff mutated no durable row, issued no receipt,
        // and changed neither the final nor the partial bytes.
        assert_eq!(store.acquisitions().unwrap(), winner);
        assert_eq!(winner.len(), 1);
        assert_eq!(winner.get(&successor_record.id), Some(&successor_record));
        assert!(service
            .consumer_receipt(successor_record.id)
            .unwrap()
            .is_none());
        assert_eq!(winner_final, b"DATA");
        assert_eq!(winner_partial, b"KEEP");
        assert_eq!(winner_entries, 2);
        assert_eq!(std::fs::read(stage.join("payload.bin")).unwrap(), b"DATA");
        assert_eq!(
            std::fs::read(stage.join("payload.bin.part")).unwrap(),
            b"KEEP"
        );
        assert_eq!(std::fs::read_dir(&stage).unwrap().count(), 2);
    }

    #[tokio::test]
    async fn pause_stops_partial_writes_and_preserves_transfer_demand() {
        controlled_partial_transfer_stops_writing(true).await;
    }

    #[tokio::test]
    async fn cold_service_reopen_restarts_partial_transfer_from_zero() {
        let timeout = Duration::from_secs(5);
        let temp = tempfile::TempDir::new().unwrap();
        let stage = temp.path().join("stage");
        std::fs::create_dir(&stage).unwrap();
        let store = Arc::new(AcquisitionStore::new(temp.path()));
        let first_service = Arc::new(AcquisitionService::new(store.clone()));
        let first_consumer = Arc::new(first_service.open_consumer("fixture").unwrap());
        let demand = AcquisitionDemand {
            consumer: "fixture".into(),
            operation: "cold-owner-reopen".into(),
        };
        let manifest = manifest_with_size("payload.bin", 8);
        let first_demand = demand.clone();
        let first_manifest = manifest.clone();
        let first_workspace = workspace(&stage);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}/fixture", listener.local_addr().unwrap());
        let first_url = url.clone();
        let (first_sent_sender, first_sent) = tokio::sync::oneshot::channel();
        let (continue_sender, continue_receiver) = tokio::sync::oneshot::channel();
        let (range_sender, range_receiver) = tokio::sync::oneshot::channel();
        let server = tokio::spawn(async move {
            let (mut initial, _) = listener.accept().await.unwrap();
            let mut initial_request = [0_u8; 2048];
            assert!(initial.read(&mut initial_request).await.unwrap() > 0);
            initial
                .write_all(
                    b"HTTP/1.1 200 OK\r\nContent-Length: 8\r\nETag: \"stable-v1\"\r\nConnection: close\r\n\r\nDATA",
                )
                .await
                .unwrap();
            let _ = first_sent_sender.send(());
            if continue_receiver.await.is_ok() {
                let _ = initial.write_all(b"TAIL").await;
            }

            let (mut reopened, _) = listener.accept().await.unwrap();
            let mut request = Vec::new();
            while !request.ends_with(b"\r\n\r\n") {
                assert!(request.len() < 8192, "fixture request headers too large");
                request.push(reopened.read_u8().await.unwrap());
            }
            let request = String::from_utf8(request).unwrap();
            let had_range = request.lines().any(|line| {
                line.split_once(':')
                    .is_some_and(|(name, _)| name.eq_ignore_ascii_case("range"))
            });
            let _ = range_sender.send(had_range);
            if had_range {
                reopened
                    .write_all(
                        b"HTTP/1.1 206 Partial Content\r\nContent-Length: 4\r\nContent-Range: bytes 4-7/8\r\nETag: \"changed-v2\"\r\nConnection: close\r\n\r\nEVIL",
                    )
                    .await
                    .unwrap();
            } else {
                reopened
                    .write_all(
                        b"HTTP/1.1 200 OK\r\nContent-Length: 8\r\nETag: \"changed-v2\"\r\nConnection: close\r\n\r\nDATATAIL",
                    )
                    .await
                    .unwrap();
            }
        });

        let (progress_sender, mut progress) = tokio::sync::watch::channel(0_u64);
        let host = ControlledHost {
            paused: Arc::new(AtomicBool::new(false)),
            cancelled: Arc::new(AtomicBool::new(false)),
            pause_wake: Arc::new(tokio::sync::Notify::new()),
            cancel_wake: Arc::new(tokio::sync::Notify::new()),
            progress: progress_sender,
        };
        let controls = host.clone();
        let first_transfer_consumer = first_consumer.clone();
        let mut first_transfer = tokio::spawn(async move {
            first_transfer_consumer
                .acquire_http(
                    AcquisitionHttpRequest {
                        demand: first_demand,
                        manifest: first_manifest,
                        workspace: first_workspace,
                        sources: vec![AcquisitionHttpSource {
                            url: first_url,
                            authorization: None,
                        }],
                        retry: retry(),
                    },
                    reqwest::Client::new(),
                    Box::new(host),
                    |_| async { Ok::<((), Value), PumasError>(((), Value::Null)) },
                    |(), _receipt| async { Ok::<(), PumasError>(()) },
                )
                .await
        });

        tokio::time::timeout(timeout, async {
            first_sent.await.unwrap();
            while *progress.borrow_and_update() < 4 {
                progress.changed().await.unwrap();
            }
        })
        .await
        .expect("first transfer must write DATA before pause");
        controls.pause();
        let paused = tokio::time::timeout(timeout, &mut first_transfer)
            .await
            .expect("pause must stop the first transfer")
            .unwrap();
        assert!(matches!(paused, Err(PumasError::DownloadPaused)));
        continue_sender.send(()).unwrap();
        assert_eq!(
            std::fs::read(stage.join("payload.bin.part")).unwrap(),
            b"DATA"
        );

        drop(first_transfer);
        drop(first_consumer);
        drop(first_service);
        let reopened_service = Arc::new(AcquisitionService::new(store));
        let reopened_consumer = reopened_service.open_consumer("fixture").unwrap();
        let reopened_receipt = tokio::time::timeout(
            timeout,
            reopened_consumer.acquire_http(
                AcquisitionHttpRequest {
                    demand,
                    manifest,
                    workspace: workspace(&stage),
                    sources: vec![AcquisitionHttpSource {
                        url,
                        authorization: None,
                    }],
                    retry: retry(),
                },
                reqwest::Client::new(),
                Box::new(Host),
                |_| async { Ok::<((), Value), PumasError>(((), Value::Null)) },
                |(), receipt| async { Ok::<_, PumasError>(receipt) },
            ),
        )
        .await
        .expect("cold recovery must finish a fresh transfer")
        .unwrap();
        let had_range = range_receiver.await.unwrap();
        tokio::time::timeout(timeout, server)
            .await
            .expect("HTTP fixture must finish")
            .unwrap();
        assert_eq!(
            std::fs::read(stage.join("payload.bin")).unwrap(),
            b"DATATAIL"
        );
        assert!(!had_range, "cold owner must not send Range");
        assert!(!stage.join("payload.bin.part").exists());
        assert_eq!(reopened_receipt.verified_files[0].bytes, 8);
    }

    #[tokio::test]
    async fn digestless_full_size_partial_is_replaced_by_fresh_http_body() {
        let temp = tempfile::TempDir::new().unwrap();
        let stage = temp.path().join("stage");
        std::fs::create_dir(&stage).unwrap();
        std::fs::write(stage.join("payload.bin.part"), b"DATAEVIL").unwrap();
        let store = Arc::new(AcquisitionStore::new(temp.path()));
        let service = Arc::new(AcquisitionService::new(store));
        let consumer = service.open_consumer("fixture").unwrap();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}/fixture", listener.local_addr().unwrap());
        let mut server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut request = Vec::new();
            while !request.ends_with(b"\r\n\r\n") {
                assert!(request.len() < 8192);
                request.push(socket.read_u8().await.unwrap());
            }
            let request = String::from_utf8(request).unwrap().to_ascii_lowercase();
            assert!(!request.contains("range:"));
            socket
                .write_all(
                    b"HTTP/1.1 200 OK\r\nContent-Length: 8\r\nConnection: close\r\n\r\nDATATAIL",
                )
                .await
                .unwrap();
        });
        let result = tokio::time::timeout(
            Duration::from_secs(5),
            consumer.acquire_http(
                AcquisitionHttpRequest {
                    demand: AcquisitionDemand {
                        consumer: "fixture".into(),
                        operation: "full-size-digestless".into(),
                    },
                    manifest: manifest_with_size("payload.bin", 8),
                    workspace: workspace(&stage),
                    sources: vec![AcquisitionHttpSource {
                        url,
                        authorization: None,
                    }],
                    retry: retry(),
                },
                reqwest::Client::new(),
                Box::new(Host),
                |_| async { Ok::<((), Value), PumasError>(((), Value::Null)) },
                |(), _receipt| async { Ok::<(), PumasError>(()) },
            ),
        )
        .await;
        if result.is_err() {
            server.abort();
        }
        let joined = tokio::time::timeout(Duration::from_secs(5), &mut server).await;
        if joined.is_err() {
            server.abort();
            let _ = server.await;
        }
        consumer.shutdown().await.unwrap();
        service.shutdown().await.unwrap();
        result.expect("fresh full transfer must finish").unwrap();
        joined.expect("fresh HTTP fixture must join").unwrap();
        assert_eq!(
            std::fs::read(stage.join("payload.bin")).unwrap(),
            b"DATATAIL"
        );
        assert!(!stage.join("payload.bin.part").exists());
    }

    #[tokio::test]
    async fn digest_backed_full_size_partial_requires_expected_sha256_before_publication() {
        for bytes in [b"DATATAIL", b"DATAEVIL"] {
            let temp = tempfile::TempDir::new().unwrap();
            let stage = temp.path().join("stage");
            std::fs::create_dir(&stage).unwrap();
            std::fs::write(stage.join("payload.bin.part"), bytes).unwrap();
            let store = Arc::new(AcquisitionStore::new(temp.path()));
            let service = Arc::new(AcquisitionService::new(store.clone()));
            let consumer = service.open_consumer("fixture").unwrap();
            let selected = manifest_with_size("payload.bin", 8);
            let selection = ArtifactManifest::new(
                selected.source().clone(),
                vec![ArtifactFile::new(
                    "payload.bin",
                    "payload",
                    Some(8),
                    Some(
                        super::super::Sha256Evidence::new(
                            "publisher.sha256",
                            hex::encode(Sha256::digest(b"DATATAIL")),
                        )
                        .unwrap(),
                    ),
                    FileVerificationRequirement::Sha256,
                )
                .unwrap()],
            )
            .unwrap();
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let url = format!("http://{}/fixture", listener.local_addr().unwrap());
            let result = tokio::time::timeout(
                Duration::from_secs(5),
                consumer.acquire_http(
                    AcquisitionHttpRequest {
                        demand: AcquisitionDemand {
                            consumer: "fixture".into(),
                            operation: "full-size-digest".into(),
                        },
                        manifest: selection,
                        workspace: workspace(&stage),
                        sources: vec![AcquisitionHttpSource {
                            url,
                            authorization: None,
                        }],
                        retry: retry(),
                    },
                    reqwest::Client::new(),
                    Box::new(Host),
                    |_| async { Ok::<((), Value), PumasError>(((), Value::Null)) },
                    |(), _receipt| async { Ok::<(), PumasError>(()) },
                ),
            )
            .await
            .expect("digest check must finish without HTTP");
            consumer.shutdown().await.unwrap();
            service.shutdown().await.unwrap();
            assert!(
                tokio::time::timeout(Duration::from_millis(50), listener.accept())
                    .await
                    .is_err()
            );
            if bytes == b"DATATAIL" {
                result.unwrap();
                assert_eq!(std::fs::read(stage.join("payload.bin")).unwrap(), bytes);
            } else {
                assert!(matches!(result, Err(PumasError::HashMismatch { .. })));
                assert!(!stage.join("payload.bin").exists());
                assert_eq!(
                    std::fs::read(stage.join("payload.bin.part")).unwrap(),
                    bytes
                );
                let records = store.acquisitions().unwrap();
                let record = records.values().next().unwrap();
                assert!(matches!(record.phase, AcquisitionPhase::Transferring));
                assert!(service.consumer_receipt(record.id).unwrap().is_none());
            }
        }
    }

    #[derive(Clone, Copy)]
    enum WarmReply {
        Partial,
        Full,
        ChangedValidator,
        MissingValidator,
        WeakValidator,
        DifferentResource,
        MutatedPrefix,
        CapacityPressure,
        UnknownPublicationPressure,
        MetadataPressure,
    }

    #[tokio::test]
    async fn warm_same_validator_full_response_replaces_prefix_from_zero() {
        assert_warm_continuation(WarmReply::Full).await;
    }

    #[tokio::test]
    async fn warm_changed_missing_or_weak_validator_refuses_without_append() {
        for reply in [
            WarmReply::ChangedValidator,
            WarmReply::MissingValidator,
            WarmReply::WeakValidator,
        ] {
            assert_warm_continuation(reply).await;
        }
    }

    #[tokio::test]
    async fn warm_same_etag_on_different_effective_resource_refuses_without_append() {
        assert_warm_continuation(WarmReply::DifferentResource).await;
    }

    #[tokio::test]
    async fn warm_mutated_prefix_restarts_without_range() {
        assert_warm_continuation(WarmReply::MutatedPrefix).await;
    }

    #[tokio::test]
    async fn paused_http_acquisition_resumes_same_demand_with_range() {
        assert_warm_continuation(WarmReply::Partial).await;
    }

    #[tokio::test]
    async fn checkpoint_capacity_pressure_preserves_pause_and_restarts_from_zero() {
        assert_warm_continuation(WarmReply::CapacityPressure).await;
    }

    #[tokio::test]
    async fn checkpoint_metadata_pressure_preserves_pause_and_restarts_from_zero() {
        assert_warm_continuation(WarmReply::MetadataPressure).await;
    }

    #[tokio::test]
    async fn checkpoint_pressure_preserves_unknown_consumer_publication_custody() {
        assert_warm_continuation(WarmReply::UnknownPublicationPressure).await;
    }

    async fn assert_warm_continuation(reply: WarmReply) {
        use futures::FutureExt;

        let timeout = Duration::from_secs(5);
        let temp = tempfile::TempDir::new().unwrap();
        let stage = temp.path().join("stage");
        std::fs::create_dir(&stage).unwrap();
        let store = Arc::new(AcquisitionStore::new(temp.path()));
        let service = Arc::new(AcquisitionService::new(store.clone()));
        let consumer = Arc::new(service.open_consumer("fixture").unwrap());
        let demand = AcquisitionDemand {
            consumer: "fixture".into(),
            operation: if matches!(reply, WarmReply::MetadataPressure) {
                "x".repeat(CHECKPOINT_METADATA_LIMIT)
            } else {
                "pause-then-resume".into()
            },
        };
        let manifest = manifest_with_size("payload.bin", 8);
        let workspace = workspace(&stage);
        if matches!(
            reply,
            WarmReply::CapacityPressure | WarmReply::UnknownPublicationPressure
        ) {
            let (record, prefix) = checkpoint_fixture(&workspace);
            for index in 0..service.supervisor.checkpoint_limit() {
                assert!(retain_fixture(
                    &service, &workspace, &record, index, &prefix, "retained"
                ));
            }
        }
        let (url, first_server, first_sent, continue_sender) =
            serve_after_gate(b"DATA", b"TAIL").await;
        let mut first_server = Some(first_server);
        let mut continue_sender = Some(continue_sender);
        let mut second_server = None;
        let (progress_sender, mut progress) = tokio::sync::watch::channel(0_u64);
        let host = ControlledHost {
            paused: Arc::new(AtomicBool::new(false)),
            cancelled: Arc::new(AtomicBool::new(false)),
            pause_wake: Arc::new(tokio::sync::Notify::new()),
            cancel_wake: Arc::new(tokio::sync::Notify::new()),
            progress: progress_sender,
        };
        let controls = host.clone();
        let first_consumer = consumer.clone();
        let first_request = AcquisitionHttpRequest {
            demand: demand.clone(),
            manifest: manifest.clone(),
            workspace: workspace.clone(),
            sources: vec![AcquisitionHttpSource {
                url: url.clone(),
                authorization: None,
            }],
            retry: retry(),
        };
        let first_transfer = tokio::spawn(async move {
            first_consumer
                .acquire_http(
                    first_request,
                    reqwest::Client::new(),
                    Box::new(host),
                    |_| async { Ok::<((), Value), PumasError>(((), Value::Null)) },
                    |(), _receipt| async { Ok::<(), PumasError>(()) },
                )
                .await
        });
        let mut first_transfer = Some(first_transfer);
        // Keep all gates and task handles outside the assertion block so even
        // a panic or timeout must pass through explicit joins and custody drain.
        let outcome = std::panic::AssertUnwindSafe(async {

        tokio::time::timeout(timeout, async {
            first_sent.await.unwrap();
            while *progress.borrow_and_update() < 4 {
                progress.changed().await.unwrap();
            }
        })
        .await
        .expect("first transfer must write DATA before pause");
        let before_pause = store.acquisitions().unwrap();
        assert_eq!(before_pause.len(), 1);
        let original = before_pause.values().next().unwrap().clone();
        assert_eq!(original.demand, demand);
        assert_eq!(original.manifest, manifest);
        assert_eq!(original.workspace, *workspace.identity());
        assert!(matches!(original.phase, AcquisitionPhase::Transferring));
        assert!(original.files.is_empty());

        let checkpoint_count = service.checkpoints.lock().unwrap().len();
        controls.pause();
        let paused = tokio::time::timeout(timeout, first_transfer.as_mut().unwrap())
            .await
            .expect("pause must stop the stalled transfer");
        first_transfer.take();
        let paused = paused.unwrap();
        // Release the source only after the public pause has completed.
        continue_sender.take().unwrap().send(()).unwrap();
        let first_join = tokio::time::timeout(timeout, first_server.as_mut().unwrap())
            .await
            .expect("first HTTP fixture must finish");
        first_server.take();
        first_join.unwrap();
        assert!(matches!(paused, Err(PumasError::DownloadPaused)));
        if matches!(reply, WarmReply::CapacityPressure | WarmReply::UnknownPublicationPressure | WarmReply::MetadataPressure) {
            assert_eq!(service.checkpoints.lock().unwrap().len(), checkpoint_count);
            assert!(service.lookup_checkpoint((original.id, 0)).unwrap().is_none());
        }
        assert_eq!(
            std::fs::read(stage.join("payload.bin.part")).unwrap(),
            b"DATA"
        );
        assert!(!stage.join("payload.bin").exists());
        assert_eq!(store.acquisitions().unwrap(), before_pause);
        assert!(service.consumer_receipt(original.id).unwrap().is_none());

        if matches!(reply, WarmReply::MutatedPrefix) {
            std::fs::write(stage.join("payload.bin.part"), b"EVIL").unwrap();
        }
        let address = reqwest::Url::parse(&url).unwrap().socket_addrs(|| None).unwrap()[0];
        let listener = tokio::net::TcpListener::bind(address).await.unwrap();
        second_server = Some(tokio::spawn(async move {
            tokio::time::timeout(timeout, async {
                let (mut socket, _) = listener.accept().await.unwrap();
                let mut request = Vec::new();
                while !request.ends_with(b"\r\n\r\n") {
                    assert!(request.len() < 8192, "fixture request headers too large");
                    let byte = socket.read_u8().await.unwrap();
                    request.push(byte);
                }
                let request = String::from_utf8(request).unwrap();
                let request = request.to_ascii_lowercase();
                if matches!(reply, WarmReply::MutatedPrefix | WarmReply::CapacityPressure | WarmReply::UnknownPublicationPressure | WarmReply::MetadataPressure) {
                    assert!(!request.contains("range:"));
                    assert!(!request.contains("if-match:"));
                } else {
                    assert!(request.contains("range: bytes=4-"));
                    assert!(request.contains("if-match: \"fixture-v1\""));
                }
                if matches!(reply, WarmReply::CapacityPressure) {
                    socket.write_all(b"HTTP/1.1 503 Service Unavailable\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").await.unwrap();
                    drop(socket);
                    socket = listener.accept().await.unwrap().0;
                    let mut retry_request = Vec::new();
                    while !retry_request.ends_with(b"\r\n\r\n") {
                        assert!(retry_request.len() < 8192);
                        retry_request.push(socket.read_u8().await.unwrap());
                    }
                    let retry_request = String::from_utf8(retry_request).unwrap().to_ascii_lowercase();
                    assert!(!retry_request.contains("range:"));
                    assert!(!retry_request.contains("if-match:"));
                }
                if matches!(reply, WarmReply::DifferentResource) {
                    socket.write_all(b"HTTP/1.1 302 Found\r\nLocation: /different-object\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").await.unwrap();
                    drop(socket);
                    socket = listener.accept().await.unwrap().0;
                    let mut redirected = Vec::new();
                    while !redirected.ends_with(b"\r\n\r\n") {
                        assert!(redirected.len() < 8192);
                        redirected.push(socket.read_u8().await.unwrap());
                    }
                    assert!(String::from_utf8(redirected).unwrap().starts_with("GET /different-object "));
                }
                let etag = match reply {
                    WarmReply::ChangedValidator => "ETag: \"changed-v2\"\r\n",
                    WarmReply::MissingValidator => "",
                    WarmReply::WeakValidator => "ETag: W/\"fixture-v1\"\r\n",
                    _ => "ETag: \"fixture-v1\"\r\n",
                };
                let response = if matches!(reply, WarmReply::Full | WarmReply::MutatedPrefix | WarmReply::CapacityPressure | WarmReply::UnknownPublicationPressure | WarmReply::MetadataPressure) {
                    format!("HTTP/1.1 200 OK\r\n{etag}Content-Length: 8\r\nConnection: close\r\n\r\nDATATAIL")
                } else {
                    format!("HTTP/1.1 206 Partial Content\r\n{etag}Content-Length: 4\r\nContent-Range: bytes 4-7/8\r\nConnection: close\r\n\r\nTAIL")
                };
                socket.write_all(response.as_bytes()).await.unwrap();
            })
            .await
            .expect("resumed HTTP fixture must finish");
        }));
        let mut resumed_retry = retry();
        if matches!(reply, WarmReply::CapacityPressure) {
            resumed_retry.attempts = Some(2);
            resumed_retry.backoff = resumed_retry.backoff.with_base_delay(Duration::from_millis(1));
        }
        let published_receipt = tokio::time::timeout(
            timeout,
            consumer.acquire_http(
                AcquisitionHttpRequest {
                    demand: demand.clone(),
                    manifest: manifest.clone(),
                    workspace: workspace.clone(),
                    sources: vec![AcquisitionHttpSource {
                        url,
                        authorization: None,
                    }],
                    retry: resumed_retry,
                },
                reqwest::Client::new(),
                Box::new(Host),
                |_| async { Ok::<((), Value), PumasError>(((), Value::Null)) },
                move |(), receipt| async move {
                    if matches!(reply, WarmReply::UnknownPublicationPressure) {
                        // Consumer publication reports uncertainty after receipt
                        // issuance; optional retention pressure must preserve it.
                        return Err(PumasError::Other("Fixture publication visibility unknown".into()));
                    }
                    Ok::<_, PumasError>(receipt)
                },
            ),
        )
        .await
        .expect("resume must finish");
        let second_join = tokio::time::timeout(timeout, second_server.as_mut().unwrap())
            .await
            .expect("resumed HTTP fixture must join");
        second_server.take();
        second_join.unwrap();
        if matches!(reply, WarmReply::ChangedValidator | WarmReply::MissingValidator | WarmReply::WeakValidator | WarmReply::DifferentResource) {
            assert!(matches!(published_receipt, Err(PumasError::Validation { .. })));
            assert_eq!(std::fs::read(stage.join("payload.bin.part")).unwrap(), b"DATA");
            assert!(!stage.join("payload.bin").exists());
            assert_eq!(store.acquisitions().unwrap(), before_pause);
            assert!(service.consumer_receipt(original.id).unwrap().is_none());
            return;
        }
        if matches!(reply, WarmReply::UnknownPublicationPressure) {
            assert!(matches!(published_receipt, Err(PumasError::Other(ref message)) if message == "Fixture publication visibility unknown"));
            let records = store.acquisitions().unwrap();
            let retained = records.get(&original.id).unwrap();
            assert!(matches!(retained.phase, AcquisitionPhase::Using { .. }));
            assert_eq!(retained.demand, original.demand);
            assert!(service.consumer_receipt(original.id).unwrap().is_some());
            assert_eq!(std::fs::read(stage.join("payload.bin")).unwrap(), b"DATATAIL");
            assert_eq!(service.checkpoints.lock().unwrap().len(), checkpoint_count);
            return;
        }
        let published_receipt = published_receipt.unwrap();
        assert_eq!(
            std::fs::read(stage.join("payload.bin")).unwrap(),
            b"DATATAIL"
        );
        assert!(!stage.join("payload.bin.part").exists());
        let records = store.acquisitions().unwrap();
        assert_eq!(
            records.len(),
            1,
            "resume must reuse the original acquisition"
        );
        let resumed = records.get(&original.id).unwrap();
        assert_eq!(resumed.demand, original.demand);
        assert_eq!(resumed.manifest, original.manifest);
        assert_eq!(resumed.workspace, original.workspace);
        assert!(matches!(resumed.phase, AcquisitionPhase::Adopted { .. }));
        assert_eq!(published_receipt.acquisition_id, original.id.to_string());
        assert_eq!(published_receipt.demand, original.demand);
        assert_eq!(
            service.consumer_receipt(original.id).unwrap(),
            Some(published_receipt)
        );

        })
        .catch_unwind()
        .await;

        // A failed setup/pause/resume must release the stalled source and join
        // every task. Dropping a timed-out acquire_http future cancels its
        // invocation; shutdown then awaits the retained worker/blocking effects.
        controls.cancel();
        if let Some(sender) = continue_sender.take() {
            let _ = sender.send(());
        }
        if let Some(transfer) = first_transfer.take() {
            transfer.abort();
            let _ = transfer.await;
        }
        for server in [first_server.take(), second_server.take()]
            .into_iter()
            .flatten()
        {
            server.abort();
            let _ = server.await;
        }
        let consumer_drain = tokio::time::timeout(timeout, consumer.shutdown()).await;
        let service_drain = tokio::time::timeout(timeout, service.shutdown()).await;
        consumer_drain
            .expect("consumer cleanup must drain")
            .unwrap();
        service_drain.expect("service cleanup must drain").unwrap();
        if let Err(panic) = outcome {
            std::panic::resume_unwind(panic);
        }
    }

    #[tokio::test]
    async fn active_cancel_stops_partial_writes_and_preserves_transfer_demand() {
        controlled_partial_transfer_stops_writing(false).await;
    }
}
