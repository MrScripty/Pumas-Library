//! Qualification receipts share the existing checkpoint slot and instance owner.
//! No PID liveness, TTL, new owner table or child cessation inference.
use super::pending_recovery::{boot_id, registry_identity, require_full_durability};
use super::*;
use crate::platform::store_lifetime::StoreLifetime;

const POLICY: &str = "pumas.existing-index-query-only@1";
const REGISTRY_ID: &str = "pending_reservation_registry_id@1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CatalogOwnerCheckpoint {
    pub contract_version: u32,
    pub generation: String,
    pub library_id: String,
    pub models_sha256: String,
    pub checkpoint_id: String,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Qualification {
    policy: String,
    version: String,
    checkpoint: CatalogOwnerCheckpoint,
    owner: InstanceEntry,
    claim_token: Option<String>,
    registry_id: String,
    device: u64,
    inode: u64,
    database_identity: (u64, u64),
    boot_id: String,
}

pub(crate) fn denied(message: &str) -> PumasError {
    PumasError::InvalidParams {
        message: format!("catalog query profile: {message}"),
    }
}

/// Stable cooperating namespace only; aliased/shared external writers are not
/// admitted. Check before and after opening/snapshotting the selected database.
#[cfg(target_os = "linux")]
pub(crate) fn database_identity(root: &Path) -> Result<(u64, u64)> {
    use std::os::unix::fs::MetadataExt;
    if root
        .to_str()
        .is_none_or(|text| text.is_empty() || text.len() > 4096)
    {
        return Err(denied("bounded UTF-8 root required"));
    }
    let path = root.join("shared-resources/models/models.db");
    if path.canonicalize()? != path {
        return Err(denied("symlinked index subtree is unqualified"));
    }
    for suffix in ["", "-wal", "-shm"] {
        let candidate = PathBuf::from(format!("{}{suffix}", path.display()));
        match std::fs::symlink_metadata(&candidate) {
            Ok(meta) if !meta.is_file() || meta.nlink() != 1 => {
                return Err(denied("aliased or nonregular index files are unqualified"))
            }
            Ok(_) => (),
            Err(error) if !suffix.is_empty() && error.kind() == std::io::ErrorKind::NotFound => (),
            Err(error) => return Err(error.into()),
        }
    }
    let meta = std::fs::metadata(path)?;
    Ok((meta.dev(), meta.ino()))
}
#[cfg(not(target_os = "linux"))]
pub(crate) fn database_identity(_root: &Path) -> Result<(u64, u64)> {
    Err(denied("Linux qualification required"))
}

