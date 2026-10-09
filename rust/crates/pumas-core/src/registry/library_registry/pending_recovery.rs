//! Bounded pending-reservation receipts in the existing registry, not a second
//! ownership system. Only discovery's opaque unstarted authority calls issuance.
use super::*;
use crate::platform::store_lifetime::StoreLifetime;
use sha2::{Digest, Sha256};

const POLICY: &str = "pumas.unstarted-registry-metadata@1";
const REGISTRY_ID: &str = "pending_reservation_registry_id@1";
const MAX_METADATA: usize = 64 * 1024;
const MAX_QUALIFICATION: usize = 8 * 1024;
const MAX_IDENTITY: usize = 128;

/// Serializable observation of an acknowledged durable pending checkpoint.
/// Possession does not authorize replacement: the complete on-disk receipt,
/// selected physical root and held native lease must all validate again.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PendingReservationCheckpoint {
    pub contract_version: u32,
    pub generation: String,
    pub library_id: String,
    pub metadata_sha256: String,
    pub checkpoint_id: String,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Qualification {
    policy: String,
    checkpoint: PendingReservationCheckpoint,
    registry_id: String,
    root: PathBuf,
    pid: u32,
    claim_token: String,
    device: u64,
    inode: u64,
    boot_id: String,
}

fn invalid(message: &str) -> PumasError {
    PumasError::InvalidParams {
        message: format!("pending reservation recovery: {message}"),
    }
}

fn path_text(path: &Path) -> Result<&str> {
    path.to_str()
        .ok_or_else(|| invalid("non-UTF-8 root is not qualified"))
}

fn boot_id() -> Result<String> {
    if !cfg!(target_os = "linux") {
        return Err(invalid("Linux same-boot qualification required"));
    }
    let text = std::fs::read_to_string("/proc/sys/kernel/random/boot_id")
        .map_err(|_| invalid("kernel boot identity unavailable"))?;
    let id = text.trim();
    uuid::Uuid::parse_str(id).map_err(|_| invalid("invalid kernel boot identity"))?;
    Ok(id.into())
}

fn digest(text: &str) -> String {
    hex::encode(Sha256::digest(text.as_bytes()))
}

fn validate_metadata(text: &str) -> Result<()> {
    if text.len() > MAX_METADATA {
        return Err(invalid("metadata exceeds 64 KiB"));
    }
    let json: serde_json::Value = serde_json::from_str(text)
        .map_err(|_| invalid("metadata must be a complete JSON object"))?;
    if !json.is_object() {
        return Err(invalid("metadata must be a JSON object"));
    }
    Ok(())
}

/// Per-connection durability, checked before BEGIN and held through COMMIT.
/// Leave FULL in place: resetting it is unnecessary and must not race a commit.
pub(super) fn require_full_durability(conn: &Connection) -> Result<()> {
    conn.pragma_update(None, "synchronous", "FULL")?;
    let mode: i64 = conn.pragma_query_value(None, "synchronous", |row| row.get(0))?;
    let journal: String = conn.pragma_query_value(None, "journal_mode", |row| row.get(0))?;
    if mode != 2 || journal != "wal" {
        return Err(invalid("verified SQLite WAL/FULL durability required"));
    }
    Ok(())
}

fn exact_pending(
    conn: &Connection,
    claim: &PrimaryInstanceClaim,
    generation: Option<&str>,
) -> Result<String> {
    conn.query_row(
        "SELECT CASE WHEN length(CAST(started_at AS BLOB)) BETWEEN 1 AND ?5
         THEN started_at ELSE NULL END FROM instances WHERE library_path=?1 AND pid=?2
         AND claim_token=?3 AND status='claiming' AND port=0
         AND transport_kind='loopback_tcp' AND endpoint IS NULL AND connection_token IS NULL
         AND (?4 IS NULL OR started_at=?4)
         AND NOT EXISTS(SELECT 1 FROM http_services WHERE library_path=?1)",
        params![
            path_text(&claim.library_path)?,
            claim.pid,
            claim.claim_token,
            generation,
            MAX_IDENTITY
        ],
        |row| row.get::<_, Option<String>>(0),
    )
    .optional()?
    .flatten()
    .ok_or_else(|| invalid("exact unstarted claim or custody scope changed"))
}

