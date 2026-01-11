"""Model import utilities for the model library."""

from __future__ import annotations

import re
import shutil
from pathlib import Path
from typing import Any, Optional, TypedDict

from backend.logging_config import get_logger
from backend.model_library.io.hashing import compute_dual_hash
from backend.model_library.io.manager import io_manager
from backend.model_library.library import ModelLibrary
from backend.model_library.model_identifier import identify_model_type
from backend.model_library.naming import normalize_filename, normalize_name, unique_path
from backend.models import ModelFileInfo, ModelMetadata, get_iso_timestamp
from backend.utils import ensure_directory

logger = get_logger(__name__)

# Prefix for temporary directories during atomic imports
_TEMP_PREFIX = ".tmp_import_"


# ============================================================================
# Type Definitions
# ============================================================================


class ShardValidation(TypedDict):
    """Result of shard completeness validation."""

    complete: bool
    missing_shards: list[int]
    total_expected: int
    total_found: int
    error: str


class FileTypeValidation(TypedDict):
    """Result of file type validation."""

    valid: bool
    detected_type: str  # 'safetensors', 'gguf', 'pickle', 'unknown'
    error: str


# ============================================================================
# Sharded Set Detection
# ============================================================================


def detect_sharded_sets(files: list[Path]) -> dict[str, list[Path]]:
    """Detect and group sharded model files.

    Common patterns:
    - model-00001-of-00005.safetensors, model-00002-of-00005.safetensors, ...
    - pytorch_model-00001-of-00003.bin, pytorch_model-00002-of-00003.bin, ...
    - model.safetensors.part1, model.safetensors.part2, ...

    Args:
        files: List of file paths to analyze

    Returns:
        Dict mapping base name to list of shard files
        Example: {'model.safetensors': [Path('model-00001-of-00005.safetensors'), ...]}
    """
    # Pattern 1: model-00001-of-00005.safetensors
    pattern1 = re.compile(r"^(.+)-(\d+)-of-(\d+)(\.[^.]+)$")

    # Pattern 2: model.safetensors.part1
    pattern2 = re.compile(r"^(.+\.[^.]+)\.part(\d+)$")

    # Pattern 3: model_00001.safetensors (no total count)
    pattern3 = re.compile(r"^(.+)_(\d{5})(\.[^.]+)$")

    sharded_groups: dict[str, list[Path]] = {}
    standalone_files: list[Path] = []

    for file_path in files:
        filename = file_path.name

        # Try pattern 1: model-00001-of-00005.ext
        match1 = pattern1.match(filename)
        if match1:
            base_name = match1.group(1)
            ext = match1.group(4)
            group_key = f"{base_name}{ext}"

            if group_key not in sharded_groups:
                sharded_groups[group_key] = []

            sharded_groups[group_key].append(file_path)
            continue

        # Try pattern 2: model.ext.part1
        match2 = pattern2.match(filename)
        if match2:
            base_name = match2.group(1)

            if base_name not in sharded_groups:
                sharded_groups[base_name] = []

            sharded_groups[base_name].append(file_path)
            continue

        # Try pattern 3: model_00001.ext
        match3 = pattern3.match(filename)
        if match3:
            base_name = match3.group(1)
            ext = match3.group(3)
            group_key = f"{base_name}{ext}"

            if group_key not in sharded_groups:
                sharded_groups[group_key] = []

            sharded_groups[group_key].append(file_path)
            continue

        # No pattern matched - standalone file
        standalone_files.append(file_path)

    # Filter out groups with only one file (false positives)
    filtered_groups: dict[str, list[Path]] = {}

    for key, files_list in sharded_groups.items():
        sorted_files = sorted(files_list, key=lambda p: p.name)
        if len(sorted_files) > 1:
            # True sharded set
            filtered_groups[key] = sorted_files
        else:
            # False positive - treat as standalone
            standalone_files.extend(sorted_files)

    # Add standalone files as single-item groups
    for file_path in standalone_files:
        filtered_groups[file_path.name] = [file_path]

    return filtered_groups


