//! Durable acquisition custody shared by source adapters and consumers.
use super::http::{
    open_http_artifact, stream_http_artifact, HttpArtifactSink, HttpAttemptHost, HttpBodyOutcome,
};
use super::store::AcquisitionStore;
use super::task_custody::{TaskContext, TaskCustodyOwner};
use super::workspace::{write_chunk, AcquisitionWorkspace, VerifiedFile, WorkspaceIdentity};
use super::ArtifactManifest;
use crate::{PumasError, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::future::Future;
use std::sync::Arc;
use std::time::{Duration, Instant};
use uuid::Uuid;

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
    pub elapsed: Duration,
    pub backoff: crate::network::RetryConfig,
}

#[async_trait::async_trait]
pub trait AcquisitionHost: HttpAttemptHost {
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
        owned(&self.context, name, work).await
    }
}

/// One durable lifecycle/store owner and the existing shared task supervisor.
/// Source access is ephemeral and is never written to the durable manifest.
pub struct AcquisitionService {
    store: Arc<AcquisitionStore>,
    supervisor: Arc<TaskCustodyOwner>,
}

impl AcquisitionService {
    pub fn new(store: Arc<AcquisitionStore>) -> Self {
        Self {
            store,
            supervisor: Arc::new(TaskCustodyOwner::new()),
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
        })
    }

    pub(crate) fn with_store(&self, store: Arc<AcquisitionStore>) -> Self {
        Self {
            store,
            supervisor: self.supervisor.clone(),
        }
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
        self.supervisor.request_shutdown().wait().await
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
        client: &reqwest::Client,
        url: &str,
        authorization: Option<&str>,
        retry: &AcquisitionRetryPolicy,
        host: &mut dyn AcquisitionHost,
    ) -> Result<u64> {
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
        let started = Instant::now();
        let mut attempt = 0_u32;
        loop {
            attempt = attempt
                .checked_add(1)
                .ok_or_else(|| invalid("Retry counter exhausted"))?;
            host.retry(attempt, None, None).await?;
            if host.cancel_requested() {
                return Err(PumasError::DownloadCancelled);
            }
            if host.pause_requested_now() {
                return Err(PumasError::DownloadPaused);
            }
            let inspect = workspace.clone();
            let path = file.logical_path().to_owned();
            let mut resume = owned(context, "inspect acquisition partial file", move || {
                inspect.file_len(&path, true)
            })
            .await?
            .unwrap_or(0);
            if resume > 0
                && ((!operation.record.manifest.permits_resume(file_index))
                    || (compare_existing && attempt == 1))
            {
                let remove = workspace.clone();
                let path = file.logical_path().to_owned();
                owned(context, "discard unbound acquisition partial", move || {
                    remove.remove_part(&path)
                })
                .await?;
                resume = 0;
            }
            if !compare_existing
                && file.expected_size() == Some(resume)
                && resume > 0
                && operation.record.manifest.permits_resume(file_index)
            {
                let publish = workspace.clone();
                let selected = file.clone();
                return owned(context, "publish complete acquisition partial", move || {
                    publish
                        .publish_part(&selected, false)
                        .map(|receipt| receipt.bytes)
                })
                .await;
            }
            let response = tokio::select! {
                biased;
                _ = host.pause_requested() => return Err(PumasError::DownloadPaused),
                response = open_http_artifact(client, url, &operation.record.manifest, file_index, resume, authorization) => response,
            };
            let outcome = match response {
                Ok(response) => {
                    let open = workspace.clone();
                    let path = file.logical_path().to_owned();
                    let append = response.resumed;
                    let file = owned(context, "open acquisition partial file", move || {
                        open.open_part(&path, append)
                    })
                    .await?;
                    let mut sink = AcquisitionSink {
                        file: Some(file),
                        context,
                    };
                    stream_http_artifact(response, resume, &mut sink, host).await
                }
                Err(error) => Err(error),
            };
            match outcome {
                Ok(HttpBodyOutcome::Complete { .. }) => {
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
                Ok(HttpBodyOutcome::Cancelled) => return Err(PumasError::DownloadCancelled),
                Err(error) => {
                    if !error.is_retryable() || host.cancel_requested() {
                        return Err(error);
                    }
                    if retry.attempts.is_some_and(|limit| attempt >= limit)
                        || (retry.elapsed > Duration::ZERO && started.elapsed() >= retry.elapsed)
                    {
                        return Err(PumasError::DownloadFailed { url: "artifact source".into(), message: format!("Acquisition retry budget exhausted after {attempt} attempts: {error}") });
                    }
                    let delay = retry.backoff.calculate_delay(attempt.saturating_sub(1));
                    host.retry(attempt, Some(delay), Some(&error.to_string()))
                        .await?;
                    tokio::select! {
                        biased;
                        _ = host.pause_requested() => return Err(PumasError::DownloadPaused),
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
        if !context.shares_scope(&operation.context)
            || !context.generation().matches(operation.context.generation())
        {
            return Err(invalid(
                "Verified handoff belongs to another operation generation",
            ));
        }
        let seal = workspace.clone();
        let manifest = operation.record.manifest.clone();
        let files = owned(context, "seal verified acquisition file set", move || {
            seal.seal(&manifest)
        })
        .await?;
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
        client: reqwest::Client,
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
        let service = self.service.clone();
        self.scope
            .run_worker_invocation(move |context| async move {
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
                    service
                        .acquire_file(
                            &context,
                            &operation,
                            &request.workspace,
                            file_index,
                            &client,
                            &source.url,
                            source.authorization.as_deref(),
                            &request.retry,
                            host.as_mut(),
                        )
                        .await?;
                }
                let lease = service
                    .files_ready(&context, operation, request.workspace)
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
}

#[async_trait::async_trait]
impl HttpArtifactSink for AcquisitionSink<'_> {
    async fn write_all(&mut self, bytes: &[u8]) -> Result<()> {
        let mut file = self
            .file
            .take()
            .ok_or_else(|| invalid("Partial file effect is unfinished"))?;
        let bytes = bytes.to_owned();
        self.file = Some(
            owned(self.context, "write acquisition partial file", move || {
                write_chunk(&mut file, &bytes)?;
                Ok(file)
            })
            .await?,
        );
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
        FileVerificationRequirement, RevisionStrength,
    };
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    fn manifest(path: &str) -> ArtifactManifest {
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
                Some(4),
                None,
                FileVerificationRequirement::SizeAndImmutableRevision,
            )
            .unwrap()],
        )
        .unwrap()
    }

    fn workspace(path: &std::path::Path) -> AcquisitionWorkspace {
        let root = crate::platform::capability_fs::open_directory(path).unwrap();
        let check = root.try_clone().unwrap();
        let expected = std::fs::canonicalize(path).unwrap();
        let source = path.to_path_buf();
        AcquisitionWorkspace::from_capability(
            root,
            WorkspaceIdentity {
                root_identity: "fixture-physical-root".into(),
                relative_target: "staging".into(),
            },
            Arc::new(()),
            move || {
                if std::fs::canonicalize(&source)? != expected || !check.dir_metadata()?.is_dir() {
                    return Err(invalid("Fixture grant changed"));
                }
                Ok(())
            },
        )
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

    fn retry() -> AcquisitionRetryPolicy {
        AcquisitionRetryPolicy {
            attempts: Some(1),
            elapsed: Duration::from_secs(5),
            backoff: crate::network::RetryConfig::new(),
        }
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
                        &reqwest::Client::new(),
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
                        &reqwest::Client::new(),
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
}
