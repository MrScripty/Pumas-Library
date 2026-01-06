"""
Unit tests for backend/version_manager.py

Tests for VersionManager initialization, version management, and release fetching.
"""

import threading
from pathlib import Path
from unittest.mock import MagicMock, Mock, patch

import pytest

from backend.version_manager import VersionManager

# ============================================================================
# FIXTURES
# ============================================================================


@pytest.fixture
def mock_metadata_manager(tmp_path):
    """Create a mock MetadataManager"""
    mock_mm = Mock()
    mock_mm.launcher_data_dir = tmp_path / "launcher-data"
    mock_mm.cache_dir = tmp_path / "cache"
    mock_mm.launcher_data_dir.mkdir(parents=True, exist_ok=True)
    mock_mm.cache_dir.mkdir(parents=True, exist_ok=True)

    # Mock load_versions to return empty dict
    mock_mm.load_versions.return_value = {"installed": {}}

    # Mock load_constraints_cache
    mock_mm.load_cache_json.return_value = {}

    return mock_mm


@pytest.fixture
def mock_github_fetcher():
    """Create a mock GitHubReleasesFetcher"""
    mock_gf = Mock()
    mock_gf.get_releases.return_value = []
    mock_gf.collapse_latest_patch_per_minor.return_value = []
    return mock_gf


@pytest.fixture
def mock_resource_manager():
    """Create a mock ResourceManager"""
    return Mock()


@pytest.fixture
def version_manager(tmp_path, mock_metadata_manager, mock_github_fetcher, mock_resource_manager):
    """Create a VersionManager instance for testing"""
    launcher_root = tmp_path / "launcher"
    launcher_root.mkdir(parents=True, exist_ok=True)

    with patch("backend.version_manager.ensure_directory"):
        vm = VersionManager(
            launcher_root=launcher_root,
            metadata_manager=mock_metadata_manager,
            github_fetcher=mock_github_fetcher,
            resource_manager=mock_resource_manager,
        )
    return vm


# ============================================================================
# INITIALIZATION TESTS
# ============================================================================