pub(super) fn bounded_instance(conn: &Connection, root: &Path) -> Result<Option<InstanceEntry>> {
    let valid:Option<bool>=conn.query_row("SELECT length(CAST(library_path AS BLOB)) BETWEEN 1 AND 4096 AND length(CAST(started_at AS BLOB)) BETWEEN 1 AND 128 AND (version IS NULL OR length(CAST(version AS BLOB))<=128) AND length(CAST(status AS BLOB))<=32 AND length(CAST(transport_kind AS BLOB))<=32 AND (claim_token IS NULL OR length(CAST(claim_token AS BLOB))<=128) AND (endpoint IS NULL OR length(CAST(endpoint AS BLOB))<=256) AND (connection_token IS NULL OR length(CAST(connection_token AS BLOB))<=128) FROM instances WHERE library_path=?1",[root.to_string_lossy().as_ref()],|row|row.get(0)).optional()?;
    match valid {
        None => Ok(None),
        Some(true) => LibraryRegistry::read_instance_entry(conn, &root.to_string_lossy()),
        Some(false) => Err(denied("oversized or incomplete instance identity")),
    }
}
fn library_id(conn: &Connection, root: &Path) -> Result<String> {
    let id:Option<String>=conn.query_row("SELECT CASE WHEN length(CAST(id AS BLOB)) BETWEEN 1 AND 128 THEN id ELSE NULL END FROM libraries WHERE path=?1",[root.to_string_lossy().as_ref()],|row|row.get(0)).optional()?.flatten();
    id.ok_or_else(|| denied("missing or oversized library identity"))
}
fn load(conn: &Connection, root: &Path) -> Result<Qualification> {
    let text: Option<String> = conn.query_row("SELECT CASE WHEN length(CAST(qualification_json AS BLOB))<=8192 THEN qualification_json ELSE NULL END FROM pending_reservation_checkpoints WHERE library_path=?1",[root.to_string_lossy().as_ref()],|row|row.get(0)).optional()?.flatten();
    serde_json::from_str(&text.ok_or_else(|| denied("missing bounded catalog qualification"))?)
        .map_err(|_| denied("invalid catalog qualification"))
}
fn store(conn: &Connection, q: &Qualification) -> Result<()> {
    let text = serde_json::to_string(q)?;
    if text.len() > 8192 {
        return Err(denied("qualification exceeds bound"));
    }
    conn.execute("INSERT OR REPLACE INTO pending_reservation_checkpoints(library_path,qualification_json) VALUES(?1,?2)",params![q.owner.library_path.to_string_lossy(),text])?;
    Ok(())
}
fn validate(conn: &Connection, q: &Qualification, lifetime: &StoreLifetime) -> Result<()> {
    let root = &q.owner.library_path;
    lifetime.require_root(root)?;
    let (device, inode) = lifetime.physical_identity()?;
    let current = bounded_instance(conn, root)?.ok_or_else(|| denied("owner missing"))?;
    let library_id = library_id(conn, root)?;
    let token: Option<String> = conn.query_row(
        "SELECT claim_token FROM instances WHERE library_path=?1",
        [root.to_string_lossy().as_ref()],
        |row| row.get(0),
    )?;
    let http: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM http_services WHERE library_path=?1)",
        [root.to_string_lossy().as_ref()],
        |row| row.get(0),
    )?;
    if q.policy != POLICY
        || q.version != env!("CARGO_PKG_VERSION")
        || q.checkpoint.contract_version != 1
        || q.checkpoint.generation != current.started_at
        || q.checkpoint.library_id != library_id
        || q.registry_id != registry_identity(conn)?
        || q.device != device
        || q.inode != inode
        || q.database_identity != database_identity(root)?
        || q.boot_id != boot_id()?
        || serde_json::to_value(&current)? != serde_json::to_value(&q.owner)?
        || token != q.claim_token
        || http
        || uuid::Uuid::parse_str(&q.checkpoint.checkpoint_id).is_err()
    {
        return Err(denied("scope, generation or physical identity changed"));
    }
    Ok(())
}
impl LibraryRegistry {
    pub(crate) fn register_catalog_library(&self, root: &Path) -> Result<()> {
        let mut conn = self.lock_conn()?;
        require_full_durability(&conn)?;
        let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        tx.execute("INSERT OR IGNORE INTO libraries(id,name,path,created_at,last_accessed,version,metadata_json) VALUES(?1,'catalog library',?2,?3,?3,?4,'{}')",params![uuid::Uuid::new_v4().to_string(),root.to_string_lossy(),Utc::now().to_rfc3339(),env!("CARGO_PKG_VERSION")])?;
        library_id(&tx, root)?;
        tx.commit()?;
        Ok(())
    }

