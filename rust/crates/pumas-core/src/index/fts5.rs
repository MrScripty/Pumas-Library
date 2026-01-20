//! FTS5 virtual table setup and management.

use crate::Result;
use rusqlite::Connection;
use tracing::{debug, info};

/// Configuration for FTS5 table.
#[derive(Debug, Clone)]
pub struct FTS5Config {
    /// Name of the FTS5 virtual table.
    pub table_name: String,
    /// Tokenizer configuration.
    pub tokenizer: String,
}

impl Default for FTS5Config {
    fn default() -> Self {
        Self {
            table_name: "model_search".to_string(),
            tokenizer: "unicode61 remove_diacritics 1".to_string(),
        }
    }
}

/// Manager for FTS5 setup and maintenance.
pub struct FTS5Manager<'a> {
    config: &'a FTS5Config,
}

impl<'a> FTS5Manager<'a> {
    /// Create a new FTS5 manager.
    pub fn new(config: &'a FTS5Config) -> Self {
        Self { config }
    }

    /// Check if the FTS5 table exists.
    pub fn table_exists(&self, conn: &Connection) -> Result<bool> {
        let count: i32 = conn.query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name=?1",
            [&self.config.table_name],
            |row| row.get(0),
        )?;
        Ok(count > 0)
    }

    /// Check if the FTS5 triggers exist.
    pub fn triggers_exist(&self, conn: &Connection) -> Result<bool> {
        let trigger_name = format!("{}_ai", self.config.table_name);
        let count: i32 = conn.query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='trigger' AND name=?1",
            [&trigger_name],
            |row| row.get(0),
        )?;
        Ok(count > 0)
    }

    /// Ensure FTS5 is fully set up.
    pub fn ensure_setup(&self, conn: &Connection) -> Result<()> {
        if !self.table_exists(conn)? {
            self.create_table(conn)?;
            self.populate_from_models(conn)?;
        } else if !self.triggers_exist(conn)? {
            // Table exists but triggers missing - rebuild
            self.populate_from_models(conn)?;
        }

        self.create_triggers(conn)?;
        Ok(())
    }

    /// Create the FTS5 virtual table.
    pub fn create_table(&self, conn: &Connection) -> Result<()> {
        let sql = format!(
            "CREATE VIRTUAL TABLE IF NOT EXISTS {} USING fts5(
                id,
                official_name,
                cleaned_name,
                model_type,
                tags,
                family,
                description,
                tokenize='{}'
            )",
            self.config.table_name, self.config.tokenizer
        );

        conn.execute(&sql, [])?;
        info!("Created FTS5 table: {}", self.config.table_name);
        Ok(())
    }

    /// Create triggers to keep FTS5 in sync with models table.
    pub fn create_triggers(&self, conn: &Connection) -> Result<()> {
        let table = &self.config.table_name;

        // AFTER INSERT trigger
        let insert_trigger = format!(
            "CREATE TRIGGER IF NOT EXISTS {}_ai AFTER INSERT ON models BEGIN
                INSERT INTO {} (
                    id, official_name, cleaned_name, model_type,
                    tags, family, description
                ) VALUES (
                    NEW.id,
                    NEW.official_name,
                    NEW.cleaned_name,
                    NEW.model_type,
                    (SELECT GROUP_CONCAT(value, ' ') FROM json_each(NEW.tags_json)),
                    json_extract(NEW.metadata_json, '$.family'),
                    json_extract(NEW.metadata_json, '$.description')
                );
            END",
            table, table
        );
        conn.execute(&insert_trigger, [])?;

        // AFTER UPDATE trigger
        let update_trigger = format!(
            "CREATE TRIGGER IF NOT EXISTS {}_au AFTER UPDATE ON models BEGIN
                DELETE FROM {} WHERE id = OLD.id;
                INSERT INTO {} (
                    id, official_name, cleaned_name, model_type,
                    tags, family, description
                ) VALUES (
                    NEW.id,
                    NEW.official_name,
                    NEW.cleaned_name,
                    NEW.model_type,
                    (SELECT GROUP_CONCAT(value, ' ') FROM json_each(NEW.tags_json)),
                    json_extract(NEW.metadata_json, '$.family'),
                    json_extract(NEW.metadata_json, '$.description')
                );
            END",
            table, table, table
        );
        conn.execute(&update_trigger, [])?;

        // AFTER DELETE trigger
        let delete_trigger = format!(
            "CREATE TRIGGER IF NOT EXISTS {}_ad AFTER DELETE ON models BEGIN
                DELETE FROM {} WHERE id = OLD.id;
            END",
            table, table
        );
        conn.execute(&delete_trigger, [])?;

        debug!("Created FTS5 triggers for {}", table);
        Ok(())
    }

    /// Populate FTS5 from existing models table.
    pub fn populate_from_models(&self, conn: &Connection) -> Result<()> {
        let table = &self.config.table_name;

        // Clear existing FTS5 data using execute_batch to avoid "returns results" error
        conn.execute_batch(&format!("DELETE FROM {};", table))?;

        // Populate from models
        let sql = format!(
            "INSERT INTO {} (id, official_name, cleaned_name, model_type, tags, family, description)
             SELECT
                 id,
                 official_name,
                 cleaned_name,
                 model_type,
                 (SELECT GROUP_CONCAT(value, ' ') FROM json_each(tags_json)),
                 json_extract(metadata_json, '$.family'),
                 json_extract(metadata_json, '$.description')
             FROM models",
            table
        );
        conn.execute(&sql, [])?;

        info!("Populated FTS5 table from models");
        Ok(())
    }

    /// Rebuild the FTS5 index completely.
    pub fn rebuild(&self, conn: &Connection) -> Result<()> {
        // Drop and recreate
        let drop_sql = format!("DROP TABLE IF EXISTS {}", self.config.table_name);
        conn.execute(&drop_sql, [])?;

        // Drop triggers
        conn.execute(
            &format!("DROP TRIGGER IF EXISTS {}_ai", self.config.table_name),
            [],
        )?;
        conn.execute(
            &format!("DROP TRIGGER IF EXISTS {}_au", self.config.table_name),
            [],
        )?;
        conn.execute(
            &format!("DROP TRIGGER IF EXISTS {}_ad", self.config.table_name),
            [],
        )?;

        // Recreate
        self.create_table(conn)?;
        self.create_triggers(conn)?;
        self.populate_from_models(conn)?;

        info!("Rebuilt FTS5 index");
        Ok(())
    }

    /// Optimize the FTS5 index.
    pub fn optimize(&self, conn: &Connection) -> Result<()> {
        let sql = format!(
            "INSERT INTO {}({}) VALUES('optimize')",
            self.config.table_name, self.config.table_name
        );
        conn.execute(&sql, [])?;
        debug!("Optimized FTS5 index");
        Ok(())
    }

    /// Get statistics about the FTS5 index.
    pub fn get_stats(&self, conn: &Connection) -> Result<FTS5Stats> {
        let row_count: usize = conn.query_row(
            &format!("SELECT COUNT(*) FROM {}", self.config.table_name),
            [],
            |row| row.get(0),
        )?;

        Ok(FTS5Stats {
            table_name: self.config.table_name.clone(),
            row_count,
            tokenizer: self.config.tokenizer.clone(),
        })
    }
}

