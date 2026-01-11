"""HuggingFace model search functionality."""

from __future__ import annotations

from typing import TYPE_CHECKING, Optional

from backend.logging_config import get_logger
from backend.model_library.hf.formats import extract_formats, extract_formats_from_paths
from backend.model_library.hf.metadata import collect_paths_with_sizes, infer_kind_from_tags
from backend.model_library.hf.quant import (
    extract_quants_from_paths,
    quant_sizes_from_paths,
    sorted_quants,
)

if TYPE_CHECKING:
    from huggingface_hub import HfApi

logger = get_logger(__name__)


def list_repo_tree_paths(api: HfApi, repo_id: str) -> list[tuple[str, int]]:
    """List all files in a repository with their sizes.

    Args:
        api: HuggingFace API instance
        repo_id: Repository ID (e.g., 'username/model-name')

    Returns:
        List of (path, size_bytes) tuples
    """
    try:
        items = api.list_repo_tree(repo_id=repo_id, repo_type="model", recursive=True)
    except OSError as exc:
        logger.debug("Failed to list repo tree for %s: %s", repo_id, exc)
        return []
    except RuntimeError as exc:
        logger.debug("Failed to list repo tree for %s: %s", repo_id, exc)
        return []
    except ValueError as exc:
        logger.debug("Failed to list repo tree for %s: %s", repo_id, exc)
        return []

    results: list[tuple[str, int]] = []
    for item in items:
        path = getattr(item, "path", "") or getattr(item, "rfilename", "") or ""
        if not path:
            continue
        size = getattr(item, "size", None)
        try:
            size_value = int(size) if size is not None else 0
        except TypeError as exc:
            logger.debug("Invalid size value %r: %s", size, exc)
            size_value = 0
        except ValueError as exc:
            logger.debug("Invalid size value %r: %s", size, exc)
            size_value = 0
        if size_value > 0:
            results.append((path, size_value))
    return results


def _extract_quants_from_siblings(siblings: list[object], tags: list[str]) -> list[str]:
    """Extract quants from sibling objects and tags."""
    paths = [getattr(sibling, "rfilename", "") or "" for sibling in siblings]
    return extract_quants_from_paths(paths, tags)


def search_models(
    api: HfApi,
    query: str,
    kind: Optional[str] = None,
    limit: int = 25,
) -> list[dict[str, object]]:
    """Search for models on HuggingFace Hub.

    Args:
        api: HuggingFace API instance
        query: Search query string
        kind: Optional model kind/task filter
        limit: Maximum number of results

    Returns:
        List of model dictionaries with metadata
    """
    filter_arg = None
    if kind:
        try:
            from huggingface_hub import ModelFilter
        except ImportError as exc:
            logger.debug("huggingface_hub ModelFilter unavailable: %s", exc)
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
            kind_value = infer_kind_from_tags(tags)

        release_date = _extract_release_date(info)
        downloads = _extract_downloads(info)

        formats = extract_formats(siblings, tags)
        quants = _extract_quants_from_siblings(siblings, tags)
        paths_with_sizes = collect_paths_with_sizes(siblings)
        total_size_bytes = sum(size for _, size in paths_with_sizes)
        quant_sizes = quant_sizes_from_paths(paths_with_sizes)

        if not formats or not quants or total_size_bytes == 0:
            repo_files = _list_repo_files_safe(api, repo_id)
            if repo_files:
                if not formats:
                    formats = extract_formats_from_paths(repo_files, tags)
                if not quants:
                    quants = extract_quants_from_paths(repo_files, tags)

        if total_size_bytes == 0 or (quants and not quant_sizes):
            repo_paths_with_sizes = list_repo_tree_paths(api, repo_id)
            if repo_paths_with_sizes:
                if total_size_bytes == 0:
                    total_size_bytes = sum(size for _, size in repo_paths_with_sizes)
                if quants and not quant_sizes:
                    quant_sizes = quant_sizes_from_paths(repo_paths_with_sizes)

        if quant_sizes:
            quant_candidates = set(quant_sizes.keys())
        else:
            quant_candidates = set(quants)
        sorted_quant_list = sorted_quants(quant_candidates)
        download_options = [
            {"quant": quant, "sizeBytes": quant_sizes.get(quant)} for quant in sorted_quant_list
        ]

        models.append(
            {
                "repoId": repo_id,
                "name": repo_id.split("/")[-1],
                "developer": developer,
                "kind": kind_value,
                "formats": formats,
                "quants": sorted_quant_list,
                "url": f"https://huggingface.co/{repo_id}",
                "releaseDate": release_date,
                "downloads": downloads,
                "totalSizeBytes": total_size_bytes or None,
                "quantSizes": quant_sizes,
                "downloadOptions": download_options,
            }
        )
    return models


def _extract_release_date(info: object) -> str:
    """Extract release date from model info."""
    last_modified = getattr(info, "last_modified", None)
    if not last_modified:
        return ""
    try:
        result: str = last_modified.isoformat()
        return result
    except AttributeError as exc:
        logger.debug("Failed to format last_modified %r: %s", last_modified, exc)
        return str(last_modified)


def _extract_downloads(info: object) -> int | None:
    """Extract download count from model info."""
    downloads = getattr(info, "downloads", None)
    if downloads is None:
        return None
    try:
        return int(downloads)
    except TypeError as exc:
        logger.debug("Invalid downloads value %r: %s", downloads, exc)
        return None
    except ValueError as exc:
        logger.debug("Invalid downloads value %r: %s", downloads, exc)
        return None


def _list_repo_files_safe(api: HfApi, repo_id: str) -> list[str]:
    """List repository files with error handling."""
    try:
        return list(api.list_repo_files(repo_id=repo_id, repo_type="model"))
    except OSError as exc:
        logger.debug("Failed to list repo files for %s: %s", repo_id, exc)
        return []
    except RuntimeError as exc:
        logger.debug("Failed to list repo files for %s: %s", repo_id, exc)
        return []
    except ValueError as exc:
        logger.debug("Failed to list repo files for %s: %s", repo_id, exc)
        return []
