"""Canonical model library management."""

from __future__ import annotations

import json
import threading
from pathlib import Path
from typing import Any, Callable, Dict, Iterable, List, Optional, cast

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
        """Rebuild the SQLite index from existing metadata.json files.

        This is a fast rebuild that only reads existing metadata files.
        Use deep_scan_rebuild() for a full verification with hash recalculation.
        """
        self.index.clear()
        for model_dir in self.model_dirs():
            metadata = self.load_metadata(model_dir)
            if not metadata:
                continue
            self.index_model_dir(model_dir, metadata)

    def deep_scan_rebuild(
        self,
        verify_hashes: bool = False,
        progress_callback: Optional[Callable[[int, int, str], None]] = None,
    ) -> dict[str, Any]:
        """Perform a deep scan and rebuild of the library index.

        Scans all model directories, validates metadata, optionally
        recalculates file hashes, and rebuilds the SQLite index.

        Args:
            verify_hashes: If True, recalculate file hashes and update metadata
            progress_callback: Optional callback(current, total, model_id) for progress

        Returns:
            Dictionary with scan results:
            - total_models: Total number of models found
            - indexed: Number successfully indexed
            - errors: List of error messages
            - hash_mismatches: List of models with hash mismatches (if verify_hashes)
            - orphaned_dirs: Directories without valid metadata
        """
        from backend.model_library.io.hashing import compute_dual_hash

        results: dict[str, Any] = {
            "total_models": 0,
            "indexed": 0,
            "errors": [],
            "hash_mismatches": [],
            "orphaned_dirs": [],
        }

        # Find all potential model directories
        model_dirs = list(self.model_dirs())
        results["total_models"] = len(model_dirs)

        # Clear index for full rebuild
        self.index.clear()

        for i, model_dir in enumerate(model_dirs):
            model_id = str(model_dir.relative_to(self.library_root))

            if progress_callback:
                try:
                    progress_callback(i + 1, len(model_dirs), model_id)
                except TypeError:  # noqa: no-except-logging
                    # Ignore callback errors - may have wrong signature
                    pass

            metadata = self.load_metadata(model_dir)
            if not metadata:
                results["orphaned_dirs"].append(model_id)
                results["errors"].append(f"No valid metadata: {model_id}")
                continue

            # Optionally verify/recalculate hashes
            if verify_hashes:
                primary_file = self._find_primary_file(model_dir)
                if primary_file:
                    try:
                        sha256, blake3 = compute_dual_hash(primary_file)
                        stored_hashes = metadata.get("hashes", {})
                        stored_sha256 = stored_hashes.get("sha256", "")
                        stored_blake3 = stored_hashes.get("blake3", "")

                        if stored_sha256 and stored_sha256 != sha256:
                            results["hash_mismatches"].append(
                                {
                                    "model_id": model_id,
                                    "file": primary_file.name,
                                    "expected_sha256": stored_sha256,
                                    "actual_sha256": sha256,
                                }
                            )
                            logger.warning(
                                "SHA256 mismatch for %s: expected %s, got %s",
                                model_id,
                                stored_sha256[:16],
                                sha256[:16],
                            )

                        # Update metadata with recalculated hashes
                        metadata["hashes"] = {
                            "sha256": sha256,
                            "blake3": blake3,
                        }
                        self.save_metadata(model_dir, metadata)

                    except OSError as e:
                        results["errors"].append(f"Hash error for {model_id}: {e}")
                        logger.error("Failed to hash %s: %s", model_id, e)

            # Index the model
            try:
                self.index_model_dir(model_dir, metadata)
                results["indexed"] += 1
            except OSError as e:
                results["errors"].append(f"Index error for {model_id}: {e}")
                logger.error("Failed to index %s: %s", model_id, e)

        # Checkpoint WAL after batch rebuild
        self.index.checkpoint_wal()

        logger.info(
            "Deep scan complete: %d/%d indexed, %d errors, %d hash mismatches",
            results["indexed"],
            results["total_models"],
            len(results["errors"]),
            len(results["hash_mismatches"]),
        )

        return results

    def _find_primary_file(self, model_dir: Path) -> Path | None:
        """Find the primary model file in a directory.

        Returns the largest file with a recognized model extension.

        Args:
            model_dir: Model directory to search

        Returns:
            Path to primary file, or None if not found
        """
        candidates = [
            path
            for path in model_dir.rglob("*")
            if path.is_file()
            and path.suffix.lower() in {".gguf", ".safetensors", ".ckpt", ".pt", ".bin"}
        ]
        if not candidates:
            return None
        return max(candidates, key=lambda p: p.stat().st_size)

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
