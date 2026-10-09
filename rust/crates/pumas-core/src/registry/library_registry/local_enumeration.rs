//! Bounded, observation-only copy. Never expose a copied registry as authority.
use super::{InstanceStatus, LibraryRegistry, LocalInstanceTransportKind};
use crate::discovery::{RegisteredLibraryObservation, RegisteredOwnerObservation};
use crate::{PumasError, Result};
use std::fs::Metadata;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

const MAX_DB_BYTES: u64 = 32 * 1024 * 1024;
const MAX_WAL_BYTES: u64 = 64 * 1024 * 1024;
const MAX_LIBRARIES: usize = 4096;

fn denied() -> PumasError {
    PumasError::InvalidParams {
        message: "local registry observation is invalid, changed or exceeds bounds".into(),
    }
}

#[derive(PartialEq, Eq)]
struct Stamp {
    len: u64,
    modified: SystemTime,
    #[cfg(unix)]
    identity: (u64, u64, i64, i64),
}
impl Stamp {
    fn of(metadata: &Metadata) -> Result<Self> {
        if !metadata.is_file() {
            return Err(denied());
        }
        Ok(Self {
            len: metadata.len(),
            modified: metadata.modified().map_err(PumasError::from)?,
            #[cfg(unix)]
            identity: {
                use std::os::unix::fs::MetadataExt;
                (
                    metadata.dev(),
                    metadata.ino(),
                    metadata.ctime(),
                    metadata.ctime_nsec(),
                )
            },
        })
    }
}

#[derive(PartialEq, Eq)]
struct CapturedFile {
    stamp: Stamp,
    bytes: Vec<u8>,
}

fn sidecar(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.as_os_str().to_owned();
    name.push(suffix);
    PathBuf::from(name)
}

fn capture_file(path: &Path, limit: u64) -> Result<Option<CapturedFile>> {
    let metadata = match std::fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    if metadata.file_type().is_symlink() || metadata.len() > limit {
        return Err(denied());
    }
    let stamp = Stamp::of(&metadata)?;
    let mut options = std::fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    }
    let mut file = options.open(path)?;
    if Stamp::of(&file.metadata()?)? != stamp {
        return Err(denied());
    }
    let mut bytes = Vec::new();
    (&mut file).take(limit + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 != stamp.len
        || bytes.len() as u64 > limit
        || Stamp::of(&file.metadata()?)? != stamp
        || Stamp::of(&std::fs::symlink_metadata(path)?)? != stamp
    {
        return Err(denied());
    }
    Ok(Some(CapturedFile { stamp, bytes }))
}

#[derive(PartialEq, Eq)]
struct Capture {
    database: CapturedFile,
    wal: Option<CapturedFile>,
}
fn capture(path: &Path) -> Result<Capture> {
    match std::fs::symlink_metadata(sidecar(path, "-journal")) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        _ => return Err(denied()),
    }
    let database = capture_file(path, MAX_DB_BYTES)?
        .ok_or_else(|| PumasError::FileNotFound(path.to_owned()))?;
    let wal = capture_file(&sidecar(path, "-wal"), MAX_WAL_BYTES)?;
    Ok(Capture { database, wal })
}

pub(crate) fn observe(path: &Path) -> Result<Vec<RegisteredLibraryObservation>> {
    let absolute = std::path::absolute(path)?;
    let path = absolute.as_path();
    let first = capture(path)?;
    if capture(path).map_err(|_| denied())? != first {
        return Err(denied());
    }
    // Ignore ambient TMPDIR/TEMP so a consumer cannot place observer writes in
    // its source store. /tmp itself (or an ancestor) is not a qualified store.
    let scratch = Path::new("/tmp").canonicalize()?;
    let source_parent = path.parent().ok_or_else(denied)?.canonicalize()?;
    if scratch.starts_with(source_parent) {
        return Err(denied());
    }
    let temporary = tempfile::Builder::new()
        .prefix(&format!(
            "pumas-registry-observation-{}-",
            std::process::id()
        ))
        .tempdir_in(&scratch)?;
    let result = (|| {
        let copy = temporary.path().join("registry.db");
        std::fs::write(&copy, &first.database.bytes)?;
        if let Some(wal) = &first.wal {
            std::fs::write(sidecar(&copy, "-wal"), &wal.bytes)?;
        }
        // Only the private disposable copy can create SQLite coordination sidecars.
        let observations = {
            let registry = LibraryRegistry::open_read_only_at(&copy)?;
            registry.enumeration_rows()?
        };
        if observations
            .iter()
            .any(|row| scratch.starts_with(&row.library_root))
        {
            return Err(denied());
        }
        if capture(path).map_err(|_| denied())? != first {
            return Err(denied());
        }
        Ok(observations)
    })();
    // Never acknowledge cleanup while a copied SQLite connection remains open.
    temporary.close()?;
    result
}

