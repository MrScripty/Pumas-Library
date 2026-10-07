//! SQLite-backed global registry for library paths and running instances.

use crate::config::RegistryConfig;
use crate::{PumasError, Result};
use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tracing::debug;

/// A registered library entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LibraryEntry {
    pub id: String,
    pub name: String,
    pub path: PathBuf,
    pub created_at: String,
    pub last_accessed: String,
    pub version: Option<String>,
    pub metadata_json: String,
}

/// A running instance entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstanceEntry {
    pub library_path: PathBuf,
    pub pid: u32,
    pub port: u16,
    pub transport_kind: LocalInstanceTransportKind,
    pub endpoint: String,
    pub connection_token: Option<String>,
    pub started_at: String,
    pub version: Option<String>,
    pub status: InstanceStatus,
}

/// Local transport advertised by a running Pumas instance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LocalInstanceTransportKind {
    LoopbackTcp,
    UnixSocket,
    WindowsNamedPipe,
}

impl LocalInstanceTransportKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::LoopbackTcp => "loopback_tcp",
            Self::UnixSocket => "unix_socket",
            Self::WindowsNamedPipe => "windows_named_pipe",
        }
    }

    pub fn preferred_order() -> &'static [Self] {
        #[cfg(unix)]
        {
            &[Self::UnixSocket, Self::LoopbackTcp]
        }

        #[cfg(windows)]
        {
            &[Self::WindowsNamedPipe, Self::LoopbackTcp]
        }

        #[cfg(not(any(unix, windows)))]
        {
            &[Self::LoopbackTcp]
        }
    }

    fn from_db(value: &str) -> Result<Self> {
        match value {
            "loopback_tcp" => Ok(Self::LoopbackTcp),
            "unix_socket" => Ok(Self::UnixSocket),
            "windows_named_pipe" => Ok(Self::WindowsNamedPipe),
            _ => Err(PumasError::Validation {
                field: "instances.transport_kind".to_string(),
                message: format!("unknown instance transport kind '{}'", value),
            }),
        }
    }
}

/// Lifecycle status for a tracked instance row.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InstanceStatus {
    Claiming,
    Ready,
}

impl InstanceStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Claiming => "claiming",
            Self::Ready => "ready",
        }
    }

    fn from_db(value: &str) -> Result<Self> {
        match value {
            "claiming" => Ok(Self::Claiming),
            "ready" => Ok(Self::Ready),
            _ => Err(PumasError::Validation {
                field: "instances.status".to_string(),
                message: format!("unknown instance status '{}'", value),
            }),
        }
    }
}

/// Claim row owned by a primary instance while startup is in progress.
#[derive(Debug, Clone)]
pub struct PrimaryInstanceClaim {
    pub library_path: PathBuf,
    pub pid: u32,
    pub claim_token: String,
}

/// Outcome of attempting to claim primary ownership for a library path.
#[derive(Debug, Clone)]
pub enum InstanceClaimResult {
    Claimed(PrimaryInstanceClaim),
    Occupied(InstanceEntry),
}

fn loopback_tcp_endpoint(port: u16) -> String {
    format!("127.0.0.1:{port}")
}

/// SQLite-backed global registry for library discovery and instance coordination.
///
/// Uses WAL mode for safe concurrent access across processes and
/// `Arc<Mutex<Connection>>` for thread safety within a process.
#[derive(Clone)]
pub struct LibraryRegistry {
    conn: Arc<Mutex<Connection>>,
}

impl LibraryRegistry {
    /// Open the registry at the default platform location.
    ///
    /// Creates the database and parent directories if they don't exist.
    pub fn open() -> Result<Self> {
        let db_path = crate::platform::registry_db_path()?;
        Self::open_at(&db_path)
    }

    /// Open the registry at a specific path.
    ///
    /// Creates the database and parent directories if they don't exist.
    pub fn open_at(db_path: &Path) -> Result<Self> {
        if let Some(parent) = db_path.parent() {
            if !parent.exists() {
                std::fs::create_dir_all(parent).map_err(|e| PumasError::Io {
                    message: format!("Failed to create registry directory: {}", parent.display()),
                    path: Some(parent.to_path_buf()),
                    source: Some(e),
                })?;
            }
        }

        let conn = Connection::open(db_path)?;
        Self::configure_connection(&conn)?;
        Self::ensure_schema(&conn)?;

        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    /// Observe an existing registry without creating it, migrating schema, or cleaning rows.
    pub fn open_read_only_at(db_path: &Path) -> Result<Self> {
        let conn =
            Connection::open_with_flags(db_path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)?;
        conn.busy_timeout(Duration::from_millis(u64::from(
            RegistryConfig::BUSY_TIMEOUT_MS,
        )))?;
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    fn configure_connection(conn: &Connection) -> Result<()> {
        Self::configure_wal(
            conn,
            Duration::from_millis(u64::from(RegistryConfig::BUSY_TIMEOUT_MS)),
        )?;
        conn.busy_timeout(Duration::from_millis(u64::from(
            RegistryConfig::BUSY_TIMEOUT_MS,
        )))?;
        conn.execute_batch("PRAGMA synchronous=NORMAL; PRAGMA temp_store=MEMORY;")?;
        Ok(())
    }

    /// WAL conversion can return SQLITE_BUSY without invoking SQLite's busy
    /// handler (lock-upgrade avoidance). Retry that initialization step only,
    /// within one shared budget; never turn other initialization failures into
    /// a fallback database or silently continue without the requested mode.
    fn configure_wal(conn: &Connection, budget: Duration) -> Result<()> {
        let deadline = Instant::now() + budget;
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            conn.busy_timeout(remaining.min(Duration::from_millis(20)))?;
            let outcome =
                conn.query_row("PRAGMA journal_mode=WAL", [], |row| row.get::<_, String>(0));
            match outcome {
                Ok(mode) if mode.eq_ignore_ascii_case("wal") => return Ok(()),
                Ok(_) => {
                    return Err(PumasError::Database {
                        message: "Registry could not establish WAL journal mode".into(),
                        source: None,
                    })
                }
                Err(error) => {
                    let remaining = deadline.saturating_duration_since(Instant::now());
                    if error.sqlite_error_code() != Some(rusqlite::ErrorCode::DatabaseBusy)
                        || remaining.is_zero()
                    {
                        return Err(error.into());
                    }
                    std::thread::sleep(remaining.min(Duration::from_millis(5)));
                }
            }
        }
    }

    fn ensure_schema(conn: &Connection) -> Result<()> {
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS libraries (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                path TEXT NOT NULL UNIQUE,
                created_at TEXT NOT NULL,
                last_accessed TEXT NOT NULL,
                version TEXT,
                metadata_json TEXT NOT NULL DEFAULT '{}'
            );

            CREATE TABLE IF NOT EXISTS instances (
                library_path TEXT PRIMARY KEY,
                pid INTEGER NOT NULL,
                port INTEGER NOT NULL,
                started_at TEXT NOT NULL,
                version TEXT,
                status TEXT NOT NULL DEFAULT 'ready',
                claim_token TEXT,
                transport_kind TEXT NOT NULL DEFAULT 'loopback_tcp',
                endpoint TEXT,
                connection_token TEXT
            );

            CREATE TABLE IF NOT EXISTS registry_config (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );",
        )?;
        Self::ensure_instances_columns(conn)?;
        Ok(())
    }

