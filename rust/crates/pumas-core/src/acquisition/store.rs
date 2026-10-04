//! The single transaction and atomic-publication authority for downloads.json.
//!
//! Model custody is an opaque partition decoded by its model-owned facade.
//! This module never interprets model requests, statuses, or recovery policy.
use super::service::AcquisitionConsumerReceipt;
use super::service::AcquisitionRecord;
pub(crate) use super::service::{AcquisitionProof, AcquisitionTransferProof, AcquisitionUseProof};
use crate::metadata::{
    AtomicJsonTarget, AtomicPublication, AtomicPublishFailure, AtomicPublishFailureKind,
    AtomicPublishResult, AtomicPublishStage, StagingCleanup,
};
use crate::{PumasError, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::fs::File;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};
use uuid::Uuid;

const SCHEMA_VERSION: u32 = 7;
const LOCK_FILE: &str = ".downloads.lock";

#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct AcquisitionDocument {
    schema_version: u32,
    #[serde(with = "super::service::uuid_map")]
    pub(crate) acquisitions: BTreeMap<Uuid, AcquisitionRecord>,
    /// Opaque to the neutral acquisition owner; interpreted by the model
    /// consumer that issued each exact completion proof.
    #[serde(with = "uuid_value_map")]
    pub(crate) consumer_receipts: BTreeMap<Uuid, Value>,
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
            consumer_receipts: BTreeMap::new(),
            legacy: BTreeMap::new(),
        }
    }
    fn validate(&self) -> Result<()> {
        if self.schema_version != SCHEMA_VERSION {
            return Err(invalid_schema());
        }
        validate_acquisition_records(&self.acquisitions)?;
        for id in self.consumer_receipts.keys() {
            let Some(record) = self.acquisitions.get(id) else {
                return Err(invalid_receipt("Receipt has no acquisition record"));
            };
            if !matches!(
                &record.phase,
                super::service::AcquisitionPhase::Using { .. }
                    | super::service::AcquisitionPhase::Adopted { .. }
            ) {
                return Err(invalid_receipt(
                    "Receipt is only valid for a current or adopted consumer use",
                ));
            }
            let value = &self.consumer_receipts[id];
            if value.get("receipt_kind").and_then(Value::as_str)
                == Some("pumas.consumer-completion")
            {
                let receipt: AcquisitionConsumerReceipt = serde_json::from_value(value.clone())
                    .map_err(|_| invalid_receipt("Consumer completion receipt is malformed"))?;
                let lease = match &record.phase {
                    super::service::AcquisitionPhase::Using { lease }
                    | super::service::AcquisitionPhase::Adopted { lease } => *lease,
                    _ => return Err(invalid_receipt("Receipt has no consumer use lease")),
                };
                receipt.validate_for_record(record, lease).map_err(|_| {
                    invalid_receipt("Consumer receipt does not match its acquisition")
                })?;
            } else if record.demand.consumer == "hf.model" {
                validate_hf_receipt_binding(value, record)?;
            } else {
                return Err(invalid_receipt(
                    "Unwrapped receipts are reserved for the HF model consumer",
                ));
            }
        }
        Ok(())
    }
}

/// Validate the stable identity envelope of the legacy HF-owned receipt while
/// leaving its queue and output proof interpretation to the HF facade.
fn validate_hf_receipt_binding(value: &Value, record: &AcquisitionRecord) -> Result<()> {
    const FIELDS: [&str; 12] = [
        "receipt_version",
        "output_proof_version",
        "acquisition_id",
        "use_lease",
        "demand",
        "manifest",
        "workspace",
        "verified_files",
        "download_id",
        "queue_admission",
        "model_id",
        "outputs",
    ];

    let object = value
        .as_object()
        .ok_or_else(|| invalid_receipt("HF completion receipt must be an object"))?;
    let acquisition_id = record.id.to_string();
    let demand = serde_json::to_value(&record.demand)?;
    let manifest = serde_json::to_value(&record.manifest)?;
    let workspace = serde_json::to_value(&record.workspace)?;
    let verified_files = serde_json::to_value(&record.files)?;
    if object.len() != FIELDS.len() || FIELDS.iter().any(|field| !object.contains_key(*field)) {
        return Err(invalid_receipt(
            "HF completion receipt has an unsupported shape",
        ));
    }
    if value.get("receipt_version").and_then(Value::as_u64) != Some(1)
        || value.get("output_proof_version").and_then(Value::as_u64) != Some(1)
        || value.get("acquisition_id").and_then(Value::as_str) != Some(acquisition_id.as_str())
        || value.get("demand") != Some(&demand)
        || value.get("manifest") != Some(&manifest)
        || value.get("workspace") != Some(&workspace)
        || value.get("verified_files") != Some(&verified_files)
        || value
            .get("download_id")
            .and_then(Value::as_str)
            .is_none_or(str::is_empty)
        || !value.get("queue_admission").is_some_and(Value::is_object)
        || value
            .get("model_id")
            .and_then(Value::as_str)
            .is_none_or(str::is_empty)
        || !value.get("outputs").is_some_and(Value::is_object)
    {
        return Err(invalid_receipt(
            "HF completion receipt does not bind the supported acquisition identity",
        ));
    }

    let receipt_lease_text = value
        .get("use_lease")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid_receipt("HF completion receipt has an invalid use lease"))?;
    let receipt_lease = Uuid::parse_str(receipt_lease_text)
        .ok()
        .filter(|lease| !lease.is_nil())
        .ok_or_else(|| invalid_receipt("HF completion receipt has an invalid use lease"))?;
    if receipt_lease.to_string() != receipt_lease_text {
        return Err(invalid_receipt(
            "HF completion receipt use lease is not canonical",
        ));
    }
    let record_lease = match &record.phase {
        super::service::AcquisitionPhase::Using { lease }
        | super::service::AcquisitionPhase::Adopted { lease } => *lease,
        _ => return Err(invalid_receipt("HF receipt has no consumer use lease")),
    };
    if receipt_lease != record_lease || record.demand.consumer != "hf.model" {
        return Err(invalid_receipt(
            "HF completion receipt lease or owner does not match its acquisition",
        ));
    }
    Ok(())
}

