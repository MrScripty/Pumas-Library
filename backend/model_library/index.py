"""SQLite index for model library metadata."""

from __future__ import annotations

import json
import sqlite3
from pathlib import Path
from typing import Any, Dict, List, cast

from backend.logging_config import get_logger
from backend.models import ModelMetadata, get_iso_timestamp

logger = get_logger(__name__)


class ModelIndex:
    """SQLite-backed index for model metadata."""

    def __init__(self, db_path: Path) -> None:
        self.db_path = Path(db_path)
        self._ensure_parent()
        self._ensure_schema()

    def _ensure_parent(self) -> None:
        self.db_path.parent.mkdir(parents=True, exist_ok=True)

    def _connect(self) -> sqlite3.Connection:
        """Create a database connection with WAL mode and optimizations.

        Returns:
            sqlite3.Connection with WAL mode, busy timeout, and optimizations enabled
        """
        conn = sqlite3.connect(self.db_path, timeout=30.0)
        conn.row_factory = sqlite3.Row
        # Enable WAL mode for concurrent read/write access
        conn.execute("PRAGMA journal_mode=WAL")
        # Busy timeout for handling concurrent access
        conn.execute("PRAGMA busy_timeout=30000")
        # Faster sync while still safe
        conn.execute("PRAGMA synchronous=NORMAL")
        # Use memory for temp tables
        conn.execute("PRAGMA temp_store=MEMORY")
        return conn

    def _ensure_schema(self) -> None:
        with self._connect() as conn:
            conn.execute(
                """
                CREATE TABLE IF NOT EXISTS models (
                    id TEXT PRIMARY KEY,
                    path TEXT NOT NULL,
                    cleaned_name TEXT NOT NULL,
                    official_name TEXT NOT NULL,
                    model_type TEXT NOT NULL,
                    tags_json TEXT NOT NULL,
                    hashes_json TEXT NOT NULL,
                    metadata_json TEXT NOT NULL,
                    updated_at TEXT NOT NULL
                )
                """
            )
            conn.commit()

    def upsert(self, record_id: str, path: str, metadata: ModelMetadata) -> None:
        cleaned_name = metadata.get("cleaned_name", "")
        official_name = metadata.get("official_name", "")
        model_type = metadata.get("model_type", "")
        tags_json = json.dumps(metadata.get("tags", []))
        hashes_json = json.dumps(metadata.get("hashes", {}))
        metadata_json = json.dumps(metadata, indent=2)
        updated_at = get_iso_timestamp()

        with self._connect() as conn:
            conn.execute(
                """
                INSERT INTO models (
                    id,
                    path,
                    cleaned_name,
                    official_name,
                    model_type,
                    tags_json,
                    hashes_json,
                    metadata_json,
                    updated_at
                )
                VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
                ON CONFLICT(id) DO UPDATE SET
                    path=excluded.path,
                    cleaned_name=excluded.cleaned_name,
                    official_name=excluded.official_name,
                    model_type=excluded.model_type,
                    tags_json=excluded.tags_json,
                    hashes_json=excluded.hashes_json,
                    metadata_json=excluded.metadata_json,
                    updated_at=excluded.updated_at
                """,
                (
                    record_id,
                    path,
                    cleaned_name,
                    official_name,
                    model_type,
                    tags_json,
                    hashes_json,
                    metadata_json,
                    updated_at,
                ),
            )
            conn.commit()

    def delete(self, record_id: str) -> None:
        with self._connect() as conn:
            conn.execute("DELETE FROM models WHERE id = ?", (record_id,))
            conn.commit()

    def clear(self) -> None:
        with self._connect() as conn:
            conn.execute("DELETE FROM models")
            conn.commit()

    def list_metadata(self) -> List[Dict[str, Any]]:
        with self._connect() as conn:
            rows = conn.execute("SELECT path, metadata_json FROM models ORDER BY path").fetchall()

        results: List[Dict[str, Any]] = []
        for row in rows:
            try:
                payload = cast(Dict[str, Any], json.loads(row["metadata_json"]))
            except json.JSONDecodeError:
                logger.warning("Invalid metadata JSON in models.db for %s", row["path"])
                continue
            payload.setdefault("library_path", row["path"])
            results.append(payload)
        return results

    def get_metadata(self, record_id: str) -> Dict[str, Any] | None:
        with self._connect() as conn:
            row = conn.execute(
                "SELECT path, metadata_json FROM models WHERE id = ?", (record_id,)
            ).fetchone()

        if not row:
            return None

        try:
            payload = cast(Dict[str, Any], json.loads(row["metadata_json"]))
        except json.JSONDecodeError:
            logger.warning("Invalid metadata JSON in models.db for %s", row["path"])
            return None

        payload.setdefault("library_path", row["path"])
        return payload

    def checkpoint_wal(self) -> dict[str, int]:
        """Checkpoint WAL file to main database.

        Should be called after large batch operations, periodic maintenance,
        or deep scan rebuilds to consolidate WAL data.

        Returns:
            Dictionary with checkpoint results:
            - busy: 1 if checkpoint couldn't complete, 0 otherwise
            - log_pages: Total pages in WAL log
            - checkpointed_pages: Pages written to database
        """
        with self._connect() as conn:
            result = conn.execute("PRAGMA wal_checkpoint(TRUNCATE)").fetchone()
            if result:
                busy, log_pages, checkpointed_pages = result
                logger.info(
                    "WAL checkpoint: busy=%d, log_pages=%d, checkpointed=%d",
                    busy,
                    log_pages,
                    checkpointed_pages,
                )
                return {
                    "busy": busy,
                    "log_pages": log_pages,
                    "checkpointed_pages": checkpointed_pages,
                }
        return {"busy": 0, "log_pages": 0, "checkpointed_pages": 0}
