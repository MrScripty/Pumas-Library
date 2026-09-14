//! Private durable desired-model declarations and deletion claims.
//!
//! M3a establishes the SQLite authority and its atomic operations. The
//! crate-private mutation methods are staged for M3b orchestration, so their
//! narrowly scoped `dead_code` allowances should be removed as callers land.

use super::ModelIndex;
use crate::intent::{validate_requirement, AcquisitionPolicy, ModelRequirement, ModelSelector};
use crate::model_library::artifact_identity::DownloadRevision;
use crate::model_library::{DownloadRecoveryModelId, DownloadRequest, SelectedArtifactIdentity};
use crate::models::{PackageArtifactKind, PumasModelRef};
use crate::{PumasError, Result};
use rusqlite::{params, Connection, OptionalExtension, Transaction, TransactionBehavior};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use uuid::Uuid;

const INTENT_SCHEMA_VERSION: i64 = 1;
const DECLARATION_HASH_DOMAIN: &[u8] = b"pumas-intent-declaration-v1\0";
const MAX_CONSUMER_KEY_BYTES: usize = 128;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case", tag = "kind", deny_unknown_fields)]
pub(crate) enum BoundTarget {
    Local {
        model_ref: PumasModelRef,
    },
    Upstream {
        model_ref: PumasModelRef,
        repository_id: String,
        commit: String,
        filename: String,
        format: PackageArtifactKind,
    },
}