fn validate_acquisition_records(acquisitions: &BTreeMap<Uuid, AcquisitionRecord>) -> Result<()> {
    let mut demands = BTreeSet::new();
    let mut active_workspaces = BTreeSet::new();
    for (id, record) in acquisitions {
        record.validate(*id)?;
        if !demands.insert((
            record.demand.consumer.clone(),
            record.demand.operation.clone(),
        )) {
            return Err(PumasError::Validation {
                field: "acquisition.custody".into(),
                message: "Acquisition demand is duplicated in the durable document".into(),
            });
        }
        if matches!(
            record.phase,
            super::service::AcquisitionPhase::Transferring
                | super::service::AcquisitionPhase::FilesReady
                | super::service::AcquisitionPhase::Using { .. }
        ) && !active_workspaces.insert((
            record.workspace.root_identity.clone(),
            record.workspace.relative_target.clone(),
        )) {
            return Err(PumasError::Validation {
                field: "acquisition.custody".into(),
                message: "Multiple active acquisition demands share one workspace".into(),
            });
        }
    }
    Ok(())
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
    receipt_issuance: Option<(AcquisitionRecord, Value)>,
    receipt_settlement: Option<(AcquisitionRecord, Value)>,
}

fn schema(value: &Value) -> Option<u64> {
    value.get("schema_version").and_then(Value::as_u64)
}

pub(crate) fn migration_required() -> PumasError {
    PumasError::Validation {
        field: "acquisition.migration_required".into(),
        message: "Acquisition requires schema 7; stop all old readers/writers and explicitly migrate the supported v4/v5 or schema-6 store offline".into(),
    }
}

fn invalid_schema() -> PumasError {
    PumasError::Validation { field: "downloads.schema_version".into(), message: "Only complete supported schema 4/5 model projections and schema 7 acquisition documents are accepted".into() }
}

fn invalid_receipt(message: &str) -> PumasError {
    PumasError::Validation {
        field: "acquisition.consumer_receipts".into(),
        message: message.into(),
    }
}

mod uuid_value_map {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use serde_json::Value;
    use std::collections::BTreeMap;
    use uuid::Uuid;

    pub(crate) fn serialize<S: Serializer>(
        values: &BTreeMap<Uuid, Value>,
        serializer: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        values
            .iter()
            .map(|(id, value)| (id.to_string(), value))
            .collect::<BTreeMap<_, _>>()
            .serialize(serializer)
    }

