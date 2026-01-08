"""Model import utilities for the model library."""

from __future__ import annotations

import shutil
from pathlib import Path
from typing import Optional, Tuple

from backend.logging_config import get_logger
from backend.model_library.library import ModelLibrary
from backend.model_library.naming import normalize_filename, normalize_name, unique_path
from backend.models import ModelFileInfo, ModelMetadata, get_iso_timestamp
from backend.utils import calculate_file_hash, ensure_directory

logger = get_logger(__name__)

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

    def _compute_blake3(self, file_path: Path) -> str:
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

    def import_path(
        self,
        local_path: Path,
        family: str,
        official_name: str,
        repo_id: Optional[str] = None,
    ) -> Path:
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
        model_dir = self.library.build_model_path(model_type, family, cleaned_name)
        model_dir = unique_path(model_dir)
        ensure_directory(model_dir)

        file_infos: list[ModelFileInfo] = []
        total_size = 0

        if local_path.is_file():
            source_files = [local_path]
        else:
            source_files = [p for p in local_path.iterdir() if p.is_file()]

        for source_file in source_files:
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

        now = get_iso_timestamp()
        metadata: ModelMetadata = {
            "model_id": model_dir.name,
            "family": normalize_name(family),
            "model_type": model_type,
            "subtype": subtype,
            "official_name": official_name,
            "cleaned_name": model_dir.name,
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

        self.library.save_metadata(model_dir, metadata)
        self.library.save_overrides(model_dir, {})
        self.library.index_model_dir(model_dir, metadata)

        if local_path.is_dir():
            try:
                if not any(local_path.iterdir()):
                    local_path.rmdir()
            except OSError as exc:
                logger.debug("Failed to remove empty import directory %s: %s", local_path, exc)

        return model_dir
