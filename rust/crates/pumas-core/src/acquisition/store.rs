//! The single transaction and atomic-publication authority for downloads.json.
//!
//! Model custody is an opaque partition decoded by its model-owned facade.
//! This module never interprets model requests, statuses, or recovery policy.
use super::service::AcquisitionRecord;
use crate::metadata::{
    AtomicJsonTarget, AtomicPublication, AtomicPublishFailure, AtomicPublishFailureKind,
    AtomicPublishResult, AtomicPublishStage, StagingCleanup,
};
use crate::{PumasError, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::fs::File;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};
use uuid::Uuid;

const SCHEMA_VERSION: u32 = 6;
const LOCK_FILE: &str = ".downloads.lock";

#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct AcquisitionDocument {
    schema_version: u32,
    #[serde(with = "super::service::uuid_map")]
    pub(crate) acquisitions: BTreeMap<Uuid, AcquisitionRecord>,
    /// Existing model partitions retain their exact serialized custody shape.
    /// Only the trusted model facade may decode or replace these values.
    #[serde(flatten)]
    legacy: BTreeMap<String, Value>,
}

impl AcquisitionDocument {
    fn empty() -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            acquisitions: BTreeMap::new(),
            legacy: BTreeMap::new(),
        }
    }
    fn validate(&self) -> Result<()> {
        if self.schema_version != SCHEMA_VERSION {
            return Err(invalid_schema());
        }
        for (id, record) in &self.acquisitions {
            record.validate(*id)?;
        }
        Ok(())
    }
}

/// Source-neutral physical store authority. Construction has no I/O effects.
pub struct AcquisitionStore {
    path: PathBuf,
    mutation: Mutex<()>,
}

pub(crate) struct AcquisitionTransaction<'a> {
    _instance_guard: MutexGuard<'a, ()>,
    target: AtomicJsonTarget,
    _os_lock: Option<File>,
    legacy_read_only: bool,
    consumer_settlement: Option<(String, String)>,
}

fn schema(value: &Value) -> Option<u64> {
    value.get("schema_version").and_then(Value::as_u64)
}

pub(crate) fn migration_required() -> PumasError {
    PumasError::Validation {
        field: "acquisition.migration_required".into(),
        message: "Acquisition requires schema 6; stop all old readers/writers and explicitly migrate the supported v4/v5 store offline".into(),
    }
}

fn invalid_schema() -> PumasError {
    PumasError::Validation { field: "downloads.schema_version".into(), message: "Only complete supported schema 4/5 projection and schema 6 acquisition documents are accepted".into() }
}

impl AcquisitionStore {
    pub fn new(data_dir: &Path) -> Self {
        Self {
            path: data_dir.join("downloads.json"),
            mutation: Mutex::new(()),
        }
    }

    /// Eligibility check; does not publish or obtain workspace authority.
    pub fn require_acquisition_schema(&self) -> Result<()> {
        let target = AtomicJsonTarget::open(&self.path)?;
        if let Some(value) = target.read_json::<Value>()? {
            if matches!(schema(&value), Some(4 | 5)) {
                return Err(migration_required());
            }
            let document: AcquisitionDocument = serde_json::from_value(value)?;
            document.validate()?;
        }
        Ok(())
    }

    pub(crate) fn transaction(&self, read_only: bool) -> Result<AcquisitionTransaction<'_>> {
        self.transaction_observed(read_only, || {}, || {})
    }

    pub(crate) fn transaction_observed(
        &self,
        read_only: bool,
        attempting: impl FnOnce(),
        acquired: impl FnOnce(),
    ) -> Result<AcquisitionTransaction<'_>> {
        let guard = self
            .mutation
            .lock()
            .map_err(|_| PumasError::Other("Acquisition store lock is poisoned".into()))?;
        let target = AtomicJsonTarget::open(&self.path)?;
        let legacy = target
            .read_json::<Value>()?
            .is_some_and(|value| matches!(schema(&value), Some(4 | 5)));
        if legacy {
            if !read_only {
                return Err(migration_required());
            }
            return Ok(AcquisitionTransaction {
                _instance_guard: guard,
                target,
                _os_lock: None,
                legacy_read_only: true,
                consumer_settlement: None,
            });
        }
        let os_lock = target.open_lock_file(LOCK_FILE)?;
        attempting();
        os_lock.lock()?;
        acquired();
        // Repeat the schema check after taking the shared lock.
        if !read_only
            && target
                .read_json::<Value>()?
                .is_some_and(|value| matches!(schema(&value), Some(4 | 5)))
        {
            return Err(migration_required());
        }
        Ok(AcquisitionTransaction {
            _instance_guard: guard,
            target,
            _os_lock: Some(os_lock),
            legacy_read_only: false,
            consumer_settlement: None,
        })
    }

    /// Private trusted conversion hook; only the model facade supplies it.
    /// The caller has stopped all old readers/writers. The converter sees the
    /// exact locked source and validates every supported legacy custody field.
    pub(crate) fn migrate_legacy_offline(
        &self,
        convert: impl FnOnce(Value) -> Result<Value>,
    ) -> Result<()> {
        let guard = self
            .mutation
            .lock()
            .map_err(|_| PumasError::Other("Acquisition store lock is poisoned".into()))?;
        let target = AtomicJsonTarget::open(&self.path)?;
        let lock = target.open_lock_file(LOCK_FILE)?;
        lock.lock()?;
        let transaction = AcquisitionTransaction {
            _instance_guard: guard,
            target,
            _os_lock: Some(lock),
            legacy_read_only: false,
            consumer_settlement: None,
        };
        let value = transaction
            .target
            .read_json::<Value>()?
            .ok_or_else(invalid_schema)?;
        if !matches!(schema(&value), Some(4 | 5)) {
            return Err(invalid_schema());
        }
        let legacy = convert(value)?;
        let document = document_with_partition(AcquisitionDocument::empty(), legacy)?;
        require_durable(transaction.publish_document(&document))
    }

    pub(crate) fn update_acquisitions<T>(
        &self,
        update: impl FnOnce(&mut BTreeMap<Uuid, AcquisitionRecord>) -> Result<T>,
    ) -> Result<T> {
        self.update_acquisition_records(update, true)
    }

    pub(crate) fn update_acquisitions_if_changed<T>(
        &self,
        update: impl FnOnce(&mut BTreeMap<Uuid, AcquisitionRecord>) -> Result<T>,
    ) -> Result<T> {
        self.update_acquisition_records(update, false)
    }

    fn update_acquisition_records<T>(
        &self,
        update: impl FnOnce(&mut BTreeMap<Uuid, AcquisitionRecord>) -> Result<T>,
        publish_unchanged: bool,
    ) -> Result<T> {
        let transaction = self.transaction(false)?;
        let mut document = transaction.document()?;
        let before = document.acquisitions.clone();
        let result = update(&mut document.acquisitions)?;
        if !publish_unchanged && document.acquisitions == before {
            return Ok(result);
        }
        document.validate()?;
        require_durable(transaction.publish_document(&document))?;
        Ok(result)
    }

    pub(crate) fn acquisitions(&self) -> Result<BTreeMap<Uuid, AcquisitionRecord>> {
        let transaction = self.transaction(true)?;
        if transaction.legacy_read_only {
            return Ok(BTreeMap::new());
        }
        Ok(transaction.document()?.acquisitions)
    }
}

