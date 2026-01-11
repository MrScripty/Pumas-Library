#!/usr/bin/env python3
"""
Resource Manager for ComfyUI Version Manager
Main coordinator for shared storage, custom nodes, and model library management
"""

from __future__ import annotations

from dataclasses import asdict
from pathlib import Path
from typing import Dict, List, Optional

from backend.logging_config import get_logger
from backend.metadata_manager import MetadataManager
from backend.model_library import ModelDownloader, ModelImporter, ModelLibrary, ModelMapper
from backend.model_library.link_registry import LinkRegistry
from backend.model_library.search import SearchResult
from backend.models import ModelOverrides, RepairReport, ScanResult
from backend.resources.custom_nodes_manager import CustomNodesManager
from backend.resources.shared_storage import SharedStorageManager
from backend.resources.symlink_manager import SymlinkManager
from backend.utils import ensure_directory, get_directory_size

logger = get_logger(__name__)


class ResourceManager:
    """Manages shared resources, model library, symlinks, and custom nodes"""

    def __init__(self, launcher_root: Path, metadata_manager: MetadataManager):
        self.launcher_root = Path(launcher_root)
        self.metadata_manager = metadata_manager

        # Main directories
        self.shared_dir = self.launcher_root / "shared-resources"
        self.versions_dir = self.launcher_root / "comfyui-versions"

        # Shared resource subdirectories
        self.shared_models_dir = self.shared_dir / "models"
        self.shared_custom_nodes_cache_dir = self.shared_dir / "custom_nodes_cache"
        self.shared_user_dir = self.shared_dir / "user"
        self.shared_workflows_dir = self.shared_user_dir / "workflows"
        self.shared_settings_dir = self.shared_user_dir / "settings"

        # Model translation configs
        self.translation_config_dir = self.metadata_manager.config_dir / "model-library-translation"
        self.translation_config_dir.mkdir(parents=True, exist_ok=True)

        # Initialize specialized managers
        self.storage_mgr = SharedStorageManager(
            self.shared_dir, self.versions_dir, self.launcher_root
        )

        self.symlink_mgr = SymlinkManager(
            self.shared_user_dir,
            self.versions_dir,
            self.launcher_root,
        )

        self.model_library = ModelLibrary(self.shared_models_dir)

        # Link registry for tracking symlinks created by mapper
        self.link_registry_db = self.shared_models_dir / "link_registry.db"
        self.link_registry = LinkRegistry(self.link_registry_db)

        self.model_mapper = ModelMapper(
            self.model_library, self.translation_config_dir, link_registry=self.link_registry
        )
        self.model_importer = ModelImporter(self.model_library)
        self.model_downloader = ModelDownloader(self.model_library)

        self.custom_nodes_mgr = CustomNodesManager(
            self.shared_custom_nodes_cache_dir, self.versions_dir
        )

    # ==================== Shared Storage Operations ====================

    def initialize_shared_storage(self) -> bool:
        """Create shared-resources directory structure."""
        return self.storage_mgr.initialize_shared_storage()

    def scan_shared_storage(self) -> ScanResult:
        """Scan shared storage and update model index."""
        self.model_library.rebuild_index()
        models = self.model_library.list_models()
        models_found = len(models)
        models_size = sum(int(model.get("size_bytes", 0)) for model in models)

        workflows_found = 0
        workflows_size = 0
        if self.shared_workflows_dir.exists():
            workflows_found = len([p for p in self.shared_workflows_dir.iterdir() if p.is_file()])
            workflows_size = get_directory_size(self.shared_workflows_dir)

        return {
            "modelsFound": models_found,
            "workflowsFound": workflows_found,
            "customNodesFound": 0,
            "totalSize": models_size + workflows_size,
        }

    # ==================== Symlink Operations ====================

    def setup_version_symlinks(self, version_tag: str) -> bool:
        """Setup symlinks for a version and apply model mapping."""
        version_path = self.versions_dir / version_tag
        if not version_path.exists():
            logger.error("Error: Version directory not found: %s", version_path)
            return False

        success = self.symlink_mgr.setup_version_symlinks(version_tag)
        self.model_library.rebuild_index()

        models_dir = version_path / "models"
        if models_dir.is_symlink():
            models_dir.unlink()
        ensure_directory(models_dir)

        app_version = version_tag.lstrip("vV")
        try:
            links_created = self.model_mapper.apply_for_app("comfyui", app_version, models_dir)
            logger.info("Mapped %s model links for ComfyUI %s", links_created, app_version)
        except OSError as exc:
            logger.error("Error mapping models for %s: %s", version_tag, exc, exc_info=True)
            success = False
        except RuntimeError as exc:
            logger.error("Error mapping models for %s: %s", version_tag, exc, exc_info=True)
            success = False
        except ValueError as exc:
            logger.error("Error mapping models for %s: %s", version_tag, exc, exc_info=True)
            success = False

        return success

    def validate_and_repair_symlinks(self, version_tag: str) -> RepairReport:
        """Check for broken user symlinks and attempt repair."""
        return self.symlink_mgr.validate_and_repair_symlinks(version_tag)

    # ==================== Model Library Operations ====================

    def get_models(self) -> Dict[str, Dict[str, object]]:
        """Return models indexed in the library."""
        self.model_library.rebuild_index()
        models: Dict[str, Dict[str, object]] = {}
        for metadata in self.model_library.list_models():
            rel_path = metadata.get("library_path")
            if not rel_path:
                continue
            model_type = metadata.get("subtype") or metadata.get("model_type") or "unknown"
            models[rel_path] = {
                "path": rel_path,
                "size": metadata.get("size_bytes"),
                "addedDate": metadata.get("added_date"),
                "modelType": model_type,
                "officialName": metadata.get("official_name"),
                "cleanedName": metadata.get("cleaned_name"),
            }
        return models

    def refresh_model_index(self) -> None:
        """Rebuild the SQLite index from metadata.json files."""
        self.model_library.rebuild_index()

    def refresh_model_mappings(self, app_id: str = "comfyui") -> Dict[str, int]:
        """Apply model mappings for all installed versions."""
        results: Dict[str, int] = {}
        self.model_library.rebuild_index()
        for version_path in sorted(self.versions_dir.iterdir()):
            if not version_path.is_dir() or version_path.name.startswith("."):
                continue

            models_dir = version_path / "models"
            if models_dir.is_symlink():
                models_dir.unlink()
            ensure_directory(models_dir)

            app_version = version_path.name.lstrip("vV")
            links_created = self.model_mapper.apply_for_app(app_id, app_version, models_dir)
            results[version_path.name] = links_created
        return results

    def get_model_overrides(self, rel_path: str) -> ModelOverrides:
        """Load overrides for a model by relative path."""
        model_dir = self.shared_models_dir / rel_path
        if not model_dir.exists():
            return {}
        return self.model_library.load_overrides(model_dir)

    def update_model_overrides(self, rel_path: str, overrides: ModelOverrides) -> bool:
        """Persist overrides for a model by relative path."""
        model_dir = self.shared_models_dir / rel_path
        if not model_dir.exists():
            logger.error("Overrides update failed; model not found: %s", rel_path)
            return False
        self.model_library.save_overrides(model_dir, overrides)
        return True

    def import_model(
        self,
        local_path: Path,
        family: str,
        official_name: str,
        repo_id: Optional[str] = None,
    ) -> Path:
        """Import a local model into the library."""
        return self.model_importer.import_path(local_path, family, official_name, repo_id)

    def download_model_from_hf(
        self,
        repo_id: str,
        family: str,
        official_name: str,
        model_type: Optional[str] = None,
        subtype: str = "",
        quant: Optional[str] = None,
    ) -> Path:
        """Download a model from Hugging Face into the library."""
        return self.model_downloader.download_from_hf(
            repo_id=repo_id,
            family=family,
            official_name=official_name,
            model_type=model_type,
            subtype=subtype,
            quant=quant,
        )

    def start_model_download_from_hf(
        self,
        repo_id: str,
        family: str,
        official_name: str,
        model_type: Optional[str] = None,
        subtype: str = "",
        quant: Optional[str] = None,
    ) -> Dict[str, object]:
        """Start a Hugging Face download and return progress metadata."""
        return self.model_downloader.start_model_download(
            repo_id=repo_id,
            family=family,
            official_name=official_name,
            model_type=model_type,
            subtype=subtype,
            quant=quant,
        )

    def get_model_download_status(self, download_id: str) -> Optional[Dict[str, object]]:
        """Get status for a model download by id."""
        return self.model_downloader.get_model_download_status(download_id)

    def cancel_model_download(self, download_id: str) -> bool:
        """Cancel an active model download."""
        return self.model_downloader.cancel_model_download(download_id)

    def search_hf_models(
        self,
        query: str,
        kind: Optional[str] = None,
        limit: int = 25,
    ) -> List[Dict[str, object]]:
        """Search Hugging Face models for download UI."""
        return self.model_downloader.search_models(query=query, kind=kind, limit=limit)

    def search_models_fts(
        self,
        query: str,
        limit: int = 100,
        offset: int = 0,
        model_type: Optional[str] = None,
        tags: Optional[List[str]] = None,
    ) -> Dict[str, object]:
        """Search local model library using FTS5 full-text search.

        Performs fast full-text search across model metadata including
        names, types, tags, family, and description.

        Args:
            query: Search terms (space-separated for OR matching)
            limit: Maximum number of results to return
            offset: Number of results to skip (for pagination)
            model_type: Filter by model type (e.g., "diffusion", "llm")
            tags: Filter by required tags

        Returns:
            Dict with keys:
                - models: List of matching model metadata
                - total_count: Total number of matches
                - query_time_ms: Query execution time in milliseconds
                - query: The FTS5 query that was executed
        """
        result: SearchResult = self.model_library.search_models(
            terms=query,
            limit=limit,
            offset=offset,
            model_type=model_type,
            tags=tags,
        )
        return asdict(result)

    def import_batch(
        self,
        import_specs: List[Dict[str, str]],
    ) -> Dict[str, object]:
        """Import multiple models in a batch operation.

        Args:
            import_specs: List of import specifications, each containing:
                - path: Local filesystem path to model file or directory
                - family: Model family name
                - official_name: Display name for the model
                - repo_id: Optional Hugging Face repo ID

        Returns:
            Dict with keys:
                - success: Overall success status
                - imported: Number of successfully imported models
                - failed: Number of failed imports
                - results: List of individual import results
        """
        results: List[Dict[str, object]] = []
        imported = 0
        failed = 0

        for spec in import_specs:
            path = spec.get("path", "")
            family = spec.get("family", "unknown")
            official_name = spec.get("official_name", "")
            repo_id = spec.get("repo_id")

            if not path:
                results.append(
                    {
                        "path": path,
                        "success": False,
                        "error": "Missing path",
                    }
                )
                failed += 1
                continue

            try:
                local_path = Path(path)
                if not local_path.exists():
                    results.append(
                        {
                            "path": path,
                            "success": False,
                            "error": "Path does not exist",
                        }
                    )
                    failed += 1
                    continue

                # Use filename as official_name if not provided
                if not official_name:
                    official_name = local_path.stem

                model_dir = self.model_importer.import_path(
                    local_path, family, official_name, repo_id
                )
                results.append(
                    {
                        "path": path,
                        "success": True,
                        "model_path": str(model_dir),
                    }
                )
                imported += 1
            except OSError as exc:
                logger.error("Batch import failed for %s: %s", path, exc, exc_info=True)
                results.append(
                    {
                        "path": path,
                        "success": False,
                        "error": str(exc),
                    }
                )
                failed += 1
            except ValueError as exc:
                logger.error("Batch import failed for %s: %s", path, exc, exc_info=True)
                results.append(
                    {
                        "path": path,
                        "success": False,
                        "error": str(exc),
                    }
                )
                failed += 1

        return {
            "success": failed == 0,
            "imported": imported,
            "failed": failed,
            "results": results,
        }

    # ==================== Link Registry Operations ====================

    def get_link_health(self, app_models_root: Optional[Path] = None) -> Dict[str, object]:
        """Get health status of model symlinks.

        Checks for broken links, orphaned links, and cross-filesystem warnings.

        Args:
            app_models_root: Optional path to check for orphaned links

        Returns:
            Dict with health check results including:
                - status: Overall health status (healthy, warnings, errors)
                - total_links: Total registered links
                - healthy_links: Number of valid links
                - broken_links: List of broken link info
                - orphaned_links: List of orphaned symlink paths
                - warnings: List of warning messages
                - errors: List of error messages
        """
        result = self.link_registry.perform_health_check(app_models_root)
        return {
            "status": result.status.value,
            "total_links": result.total_links,
            "healthy_links": result.healthy_links,
            "broken_links": [
                {
                    "link_id": bl.link_id,
                    "target_path": bl.target_path,
                    "expected_source": bl.expected_source,
                    "model_id": bl.model_id,
                    "reason": bl.reason,
                }
                for bl in result.broken_links
            ],
            "orphaned_links": result.orphaned_links,
            "warnings": result.warnings,
            "errors": result.errors,
        }

    def clean_broken_links(self) -> Dict[str, object]:
        """Remove broken links from the registry and filesystem.

        Returns:
            Dict with cleanup results:
                - success: Whether cleanup completed
                - cleaned: Number of broken links removed
        """
        try:
            cleaned = self.link_registry.clean_broken_links()
            return {"success": True, "cleaned": cleaned}
        except OSError as exc:
            logger.error("Failed to clean broken links: %s", exc, exc_info=True)
            return {"success": False, "error": str(exc), "cleaned": 0}

    def remove_orphaned_links(self, app_models_root: Path) -> Dict[str, object]:
        """Remove orphaned symlinks from the application directory.

        Args:
            app_models_root: Root path to the application's models directory

        Returns:
            Dict with cleanup results:
                - success: Whether cleanup completed
                - removed: Number of orphaned links removed
        """
        try:
            removed = self.link_registry.remove_orphaned_links(app_models_root)
            return {"success": True, "removed": removed}
        except OSError as exc:
            logger.error("Failed to remove orphaned links: %s", exc, exc_info=True)
            return {"success": False, "error": str(exc), "removed": 0}

    def get_links_for_model(self, model_id: str) -> List[Dict[str, object]]:
        """Get all links for a specific model.

        Args:
            model_id: ID of the model

        Returns:
            List of link information dictionaries
        """
        links = self.link_registry.get_links_for_model(model_id)
        return [self.link_registry.to_dict(link) for link in links]

    def delete_model_with_cascade(self, model_id: str) -> Dict[str, object]:
        """Delete a model and all its symlinks.

        Args:
            model_id: ID of the model to delete

        Returns:
            Dict with deletion results:
                - success: Whether deletion completed
                - links_removed: Number of symlinks removed
        """
        try:
            links_removed = self.model_mapper.delete_model_with_cascade(model_id)
            return {"success": True, "links_removed": links_removed}
        except OSError as exc:
            logger.error("Failed to cascade delete model %s: %s", model_id, exc, exc_info=True)
            return {"success": False, "error": str(exc), "links_removed": 0}

    def preview_model_mapping(self, version_tag: str, app_id: str = "comfyui") -> Dict[str, object]:
        """Preview model mapping operations without making changes.

        Args:
            version_tag: ComfyUI version tag
            app_id: Application identifier (default: "comfyui")

        Returns:
            Dict with preview information:
                - to_create: List of links to create
                - conflicts: List of conflicts
                - broken_to_remove: List of broken links to clean
                - warnings: List of warning messages
                - errors: List of error messages
        """
        version_path = self.versions_dir / version_tag
        if not version_path.exists():
            return {"success": False, "error": f"Version {version_tag} not found"}

        models_dir = version_path / "models"
        app_version = version_tag.lstrip("vV")

        preview = self.model_mapper.preview_mapping(app_id, app_version, models_dir)

        # Convert dataclass to dict for JSON serialization
        return {
            "success": True,
            "to_create": [
                {
                    "model_id": a.model_id,
                    "model_name": a.model_name,
                    "source_path": str(a.source_path),
                    "target_path": str(a.target_path),
                    "link_type": a.link_type,
                    "reason": a.reason,
                }
                for a in preview.to_create
            ],
            "to_skip_exists": [
                {
                    "model_id": a.model_id,
                    "model_name": a.model_name,
                    "source_path": str(a.source_path),
                    "target_path": str(a.target_path),
                    "reason": a.reason,
                }
                for a in preview.to_skip_exists
            ],
            "conflicts": [
                {
                    "model_id": a.model_id,
                    "model_name": a.model_name,
                    "source_path": str(a.source_path),
                    "target_path": str(a.target_path),
                    "reason": a.reason,
                    "existing_target": a.existing_target,
                }
                for a in preview.conflicts
            ],
            "broken_to_remove": [
                {
                    "target_path": str(a.target_path),
                    "existing_target": a.existing_target,
                    "reason": a.reason,
                }
                for a in preview.broken_to_remove
            ],
            "total_actions": preview.total_actions,
            "warnings": preview.warnings,
            "errors": preview.errors,
        }

    def sync_models_incremental(
        self,
        version_tag: str,
        model_ids: List[str],
        app_id: str = "comfyui",
    ) -> Dict[str, object]:
        """Incrementally sync specific models to a version.

        Much faster than full sync when only a few models were added.

        Args:
            version_tag: ComfyUI version tag
            model_ids: List of model IDs (library paths) to sync
            app_id: Application identifier (default: "comfyui")

        Returns:
            Dict with sync results:
                - success: Whether sync completed
                - links_created: Number of new links created
                - links_updated: Number of links updated
                - links_skipped: Number of links already correct
        """
        version_path = self.versions_dir / version_tag
        if not version_path.exists():
            return {"success": False, "error": f"Version {version_tag} not found"}

        models_dir = version_path / "models"
        ensure_directory(models_dir)
        app_version = version_tag.lstrip("vV")

        try:
            result = self.model_mapper.sync_models_incremental(
                app_id, app_version, models_dir, model_ids
            )
            return {"success": True, **result}
        except OSError as exc:
            logger.error("Failed incremental sync: %s", exc, exc_info=True)
            return {
                "success": False,
                "error": str(exc),
                "links_created": 0,
                "links_updated": 0,
                "links_skipped": 0,
            }

    def get_cross_filesystem_warning(self, version_tag: str) -> Optional[Dict[str, object]]:
        """Check if library and app version are on different filesystems.

        Args:
            version_tag: ComfyUI version tag

        Returns:
            Warning dict if cross-filesystem, None otherwise
        """
        version_path = self.versions_dir / version_tag
        if not version_path.exists():
            return None

        models_dir = version_path / "models"
        return self.model_mapper.get_cross_filesystem_warning(models_dir)

    # ==================== Custom Nodes Operations ====================

    def get_version_custom_nodes_dir(self, version_tag: str) -> Path:
        return self.custom_nodes_mgr.get_version_custom_nodes_dir(version_tag)

    def list_version_custom_nodes(self, version_tag: str) -> List[str]:
        return self.custom_nodes_mgr.list_version_custom_nodes(version_tag)

    def install_custom_node(
        self, git_url: str, version_tag: str, node_name: Optional[str] = None
    ) -> bool:
        return self.custom_nodes_mgr.install_custom_node(git_url, version_tag, node_name)

    def update_custom_node(self, node_name: str, version_tag: str) -> bool:
        return self.custom_nodes_mgr.update_custom_node(node_name, version_tag)

    def remove_custom_node(self, node_name: str, version_tag: str) -> bool:
        return self.custom_nodes_mgr.remove_custom_node(node_name, version_tag)