    fn ensure_instances_columns(conn: &Connection) -> Result<()> {
        if !Self::column_exists(conn, "instances", "status")? {
            conn.execute(
                "ALTER TABLE instances ADD COLUMN status TEXT NOT NULL DEFAULT 'ready'",
                [],
            )?;
        }
        if !Self::column_exists(conn, "instances", "claim_token")? {
            conn.execute("ALTER TABLE instances ADD COLUMN claim_token TEXT", [])?;
        }
        if !Self::column_exists(conn, "instances", "transport_kind")? {
            conn.execute(
                "ALTER TABLE instances ADD COLUMN transport_kind TEXT NOT NULL DEFAULT 'loopback_tcp'",
                [],
            )?;
        }
        if !Self::column_exists(conn, "instances", "endpoint")? {
            conn.execute("ALTER TABLE instances ADD COLUMN endpoint TEXT", [])?;
        }
        if !Self::column_exists(conn, "instances", "connection_token")? {
            conn.execute("ALTER TABLE instances ADD COLUMN connection_token TEXT", [])?;
        }
        Ok(())
    }

    fn column_exists(conn: &Connection, table: &str, column: &str) -> Result<bool> {
        let pragma = format!("PRAGMA table_info({})", table);
        let mut stmt = conn.prepare(&pragma)?;
        let rows = stmt.query_map([], |row| row.get::<_, String>(1))?;
        for row in rows {
            if row? == column {
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn lock_conn(&self) -> Result<std::sync::MutexGuard<'_, Connection>> {
        self.conn.lock().map_err(|_| PumasError::Database {
            message: "Failed to acquire registry connection lock".to_string(),
            source: None,
        })
    }

    fn canonicalize_library_path(path: &Path) -> Result<PathBuf> {
        path.canonicalize().map_err(|e| PumasError::Io {
            message: format!("Failed to canonicalize path: {}", path.display()),
            path: Some(path.to_path_buf()),
            source: Some(e),
        })
    }

    fn read_instance_entry(conn: &Connection, path_str: &str) -> Result<Option<InstanceEntry>> {
        conn.query_row(
            "SELECT library_path, pid, port, started_at, version, status,
                    transport_kind, endpoint, connection_token
             FROM instances WHERE library_path = ?1",
            params![path_str],
            |row| {
                let status: String = row.get(5)?;
                let transport_kind: String = row.get(6)?;
                let port: u16 = row.get(2)?;
                let endpoint = row
                    .get::<_, Option<String>>(7)?
                    .unwrap_or_else(|| loopback_tcp_endpoint(port));
                Ok(InstanceEntry {
                    library_path: PathBuf::from(row.get::<_, String>(0)?),
                    pid: row.get(1)?,
                    port,
                    transport_kind: LocalInstanceTransportKind::from_db(&transport_kind).map_err(
                        |err| {
                            rusqlite::Error::FromSqlConversionFailure(
                                6,
                                rusqlite::types::Type::Text,
                                Box::new(err),
                            )
                        },
                    )?,
                    endpoint,
                    connection_token: row.get(8)?,
                    started_at: row.get(3)?,
                    version: row.get(4)?,
                    status: InstanceStatus::from_db(&status).map_err(|err| {
                        rusqlite::Error::FromSqlConversionFailure(
                            5,
                            rusqlite::types::Type::Text,
                            Box::new(err),
                        )
                    })?,
                })
            },
        )
        .optional()
        .map_err(PumasError::from)
    }

    // ========================================
    // Library CRUD
    // ========================================

    /// Register a library path. Idempotent: updates last_accessed if already registered.
    pub fn register(&self, path: &Path, name: &str) -> Result<LibraryEntry> {
        let conn = self.lock_conn()?;
        let canonical = Self::canonicalize_library_path(path)?;
        let path_str = canonical.to_string_lossy().to_string();
        let now = Utc::now().to_rfc3339();

        // Check if already registered
        let existing: Option<String> = conn
            .query_row(
                "SELECT id FROM libraries WHERE path = ?1",
                params![path_str],
                |row| row.get(0),
            )
            .optional()?;

        if let Some(id) = existing {
            // Update last_accessed and name
            conn.execute(
                "UPDATE libraries SET last_accessed = ?1, name = ?2 WHERE id = ?3",
                params![now, name, id],
            )?;
            debug!("Updated existing library registration: {}", path_str);
            drop(conn);
            return self
                .get_by_path(&canonical)?
                .ok_or_else(|| PumasError::Database {
                    message: "Library disappeared after update".to_string(),
                    source: None,
                });
        }

        let id = uuid::Uuid::new_v4().to_string();
        let version = env!("CARGO_PKG_VERSION").to_string();

        conn.execute(
            "INSERT INTO libraries (id, name, path, created_at, last_accessed, version, metadata_json)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, '{}')",
            params![id, name, path_str, now, now, version],
        )?;

        debug!("Registered new library: {} at {}", name, path_str);

        let entry = LibraryEntry {
            id,
            name: name.to_string(),
            path: canonical,
            created_at: now.clone(),
            last_accessed: now,
            version: Some(version),
            metadata_json: "{}".to_string(),
        };

        Ok(entry)
    }

    /// Unregister a library path. Also removes any associated instance entry.
    pub fn unregister(&self, path: &Path) -> Result<bool> {
        let conn = self.lock_conn()?;
        let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
        let path_str = canonical.to_string_lossy().to_string();

        conn.execute(
            "DELETE FROM instances WHERE library_path = ?1",
            params![path_str],
        )?;
        let rows = conn.execute("DELETE FROM libraries WHERE path = ?1", params![path_str])?;

        if rows > 0 {
            debug!("Unregistered library: {}", path_str);
        }

        Ok(rows > 0)
    }

    /// List all registered libraries.
    pub fn list(&self) -> Result<Vec<LibraryEntry>> {
        let conn = self.lock_conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, name, path, created_at, last_accessed, version, metadata_json
             FROM libraries ORDER BY last_accessed DESC",
        )?;

        let rows = stmt.query_map([], |row| {
            Ok(LibraryEntry {
                id: row.get(0)?,
                name: row.get(1)?,
                path: PathBuf::from(row.get::<_, String>(2)?),
                created_at: row.get(3)?,
                last_accessed: row.get(4)?,
                version: row.get(5)?,
                metadata_json: row.get(6)?,
            })
        })?;

        let mut entries = Vec::new();
        for row in rows {
            entries.push(row?);
        }

        Ok(entries)
    }

    /// Get a library entry by its canonical path.
    pub fn get_by_path(&self, path: &Path) -> Result<Option<LibraryEntry>> {
        let conn = self.lock_conn()?;
        let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
        let path_str = canonical.to_string_lossy().to_string();

        let result = conn
            .query_row(
                "SELECT id, name, path, created_at, last_accessed, version, metadata_json
                 FROM libraries WHERE path = ?1",
                params![path_str],
                |row| {
                    Ok(LibraryEntry {
                        id: row.get(0)?,
                        name: row.get(1)?,
                        path: PathBuf::from(row.get::<_, String>(2)?),
                        created_at: row.get(3)?,
                        last_accessed: row.get(4)?,
                        version: row.get(5)?,
                        metadata_json: row.get(6)?,
                    })
                },
            )
            .optional()?;

        Ok(result)
    }

    /// Get the most recently accessed library (default).
    pub fn get_default(&self) -> Result<Option<LibraryEntry>> {
        let conn = self.lock_conn()?;
        let result = conn
            .query_row(
                "SELECT id, name, path, created_at, last_accessed, version, metadata_json
                 FROM libraries ORDER BY last_accessed DESC LIMIT 1",
                [],
                |row| {
                    Ok(LibraryEntry {
                        id: row.get(0)?,
                        name: row.get(1)?,
                        path: PathBuf::from(row.get::<_, String>(2)?),
                        created_at: row.get(3)?,
                        last_accessed: row.get(4)?,
                        version: row.get(5)?,
                        metadata_json: row.get(6)?,
                    })
                },
            )
            .optional()?;

        Ok(result)
    }

    /// Update the last_accessed timestamp for a library.
    pub fn touch(&self, path: &Path) -> Result<bool> {
        let conn = self.lock_conn()?;
        let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
        let path_str = canonical.to_string_lossy().to_string();
        let now = Utc::now().to_rfc3339();

        let rows = conn.execute(
            "UPDATE libraries SET last_accessed = ?1 WHERE path = ?2",
            params![now, path_str],
        )?;

        Ok(rows > 0)
    }

    // ========================================
    // Instance tracking
    // ========================================

    /// Claim an unoccupied library path. Existing rows are unresolved authority,
    /// even if their PID or endpoint is invisible in this observer's namespace.
    /// Crash recovery requires independently qualified lifetime custody; this
    /// registry is a rendezvous cache, not a physical-store lease.
    pub fn try_claim_instance(&self, path: &Path, pid: u32) -> Result<InstanceClaimResult> {
        let canonical = Self::canonicalize_library_path(path)?;
        let path_str = canonical.to_string_lossy().to_string();
        let now = Utc::now().to_rfc3339();
        let version = env!("CARGO_PKG_VERSION").to_string();
        let mut conn = self.lock_conn()?;
        // Serialize observation and replacement across independently opened
        // registries. A per-connection mutex cannot protect primary admission.
        let transaction =
            conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;

        if let Some(existing) = Self::read_instance_entry(&transaction, &path_str)? {
            return Ok(InstanceClaimResult::Occupied(existing));
        }

        let claim_token = uuid::Uuid::new_v4().to_string();
        transaction.execute(
            "INSERT INTO instances (
                 library_path, pid, port, started_at, version, status, claim_token,
                 transport_kind, endpoint, connection_token
             )
             VALUES (?1, ?2, 0, ?3, ?4, ?5, ?6, ?7, NULL, NULL)
             ON CONFLICT(library_path) DO UPDATE SET
                 pid=excluded.pid,
                 port=excluded.port,
                 started_at=excluded.started_at,
                 version=excluded.version,
                 status=excluded.status,
                 claim_token=excluded.claim_token,
                 transport_kind=excluded.transport_kind,
                 endpoint=NULL,
                 connection_token=NULL",
            params![
                path_str,
                pid,
                now,
                version,
                InstanceStatus::Claiming.as_str(),
                claim_token,
                LocalInstanceTransportKind::LoopbackTcp.as_str(),
            ],
        )?;

        transaction.commit()?;

        Ok(InstanceClaimResult::Claimed(PrimaryInstanceClaim {
            library_path: canonical,
            pid,
            claim_token,
        }))
    }