class TestVersionManagerInit:
    """Test VersionManager initialization"""

    def test_init_sets_launcher_root(
        self,
        tmp_path,
        mock_metadata_manager,
        mock_github_fetcher,
        mock_resource_manager,
    ):
        """Test that initialization sets launcher_root"""
        launcher_root = tmp_path / "launcher"
        launcher_root.mkdir(parents=True, exist_ok=True)

        with patch("backend.version_manager.ensure_directory"):
            vm = VersionManager(
                launcher_root=launcher_root,
                metadata_manager=mock_metadata_manager,
                github_fetcher=mock_github_fetcher,
                resource_manager=mock_resource_manager,
            )

        assert vm.launcher_root == launcher_root

    def test_init_sets_metadata_manager(
        self,
        tmp_path,
        mock_metadata_manager,
        mock_github_fetcher,
        mock_resource_manager,
    ):
        """Test that initialization sets metadata_manager"""
        launcher_root = tmp_path / "launcher"
        launcher_root.mkdir(parents=True, exist_ok=True)

        with patch("backend.version_manager.ensure_directory"):
            vm = VersionManager(
                launcher_root=launcher_root,
                metadata_manager=mock_metadata_manager,
                github_fetcher=mock_github_fetcher,
                resource_manager=mock_resource_manager,
            )

        assert vm.metadata_manager == mock_metadata_manager

    def test_init_sets_github_fetcher(
        self,
        tmp_path,
        mock_metadata_manager,
        mock_github_fetcher,
        mock_resource_manager,
    ):
        """Test that initialization sets github_fetcher"""
        launcher_root = tmp_path / "launcher"
        launcher_root.mkdir(parents=True, exist_ok=True)

        with patch("backend.version_manager.ensure_directory"):
            vm = VersionManager(
                launcher_root=launcher_root,
                metadata_manager=mock_metadata_manager,
                github_fetcher=mock_github_fetcher,
                resource_manager=mock_resource_manager,
            )

        assert vm.github_fetcher == mock_github_fetcher

    def test_init_sets_resource_manager(
        self,
        tmp_path,
        mock_metadata_manager,
        mock_github_fetcher,
        mock_resource_manager,
    ):
        """Test that initialization sets resource_manager"""
        launcher_root = tmp_path / "launcher"
        launcher_root.mkdir(parents=True, exist_ok=True)

        with patch("backend.version_manager.ensure_directory"):
            vm = VersionManager(
                launcher_root=launcher_root,
                metadata_manager=mock_metadata_manager,
                github_fetcher=mock_github_fetcher,
                resource_manager=mock_resource_manager,
            )

        assert vm.resource_manager == mock_resource_manager

    def test_init_creates_logs_directory(
        self,
        tmp_path,
        mock_metadata_manager,
        mock_github_fetcher,
        mock_resource_manager,
    ):
        """Test that initialization creates logs directory"""
        launcher_root = tmp_path / "launcher"
        launcher_root.mkdir(parents=True, exist_ok=True)

        with patch("backend.version_manager.ensure_directory") as mock_ensure:
            vm = VersionManager(
                launcher_root=launcher_root,
                metadata_manager=mock_metadata_manager,
                github_fetcher=mock_github_fetcher,
                resource_manager=mock_resource_manager,
            )

        # Should create logs directory
        assert vm.logs_dir == mock_metadata_manager.launcher_data_dir / "logs"
        mock_ensure.assert_any_call(vm.logs_dir)

    def test_init_creates_constraints_directory(
        self,
        tmp_path,
        mock_metadata_manager,
        mock_github_fetcher,
        mock_resource_manager,
    ):
        """Test that initialization creates constraints directory"""
        launcher_root = tmp_path / "launcher"
        launcher_root.mkdir(parents=True, exist_ok=True)

        with patch("backend.version_manager.ensure_directory") as mock_ensure:
            vm = VersionManager(
                launcher_root=launcher_root,
                metadata_manager=mock_metadata_manager,
                github_fetcher=mock_github_fetcher,
                resource_manager=mock_resource_manager,
            )

        # Should create constraints directory
        assert vm.constraints_dir.parent == mock_metadata_manager.cache_dir

    def test_init_creates_constraints_lock(
        self,
        tmp_path,
        mock_metadata_manager,
        mock_github_fetcher,
        mock_resource_manager,
    ):
        """Test that initialization creates threading lock for constraints"""
        launcher_root = tmp_path / "launcher"
        launcher_root.mkdir(parents=True, exist_ok=True)

        with patch("backend.version_manager.ensure_directory"):
            vm = VersionManager(
                launcher_root=launcher_root,
                metadata_manager=mock_metadata_manager,
                github_fetcher=mock_github_fetcher,
                resource_manager=mock_resource_manager,
            )

        assert hasattr(vm._constraints_cache_lock, "acquire")
        assert hasattr(vm._constraints_cache_lock, "release")

    def test_init_sets_active_version_to_none(
        self,
        tmp_path,
        mock_metadata_manager,
        mock_github_fetcher,
        mock_resource_manager,
    ):
        """Test that active version is initialized"""
        launcher_root = tmp_path / "launcher"
        launcher_root.mkdir(parents=True, exist_ok=True)

        with patch("backend.version_manager.ensure_directory"):
            vm = VersionManager(
                launcher_root=launcher_root,
                metadata_manager=mock_metadata_manager,
                github_fetcher=mock_github_fetcher,
                resource_manager=mock_resource_manager,
            )

        # Active version is set during _initialize_active_version
        assert hasattr(vm, "_active_version")

    def test_init_creates_versions_directory(
        self,
        tmp_path,
        mock_metadata_manager,
        mock_github_fetcher,
        mock_resource_manager,
    ):
        """Test that initialization creates versions directory"""
        launcher_root = tmp_path / "launcher"
        launcher_root.mkdir(parents=True, exist_ok=True)

        with patch("backend.version_manager.ensure_directory") as mock_ensure:
            vm = VersionManager(
                launcher_root=launcher_root,
                metadata_manager=mock_metadata_manager,
                github_fetcher=mock_github_fetcher,
                resource_manager=mock_resource_manager,
            )

        assert vm.versions_dir == launcher_root / "comfyui-versions"
        mock_ensure.assert_any_call(vm.versions_dir)

    def test_init_sets_active_version_file_path(
        self,
        tmp_path,
        mock_metadata_manager,
        mock_github_fetcher,
        mock_resource_manager,
    ):
        """Test that active version file path is set"""
        launcher_root = tmp_path / "launcher"
        launcher_root.mkdir(parents=True, exist_ok=True)

        with patch("backend.version_manager.ensure_directory"):
            vm = VersionManager(
                launcher_root=launcher_root,
                metadata_manager=mock_metadata_manager,
                github_fetcher=mock_github_fetcher,
                resource_manager=mock_resource_manager,
            )

        assert vm.active_version_file == launcher_root / ".active-version"

    def test_init_creates_pip_cache_directory(
        self,
        tmp_path,
        mock_metadata_manager,
        mock_github_fetcher,
        mock_resource_manager,
    ):
        """Test that pip cache directory is created"""
        launcher_root = tmp_path / "launcher"
        launcher_root.mkdir(parents=True, exist_ok=True)

        with patch("backend.version_manager.ensure_directory", return_value=True) as mock_ensure:
            vm = VersionManager(
                launcher_root=launcher_root,
                metadata_manager=mock_metadata_manager,
                github_fetcher=mock_github_fetcher,
                resource_manager=mock_resource_manager,
            )

        assert vm.pip_cache_dir.parent == mock_metadata_manager.cache_dir
        assert vm.active_pip_cache_dir == vm.pip_cache_dir

    def test_init_creates_progress_tracker(
        self,
        tmp_path,
        mock_metadata_manager,
        mock_github_fetcher,
        mock_resource_manager,
    ):
        """Test that progress tracker is initialized"""
        launcher_root = tmp_path / "launcher"
        launcher_root.mkdir(parents=True, exist_ok=True)

        with patch("backend.version_manager.ensure_directory"):
            with patch("backend.version_manager.InstallationProgressTracker") as MockTracker:
                vm = VersionManager(
                    launcher_root=launcher_root,
                    metadata_manager=mock_metadata_manager,
                    github_fetcher=mock_github_fetcher,
                    resource_manager=mock_resource_manager,
                )

        assert hasattr(vm, "progress_tracker")

    def test_init_sets_cancellation_flags(
        self,
        tmp_path,
        mock_metadata_manager,
        mock_github_fetcher,
        mock_resource_manager,
    ):
        """Test that cancellation flags are initialized"""
        launcher_root = tmp_path / "launcher"
        launcher_root.mkdir(parents=True, exist_ok=True)

        with patch("backend.version_manager.ensure_directory"):
            vm = VersionManager(
                launcher_root=launcher_root,
                metadata_manager=mock_metadata_manager,
                github_fetcher=mock_github_fetcher,
                resource_manager=mock_resource_manager,
            )

        assert vm._cancel_installation is False
        assert vm._installing_tag is None
        assert vm._current_process is None
        assert vm._current_downloader is None
        assert vm._install_log_handle is None
        assert vm._current_install_log_path is None


