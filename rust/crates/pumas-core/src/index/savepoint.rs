//! All-or-nothing execution of multi-statement index changes.

use crate::{PumasError, Result};
use rusqlite::Connection;

/// Run `body` inside a named savepoint.
///
/// A savepoint nests inside a caller's transaction and, at the outermost
/// level, behaves as its own transaction, so it is atomic across a crash. A
/// failure rolls back every statement `body` ran and leaves no savepoint
/// open. `name` must be a plain SQL identifier chosen by the caller.
pub(crate) fn with_savepoint<T>(
    conn: &Connection,
    name: &str,
    body: impl FnOnce(&Connection) -> Result<T>,
) -> Result<T> {
    conn.execute_batch(&format!("SAVEPOINT {name};"))?;
    match body(conn) {
        Ok(value) => {
            conn.execute_batch(&format!("RELEASE {name};"))?;
            Ok(value)
        }
        Err(error) => {
            if let Err(rollback) =
                conn.execute_batch(&format!("ROLLBACK TO {name}; RELEASE {name};"))
            {
                return Err(PumasError::Database {
                    message: format!("{error}; rolling back {name} also failed: {rollback}"),
                    source: None,
                });
            }
            Err(error)
        }
    }
}
