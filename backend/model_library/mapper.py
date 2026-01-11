"""Model mapping engine for linking library models into app directories."""

from __future__ import annotations

import json
import os
from dataclasses import dataclass, field
from datetime import datetime, timezone
from enum import Enum
from pathlib import Path
from typing import TYPE_CHECKING, Any, Dict, Iterable, List, Literal, Optional

from packaging.specifiers import InvalidSpecifier, SpecifierSet
from packaging.version import InvalidVersion, Version

from backend.logging_config import get_logger
from backend.model_library.io.platform import (
    LinkStrategy,
    create_link,
    get_default_strategy,
    is_cross_filesystem,
    verify_link,
)
from backend.model_library.library import ModelLibrary
from backend.model_library.naming import normalize_filename, unique_path
from backend.utils import ensure_directory

if TYPE_CHECKING:
    from backend.model_library.link_registry import LinkRegistry

logger = get_logger(__name__)


class MappingActionType(Enum):
    """Types of mapping actions."""

    CREATE = "create"
    SKIP_EXISTS = "skip_exists"
    SKIP_CONFLICT = "skip_conflict"
    REMOVE_BROKEN = "remove_broken"


@dataclass
class MappingAction:
    """Represents a single mapping operation to be performed."""

    action_type: MappingActionType
    model_id: str
    model_name: str
    source_path: Path
    target_path: Path
    link_type: str
    reason: str = ""
    existing_target: str = ""


@dataclass
class MappingPreview:
    """Complete preview of all mapping operations."""

    to_create: List[MappingAction] = field(default_factory=list)
    to_skip_exists: List[MappingAction] = field(default_factory=list)
    conflicts: List[MappingAction] = field(default_factory=list)
    broken_to_remove: List[MappingAction] = field(default_factory=list)
    total_actions: int = 0
    warnings: List[str] = field(default_factory=list)
    errors: List[str] = field(default_factory=list)


@dataclass
class SandboxInfo:
    """Information about sandbox environment detection."""

    sandboxed: bool = False
    sandbox_type: Optional[Literal["flatpak", "snap", "docker"]] = None
    permissions_needed: List[str] = field(default_factory=list)


def detect_sandbox_environment() -> SandboxInfo:
    """Detect if running in a sandboxed environment.

    Returns:
        SandboxInfo with detection results and required permissions
    """
    # Check for Flatpak
    if Path("/.flatpak-info").exists():
        return SandboxInfo(
            sandboxed=True,
            sandbox_type="flatpak",
            permissions_needed=[
                "Filesystem access to library directory",
                "Filesystem access to ComfyUI directory",
            ],
        )

    # Check for Snap
    if "SNAP" in os.environ:
        return SandboxInfo(
            sandboxed=True,
            sandbox_type="snap",
            permissions_needed=["Connect 'removable-media' interface"],
        )

    # Check for Docker
    if Path("/.dockerenv").exists():
        return SandboxInfo(
            sandboxed=True,
            sandbox_type="docker",
            permissions_needed=["Mount library directory as volume"],
        )

    return SandboxInfo(sandboxed=False)


