"""Model downloading utilities for the model library."""

from __future__ import annotations

import hashlib
import os
import re
import shutil
import tempfile
import threading
import time
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

    _DOWNLOAD_CACHE_DIRNAME = ".hf_cache"

    def __init__(self, library: ModelLibrary) -> None:
        self.library = library
        self.hf_token = os.getenv("HF_TOKEN")
        self._api = None
        self._download_lock = threading.Lock()
        self._downloads: dict[str, dict[str, object]] = {}

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

    def _known_formats(self) -> set[str]:
        return {
            "safetensors",
            "gguf",
            "ckpt",
            "pt",
            "pth",
            "bin",
            "onnx",
            "tflite",
            "mlmodel",
            "pb",
        }

    def _quant_tokens(self) -> list[str]:
        return [
            "iq1",
            "iq1_s",
            "iq1_m",
            "iq2_xxs",
            "iq2_xs",
            "iq2_s",
            "iq2_k_s",
            "iq2_k",
            "iq2_m",
            "iq3_xxs",
            "iq3_xs",
            "iq3_s",
            "iq3_k_s",
            "iq3_m",
            "iq3_k_m",
            "iq3_k_l",
            "iq4_xxs",
            "iq4_xs",
            "iq4_s",
            "iq4_m",
            "iq4_k_s",
            "iq4_k_m",
            "iq4_k_l",
            "q2",
            "q2_k",
            "q2_k_s",
            "q2_k_m",
            "q3",
            "q3_k",
            "q3_k_s",
            "q3_k_m",
            "q3_k_l",
            "q4",
            "q4_0",
            "q4_1",
            "q4_k_s",
            "q4_k_m",
            "q5",
            "q5_0",
            "q5_1",
            "q5_k_s",
            "q5_k_m",
            "q6",
            "q6_k",
            "q8",
            "q8_0",
            "int4",
            "int8",
            "fp16",
            "fp32",
            "bf16",
            "f16",
            "f32",
        ]

    def _normalize_quant_source(self, value: str) -> str:
        normalized = re.sub(r"[^a-z0-9]+", "_", value.lower())
        return normalized.strip("_")

    def _token_in_normalized(self, normalized: str, token: str) -> bool:
        if not normalized or not token:
            return False
        segments = normalized.split("_")
        token_segments = token.split("_")
        if not token_segments or len(token_segments) > len(segments):
            return False
        for index in range(len(segments) - len(token_segments) + 1):
            if segments[index : index + len(token_segments)] == token_segments:
                return True
        return False

    def _sorted_quants(self, quants: Iterable[str]) -> list[str]:
        order = {token: index for index, token in enumerate(self._quant_tokens())}
        unique = {quant for quant in quants if quant}
        return sorted(unique, key=lambda token: (order.get(token, len(order)), token))

    def _coerce_int(self, value: object) -> int:
        if isinstance(value, bool):
            return int(value)
        if isinstance(value, int):
            return value
        if isinstance(value, float):
            return int(value)
        if isinstance(value, str):
            try:
                return int(value)
            except ValueError:
                return 0
        return 0

    def _download_allow_patterns(self, quant: Optional[str]) -> list[str] | None:
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
        normalized_path = self._normalize_quant_source(path)
        lower = path.lower()
        token = self._normalize_quant_source(quant)
        if self._token_in_normalized(normalized_path, token):
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

    def _collect_paths_with_sizes(self, siblings: Iterable[object]) -> list[tuple[str, int]]:
        results: list[tuple[str, int]] = []
        for sibling in siblings:
            path = getattr(sibling, "rfilename", "") or ""
            if not path:
                continue
            size = getattr(sibling, "size", None)
            if size is None:
                lfs = getattr(sibling, "lfs", None)
                if isinstance(lfs, dict):
                    size = lfs.get("size")
            try:
                size_value = int(size) if size is not None else 0
            except (TypeError, ValueError):
                size_value = 0
            if size_value > 0:
                results.append((path, size_value))
        return results

    def _list_repo_tree_paths(self, api, repo_id: str) -> list[tuple[str, int]]:
        try:
            items = api.list_repo_tree(repo_id=repo_id, repo_type="model", recursive=True)
        except (OSError, RuntimeError, ValueError):
            return []

        results: list[tuple[str, int]] = []
        for item in items:
            path = getattr(item, "path", "") or getattr(item, "rfilename", "") or ""
            if not path:
                continue
            size = getattr(item, "size", None)
            try:
                size_value = int(size) if size is not None else 0
            except (TypeError, ValueError):
                size_value = 0
            if size_value > 0:
                results.append((path, size_value))
        return results

    def _extract_formats_from_paths(self, paths: Iterable[str], tags: Iterable[str]) -> list[str]:
        formats = set()
        for path in paths:
            lower = path.lower()
            for ext in self._known_formats():
                token = f".{ext}"
                if lower.endswith(token) or token in lower:
                    formats.add(ext)
        for tag in tags:
            lower = tag.lower()
            for ext in self._known_formats():
                if ext in lower:
                    formats.add(ext)
        return sorted(formats)

    def _extract_formats(self, siblings: Iterable[object], tags: Iterable[str]) -> list[str]:
        paths = [getattr(sibling, "rfilename", "") or "" for sibling in siblings]
        return self._extract_formats_from_paths(paths, tags)

    def _extract_quants_from_paths(self, paths: Iterable[str], tags: Iterable[str]) -> list[str]:
        quants = set()
        quant_tokens = sorted(self._quant_tokens(), key=len, reverse=True)
        for path in paths:
            normalized = self._normalize_quant_source(path)
            matched = None
            for token in quant_tokens:
                if self._token_in_normalized(normalized, token):
                    matched = token
                    break
            if matched:
                quants.add(matched)
        for tag in tags:
            normalized = self._normalize_quant_source(tag)
            matched = None
            for token in quant_tokens:
                if self._token_in_normalized(normalized, token):
                    matched = token
                    break
            if matched:
                quants.add(matched)
        return self._sorted_quants(quants)

    def _extract_quants(self, siblings: Iterable[object], tags: Iterable[str]) -> list[str]:
        paths = [getattr(sibling, "rfilename", "") or "" for sibling in siblings]
        return self._extract_quants_from_paths(paths, tags)

    def _quant_sizes_from_paths(
        self, paths_with_sizes: Iterable[tuple[str, int]]
    ) -> dict[str, int]:
        quant_sizes: dict[str, int] = {}
        tokens = sorted(self._quant_tokens(), key=len, reverse=True)
        shared_size = 0
        shared_exts = {".json", ".yml", ".yaml", ".txt", ".md"}
        for path, size in paths_with_sizes:
            normalized = self._normalize_quant_source(path)
            lower = path.lower()
            matched = None
            for token in tokens:
                if self._token_in_normalized(normalized, token):
                    matched = token
                    break
            if matched:
                quant_sizes[matched] = quant_sizes.get(matched, 0) + size
            else:
                if any(lower.endswith(ext) for ext in shared_exts):
                    shared_size += size
        if shared_size and quant_sizes:
            for token in list(quant_sizes.keys()):
                quant_sizes[token] += shared_size
        return quant_sizes

    def _infer_kind_from_tags(self, tags: Iterable[str]) -> str:
        normalized = [tag.lower() for tag in tags]
        mapping = {
            "text-to-image": ["text-to-image", "text2img", "text-to-img"],
            "image-to-image": ["image-to-image", "img2img", "image-to-img"],
            "text-to-video": ["text-to-video", "text2video"],
            "text-to-audio": ["text-to-audio", "text-to-speech", "tts"],
            "audio-to-text": [
                "audio-to-text",
                "speech-recognition",
                "automatic-speech-recognition",
                "asr",
            ],
            "text-to-3d": ["text-to-3d", "text2shape"],
            "image-to-3d": ["image-to-3d", "img2shape", "image-to-shape"],
        }
        for kind, needles in mapping.items():
            for tag in normalized:
                if any(needle in tag for needle in needles):
                    return kind
        for tag in normalized:
            if "video" in tag:
                return "video"
            if "audio" in tag:
                return "audio"
            if "image" in tag:
                return "image"
            if "text" in tag:
                return "text"
            if "3d" in tag:
                return "3d"
        return "unknown"

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
            if kind_value == "unknown" and tags:
                kind_value = self._infer_kind_from_tags(tags)
            release_date = ""
            last_modified = getattr(info, "last_modified", None)
            if last_modified:
                try:
                    release_date = last_modified.isoformat()
                except AttributeError:
                    release_date = str(last_modified)
            downloads = getattr(info, "downloads", None)
            if downloads is not None:
                try:
                    downloads = int(downloads)
                except (TypeError, ValueError):
                    downloads = None
            formats = self._extract_formats(siblings, tags)
            quants = self._extract_quants(siblings, tags)
            paths_with_sizes = self._collect_paths_with_sizes(siblings)
            total_size_bytes = sum(size for _, size in paths_with_sizes)
            quant_sizes = self._quant_sizes_from_paths(paths_with_sizes)

            if not formats or not quants or total_size_bytes == 0:
                try:
                    repo_files = api.list_repo_files(repo_id=repo_id, repo_type="model")
                except (OSError, RuntimeError, ValueError):
                    repo_files = []
                if repo_files:
                    if not formats:
                        formats = self._extract_formats_from_paths(repo_files, tags)
                    if not quants:
                        quants = self._extract_quants_from_paths(repo_files, tags)
            if total_size_bytes == 0 or (quants and not quant_sizes):
                repo_paths_with_sizes = self._list_repo_tree_paths(api, repo_id)
                if repo_paths_with_sizes:
                    if total_size_bytes == 0:
                        total_size_bytes = sum(size for _, size in repo_paths_with_sizes)
                    if quants and not quant_sizes:
                        quant_sizes = self._quant_sizes_from_paths(repo_paths_with_sizes)

            if quant_sizes:
                quant_candidates = set(quant_sizes.keys())
            else:
                quant_candidates = set(quants)
            sorted_quants = self._sorted_quants(quant_candidates)
            download_options = [
                {"quant": quant, "sizeBytes": quant_sizes.get(quant)} for quant in sorted_quants
            ]

            models.append(
                {
                    "repoId": repo_id,
                    "name": repo_id.split("/")[-1],
                    "developer": developer,
                    "kind": kind_value,
                    "formats": formats,
                    "quants": sorted_quants,
                    "url": f"https://huggingface.co/{repo_id}",
                    "releaseDate": release_date,
                    "downloads": downloads,
                    "totalSizeBytes": total_size_bytes or None,
                    "quantSizes": quant_sizes,
                    "downloadOptions": download_options,
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
        quant: Optional[str] = None,
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
        with self._download_lock:
            state = self._downloads.get(download_id)
            if not state:
                return None

        status = str(state.get("status", "unknown"))
        total_bytes = self._coerce_int(state.get("total_bytes"))
        downloaded_bytes = self._coerce_int(state.get("downloaded_bytes"))
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
        total = 0
        try:
            for path in temp_dir.rglob("*"):
                if path.is_file():
                    if self._DOWNLOAD_CACHE_DIRNAME in path.parts:
                        continue
                    total += path.stat().st_size
        except (OSError, RuntimeError):
            return 0
        return total

    def _calculate_total_bytes(self, repo_id: str, quant: Optional[str]) -> int:
        api = self._get_api()
        paths_with_sizes = self._list_repo_tree_paths(api, repo_id)
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
        api = self._get_api()
        try:
            items = api.list_repo_tree(repo_id=repo_id, repo_type="model", recursive=True)
        except (OSError, RuntimeError, ValueError) as exc:
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

        try:
            quant_value = state.get("quant")
            quant = quant_value if isinstance(quant_value, str) else None
            self._download_files_with_cancel(
                repo_id=repo_id,
                temp_dir=temp_dir,
                quant=quant,
                cancel_event=cancel_event,
            )
        except (OSError, RuntimeError, ValueError) as exc:
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
        except (OSError, RuntimeError, ValueError) as exc:
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