    pub(crate) fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> std::result::Result<BTreeMap<Uuid, Value>, D::Error> {
        let encoded = BTreeMap::<String, Value>::deserialize(deserializer)?;
        let mut decoded = BTreeMap::new();
        for (key, value) in encoded {
            let id = Uuid::parse_str(&key).map_err(serde::de::Error::custom)?;
            if key != id.to_string() || decoded.insert(id, value).is_some() {
                return Err(serde::de::Error::custom(
                    "acquisition receipt keys must be canonical and unique",
                ));
            }
        }
        Ok(decoded)
    }
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
        if let Some(value) = target.read_downloads_json_value()? {
            if matches!(schema(&value), Some(4..=6)) {
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
        let observed = target.read_downloads_json_value()?;
        if observed.as_ref().and_then(schema) == Some(6) {
            return Err(migration_required());
        }
        let legacy = observed
            .as_ref()
            .and_then(schema)
            .is_some_and(|version| matches!(version, 4..=5));
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
                receipt_issuance: None,
                receipt_settlement: None,
            });
        }
        let os_lock = target.open_lock_file(LOCK_FILE)?;
        attempting();
        os_lock.lock()?;
        acquired();
        // Repeat the schema check after taking the shared lock.
        if !read_only
            && target
                .read_downloads_json_value()?
                .is_some_and(|value| matches!(schema(&value), Some(4..=6)))
        {
            return Err(migration_required());
        }
        Ok(AcquisitionTransaction {
            _instance_guard: guard,
            target,
            _os_lock: Some(os_lock),
            legacy_read_only: false,
            consumer_settlement: None,
            receipt_issuance: None,
            receipt_settlement: None,
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
            receipt_issuance: None,
            receipt_settlement: None,
        };
        let value = transaction
            .target
            .read_downloads_json_value()?
            .ok_or_else(invalid_schema)?;
        if !matches!(schema(&value), Some(4..=6)) {
            return Err(invalid_schema());
        }
        let document = match schema(&value) {
            Some(4 | 5) => {
                let legacy = convert(value)?;
                document_with_partition(AcquisitionDocument::empty(), legacy)?
            }
            Some(6) => {
                if value.get("consumer_receipts").is_some() {
                    return Err(invalid_schema());
                }
                let mut upgraded_value = value;
                let object = upgraded_value.as_object_mut().ok_or_else(invalid_schema)?;
                object.insert("schema_version".into(), Value::from(SCHEMA_VERSION));
                object.insert(
                    "consumer_receipts".into(),
                    Value::Object(Default::default()),
                );
                let mut document: AcquisitionDocument = serde_json::from_value(upgraded_value)?;
                validate_acquisition_records(&document.acquisitions)?;
                document.consumer_receipts.clear();
                if !document.legacy.is_empty() {
                    let mut model_projection: serde_json::Map<String, Value> =
                        document.legacy.clone().into_iter().collect();
                    model_projection.insert("schema_version".into(), Value::from(5));
                    let normalized = convert(Value::Object(model_projection))?;
                    let normalized =
                        document_with_partition(AcquisitionDocument::empty(), normalized)?;
                    document.legacy = normalized.legacy;
                }
                document.validate()?;
                document
            }
            _ => return Err(invalid_schema()),
        };
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

    pub(crate) fn require_unreceipted_use(&self, expected: &AcquisitionRecord) -> Result<()> {
        let transaction = self.transaction(true)?;
        let document = transaction.document()?;
        if !matches!(
            expected.phase,
            super::service::AcquisitionPhase::Using { .. }
        ) || document.acquisitions.get(&expected.id) != Some(expected)
            || document.consumer_receipts.contains_key(&expected.id)
        {
            return Err(invalid_receipt(
                "Withdrawal requires an exact unreceipted Using lease",
            ));
        }
        Ok(())
    }

    /// Commit cancellation only for the unchanged, unreceipted consumer lease.
    pub(crate) fn withdraw_unreceipted_use(&self, expected: &AcquisitionRecord) -> Result<()> {
        let transaction = self.transaction(false)?;
        let mut document = transaction.document()?;
        if !matches!(
            expected.phase,
            super::service::AcquisitionPhase::Using { .. }
        ) || document.acquisitions.get(&expected.id) != Some(expected)
            || document.consumer_receipts.contains_key(&expected.id)
        {
            return Err(invalid_receipt(
                "Withdrawal requires an exact unreceipted Using lease",
            ));
        }
        let record = document.acquisitions.get_mut(&expected.id).unwrap();
        record.phase = super::service::AcquisitionPhase::Withdrawn;
        record.files.clear();
        document.validate()?;
        require_durable(transaction.publish_document(&document))
    }

    pub fn acquisitions(&self) -> Result<BTreeMap<Uuid, AcquisitionRecord>> {
        let transaction = self.transaction(true)?;
        if transaction.legacy_read_only {
            return Ok(BTreeMap::new());
        }
        Ok(transaction.document()?.acquisitions)
    }

    /// Read a versioned generic consumer receipt. Legacy HF receipts remain
    /// interpreted only by the HF persistence facade.
    pub(crate) fn consumer_receipt(&self, id: Uuid) -> Result<Option<AcquisitionConsumerReceipt>> {
        let transaction = self.transaction(true)?;
        if transaction.legacy_read_only {
            return Ok(None);
        }
        let Some(value) = transaction.document()?.consumer_receipts.remove(&id) else {
            return Ok(None);
        };
        if value.get("receipt_kind").and_then(Value::as_str) != Some("pumas.consumer-completion") {
            return Ok(None);
        }
        serde_json::from_value(value)
            .map(Some)
            .map_err(|_| invalid_receipt("Consumer completion receipt is malformed"))
    }

    /// Atomically publish a consumer-owned completion receipt with the exact
    /// lease's transition from `Using` to `Adopted`.
    pub(crate) fn settle_consumer_use(
        &self,
        expected: &AcquisitionRecord,
        lease: Uuid,
        receipt: AcquisitionConsumerReceipt,
    ) -> Result<()> {
        receipt
            .validate_for_record(expected, lease)
            .map_err(|_| invalid_receipt("Consumer receipt does not match its acquisition"))?;
        if !matches!(&expected.phase,
            super::service::AcquisitionPhase::Using { lease: current_lease }
                | super::service::AcquisitionPhase::Adopted { lease: current_lease }
                if *current_lease == lease)
        {
            return Err(invalid_receipt(
                "Consumer settlement requires its exact use lease",
            ));
        }
        let transaction = self.transaction(false)?;
        let mut document = transaction.document()?;
        let Some(current) = document.acquisitions.get(&expected.id) else {
            return Err(invalid_receipt("Consumer acquisition disappeared"));
        };
        let receipt_value = serde_json::to_value(receipt)?;
        let mut adopted = expected.clone();
        adopted.phase = super::service::AcquisitionPhase::Adopted { lease };
        if current == &adopted
            && document.consumer_receipts.get(&expected.id) == Some(&receipt_value)
        {
            return Ok(());
        }
        if current != expected
            || !matches!(&expected.phase, super::service::AcquisitionPhase::Using { lease: current_lease } if *current_lease == lease)
        {
            return Err(invalid_receipt(
                "Consumer settlement does not match the exact active acquisition lease",
            ));
        }
        if document
            .consumer_receipts
            .get(&expected.id)
            .is_some_and(|stored| stored != &receipt_value)
        {
            return Err(invalid_receipt(
                "A conflicting consumer receipt is already published",
            ));
        }
        document.acquisitions.insert(expected.id, adopted);
        document
            .consumer_receipts
            .insert(expected.id, receipt_value);
        document.validate()?;
        require_durable(transaction.publish_document(&document))
    }

    pub(crate) fn issue_consumer_receipt(
        &self,
        expected: &AcquisitionRecord,
        lease: Uuid,
        receipt: &AcquisitionConsumerReceipt,
    ) -> Result<()> {
        receipt
            .validate_for_record(expected, lease)
            .map_err(|_| invalid_receipt("Consumer receipt does not match its acquisition"))?;
        let transaction = self.transaction(false)?;
        let mut document = transaction.document()?;
        let Some(current) = document.acquisitions.get(&expected.id) else {
            return Err(invalid_receipt("Consumer acquisition disappeared"));
        };
        let receipt_value = serde_json::to_value(receipt)?;
        if current == expected
            && matches!(&current.phase, super::service::AcquisitionPhase::Using { lease: current_lease } if *current_lease == lease)
            && document.consumer_receipts.get(&expected.id) == Some(&receipt_value)
        {
            return Ok(());
        }
        if current == expected
            && matches!(&current.phase, super::service::AcquisitionPhase::Using { lease: current_lease } if *current_lease == lease)
        {
            if document
                .consumer_receipts
                .get(&expected.id)
                .is_some_and(|stored| stored != &receipt_value)
            {
                return Err(invalid_receipt(
                    "A conflicting consumer receipt is already published",
                ));
            }
            document
                .consumer_receipts
                .insert(expected.id, receipt_value);
            document.validate()?;
            return require_durable(transaction.publish_document(&document));
        }
        Err(invalid_receipt(
            "Consumer receipt issuance does not match the exact active acquisition lease",
        ))
    }

    pub(crate) fn settle_consumer_receipt(
        &self,
        expected: &AcquisitionRecord,
        lease: Uuid,
        receipt: &AcquisitionConsumerReceipt,
    ) -> Result<()> {
        receipt
            .validate_for_record(expected, lease)
            .map_err(|_| invalid_receipt("Consumer receipt does not match its acquisition"))?;
        let transaction = self.transaction(false)?;
        let mut document = transaction.document()?;
        let Some(current) = document.acquisitions.get(&expected.id) else {
            return Err(invalid_receipt("Consumer acquisition disappeared"));
        };
        let receipt_value = serde_json::to_value(receipt)?;
        if current == expected
            && matches!(&current.phase, super::service::AcquisitionPhase::Using { lease: current_lease } if *current_lease == lease)
            && document.consumer_receipts.get(&expected.id) == Some(&receipt_value)
        {
            document
                .acquisitions
                .get_mut(&expected.id)
                .expect("checked acquisition exists")
                .phase = super::service::AcquisitionPhase::Adopted { lease };
            document.validate()?;
            return require_durable(transaction.publish_document(&document));
        }
        if current == expected
            && matches!(&current.phase, super::service::AcquisitionPhase::Adopted { lease: current_lease } if *current_lease == lease)
            && document.consumer_receipts.get(&expected.id) == Some(&receipt_value)
        {
            return Ok(());
        }
        Err(invalid_receipt(
            "Consumer settlement does not match the exact active acquisition lease",
        ))
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
        let Some(value) = self.target.read_downloads_json_value()? else {
            return Ok(AcquisitionDocument::empty());
        };
        if schema(&value) != Some(7) {
            return Err(invalid_schema());
        }
        let document: AcquisitionDocument = serde_json::from_value(value)?;
        document.validate()?;
        Ok(document)
    }

    /// Read both custody partitions from one document revision while this
    /// canonical transaction excludes every cooperative store writer.
    pub(crate) fn import_custody_partition(
        &self,
    ) -> Result<(Option<Value>, BTreeMap<Uuid, AcquisitionRecord>)> {
        let Some(value) = self.target.read_downloads_json_value()? else {
            return Ok((None, BTreeMap::new()));
        };
        if matches!(schema(&value), Some(4 | 5)) && self.legacy_read_only {
            return Ok((Some(value), BTreeMap::new()));
        }
        let document: AcquisitionDocument = serde_json::from_value(value)?;
        document.validate()?;
        let partition = if document.legacy.is_empty() {
            None
        } else {
            let mut value: serde_json::Map<String, Value> = document.legacy.into_iter().collect();
            value.insert("schema_version".into(), 5.into());
            Some(Value::Object(value))
        };
        Ok((partition, document.acquisitions))
    }

    pub(crate) fn consumer_completion_state(
        &self,
        id: Uuid,
    ) -> Result<(Option<AcquisitionRecord>, Option<Value>)> {
        if self.legacy_read_only {
            return Ok((None, None));
        }
        let document = self.document()?;
        Ok((
            document.acquisitions.get(&id).cloned(),
            document.consumer_receipts.get(&id).cloned(),
        ))
    }

    pub(crate) fn consumer_completion_partition(
        &self,
    ) -> Result<(BTreeMap<Uuid, AcquisitionRecord>, BTreeMap<Uuid, Value>)> {
        if self.legacy_read_only {
            return Ok((BTreeMap::new(), BTreeMap::new()));
        }
        let document = self.document()?;
        Ok((document.acquisitions, document.consumer_receipts))
    }

    pub(crate) fn model_partition(&self) -> Result<Option<Value>> {
        let Some(value) = self.target.read_downloads_json_value()? else {
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
            if let Some((expected, receipt)) = &self.receipt_issuance {
                let Some(record) = document.acquisitions.get(&expected.id) else {
                    return Err(invalid_receipt("Receipt acquisition disappeared"));
                };
                if record != expected
                    || !matches!(
                        record.phase,
                        super::service::AcquisitionPhase::Using { .. }
                            | super::service::AcquisitionPhase::Adopted { .. }
                    )
                {
                    return Err(invalid_receipt(
                        "Receipt issuance requires the exact persisted consumer lease",
                    ));
                }
                if document
                    .consumer_receipts
                    .get(&expected.id)
                    .is_some_and(|current| current != receipt)
                {
                    return Err(invalid_receipt(
                        "A conflicting consumer receipt is already published",
                    ));
                }
                document
                    .consumer_receipts
                    .insert(expected.id, receipt.clone());
            }
            if let Some((expected, receipt)) = &self.receipt_settlement {
                let Some(record) = document.acquisitions.get_mut(&expected.id) else {
                    return Err(invalid_receipt("Receipt acquisition disappeared"));
                };
                if record != expected {
                    return Err(invalid_receipt(
                        "Receipt settlement acquisition identity is stale",
                    ));
                }
                let lease = match record.phase {
                    super::service::AcquisitionPhase::Using { lease } => lease,
                    _ => return Err(invalid_receipt("Receipt use is already settled")),
                };
                if document.consumer_receipts.get(&expected.id) != Some(receipt) {
                    return Err(invalid_receipt(
                        "Receipt changed before exact consumer settlement",
                    ));
                }
                record.phase = super::service::AcquisitionPhase::Adopted { lease };
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

    pub(crate) fn stage_consumer_receipt_issuance(
        &mut self,
        expected: &AcquisitionRecord,
        receipt: Value,
    ) {
        self.receipt_issuance = Some((expected.clone(), receipt));
    }

    pub(crate) fn stage_consumer_receipt_settlement(
        &mut self,
        expected: &AcquisitionRecord,
        receipt: Value,
    ) {
        self.receipt_settlement = Some((expected.clone(), receipt));
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::acquisition::{
        AcquisitionDemand, AcquisitionPhase, AcquisitionRecord, ArtifactFile, ArtifactManifest,
        ArtifactRevisionEvidence, ArtifactSourceIdentity, FileVerificationRequirement,
        RevisionStrength, VerifiedFile, WorkspaceIdentity,
    };

    fn record(
        id: Uuid,
        operation: &str,
        workspace: &str,
        phase: AcquisitionPhase,
    ) -> AcquisitionRecord {
        let manifest = ArtifactManifest::new(
            ArtifactSourceIdentity::new(
                "fixture",
                "object",
                ArtifactRevisionEvidence::new(
                    "fixture.revision",
                    "v1",
                    RevisionStrength::Immutable,
                )
                .unwrap(),
            )
            .unwrap(),
            vec![ArtifactFile::new(
                "payload.bin",
                "payload",
                Some(1),
                None,
                FileVerificationRequirement::SizeAndImmutableRevision,
            )
            .unwrap()],
        )
        .unwrap();
        let files = if matches!(
            phase,
            AcquisitionPhase::FilesReady
                | AcquisitionPhase::Using { .. }
                | AcquisitionPhase::Adopted { .. }
        ) {
            vec![VerifiedFile {
                path: "payload.bin".into(),
                bytes: 1,
                sha256: "0".repeat(64),
            }]
        } else {
            Vec::new()
        };
        AcquisitionRecord {
            id,
            demand: AcquisitionDemand {
                consumer: "fixture.consumer".into(),
                operation: operation.into(),
            },
            manifest,
            workspace: WorkspaceIdentity {
                root_identity: "fixture.root".into(),
                relative_target: workspace.into(),
            },
            phase,
            files,
        }
    }

    fn document(records: Vec<AcquisitionRecord>) -> AcquisitionDocument {
        AcquisitionDocument {
            schema_version: SCHEMA_VERSION,
            acquisitions: records
                .into_iter()
                .map(|record| (record.id, record))
                .collect(),
            consumer_receipts: BTreeMap::new(),
            legacy: BTreeMap::new(),
        }
    }

    fn generic_receipt(record: &AcquisitionRecord, lease: Uuid) -> Value {
        serde_json::to_value(AcquisitionConsumerReceipt {
            receipt_kind: "pumas.consumer-completion".into(),
            receipt_version: 1,
            owner: record.demand.consumer.clone(),
            acquisition_id: record.id.to_string(),
            use_lease: lease.to_string(),
            demand: record.demand.clone(),
            manifest: record.manifest.clone(),
            workspace: record.workspace.clone(),
            verified_files: record.files.clone(),
            payload: serde_json::json!({"fixture": "complete"}),
        })
        .unwrap()
    }

    fn hf_receipt(record: &AcquisitionRecord, lease: Uuid) -> Value {
        serde_json::json!({
            "receipt_version": 1,
            "output_proof_version": 1,
            "acquisition_id": record.id.to_string(),
            "use_lease": lease.to_string(),
            "demand": &record.demand,
            "manifest": &record.manifest,
            "workspace": &record.workspace,
            "verified_files": &record.files,
            "download_id": "download-1",
            "queue_admission": {},
            "model_id": "model-1",
            "outputs": {},
        })
    }

    fn persisted_document_is_refused_without_rewrite(document: AcquisitionDocument) {
        let temp = tempfile::TempDir::new().unwrap();
        let path = temp.path().join("downloads.json");
        let bytes = serde_json::to_vec(&document).unwrap();
        std::fs::write(&path, &bytes).unwrap();

        assert!(matches!(
            AcquisitionStore::new(temp.path()).require_acquisition_schema(),
            Err(PumasError::Validation { ref field, .. }) if field == "acquisition.custody"
        ));
        assert_eq!(std::fs::read(path).unwrap(), bytes);
    }

    #[test]
    fn consumer_settlement_is_atomic_and_retry_accepts_exact_receipt() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("downloads.json");
        let lease = Uuid::new_v4();
        let expected = record(
            Uuid::new_v4(),
            "complete",
            "model/path",
            AcquisitionPhase::Using { lease },
        );
        std::fs::write(
            &path,
            serde_json::to_vec(&document(vec![expected.clone()])).unwrap(),
        )
        .unwrap();
        let store = AcquisitionStore::new(temp.path());
        let receipt: AcquisitionConsumerReceipt =
            serde_json::from_value(generic_receipt(&expected, lease)).unwrap();
        store
            .settle_consumer_use(&expected, lease, receipt.clone())
            .unwrap();
        assert_eq!(
            store.acquisitions().unwrap()[&expected.id].phase,
            AcquisitionPhase::Adopted { lease }
        );
        assert_eq!(
            store.consumer_receipt(expected.id).unwrap(),
            Some(receipt.clone())
        );
        let settled = std::fs::read(&path).unwrap();
        store
            .settle_consumer_use(&expected, lease, receipt.clone())
            .unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), settled);
        let mut conflict = receipt;
        conflict.payload = serde_json::json!({"fixture": "different"});
        assert!(store
            .settle_consumer_use(&expected, lease, conflict)
            .is_err());
        let mut changed = expected.clone();
        changed.demand.operation = "different".into();
        let changed_receipt = serde_json::from_value(generic_receipt(&changed, lease)).unwrap();
        assert!(store
            .settle_consumer_use(&changed, lease, changed_receipt)
            .is_err());
        assert_eq!(std::fs::read(&path).unwrap(), settled);
    }

    #[test]
    fn receipt_issuance_is_idempotent_and_settlement_can_finish_it() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("downloads.json");
        let lease = Uuid::new_v4();
        let expected = record(
            Uuid::new_v4(),
            "complete",
            "model/path",
            AcquisitionPhase::Using { lease },
        );
        std::fs::write(
            &path,
            serde_json::to_vec(&document(vec![expected.clone()])).unwrap(),
        )
        .unwrap();
        let store = AcquisitionStore::new(temp.path());
        let receipt: AcquisitionConsumerReceipt =
            serde_json::from_value(generic_receipt(&expected, lease)).unwrap();
        store
            .issue_consumer_receipt(&expected, lease, &receipt)
            .unwrap();
        let issued = std::fs::read(&path).unwrap();
        store
            .issue_consumer_receipt(&expected, lease, &receipt)
            .unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), issued);
        store
            .settle_consumer_use(&expected, lease, receipt)
            .unwrap();
        assert_eq!(
            store.acquisitions().unwrap()[&expected.id].phase,
            AcquisitionPhase::Adopted { lease }
        );
    }

    #[test]
    fn withdrawal_requires_exact_using_lease_and_absent_receipt() {
        let temp = tempfile::TempDir::new().unwrap();
        let path = temp.path().join("downloads.json");
        let lease = Uuid::new_v4();
        let expected = record(
            Uuid::new_v4(),
            "cancelled",
            "model/path",
            AcquisitionPhase::Using { lease },
        );
        let store = AcquisitionStore::new(temp.path());
        let original = document(vec![expected.clone()]);
        std::fs::write(&path, serde_json::to_vec(&original).unwrap()).unwrap();
        let mut wrong_lease = expected.clone();
        wrong_lease.phase = AcquisitionPhase::Using {
            lease: Uuid::new_v4(),
        };
        let before = std::fs::read(&path).unwrap();
        assert!(store.require_unreceipted_use(&wrong_lease).is_err());
        assert!(store.withdraw_unreceipted_use(&wrong_lease).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), before);
        let mut receipted = original.clone();
        receipted
            .consumer_receipts
            .insert(expected.id, generic_receipt(&expected, lease));
        std::fs::write(&path, serde_json::to_vec(&receipted).unwrap()).unwrap();
        let before = std::fs::read(&path).unwrap();
        assert!(store.require_unreceipted_use(&expected).is_err());
        assert!(store.withdraw_unreceipted_use(&expected).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), before);
        std::fs::write(&path, serde_json::to_vec(&original).unwrap()).unwrap();
        store.withdraw_unreceipted_use(&expected).unwrap();
        let result = store.acquisitions().unwrap();
        assert_eq!(result[&expected.id].phase, AcquisitionPhase::Withdrawn);
        assert!(result[&expected.id].files.is_empty());
        assert!(store.consumer_receipt(expected.id).unwrap().is_none());
    }

    #[test]
    fn schema_seven_rejects_duplicate_demand_even_when_one_row_is_using() {
        let demand = "same-operation";
        let document = document(vec![
            record(
                Uuid::from_u128(1),
                demand,
                "model/path",
                AcquisitionPhase::Transferring,
            ),
            record(
                Uuid::from_u128(2),
                demand,
                "model/path",
                AcquisitionPhase::Using {
                    lease: Uuid::from_u128(3),
                },
            ),
        ]);

        persisted_document_is_refused_without_rewrite(document);
    }

    #[test]
    fn schema_seven_rejects_distinct_active_demands_for_one_workspace() {
        let document = document(vec![
            record(
                Uuid::from_u128(1),
                "first-operation",
                "model/path",
                AcquisitionPhase::Transferring,
            ),
            record(
                Uuid::from_u128(2),
                "second-operation",
                "model/path",
                AcquisitionPhase::Using {
                    lease: Uuid::from_u128(3),
                },
            ),
        ]);

        persisted_document_is_refused_without_rewrite(document);
    }

    #[test]
    fn terminal_history_may_share_a_workspace_with_one_active_successor() {
        let document = document(vec![
            record(
                Uuid::from_u128(1),
                "adopted-operation",
                "model/path",
                AcquisitionPhase::Adopted {
                    lease: Uuid::from_u128(3),
                },
            ),
            record(
                Uuid::from_u128(2),
                "withdrawn-operation",
                "model/path",
                AcquisitionPhase::Withdrawn,
            ),
            record(
                Uuid::from_u128(4),
                "successor-operation",
                "model/path",
                AcquisitionPhase::Transferring,
            ),
        ]);

        document.validate().unwrap();
    }

    #[test]
    fn receipt_stays_with_using_or_adopted_record_in_one_owner_partition() {
        let id = Uuid::from_u128(1);
        let lease = Uuid::from_u128(2);
        let using = record(
            id,
            "receipt-operation",
            "model/path",
            AcquisitionPhase::Using { lease },
        );
        let mut doc = document(vec![using.clone()]);
        doc.consumer_receipts
            .insert(id, generic_receipt(&using, lease));
        doc.validate().unwrap();

        doc.consumer_receipts.insert(
            id,
            serde_json::json!({
                "receipt_kind": "pumas.consumer-completion",
                "receipt_version": 999
            }),
        );
        assert!(matches!(
            doc.validate(),
            Err(PumasError::Validation { ref field, .. })
                if field == "acquisition.consumer_receipts"
        ));
        doc.consumer_receipts
            .insert(id, generic_receipt(&using, lease));

        doc.acquisitions.get_mut(&id).unwrap().phase = AcquisitionPhase::Adopted { lease };
        doc.validate().unwrap();

        doc.acquisitions.clear();
        assert!(matches!(
            doc.validate(),
            Err(PumasError::Validation { ref field, .. })
                if field == "acquisition.consumer_receipts"
        ));
    }

    #[test]
    fn hf_receipts_require_supported_version_and_exact_acquisition_binding() {
        let id = Uuid::from_u128(11);
        let lease = Uuid::from_u128(12);
        let mut hf_record = record(
            id,
            "hf-operation",
            "model/path",
            AcquisitionPhase::Using { lease },
        );
        hf_record.demand.consumer = "hf.model".into();

        let mut doc = document(vec![hf_record.clone()]);
        doc.consumer_receipts
            .insert(id, hf_receipt(&hf_record, lease));
        doc.validate().unwrap();

        let mut unsupported_version = hf_receipt(&hf_record, lease);
        unsupported_version["receipt_version"] = 999.into();
        doc.consumer_receipts.insert(id, unsupported_version);
        assert!(matches!(
            doc.validate(),
            Err(PumasError::Validation { ref field, .. })
                if field == "acquisition.consumer_receipts"
        ));

        let mut mismatched_lease = hf_receipt(&hf_record, lease);
        mismatched_lease["use_lease"] = Uuid::from_u128(13).to_string().into();
        doc.consumer_receipts.insert(id, mismatched_lease);
        assert!(matches!(
            doc.validate(),
            Err(PumasError::Validation { ref field, .. })
                if field == "acquisition.consumer_receipts"
        ));

        let mut truncated = hf_receipt(&hf_record, lease);
        truncated.as_object_mut().unwrap().remove("outputs");
        doc.consumer_receipts.insert(id, truncated);
        assert!(matches!(
            doc.validate(),
            Err(PumasError::Validation { ref field, .. })
                if field == "acquisition.consumer_receipts"
        ));
    }

    #[test]
    fn receipt_partition_rejects_noncanonical_uuid_aliases() {
        #[derive(Deserialize)]
        struct ReceiptKeys {
            #[serde(with = "uuid_value_map")]
            values: BTreeMap<Uuid, Value>,
        }

        let id = Uuid::from_u128(1);
        let input = serde_json::json!({
            "values": {
                id.to_string(): {"receipt_version": 1},
                format!("urn:uuid:{id}"): {"receipt_version": 999}
            }
        });
        assert!(serde_json::from_value::<ReceiptKeys>(input).is_err());
        let canonical: ReceiptKeys = serde_json::from_value(serde_json::json!({
            "values": {id.to_string(): {"receipt_version": 1}}
        }))
        .unwrap();
        assert_eq!(canonical.values.len(), 1);
    }

    #[test]
    fn schema_six_migration_preserves_acquisition_and_model_partition_without_receipts() {
        use crate::model_library::download_store::{
            DownloadAdmissionDomain, DownloadAdmissionPosition, DownloadAdmissionRequest,
            DownloadPersistence, LifecycleCleanupDisposition, LifecycleQuarantineDomain,
            PersistedDestinationIdentity, PersistedDownload, PersistedQueueAdmission,
            QueuePredecessor,
        };

        let temp = tempfile::TempDir::new().unwrap();
        let path = temp.path().join("downloads.json");
        let id = Uuid::from_u128(1);
        let lease = Uuid::from_u128(2);
        let released_attempt = Uuid::from_u128(3).to_string();
        let active_attempt = Uuid::from_u128(4).to_string();
        let pending_attempt = Uuid::from_u128(5).to_string();
        let hidden_attempt = Uuid::from_u128(6).to_string();
        let destination = PersistedDestinationIdentity {
            library_root: "fixture.root".into(),
            relative_target: "model/path".into(),
        };
        let snapshot = |download_id: &str| -> PersistedDownload {
            serde_json::from_value(serde_json::json!({
                "download_id": download_id,
                "repo_id": "fixture/model",
                "filename": "payload.bin",
                "filenames": ["payload.bin"],
                "dest_dir": temp.path().join("model/path"),
                "total_bytes": 1,
                "status": "paused",
                "download_request": {
                    "repo_id": "fixture/model",
                    "family": "fixture",
                    "official_name": "Fixture",
                    "filename": "payload.bin"
                },
                "revision": "a".repeat(40),
                "created_at": "2026-01-01T00:00:00Z",
                "known_sha256": "0".repeat(64),
                "huggingface_evidence": null
            }))
            .unwrap()
        };
        let queue = |attempt: &str, domain, ordinal, predecessor| PersistedQueueAdmission {
            attempt_id: attempt.into(),
            domain,
            destination: destination.clone(),
            requested_payload_files: vec!["payload.bin".into()],
            execution_files: vec!["payload.bin".into()],
            position: DownloadAdmissionPosition {
                ordinal,
                predecessor,
            },
        };
        let predecessor = |download_id: &str, attempt: &str| {
            Some(QueuePredecessor {
                download_id: download_id.into(),
                admission_attempt_id: attempt.into(),
            })
        };
        let released = queue(&released_attempt, DownloadAdmissionDomain::Ambient, 1, None);
        let active = queue(
            &active_attempt,
            DownloadAdmissionDomain::Ambient,
            2,
            predecessor("released", &released_attempt),
        );
        let pending = queue(
            &pending_attempt,
            DownloadAdmissionDomain::Recovery,
            3,
            predecessor("active", &active_attempt),
        );
        let hidden_request = DownloadAdmissionRequest {
            snapshot: snapshot("hidden"),
            domain: DownloadAdmissionDomain::Ambient,
            destination: destination.clone(),
            requested_payload_files: vec!["payload.bin".into()],
            execution_files: vec!["payload.bin".into()],
        };
        let hidden_position = DownloadAdmissionPosition {
            ordinal: 4,
            predecessor: predecessor("pending", &pending_attempt),
        };
        let recovery_snapshot = snapshot("pending");
        let mut quarantine_snapshot = recovery_snapshot.clone();
        quarantine_snapshot.status = crate::models::DownloadStatus::Error;
        let mut record = record(
            id,
            &active_attempt,
            "model/path",
            AcquisitionPhase::Using { lease },
        );
        record.demand.consumer = "hf.model".into();
        let mut value = serde_json::to_value(document(vec![record.clone()])).unwrap();
        value.as_object_mut().unwrap().remove("consumer_receipts");
        value["schema_version"] = 6.into();
        value["downloads"] = serde_json::json!([snapshot("active")]);
        value["admission_attempts"] = serde_json::json!({
            hidden_attempt.clone(): {"request": hidden_request, "position": hidden_position}
        });
        value["queue_admissions"] = serde_json::json!({
            "active": active,
            "pending": pending
        });
        value["released_queue_admissions"] = serde_json::json!({"released": released});
        value["recovery_revocations"] = serde_json::json!({
            "pending": {
                "attempt_id": Uuid::from_u128(7).to_string(),
                "disposition": "durable",
                "origin": {
                    "kind": "admitted",
                    "admission_attempt_id": pending_attempt,
                    "snapshot": recovery_snapshot
                }
            }
        });
        value["lifecycle_quarantines"] = serde_json::json!({
            "pending": {
                "snapshot": quarantine_snapshot,
                "domain": "recovery",
                "disposition": "pending",
                "sticky_failure": true
            }
        });
        let before = serde_json::to_vec_pretty(&value).unwrap();
        std::fs::write(&path, &before).unwrap();

        let store = AcquisitionStore::new(temp.path());
        let models = DownloadPersistence::new(temp.path());
        assert!(matches!(
            store.require_acquisition_schema(),
            Err(PumasError::Validation { ref field, .. })
                if field == "acquisition.migration_required"
        ));
        assert!(matches!(
            models.load_lifecycle_inventory_strict(),
            Err(PumasError::Validation { ref field, .. })
                if field == "acquisition.migration_required"
        ));
        assert_eq!(std::fs::read(&path).unwrap(), before);
        drop(models);
        drop(store);

        DownloadPersistence::migrate_legacy_offline(temp.path()).unwrap();
        let mut expected = value;
        expected["schema_version"] = 7.into();
        expected["consumer_receipts"] = serde_json::json!({});
        let migrated: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(migrated, expected);

        // Cold owners have no process-local admission or cleanup confirmations.
        let reopened = AcquisitionStore::new(temp.path());
        let models = DownloadPersistence::new(temp.path());
        reopened.require_acquisition_schema().unwrap();
        assert_eq!(reopened.acquisitions().unwrap().get(&id), Some(&record));
        assert_eq!(record.phase, AcquisitionPhase::Using { lease });
        assert_eq!(record.files.len(), 1);
        assert!(reopened.consumer_receipt(id).unwrap().is_none());
        let (acquisitions, receipts) = reopened
            .transaction(true)
            .unwrap()
            .consumer_completion_partition()
            .unwrap();
        assert_eq!(acquisitions.get(&id), Some(&record));
        assert!(receipts.is_empty());
        assert!(models.read_hf_completion_receipt(id).unwrap().is_none());
        let inventory = models.load_lifecycle_inventory_strict().unwrap();
        assert!(inventory.downloads.is_empty());
        assert!(inventory.queue_admissions.is_empty());
        assert_eq!(inventory.hidden_admissions.len(), 3);
        let hidden = &inventory.hidden_admissions["hidden"];
        assert_eq!(
            serde_json::to_value(&hidden.request).unwrap(),
            serde_json::to_value(&hidden_request).unwrap()
        );
        assert_eq!(hidden.position, hidden_position);
        for (download_id, admission, snapshot) in [
            ("active", &active, snapshot("active")),
            ("pending", &pending, quarantine_snapshot.clone()),
        ] {
            let projected = &inventory.hidden_admissions[download_id];
            assert_eq!(
                serde_json::to_value(&projected.request.snapshot).unwrap(),
                serde_json::to_value(snapshot).unwrap()
            );
            assert_eq!(projected.request.domain, admission.domain);
            assert_eq!(projected.request.destination, admission.destination);
            assert_eq!(
                projected.request.requested_payload_files,
                admission.requested_payload_files
            );
            assert_eq!(projected.request.execution_files, admission.execution_files);
            assert_eq!(projected.position, admission.position);
        }
        assert!(!inventory.hidden_admissions.contains_key("released"));
        let quarantine = &inventory.quarantines["pending"];
        assert_eq!(inventory.quarantines.len(), 1);
        assert_eq!(quarantine.domain, LifecycleQuarantineDomain::Recovery);
        assert_eq!(quarantine.disposition, LifecycleCleanupDisposition::Pending);
        assert!(quarantine.sticky_failure);
        assert_eq!(
            serde_json::to_value(&quarantine.snapshot).unwrap(),
            serde_json::to_value(&quarantine_snapshot).unwrap()
        );
        assert!(models
            .validate_queue_execution(
                "pending",
                &pending_attempt,
                pending.domain,
                &destination,
                &pending.execution_files,
            )
            .is_err());

        // Only settlement is inside this byte comparison; inventory projection
        // and any future reconciliation must not obscure a refused write.
        let before_settlement = std::fs::read(&path).unwrap();
        assert!(matches!(
            models.settle_queue_admission("pending", &pending_attempt),
            Err(PumasError::Validation { field, message })
                if field == "downloads.lifecycle_quarantines"
                    && message == "Queue release requires verified failure cleanup"
        ));
        assert_eq!(std::fs::read(&path).unwrap(), before_settlement);
        let after: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(after, expected);
        assert_eq!(reopened.acquisitions().unwrap().get(&id), Some(&record));
        assert!(reopened.consumer_receipt(id).unwrap().is_none());
        assert!(models.read_hf_completion_receipt(id).unwrap().is_none());
        assert_eq!(
            models
                .load_lifecycle_inventory_strict()
                .unwrap()
                .quarantines["pending"]
                .disposition,
            LifecycleCleanupDisposition::Pending
        );
    }

    #[test]
    fn schema_six_rejects_normal_read_without_rewrite() {
        let temp = tempfile::TempDir::new().unwrap();
        let mut old = document(Vec::new());
        old.schema_version = 6;
        let bytes = serde_json::to_vec(&old).unwrap();
        let path = temp.path().join("downloads.json");
        std::fs::write(&path, &bytes).unwrap();

        assert!(matches!(
            AcquisitionStore::new(temp.path()).acquisitions(),
            Err(PumasError::Validation { ref field, .. })
                if field == "acquisition.migration_required"
        ));
        assert_eq!(std::fs::read(path).unwrap(), bytes);
    }

    fn assert_duplicate_member_refusal(result: Result<()>) {
        assert!(
            matches!(
                result,
                Err(PumasError::Validation { ref field, .. })
                    if field == "downloads.duplicate_member"
            ),
            "expected a downloads.duplicate_member refusal"
        );
    }

    #[test]
    fn duplicate_top_level_members_fail_closed_without_rewrite() {
        let temp = tempfile::TempDir::new().unwrap();
        let path = temp.path().join("downloads.json");
        let raw =
            r#"{"schema_version":7,"schema_version":7,"acquisitions":{},"consumer_receipts":{}}"#;
        std::fs::write(&path, raw.as_bytes()).unwrap();
        let before = std::fs::read(&path).unwrap();

        let store = AcquisitionStore::new(temp.path());
        assert_duplicate_member_refusal(store.require_acquisition_schema().map(|_| ()));
        assert_duplicate_member_refusal(store.acquisitions().map(|_| ()));
        assert_duplicate_member_refusal(
            store
                .transaction(true)
                .and_then(|transaction| transaction.model_partition())
                .map(|_| ()),
        );
        assert_eq!(std::fs::read(&path).unwrap(), before);
    }

    #[test]
    fn duplicate_consumer_receipt_keys_fail_closed_without_publication() {
        let temp = tempfile::TempDir::new().unwrap();
        let path = temp.path().join("downloads.json");
        let id = Uuid::from_u128(0x1111_1111_1111_1111_1111_1111_1111_1111);
        let raw = format!(
            "{{\"schema_version\":7,\"acquisitions\":{{}},\"consumer_receipts\":{{\"{id}\":{{\"a\":1}},\"{id}\":{{\"a\":1}}}}}}"
        );
        std::fs::write(&path, raw.as_bytes()).unwrap();
        let before = std::fs::read(&path).unwrap();

        let store = AcquisitionStore::new(temp.path());
        assert_duplicate_member_refusal(store.require_acquisition_schema().map(|_| ()));
        assert_duplicate_member_refusal(store.consumer_receipt(id).map(|_| ()));
        assert_duplicate_member_refusal(
            store
                .transaction(true)
                .and_then(|transaction| transaction.consumer_completion_partition())
                .map(|_| ()),
        );
        assert_eq!(std::fs::read(&path).unwrap(), before);
    }

    #[test]
    fn duplicate_nested_receipt_members_fail_closed_without_publication() {
        let temp = tempfile::TempDir::new().unwrap();
        let path = temp.path().join("downloads.json");
        let id = Uuid::from_u128(0x2222_2222_2222_2222_2222_2222_2222_2222);
        let raw = format!(
            "{{\"schema_version\":7,\"acquisitions\":{{}},\"consumer_receipts\":{{\"{id}\":{{\"demand\":1,\"demand\":2}}}}}}"
        );
        std::fs::write(&path, raw.as_bytes()).unwrap();
        let before = std::fs::read(&path).unwrap();

        let store = AcquisitionStore::new(temp.path());
        assert_duplicate_member_refusal(store.require_acquisition_schema().map(|_| ()));
        assert_duplicate_member_refusal(store.acquisitions().map(|_| ()));
        assert_duplicate_member_refusal(store.consumer_receipt(id).map(|_| ()));
        assert_eq!(std::fs::read(&path).unwrap(), before);
    }
}