fn document_with_partition(
    mut document: AcquisitionDocument,
    value: Value,
) -> Result<AcquisitionDocument> {
    let mut legacy = value.as_object().cloned().ok_or_else(invalid_schema)?;
    if schema(&value) != Some(5) || legacy.contains_key("acquisitions") {
        return Err(invalid_schema());
    }
    legacy.remove("schema_version");
    document.legacy = legacy.into_iter().collect();
    document.validate()?;
    Ok(document)
}

impl AcquisitionTransaction<'_> {
    fn document(&self) -> Result<AcquisitionDocument> {
        let Some(value) = self.target.read_json::<Value>()? else {
            return Ok(AcquisitionDocument::empty());
        };
        if schema(&value) != Some(6) {
            return Err(invalid_schema());
        }
        let document: AcquisitionDocument = serde_json::from_value(value)?;
        document.validate()?;
        Ok(document)
    }

    pub(crate) fn model_partition(&self) -> Result<Option<Value>> {
        let Some(value) = self.target.read_json::<Value>()? else {
            return Ok(None);
        };
        if matches!(schema(&value), Some(4 | 5)) && self.legacy_read_only {
            return Ok(Some(value));
        }
        let document = self.document()?;
        if document.legacy.is_empty() {
            return Ok(None);
        }
        let mut value: serde_json::Map<String, Value> = document.legacy.into_iter().collect();
        value.insert("schema_version".into(), 5.into());
        Ok(Some(Value::Object(value)))
    }

    pub(crate) fn publish_model_partition<T: Serialize>(&self, data: &T) -> AtomicPublishResult {
        let document = (|| -> Result<_> {
            if self.legacy_read_only {
                return Err(migration_required());
            }
            let mut document =
                document_with_partition(self.document()?, serde_json::to_value(data)?)?;
            if let Some((consumer, operation)) = &self.consumer_settlement {
                for record in document.acquisitions.values_mut().filter(|record| {
                    &record.demand.consumer == consumer && &record.demand.operation == operation
                }) {
                    if !matches!(
                        record.phase,
                        super::service::AcquisitionPhase::Using { .. }
                            | super::service::AcquisitionPhase::Adopted { .. }
                    ) {
                        record.phase = super::service::AcquisitionPhase::Withdrawn;
                        record.files.clear();
                    }
                }
            }
            document.validate()?;
            Ok(document)
        })();
        match document {
            Ok(document) => self.publish_document(&document),
            Err(error) => Err(Box::new(AtomicPublishFailure {
                stage: AtomicPublishStage::Serialization,
                kind: AtomicPublishFailureKind::InvalidData,
                error,
                cleanup: StagingCleanup::NotRequired,
            })),
        }
    }

    pub(crate) fn stage_consumer_settlement(&mut self, consumer: &str, operation: &str) {
        self.consumer_settlement = Some((consumer.into(), operation.into()));
    }

    fn publish_document(&self, document: &AcquisitionDocument) -> AtomicPublishResult {
        self.target.publish_json(document)
    }
}

fn require_durable(publication: AtomicPublishResult) -> Result<()> {
    match publication {
        Ok(AtomicPublication::Durable) => Ok(()),
        Ok(AtomicPublication::PublishedDurabilityUnknown { error }) => Err(error),
        Ok(AtomicPublication::VisibilityUnknown { error, cleanup }) => Err(AtomicPublishFailure {
            stage: AtomicPublishStage::Rename,
            kind: AtomicPublishFailureKind::Filesystem,
            error,
            cleanup,
        }
        .into_error()),
        Err(failure) => Err(failure.into_error()),
    }
}