    pub(crate) fn prepare_catalog_scope(
        &self,
        claim: &PrimaryInstanceClaim,
        lifetime: &StoreLifetime,
    ) -> Result<()> {
        lifetime.require_root(&claim.library_path)?;
        let database_identity = database_identity(&claim.library_path)?;
        let (device, inode) = lifetime.physical_identity()?;
        let mut conn = self.lock_conn()?;
        require_full_durability(&conn)?;
        let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let owner =
            bounded_instance(&tx, &claim.library_path)?.ok_or_else(|| denied("claim missing"))?;
        let token: Option<String> = tx.query_row(
            "SELECT claim_token FROM instances WHERE library_path=?1",
            [claim.library_path.to_string_lossy().as_ref()],
            |row| row.get(0),
        )?;
        if owner.status != InstanceStatus::Claiming
            || owner.pid != claim.pid
            || token.as_deref() != Some(&claim.claim_token)
        {
            return Err(denied("claim changed"));
        }
        tx.execute(
            "INSERT OR IGNORE INTO registry_config(key,value) VALUES(?1,?2)",
            params![REGISTRY_ID, uuid::Uuid::new_v4().to_string()],
        )?;
        let library_id = library_id(&tx, &claim.library_path)?;
        let q = Qualification {
            policy: POLICY.into(),
            version: env!("CARGO_PKG_VERSION").into(),
            checkpoint: CatalogOwnerCheckpoint {
                contract_version: 1,
                generation: owner.started_at.clone(),
                library_id,
                models_sha256: String::new(),
                checkpoint_id: uuid::Uuid::new_v4().to_string(),
            },
            owner,
            claim_token: token,
            registry_id: registry_identity(&tx)?,
            device,
            inode,
            database_identity,
            boot_id: boot_id()?,
        };
        validate(&tx, &q, lifetime)?;
        store(&tx, &q)?;
        tx.commit()?;
        Ok(())
    }
    pub(crate) fn promote_catalog_ready(
        &self,
        claim: &PrimaryInstanceClaim,
        lifetime: &StoreLifetime,
        port: u16,
        digest: &str,
    ) -> Result<(InstanceEntry, CatalogOwnerCheckpoint)> {
        let mut conn = self.lock_conn()?;
        require_full_durability(&conn)?;
        let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let mut q = load(&tx, &claim.library_path)?;
        validate(&tx, &q, lifetime)?;
        if q.owner.status != InstanceStatus::Claiming
            || q.claim_token.as_deref() != Some(&claim.claim_token)
            || digest.len() != 64
        {
            return Err(denied("unqualified ready transition"));
        }
        tx.execute("UPDATE instances SET port=?2,status='ready',claim_token=NULL,endpoint=?3,connection_token=?4 WHERE library_path=?1",params![claim.library_path.to_string_lossy(),port,loopback_tcp_endpoint(port),uuid::Uuid::new_v4().to_string()])?;
        // Existing invalidation trigger has consumed the pending receipt. Ready
        // promotion and its replacement receipt commit in one FULL transaction.
        q.owner = bounded_instance(&tx, &claim.library_path)?
            .ok_or_else(|| denied("ready owner missing"))?;
        q.claim_token = None;
        q.checkpoint.models_sha256 = digest.into();
        store(&tx, &q)?;
        tx.commit()?;
        Ok((q.owner, q.checkpoint))
    }
    pub(crate) fn validate_catalog_checkpoint(
        &self,
        owner: &InstanceEntry,
        lifetime: &StoreLifetime,
        digest: Option<&str>,
    ) -> Result<CatalogOwnerCheckpoint> {
        let mut conn = self.lock_conn()?;
        let tx = conn.transaction()?;
        let q = load(&tx, &owner.library_path)?;
        validate(&tx, &q, lifetime)?;
        if q.owner.status != InstanceStatus::Ready
            || !same_ready_generation(&q.owner, owner)
            || digest.is_some_and(|digest| q.checkpoint.models_sha256 != digest)
        {
            return Err(denied("catalog checkpoint changed"));
        }
        tx.commit()?;
        Ok(q.checkpoint)
    }
    pub(crate) fn recover_catalog_claim(
        &self,
        root: &Path,
        expected: &CatalogOwnerCheckpoint,
        lifetime: &StoreLifetime,
        digest: &str,
    ) -> Result<PrimaryInstanceClaim> {
        let mut conn = self.lock_conn()?;
        require_full_durability(&conn)?;
        let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let q = load(&tx, root)?;
        validate(&tx, &q, lifetime)?;
        if q.owner.library_path != root
            || q.owner.status != InstanceStatus::Ready
            || q.checkpoint != *expected
            || q.checkpoint.models_sha256 != digest
            || digest.len() != 64
        {
            return Err(denied("stale or unacknowledged catalog checkpoint"));
        }
        let claim = PrimaryInstanceClaim {
            library_path: root.into(),
            pid: std::process::id(),
            claim_token: uuid::Uuid::new_v4().to_string(),
        };
        let changed=tx.execute("UPDATE instances SET pid=?2,started_at=?3,claim_token=?4,status='claiming',port=0,endpoint=NULL,connection_token=NULL WHERE library_path=?1 AND started_at=?5 AND status='ready'",params![root.to_string_lossy(),claim.pid,uuid::Uuid::new_v4().to_string(),claim.claim_token,expected.generation])?;
        if changed != 1 {
            return Err(denied("generation changed during redemption"));
        }
        lifetime.require_root(root)?;
        tx.commit()?;
        Ok(claim)
    }
}