fn registry_identity(conn: &Connection) -> Result<String> {
    let value: Option<Option<String>> = conn
        .query_row(
            "SELECT CASE WHEN length(CAST(value AS BLOB)) BETWEEN 1 AND ?2
         THEN value ELSE NULL END FROM registry_config WHERE key=?1",
            params![REGISTRY_ID, MAX_IDENTITY],
            |row| row.get(0),
        )
        .optional()?;
    let value = value
        .flatten()
        .ok_or_else(|| invalid("missing or oversized registry identity"))?;
    uuid::Uuid::parse_str(&value).map_err(|_| invalid("invalid registry identity"))?;
    Ok(value)
}

fn load(conn: &Connection, root: &Path) -> Result<Option<Qualification>> {
    let row: Option<Option<String>> = conn
        .query_row(
            "SELECT CASE WHEN length(CAST(qualification_json AS BLOB))<=?2
         THEN qualification_json ELSE NULL END FROM pending_reservation_checkpoints
         WHERE library_path=?1",
            params![path_text(root)?, MAX_QUALIFICATION],
            |row| row.get(0),
        )
        .optional()?;
    match row {
        None => Ok(None),
        Some(None) => Err(invalid("oversized qualification")),
        Some(Some(text)) => serde_json::from_str(&text)
            .map(Some)
            .map_err(|_| invalid("corrupt or incomplete qualification")),
    }
}

