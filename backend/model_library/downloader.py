"""Model downloading utilities for the model library."""

from __future__ import annotations

import hashlib
import re
import shutil
import threading
import time
from datetime import datetime, timedelta
from difflib import SequenceMatcher
from pathlib import Path
from typing import Any, Optional, TypedDict

from backend.logging_config import get_logger
from backend.model_library.hf.client import HfClient
from backend.model_library.hf.metadata import coerce_int
from backend.model_library.hf.quant import normalize_quant_source, token_in_normalized
from backend.model_library.hf.search import list_repo_tree_paths, search_models
from backend.model_library.library import ModelLibrary
from backend.model_library.naming import normalize_filename, normalize_name, unique_path
from backend.models import ModelFileInfo, ModelMetadata, get_iso_timestamp
from backend.utils import calculate_file_hash, ensure_directory

logger = get_logger(__name__)

# Cache for repo file lists (24 hours TTL)
_repo_cache: dict[str, tuple[list, datetime]] = {}
_repo_cache_ttl = timedelta(hours=24)


class HFMetadataResult(TypedDict, total=False):
    """Result of HuggingFace metadata lookup."""

    repo_id: str
    official_name: str
    family: str
    model_type: str
    subtype: str
    variant: str
    precision: str
    tags: list[str]
    base_model: str
    download_url: str
    description: str
    match_confidence: float
    match_method: str  # 'hash', 'filename_exact', 'filename_fuzzy'
    requires_confirmation: bool
    hash_mismatch: bool
    matched_filename: str
    pending_full_verification: bool
    fast_hash: str
    expected_sha256: str


def compute_fast_hash(file_path: Path) -> str:
    """Compute a fast hash using first and last 8MB of file.

    This provides a quick candidate filter without reading the entire file.
    For a 20GB file on HDD, this reads ~16MB instead of 20GB.

    Args:
        file_path: Path to the model file

    Returns:
        SHA256 hash of (first_8MB + last_8MB + file_size)
    """
    chunk_size = 8 * 1024 * 1024  # 8MB

    file_size = file_path.stat().st_size
    hasher = hashlib.sha256()

    with open(file_path, "rb") as f:
        # Read first 8MB
        first_chunk = f.read(chunk_size)
        hasher.update(first_chunk)

        # Read last 8MB (if file is larger than 16MB)
        if file_size > chunk_size * 2:
            f.seek(-chunk_size, 2)  # Seek to last 8MB
            last_chunk = f.read(chunk_size)
            hasher.update(last_chunk)
        elif file_size > chunk_size:
            # File is between 8-16MB, read remaining
            remaining = f.read()
            hasher.update(remaining)

        # Include file size to differentiate files with same head/tail
        hasher.update(str(file_size).encode())

    return hasher.hexdigest()