/// Statistics about an FTS5 index.
#[derive(Debug, Clone)]
pub struct FTS5Stats {
    pub table_name: String,
    pub row_count: usize,
    pub tokenizer: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_test_db() -> (Connection, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let conn = Connection::open(&db_path).unwrap();

        // Create models table
        conn.execute(
            "CREATE TABLE models (
                id TEXT PRIMARY KEY,
                path TEXT NOT NULL,
                cleaned_name TEXT NOT NULL,
                official_name TEXT NOT NULL,
                model_type TEXT NOT NULL,
                tags_json TEXT NOT NULL,
                hashes_json TEXT NOT NULL,
                metadata_json TEXT NOT NULL,
                updated_at TEXT NOT NULL
            )",
            [],
        )
        .unwrap();

        (conn, temp_dir)
    }

    #[test]
    fn test_fts5_setup() {
        let (conn, _temp) = create_test_db();
        let config = FTS5Config::default();
        let manager = FTS5Manager::new(&config);

        // Initially no table
        assert!(!manager.table_exists(&conn).unwrap());

        // Setup
        manager.ensure_setup(&conn).unwrap();

        // Now table exists
        assert!(manager.table_exists(&conn).unwrap());
        assert!(manager.triggers_exist(&conn).unwrap());
    }

    #[test]
    fn test_fts5_triggers() {
        let (conn, _temp) = create_test_db();
        let config = FTS5Config::default();
        let manager = FTS5Manager::new(&config);

        manager.ensure_setup(&conn).unwrap();

        // Insert a model
        conn.execute(
            "INSERT INTO models VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
            [
                "test-id",
                "path/to/model",
                "test_model",
                "Test Model",
                "checkpoint",
                r#"["tag1", "tag2"]"#,
                r#"{"sha256": "abc123"}"#,
                r#"{"family": "test", "description": "A test model"}"#,
                "2024-01-01T00:00:00Z",
            ],
        )
        .unwrap();

        // Check FTS5 was updated
        let count: usize = conn
            .query_row("SELECT COUNT(*) FROM model_search", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 1);

        // Verify content
        let (name, tags): (String, Option<String>) = conn
            .query_row(
                "SELECT official_name, tags FROM model_search WHERE id = ?",
                ["test-id"],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(name, "Test Model");
        assert!(tags.unwrap_or_default().contains("tag1"));
    }

    #[test]
    fn test_fts5_rebuild() {
        let (conn, _temp) = create_test_db();
        let config = FTS5Config::default();
        let manager = FTS5Manager::new(&config);

        manager.ensure_setup(&conn).unwrap();

        // Add data
        conn.execute(
            "INSERT INTO models VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
            [
                "model-1",
                "path",
                "name",
                "Name",
                "type",
                "[]",
                "{}",
                "{}",
                "2024-01-01",
            ],
        )
        .unwrap();

        // Rebuild
        manager.rebuild(&conn).unwrap();

        // Table still works
        assert!(manager.table_exists(&conn).unwrap());
    }
}