def validate_shard_completeness(
    shard_files: list[Path],
    expected_pattern: str = "sequential",
) -> ShardValidation:
    """Validate that a sharded set is complete.

    Args:
        shard_files: List of shard files in the group
        expected_pattern: "sequential" or "indexed"

    Returns:
        ShardValidation dict with completeness info
    """
    if not shard_files:
        return {
            "complete": False,
            "missing_shards": [],
            "total_expected": 0,
            "total_found": 0,
            "error": "",
        }

    # Extract shard indices from filenames
    pattern = re.compile(r"-(\d+)-of-(\d+)\.")

    indices: list[int] = []
    expected_total: int | None = None

    for file_path in shard_files:
        match = pattern.search(file_path.name)
        if match:
            current_idx = int(match.group(1))
            total_count = int(match.group(2))

            indices.append(current_idx)

            if expected_total is None:
                expected_total = total_count
            elif expected_total != total_count:
                # Inconsistent total counts
                return {
                    "complete": False,
                    "missing_shards": [],
                    "total_expected": expected_total,
                    "total_found": len(indices),
                    "error": "Inconsistent shard counts in filenames",
                }

    if expected_total is None:
        # Could not determine expected total
        return {
            "complete": True,
            "missing_shards": [],
            "total_expected": len(shard_files),
            "total_found": len(shard_files),
            "error": "",
        }

    # Check for missing shards
    expected_indices = set(range(1, expected_total + 1))
    found_indices = set(indices)
    missing_indices = sorted(expected_indices - found_indices)

    return {
        "complete": len(missing_indices) == 0,
        "missing_shards": missing_indices,
        "total_expected": expected_total,
        "total_found": len(found_indices),
        "error": "",
    }


# ============================================================================
# File Type Validation (Magic Bytes)
# ============================================================================


def validate_file_type(file_path: Path) -> FileTypeValidation:
    """Validate file type using magic bytes.

    Prevents importing .txt/.html files masquerading as models.

    Args:
        file_path: Path to the file to validate

    Returns:
        FileTypeValidation dict with type info
    """
    try:
        with open(file_path, "rb") as f:
            header = f.read(16)

        # Check for safetensors JSON header
        # Safetensors files start with a little-endian 64-bit length followed by JSON
        if len(header) >= 8:
            # Safetensors header: 8-byte length + JSON starting with '{'
            # But the JSON might not be at offset 8 if length is in header
            # More reliable: check if starts with small number followed by '{'
            try:
                # Read first 8 bytes as little-endian uint64
                import struct

                header_len = struct.unpack("<Q", header[:8])[0]
                # If header_len is reasonable (< 10MB) and next char could be JSON
                if header_len < 10 * 1024 * 1024:
                    # Read actual JSON header start
                    with open(file_path, "rb") as f:
                        f.seek(8)
                        json_start = f.read(1)
                        if json_start == b"{":
                            return {
                                "valid": True,
                                "detected_type": "safetensors",
                                "error": "",
                            }
            except struct.error:  # noqa: no-except-logging
                pass
            except OSError:  # noqa: no-except-logging
                pass

        # Check for GGUF magic number
        if header.startswith(b"GGUF"):
            return {"valid": True, "detected_type": "gguf", "error": ""}

        # Check for GGML magic (older format)
        if header.startswith(b"GGML"):
            return {"valid": True, "detected_type": "ggml", "error": ""}

        # Check for PyTorch pickle formats
        # Pickle protocol markers: 0x80 followed by protocol version (2-5)
        pickle_signatures = [
            b"\x80\x02",  # Protocol 2
            b"\x80\x03",  # Protocol 3
            b"\x80\x04",  # Protocol 4
            b"\x80\x05",  # Protocol 5
        ]
        for sig in pickle_signatures:
            if header.startswith(sig):
                return {"valid": True, "detected_type": "pickle", "error": ""}

        # Check for ZIP format (PyTorch .pt files are often ZIP archives)
        if header.startswith(b"PK\x03\x04"):
            return {"valid": True, "detected_type": "pickle", "error": ""}

        # Check for ONNX (Protocol Buffers format)
        # ONNX files start with protobuf field tags
        if header.startswith(b"\x08") and len(header) > 2:
            # Could be protobuf - check file extension
            if file_path.suffix.lower() == ".onnx":
                return {"valid": True, "detected_type": "onnx", "error": ""}

        # Unknown/invalid format
        return {
            "valid": False,
            "detected_type": "unknown",
            "error": f"Unrecognized file format. Header: {header[:8].hex()}",
        }

    except OSError as e:
        logger.warning("Failed to validate file type for %s: %s", file_path, e)
        return {"valid": False, "detected_type": "error", "error": str(e)}