impl BoundTarget {
    pub(crate) fn model_ref(&self) -> &PumasModelRef {
        match self {
            Self::Local { model_ref } | Self::Upstream { model_ref, .. } => model_ref,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct IntentDeclarationRecord {
    pub(crate) declaration_id: String,
    pub(crate) consumer_key: String,
    pub(crate) original_requirement: ModelRequirement,
    pub(crate) generation: Uuid,
    pub(crate) bound_target: Option<BoundTarget>,
    pub(crate) created_at: String,
    pub(crate) updated_at: String,
    stored_original_json: String,
    stored_bound_target_json: Option<String>,
    stored_model_id: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum IntentDeclarationRelease {
    Released,
    AlreadyAbsent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum IntentDeletionClaimResult {
    Claimed,
    AlreadyClaimed,
    Retained,
    Conflict,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct IntentDeletionClaim {
    pub(crate) model_id: String,
    pub(crate) claim_token: Uuid,
    pub(crate) created_at: String,
}

impl ModelIndex {
    pub(super) fn inspect_intent_schema(conn: &Connection) -> Result<bool> {
        let objects = intent_sqlite_objects(conn)?;
        if objects.is_empty() {
            return Ok(false);
        }
        let expected = BTreeSet::from([
            (
                "idx_intent_declarations_consumer".to_string(),
                "index".to_string(),
            ),
            (
                "idx_intent_declarations_model".to_string(),
                "index".to_string(),
            ),
            ("intent_declarations".to_string(), "table".to_string()),
            ("intent_deletion_claims".to_string(), "table".to_string()),
            ("intent_schema_meta".to_string(), "table".to_string()),
        ]);
        if objects != expected {
            return Err(intent_schema_error("intent schema is partial"));
        }
        validate_columns(
            conn,
            "intent_schema_meta",
            &[
                ("singleton", "INTEGER", 0, 1),
                ("schema_version", "INTEGER", 1, 0),
            ],
        )?;
        validate_columns(
            conn,
            "intent_declarations",
            &[
                ("declaration_id", "TEXT", 0, 1),
                ("consumer_key", "TEXT", 1, 0),
                ("original_requirement_json", "TEXT", 1, 0),
                ("generation", "TEXT", 1, 0),
                ("model_id", "TEXT", 0, 0),
                ("bound_target_json", "TEXT", 0, 0),
                ("created_at", "TEXT", 1, 0),
                ("updated_at", "TEXT", 1, 0),
            ],
        )?;
        validate_columns(
            conn,
            "intent_deletion_claims",
            &[
                ("model_id", "TEXT", 0, 1),
                ("claim_token", "TEXT", 1, 0),
                ("created_at", "TEXT", 1, 0),
            ],
        )?;
        validate_index(conn, "idx_intent_declarations_model", &["model_id"])?;
        validate_index(conn, "idx_intent_declarations_consumer", &["consumer_key"])?;
        validate_table_contracts(conn)?;
        let versions = conn
            .prepare("SELECT singleton, schema_version FROM intent_schema_meta")?
            .query_map([], |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        if versions.as_slice() != [(1, INTENT_SCHEMA_VERSION)] {
            return Err(intent_schema_error(
                "unsupported or malformed intent schema marker",
            ));
        }
        validate_all_intent_rows(conn)?;
        Ok(true)
    }

    pub(super) fn ensure_intent_schema(conn: &mut Connection) -> Result<()> {
        Self::ensure_intent_schema_with_hook(conn, |_| Ok(()))
    }

    pub(super) fn ensure_intent_schema_with_hook(
        conn: &mut Connection,
        before_marker: impl FnOnce(&Transaction<'_>) -> Result<()>,
    ) -> Result<()> {
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        if Self::inspect_intent_schema(&tx)? {
            tx.commit()?;
            return Ok(());
        }
        tx.execute_batch(
            "CREATE TABLE intent_schema_meta (
                singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
                schema_version INTEGER NOT NULL CHECK (schema_version > 0)
             );
             CREATE TABLE intent_declarations (
                declaration_id TEXT PRIMARY KEY,
                consumer_key TEXT NOT NULL,
                original_requirement_json TEXT NOT NULL CHECK (json_valid(original_requirement_json)),
                generation TEXT NOT NULL,
                model_id TEXT,
                bound_target_json TEXT CHECK (bound_target_json IS NULL OR json_valid(bound_target_json)),
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
             );
             CREATE INDEX idx_intent_declarations_model
                ON intent_declarations(model_id);
             CREATE INDEX idx_intent_declarations_consumer
                ON intent_declarations(consumer_key);
             CREATE TABLE intent_deletion_claims (
                model_id TEXT PRIMARY KEY,
                claim_token TEXT NOT NULL UNIQUE,
                created_at TEXT NOT NULL
             );",
        )?;
        before_marker(&tx)?;
        tx.execute(
            "INSERT INTO intent_schema_meta(singleton, schema_version) VALUES (1, ?1)",
            [INTENT_SCHEMA_VERSION],
        )?;
        if !Self::inspect_intent_schema(&tx)? {
            return Err(intent_schema_error(
                "intent schema installation did not validate",
            ));
        }
        tx.commit()?;
        Ok(())
    }

    #[allow(dead_code)]
    pub(crate) fn commit_intent_declaration(
        &self,
        consumer_key: &str,
        original_requirement: &ModelRequirement,
    ) -> Result<IntentDeclarationRecord> {
        self.commit_intent_declaration_with_hook(consumer_key, original_requirement, |_| Ok(()))
    }

    fn commit_intent_declaration_with_hook(
        &self,
        consumer_key: &str,
        original_requirement: &ModelRequirement,
        before_commit: impl FnOnce(&Transaction<'_>) -> Result<()>,
    ) -> Result<IntentDeclarationRecord> {
        validate_consumer_key(consumer_key)?;
        let normalized = normalize_original_requirement(original_requirement)?;
        let original_json = serde_json::to_string(&normalized)?;
        let declaration_id = declaration_id(consumer_key, &original_json);
        let mut conn = self.intent_connection()?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let result = (|| {
            require_current_intent_schema(&tx)?;
            if let Some(existing) = read_declaration(&tx, &declaration_id)? {
                if existing.consumer_key != consumer_key
                    || existing.original_requirement != normalized
                {
                    return Err(intent_validation("declaration ID collision"));
                }
                return Ok(existing);
            }
            let bound_target = match &normalized.selector {
                ModelSelector::LocalModel { model_ref } => Some(BoundTarget::Local {
                    model_ref: model_ref.clone(),
                }),
                ModelSelector::UpstreamRepository { .. } => None,
            };
            if let Some(target) = &bound_target {
                validate_bound_target(&normalized, target)?;
                reject_deletion_claim(&tx, &target.model_ref().model_id)?;
            }
            let generation = Uuid::new_v4();
            let now = chrono::Utc::now().to_rfc3339();
            tx.execute(
                "INSERT INTO intent_declarations(
                    declaration_id, consumer_key, original_requirement_json, generation,
                    model_id, bound_target_json, created_at, updated_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7)",
                params![
                    declaration_id,
                    consumer_key,
                    original_json,
                    generation.to_string(),
                    bound_target
                        .as_ref()
                        .map(|target| &target.model_ref().model_id),
                    bound_target
                        .as_ref()
                        .map(serde_json::to_string)
                        .transpose()?,
                    now,
                ],
            )?;
            let inserted = read_declaration(&tx, &declaration_id)?.ok_or_else(|| {
                intent_schema_error("inserted intent declaration is not readable")
            })?;
            before_commit(&tx)?;
            Ok(inserted)
        })();
        finish_immediate(tx, result)
    }

    #[allow(dead_code)]
    pub(crate) fn bind_intent_declaration(
        &self,
        declaration_id: &str,
        consumer_key: &str,
        expected_generation: Uuid,
        bound_target: &BoundTarget,
    ) -> Result<IntentDeclarationRecord> {
        validate_declaration_id(declaration_id)?;
        validate_consumer_key(consumer_key)?;
        let mut conn = self.intent_connection()?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let result = (|| {
            require_current_intent_schema(&tx)?;
            let existing = read_declaration(&tx, declaration_id)?
                .ok_or_else(|| intent_conflict("intent declaration no longer exists"))?;
            if existing.consumer_key != consumer_key || existing.generation != expected_generation {
                return Err(intent_conflict("intent declaration generation is stale"));
            }
            validate_bound_target(&existing.original_requirement, bound_target)?;
            if let Some(current) = &existing.bound_target {
                if current == bound_target {
                    return Ok(existing);
                }
                return Err(intent_conflict("intent declaration is already bound"));
            }
            reject_deletion_claim(&tx, &bound_target.model_ref().model_id)?;
            tx.execute(
                "UPDATE intent_declarations
                 SET model_id = ?1, bound_target_json = ?2,
                     updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
                 WHERE declaration_id = ?3 AND consumer_key = ?4 AND generation = ?5",
                params![
                    bound_target.model_ref().model_id,
                    serde_json::to_string(bound_target)?,
                    declaration_id,
                    consumer_key,
                    expected_generation.to_string(),
                ],
            )?;
            read_declaration(&tx, declaration_id)?
                .ok_or_else(|| intent_schema_error("bound intent declaration is not readable"))
        })();
        finish_immediate(tx, result)
    }

    #[allow(dead_code)]
    pub(crate) fn release_intent_declaration(
        &self,
        declaration_id: &str,
        consumer_key: &str,
        expected_generation: Uuid,
    ) -> Result<IntentDeclarationRelease> {
        validate_declaration_id(declaration_id)?;
        validate_consumer_key(consumer_key)?;
        let mut conn = self.intent_connection()?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let result = (|| {
            require_current_intent_schema(&tx)?;
            let Some(existing) = read_declaration(&tx, declaration_id)? else {
                return Ok(IntentDeclarationRelease::AlreadyAbsent);
            };
            if existing.consumer_key != consumer_key || existing.generation != expected_generation {
                return Err(intent_conflict("intent declaration generation is stale"));
            }
            tx.execute(
                "DELETE FROM intent_declarations
                 WHERE declaration_id = ?1 AND consumer_key = ?2 AND generation = ?3",
                params![
                    declaration_id,
                    consumer_key,
                    expected_generation.to_string()
                ],
            )?;
            Ok(IntentDeclarationRelease::Released)
        })();
        finish_immediate(tx, result)
    }

    #[allow(dead_code)]
    pub(crate) fn get_intent_declaration(
        &self,
        declaration_id: &str,
    ) -> Result<Option<IntentDeclarationRecord>> {
        validate_declaration_id(declaration_id)?;
        let mut conn = self.intent_connection()?;
        let tx = conn.transaction()?;
        if !Self::inspect_intent_schema(&tx)? {
            tx.commit()?;
            return Ok(None);
        }
        let result = read_declaration(&tx, declaration_id)?;
        tx.commit()?;
        Ok(result)
    }

    #[allow(dead_code)]
    pub(crate) fn list_intent_declarations(&self) -> Result<Vec<IntentDeclarationRecord>> {
        self.list_intent_declarations_query(
            "SELECT declaration_id, consumer_key, original_requirement_json, generation,
                    model_id, bound_target_json, created_at, updated_at
             FROM intent_declarations ORDER BY declaration_id",
            [],
        )
    }

    #[allow(dead_code)]
    pub(crate) fn list_intent_declarations_for_model(
        &self,
        model_id: &str,
    ) -> Result<Vec<IntentDeclarationRecord>> {
        validate_model_id(model_id)?;
        self.list_intent_declarations_query(
            "SELECT declaration_id, consumer_key, original_requirement_json, generation,
                    model_id, bound_target_json, created_at, updated_at
             FROM intent_declarations WHERE model_id = ?1 ORDER BY declaration_id",
            [model_id],
        )
    }

    fn list_intent_declarations_query<P: rusqlite::Params>(
        &self,
        sql: &str,
        params: P,
    ) -> Result<Vec<IntentDeclarationRecord>> {
        let mut conn = self.intent_connection()?;
        let tx = conn.transaction()?;
        if !Self::inspect_intent_schema(&tx)? {
            tx.commit()?;
            return Ok(Vec::new());
        }
        let rows = {
            let mut statement = tx.prepare(sql)?;
            let rows = statement
                .query_map(params, decode_declaration_row)?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            rows
        };
        let result = rows
            .into_iter()
            .map(validate_decoded_declaration)
            .collect::<Result<Vec<_>>>()?;
        tx.commit()?;
        Ok(result)
    }

    #[allow(dead_code)]
    pub(crate) fn claim_intent_model_deletion(
        &self,
        model_id: &str,
        claim_token: Uuid,
    ) -> Result<IntentDeletionClaimResult> {
        validate_model_id(model_id)?;
        let mut conn = self.intent_connection()?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let result = (|| {
            require_current_intent_schema(&tx)?;
            if tx.query_row(
                "SELECT EXISTS(SELECT 1 FROM intent_declarations WHERE model_id = ?1)",
                [model_id],
                |row| row.get::<_, bool>(0),
            )? {
                return Ok(IntentDeletionClaimResult::Retained);
            }
            let existing = tx
                .query_row(
                    "SELECT claim_token FROM intent_deletion_claims WHERE model_id = ?1",
                    [model_id],
                    |row| row.get::<_, String>(0),
                )
                .optional()?;
            match existing {
                Some(token) if token == claim_token.to_string() => {
                    Ok(IntentDeletionClaimResult::AlreadyClaimed)
                }
                Some(_) => Ok(IntentDeletionClaimResult::Conflict),
                None => {
                    tx.execute(
                        "INSERT INTO intent_deletion_claims(model_id, claim_token, created_at)
                         VALUES (?1, ?2, ?3)",
                        params![
                            model_id,
                            claim_token.to_string(),
                            chrono::Utc::now().to_rfc3339()
                        ],
                    )?;
                    Ok(IntentDeletionClaimResult::Claimed)
                }
            }
        })();
        finish_immediate(tx, result)
    }

    #[allow(dead_code)]
    pub(crate) fn release_intent_model_deletion(
        &self,
        model_id: &str,
        claim_token: Uuid,
    ) -> Result<bool> {
        validate_model_id(model_id)?;
        let mut conn = self.intent_connection()?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let result = (|| {
            require_current_intent_schema(&tx)?;
            tx.execute(
                "DELETE FROM intent_deletion_claims WHERE model_id = ?1 AND claim_token = ?2",
                params![model_id, claim_token.to_string()],
            )
            .map(|changed| changed == 1)
            .map_err(Into::into)
        })();
        finish_immediate(tx, result)
    }

    #[allow(dead_code)]
    pub(crate) fn list_intent_model_deletion_claims(&self) -> Result<Vec<IntentDeletionClaim>> {
        let mut conn = self.intent_connection()?;
        let tx = conn.transaction()?;
        if !Self::inspect_intent_schema(&tx)? {
            tx.commit()?;
            return Ok(Vec::new());
        }
        let raw = {
            let mut statement = tx.prepare(
                "SELECT model_id, claim_token, created_at
                 FROM intent_deletion_claims ORDER BY model_id",
            )?;
            let rows = statement
                .query_map([], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                    ))
                })?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            rows
        };
        let result = raw
            .into_iter()
            .map(|(model_id, claim_token, created_at)| {
                validate_model_id(&model_id)?;
                validate_timestamp(&created_at)?;
                Ok(IntentDeletionClaim {
                    model_id,
                    claim_token: Uuid::parse_str(&claim_token)
                        .map_err(|_| intent_schema_error("invalid deletion claim token"))?,
                    created_at,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        tx.commit()?;
        Ok(result)
    }

    fn intent_connection(&self) -> Result<std::sync::MutexGuard<'_, Connection>> {
        self.conn.lock().map_err(|_| PumasError::Database {
            message: "Failed to acquire connection lock".to_string(),
            source: None,
        })
    }
}

fn finish_immediate<T>(tx: Transaction<'_>, result: Result<T>) -> Result<T> {
    match result {
        Ok(value) => {
            tx.commit()?;
            Ok(value)
        }
        Err(error) => {
            let _ = tx.rollback();
            Err(error)
        }
    }
}

fn normalize_original_requirement(requirement: &ModelRequirement) -> Result<ModelRequirement> {
    if !validate_requirement(requirement).is_empty() {
        return Err(intent_validation("invalid intent requirement"));
    }
    if requirement.artifact.format == Some(PackageArtifactKind::Unknown) {
        return Err(intent_validation("unsupported intent artifact format"));
    }
    let mut normalized = requirement.clone();
    if let Some(value) = &mut normalized.artifact.quantization {
        *value = crate::intent::normalized_quantization(value.trim());
    }
    match &mut normalized.selector {
        ModelSelector::LocalModel { model_ref } => {
            if let Some(revision) = &mut model_ref.revision {
                if revision.len() == 40 && revision.bytes().all(|byte| byte.is_ascii_hexdigit()) {
                    *revision = revision.to_ascii_lowercase();
                }
            }
            if normalized.acquisition_policy != AcquisitionPolicy::LocalOnly {
                return Err(intent_validation(
                    "local intent declarations must be local-only",
                ));
            }
        }
        ModelSelector::UpstreamRepository { revision, .. } => {
            if normalized.acquisition_policy != AcquisitionPolicy::AllowUpstream {
                return Err(intent_validation(
                    "upstream intent declarations must allow upstream acquisition",
                ));
            }
            if let Some(revision) = revision {
                if revision.trim() != revision {
                    return Err(intent_validation("invalid upstream revision selector"));
                }
                if revision.len() == 40 && revision.bytes().all(|byte| byte.is_ascii_hexdigit()) {
                    *revision = revision.to_ascii_lowercase();
                }
            }
        }
    }
    Ok(normalized)
}

fn validate_bound_target(original: &ModelRequirement, target: &BoundTarget) -> Result<()> {
    match (target, &original.selector) {
        (
            BoundTarget::Local { model_ref },
            ModelSelector::LocalModel {
                model_ref: original_ref,
            },
        ) => {
            if model_ref != original_ref
                || original.acquisition_policy != AcquisitionPolicy::LocalOnly
            {
                return Err(intent_validation(
                    "local bound target contradicts original requirement",
                ));
            }
        }
        (
            BoundTarget::Upstream {
                model_ref,
                repository_id,
                commit,
                filename,
                format,
            },
            ModelSelector::UpstreamRepository {
                repository_id: original_repo,
                revision: original_revision,
            },
        ) => {
            if repository_id != original_repo
                || original.acquisition_policy != AcquisitionPolicy::AllowUpstream
            {
                return Err(intent_validation(
                    "upstream bound target contradicts original requirement",
                ));
            }
            let revision = DownloadRevision::from_commit(commit)?;
            if revision.as_str() != commit {
                return Err(intent_validation(
                    "bound commit must be canonical lowercase",
                ));
            }
            if original_revision.as_deref().is_some_and(|selector| {
                DownloadRevision::from_commit(selector)
                    .is_ok_and(|required| required.as_str() != commit)
            }) {
                return Err(intent_validation(
                    "bound commit contradicts original immutable revision",
                ));
            }
            if !portable_relative_filename(filename) {
                return Err(intent_validation(
                    "bound filename must be one portable relative file",
                ));
            }
            if !matches!(
                format,
                PackageArtifactKind::Gguf
                    | PackageArtifactKind::Safetensors
                    | PackageArtifactKind::Onnx
            ) || !format_matches_filename(*format, filename)
                || original
                    .artifact
                    .format
                    .is_some_and(|required| required != *format)
            {
                return Err(intent_validation(
                    "bound artifact format is unsupported or contradictory",
                ));
            }
            let (family, official_name) = repository_id
                .split_once('/')
                .ok_or_else(|| intent_validation("invalid bound repository"))?;
            let request = DownloadRequest {
                repo_id: repository_id.clone(),
                family: family.to_string(),
                official_name: official_name.to_string(),
                model_type: (*format == PackageArtifactKind::Gguf).then(|| "llm".to_string()),
                quant: None,
                filename: Some(filename.clone()),
                filenames: None,
                pipeline_tag: None,
                bundle_format: None,
                pipeline_class: None,
                release_date: None,
                download_url: None,
                model_card_json: None,
                license_status: None,
            };
            let identity = SelectedArtifactIdentity::from_download_request_at_revision(
                &request,
                Some(vec![filename.clone()]),
                &revision,
            );
            if model_ref.revision.as_deref() != Some(commit)
                || model_ref.selected_artifact_id.as_deref() != Some(identity.artifact_id.as_str())
                || original
                    .artifact
                    .selected_artifact_id
                    .as_deref()
                    .is_some_and(|required| required != identity.artifact_id)
            {
                return Err(intent_validation(
                    "bound model reference contradicts artifact identity",
                ));
            }
            if model_ref
                .selected_artifact_path
                .as_deref()
                .is_some_and(|path| {
                    !portable_relative_filename(path)
                        || !path.starts_with(&format!("{}/", model_ref.model_id))
                        || !path.ends_with(&format!("/{filename}"))
                })
            {
                return Err(intent_validation(
                    "bound selected artifact path is inconsistent",
                ));
            }
            let reference_requirement = ModelRequirement {
                selector: ModelSelector::LocalModel {
                    model_ref: model_ref.clone(),
                },
                artifact: original.artifact.clone(),
                acquisition_policy: AcquisitionPolicy::LocalOnly,
            };
            if !validate_requirement(&reference_requirement).is_empty() {
                return Err(intent_validation("bound model reference is invalid"));
            }
        }
        _ => {
            return Err(intent_validation(
                "bound target kind contradicts original selector",
            ))
        }
    }
    Ok(())
}

fn read_declaration(
    conn: &Connection,
    declaration_id: &str,
) -> Result<Option<IntentDeclarationRecord>> {
    let decoded = conn
        .query_row(
            "SELECT declaration_id, consumer_key, original_requirement_json, generation,
                    model_id, bound_target_json, created_at, updated_at
             FROM intent_declarations WHERE declaration_id = ?1",
            [declaration_id],
            decode_declaration_row,
        )
        .optional()?;
    decoded.map(validate_decoded_declaration).transpose()
}

fn decode_declaration_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<IntentDeclarationRecord> {
    let original_json: String = row.get(2)?;
    let bound_json: Option<String> = row.get(5)?;
    let original_requirement = serde_json::from_str(&original_json).map_err(sql_decode_error)?;
    let bound_target = bound_json
        .as_deref()
        .map(serde_json::from_str)
        .transpose()
        .map_err(sql_decode_error)?;
    let generation: String = row.get(3)?;
    let parsed_generation = Uuid::parse_str(&generation).map_err(sql_decode_error)?;
    if parsed_generation.to_string() != generation {
        return Err(sql_decode_error(intent_schema_error(
            "noncanonical declaration generation",
        )));
    }
    Ok(IntentDeclarationRecord {
        declaration_id: row.get(0)?,
        consumer_key: row.get(1)?,
        original_requirement,
        generation: parsed_generation,
        bound_target,
        created_at: row.get(6)?,
        updated_at: row.get(7)?,
        stored_original_json: original_json,
        stored_bound_target_json: bound_json,
        stored_model_id: row.get(4)?,
    })
}

fn validate_decoded_declaration(
    record: IntentDeclarationRecord,
) -> Result<IntentDeclarationRecord> {
    validate_consumer_key(&record.consumer_key)?;
    validate_timestamp(&record.created_at)?;
    validate_timestamp(&record.updated_at)?;
    let normalized = normalize_original_requirement(&record.original_requirement)?;
    if normalized != record.original_requirement {
        return Err(intent_schema_error(
            "stored original requirement is not canonical",
        ));
    }
    let original_json = serde_json::to_string(&normalized)?;
    if original_json != record.stored_original_json {
        return Err(intent_schema_error(
            "stored original requirement is not canonical",
        ));
    }
    if declaration_id(&record.consumer_key, &original_json) != record.declaration_id {
        return Err(intent_schema_error(
            "stored declaration ID is not canonical",
        ));
    }
    if let Some(target) = &record.bound_target {
        validate_bound_target(&record.original_requirement, target)?;
    } else if matches!(
        record.original_requirement.selector,
        ModelSelector::LocalModel { .. }
    ) {
        return Err(intent_schema_error(
            "local declaration has lost its retained target",
        ));
    }
    let canonical_target = record
        .bound_target
        .as_ref()
        .map(serde_json::to_string)
        .transpose()?;
    if canonical_target != record.stored_bound_target_json
        || record.stored_model_id.as_deref()
            != record
                .bound_target
                .as_ref()
                .map(|target| target.model_ref().model_id.as_str())
    {
        return Err(intent_schema_error(
            "stored bound target projection is inconsistent",
        ));
    }
    Ok(record)
}

fn validate_all_intent_rows(conn: &Connection) -> Result<()> {
    let mut declaration_statement = conn.prepare(
        "SELECT declaration_id, consumer_key, original_requirement_json, generation,
                model_id, bound_target_json, created_at, updated_at
         FROM intent_declarations ORDER BY declaration_id",
    )?;
    let raw = declaration_statement
        .query_map([], |row| {
            Ok((
                decode_declaration_row(row)?,
                row.get::<_, Option<String>>(4)?,
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    for (record, stored_model_id) in raw {
        let record = validate_decoded_declaration(record)?;
        let expected = record
            .bound_target
            .as_ref()
            .map(|target| target.model_ref().model_id.as_str());
        if stored_model_id.as_deref() != expected {
            return Err(intent_schema_error(
                "stored declaration target projection is inconsistent",
            ));
        }
    }
    let mut claim_statement = conn.prepare(
        "SELECT model_id, claim_token, created_at FROM intent_deletion_claims ORDER BY model_id",
    )?;
    let claims = claim_statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    for (model_id, token, created_at) in claims {
        validate_model_id(&model_id)?;
        let parsed = Uuid::parse_str(&token)
            .map_err(|_| intent_schema_error("invalid deletion claim token"))?;
        if parsed.to_string() != token {
            return Err(intent_schema_error("noncanonical deletion claim token"));
        }
        validate_timestamp(&created_at)?;
        if conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM intent_declarations WHERE model_id = ?1)",
            [&model_id],
            |row| row.get::<_, bool>(0),
        )? {
            return Err(intent_schema_error(
                "deletion claim overlaps a retained intent target",
            ));
        }
    }
    Ok(())
}

fn require_current_intent_schema(conn: &Connection) -> Result<()> {
    if ModelIndex::inspect_intent_schema(conn)? {
        Ok(())
    } else {
        Err(intent_schema_error("intent schema is not installed"))
    }
}

fn validate_declaration_id(value: &str) -> Result<()> {
    if value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
    {
        Ok(())
    } else {
        Err(intent_validation("invalid intent declaration ID"))
    }
}

fn reject_deletion_claim(conn: &Connection, model_id: &str) -> Result<()> {
    if conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM intent_deletion_claims WHERE model_id = ?1)",
        [model_id],
        |row| row.get::<_, bool>(0),
    )? {
        Err(intent_conflict("model has an active deletion claim"))
    } else {
        Ok(())
    }
}

fn declaration_id(consumer_key: &str, canonical_requirement: &str) -> String {
    let mut digest = Sha256::new();
    digest.update(DECLARATION_HASH_DOMAIN);
    digest.update((consumer_key.len() as u64).to_be_bytes());
    digest.update(consumer_key.as_bytes());
    digest.update((canonical_requirement.len() as u64).to_be_bytes());
    digest.update(canonical_requirement.as_bytes());
    hex::encode(digest.finalize())
}

fn validate_consumer_key(value: &str) -> Result<()> {
    let valid = !value.is_empty()
        && value.len() <= MAX_CONSUMER_KEY_BYTES
        && value.is_ascii()
        && value.as_bytes()[0].is_ascii_alphanumeric()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b':' | b'-'));
    if valid {
        Ok(())
    } else {
        Err(intent_validation("invalid intent consumer key"))
    }
}

fn validate_model_id(value: &str) -> Result<()> {
    let reference = PumasModelRef {
        model_id: value.to_string(),
        ..Default::default()
    };
    let requirement = ModelRequirement {
        selector: ModelSelector::LocalModel {
            model_ref: reference,
        },
        artifact: Default::default(),
        acquisition_policy: AcquisitionPolicy::LocalOnly,
    };
    if validate_requirement(&requirement).is_empty() {
        Ok(())
    } else {
        Err(intent_validation("invalid managed model ID"))
    }
}

fn portable_relative_filename(value: &str) -> bool {
    DownloadRecoveryModelId::parse(value).is_some()
}

fn format_matches_filename(format: PackageArtifactKind, filename: &str) -> bool {
    let filename = filename.to_ascii_lowercase();
    match format {
        PackageArtifactKind::Gguf => filename.ends_with(".gguf"),
        PackageArtifactKind::Safetensors => filename.ends_with(".safetensors"),
        PackageArtifactKind::Onnx => filename.ends_with(".onnx"),
        _ => false,
    }
}

fn validate_timestamp(value: &str) -> Result<()> {
    chrono::DateTime::parse_from_rfc3339(value)
        .map(|_| ())
        .map_err(|_| intent_schema_error("invalid intent timestamp"))
}

fn intent_sqlite_objects(conn: &Connection) -> Result<BTreeSet<(String, String)>> {
    let mut statement = conn.prepare(
        "SELECT name, type FROM sqlite_master
         WHERE name LIKE 'intent_%' OR name LIKE 'idx_intent_%'
         ORDER BY name",
    )?;
    let objects = statement
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?
        .collect::<rusqlite::Result<_>>()?;
    Ok(objects)
}

fn validate_table_contracts(conn: &Connection) -> Result<()> {
    for table in [
        "intent_schema_meta",
        "intent_declarations",
        "intent_deletion_claims",
    ] {
        let mut foreign_keys = conn.prepare(&format!("PRAGMA foreign_key_list({table})"))?;
        if foreign_keys.query([])?.next()?.is_some() {
            return Err(intent_schema_error(
                "intent tables must not have foreign keys",
            ));
        }
    }
    let schema_sql = table_sql(conn, "intent_schema_meta")?;
    let declaration_sql = table_sql(conn, "intent_declarations")?;
    let claims_sql = table_sql(conn, "intent_deletion_claims")?;
    if schema_sql
        != normalized_sql(
            "CREATE TABLE intent_schema_meta (
                singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
                schema_version INTEGER NOT NULL CHECK (schema_version > 0)
             )",
        )
        || declaration_sql
            != normalized_sql(
                "CREATE TABLE intent_declarations (
                    declaration_id TEXT PRIMARY KEY,
                    consumer_key TEXT NOT NULL,
                    original_requirement_json TEXT NOT NULL CHECK (json_valid(original_requirement_json)),
                    generation TEXT NOT NULL,
                    model_id TEXT,
                    bound_target_json TEXT CHECK (bound_target_json IS NULL OR json_valid(bound_target_json)),
                    created_at TEXT NOT NULL,
                    updated_at TEXT NOT NULL
                 )",
            )
        || claims_sql
            != normalized_sql(
                "CREATE TABLE intent_deletion_claims (
                    model_id TEXT PRIMARY KEY,
                    claim_token TEXT NOT NULL UNIQUE,
                    created_at TEXT NOT NULL
                 )",
            )
    {
        return Err(intent_schema_error("intent table constraints are incompatible"));
    }
    let mut indexes = conn.prepare("PRAGMA index_list(intent_deletion_claims)")?;
    let unique_indexes = indexes
        .query_map([], |row| {
            Ok((row.get::<_, String>(1)?, row.get::<_, i64>(2)?))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let mut claim_token_unique = false;
    for (index, unique) in unique_indexes {
        if unique == 1 {
            let mut columns = conn.prepare(&format!("PRAGMA index_info({index})"))?;
            let columns = columns
                .query_map([], |row| row.get::<_, String>(2))?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            claim_token_unique |= columns == ["claim_token"];
        }
    }
    if !claim_token_unique {
        return Err(intent_schema_error("deletion claim token must be unique"));
    }
    Ok(())
}

fn table_sql(conn: &Connection, table: &str) -> Result<String> {
    let sql: String = conn.query_row(
        "SELECT sql FROM sqlite_master WHERE type = 'table' AND name = ?1",
        [table],
        |row| row.get(0),
    )?;
    Ok(normalized_sql(&sql))
}

fn normalized_sql(sql: &str) -> String {
    sql.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase()
}

fn validate_columns(
    conn: &Connection,
    table: &str,
    expected: &[(&str, &str, i64, i64)],
) -> Result<()> {
    let mut statement = conn.prepare(&format!("PRAGMA table_info({table})"))?;
    let actual = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, i64>(3)?,
                row.get::<_, i64>(5)?,
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let expected = expected
        .iter()
        .map(|(name, ty, not_null, primary)| {
            ((*name).to_string(), (*ty).to_string(), *not_null, *primary)
        })
        .collect::<Vec<_>>();
    if actual == expected {
        Ok(())
    } else {
        Err(intent_schema_error(&format!(
            "intent table {table} has incompatible columns"
        )))
    }
}

fn validate_index(conn: &Connection, index: &str, expected_columns: &[&str]) -> Result<()> {
    let mut statement = conn.prepare(&format!("PRAGMA index_info({index})"))?;
    let actual = statement
        .query_map([], |row| row.get::<_, String>(2))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let mut indexes = conn.prepare("PRAGMA index_list(intent_declarations)")?;
    let details = indexes
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, i64>(4)?,
            ))
        })?
        .find_map(|row| match row {
            Ok((name, unique, origin, partial)) if name == index => {
                Some(Ok((unique, origin, partial)))
            }
            Ok(_) => None,
            Err(error) => Some(Err(error)),
        })
        .transpose()?;
    let expected_sql = normalized_sql(&format!(
        "CREATE INDEX {index} ON intent_declarations({})",
        expected_columns.join(", ")
    ));
    let stored_sql: String = conn.query_row(
        "SELECT sql FROM sqlite_master WHERE type = 'index' AND name = ?1",
        [index],
        |row| row.get(0),
    )?;
    if actual == expected_columns
        && details == Some((0, "c".to_string(), 0))
        && normalized_sql(&stored_sql) == expected_sql
    {
        Ok(())
    } else {
        Err(intent_schema_error(&format!(
            "intent index {index} is incompatible"
        )))
    }
}

fn sql_decode_error(error: impl std::error::Error + Send + Sync + 'static) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(error))
}

fn intent_validation(message: &str) -> PumasError {
    PumasError::Validation {
        field: "intent.declaration".to_string(),
        message: message.to_string(),
    }
}

fn intent_conflict(message: &str) -> PumasError {
    PumasError::Other(format!("Intent declaration conflict: {message}"))
}

fn intent_schema_error(message: &str) -> PumasError {
    PumasError::Database {
        message: message.to_string(),
        source: None,
    }
}

#[cfg(test)]
mod tests;
