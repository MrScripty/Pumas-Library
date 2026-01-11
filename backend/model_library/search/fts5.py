"""FTS5 virtual table setup and migration for model search.

Provides full-text search capabilities for model metadata using SQLite's
FTS5 extension. Includes automatic sync triggers and migration utilities.
"""

from __future__ import annotations

import json
import sqlite3
from dataclasses import dataclass, field
from typing import Any

from backend.logging_config import get_logger

logger = get_logger(__name__)


@dataclass
class FTS5Config:
    """Configuration for FTS5 virtual table.

    Attributes:
        table_name: Name of the FTS5 virtual table
        tokenizer: FTS5 tokenizer to use (unicode61, porter, ascii)
        remove_diacritics: Whether to remove diacritics during tokenization
        tokenchars: Characters to treat as part of tokens (not separators)
        indexed_columns: Columns to include in the FTS5 index
    """

    table_name: str = "model_search"
    tokenizer: str = "unicode61"
    remove_diacritics: bool = True
    tokenchars: str = "-_."
    indexed_columns: list[str] = field(
        default_factory=lambda: [
            "id",
            "official_name",
            "cleaned_name",
            "model_type",
            "tags",
            "family",
            "description",
        ]
    )

    def get_tokenizer_options(self) -> str:
        """Generate tokenizer options string for FTS5 CREATE statement.

        Returns:
            Tokenizer configuration string
        """
        diacritics = "1" if self.remove_diacritics else "0"
        parts = [self.tokenizer, f"remove_diacritics {diacritics}"]

        # Note: tokenchars requires special escaping in SQLite FTS5
        # We skip it here as unicode61 handles most cases well without it
        # tokenchars would be: tokenchars '"-_."' but escaping is complex

        return " ".join(parts)


def fts5_table_exists(
    conn: sqlite3.Connection,
    table_name: str = "model_search",
) -> bool:
    """Check if FTS5 virtual table exists.

    Args:
        conn: SQLite database connection
        table_name: Name of the FTS5 table to check

    Returns:
        True if table exists, False otherwise
    """
    cursor = conn.execute(
        """
        SELECT name FROM sqlite_master
        WHERE type = 'table' AND name = ?
        """,
        (table_name,),
    )
    return cursor.fetchone() is not None


def create_fts5_table(
    conn: sqlite3.Connection,
    config: FTS5Config | None = None,
) -> None:
    """Create FTS5 virtual table for model search.

    Creates an FTS5 virtual table with the specified configuration.
    The table is idempotent - calling multiple times is safe.

    Args:
        conn: SQLite database connection
        config: FTS5 configuration (uses defaults if None)
    """
    if config is None:
        config = FTS5Config()

    if fts5_table_exists(conn, config.table_name):
        logger.debug("FTS5 table %s already exists", config.table_name)
        return

    # Build column definitions
    columns = ", ".join(config.indexed_columns)
    tokenizer_opts = config.get_tokenizer_options()

    sql = f"""
        CREATE VIRTUAL TABLE {config.table_name} USING fts5(
            {columns},
            tokenize='{tokenizer_opts}'
        )
    """

    conn.execute(sql)
    conn.commit()
    logger.info("Created FTS5 table: %s", config.table_name)


def _extract_json_field(json_str: str, field: str) -> str:
    """Extract a field from a JSON string.

    Args:
        json_str: JSON string to parse
        field: Field name to extract

    Returns:
        Field value as string, or empty string if not found
    """
    try:
        data = json.loads(json_str)
        value = data.get(field, "")
        if isinstance(value, list):
            return " ".join(str(v) for v in value)
        return str(value) if value else ""
    except (json.JSONDecodeError, TypeError):  # noqa: multi-exception  # noqa: no-except-logging
        return ""


def _extract_tags(tags_json: str) -> str:
    """Extract tags from JSON array string.

    Args:
        tags_json: JSON array string

    Returns:
        Space-separated tags string
    """
    try:
        tags = json.loads(tags_json)
        if isinstance(tags, list):
            return " ".join(str(tag) for tag in tags)
        return ""
    except (json.JSONDecodeError, TypeError):  # noqa: multi-exception  # noqa: no-except-logging
        return ""


def create_fts5_triggers(
    conn: sqlite3.Connection,
    config: FTS5Config | None = None,
) -> None:
    """Create triggers to keep FTS5 table synchronized with models table.

    Creates AFTER INSERT, AFTER UPDATE, and AFTER DELETE triggers
    on the models table to automatically sync changes to FTS5.

    Args:
        conn: SQLite database connection
        config: FTS5 configuration (uses defaults if None)
    """
    if config is None:
        config = FTS5Config()

    table = config.table_name

    # Drop existing triggers first
    drop_fts5_triggers(conn, config)

    # INSERT trigger
    conn.execute(
        f"""
        CREATE TRIGGER {table}_ai AFTER INSERT ON models BEGIN
            INSERT INTO {table} (
                id, official_name, cleaned_name, model_type,
                tags, family, description
            ) VALUES (
                NEW.id,
                NEW.official_name,
                NEW.cleaned_name,
                NEW.model_type,
                (SELECT GROUP_CONCAT(value, ' ')
                 FROM json_each(NEW.tags_json)),
                json_extract(NEW.metadata_json, '$.family'),
                json_extract(NEW.metadata_json, '$.description')
            );
        END
    """
    )

    # UPDATE trigger - delete old, insert new
    conn.execute(
        f"""
        CREATE TRIGGER {table}_au AFTER UPDATE ON models BEGIN
            DELETE FROM {table} WHERE id = OLD.id;
            INSERT INTO {table} (
                id, official_name, cleaned_name, model_type,
                tags, family, description
            ) VALUES (
                NEW.id,
                NEW.official_name,
                NEW.cleaned_name,
                NEW.model_type,
                (SELECT GROUP_CONCAT(value, ' ')
                 FROM json_each(NEW.tags_json)),
                json_extract(NEW.metadata_json, '$.family'),
                json_extract(NEW.metadata_json, '$.description')
            );
        END
    """
    )

    # DELETE trigger
    conn.execute(
        f"""
        CREATE TRIGGER {table}_ad AFTER DELETE ON models BEGIN
            DELETE FROM {table} WHERE id = OLD.id;
        END
    """
    )

    conn.commit()
    logger.info("Created FTS5 sync triggers for table: %s", table)


