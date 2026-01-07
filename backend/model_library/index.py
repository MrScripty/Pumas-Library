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
        conn = sqlite3.connect(self.db_path)
        conn.row_factory = sqlite3.Row
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