impl LibraryRegistry {
    fn enumeration_rows(&self) -> Result<Vec<RegisteredLibraryObservation>> {
        let conn = self.lock_conn()?;
        let check: String = conn.query_row("PRAGMA quick_check", [], |row| row.get(0))?;
        if check != "ok" {
            return Err(denied());
        }
        // Views/generated columns cannot project private metadata as IDs/roots.
        // Accept additive ordinary columns, while retaining source uniqueness.
        for (table, primary, columns) in [
            ("libraries", "id", &["id", "path"][..]),
            (
                "instances",
                "library_path",
                &["library_path", "started_at", "status", "transport_kind"][..],
            ),
        ] {
            let real_table: bool = conn.query_row(
                "SELECT EXISTS(SELECT 1 FROM sqlite_schema WHERE name=?1 AND type='table' AND upper(sql) NOT LIKE 'CREATE VIRTUAL%')",
                [table], |row| row.get(0),
            )?;
            let primary_valid: bool = conn.query_row(
                "SELECT count(*)=1 AND max(name)=?2 AND max(pk)=1 FROM pragma_table_xinfo(?1) WHERE pk>0",
                [table, primary], |row| row.get(0),
            )?;
            if !real_table || !primary_valid {
                return Err(denied());
            }
            for column in columns {
                let valid: bool = conn.query_row(
                    "SELECT EXISTS(SELECT 1 FROM pragma_table_xinfo(?1) WHERE name=?2 AND upper(type)='TEXT' AND hidden=0)",
                    [table, *column], |row| row.get(0),
                )?;
                if !valid {
                    return Err(denied());
                }
            }
        }
        let unique_paths: bool = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM pragma_index_list('libraries') i WHERE i.\"unique\"=1 AND i.partial=0
                AND (SELECT count(*) FROM pragma_index_info(i.name))=1
                AND (SELECT name FROM pragma_index_info(i.name) WHERE seqno=0)='path')",
            [], |row| row.get(0),
        )?;
        if !unique_paths {
            return Err(denied());
        }
        // Bounds precede String allocation. Never select metadata, tokens, PID,
        // transport endpoint, HTTP JSON or any unrelated registered root path.
        let mut statement = conn.prepare(
            "SELECT typeof(l.id)='text' AND length(CAST(l.id AS BLOB)) BETWEEN 1 AND 4096
                AND typeof(l.path)='text' AND length(CAST(l.path AS BLOB)) BETWEEN 1 AND 8192
                AND (i.library_path IS NULL OR (
                    typeof(i.started_at)='text' AND length(CAST(i.started_at AS BLOB)) BETWEEN 1 AND 4096
                    AND typeof(i.status)='text' AND length(CAST(i.status AS BLOB))<=16
                    AND typeof(i.transport_kind)='text' AND length(CAST(i.transport_kind AS BLOB))<=32)),
                l.id, l.path, i.library_path IS NOT NULL, i.started_at, i.status, i.transport_kind
             FROM libraries l LEFT JOIN instances i ON i.library_path=l.path
             ORDER BY l.id, l.path LIMIT 4097",
        )?;
        let mut rows = statement.query([])?;
        let mut result = Vec::new();
        while let Some(row) = rows.next()? {
            if result.len() >= MAX_LIBRARIES || !row.get::<_, bool>(0)? {
                return Err(denied());
            }
            let library_root = PathBuf::from(row.get::<_, String>(2)?);
            if !library_root.is_absolute() {
                return Err(denied());
            }
            let owner = if row.get::<_, bool>(3)? {
                Some(RegisteredOwnerObservation {
                    generation: row.get(4)?,
                    status: InstanceStatus::from_db(&row.get::<_, String>(5)?)?,
                    transport: LocalInstanceTransportKind::from_db(&row.get::<_, String>(6)?)?,
                })
            } else {
                None
            };
            result.push(RegisteredLibraryObservation {
                registry_library_id: row.get(1)?,
                library_root,
                owner,
            });
        }
        Ok(result)
    }
}