    /// Mark a previously claimed instance row as ready for client attachment.
    pub fn mark_instance_ready(&self, path: &Path, claim_token: &str, port: u16) -> Result<()> {
        self.promote_instance_ready(path, claim_token, port)
            .map(|_| ())
    }

    /// Retain the promoted generation atomically with the claim transition.
    pub(crate) fn promote_instance_ready(
        &self,
        path: &Path,
        claim_token: &str,
        port: u16,
    ) -> Result<InstanceEntry> {
        let mut conn = self.lock_conn()?;
        let transaction =
            conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let canonical = Self::canonicalize_library_path(path)?;
        let path_str = canonical.to_string_lossy().to_string();
        let endpoint = loopback_tcp_endpoint(port);
        let connection_token = uuid::Uuid::new_v4().to_string();

        let rows = transaction.execute(
            "UPDATE instances
             SET port = ?1,
                 status = ?2,
                 claim_token = NULL,
                 transport_kind = ?3,
                 endpoint = ?4,
                 connection_token = ?5
             WHERE library_path = ?6
               AND claim_token = ?7",
            params![
                port,
                InstanceStatus::Ready.as_str(),
                LocalInstanceTransportKind::LoopbackTcp.as_str(),
                endpoint,
                connection_token,
                path_str,
                claim_token,
            ],
        )?;

        if rows == 0 {
            return Err(PumasError::Validation {
                field: "instances.claim_token".to_string(),
                message: "primary claim could not be promoted to ready".to_string(),
            });
        }

        let instance = Self::read_instance_entry(&transaction, &path_str)?
            .ok_or_else(|| PumasError::Other("promoted instance disappeared".into()))?;
        transaction.commit()?;
        Ok(instance)
    }