class ModelDownloader:
    """Downloads models from Hugging Face into the canonical library."""

    _DOWNLOAD_CACHE_DIRNAME = ".hf_cache"

    def __init__(self, library: ModelLibrary) -> None:
        self.library = library
        self._hf_client = HfClient()
        self._download_lock = threading.Lock()
        self._downloads: dict[str, dict[str, object]] = {}

    def _get_api(self):
        """Get the HuggingFace API instance."""
        return self._hf_client.get_api()

    def search_models(
        self,
        query: str,
        kind: Optional[str] = None,
        limit: int = 25,
    ) -> list[dict[str, object]]:
        """Search for models on HuggingFace Hub.

        Args:
            query: Search query string
            kind: Optional model kind/task filter
            limit: Maximum number of results

        Returns:
            List of model dictionaries with metadata
        """
        api = self._get_api()
        return search_models(api, query, kind, limit)

    def _download_allow_patterns(self, quant: Optional[str]) -> list[str] | None:
        """Get file patterns to allow for a specific quantization."""
        if not quant:
            return None
        quant_token = quant.strip()
        if not quant_token:
            return None
        return [
            f"*{quant_token}*",
            f"*{quant_token.upper()}*",
            "model_index.json",
            "config.json",
            "generation_config.json",
            "tokenizer.json",
            "tokenizer_config.json",
            "special_tokens_map.json",
            "vocab.*",
            "merges.*",
            "*.json",
            "*.yml",
            "*.yaml",
        ]

    def _matches_quant_path(self, path: str, quant: str) -> bool:
        """Check if a file path matches a quantization pattern."""
        normalized_path = normalize_quant_source(path)
        lower = path.lower()
        token = normalize_quant_source(quant)
        if token_in_normalized(normalized_path, token):
            return True
        config_files = {
            "model_index.json",
            "config.json",
            "generation_config.json",
            "tokenizer.json",
            "tokenizer_config.json",
            "special_tokens_map.json",
        }
        if any(lower.endswith(name) for name in config_files):
            return True
        if lower.endswith((".json", ".yml", ".yaml", ".vocab", ".merges")):
            return True
        return False

    def _compute_blake3(self, file_path: Path) -> str:
        """Compute BLAKE3 hash of a file."""
        try:
            import blake3
        except ImportError:
            logger.warning("blake3 not available; skipping BLAKE3 hash")
            return ""

        h = blake3.blake3()
        with file_path.open("rb") as f:
            for chunk in iter(lambda: f.read(8192 * 1024), b""):
                h.update(chunk)
        return h.hexdigest().lower()

    def _choose_primary_file(self, model_dir: Path) -> Optional[Path]:
        """Choose the primary model file from a directory."""
        candidates = [
            path
            for path in model_dir.rglob("*")
            if path.is_file()
            and path.suffix.lower() in {".gguf", ".safetensors", ".ckpt", ".pt", ".bin"}
        ]
        if not candidates:
            return None
        return max(candidates, key=lambda p: p.stat().st_size)

    def download_from_hf(
        self,
        repo_id: str,
        family: str,
        official_name: str,
        model_type: Optional[str] = None,
        subtype: str = "",
        quant: Optional[str] = None,
    ) -> Path:
        """Download a model from HuggingFace synchronously.

        Args:
            repo_id: HuggingFace repository ID
            family: Model family name
            official_name: Official model name
            model_type: Type of model (default: 'llm')
            subtype: Model subtype
            quant: Quantization to download

        Returns:
            Path to the downloaded model directory
        """
        try:
            from huggingface_hub import snapshot_download
        except ImportError as exc:
            raise RuntimeError("huggingface_hub is not installed") from exc

        import tempfile

        cleaned_name = normalize_name(official_name)
        effective_model_type = model_type or "llm"

        model_dir = self.library.build_model_path(effective_model_type, family, cleaned_name)
        model_dir = unique_path(model_dir)
        ensure_directory(model_dir)

        temp_root = self.library.library_root / ".downloads"
        ensure_directory(temp_root)

        with tempfile.TemporaryDirectory(dir=temp_root) as temp_dir:
            temp_path = Path(temp_dir)
            logger.info("Downloading %s to %s", repo_id, temp_path)
            allow_patterns = self._download_allow_patterns(quant)
            download_kwargs = {
                "repo_id": repo_id,
                "local_dir": temp_path,
                "local_dir_use_symlinks": False,
                "ignore_patterns": ["*.md", "*.txt", "*.gitattributes"],
            }
            if allow_patterns is not None:
                download_kwargs["allow_patterns"] = allow_patterns
            snapshot_download(  # type: ignore[call-overload]
                **download_kwargs,
            )

            file_infos: list[ModelFileInfo] = []
            total_size = 0

            for source_file in temp_path.rglob("*"):
                if not source_file.is_file():
                    continue
                if source_file.name.startswith("."):
                    continue

                cleaned_filename = normalize_filename(source_file.name)
                target_path = model_dir / cleaned_filename
                target_path = unique_path(target_path)
                shutil.move(str(source_file), str(target_path))

                size = target_path.stat().st_size
                total_size += size
                file_infos.append(
                    {
                        "name": target_path.name,
                        "original_name": source_file.name,
                        "size": size,
                    }
                )

        primary_file = self._choose_primary_file(model_dir)
        sha256 = calculate_file_hash(primary_file) if primary_file else ""
        blake3_hash = self._compute_blake3(primary_file) if primary_file else ""

        api = self._get_api()
        metadata = self._build_metadata(api, repo_id, model_dir, official_name)
        metadata.update(
            {
                "model_type": effective_model_type,
                "subtype": subtype,
                "hashes": {"sha256": sha256 or "", "blake3": blake3_hash or ""},
                "size_bytes": total_size,
                "files": file_infos,
            }
        )

        self.library.save_metadata(model_dir, metadata)
        self.library.save_overrides(model_dir, {})
        self.library.index_model_dir(model_dir, metadata)

        return model_dir

    def start_model_download(
        self,
        repo_id: str,
        family: str,
        official_name: str,
        model_type: Optional[str] = None,
        subtype: str = "",
        quant: Optional[str] = None,
    ) -> dict[str, object]:
        """Start an asynchronous model download.

        Args:
            repo_id: HuggingFace repository ID
            family: Model family name
            official_name: Official model name
            model_type: Type of model
            subtype: Model subtype
            quant: Quantization to download

        Returns:
            Dictionary with download_id and total_bytes
        """
        download_id = hashlib.sha1(
            f"{repo_id}:{quant or 'all'}:{time.time()}".encode("utf-8")
        ).hexdigest()
        temp_root = self.library.library_root / ".downloads"
        ensure_directory(temp_root)
        temp_dir = temp_root / download_id
        if temp_dir.exists():
            shutil.rmtree(temp_dir, ignore_errors=True)
        ensure_directory(temp_dir)

        total_bytes = self._calculate_total_bytes(repo_id, quant)
        cancel_event = threading.Event()

        with self._download_lock:
            self._downloads[download_id] = {
                "repo_id": repo_id,
                "family": family,
                "official_name": official_name,
                "model_type": model_type,
                "subtype": subtype,
                "quant": quant,
                "temp_dir": temp_dir,
                "total_bytes": total_bytes,
                "downloaded_bytes": 0,
                "status": "queued",
                "error": "",
                "cancel_event": cancel_event,
                "started_at": time.time(),
                "completed_at": None,
            }

        thread = threading.Thread(target=self._run_download, args=(download_id,), daemon=True)
        thread.start()

        return {"download_id": download_id, "total_bytes": total_bytes}

    def get_model_download_status(self, download_id: str) -> dict[str, object] | None:
        """Get the status of an ongoing download.

        Args:
            download_id: The download ID

        Returns:
            Status dictionary or None if not found
        """
        with self._download_lock:
            state = self._downloads.get(download_id)
            if not state:
                return None

        status = str(state.get("status", "unknown"))
        total_bytes = coerce_int(state.get("total_bytes"))
        downloaded_bytes = coerce_int(state.get("downloaded_bytes"))
        temp_dir = state.get("temp_dir")

        if status in {"queued", "downloading", "cancelling"} and isinstance(temp_dir, Path):
            downloaded_bytes = self._calculate_downloaded_bytes(temp_dir)
            with self._download_lock:
                state["downloaded_bytes"] = downloaded_bytes

        progress = 0.0
        if total_bytes > 0:
            progress = min(1.0, downloaded_bytes / total_bytes)
        elif status == "completed":
            progress = 1.0

        return {
            "download_id": download_id,
            "repo_id": state.get("repo_id"),
            "status": status,
            "progress": progress,
            "downloaded_bytes": downloaded_bytes,
            "total_bytes": total_bytes,
            "error": state.get("error") or "",
        }

    def cancel_model_download(self, download_id: str) -> bool:
        """Cancel an ongoing download.

        Args:
            download_id: The download ID to cancel

        Returns:
            True if cancelled, False if not found or already complete
        """
        with self._download_lock:
            state = self._downloads.get(download_id)
            if not state:
                return False
            status = state.get("status")
            if status in {"completed", "error", "cancelled"}:
                return False
            state["status"] = "cancelling"
            cancel_event = state.get("cancel_event")
            if isinstance(cancel_event, threading.Event):
                cancel_event.set()

        temp_dir = state.get("temp_dir")
        if isinstance(temp_dir, Path) and temp_dir.exists():
            shutil.rmtree(temp_dir, ignore_errors=True)
        model_dir = state.get("model_dir")
        if isinstance(model_dir, Path) and model_dir.exists():
            shutil.rmtree(model_dir, ignore_errors=True)
        return True

    def _calculate_downloaded_bytes(self, temp_dir: Path) -> int:
        """Calculate bytes downloaded so far."""
        total = 0
        try:
            for path in temp_dir.rglob("*"):
                if path.is_file():
                    if self._DOWNLOAD_CACHE_DIRNAME in path.parts:
                        continue
                    total += path.stat().st_size
        except OSError as exc:
            logger.debug("Failed to calculate downloaded bytes: %s", exc)
            return 0
        except RuntimeError as exc:
            logger.debug("Failed to calculate downloaded bytes: %s", exc)
            return 0
        return total

    def _calculate_total_bytes(self, repo_id: str, quant: Optional[str]) -> int:
        """Calculate total bytes to download."""
        api = self._get_api()
        paths_with_sizes = list_repo_tree_paths(api, repo_id)
        if not paths_with_sizes:
            return 0
        if not quant:
            return sum(size for _, size in paths_with_sizes)
        return sum(size for path, size in paths_with_sizes if self._matches_quant_path(path, quant))

    def _finalize_download(
        self,
        repo_id: str,
        family: str,
        official_name: str,
        model_type: Optional[str],
        subtype: str,
        temp_dir: Path,
    ) -> Path:
        """Finalize a download by moving files and creating metadata."""
        cleaned_name = normalize_name(official_name)
        effective_model_type = model_type or "llm"

        model_dir = self.library.build_model_path(effective_model_type, family, cleaned_name)
        model_dir = unique_path(model_dir)
        ensure_directory(model_dir)

        file_infos: list[ModelFileInfo] = []
        total_size = 0

        for source_file in temp_dir.rglob("*"):
            if not source_file.is_file():
                continue
            if self._DOWNLOAD_CACHE_DIRNAME in source_file.parts:
                continue
            if source_file.name.startswith("."):
                continue

            cleaned_filename = normalize_filename(source_file.name)
            target_path = model_dir / cleaned_filename
            target_path = unique_path(target_path)
            shutil.move(str(source_file), str(target_path))

            size = target_path.stat().st_size
            total_size += size
            file_infos.append(
                {
                    "name": target_path.name,
                    "original_name": source_file.name,
                    "size": size,
                }
            )

        primary_file = self._choose_primary_file(model_dir)
        sha256 = calculate_file_hash(primary_file) if primary_file else ""
        blake3_hash = self._compute_blake3(primary_file) if primary_file else ""

        api = self._get_api()
        metadata = self._build_metadata(api, repo_id, model_dir, official_name)
        metadata.update(
            {
                "model_type": effective_model_type,
                "subtype": subtype,
                "hashes": {"sha256": sha256 or "", "blake3": blake3_hash or ""},
                "size_bytes": total_size,
                "files": file_infos,
            }
        )

        self.library.save_metadata(model_dir, metadata)
        self.library.save_overrides(model_dir, {})
        self.library.index_model_dir(model_dir, metadata)

        return model_dir

    def _download_files_with_cancel(
        self,
        repo_id: str,
        temp_dir: Path,
        quant: Optional[str],
        cancel_event: threading.Event,
    ) -> None:
        """Download files with cancellation support."""
        api = self._get_api()
        try:
            items = api.list_repo_tree(repo_id=repo_id, repo_type="model", recursive=True)
        except OSError as exc:
            raise RuntimeError("Failed to list model files.") from exc
        except RuntimeError as exc:
            raise RuntimeError("Failed to list model files.") from exc
        except ValueError as exc:
            raise RuntimeError("Failed to list model files.") from exc

        file_paths: list[str] = []
        for item in items:
            path = getattr(item, "path", "") or getattr(item, "rfilename", "") or ""
            if not path:
                continue
            item_type = getattr(item, "type", "") or ""
            if item_type in {"directory", "dir", "folder"} or path.endswith("/"):
                continue
            lower = path.lower()
            if lower.endswith((".md", ".txt", ".gitattributes")):
                continue
            if quant and not self._matches_quant_path(path, quant):
                continue
            file_paths.append(path)

        if not file_paths:
            raise RuntimeError("No files available to download.")

        try:
            from huggingface_hub import hf_hub_download
        except ImportError as exc:
            raise RuntimeError("huggingface_hub is not installed") from exc

        cache_dir = temp_dir / self._DOWNLOAD_CACHE_DIRNAME
        ensure_directory(cache_dir)

        for path in file_paths:
            if cancel_event.is_set():
                return
            hf_hub_download(
                repo_id=repo_id,
                filename=path,
                local_dir=temp_dir,
                cache_dir=cache_dir,
            )

    def _run_download(self, download_id: str) -> None:
        """Run the download in a background thread."""
        with self._download_lock:
            state = self._downloads.get(download_id)
            if not state:
                return
            state["status"] = "downloading"
            cancel_event = state.get("cancel_event")
            temp_dir = state.get("temp_dir")

        if not isinstance(cancel_event, threading.Event) or not isinstance(temp_dir, Path):
            return

        if cancel_event.is_set():
            with self._download_lock:
                state["status"] = "cancelled"
                state["completed_at"] = time.time()
            return

        repo_id = state.get("repo_id")
        family = state.get("family")
        official_name = state.get("official_name")
        if (
            not isinstance(repo_id, str)
            or not isinstance(family, str)
            or not isinstance(official_name, str)
        ):
            with self._download_lock:
                state["status"] = "error"
                state["error"] = "Invalid download metadata."
                state["completed_at"] = time.time()
            return

        def _set_error_state(exc: Exception) -> None:
            if cancel_event.is_set():
                status = "cancelled"
                error_msg = ""
            else:
                status = "error"
                error_msg = str(exc)
            with self._download_lock:
                state["status"] = status
                state["error"] = error_msg
                state["completed_at"] = time.time()
            shutil.rmtree(temp_dir, ignore_errors=True)

        try:
            quant_value = state.get("quant")
            quant = quant_value if isinstance(quant_value, str) else None
            self._download_files_with_cancel(
                repo_id=repo_id,
                temp_dir=temp_dir,
                quant=quant,
                cancel_event=cancel_event,
            )
        except OSError as exc:
            logger.error("Download failed for %s: %s", repo_id, exc)
            _set_error_state(exc)
            return
        except RuntimeError as exc:
            logger.error("Download failed for %s: %s", repo_id, exc)
            _set_error_state(exc)
            return
        except ValueError as exc:
            logger.error("Download failed for %s: %s", repo_id, exc)
            _set_error_state(exc)
            return

        if cancel_event.is_set():
            with self._download_lock:
                state["status"] = "cancelled"
                state["completed_at"] = time.time()
            shutil.rmtree(temp_dir, ignore_errors=True)
            return

        try:
            model_type_value = state.get("model_type")
            model_type = model_type_value if isinstance(model_type_value, str) else None
            subtype_value = state.get("subtype")
            subtype = subtype_value if isinstance(subtype_value, str) else ""
            model_dir = self._finalize_download(
                repo_id=repo_id,
                family=family,
                official_name=official_name,
                model_type=model_type,
                subtype=subtype,
                temp_dir=temp_dir,
            )
            with self._download_lock:
                state["status"] = "completed"
                state["completed_at"] = time.time()
                state["model_dir"] = model_dir
                state["downloaded_bytes"] = state.get("total_bytes") or 0
            shutil.rmtree(temp_dir, ignore_errors=True)
        except OSError as exc:
            logger.error("Finalize download failed for %s: %s", repo_id, exc)
            with self._download_lock:
                state["status"] = "error"
                state["error"] = str(exc)
                state["completed_at"] = time.time()
            shutil.rmtree(temp_dir, ignore_errors=True)
        except RuntimeError as exc:
            logger.error("Finalize download failed for %s: %s", repo_id, exc)
            with self._download_lock:
                state["status"] = "error"
                state["error"] = str(exc)
                state["completed_at"] = time.time()
            shutil.rmtree(temp_dir, ignore_errors=True)
        except ValueError as exc:
            logger.error("Finalize download failed for %s: %s", repo_id, exc)
            with self._download_lock:
                state["status"] = "error"
                state["error"] = str(exc)
                state["completed_at"] = time.time()
            shutil.rmtree(temp_dir, ignore_errors=True)

    def _build_metadata(
        self,
        api,
        repo_id: str,
        model_dir: Path,
        official_name: str,
    ) -> ModelMetadata:
        """Build metadata for a downloaded model."""
        now = get_iso_timestamp()
        metadata: ModelMetadata = {
            "model_id": model_dir.name,
            "family": model_dir.parent.name,
            "model_type": "",
            "subtype": "",
            "official_name": official_name,
            "cleaned_name": model_dir.name,
            "tags": [],
            "base_model": "",
            "preview_image": "",
            "release_date": "",
            "download_url": f"https://huggingface.co/{repo_id}",
            "model_card": {},
            "inference_settings": {},
            "compatible_apps": [],
            "hashes": {"sha256": "", "blake3": ""},
            "notes": "",
            "added_date": now,
            "updated_date": now,
            "size_bytes": 0,
            "files": [],
        }

        try:
            info = api.model_info(repo_id)
            metadata["release_date"] = info.last_modified.isoformat() if info.last_modified else ""
            metadata["tags"] = list(info.tags or [])
            if info.card_data:
                metadata["model_card"] = info.card_data.to_dict()
                metadata["base_model"] = info.card_data.get("base_model", "")

            for sibling in info.siblings:
                if sibling.rfilename.lower().endswith((".png", ".jpg", ".jpeg")):
                    try:
                        from huggingface_hub import hf_hub_download
                    except ImportError as exc:
                        logger.debug("huggingface_hub download unavailable: %s", exc)
                        break
                    hf_hub_download(
                        repo_id=repo_id, filename=sibling.rfilename, local_dir=model_dir
                    )
                    preview_path = model_dir / sibling.rfilename
                    target_preview = model_dir / "preview.png"
                    try:
                        preview_path.rename(target_preview)
                        metadata["preview_image"] = "preview.png"
                    except OSError as exc:
                        logger.debug("Failed to set preview image for %s: %s", repo_id, exc)
                    break
        except OSError as exc:
            logger.warning("Failed to enrich metadata for %s: %s", repo_id, exc)
        except RuntimeError as exc:
            logger.warning("Failed to enrich metadata for %s: %s", repo_id, exc)
        except ValueError as exc:
            logger.warning("Failed to enrich metadata for %s: %s", repo_id, exc)

        return metadata

    # =========================================================================
    # HuggingFace Metadata Lookup Methods
    # =========================================================================

    def lookup_model_metadata_by_filename(
        self,
        filename: str,
        file_path: Optional[Path] = None,
        timeout: float = 5.0,
    ) -> Optional[HFMetadataResult]:
        """Lookup HuggingFace metadata using hybrid filename + hash verification.

        Strategy:
        1. Filename-based search to find top candidates (3-5 repos)
        2. Compute fast hash if file_path provided (first 8MB + last 8MB)
        3. Verify hash against top 2 candidates
        4. Return best match with confidence score

        Args:
            filename: Name of the model file
            file_path: Optional path to local file for hash verification
            timeout: Timeout for API calls (default: 5 seconds)

        Returns:
            HFMetadataResult with match details, or None if no match found
        """
        logger.info("Looking up metadata for: %s", filename)

        # Step 1: Get candidate repos via filename search
        candidates = self._get_candidate_repos(filename, limit=5)

        if not candidates:
            logger.info("No candidates found for %s", filename)
            return None

        # Step 2: Compute fast hash for candidate filtering
        fast_hash = None
        if file_path and file_path.exists():
            try:
                fast_hash = compute_fast_hash(file_path)
                logger.debug("Fast hash computed: %s...", fast_hash[:16])
            except OSError as exc:
                logger.warning("Failed to compute fast hash: %s", exc)

        # Step 3: Sort candidates by popularity (download count)
        candidates_sorted = sorted(
            candidates,
            key=lambda r: getattr(r, "downloads", 0) or 0,
            reverse=True,
        )

        # Step 4: Try hash verification on top 2 candidates (if file provided)
        if file_path and file_path.exists():
            for candidate in candidates_sorted[:2]:
                hash_match = self._verify_hash_single_candidate(file_path, candidate, filename)
                if hash_match:
                    logger.info("Hash match found: %s", candidate.id)
                    hash_match["fast_hash"] = fast_hash or ""
                    return hash_match

        # Step 5: Fall back to filename matching with confidence
        best_match = self._find_best_filename_match(filename, candidates_sorted)

        if best_match:
            logger.info(
                "Filename match: %s (confidence: %.2f)",
                best_match.get("repo_id", ""),
                best_match.get("match_confidence", 0.0),
            )
            best_match["pending_full_verification"] = True
            best_match["fast_hash"] = fast_hash or ""
            return best_match

        return None

    def _get_candidate_repos(self, filename: str, limit: int = 5) -> list:
        """Get candidate repositories from HuggingFace based on filename.

        Args:
            filename: Model filename to search for
            limit: Maximum number of candidates to return

        Returns:
            List of HuggingFace model info objects
        """
        base_name = self._extract_base_name(filename)

        try:
            api = self._get_api()
            models = list(api.list_models(search=base_name, limit=limit))
            return models
        except OSError as exc:
            logger.warning("Failed to search HuggingFace: %s", exc)
            return []
        except RuntimeError as exc:
            logger.warning("Failed to search HuggingFace: %s", exc)
            return []

    def _extract_base_name(self, filename: str) -> str:
        """Extract base model name from filename by removing quant/version suffixes.

        Examples:
            'llama-3-8b-Q4_K_M.gguf' -> 'llama-3-8b'
            'stable-diffusion-v1-5-pruned-emaonly.safetensors' -> 'stable-diffusion-v1-5'
        """
        # Remove extension
        name = Path(filename).stem

        # Common patterns to remove
        patterns = [
            r"[-_]Q[0-9]+_K[_SM]*$",  # GGUF quantization (Q4_K_M, Q5_K_S, etc.)
            r"[-_]q[0-9]+_k[_sm]*$",  # lowercase variant
            r"[-_]fp16$",
            r"[-_]fp32$",
            r"[-_]bf16$",
            r"[-_]int[48]$",
            r"[-_]pruned[-_]?.*$",
            r"[-_]ema[-_]?.*$",
            r"[-_]v\d+(\.\d+)*$",  # Version numbers like -v1.5
            r"[-_]\d+[bB]$",  # Parameter counts like -8B
        ]

        for pattern in patterns:
            name = re.sub(pattern, "", name, flags=re.IGNORECASE)

        return name.strip("-_")

    def _verify_hash_single_candidate(
        self,
        file_path: Path,
        candidate_repo: Any,
        filename: str,
    ) -> Optional[HFMetadataResult]:
        """Verify local file hash against single candidate repo.

        Returns metadata dict with match_confidence=1.0 if found, else None.
        """
        # Compute local file hash (SHA256 for LFS compatibility)
        try:
            local_hash = calculate_file_hash(file_path)
            if local_hash is None:
                logger.warning("Failed to compute file hash for %s", file_path)
                return None
            logger.debug("Local file SHA256: %s...", local_hash[:16])
        except OSError as exc:
            logger.warning("Failed to compute file hash: %s", exc)
            return None

        repo_id = candidate_repo.id

        try:
            # Fetch LFS files with hashes
            lfs_files = self._get_lfs_files_cached(repo_id)

            # Look for hash match
            for lfs_file in lfs_files:
                file_oid = getattr(lfs_file, "oid", None) or getattr(lfs_file, "lfs", {}).get(
                    "oid", ""
                )
                if not file_oid:
                    continue

                # SHA256 OID may be prefixed with "sha256:"
                file_hash = str(file_oid).replace("sha256:", "").lower()

                if file_hash == local_hash.lower():
                    logger.info(
                        "Hash match! %s / %s (SHA256: %s...)",
                        repo_id,
                        getattr(lfs_file, "path", ""),
                        file_hash[:16],
                    )

                    # Extract full metadata with 100% confidence
                    metadata = self._extract_metadata_from_repo(candidate_repo)
                    metadata["match_confidence"] = 1.0
                    metadata["match_method"] = "hash"
                    metadata["matched_filename"] = getattr(lfs_file, "path", "")
                    metadata["requires_confirmation"] = False
                    metadata["hash_mismatch"] = False
                    metadata["expected_sha256"] = file_hash

                    return metadata

        except OSError as exc:
            logger.warning("Could not fetch LFS files for %s: %s", repo_id, exc)
        except RuntimeError as exc:
            logger.warning("Could not fetch LFS files for %s: %s", repo_id, exc)

        return None

    def _get_lfs_files_cached(self, repo_id: str) -> list:
        """Get LFS files for a repo with 24-hour caching.

        This dramatically reduces API calls when importing multiple
        files from the same repository.
        """
        global _repo_cache

        now = datetime.now()

        # Check cache
        if repo_id in _repo_cache:
            cached_files, cached_time = _repo_cache[repo_id]
            if now - cached_time < _repo_cache_ttl:
                logger.debug("Cache hit for %s (age: %s)", repo_id, now - cached_time)
                return cached_files

        # Cache miss or expired - fetch from API
        api = self._get_api()
        try:
            # Use list_repo_tree to get file info including LFS data
            items = list(api.list_repo_tree(repo_id=repo_id, repo_type="model", recursive=True))
            lfs_files = [item for item in items if hasattr(item, "lfs") or hasattr(item, "oid")]

            # Store in cache
            _repo_cache[repo_id] = (lfs_files, now)
            logger.debug("Cached LFS files for %s (%d files)", repo_id, len(lfs_files))

            return lfs_files
        except OSError as exc:
            logger.warning("Failed to list repo tree for %s: %s", repo_id, exc)
            return []
        except RuntimeError as exc:
            logger.warning("Failed to list repo tree for %s: %s", repo_id, exc)
            return []

    def _find_best_filename_match(
        self,
        filename: str,
        candidates: list,
    ) -> Optional[HFMetadataResult]:
        """Find best candidate based on filename similarity.

        Returns metadata with match_confidence < 1.0 and requires_confirmation flag.
        """
        base_name = self._extract_base_name(filename).lower()
        best_match = None
        best_score = 0.0

        for repo in candidates:
            repo_name = repo.id.lower()
            # Compare against repo name (owner/model format)
            model_name = repo_name.split("/")[-1] if "/" in repo_name else repo_name
            score = SequenceMatcher(None, base_name, model_name).ratio()

            if score > best_score:
                best_score = score
                best_match = repo

        if not best_match:
            return None

        # Extract metadata
        metadata = self._extract_metadata_from_repo(best_match)

        # Determine match method based on score
        if best_score > 0.9:
            match_method = "filename_exact"
        else:
            match_method = "filename_fuzzy"

        metadata["match_confidence"] = best_score
        metadata["match_method"] = match_method
        metadata["requires_confirmation"] = best_score < 0.6
        metadata["hash_mismatch"] = False

        return metadata

    def _extract_metadata_from_repo(self, repo: Any) -> HFMetadataResult:
        """Extract metadata from a HuggingFace model info object."""
        repo_id = repo.id
        model_name = repo_id.split("/")[-1] if "/" in repo_id else repo_id

        # Infer family from repo name
        family = self._infer_family(model_name)

        # Infer variant and precision from model info
        variant, precision = self._infer_variant_and_precision(model_name)

        # Get tags
        tags = list(getattr(repo, "tags", []) or [])

        # Determine model type from pipeline tag
        pipeline_tag = getattr(repo, "pipeline_tag", "") or ""
        if pipeline_tag in ("text-generation", "conversational"):
            model_type = "llm"
        elif pipeline_tag in ("text-to-image", "image-to-image"):
            model_type = "diffusion"
        else:
            model_type = "llm"  # Default

        result: HFMetadataResult = {
            "repo_id": repo_id,
            "official_name": getattr(repo, "id", model_name).split("/")[-1],
            "family": family,
            "model_type": model_type,
            "subtype": "",
            "variant": variant,
            "precision": precision,
            "tags": tags,
            "base_model": "",
            "download_url": f"https://huggingface.co/{repo_id}",
            "description": getattr(repo, "description", "") or "",
            "match_confidence": 0.0,
            "match_method": "",
            "requires_confirmation": True,
            "hash_mismatch": False,
            "matched_filename": "",
            "pending_full_verification": False,
            "fast_hash": "",
            "expected_sha256": "",
        }

        return result

    def _infer_family(self, model_name: str) -> str:
        """Infer model family from model name."""
        name_lower = model_name.lower()

        # Common family patterns
        families = [
            ("llama", "llama"),
            ("mistral", "mistral"),
            ("gemma", "gemma"),
            ("phi", "phi"),
            ("qwen", "qwen"),
            ("yi", "yi"),
            ("falcon", "falcon"),
            ("mpt", "mpt"),
            ("stable-diffusion", "stable-diffusion"),
            ("sdxl", "sdxl"),
            ("flux", "flux"),
            ("sd", "stable-diffusion"),
        ]

        for pattern, family in families:
            if pattern in name_lower:
                return family

        return "unknown"

    def _infer_variant_and_precision(self, filename: str) -> tuple[str, str]:
        """Infer model variant and precision from filename.

        Returns:
            (variant, precision) tuple
        """
        filename_lower = filename.lower()

        # Variant detection
        if "ema" in filename_lower:
            variant = "ema"
        elif "pruned" in filename_lower:
            variant = "pruned"
        elif "full" in filename_lower:
            variant = "full"
        elif filename.endswith(".safetensors"):
            variant = "safetensors"
        else:
            variant = "standard"

        # Precision detection
        if "fp16" in filename_lower or "half" in filename_lower:
            precision = "fp16"
        elif "bf16" in filename_lower:
            precision = "bf16"
        elif "fp32" in filename_lower or "float32" in filename_lower:
            precision = "fp32"
        elif "int8" in filename_lower or "8bit" in filename_lower:
            precision = "int8"
        elif "int4" in filename_lower or "4bit" in filename_lower:
            precision = "int4"
        else:
            precision = "unknown"

        return variant, precision