def drop_fts5_triggers(
    conn: sqlite3.Connection,
    config: FTS5Config | None = None,
) -> None:
    """Drop FTS5 synchronization triggers.

    Args:
        conn: SQLite database connection
        config: FTS5 configuration (uses defaults if None)
    """
    if config is None:
        config = FTS5Config()

    table = config.table_name

    conn.execute(f"DROP TRIGGER IF EXISTS {table}_ai")
    conn.execute(f"DROP TRIGGER IF EXISTS {table}_au")
    conn.execute(f"DROP TRIGGER IF EXISTS {table}_ad")
    conn.commit()
    logger.debug("Dropped FTS5 triggers for table: %s", table)


def populate_fts5_from_models(
    conn: sqlite3.Connection,
    config: FTS5Config | None = None,
) -> int:
    """Populate FTS5 table from existing models table data.

    Clears existing FTS5 data and repopulates from models table.

    Args:
        conn: SQLite database connection
        config: FTS5 configuration (uses defaults if None)

    Returns:
        Number of rows populated
    """
    if config is None:
        config = FTS5Config()

    table = config.table_name

    # Clear existing FTS5 data
    conn.execute(f"DELETE FROM {table}")

    # Populate from models table
    conn.execute(
        f"""
        INSERT INTO {table} (
            id, official_name, cleaned_name, model_type,
            tags, family, description
        )
        SELECT
            m.id,
            m.official_name,
            m.cleaned_name,
            m.model_type,
            (SELECT GROUP_CONCAT(value, ' ')
             FROM json_each(m.tags_json)),
            json_extract(m.metadata_json, '$.family'),
            json_extract(m.metadata_json, '$.description')
        FROM models m
    """
    )

    conn.commit()

    # Get count
    cursor = conn.execute(f"SELECT COUNT(*) as cnt FROM {table}")
    row = cursor.fetchone()
    count = row[0] if row else 0

    logger.info("Populated FTS5 table with %d rows", count)
    return count


def migrate_to_fts5(
    conn: sqlite3.Connection,
    config: FTS5Config | None = None,
) -> None:
    """Migrate database to use FTS5 for search.

    Creates FTS5 table, populates it from existing data, and sets up
    synchronization triggers. This operation is idempotent.

    Args:
        conn: SQLite database connection
        config: FTS5 configuration (uses defaults if None)
    """
    if config is None:
        config = FTS5Config()

    # Create table if needed
    if not fts5_table_exists(conn, config.table_name):
        create_fts5_table(conn, config)
        populate_fts5_from_models(conn, config)
    else:
        # Table exists, check if triggers exist
        cursor = conn.execute(
            """
            SELECT name FROM sqlite_master
            WHERE type = 'trigger' AND name = ?
            """,
            (f"{config.table_name}_ai",),
        )
        if cursor.fetchone() is None:
            # Triggers missing, recreate
            populate_fts5_from_models(conn, config)

    # Ensure triggers exist
    create_fts5_triggers(conn, config)
    logger.info("FTS5 migration complete for table: %s", config.table_name)


class FTS5Manager:
    """Manager for FTS5 search operations.

    Provides high-level operations for managing FTS5 search index,
    including migration, rebuild, and optimization.

    Attributes:
        conn: SQLite database connection
        config: FTS5 configuration
    """

    def __init__(
        self,
        conn: sqlite3.Connection,
        config: FTS5Config | None = None,
    ) -> None:
        """Initialize FTS5 manager.

        Automatically migrates database if FTS5 table doesn't exist.

        Args:
            conn: SQLite database connection
            config: FTS5 configuration (uses defaults if None)
        """
        self.conn = conn
        self.config = config or FTS5Config()

        # Migrate on init if needed
        migrate_to_fts5(self.conn, self.config)

    def rebuild(self) -> int:
        """Rebuild FTS5 index from models table.

        Clears and repopulates the FTS5 table from the models table.

        Returns:
            Number of rows populated
        """
        logger.info("Rebuilding FTS5 index...")
        return populate_fts5_from_models(self.conn, self.config)

    def optimize(self) -> None:
        """Optimize FTS5 index for better query performance.

        Runs the FTS5 'optimize' command to merge index segments.
        """
        table = self.config.table_name
        self.conn.execute(f"INSERT INTO {table}({table}) VALUES('optimize')")
        self.conn.commit()
        logger.info("Optimized FTS5 index: %s", table)

    def get_stats(self) -> dict[str, Any]:
        """Get FTS5 index statistics.

        Returns:
            Dictionary with index statistics
        """
        table = self.config.table_name

        cursor = self.conn.execute(f"SELECT COUNT(*) as cnt FROM {table}")
        row = cursor.fetchone()
        row_count = row[0] if row else 0

        return {
            "table_name": table,
            "row_count": row_count,
            "tokenizer": self.config.tokenizer,
            "indexed_columns": self.config.indexed_columns,
        }