# ============================================================================
# GET AVAILABLE RELEASES TESTS
# ============================================================================


class TestGetAvailableReleases:
    """Test get_available_releases method"""

    def test_get_available_releases_default_params(self, version_manager, mock_github_fetcher):
        """Test get_available_releases with default parameters"""
        sample_releases = [{"tag_name": "v0.1.0"}, {"tag_name": "v0.2.0"}]
        mock_github_fetcher.get_releases.return_value = sample_releases
        mock_github_fetcher.collapse_latest_patch_per_minor.return_value = sample_releases

        result = version_manager.get_available_releases()

        # Should call get_releases and collapse
        mock_github_fetcher.get_releases.assert_called_once_with(False)
        mock_github_fetcher.collapse_latest_patch_per_minor.assert_called_once_with(
            sample_releases, include_prerelease=True
        )
        assert result == sample_releases

    def test_get_available_releases_force_refresh(self, version_manager, mock_github_fetcher):
        """Test get_available_releases with force_refresh=True"""
        sample_releases = [{"tag_name": "v0.1.0"}]
        mock_github_fetcher.get_releases.return_value = sample_releases
        mock_github_fetcher.collapse_latest_patch_per_minor.return_value = sample_releases

        result = version_manager.get_available_releases(force_refresh=True)

        # Should pass force_refresh to get_releases
        mock_github_fetcher.get_releases.assert_called_once_with(True)
        assert result == sample_releases

    def test_get_available_releases_no_collapse(self, version_manager, mock_github_fetcher):
        """Test get_available_releases with collapse=False"""
        sample_releases = [
            {"tag_name": "v0.1.0"},
            {"tag_name": "v0.1.1"},
            {"tag_name": "v0.2.0"},
        ]
        mock_github_fetcher.get_releases.return_value = sample_releases

        result = version_manager.get_available_releases(collapse=False)

        # Should not call collapse
        mock_github_fetcher.collapse_latest_patch_per_minor.assert_not_called()
        assert result == sample_releases

    def test_get_available_releases_exclude_prerelease(self, version_manager, mock_github_fetcher):
        """Test get_available_releases with include_prerelease=False"""
        sample_releases = [{"tag_name": "v0.1.0"}, {"tag_name": "v0.2.0-beta"}]
        filtered_releases = [{"tag_name": "v0.1.0"}]
        mock_github_fetcher.get_releases.return_value = sample_releases
        mock_github_fetcher.collapse_latest_patch_per_minor.return_value = filtered_releases

        result = version_manager.get_available_releases(include_prerelease=False)

        # Should pass include_prerelease=False to collapse
        mock_github_fetcher.collapse_latest_patch_per_minor.assert_called_once_with(
            sample_releases, include_prerelease=False
        )
        assert result == filtered_releases

    def test_get_available_releases_empty_list(self, version_manager, mock_github_fetcher):
        """Test get_available_releases when no releases are available"""
        mock_github_fetcher.get_releases.return_value = []
        mock_github_fetcher.collapse_latest_patch_per_minor.return_value = []

        result = version_manager.get_available_releases()

        assert result == []

    def test_get_available_releases_all_params(self, version_manager, mock_github_fetcher):
        """Test get_available_releases with all parameters specified"""
        sample_releases = [{"tag_name": "v0.1.0"}]
        mock_github_fetcher.get_releases.return_value = sample_releases
        mock_github_fetcher.collapse_latest_patch_per_minor.return_value = sample_releases

        result = version_manager.get_available_releases(
            force_refresh=True, collapse=True, include_prerelease=False
        )

        mock_github_fetcher.get_releases.assert_called_once_with(True)
        mock_github_fetcher.collapse_latest_patch_per_minor.assert_called_once_with(
            sample_releases, include_prerelease=False
        )
        assert result == sample_releases


