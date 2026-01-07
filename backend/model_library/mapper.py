"""Model mapping engine for linking library models into app directories."""

from __future__ import annotations

import json
from pathlib import Path
from typing import Any, Dict, Iterable, List

from packaging.specifiers import InvalidSpecifier, SpecifierSet
from packaging.version import InvalidVersion, Version

from backend.logging_config import get_logger
from backend.model_library.library import ModelLibrary
from backend.model_library.naming import normalize_filename, unique_path
from backend.utils import ensure_directory, is_broken_symlink, make_relative_symlink

logger = get_logger(__name__)


class ModelMapper:
    """Applies translation configs to map library models into app folders."""

    def __init__(self, library: ModelLibrary, config_root: Path) -> None:
        self.library = library
        self.config_root = Path(config_root)
        self.config_root.mkdir(parents=True, exist_ok=True)

    def _load_configs(self, app_id: str, app_version: str) -> List[Dict[str, Any]]:
        configs: List[Dict[str, Any]] = []
        if not self.config_root.exists():
            return configs

        target_app = app_id.lower()
        target_version = app_version

        for config_path in sorted(self.config_root.glob("*.json")):
            parts = config_path.stem.split("_", 2)
            if len(parts) < 3:
                continue
            config_app, config_version, _ = parts
            if config_app.lower() != target_app:
                continue
            if config_version != target_version:
                continue

            try:
                with open(config_path, "r", encoding="utf-8") as f:
                    configs.append(json.load(f))
            except (OSError, json.JSONDecodeError) as exc:
                logger.error("Failed to read mapping config %s: %s", config_path, exc)

        return configs

    def _version_allowed(self, model_dir: Path, app_id: str, app_version: str) -> bool:
        overrides = self.library.load_overrides(model_dir)
        if not overrides:
            return True

        ranges = overrides.get("version_ranges", {})
        if not isinstance(ranges, dict):
            return True

        target_range = None
        for key, value in ranges.items():
            if key.lower() == app_id.lower():
                target_range = value
                break

        if not target_range:
            return True

        try:
            spec = SpecifierSet(str(target_range))
            version = Version(app_version)
            return version in spec
        except (InvalidSpecifier, InvalidVersion) as exc:
            logger.warning("Invalid version range %s for %s: %s", target_range, app_id, exc)
            return True

    def _matches_filters(self, metadata: Dict[str, Any], filters: Dict[str, Any]) -> bool:
        if not filters:
            return True

        model_type = metadata.get("model_type", "")
        subtype = metadata.get("subtype", "")
        tags = set(metadata.get("tags", []))
        family = metadata.get("family", "")

        allowed_types = filters.get("model_type") or filters.get("model_types")
        if isinstance(allowed_types, str):
            allowed_types = [allowed_types]
        if allowed_types and model_type not in allowed_types:
            return False

        allowed_subtypes = filters.get("subtypes") or filters.get("subtype")
        if isinstance(allowed_subtypes, str):
            allowed_subtypes = [allowed_subtypes]
        if allowed_subtypes and subtype not in allowed_subtypes:
            return False

        allowed_families = filters.get("families")
        if isinstance(allowed_families, str):
            allowed_families = [allowed_families]
        if allowed_families and family not in allowed_families:
            return False

        required_tags = filters.get("tags")
        if isinstance(required_tags, str):
            required_tags = [required_tags]
        if required_tags:
            if not tags.intersection(required_tags):
                return False

        return True

    def _iter_matching_files(self, model_dir: Path, patterns: Iterable[str]) -> Iterable[Path]:
        seen = set()
        for pattern in patterns:
            for candidate in model_dir.glob(pattern):
                if candidate in seen:
                    continue
                if candidate.name in ("metadata.json", "overrides.json"):
                    continue
                if not candidate.is_file():
                    continue
                seen.add(candidate)
                yield candidate

    def _create_link(self, source: Path, target: Path) -> bool:
        if target.exists():
            if target.is_symlink():
                target.unlink()
            else:
                logger.warning("Skipping existing non-symlink: %s", target)
                return False

        ensure_directory(target.parent)
        return make_relative_symlink(source, target)

    def apply_for_app(self, app_id: str, app_version: str, app_models_root: Path) -> int:
        configs = self._load_configs(app_id, app_version)
        if not configs:
            logger.info("No mapping config found for %s %s", app_id, app_version)
            return 0

        total_links = 0
        models = self.library.list_models()

        for config in configs:
            mappings = config.get("mappings", [])
            for mapping in mappings:
                method = mapping.get("method", "symlink")
                if method != "symlink":
                    logger.info("Skipping non-symlink mapping method: %s", method)
                    continue

                target_subdir = mapping.get("target_subdir")
                if not target_subdir:
                    logger.warning("Mapping entry missing target_subdir")
                    continue

                target_dir = Path(app_models_root) / target_subdir
                ensure_directory(target_dir)

                patterns = mapping.get("patterns", ["*"])
                if isinstance(patterns, str):
                    patterns = [patterns]
                filters = mapping.get("filters", {})
                if not isinstance(filters, dict):
                    filters = {}

                for metadata in models:
                    rel_path = metadata.get("library_path")
                    if not rel_path:
                        continue
                    model_dir = self.library.library_root / rel_path
                    if not model_dir.exists():
                        continue

                    if not self._version_allowed(model_dir, app_id, app_version):
                        continue
                    if not self._matches_filters(metadata, filters):
                        continue

                    for source_file in self._iter_matching_files(model_dir, patterns):
                        cleaned_name = normalize_filename(source_file.name)
                        target_path = target_dir / cleaned_name
                        if target_path.exists():
                            if target_path.is_symlink():
                                target_path.unlink()
                            else:
                                target_path = unique_path(target_path)

                        if is_broken_symlink(target_path):
                            target_path.unlink()
                        if self._create_link(source_file, target_path):
                            total_links += 1

        return total_links
