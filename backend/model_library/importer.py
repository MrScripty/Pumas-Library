"""Model import utilities for the model library."""

from __future__ import annotations

import shutil
from pathlib import Path
from typing import Optional, Tuple

from backend.logging_config import get_logger
from backend.model_library.io.hashing import compute_dual_hash
from backend.model_library.io.manager import io_manager
from backend.model_library.library import ModelLibrary
from backend.model_library.naming import normalize_filename, normalize_name, unique_path
from backend.models import ModelFileInfo, ModelMetadata, get_iso_timestamp
from backend.utils import ensure_directory

logger = get_logger(__name__)

# Prefix for temporary directories during atomic imports
_TEMP_PREFIX = ".tmp_import_"

_MODEL_EXTENSIONS = {
    "checkpoints": {".ckpt", ".safetensors", ".gguf"},
    "loras": {".safetensors", ".pt"},
    "vae": {".pt", ".safetensors"},
    "controlnet": {".safetensors", ".pt", ".gguf"},
    "embeddings": {".pt"},
    "llm": {".gguf", ".bin", ".json", ".pt"},
}


class ModelImporter:
    """Imports local models into the canonical library."""

    def __init__(self, library: ModelLibrary) -> None:
        self.library = library

    def _detect_type(self, file_path: Path) -> Tuple[str, str]:
        ext = file_path.suffix.lower()
        for subtype, extensions in _MODEL_EXTENSIONS.items():
            if ext in extensions:
                model_type = "llm" if subtype == "llm" else "diffusion"
                subtype_value = "" if model_type == "llm" else subtype
                return model_type, subtype_value
        return "llm", ""

    def _choose_primary_file(self, model_dir: Path) -> Optional[Path]:
        candidates = [
            path
            for path in model_dir.rglob("*")
            if path.is_file()
            and path.suffix.lower() in {".gguf", ".safetensors", ".ckpt", ".pt", ".bin"}
        ]
        if not candidates:
            return None
        return max(candidates, key=lambda p: p.stat().st_size)

    def _compute_hashes(self, file_path: Path) -> Tuple[str, str]:
        """Compute SHA256 and BLAKE3 hashes in a single file read.

        Uses io/hashing.compute_dual_hash for efficient streaming hash
        computation without reading the file multiple times.

        Args:
            file_path: Path to the file to hash

        Returns:
            Tuple of (sha256_hex, blake3_hex)
        """
        return compute_dual_hash(file_path)

    def import_path(
        self,
        local_path: Path,
        family: str,
        official_name: str,
        repo_id: Optional[str] = None,
    ) -> Path:
        """Import a local model file or directory into the library.

        Uses atomic imports: files are first copied to a temporary directory
        with a .tmp prefix, then renamed to the final location on success.
        This prevents partial imports from appearing in the library.

        Args:
            local_path: Path to model file or directory to import
            family: Model family name (e.g., "llama", "stable-diffusion")
            official_name: Official model name
            repo_id: Optional HuggingFace repo ID for download URL

        Returns:
            Path to the imported model directory

        Raises:
            FileNotFoundError: If local_path doesn't exist
            OSError: If import operation fails
        """
        local_path = Path(local_path).resolve()
        if not local_path.exists():
            raise FileNotFoundError(f"Local path not found: {local_path}")

        if local_path.is_file():
            model_type, subtype = self._detect_type(local_path)
        else:
            files = [p for p in local_path.iterdir() if p.is_file()]
            if files:
                model_type, subtype = self._detect_type(max(files, key=lambda p: p.stat().st_size))
            else:
                model_type, subtype = "llm", ""

        cleaned_name = normalize_name(official_name)
        final_model_dir = self.library.build_model_path(model_type, family, cleaned_name)
        final_model_dir = unique_path(final_model_dir)

        # Use a temporary directory with .tmp prefix for atomic import
        temp_dir_name = f"{_TEMP_PREFIX}{final_model_dir.name}"
        temp_model_dir = final_model_dir.parent / temp_dir_name
        ensure_directory(temp_model_dir)

        try:
            file_infos: list[ModelFileInfo] = []
            total_size = 0

            if local_path.is_file():
                source_files = [local_path]
            else:
                source_files = [p for p in local_path.iterdir() if p.is_file()]

            # Copy/move files to temp directory
            for source_file in source_files:
                cleaned_filename = normalize_filename(source_file.name)
                target_path = temp_model_dir / cleaned_filename
                target_path = unique_path(target_path)

                # Use io_manager for drive-aware I/O
                with io_manager.io_slot(target_path):
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

            primary_file = self._choose_primary_file(temp_model_dir)
            if primary_file:
                sha256, blake3_hash = self._compute_hashes(primary_file)
            else:
                sha256, blake3_hash = "", ""

            now = get_iso_timestamp()
            metadata: ModelMetadata = {
                "model_id": final_model_dir.name,
                "family": normalize_name(family),
                "model_type": model_type,
                "subtype": subtype,
                "official_name": official_name,
                "cleaned_name": final_model_dir.name,
                "tags": [],
                "base_model": "",
                "preview_image": "",
                "release_date": "",
                "download_url": "" if not repo_id else f"https://huggingface.co/{repo_id}",
                "model_card": {},
                "inference_settings": {},
                "compatible_apps": [],
                "hashes": {"sha256": sha256 or "", "blake3": blake3_hash or ""},
                "notes": "",
                "added_date": now,
                "updated_date": now,
                "size_bytes": total_size,
                "files": file_infos,
            }

            # Save metadata to temp directory first
            self.library.save_metadata(temp_model_dir, metadata)
            self.library.save_overrides(temp_model_dir, {})

            # Atomic rename: move temp dir to final location
            temp_model_dir.rename(final_model_dir)
            logger.info("Atomically renamed %s to %s", temp_model_dir, final_model_dir)

            # Index the model at its final location
            self.library.index_model_dir(final_model_dir, metadata)

        except OSError as exc:  # noqa: multi-exception
            # Clean up temp directory on failure (IOError is an alias of OSError)
            if temp_model_dir.exists():
                shutil.rmtree(temp_model_dir, ignore_errors=True)
            logger.error("Import failed, cleaned up temp directory: %s", exc)
            raise

        if local_path.is_dir():
            try:
                if not any(local_path.iterdir()):
                    local_path.rmdir()
            except OSError as exc:
                logger.debug("Failed to remove empty import directory %s: %s", local_path, exc)

        return final_model_dir
