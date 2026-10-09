//! Strict, bounded raw-model observation; no projections, FTS or filesystem refresh.
use super::*;
use sha2::{Digest, Sha256};

impl ModelIndex {
    pub(crate) fn open_catalog_read_only(
        path: PathBuf,
        lifetime: crate::platform::store_lifetime::StoreLifetime,
    ) -> Result<Self> {
        let mut index = Self::open_read_only(path)?;
        {
            let conn = index
                .conn
                .lock()
                .map_err(|_| PumasError::Other("catalog connection poisoned".into()))?;
            let journal: String =
                conn.pragma_query_value(None, "journal_mode", |row| row.get(0))?;
            if journal != "wal" {
                return Err(PumasError::Other(
                    "catalog requires existing WAL index".into(),
                ));
            }
            let integrity: String = conn.query_row("PRAGMA quick_check", [], |row| row.get(0))?;
            if integrity != "ok" {
                return Err(PumasError::Other(
                    "catalog index integrity unconfirmed".into(),
                ));
            }
        }
        index._store_lifetime = lifetime;
        Ok(index)
    }

    /// The digest covers exact stored strings, ordered by primary key. It is a
    /// models-table checkpoint, not a claim about every other index table.
    pub(crate) fn strict_catalog_snapshot(&self) -> Result<(Vec<ModelRecord>, String)> {
        let mut conn = self
            .conn
            .lock()
            .map_err(|_| PumasError::Other("catalog connection poisoned".into()))?;
        let tx = conn.transaction()?;
        let mut schema = tx.prepare("PRAGMA table_info(models)")?;
        let columns = schema
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, i64>(5)?,
                ))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        let expected = [
            "id",
            "path",
            "cleaned_name",
            "official_name",
            "model_type",
            "tags_json",
            "hashes_json",
            "metadata_json",
            "updated_at",
        ];
        if columns.len() != expected.len()
            || columns
                .iter()
                .zip(expected)
                .enumerate()
                .any(|(i, ((name, kind, pk), expected))| {
                    name != expected
                        || !kind.eq_ignore_ascii_case("TEXT")
                        || *pk != if i == 0 { 1 } else { 0 }
                })
        {
            return Err(PumasError::Other("unsupported catalog row schema".into()));
        }
        drop(schema);
        let count: i64 = tx.query_row("SELECT count(*) FROM models", [], |row| row.get(0))?;
        if !(0..=100_000).contains(&count) {
            return Err(PumasError::Other(
                "catalog exceeds qualified row bound".into(),
            ));
        }
        let mut stmt = tx.prepare("SELECT id,path,cleaned_name,official_name,model_type,tags_json,hashes_json,metadata_json,updated_at FROM models ORDER BY id")?;
        let mut rows = stmt.query([])?;
        let mut records = Vec::new();
        let mut hasher = Sha256::new();
        hasher.update(b"pumas.strict-models@1");
        let mut bytes = 0usize;
        while let Some(row) = rows.next()? {
            let mut fields = Vec::new();
            for column in 0..9 {
                let value = row
                    .get_ref(column)?
                    .as_str()
                    .map_err(|_| PumasError::Other("catalog field is not UTF-8 text".into()))?;
                bytes = bytes
                    .checked_add(value.len())
                    .ok_or_else(|| PumasError::Other("catalog size overflow".into()))?;
                if value.len() > 1024 * 1024 || bytes > 64 * 1024 * 1024 {
                    return Err(PumasError::Other(
                        "catalog exceeds qualified byte bound".into(),
                    ));
                }
                hasher.update((value.len() as u64).to_le_bytes());
                hasher.update(value.as_bytes());
                fields.push(value.to_owned());
            }
            let metadata: serde_json::Value = serde_json::from_str(&fields[7])?;
            if fields[0].is_empty() || fields[0].len() > 4096 || !metadata.is_object() {
                return Err(PumasError::Other(
                    "invalid catalog identity or metadata object".into(),
                ));
            }
            records.push(ModelRecord {
                id: fields[0].clone(),
                path: fields[1].clone(),
                cleaned_name: fields[2].clone(),
                official_name: fields[3].clone(),
                model_type: fields[4].clone(),
                tags: serde_json::from_str(&fields[5])?,
                hashes: serde_json::from_str(&fields[6])?,
                metadata,
                updated_at: fields[8].clone(),
            });
        }
        drop(rows);
        drop(stmt);
        tx.commit()?;
        // A qualified whole-catalog List must fit the existing cross-language
        // frame bound, including worst-case query/envelope overhead.
        if serde_json::to_vec(&records)?.len()
            > crate::config::RegistryConfig::MAX_IPC_MESSAGE_SIZE - 32 * 1024
        {
            return Err(PumasError::Other(
                "catalog exceeds qualified response bound".into(),
            ));
        }
        Ok((records, hex::encode(hasher.finalize())))
    }
}