    /// Release exactly the observed ready generation; stale owners cannot remove a successor.
    pub(crate) fn release_ready_instance(&self, instance: &InstanceEntry) -> Result<bool> {
        let Some(token) = &instance.connection_token else {
            return Ok(false);
        };
        let conn = self.lock_conn()?;
        let rows = conn.execute(
            "DELETE FROM instances WHERE library_path = ?1 AND started_at = ?2
             AND connection_token = ?3 AND status = 'ready'",
            params![
                instance.library_path.to_string_lossy(),
                instance.started_at,
                token
            ],
        )?;
        Ok(rows > 0)
    }

    /// Register a running instance for a library path.
    pub fn register_instance(&self, path: &Path, pid: u32, port: u16) -> Result<()> {
        let conn = self.lock_conn()?;
        let canonical = Self::canonicalize_library_path(path)?;
        let path_str = canonical.to_string_lossy().to_string();
        let now = Utc::now().to_rfc3339();
        let version = env!("CARGO_PKG_VERSION").to_string();
        let endpoint = loopback_tcp_endpoint(port);
        let connection_token = uuid::Uuid::new_v4().to_string();

        conn.execute(
            "INSERT INTO instances (
                 library_path, pid, port, started_at, version, status, claim_token,
                 transport_kind, endpoint, connection_token
             )
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL, ?7, ?8, ?9)
             ON CONFLICT(library_path) DO UPDATE SET
                 pid=excluded.pid,
                 port=excluded.port,
                 started_at=excluded.started_at,
                 version=excluded.version,
                 status=excluded.status,
                 claim_token=NULL,
                 transport_kind=excluded.transport_kind,
                 endpoint=excluded.endpoint,
                 connection_token=excluded.connection_token",
            params![
                path_str,
                pid,
                port,
                now,
                version,
                InstanceStatus::Ready.as_str(),
                LocalInstanceTransportKind::LoopbackTcp.as_str(),
                endpoint,
                connection_token,
            ],
        )?;

        debug!(
            "Registered instance for {}: PID {} on port {}",
            path_str, pid, port
        );

        Ok(())
    }

    /// Unregister a running instance for a library path.
    pub fn unregister_instance(&self, path: &Path) -> Result<bool> {
        let conn = self.lock_conn()?;
        let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
        let path_str = canonical.to_string_lossy().to_string();

        let rows = conn.execute(
            "DELETE FROM instances WHERE library_path = ?1",
            params![path_str],
        )?;

        if rows > 0 {
            debug!("Unregistered instance for {}", path_str);
        }

        Ok(rows > 0)
    }

    /// Get the running instance for a library path.
    pub fn get_instance(&self, path: &Path) -> Result<Option<InstanceEntry>> {
        let conn = self.lock_conn()?;
        let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
        let path_str = canonical.to_string_lossy().to_string();

        Self::read_instance_entry(&conn, &path_str)
    }

    /// List tracked instance rows newest-first.
    pub fn list_instances(&self) -> Result<Vec<InstanceEntry>> {
        let conn = self.lock_conn()?;
        let mut stmt =
            conn.prepare("SELECT library_path FROM instances ORDER BY started_at DESC")?;
        let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;

        let mut instances = Vec::new();
        for row in rows {
            let path_str = row?;
            if let Some(instance) = Self::read_instance_entry(&conn, &path_str)? {
                instances.push(instance);
            }
        }

        Ok(instances)
    }

    /// Compatibility no-op. A namespace-local PID or missing pathname cannot
    /// establish cessation. Explicit administrative reconciliation is required.
    pub fn cleanup_stale(&self) -> Result<usize> {
        Ok(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_test_registry() -> (LibraryRegistry, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test-registry.db");
        let registry = LibraryRegistry::open_at(&db_path).unwrap();
        (registry, temp_dir)
    }

    fn create_library_dir(parent: &Path, name: &str) -> PathBuf {
        let dir = parent.join(name);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn concurrent_fresh_registry_connections_all_establish_wal() {
        for _ in 0..8 {
            let root = TempDir::new().unwrap();
            let path = root.path().join("concurrent.db");
            let barrier = Arc::new(std::sync::Barrier::new(16));
            let workers: Vec<_> = (0..16)
                .map(|_| {
                    let path = path.clone();
                    let barrier = barrier.clone();
                    std::thread::spawn(move || {
                        barrier.wait();
                        let registry = LibraryRegistry::open_at(&path).unwrap();
                        let mode: String = registry
                            .lock_conn()
                            .unwrap()
                            .query_row("PRAGMA journal_mode", [], |row| row.get(0))
                            .unwrap();
                        assert_eq!(mode, "wal");
                    })
                })
                .collect();
            for worker in workers {
                worker.join().unwrap();
            }
        }
    }

    #[test]
    fn wal_contention_exhausts_its_budget_without_swallowing_busy() {
        let root = TempDir::new().unwrap();
        let path = root.path().join("locked.db");
        let owner = Connection::open(&path).unwrap();
        owner.execute_batch("CREATE TABLE sentinel (value TEXT); BEGIN EXCLUSIVE; INSERT INTO sentinel VALUES ('owned');").unwrap();
        let contender = Connection::open(&path).unwrap();
        let began = Instant::now();
        let error =
            LibraryRegistry::configure_wal(&contender, Duration::from_millis(40)).unwrap_err();
        assert!(
            matches!(error, PumasError::Database { source: Some(error), .. }
            if error.sqlite_error_code() == Some(rusqlite::ErrorCode::DatabaseBusy))
        );
        assert!(
            began.elapsed() >= Duration::from_millis(40),
            "busy failure must consume its retry budget"
        );
        assert!(
            began.elapsed() < Duration::from_secs(2),
            "busy retry must remain bounded"
        );
        owner.execute_batch("COMMIT").unwrap();
        LibraryRegistry::configure_wal(&contender, Duration::from_secs(1)).unwrap();
        let value: String = contender
            .query_row("SELECT value FROM sentinel", [], |row| row.get(0))
            .unwrap();
        assert_eq!(value, "owned");
    }

    #[test]
    fn wal_initialization_preserves_nonbusy_errors_and_rejects_unsupported_mode() {
        let root = TempDir::new().unwrap();
        let path = root.path().join("invalid.db");
        std::fs::write(&path, b"not a sqlite database").unwrap();
        let connection = Connection::open(&path).unwrap();
        let error =
            LibraryRegistry::configure_wal(&connection, Duration::from_secs(1)).unwrap_err();
        assert!(
            matches!(error, PumasError::Database { source: Some(error), .. }
            if error.sqlite_error_code() == Some(rusqlite::ErrorCode::NotADatabase))
        );
        assert_eq!(std::fs::read(path).unwrap(), b"not a sqlite database");
        let memory = Connection::open_in_memory().unwrap();
        assert!(LibraryRegistry::configure_wal(&memory, Duration::from_secs(1)).is_err());
    }

    #[test]
    fn test_register_library_creates_entry() {
        let (registry, temp_dir) = create_test_registry();
        let lib_dir = create_library_dir(temp_dir.path(), "my-library");

        let entry = registry.register(&lib_dir, "My Library").unwrap();

        assert_eq!(entry.name, "My Library");
        assert!(entry.version.is_some());
        assert!(!entry.id.is_empty());
    }

    #[test]
    fn test_register_library_idempotent_updates_timestamp() {
        let (registry, temp_dir) = create_test_registry();
        let lib_dir = create_library_dir(temp_dir.path(), "my-library");

        let first = registry.register(&lib_dir, "First Name").unwrap();
        let second = registry.register(&lib_dir, "Updated Name").unwrap();

        // Same ID, updated name
        assert_eq!(first.id, second.id);
        assert_eq!(second.name, "Updated Name");
        // Timestamps should differ (or be equal if very fast)
        assert!(second.last_accessed >= first.last_accessed);
    }

    #[test]
    fn test_unregister_library_removes_entry() {
        let (registry, temp_dir) = create_test_registry();
        let lib_dir = create_library_dir(temp_dir.path(), "my-library");

        registry.register(&lib_dir, "My Library").unwrap();
        assert!(registry.get_by_path(&lib_dir).unwrap().is_some());

        let removed = registry.unregister(&lib_dir).unwrap();
        assert!(removed);
        assert!(registry.get_by_path(&lib_dir).unwrap().is_none());
    }

    #[test]
    fn test_unregister_library_nonexistent_returns_false() {
        let (registry, temp_dir) = create_test_registry();
        let lib_dir = create_library_dir(temp_dir.path(), "nonexistent");

        let removed = registry.unregister(&lib_dir).unwrap();
        assert!(!removed);
    }

    #[test]
    fn test_list_libraries_returns_all() {
        let (registry, temp_dir) = create_test_registry();
        let lib1 = create_library_dir(temp_dir.path(), "lib-a");
        let lib2 = create_library_dir(temp_dir.path(), "lib-b");

        registry.register(&lib1, "Library A").unwrap();
        registry.register(&lib2, "Library B").unwrap();

        let entries = registry.list().unwrap();
        assert_eq!(entries.len(), 2);
    }

    #[test]
    fn test_list_libraries_empty_registry() {
        let (registry, _temp_dir) = create_test_registry();

        let entries = registry.list().unwrap();
        assert!(entries.is_empty());
    }

    #[test]
    fn test_get_by_path_found() {
        let (registry, temp_dir) = create_test_registry();
        let lib_dir = create_library_dir(temp_dir.path(), "my-library");

        registry.register(&lib_dir, "My Library").unwrap();

        let entry = registry.get_by_path(&lib_dir).unwrap();
        assert!(entry.is_some());
        assert_eq!(entry.unwrap().name, "My Library");
    }

    #[test]
    fn test_get_by_path_not_found() {
        let (registry, temp_dir) = create_test_registry();
        let lib_dir = create_library_dir(temp_dir.path(), "nonexistent");

        let entry = registry.get_by_path(&lib_dir).unwrap();
        assert!(entry.is_none());
    }

    #[test]
    fn test_get_default_returns_most_recent() {
        let (registry, temp_dir) = create_test_registry();
        let lib1 = create_library_dir(temp_dir.path(), "lib-older");
        let lib2 = create_library_dir(temp_dir.path(), "lib-newer");

        registry.register(&lib1, "Older Library").unwrap();
        // Small sleep to ensure different timestamps
        std::thread::sleep(std::time::Duration::from_millis(10));
        registry.register(&lib2, "Newer Library").unwrap();

        let default = registry.get_default().unwrap();
        assert!(default.is_some());
        assert_eq!(default.unwrap().name, "Newer Library");
    }

    #[test]
    fn test_get_default_empty_registry() {
        let (registry, _temp_dir) = create_test_registry();

        let default = registry.get_default().unwrap();
        assert!(default.is_none());
    }

    #[test]
    fn test_touch_updates_last_accessed() {
        let (registry, temp_dir) = create_test_registry();
        let lib_dir = create_library_dir(temp_dir.path(), "my-library");

        let entry = registry.register(&lib_dir, "My Library").unwrap();
        let original_ts = entry.last_accessed.clone();

        std::thread::sleep(std::time::Duration::from_millis(10));
        let touched = registry.touch(&lib_dir).unwrap();
        assert!(touched);

        let updated = registry.get_by_path(&lib_dir).unwrap().unwrap();
        assert!(updated.last_accessed >= original_ts);
    }

    #[test]
    fn test_register_instance_and_get() {
        let (registry, temp_dir) = create_test_registry();
        let lib_dir = create_library_dir(temp_dir.path(), "my-library");

        registry.register(&lib_dir, "My Library").unwrap();
        registry
            .register_instance(&lib_dir, std::process::id(), 12345)
            .unwrap();

        let instance = registry.get_instance(&lib_dir).unwrap();
        assert!(instance.is_some());
        let instance = instance.unwrap();
        assert_eq!(instance.pid, std::process::id());
        assert_eq!(instance.port, 12345);
        assert_eq!(
            instance.transport_kind,
            LocalInstanceTransportKind::LoopbackTcp
        );
        assert_eq!(instance.endpoint, "127.0.0.1:12345");
        assert!(instance.connection_token.is_some());
    }

    #[test]
    fn test_preferred_transport_order_is_platform_specific() {
        let order = LocalInstanceTransportKind::preferred_order();
        assert_eq!(order.last(), Some(&LocalInstanceTransportKind::LoopbackTcp));

        #[cfg(unix)]
        assert_eq!(order[0], LocalInstanceTransportKind::UnixSocket);

        #[cfg(windows)]
        assert_eq!(order[0], LocalInstanceTransportKind::WindowsNamedPipe);
    }

    #[test]
    fn test_open_at_migrates_legacy_instance_endpoint_columns() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("legacy-registry.db");
        let lib_dir = create_library_dir(temp_dir.path(), "legacy-library");
        let path_str = lib_dir
            .canonicalize()
            .unwrap()
            .to_string_lossy()
            .to_string();
        {
            let conn = Connection::open(&db_path).unwrap();
            conn.execute_batch(
                "CREATE TABLE libraries (
                    id TEXT PRIMARY KEY,
                    name TEXT NOT NULL,
                    path TEXT NOT NULL UNIQUE,
                    created_at TEXT NOT NULL,
                    last_accessed TEXT NOT NULL,
                    version TEXT,
                    metadata_json TEXT NOT NULL DEFAULT '{}'
                );
                CREATE TABLE instances (
                    library_path TEXT PRIMARY KEY,
                    pid INTEGER NOT NULL,
                    port INTEGER NOT NULL,
                    started_at TEXT NOT NULL,
                    version TEXT,
                    status TEXT NOT NULL DEFAULT 'ready',
                    claim_token TEXT
                );
                CREATE TABLE registry_config (
                    key TEXT PRIMARY KEY,
                    value TEXT NOT NULL
                );",
            )
            .unwrap();
            conn.execute(
                "INSERT INTO instances (
                    library_path, pid, port, started_at, version, status, claim_token
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL)",
                params![
                    path_str,
                    std::process::id(),
                    23456,
                    Utc::now().to_rfc3339(),
                    env!("CARGO_PKG_VERSION"),
                    InstanceStatus::Ready.as_str(),
                ],
            )
            .unwrap();
        }

        let registry = LibraryRegistry::open_at(&db_path).unwrap();
        let instance = registry.get_instance(&lib_dir).unwrap().unwrap();
        assert_eq!(
            instance.transport_kind,
            LocalInstanceTransportKind::LoopbackTcp
        );
        assert_eq!(instance.endpoint, "127.0.0.1:23456");
        assert!(instance.connection_token.is_none());
    }

    #[test]
    fn test_register_instance_upsert() {
        let (registry, temp_dir) = create_test_registry();
        let lib_dir = create_library_dir(temp_dir.path(), "my-library");

        registry.register(&lib_dir, "My Library").unwrap();
        registry.register_instance(&lib_dir, 100, 12345).unwrap();
        registry.register_instance(&lib_dir, 200, 54321).unwrap();

        let instance = registry.get_instance(&lib_dir).unwrap().unwrap();
        assert_eq!(instance.pid, 200);
        assert_eq!(instance.port, 54321);
        assert_eq!(instance.endpoint, "127.0.0.1:54321");
        assert!(instance.connection_token.is_some());
        assert_eq!(instance.status, InstanceStatus::Ready);
    }

    #[test]
    fn test_list_instances_returns_registered_instance_rows() {
        let (registry, temp_dir) = create_test_registry();
        let lib1 = create_library_dir(temp_dir.path(), "lib-one");
        let lib2 = create_library_dir(temp_dir.path(), "lib-two");

        registry.register(&lib1, "Library One").unwrap();
        registry.register(&lib2, "Library Two").unwrap();
        registry
            .register_instance(&lib1, std::process::id(), 11111)
            .unwrap();
        registry
            .register_instance(&lib2, std::process::id(), 22222)
            .unwrap();

        let instances = registry.list_instances().unwrap();
        assert_eq!(instances.len(), 2);
        assert!(instances
            .iter()
            .any(|instance| instance.port == 11111 && instance.endpoint == "127.0.0.1:11111"));
        assert!(instances
            .iter()
            .any(|instance| instance.port == 22222 && instance.endpoint == "127.0.0.1:22222"));
    }

    #[test]
    fn concurrent_claim_cannot_overwrite_an_owner_committed_while_waiting() {
        use std::cell::RefCell;
        use std::sync::mpsc;
        use std::time::Duration;

        // SQLite's busy callback gives a deterministic schedule without sleeps
        // or production hooks: the contender has reached its first write lock.
        thread_local! {
            static BUSY_GATE: RefCell<Option<(mpsc::Sender<()>, mpsc::Receiver<()>)>> =
                const { RefCell::new(None) };
        }
        fn wait_for_writer(_: i32) -> bool {
            BUSY_GATE.with(|gate| match gate.borrow_mut().take() {
                Some((blocked, release)) => {
                    blocked.send(()).is_ok() && release.recv_timeout(Duration::from_secs(5)).is_ok()
                }
                None => false,
            })
        }

        let (registry, temp) = create_test_registry();
        let library = create_library_dir(temp.path(), "competing-startup");
        registry.register(&library, "Competing startup").unwrap();
        let contender = LibraryRegistry::open_at(&temp.path().join("test-registry.db")).unwrap();
        let mut writer = registry.lock_conn().unwrap();
        let transaction = writer
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .unwrap();
        let (blocked_tx, blocked_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let contender_path = library.clone();
        let running = std::thread::spawn(move || {
            BUSY_GATE.with(|gate| *gate.borrow_mut() = Some((blocked_tx, release_rx)));
            contender
                .lock_conn()
                .unwrap()
                .busy_handler(Some(wait_for_writer))
                .unwrap();
            contender.try_claim_instance(&contender_path, std::process::id())
        });
        blocked_rx.recv_timeout(Duration::from_secs(5)).unwrap();
        transaction.execute(
            "INSERT INTO instances (library_path, pid, port, started_at, version, status, claim_token)
             VALUES (?1, ?2, 0, ?3, ?4, 'claiming', 'original-owner')",
            params![library.canonicalize().unwrap().to_string_lossy(), std::process::id(),
                Utc::now().to_rfc3339(), env!("CARGO_PKG_VERSION")],
        ).unwrap();
        transaction.commit().unwrap();
        drop(writer);
        release_tx.send(()).unwrap();
        let outcome = running.join().unwrap().unwrap();
        assert!(matches!(outcome, InstanceClaimResult::Occupied(_)));
        // The winner's exact token must still be promotable after the contender.
        registry
            .mark_instance_ready(&library, "original-owner", 12345)
            .unwrap();
    }

    #[test]
    fn test_try_claim_instance_creates_claiming_entry() {
        let (registry, temp_dir) = create_test_registry();
        let lib_dir = create_library_dir(temp_dir.path(), "my-library");

        registry.register(&lib_dir, "My Library").unwrap();
        let claim = registry
            .try_claim_instance(&lib_dir, std::process::id())
            .unwrap();

        let InstanceClaimResult::Claimed(claim) = claim else {
            panic!("expected claim to succeed");
        };
        assert_eq!(claim.pid, std::process::id());

        let instance = registry.get_instance(&lib_dir).unwrap().unwrap();
        assert_eq!(instance.pid, std::process::id());
        assert_eq!(instance.port, 0);
        assert_eq!(
            instance.transport_kind,
            LocalInstanceTransportKind::LoopbackTcp
        );
        assert_eq!(instance.endpoint, "127.0.0.1:0");
        assert!(instance.connection_token.is_none());
        assert_eq!(instance.status, InstanceStatus::Claiming);
    }

    #[test]
    fn test_try_claim_instance_returns_existing_live_instance() {
        let (registry, temp_dir) = create_test_registry();
        let lib_dir = create_library_dir(temp_dir.path(), "my-library");

        registry.register(&lib_dir, "My Library").unwrap();
        registry
            .register_instance(&lib_dir, std::process::id(), 12345)
            .unwrap();

        let claim = registry.try_claim_instance(&lib_dir, 424242).unwrap();
        let InstanceClaimResult::Occupied(existing) = claim else {
            panic!("expected live instance to block claim");
        };
        assert_eq!(existing.pid, std::process::id());
        assert_eq!(existing.port, 12345);
        assert_eq!(existing.endpoint, "127.0.0.1:12345");
        assert!(existing.connection_token.is_some());
        assert_eq!(existing.status, InstanceStatus::Ready);
    }

    #[test]
    fn test_try_claim_instance_preserves_namespace_unknown_owner() {
        let (registry, temp_dir) = create_test_registry();
        let lib_dir = create_library_dir(temp_dir.path(), "my-library");
        registry.register(&lib_dir, "My Library").unwrap();
        registry
            .register_instance(&lib_dir, 999_999_999, 12345)
            .unwrap();
        let before = registry.get_instance(&lib_dir).unwrap().unwrap();
        let result = registry.try_claim_instance(&lib_dir, 123456).unwrap();
        assert!(matches!(result, InstanceClaimResult::Occupied(_)));
        let after = registry.get_instance(&lib_dir).unwrap().unwrap();
        assert_eq!(after.connection_token, before.connection_token);
        assert_eq!(after.pid, before.pid);
    }

    #[test]
    fn test_mark_instance_ready_promotes_claim() {
        let (registry, temp_dir) = create_test_registry();
        let lib_dir = create_library_dir(temp_dir.path(), "my-library");

        registry.register(&lib_dir, "My Library").unwrap();
        let claim = registry
            .try_claim_instance(&lib_dir, std::process::id())
            .unwrap();
        let InstanceClaimResult::Claimed(claim) = claim else {
            panic!("expected claim to succeed");
        };

        registry
            .mark_instance_ready(&lib_dir, &claim.claim_token, 43210)
            .unwrap();

        let instance = registry.get_instance(&lib_dir).unwrap().unwrap();
        assert_eq!(instance.port, 43210);
        assert_eq!(
            instance.transport_kind,
            LocalInstanceTransportKind::LoopbackTcp
        );
        assert_eq!(instance.endpoint, "127.0.0.1:43210");
        assert!(instance.connection_token.is_some());
        assert_eq!(instance.status, InstanceStatus::Ready);
    }

    #[test]
    fn test_mark_instance_ready_rejects_wrong_token() {
        let (registry, temp_dir) = create_test_registry();
        let lib_dir = create_library_dir(temp_dir.path(), "my-library");

        registry.register(&lib_dir, "My Library").unwrap();
        let claim = registry
            .try_claim_instance(&lib_dir, std::process::id())
            .unwrap();
        let InstanceClaimResult::Claimed(claim) = claim else {
            panic!("expected claim to succeed");
        };

        let err = registry
            .mark_instance_ready(&lib_dir, "wrong-token", 43210)
            .unwrap_err();
        assert!(matches!(err, PumasError::Validation { .. }));

        let instance = registry.get_instance(&lib_dir).unwrap().unwrap();
        assert_eq!(instance.status, InstanceStatus::Claiming);
        assert_eq!(claim.pid, instance.pid);
    }

    #[test]
    fn test_unregister_instance() {
        let (registry, temp_dir) = create_test_registry();
        let lib_dir = create_library_dir(temp_dir.path(), "my-library");

        registry.register(&lib_dir, "My Library").unwrap();
        registry
            .register_instance(&lib_dir, std::process::id(), 12345)
            .unwrap();

        let removed = registry.unregister_instance(&lib_dir).unwrap();
        assert!(removed);

        let instance = registry.get_instance(&lib_dir).unwrap();
        assert!(instance.is_none());
    }

    #[test]
    fn test_get_instance_not_found() {
        let (registry, temp_dir) = create_test_registry();
        let lib_dir = create_library_dir(temp_dir.path(), "my-library");

        let instance = registry.get_instance(&lib_dir).unwrap();
        assert!(instance.is_none());
    }

    #[test]
    fn test_unregister_library_cascades_to_instance() {
        let (registry, temp_dir) = create_test_registry();
        let lib_dir = create_library_dir(temp_dir.path(), "my-library");

        registry.register(&lib_dir, "My Library").unwrap();
        registry
            .register_instance(&lib_dir, std::process::id(), 12345)
            .unwrap();

        registry.unregister(&lib_dir).unwrap();

        // Instance should also be removed
        let instance = registry.get_instance(&lib_dir).unwrap();
        assert!(instance.is_none());
    }

    #[test]
    fn test_cleanup_stale_preserves_unknown_pid() {
        let (registry, temp_dir) = create_test_registry();
        let lib_dir = create_library_dir(temp_dir.path(), "my-library");

        registry.register(&lib_dir, "My Library").unwrap();
        // Use a PID that almost certainly doesn't exist
        registry
            .register_instance(&lib_dir, 999_999_999, 12345)
            .unwrap();

        let removed = registry.cleanup_stale().unwrap();
        assert_eq!(removed, 0);
        assert!(registry.get_instance(&lib_dir).unwrap().is_some());
    }

    #[test]
    fn test_cleanup_stale_keeps_alive_instance() {
        let (registry, temp_dir) = create_test_registry();
        let lib_dir = create_library_dir(temp_dir.path(), "my-library");

        registry.register(&lib_dir, "My Library").unwrap();
        // Use our own PID - guaranteed alive
        registry
            .register_instance(&lib_dir, std::process::id(), 12345)
            .unwrap();

        let removed = registry.cleanup_stale().unwrap();
        assert_eq!(removed, 0);

        let instance = registry.get_instance(&lib_dir).unwrap();
        assert!(instance.is_some());
    }

    #[test]
    fn test_two_registries_same_db_concurrent_access() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("shared-registry.db");
        let lib_dir = create_library_dir(temp_dir.path(), "my-library");

        let reg1 = LibraryRegistry::open_at(&db_path).unwrap();
        let reg2 = LibraryRegistry::open_at(&db_path).unwrap();

        // Registry 1 writes
        reg1.register(&lib_dir, "Shared Library").unwrap();

        // Registry 2 reads
        let entry = reg2.get_by_path(&lib_dir).unwrap();
        assert!(entry.is_some());
        assert_eq!(entry.unwrap().name, "Shared Library");
    }

    #[test]
    fn test_path_canonicalization_prevents_duplicates() {
        let (registry, temp_dir) = create_test_registry();
        let lib_dir = create_library_dir(temp_dir.path(), "my-library");

        // Register with canonical path
        registry.register(&lib_dir, "My Library").unwrap();

        // Register with a path that includes ".." - should resolve to same
        let non_canonical = temp_dir.path().join("other").join("..").join("my-library");
        std::fs::create_dir_all(temp_dir.path().join("other")).unwrap();
        registry
            .register(&non_canonical, "My Library Again")
            .unwrap();

        // Should only have one entry
        let entries = registry.list().unwrap();
        assert_eq!(entries.len(), 1);
    }
}