class ModelMapper:
    """Applies translation configs to map library models into app folders."""

    def __init__(
        self,
        library: ModelLibrary,
        config_root: Path,
        link_registry: LinkRegistry | None = None,
    ) -> None:
        """Initialize the model mapper.

        Args:
            library: Model library instance
            config_root: Path to mapping configuration files
            link_registry: Optional link registry for tracking created links
        """
        self.library = library
        self.config_root = Path(config_root)
        self.config_root.mkdir(parents=True, exist_ok=True)
        self._link_registry = link_registry

    def _load_configs(self, app_id: str, app_version: str) -> List[Dict[str, Any]]:
        """Load configs for app version (deprecated, use _load_and_merge_configs)."""
        merged = self._load_and_merge_configs(app_id, app_version)
        if merged:
            return [merged]
        return []

    def _load_and_merge_configs(self, app_id: str, app_version: str) -> Optional[Dict[str, Any]]:
        """Load and merge all matching configs with deterministic precedence.

        Precedence (highest to lowest):
        1. Exact version + custom variant: comfyui_0.6.0_custom.json
        2. Exact version + default: comfyui_0.6.0_default.json
        3. Wildcard + custom variant: comfyui_*_custom.json
        4. Wildcard + default: comfyui_*_default.json

        Returns:
            Merged config with all mappings, sorted by priority
        """
        if not self.config_root.exists():
            return None

        configs: List[Dict[str, Any]] = []
        target_app = app_id.lower()

        for config_path in sorted(self.config_root.glob("*.json")):
            parts = config_path.stem.split("_", 2)
            if len(parts) < 3:
                continue

            config_app, config_version, config_variant = parts

            if config_app.lower() != target_app:
                continue

            # Check version match (exact or wildcard)
            if config_version != "*" and config_version != app_version:
                continue

            try:
                with open(config_path, "r", encoding="utf-8") as f:
                    config_data = json.load(f)
                    config_data["_source_file"] = config_path.name
                    config_data["_specificity"] = self._calculate_specificity(
                        config_version, config_variant
                    )
                    configs.append(config_data)
            except OSError as exc:
                logger.error("Failed to read mapping config %s: %s", config_path, exc)
            except json.JSONDecodeError as exc:
                logger.error("Failed to parse mapping config %s: %s", config_path, exc)

        if not configs:
            return None

        # Sort by specificity (highest first)
        configs.sort(key=lambda c: c.get("_specificity", 0), reverse=True)

        # Merge all mappings
        merged: Dict[str, Any] = {
            "app_id": app_id,
            "app_version": app_version,
            "variant": "merged",
            "description": f"Merged from {len(configs)} configs",
            "mappings": [],
        }

        for config in configs:
            for mapping in config.get("mappings", []):
                merged["mappings"].append(
                    {**mapping, "_source": config.get("_source_file", "unknown")}
                )

        # Sort mappings by priority (lower priority value = applied first)
        merged["mappings"].sort(key=lambda m: m.get("priority", 0))

        return merged

    def _calculate_specificity(self, version: str, variant: str) -> int:
        """Calculate config specificity score.

        Higher score = more specific = higher precedence.

        Args:
            version: Version string or "*" for wildcard
            variant: Variant name (e.g., "default", "sdxl-only")

        Returns:
            Specificity score
        """
        score = 0

        # Version specificity
        if version != "*":
            score += 100  # Exact version

        # Variant specificity
        if variant != "default":
            score += 10  # Custom variant

        return score

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
        except InvalidSpecifier as exc:
            logger.warning("Invalid version range %s for %s: %s", target_range, app_id, exc)
            return True
        except InvalidVersion as exc:
            logger.warning("Invalid version range %s for %s: %s", target_range, app_id, exc)
            return True

    def _matches_filters(self, metadata: Dict[str, Any], filters: Dict[str, Any]) -> bool:
        """Check if model metadata matches filter criteria.

        Filter logic:
        - model_type, subtype, families: AND (must match all specified)
        - tags: OR (match ANY tag in list)
        - exclude_tags: OR (exclude if has ANY excluded tag)
        - Exclusion happens AFTER inclusion (exclusion wins)

        Args:
            metadata: Model metadata dict
            filters: Filter criteria dict

        Returns:
            True if model matches all filter criteria
        """
        if not filters:
            return True

        model_type = metadata.get("model_type", "")
        subtype = metadata.get("subtype", "")
        tags = set(metadata.get("tags", []))
        family = metadata.get("family", "")

        # Check model_type (AND logic)
        allowed_types = filters.get("model_type") or filters.get("model_types")
        if isinstance(allowed_types, str):
            allowed_types = [allowed_types]
        if allowed_types and model_type not in allowed_types:
            return False

        # Check subtype (AND logic)
        allowed_subtypes = filters.get("subtypes") or filters.get("subtype")
        if isinstance(allowed_subtypes, str):
            allowed_subtypes = [allowed_subtypes]
        if allowed_subtypes and subtype not in allowed_subtypes:
            return False

        # Check family (AND logic)
        allowed_families = filters.get("families")
        if isinstance(allowed_families, str):
            allowed_families = [allowed_families]
        if allowed_families and family not in allowed_families:
            return False

        # Check required tags (OR logic - match ANY)
        required_tags = filters.get("tags")
        if isinstance(required_tags, str):
            required_tags = [required_tags]
        if required_tags:
            if not tags.intersection(required_tags):
                return False

        # Check excluded tags (OR logic - exclude if has ANY)
        # Exclusion happens AFTER inclusion, so exclusion wins
        excluded_tags = filters.get("exclude_tags")
        if isinstance(excluded_tags, str):
            excluded_tags = [excluded_tags]
        if excluded_tags:
            if tags.intersection(excluded_tags):
                return False  # Has at least one excluded tag

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

    def _create_link(
        self,
        source: Path,
        target: Path,
        strategy: LinkStrategy | None = None,
    ) -> bool:
        """Create a link from target to source using io/platform.

        This is the base link creation method without registry tracking.

        Args:
            source: The actual file to link to
            target: Where the link will be created
            strategy: Link strategy to use (defaults to platform default)

        Returns:
            True if link was created successfully
        """
        if strategy is None:
            strategy = get_default_strategy()

        # Handle existing non-symlinks
        if target.exists() and not target.is_symlink():
            logger.warning("Skipping existing non-symlink: %s", target)
            return False

        result = create_link(
            source=source,
            target=target,
            strategy=strategy,
            relative=True,
            overwrite=True,
        )

        if not result.success:
            logger.warning("Failed to create link %s -> %s: %s", target, source, result.error)
            return False

        return True

    def _create_link_with_registry(
        self,
        source: Path,
        target: Path,
        model_id: str,
        app_id: str,
        app_version: str,
        strategy: LinkStrategy | None = None,
    ) -> bool:
        """Create a link from target to source and register it.

        Args:
            source: The actual file to link to
            target: Where the link will be created
            model_id: ID of the model being linked
            app_id: Application identifier
            app_version: Application version
            strategy: Link strategy to use (defaults to platform default)

        Returns:
            True if link was created successfully
        """
        if strategy is None:
            strategy = get_default_strategy()

        # Create the link using base method
        if not self._create_link(source, target, strategy):
            return False

        # Register in link registry if available
        if self._link_registry is not None:
            from backend.model_library.link_registry import LinkType

            # Map LinkStrategy to LinkType
            link_type_map = {
                LinkStrategy.SYMLINK: LinkType.SYMLINK,
                LinkStrategy.HARDLINK: LinkType.HARDLINK,
                LinkStrategy.COPY: LinkType.COPY,
            }
            link_type = link_type_map.get(strategy, LinkType.SYMLINK)

            # Check if this is a cross-filesystem link
            is_external = is_cross_filesystem(source, target)

            # Unregister any existing link at this target first
            self._link_registry.unregister_by_target(target)

            self._link_registry.register_link(
                model_id=model_id,
                source_path=source,
                target_path=target,
                link_type=link_type,
                app_id=app_id,
                app_version=app_version,
                is_external=is_external,
            )

        return True

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

                    # Get model_id for registry tracking
                    model_id = rel_path

                    for source_file in self._iter_matching_files(model_dir, patterns):
                        cleaned_name = normalize_filename(source_file.name)
                        target_path = target_dir / cleaned_name

                        # Handle existing targets
                        if target_path.exists() or target_path.is_symlink():
                            if target_path.is_symlink():
                                # Check if symlink is broken
                                is_valid, _ = verify_link(target_path)
                                if not is_valid:
                                    target_path.unlink()
                                else:
                                    target_path.unlink()  # Replace with new link
                            else:
                                # Non-symlink exists, use unique path
                                target_path = unique_path(target_path)

                        if self._create_link_with_registry(
                            source_file, target_path, model_id, app_id, app_version
                        ):
                            total_links += 1

        return total_links

    def delete_model_with_cascade(self, model_id: str) -> int:
        """Delete all links for a model using the registry.

        This should be called before deleting a model from the library.

        Args:
            model_id: ID of the model to delete links for

        Returns:
            Number of links removed
        """
        if self._link_registry is None:
            logger.warning("No link registry available for cascade delete")
            return 0

        return self._link_registry.delete_links_for_model(model_id)

    def discover_model_directories(self, models_root: Path) -> List[str]:
        """Scan app models/ directory for subdirectories.

        Used for dynamic directory discovery when creating default configs.

        Args:
            models_root: Path to app's models/ directory

        Returns:
            List of subdirectory names (e.g., ['checkpoints', 'loras', 'ipadapter'])
        """
        if not models_root.is_dir():
            return []

        subdirs = []
        for item in models_root.iterdir():
            if item.is_dir() and not item.name.startswith("."):
                subdirs.append(item.name)

        return sorted(subdirs)

    def create_default_comfyui_config(
        self,
        version: str = "*",
        comfyui_models_path: Optional[Path] = None,
    ) -> Path:
        """Create default ComfyUI mapping config with dynamic directory discovery.

        Args:
            version: ComfyUI version (e.g., "0.6.0" or "*")
            comfyui_models_path: Path to ComfyUI models/ dir (for scanning)

        Returns:
            Path to created config file
        """
        # Static baseline mappings
        baseline_mappings = [
            {
                "name": "Main Checkpoints",
                "description": "Stable Diffusion checkpoints",
                "method": "symlink",
                "target_subdir": "checkpoints",
                "patterns": ["*.safetensors", "*.ckpt"],
                "link_type": "file",
                "filters": {"model_type": "diffusion", "subtype": "checkpoints"},
                "enabled": True,
                "priority": 10,
            },
            {
                "name": "LoRA Adapters",
                "description": "LoRA fine-tuning adapters",
                "method": "symlink",
                "target_subdir": "loras",
                "patterns": ["*.safetensors", "*.pt"],
                "link_type": "file",
                "filters": {"model_type": "diffusion", "subtype": "loras"},
                "enabled": True,
                "priority": 20,
            },
            {
                "name": "VAE Models",
                "description": "Variational Autoencoders",
                "method": "symlink",
                "target_subdir": "vae",
                "patterns": ["*.safetensors", "*.pt"],
                "link_type": "file",
                "filters": {"model_type": "diffusion", "subtype": "vae"},
                "enabled": True,
                "priority": 30,
            },
            {
                "name": "ControlNet Models",
                "description": "ControlNet conditioning models",
                "method": "symlink",
                "target_subdir": "controlnet",
                "patterns": ["*.safetensors", "*.pt", "*.gguf"],
                "link_type": "file",
                "filters": {"model_type": "diffusion", "subtype": "controlnet"},
                "enabled": True,
                "priority": 40,
            },
            {
                "name": "Embeddings",
                "description": "Textual inversion embeddings",
                "method": "symlink",
                "target_subdir": "embeddings",
                "patterns": ["*.pt", "*.safetensors"],
                "link_type": "file",
                "filters": {"model_type": "diffusion", "subtype": "embeddings"},
                "enabled": True,
                "priority": 50,
            },
            {
                "name": "Upscale Models",
                "description": "Image upscaling models",
                "method": "symlink",
                "target_subdir": "upscale_models",
                "patterns": ["*.pth", "*.safetensors"],
                "link_type": "file",
                "filters": {"model_type": "diffusion", "subtype": "upscale"},
                "enabled": True,
                "priority": 60,
            },
            {
                "name": "CLIP Models",
                "description": "CLIP text encoder models",
                "method": "symlink",
                "target_subdir": "clip",
                "patterns": ["*.safetensors", "*.pt"],
                "link_type": "file",
                "filters": {"model_type": "diffusion", "subtype": "clip"},
                "enabled": True,
                "priority": 70,
            },
            {
                "name": "CLIP Vision Models",
                "description": "CLIP vision encoder models",
                "method": "symlink",
                "target_subdir": "clip_vision",
                "patterns": ["*.safetensors", "*.pt"],
                "link_type": "file",
                "filters": {"model_type": "diffusion", "subtype": "clip_vision"},
                "enabled": True,
                "priority": 80,
            },
        ]

        mappings = list(baseline_mappings)

        # If path provided, discover additional directories
        if comfyui_models_path and comfyui_models_path.exists():
            discovered_dirs = self.discover_model_directories(comfyui_models_path)

            # Find directories not in baseline
            baseline_subdirs = {m["target_subdir"] for m in baseline_mappings}
            new_subdirs = [d for d in discovered_dirs if d not in baseline_subdirs]

            if new_subdirs:
                logger.info(
                    "Discovered %d additional model directories: %s",
                    len(new_subdirs),
                    new_subdirs,
                )

            # Create generic mappings for new directories
            for i, subdir in enumerate(new_subdirs):
                mappings.append(
                    {
                        "name": f"{subdir.replace('_', ' ').title()} (Auto-discovered)",
                        "description": "Auto-discovered directory from ComfyUI installation",
                        "method": "symlink",
                        "target_subdir": subdir,
                        "patterns": ["*.safetensors", "*.pt", "*.ckpt"],
                        "link_type": "file",
                        "filters": {"model_type": "diffusion"},
                        "enabled": True,
                        "priority": 200 + i,
                    }
                )

        timestamp = datetime.now(timezone.utc).isoformat()
        config = {
            "app": "comfyui",
            "version": version,
            "variant": "default",
            "description": f"Default model mappings for ComfyUI {version}",
            "created_at": timestamp,
            "updated_at": timestamp,
            "mappings": mappings,
        }

        # Save config
        filename = f"comfyui_{version}_default.json"
        config_path = self.config_root / filename

        with open(config_path, "w", encoding="utf-8") as f:
            json.dump(config, f, indent=2, ensure_ascii=False)

        logger.info("Created config with %d mappings: %s", len(mappings), config_path)
        return config_path

    def sync_models_incremental(
        self,
        app_id: str,
        app_version: str,
        models_root: Path,
        model_ids: List[str],
    ) -> Dict[str, int]:
        """Incrementally sync specific models only.

        Much faster than full sync when only a few models were added.

        Args:
            app_id: Application ID
            app_version: Version string
            models_root: Path to app's models/ directory
            model_ids: List of model IDs (library paths) to process

        Returns:
            Dict with counts: links_created, links_updated, links_skipped
        """
        config = self._load_and_merge_configs(app_id, app_version)
        if not config:
            logger.warning("No mapping config found for %s %s", app_id, app_version)
            return {"links_created": 0, "links_updated": 0, "links_skipped": 0}

        links_created = 0
        links_updated = 0
        links_skipped = 0

        # Get metadata for specified models only
        models_metadata = []
        for model_id in model_ids:
            metadata = self.library.get_model(model_id)
            if metadata:
                models_metadata.append(metadata)

        # Apply mappings only for these models
        for mapping in config.get("mappings", []):
            if not mapping.get("enabled", True):
                continue

            method = mapping.get("method", "symlink")
            if method != "symlink":
                continue

            target_subdir = mapping.get("target_subdir")
            if not target_subdir:
                continue

            target_dir = models_root / target_subdir
            ensure_directory(target_dir)

            patterns = mapping.get("patterns", ["*"])
            if isinstance(patterns, str):
                patterns = [patterns]
            filters = mapping.get("filters", {})
            if not isinstance(filters, dict):
                filters = {}

            for metadata in models_metadata:
                # Check if model matches this mapping's filters
                if not self._matches_filters(metadata, filters):
                    continue

                rel_path = metadata.get("library_path")
                if not rel_path:
                    continue
                model_dir = self.library.library_root / rel_path
                if not model_dir.exists():
                    continue

                # Check version constraints
                if not self._version_allowed(model_dir, app_id, app_version):
                    continue

                # Find matching files
                for source_file in self._iter_matching_files(model_dir, patterns):
                    cleaned_name = normalize_filename(source_file.name)
                    target_path = target_dir / cleaned_name

                    # Check if link already exists and is correct
                    if target_path.exists() or target_path.is_symlink():
                        if target_path.is_symlink():
                            try:
                                current_target = target_path.resolve()
                                if current_target == source_file.resolve():
                                    links_skipped += 1
                                    continue
                            except OSError:  # noqa: no-except-logging
                                pass  # Broken link, will be replaced
                            target_path.unlink()
                            links_updated += 1
                        else:
                            # Non-symlink exists, use unique path
                            target_path = unique_path(target_path)

                    # Create link
                    if self._create_link_with_registry(
                        source_file, target_path, rel_path, app_id, app_version
                    ):
                        links_created += 1

        return {
            "links_created": links_created,
            "links_updated": links_updated,
            "links_skipped": links_skipped,
        }

    def preview_mapping(
        self,
        app_id: str,
        app_version: str,
        app_models_root: Path,
    ) -> MappingPreview:
        """Preview all mapping operations without making changes.

        Performs a dry run to show what would happen if apply_for_app was called.

        Args:
            app_id: Application ID ("comfyui")
            app_version: Version string ("0.6.0")
            app_models_root: Path to app's models/ directory

        Returns:
            MappingPreview with all planned operations
        """
        preview = MappingPreview()

        config = self._load_and_merge_configs(app_id, app_version)
        if not config:
            preview.errors.append(f"No mapping config found for {app_id} {app_version}")
            return preview

        # Check for cross-filesystem
        if is_cross_filesystem(self.library.library_root, app_models_root):
            preview.warnings.append(
                "Library and app are on different filesystems. "
                "Absolute symlinks will be used, which may break if the library drive is unmounted."
            )

        # Check for sandbox environment
        sandbox_info = detect_sandbox_environment()
        if sandbox_info.sandboxed:
            preview.warnings.append(
                f"Running in {sandbox_info.sandbox_type} sandbox. "
                f"Required permissions: {', '.join(sandbox_info.permissions_needed)}"
            )

        # Phase 1: Find broken symlinks to remove
        if app_models_root.exists():
            for subdir in app_models_root.iterdir():
                if not subdir.is_dir():
                    continue

                for item in subdir.iterdir():
                    if item.is_symlink() and not item.exists():
                        # Broken symlink
                        try:
                            old_target = str(item.readlink())
                        except OSError:  # noqa: no-except-logging
                            old_target = "[unreadable]"

                        preview.broken_to_remove.append(
                            MappingAction(
                                action_type=MappingActionType.REMOVE_BROKEN,
                                model_id="",
                                model_name="",
                                source_path=Path(),
                                target_path=item,
                                link_type="symlink",
                                reason="Broken link (target missing)",
                                existing_target=old_target,
                            )
                        )

        # Phase 2: Preview all mapping operations
        models = self.library.list_models()
        link_type = get_default_strategy().value

        for mapping in config.get("mappings", []):
            if not mapping.get("enabled", True):
                continue

            method = mapping.get("method", "symlink")
            if method != "symlink":
                continue

            target_subdir = mapping.get("target_subdir")
            if not target_subdir:
                continue

            target_dir = app_models_root / target_subdir
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

                official_name = metadata.get("official_name", "")

                for source_file in self._iter_matching_files(model_dir, patterns):
                    cleaned_name = normalize_filename(source_file.name)
                    target_path = target_dir / cleaned_name

                    action = self._preview_single_link(
                        model_id=rel_path,
                        model_name=official_name,
                        source_path=source_file,
                        target_path=target_path,
                        link_type=link_type,
                    )

                    if action.action_type == MappingActionType.CREATE:
                        preview.to_create.append(action)
                    elif action.action_type == MappingActionType.SKIP_EXISTS:
                        preview.to_skip_exists.append(action)
                    elif action.action_type == MappingActionType.SKIP_CONFLICT:
                        preview.conflicts.append(action)

        preview.total_actions = (
            len(preview.to_create)
            + len(preview.to_skip_exists)
            + len(preview.conflicts)
            + len(preview.broken_to_remove)
        )

        return preview

    def _preview_single_link(
        self,
        model_id: str,
        model_name: str,
        source_path: Path,
        target_path: Path,
        link_type: str,
    ) -> MappingAction:
        """Preview a single link operation.

        Args:
            model_id: ID of the model
            model_name: Display name of the model
            source_path: Source file path
            target_path: Target link path
            link_type: Type of link to create

        Returns:
            MappingAction describing what would happen
        """
        # Check if target exists
        if target_path.exists() or target_path.is_symlink():
            if target_path.is_symlink():
                try:
                    current_target = target_path.resolve()

                    # Check if it already points to correct source
                    if current_target == source_path.resolve():
                        return MappingAction(
                            action_type=MappingActionType.SKIP_EXISTS,
                            model_id=model_id,
                            model_name=model_name,
                            source_path=source_path,
                            target_path=target_path,
                            link_type=link_type,
                            reason="Already linked to correct source",
                            existing_target=str(current_target),
                        )

                    # Conflict: points to different source
                    return MappingAction(
                        action_type=MappingActionType.SKIP_CONFLICT,
                        model_id=model_id,
                        model_name=model_name,
                        source_path=source_path,
                        target_path=target_path,
                        link_type=link_type,
                        reason="Symlink points to different source",
                        existing_target=str(current_target),
                    )
                except OSError:  # noqa: no-except-logging
                    # Broken symlink - will be replaced
                    return MappingAction(
                        action_type=MappingActionType.CREATE,
                        model_id=model_id,
                        model_name=model_name,
                        source_path=source_path,
                        target_path=target_path,
                        link_type=link_type,
                        reason="Replace broken symlink",
                    )

            # Conflict: non-symlink file exists
            return MappingAction(
                action_type=MappingActionType.SKIP_CONFLICT,
                model_id=model_id,
                model_name=model_name,
                source_path=source_path,
                target_path=target_path,
                link_type=link_type,
                reason="Non-symlink file exists at target",
                existing_target="[regular file]",
            )

        # Target doesn't exist - will create
        return MappingAction(
            action_type=MappingActionType.CREATE,
            model_id=model_id,
            model_name=model_name,
            source_path=source_path,
            target_path=target_path,
            link_type=link_type,
            reason="New symlink to be created",
        )

    def get_cross_filesystem_warning(self, app_models_root: Path) -> Optional[Dict[str, Any]]:
        """Check if library and app are on different filesystems.

        Args:
            app_models_root: Path to app's models/ directory

        Returns:
            Warning dict if cross-filesystem, None otherwise
        """
        if is_cross_filesystem(self.library.library_root, app_models_root):
            return {
                "cross_filesystem": True,
                "library_path": str(self.library.library_root),
                "app_path": str(app_models_root),
                "warning": (
                    "Your model library is on a different drive than the application. "
                    "Links use absolute paths and will break if the library drive is "
                    "unplugged or the mount point changes."
                ),
                "recommendation": (
                    "Move library to same drive as the application for portable relative symlinks."
                ),
            }
        return None

    def check_mapping_config_exists(self, app_id: str, version: str) -> bool:
        """Check if a mapping config exists for this app version.

        Args:
            app_id: Application ID
            version: Version string

        Returns:
            True if config exists (specific or wildcard)
        """
        if not self.config_root.exists():
            return False

        # Check for specific config
        specific_config = self.config_root / f"{app_id.lower()}_{version}_default.json"
        if specific_config.exists():
            return True

        # Check for wildcard config
        wildcard_config = self.config_root / f"{app_id.lower()}_*_default.json"
        if wildcard_config.exists():
            return True

        return False