# Extension-based fallback for when content detection fails
# Order matters: LLM first since GGUF is specifically for LLMs
_MODEL_EXTENSIONS = {
    "llm": {".gguf", ".bin"},  # GGUF was created for llama.cpp (LLMs)
    "checkpoints": {".ckpt", ".safetensors"},
    "loras": {".safetensors", ".pt"},
    "vae": {".pt", ".safetensors"},
    "controlnet": {".safetensors", ".pt"},
    "embeddings": {".pt"},
}


class ModelImporter:
    """Imports local models into the canonical library."""

    def __init__(self, library: ModelLibrary) -> None:
        self.library = library

    def _detect_type(self, file_path: Path) -> tuple[str, str, Optional[str]]:
        """Detect model type using content inspection with extension fallback.

        Args:
            file_path: Path to the model file

        Returns:
            Tuple of (model_type, subtype, detected_family)
            - model_type: "llm" or "diffusion"
            - subtype: "" for LLM, or "checkpoints"/"loras"/etc. for diffusion
            - detected_family: Family detected from file content, or None
        """
        # Try content-based detection first (reads GGUF/safetensors headers)
        detected_type, detected_family, extra = identify_model_type(file_path)

        if detected_type:
            subtype = "" if detected_type == "llm" else "checkpoints"
            logger.info(
                "Content-based detection: type=%s, family=%s for %s",
                detected_type,
                detected_family,
                file_path.name,
            )
            return detected_type, subtype, detected_family

        # Fall back to extension-based detection
        ext = file_path.suffix.lower()
        for subtype_key, extensions in _MODEL_EXTENSIONS.items():
            if ext in extensions:
                model_type = "llm" if subtype_key == "llm" else "diffusion"
                subtype_value = "" if model_type == "llm" else subtype_key
                logger.debug(
                    "Extension-based detection: type=%s, subtype=%s for %s",
                    model_type,
                    subtype_value,
                    file_path.name,
                )
                return model_type, subtype_value, None

        # Default to LLM for unknown extensions
        return "llm", "", None

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

    def _compute_hashes(self, file_path: Path) -> tuple[str, str]:
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
        model_type: Optional[str] = None,
        subtype: Optional[str] = None,
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
            model_type: Optional override for model type (from HF metadata)
            subtype: Optional override for model subtype (from HF metadata)

        Returns:
            Path to the imported model directory

        Raises:
            FileNotFoundError: If local_path doesn't exist
            OSError: If import operation fails
        """
        local_path = Path(local_path).resolve()
        if not local_path.exists():
            raise FileNotFoundError(f"Local path not found: {local_path}")

        # Determine model type using priority: provided > content detection > extension
        detected_family: Optional[str] = None

        if model_type:
            # Use provided model_type (from HuggingFace lookup)
            effective_model_type = model_type
            effective_subtype = subtype or ("" if model_type == "llm" else "checkpoints")
            logger.info(
                "Using provided model_type=%s, subtype=%s for %s",
                effective_model_type,
                effective_subtype,
                local_path.name,
            )
        elif local_path.is_file():
            effective_model_type, effective_subtype, detected_family = self._detect_type(local_path)
        else:
            files = [p for p in local_path.iterdir() if p.is_file()]
            if files:
                largest_file = max(files, key=lambda p: p.stat().st_size)
                effective_model_type, effective_subtype, detected_family = self._detect_type(
                    largest_file
                )
            else:
                effective_model_type, effective_subtype, detected_family = "llm", "", None

        # Use detected family if the provided family is generic
        effective_family = family
        if detected_family and family in ("imported", "unknown", ""):
            effective_family = detected_family
            logger.info("Using detected family '%s' instead of '%s'", detected_family, family)

        cleaned_name = normalize_name(official_name)
        final_model_dir = self.library.build_model_path(
            effective_model_type, effective_family, cleaned_name
        )
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
                "family": normalize_name(effective_family),
                "model_type": effective_model_type,
                "subtype": effective_subtype,
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
