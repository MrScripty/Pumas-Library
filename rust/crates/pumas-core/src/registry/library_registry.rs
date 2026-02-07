//! SQLite-backed global registry for library paths and running instances.

use crate::config::RegistryConfig;
use crate::{PumasError, Result};
use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tracing::{debug, warn};

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
    pub started_at: String,
    pub version: Option<String>,
}

/// SQLite-backed global registry for library discovery and instance coordination.
///
/// Uses WAL mode for safe concurrent access across processes and
/// `Arc<Mutex<Connection>>` for thread safety within a process.
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
                    message: format!(
                        "Failed to create registry directory: {}",
                        parent.display()
                    ),
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

    fn configure_connection(conn: &Connection) -> Result<()> {
        conn.execute_batch(&format!(
            "PRAGMA journal_mode=WAL;\n\
             PRAGMA busy_timeout={};\n\
             PRAGMA synchronous=NORMAL;\n\
             PRAGMA temp_store=MEMORY;",
            RegistryConfig::BUSY_TIMEOUT_MS,
        ))?;
        Ok(())
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
                version TEXT
            );

            CREATE TABLE IF NOT EXISTS registry_config (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );",
        )?;
        Ok(())
    }

    fn lock_conn(&self) -> Result<std::sync::MutexGuard<'_, Connection>> {
        self.conn.lock().map_err(|_| PumasError::Database {
            message: "Failed to acquire registry connection lock".to_string(),
            source: None,
        })
    }

    // ========================================
    // Library CRUD
    // ========================================

    /// Register a library path. Idempotent: updates last_accessed if already registered.
    pub fn register(&self, path: &Path, name: &str) -> Result<LibraryEntry> {
        let conn = self.lock_conn()?;
        let canonical = path.canonicalize().map_err(|e| PumasError::Io {
            message: format!("Failed to canonicalize path: {}", path.display()),
            path: Some(path.to_path_buf()),
            source: Some(e),
        })?;
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
        let canonical = path
            .canonicalize()
            .unwrap_or_else(|_| path.to_path_buf());
        let path_str = canonical.to_string_lossy().to_string();

        conn.execute(
            "DELETE FROM instances WHERE library_path = ?1",
            params![path_str],
        )?;
        let rows = conn.execute(
            "DELETE FROM libraries WHERE path = ?1",
            params![path_str],
        )?;

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
        let canonical = path
            .canonicalize()
            .unwrap_or_else(|_| path.to_path_buf());
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
        let canonical = path
            .canonicalize()
            .unwrap_or_else(|_| path.to_path_buf());
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

    /// Register a running instance for a library path.
    pub fn register_instance(&self, path: &Path, pid: u32, port: u16) -> Result<()> {
        let conn = self.lock_conn()?;
        let canonical = path.canonicalize().map_err(|e| PumasError::Io {
            message: format!("Failed to canonicalize path: {}", path.display()),
            path: Some(path.to_path_buf()),
            source: Some(e),
        })?;
        let path_str = canonical.to_string_lossy().to_string();
        let now = Utc::now().to_rfc3339();
        let version = env!("CARGO_PKG_VERSION").to_string();

        conn.execute(
            "INSERT INTO instances (library_path, pid, port, started_at, version)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(library_path) DO UPDATE SET
                 pid=excluded.pid,
                 port=excluded.port,
                 started_at=excluded.started_at,
                 version=excluded.version",
            params![path_str, pid, port, now, version],
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
        let canonical = path
            .canonicalize()
            .unwrap_or_else(|_| path.to_path_buf());
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
        let canonical = path
            .canonicalize()
            .unwrap_or_else(|_| path.to_path_buf());
        let path_str = canonical.to_string_lossy().to_string();

        let result = conn
            .query_row(
                "SELECT library_path, pid, port, started_at, version
                 FROM instances WHERE library_path = ?1",
                params![path_str],
                |row| {
                    Ok(InstanceEntry {
                        library_path: PathBuf::from(row.get::<_, String>(0)?),
                        pid: row.get(1)?,
                        port: row.get(2)?,
                        started_at: row.get(3)?,
                        version: row.get(4)?,
                    })
                },
            )
            .optional()?;

        Ok(result)
    }

    /// Remove stale instance entries (dead PIDs or nonexistent library paths).
    pub fn cleanup_stale(&self) -> Result<usize> {
        let conn = self.lock_conn()?;

        let mut stmt = conn.prepare(
            "SELECT library_path, pid FROM instances",
        )?;

        let entries: Vec<(String, u32)> = stmt
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
            .filter_map(|r| r.ok())
            .collect();
        drop(stmt);

        let mut removed = 0;
        for (path_str, pid) in &entries {
            let path = Path::new(path_str);
            let pid_alive = crate::platform::is_process_alive(*pid);
            let path_exists = path.exists();

            if !pid_alive || !path_exists {
                conn.execute(
                    "DELETE FROM instances WHERE library_path = ?1",
                    params![path_str],
                )?;
                removed += 1;

                if !pid_alive {
                    debug!("Cleaned up stale instance: PID {} (dead)", pid);
                } else {
                    debug!("Cleaned up stale instance: path {} (missing)", path_str);
                }
            }
        }

        // Also clean up library entries with nonexistent paths
        let mut lib_stmt = conn.prepare("SELECT path FROM libraries")?;
        let lib_paths: Vec<String> = lib_stmt
            .query_map([], |row| row.get(0))?
            .filter_map(|r| r.ok())
            .collect();
        drop(lib_stmt);

        for path_str in &lib_paths {
            let path = Path::new(path_str);
            if !path.exists() {
                conn.execute(
                    "DELETE FROM instances WHERE library_path = ?1",
                    params![path_str],
                )?;
                conn.execute(
                    "DELETE FROM libraries WHERE path = ?1",
                    params![path_str],
                )?;
                removed += 1;
                warn!("Removed library with nonexistent path: {}", path_str);
            }
        }

        Ok(removed)
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
    }

    #[test]
    fn test_register_instance_upsert() {
        let (registry, temp_dir) = create_test_registry();
        let lib_dir = create_library_dir(temp_dir.path(), "my-library");

        registry.register(&lib_dir, "My Library").unwrap();
        registry
            .register_instance(&lib_dir, 100, 12345)
            .unwrap();
        registry
            .register_instance(&lib_dir, 200, 54321)
            .unwrap();

        let instance = registry.get_instance(&lib_dir).unwrap().unwrap();
        assert_eq!(instance.pid, 200);
        assert_eq!(instance.port, 54321);
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
    fn test_cleanup_stale_removes_dead_pid() {
        let (registry, temp_dir) = create_test_registry();
        let lib_dir = create_library_dir(temp_dir.path(), "my-library");

        registry.register(&lib_dir, "My Library").unwrap();
        // Use a PID that almost certainly doesn't exist
        registry
            .register_instance(&lib_dir, 999_999_999, 12345)
            .unwrap();

        let removed = registry.cleanup_stale().unwrap();
        assert!(removed >= 1);

        let instance = registry.get_instance(&lib_dir).unwrap();
        assert!(instance.is_none());
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
        registry.register(&non_canonical, "My Library Again").unwrap();

        // Should only have one entry
        let entries = registry.list().unwrap();
        assert_eq!(entries.len(), 1);
    }
}