fn validate(
    conn: &Connection,
    claim: &PrimaryInstanceClaim,
    lifetime: &StoreLifetime,
    q: &Qualification,
) -> Result<()> {
    lifetime.require_root(&claim.library_path)?;
    let (device, inode) = lifetime.physical_identity()?;
    if q.policy != POLICY
        || q.checkpoint.contract_version != 1
        || q.root != claim.library_path
        || q.pid != claim.pid
        || q.claim_token != claim.claim_token
        || q.device != device
        || q.inode != inode
        || q.boot_id != boot_id()?
        || uuid::Uuid::parse_str(&q.checkpoint.checkpoint_id).is_err()
        || q.checkpoint.generation.is_empty()
        || q.checkpoint.generation.len() > MAX_IDENTITY
        || q.checkpoint.library_id.is_empty()
        || q.checkpoint.library_id.len() > MAX_IDENTITY
        || q.checkpoint.metadata_sha256.len() != 64
    {
        return Err(invalid(
            "qualification policy, owner or physical identity changed",
        ));
    }
    exact_pending(conn, claim, Some(&q.checkpoint.generation))?;
    if registry_identity(conn)? != q.registry_id {
        return Err(invalid("registry identity changed or missing"));
    }
    let row: Option<(Option<String>, Option<String>)> = conn
        .query_row(
            "SELECT CASE WHEN length(CAST(id AS BLOB)) BETWEEN 1 AND ?3 THEN id ELSE NULL END,
         CASE WHEN length(CAST(metadata_json AS BLOB))<=?2
         THEN metadata_json ELSE NULL END FROM libraries WHERE path=?1",
            params![path_text(&q.root)?, MAX_METADATA, MAX_IDENTITY],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?;
    let (id, metadata) = row.ok_or_else(|| invalid("checkpoint library missing"))?;
    let id = id.ok_or_else(|| invalid("empty or oversized library identity"))?;
    let metadata = metadata.ok_or_else(|| invalid("oversized metadata"))?;
    validate_metadata(&metadata)?;
    if id != q.checkpoint.library_id || digest(&metadata) != q.checkpoint.metadata_sha256 {
        return Err(invalid("checkpoint payload or library identity changed"));
    }
    Ok(())
}

impl LibraryRegistry {
    pub(crate) fn checkpoint_pending_reservation(
        &self,
        claim: &PrimaryInstanceClaim,
        lifetime: &StoreLifetime,
        metadata: &str,
    ) -> Result<PendingReservationCheckpoint> {
        validate_metadata(metadata)?;
        lifetime.require_root(&claim.library_path)?;
        let (device, inode) = lifetime.physical_identity()?;
        let boot_id = boot_id()?;
        let path = path_text(&claim.library_path)?;
        let mut conn = self.lock_conn()?;
        require_full_durability(&conn)?;
        let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let generation = exact_pending(&tx, claim, None)?;
        tx.execute(
            "INSERT OR IGNORE INTO registry_config(key,value) VALUES(?1,?2)",
            params![REGISTRY_ID, uuid::Uuid::new_v4().to_string()],
        )?;
        let registry_id = registry_identity(&tx)?;
        let now = Utc::now().to_rfc3339();
        tx.execute(
            "INSERT OR IGNORE INTO libraries(id,name,path,created_at,last_accessed,version,metadata_json)
             VALUES(?1,'Pending local library',?2,?3,?3,?4,'{}')",
            params![uuid::Uuid::new_v4().to_string(), path, now, env!("CARGO_PKG_VERSION")],
        )?;
        tx.execute(
            "UPDATE libraries SET metadata_json=?2 WHERE path=?1",
            params![path, metadata],
        )?;
        let library_id: Option<String> = tx.query_row(
            "SELECT CASE WHEN length(CAST(id AS BLOB)) BETWEEN 1 AND ?2 THEN id ELSE NULL END
             FROM libraries WHERE path=?1",
            params![path, MAX_IDENTITY],
            |row| row.get(0),
        )?;
        let library_id =
            library_id.ok_or_else(|| invalid("empty or oversized library identity"))?;
        let checkpoint = PendingReservationCheckpoint {
            contract_version: 1,
            generation,
            library_id,
            metadata_sha256: digest(metadata),
            checkpoint_id: uuid::Uuid::new_v4().to_string(),
        };
        let qualification = Qualification {
            policy: POLICY.into(),
            checkpoint: checkpoint.clone(),
            registry_id,
            root: claim.library_path.clone(),
            pid: claim.pid,
            claim_token: claim.claim_token.clone(),
            device,
            inode,
            boot_id,
        };
        let json = serde_json::to_string(&qualification)?;
        if json.len() > MAX_QUALIFICATION {
            return Err(invalid("oversized qualification"));
        }
        tx.execute("INSERT INTO pending_reservation_checkpoints(library_path,qualification_json)
                    VALUES(?1,?2) ON CONFLICT(library_path) DO UPDATE SET qualification_json=excluded.qualification_json",
            params![path, json])?;
        validate(&tx, claim, lifetime, &qualification)?;
        tx.commit()?;
        Ok(checkpoint)
    }

    pub(crate) fn consume_pending_checkpoint(
        &self,
        claim: &PrimaryInstanceClaim,
        lifetime: &StoreLifetime,
    ) -> Result<()> {
        lifetime.require_root(&claim.library_path)?;
        let mut conn = self.lock_conn()?;
        require_full_durability(&conn)?;
        let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        exact_pending(&tx, claim, None)?;
        if let Some(q) = load(&tx, &claim.library_path)? {
            validate(&tx, claim, lifetime, &q)?;
        }
        tx.execute(
            "DELETE FROM pending_reservation_checkpoints WHERE library_path=?1",
            [path_text(&claim.library_path)?],
        )?;
        tx.commit()?;
        Ok(())
    }

    pub(crate) fn recover_pending_claim(
        &self,
        root: &Path,
        expected: &PendingReservationCheckpoint,
        lifetime: &StoreLifetime,
    ) -> Result<PrimaryInstanceClaim> {
        lifetime.require_root(root)?;
        let mut conn = self.lock_conn()?;
        require_full_durability(&conn)?;
        let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let q =
            load(&tx, root)?.ok_or_else(|| invalid("no complete durable pending qualification"))?;
        if &q.checkpoint != expected {
            return Err(invalid("stale checkpoint observation"));
        }
        let predecessor = PrimaryInstanceClaim {
            library_path: root.to_owned(),
            pid: q.pid,
            claim_token: q.claim_token.clone(),
        };
        validate(&tx, &predecessor, lifetime, &q)?;
        let successor = PrimaryInstanceClaim {
            library_path: root.to_owned(),
            pid: std::process::id(),
            claim_token: uuid::Uuid::new_v4().to_string(),
        };
        // Generations are opaque strings. Mint a fresh UUID rather than relying
        // on clock monotonicity. This update's trigger invalidates qualification.
        let generation = uuid::Uuid::new_v4().to_string();
        let changed = tx.execute(
            "UPDATE instances SET pid=?2,started_at=?3,claim_token=?4,version=?5
             WHERE library_path=?1 AND pid=?6 AND started_at=?7 AND claim_token=?8 AND status='claiming'",
            params![path_text(root)?, successor.pid, generation, successor.claim_token,
                env!("CARGO_PKG_VERSION"), predecessor.pid, q.checkpoint.generation, predecessor.claim_token],
        )?;
        if changed != 1 {
            return Err(invalid("predecessor changed during recovery"));
        }
        lifetime.require_root(root)?;
        tx.commit()?;
        Ok(successor)
    }
}
