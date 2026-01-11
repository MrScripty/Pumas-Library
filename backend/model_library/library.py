"""Canonical model library management."""

from __future__ import annotations

import json
import threading
from pathlib import Path
from typing import Any, Dict, Iterable, List, Optional, cast

from backend.file_utils import atomic_write_json
from backend.logging_config import get_logger
from backend.model_library.index import ModelIndex
from backend.model_library.naming import normalize_name
from backend.model_library.search import FTS5Manager, SearchResult, search_models
from backend.models import ModelMetadata, ModelOverrides
from backend.utils import ensure_directory

logger = get_logger(__name__)


class ModelLibrary:
    """Manages model metadata and the SQLite index.

    Provides FTS5 full-text search via the search module integration.
    """

    def __init__(self, library_root: Path) -> None:
        self.library_root = Path(library_root)
        self.db_path = self.library_root / "models.db"
        self._write_lock = threading.Lock()
        ensure_directory(self.library_root)
        self.index = ModelIndex(self.db_path)
        self._fts5_manager: FTS5Manager | None = None

    def ensure_library(self) -> None:
        ensure_directory(self.library_root)
        self.index = ModelIndex(self.db_path)
        self._fts5_manager = None

    def model_dirs(self) -> Iterable[Path]:
        for meta_path in self.library_root.rglob("metadata.json"):
            if meta_path.name != "metadata.json":
                continue
            yield meta_path.parent

    def load_metadata(self, model_dir: Path) -> Optional[ModelMetadata]:
        meta_path = model_dir / "metadata.json"
        if not meta_path.exists():
            return None
        try:
            with open(meta_path, "r", encoding="utf-8") as f:
                return cast(ModelMetadata, json.load(f))
        except OSError as exc:
            logger.error("Failed to read metadata at %s: %s", meta_path, exc)
            return None
        except json.JSONDecodeError as exc:
            logger.error("Failed to read metadata at %s: %s", meta_path, exc)
            return None

    def save_metadata(self, model_dir: Path, metadata: ModelMetadata) -> None:
        meta_path = model_dir / "metadata.json"
        atomic_write_json(meta_path, metadata, lock=self._write_lock, keep_backup=True)

    def load_overrides(self, model_dir: Path) -> ModelOverrides:
        overrides_path = model_dir / "overrides.json"
        if not overrides_path.exists():
            return {}
        try:
            with open(overrides_path, "r", encoding="utf-8") as f:
                return cast(ModelOverrides, json.load(f))
        except OSError as exc:
            logger.error("Failed to read overrides at %s: %s", overrides_path, exc)
            return {}
        except json.JSONDecodeError as exc:
            logger.error("Failed to read overrides at %s: %s", overrides_path, exc)
            return {}

    def save_overrides(self, model_dir: Path, overrides: ModelOverrides) -> None:
        overrides_path = model_dir / "overrides.json"
        atomic_write_json(overrides_path, overrides, lock=self._write_lock, keep_backup=True)

    def build_model_path(self, model_type: str, family: str, cleaned_name: str) -> Path:
        cleaned_family = normalize_name(family)
        cleaned_model = normalize_name(cleaned_name)
        return self.library_root / model_type / cleaned_family / cleaned_model

    def index_model_dir(self, model_dir: Path, metadata: ModelMetadata) -> None:
        rel_path = str(model_dir.relative_to(self.library_root))
        record_id = rel_path
        self.index.upsert(record_id, rel_path, metadata)

    def rebuild_index(self) -> None:
        self.index.clear()
        for model_dir in self.model_dirs():
            metadata = self.load_metadata(model_dir)
            if not metadata:
                continue
            self.index_model_dir(model_dir, metadata)

    def list_models(self) -> List[Dict[str, Any]]:
        return self.index.list_metadata()

    def get_model(self, rel_path: str) -> Optional[Dict[str, Any]]:
        return self.index.get_metadata(rel_path)

    def _ensure_fts5(self) -> FTS5Manager:
        """Ensure FTS5 is set up and return the manager.

        Returns:
            FTS5Manager instance connected to the database
        """
        if self._fts5_manager is None:
            conn = self.index._connect()
            self._fts5_manager = FTS5Manager(conn)
        return self._fts5_manager

    def search_models(
        self,
        terms: str,
        limit: int = 100,
        offset: int = 0,
        model_type: str | list[str] | None = None,
        tags: list[str] | None = None,
    ) -> SearchResult:
        """Search models using FTS5 full-text search.

        Performs fast full-text search across model metadata including
        names, types, tags, family, and description.

        Args:
            terms: Search terms (space-separated for OR matching)
            limit: Maximum number of results to return
            offset: Number of results to skip
            model_type: Filter by model type(s)
            tags: Filter by required tags

        Returns:
            SearchResult with matching models and statistics
        """
        self._ensure_fts5()
        conn = self.index._connect()
        return search_models(
            conn=conn,
            terms=terms,
            limit=limit,
            offset=offset,
            model_type=model_type,
            tags=tags,
        )