#[cfg(test)]
impl ModelIndex {
    pub(crate) fn hold_catalog_connection_for_test(
        &self,
        entered: std::sync::mpsc::Sender<()>,
        released: std::sync::mpsc::Receiver<()>,
    ) {
        let _connection = self.conn.lock().unwrap();
        entered.send(()).unwrap();
        released.recv().unwrap();
    }
}

#[cfg(test)]
mod bounded_reader_tests {
    use super::*;
    fn fixture() -> (tempfile::TempDir, ModelIndex) {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("index.db");
        let conn = Connection::open(&path).unwrap();
        conn.execute_batch("PRAGMA journal_mode=WAL; CREATE TABLE models(id TEXT PRIMARY KEY,path TEXT NOT NULL,cleaned_name TEXT NOT NULL,official_name TEXT NOT NULL,model_type TEXT NOT NULL,tags_json TEXT NOT NULL,hashes_json TEXT NOT NULL,metadata_json TEXT NOT NULL,updated_at TEXT NOT NULL);").unwrap();
        drop(conn);
        let index = ModelIndex::open_read_only(path).unwrap();
        (temp, index)
    }
    fn insert(index: &ModelIndex, id: &str, name: &str) {
        let conn = Connection::open(index.db_path()).unwrap();
        conn.execute(
            "INSERT INTO models VALUES(?1,'path',?2,'name','llm','[]','{}','{}','fixture')",
            params![id, name],
        )
        .unwrap();
    }
    #[test]
    fn labelled_stored_id_and_escaped_transport_size_bounds_refuse() {
        let (_temp, index) = fixture();
        insert(&index, &"i".repeat(4097), "fixture");
        assert!(index
            .strict_catalog_snapshot()
            .unwrap_err()
            .to_string()
            .contains("identity"));
        let (_temp, index) = fixture();
        for id in ["a", "b", "c"] {
            insert(&index, id, &"\0".repeat(1024 * 1024));
        }
        assert!(index
            .strict_catalog_snapshot()
            .unwrap_err()
            .to_string()
            .contains("response bound"));
    }
    #[test]
    fn labelled_incomplete_json_and_uncommitted_transaction_preserve_strict_observation() {
        let (_temp, index) = fixture();
        insert(&index, "fixture", "before");
        let (_, before) = index.strict_catalog_snapshot().unwrap();
        let conn = Connection::open(index.db_path()).unwrap();
        conn.execute_batch("BEGIN IMMEDIATE; UPDATE models SET cleaned_name='uncommitted';")
            .unwrap();
        assert_eq!(index.strict_catalog_snapshot().unwrap().1, before);
        drop(conn);
        assert_eq!(index.strict_catalog_snapshot().unwrap().1, before);
        let conn = Connection::open(index.db_path()).unwrap();
        conn.execute("UPDATE models SET hashes_json='incomplete'", [])
            .unwrap();
        assert!(index.strict_catalog_snapshot().is_err());
    }
}
