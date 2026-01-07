"""Model downloading utilities for the model library."""

from __future__ import annotations

import os
import shutil
import tempfile
from pathlib import Path
from typing import Iterable, Optional

from backend.logging_config import get_logger
from backend.model_library.library import ModelLibrary
from backend.model_library.naming import normalize_filename, normalize_name, unique_path
from backend.models import ModelFileInfo, ModelMetadata, get_iso_timestamp
from backend.utils import calculate_file_hash, ensure_directory

logger = get_logger(__name__)


class ModelDownloader:
    """Downloads models from Hugging Face into the canonical library."""

    def __init__(self, library: ModelLibrary) -> None:
        self.library = library
        self.hf_token = os.getenv("HF_TOKEN")
        self._api = None

    def _get_api(self):
        if self._api:
            return self._api
        try:
            from huggingface_hub import HfApi, login
        except ImportError as exc:
            raise RuntimeError("huggingface_hub is not installed") from exc

        if self.hf_token:
            login(self.hf_token)
        self._api = HfApi()
        return self._api

    def _extract_formats(self, siblings: Iterable[object]) -> list[str]:
        formats = set()
        known_formats = {
            ".safetensors",
            ".gguf",
            ".ckpt",
            ".pt",
            ".pth",
            ".bin",
            ".onnx",
        }
        for sibling in siblings:
            filename = getattr(sibling, "rfilename", "") or ""
            lower = filename.lower()
            for ext in known_formats:
                if lower.endswith(ext):
                    formats.add(ext.lstrip("."))
        return sorted(formats)

    def _extract_quants(self, siblings: Iterable[object], tags: Iterable[str]) -> list[str]:
        quants = set()
        quant_tokens = [
            "q2",
            "q3",
            "q4",
            "q5",
            "q6",
            "q8",
            "q4_k_m",
            "q4_k_s",
            "q5_k_m",
            "q5_k_s",
            "q6_k",
            "q8_0",
            "int4",
            "int8",
            "fp16",
            "fp32",
            "bf16",
            "f16",
            "f32",
        ]
        for sibling in siblings:
            filename = (getattr(sibling, "rfilename", "") or "").lower()
            for token in quant_tokens:
                if token in filename:
                    quants.add(token)
        for tag in tags:
            lower = tag.lower()
            for token in quant_tokens:
                if token in lower:
                    quants.add(token)
        return sorted(quants)

    def search_models(
        self,
        query: str,
        kind: Optional[str] = None,
        limit: int = 25,
    ) -> list[dict[str, object]]:
        api = self._get_api()
        filter_arg = None
        if kind:
            try:
                from huggingface_hub import ModelFilter
            except ImportError:
                filter_arg = kind
            else:
                filter_arg = ModelFilter(task=kind)

        results = api.list_models(
            search=query or None,
            filter=filter_arg,
            full=True,
            limit=limit,
        )

        models: list[dict[str, object]] = []
        for info in results:
            repo_id = getattr(info, "modelId", None) or getattr(info, "id", None)
            if not repo_id:
                continue
            tags = list(getattr(info, "tags", []) or [])
            siblings = getattr(info, "siblings", []) or []
            developer = getattr(info, "author", None) or repo_id.split("/")[0]
            kind_value = getattr(info, "pipeline_tag", None) or "unknown"
            models.append(
                {
                    "repoId": repo_id,
                    "name": repo_id.split("/")[-1],
                    "developer": developer,
                    "kind": kind_value,
                    "formats": self._extract_formats(siblings),
                    "quants": self._extract_quants(siblings, tags),
                    "url": f"https://huggingface.co/{repo_id}",
                }
            )
        return models

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

    def download_from_hf(
        self,
        repo_id: str,
        family: str,
        official_name: str,
        model_type: Optional[str] = None,
        subtype: str = "",
    ) -> Path:
        try:
            from huggingface_hub import snapshot_download
        except ImportError as exc:
            raise RuntimeError("huggingface_hub is not installed") from exc

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
            snapshot_download(  # type: ignore[call-overload]
                repo_id=repo_id,
                local_dir=temp_path,
                local_dir_use_symlinks=False,
                ignore_patterns=["*.md", "*.txt", "*.gitattributes"],
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

    def _build_metadata(
        self,
        api,
        repo_id: str,
        model_dir: Path,
        official_name: str,
    ) -> ModelMetadata:
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
                    except ImportError:
                        break
                    hf_hub_download(
                        repo_id=repo_id, filename=sibling.rfilename, local_dir=model_dir
                    )
                    preview_path = model_dir / sibling.rfilename
                    target_preview = model_dir / "preview.png"
                    try:
                        preview_path.rename(target_preview)
                        metadata["preview_image"] = "preview.png"
                    except OSError:
                        pass
                    break
        except (OSError, RuntimeError, ValueError) as exc:
            logger.warning("Failed to enrich metadata for %s: %s", repo_id, exc)

        return metadata