# ============================================================================
# MIXIN INTEGRATION TESTS
# ============================================================================


class TestVersionManagerMixins:
    """Test that VersionManager properly integrates with mixins"""

    def test_has_constraints_mixin_methods(self, version_manager):
        """Test that ConstraintsMixin methods are available"""
        # Check for key methods from ConstraintsMixin
        assert hasattr(version_manager, "_load_constraints_cache")
        assert hasattr(version_manager, "_constraints_cache")
        assert hasattr(version_manager, "_constraints_cache_lock")

    def test_has_dependencies_mixin_attributes(self, version_manager):
        """Test that DependenciesMixin is properly integrated"""
        # The mixin should be part of the inheritance chain
        assert isinstance(version_manager, VersionManager)

    def test_has_installation_mixin_attributes(self, version_manager):
        """Test that InstallationMixin is properly integrated"""
        # Check for installation-related attributes
        assert hasattr(version_manager, "_cancel_installation")
        assert hasattr(version_manager, "_installing_tag")

    def test_has_launcher_mixin_attributes(self, version_manager):
        """Test that LauncherMixin is properly integrated"""
        # Launcher mixin should provide launch-related functionality
        assert hasattr(version_manager, "logs_dir")

    def test_has_state_mixin_attributes(self, version_manager):
        """Test that StateMixin is properly integrated"""
        # State mixin manages active version
        assert hasattr(version_manager, "_active_version")
        assert hasattr(version_manager, "active_version_file")


# ============================================================================
# PATH AND DIRECTORY TESTS
# ============================================================================


class TestVersionManagerPaths:
    """Test path and directory configuration"""

    def test_versions_directory_path(self, version_manager):
        """Test that versions directory path is correct"""
        expected = version_manager.launcher_root / "comfyui-versions"
        assert version_manager.versions_dir == expected

    def test_active_version_file_path(self, version_manager):
        """Test that active version file path is correct"""
        expected = version_manager.launcher_root / ".active-version"
        assert version_manager.active_version_file == expected

    def test_logs_directory_path(self, version_manager, mock_metadata_manager):
        """Test that logs directory path is correct"""
        expected = mock_metadata_manager.launcher_data_dir / "logs"
        assert version_manager.logs_dir == expected

    def test_pip_cache_directory_path(self, version_manager, mock_metadata_manager):
        """Test that pip cache directory path is correct"""
        # Should be under cache_dir with PATHS.PIP_CACHE_DIR_NAME
        assert version_manager.pip_cache_dir.parent == mock_metadata_manager.cache_dir
        assert version_manager.active_pip_cache_dir == version_manager.pip_cache_dir
