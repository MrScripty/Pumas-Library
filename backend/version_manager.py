#!/usr/bin/env python3
"""
Version Manager for ComfyUI
Handles installation, switching, and launching of ComfyUI versions
"""

import json
import shutil
import tarfile
import zipfile
import subprocess
from pathlib import Path
from typing import Optional, List, Callable, Dict, Tuple
from backend.models import (
    VersionsMetadata, VersionInfo, VersionConfig, DependencyStatus,
    GitHubRelease, get_iso_timestamp
)
from backend.metadata_manager import MetadataManager
from backend.github_integration import GitHubReleasesFetcher, DownloadManager
from backend.resource_manager import ResourceManager
from backend.utils import (
    ensure_directory, run_command, check_command_exists,
    parse_requirements_file
)
from backend.installation_progress_tracker import (
    InstallationProgressTracker,
    InstallationStage
)


class VersionManager:
    """Manages ComfyUI version installation, switching, and launching"""

    def __init__(
        self,
        launcher_root: Path,
        metadata_manager: MetadataManager,
        github_fetcher: GitHubReleasesFetcher,
        resource_manager: ResourceManager
    ):
        """
        Initialize VersionManager

        Args:
            launcher_root: Root directory for the launcher
            metadata_manager: MetadataManager instance
            github_fetcher: GitHubReleasesFetcher instance
            resource_manager: ResourceManager instance
        """
        self.launcher_root = Path(launcher_root)
        self.metadata_manager = metadata_manager
        self.github_fetcher = github_fetcher
        self.resource_manager = resource_manager

        # Directories
        self.versions_dir = self.launcher_root / "comfyui-versions"
        self.active_version_file = self.launcher_root / ".active-version"

        # Ensure versions directory exists
        ensure_directory(self.versions_dir)

        # Initialize progress tracker (Phase 6.2.5b)
        cache_dir = metadata_manager.launcher_data_dir / "cache"
        self.progress_tracker = InstallationProgressTracker(cache_dir)

    def get_installation_progress(self) -> Optional[Dict]:
        """
        Get current installation progress (Phase 6.2.5b)

        Returns:
            Progress state dict or None if no installation in progress
        """
        return self.progress_tracker.get_current_state()

    def get_available_releases(self, force_refresh: bool = False) -> List[GitHubRelease]:
        """
        Get available ComfyUI releases from GitHub

        Args:
            force_refresh: Force refresh from GitHub

        Returns:
            List of GitHubRelease objects
        """
        return self.github_fetcher.get_releases(force_refresh)

    def get_installed_versions(self) -> List[str]:
        """
        Get list of installed version tags

        Returns:
            List of version tags
        """
        versions_metadata = self.metadata_manager.load_versions()
        return list(versions_metadata.get('installed', {}).keys())

    def get_version_info(self, tag: str) -> Optional[VersionInfo]:
        """
        Get info about an installed version

        Args:
            tag: Version tag

        Returns:
            VersionInfo or None if not installed
        """
        versions_metadata = self.metadata_manager.load_versions()
        return versions_metadata.get('installed', {}).get(tag)

    def get_active_version(self) -> Optional[str]:
        """
        Get currently active version tag

        Returns:
            Active version tag or None
        """
        if self.active_version_file.exists():
            try:
                return self.active_version_file.read_text().strip()
            except Exception as e:
                print(f"Error reading active version file: {e}")

        # Fallback to metadata
        versions_metadata = self.metadata_manager.load_versions()
        return versions_metadata.get('lastSelectedVersion')

    def set_active_version(self, tag: str) -> bool:
        """
        Set a version as active

        Args:
            tag: Version tag to activate

        Returns:
            True if successful
        """
        # Verify version is installed
        if tag not in self.get_installed_versions():
            print(f"Version {tag} is not installed")
            return False

        # Validate symlinks
        print(f"Validating symlinks for {tag}...")
        repair_report = self.resource_manager.validate_and_repair_symlinks(tag)

        if repair_report['broken']:
            print(f"Warning: Found {len(repair_report['broken'])} broken symlinks")
            print(f"Repaired: {len(repair_report['repaired'])}, Removed: {len(repair_report['removed'])}")

        # Update active version file
        try:
            self.active_version_file.write_text(tag)
        except Exception as e:
            print(f"Error writing active version file: {e}")
            return False

        # Update metadata
        versions_metadata = self.metadata_manager.load_versions()
        versions_metadata['lastSelectedVersion'] = tag

        self.metadata_manager.save_versions(versions_metadata)

        print(f"✓ Activated version: {tag}")
        return True

    def install_version(
        self,
        tag: str,
        progress_callback: Optional[Callable[[str, int, int], None]] = None
    ) -> bool:
        """
        Install a ComfyUI version (Enhanced with Phase 6.2.5b progress tracking)

        Args:
            tag: Version tag to install
            progress_callback: Optional callback(message, current, total)

        Returns:
            True if successful
        """
        # Check if already installed
        if tag in self.get_installed_versions():
            print(f"Version {tag} is already installed")
            return False

        # Get release info
        release = self.github_fetcher.get_release_by_tag(tag)
        if not release:
            print(f"Release {tag} not found")
            return False

        print(f"Installing ComfyUI {tag}...")

        version_path = self.versions_dir / tag
        if version_path.exists():
            print(f"Version directory already exists: {version_path}")
            return False

        try:
            # Initialize progress tracking
            self.progress_tracker.start_installation(tag)

            # Step 1: Download release
            self.progress_tracker.update_stage(InstallationStage.DOWNLOAD, 0, f"Downloading {tag}")
            if progress_callback:
                progress_callback("Downloading release...", 1, 5)

            download_url = release.get('zipball_url') or release.get('tarball_url')
            if not download_url:
                error_msg = "No download URL found in release"
                print(error_msg)
                self.progress_tracker.set_error(error_msg)
                return False

            # Determine archive type
            is_zip = 'zipball' in download_url

            # Download to temporary file
            download_dir = self.launcher_root / "temp"
            ensure_directory(download_dir)

            archive_ext = '.zip' if is_zip else '.tar.gz'
            archive_path = download_dir / f"{tag}{archive_ext}"

            # TODO: Enhance DownloadManager to provide progress callbacks
            # For now, just update progress at start and end
            downloader = DownloadManager()
            success = downloader.download_with_retry(download_url, archive_path)

            if not success:
                error_msg = "Download failed"
                print(error_msg)
                self.progress_tracker.set_error(error_msg)
                return False

            # Get archive size
            archive_size = archive_path.stat().st_size
            self.progress_tracker.update_download_progress(archive_size, archive_size)
            self.progress_tracker.add_completed_item(archive_path.name, 'archive', archive_size)

            # Step 2: Extract archive
            self.progress_tracker.update_stage(InstallationStage.EXTRACT, 0, "Extracting archive")
            if progress_callback:
                progress_callback("Extracting archive...", 2, 5)

            print(f"Extracting {archive_path.name}...")
            temp_extract_dir = download_dir / f"extract-{tag}"
            ensure_directory(temp_extract_dir)

            if is_zip:
                with zipfile.ZipFile(archive_path, 'r') as zip_ref:
                    zip_ref.extractall(temp_extract_dir)
            else:
                with tarfile.open(archive_path, 'r:gz') as tar_ref:
                    tar_ref.extractall(temp_extract_dir)

            self.progress_tracker.update_stage(InstallationStage.EXTRACT, 100, "Extraction complete")

            # GitHub archives extract to a subdirectory, find it
            extracted_contents = list(temp_extract_dir.iterdir())
            if len(extracted_contents) == 1 and extracted_contents[0].is_dir():
                actual_dir = extracted_contents[0]
            else:
                actual_dir = temp_extract_dir

            # Move to final location
            ensure_directory(self.versions_dir)
            shutil.move(str(actual_dir), str(version_path))

            # Clean up
            archive_path.unlink()
            if temp_extract_dir.exists():
                shutil.rmtree(temp_extract_dir)

            # Step 3: Create venv with UV
            self.progress_tracker.update_stage(InstallationStage.VENV, 0, "Creating virtual environment")
            if progress_callback:
                progress_callback("Creating virtual environment...", 3, 5)

            if not self._create_venv(version_path):
                error_msg = "Failed to create virtual environment"
                print(error_msg)
                self.progress_tracker.set_error(error_msg)
                shutil.rmtree(version_path)
                return False

            self.progress_tracker.update_stage(InstallationStage.VENV, 100, "Virtual environment created")

            # Step 4: Install dependencies with progress tracking
            self.progress_tracker.update_stage(InstallationStage.DEPENDENCIES, 0, "Installing dependencies")
            if progress_callback:
                progress_callback("Installing dependencies...", 4, 5)

            if not self._install_dependencies_with_progress(tag):
                print("Warning: Dependency installation had errors")
                # Continue anyway, user can retry later

            # Step 5: Setup symlinks
            self.progress_tracker.update_stage(InstallationStage.SETUP, 0, "Setting up symlinks")
            if progress_callback:
                progress_callback("Setting up symlinks...", 5, 5)

            self.resource_manager.setup_version_symlinks(tag)
            self.progress_tracker.update_stage(InstallationStage.SETUP, 100, "Setup complete")

            # Update metadata
            version_info: VersionInfo = {
                'path': str(version_path.relative_to(self.launcher_root)),
                'installedDate': get_iso_timestamp(),
                'pythonVersion': self._get_python_version(version_path),
                'releaseTag': tag
            }

            versions_metadata = self.metadata_manager.load_versions()
            if 'installed' not in versions_metadata:
                versions_metadata['installed'] = {}

            versions_metadata['installed'][tag] = version_info
            self.metadata_manager.save_versions(versions_metadata)

            # Mark installation as complete
            self.progress_tracker.complete_installation(True)

            print(f"✓ Successfully installed {tag}")
            return True

        except Exception as e:
            error_msg = f"Error installing version {tag}: {e}"
            print(error_msg)
            self.progress_tracker.set_error(error_msg)
            self.progress_tracker.complete_installation(False)

            # Clean up on failure
            if version_path.exists():
                shutil.rmtree(version_path)
            return False
        finally:
            # Clear progress state after a short delay (allow UI to read final state)
            import time
            time.sleep(2)
            self.progress_tracker.clear_state()

    def _create_venv(self, version_path: Path) -> bool:
        """
        Create virtual environment for a version using UV

        Args:
            version_path: Path to version directory

        Returns:
            True if successful
        """
        # Check if UV is installed
        if not check_command_exists('uv'):
            print("UV package manager not found. Attempting to install...")
            # Try to install UV
            success, stdout, stderr = run_command(
                ['pip', 'install', 'uv'],
                timeout=60
            )
            if not success:
                print("Failed to install UV. Please install it manually:")
                print("  pip install uv")
                return False

        venv_path = version_path / "venv"

        print(f"Creating virtual environment with UV...")
        success, stdout, stderr = run_command(
            ['uv', 'venv', str(venv_path)],
            timeout=120
        )

        if not success:
            print(f"Failed to create venv: {stderr}")
            return False

        print("✓ Virtual environment created")
        return True

    def _get_python_version(self, version_path: Path) -> str:
        """
        Get Python version for a version's venv

        Args:
            version_path: Path to version directory

        Returns:
            Python version string or "unknown"
        """
        venv_python = version_path / "venv" / "bin" / "python"

        if not venv_python.exists():
            return "unknown"

        success, stdout, stderr = run_command(
            [str(venv_python), '--version'],
            timeout=5
        )

        if success:
            # Output is like "Python 3.11.7"
            return stdout.strip()

        return "unknown"

    def check_dependencies(self, tag: str) -> DependencyStatus:
        """
        Check dependency installation status for a version

        Args:
            tag: Version tag

        Returns:
            DependencyStatus with installed/missing packages
        """
        version_path = self.versions_dir / tag

        if not version_path.exists():
            return {
                'installed': [],
                'missing': [],
                'requirementsFile': None
            }

        requirements_file = version_path / "requirements.txt"

        if not requirements_file.exists():
            return {
                'installed': [],
                'missing': [],
                'requirementsFile': None
            }

        # Parse requirements
        requirements = parse_requirements_file(requirements_file)

        venv_python = version_path / "venv" / "bin" / "python"

        if not venv_python.exists():
            # No venv, all packages are missing
            return {
                'installed': [],
                'missing': list(requirements.keys()),
                'requirementsFile': str(requirements_file.relative_to(self.launcher_root))
            }

        # Check which packages are installed
        installed = []
        missing = []

        for package in requirements.keys():
            # Use pip list to check if package is installed
            success, stdout, stderr = run_command(
                [str(venv_python), '-m', 'pip', 'list', '--format=freeze'],
                timeout=30
            )

            if success:
                # Check if package is in the output
                package_lower = package.lower()
                found = any(
                    line.lower().startswith(package_lower + '==')
                    for line in stdout.split('\n')
                )

                if found:
                    installed.append(package)
                else:
                    missing.append(package)
            else:
                # If we can't check, assume missing
                missing.append(package)

        return {
            'installed': installed,
            'missing': missing,
            'requirementsFile': str(requirements_file.relative_to(self.launcher_root))
        }

    def install_dependencies(
        self,
        tag: str,
        progress_callback: Optional[Callable[[str], None]] = None
    ) -> bool:
        """
        Install dependencies for a version

        Args:
            tag: Version tag
            progress_callback: Optional callback for progress messages

        Returns:
            True if successful
        """
        version_path = self.versions_dir / tag

        if not version_path.exists():
            print(f"Version {tag} not found")
            return False

        requirements_file = version_path / "requirements.txt"

        if not requirements_file.exists():
            print(f"No requirements.txt found for {tag}")
            return True  # No requirements is not an error

        venv_python = version_path / "venv" / "bin" / "python"

        if not venv_python.exists():
            print(f"Virtual environment not found for {tag}")
            return False

        print(f"Installing dependencies for {tag}...")

        if progress_callback:
            progress_callback("Installing Python packages...")

        # Use UV to install requirements
        success, stdout, stderr = run_command(
            [
                'uv', 'pip', 'install',
                '-r', str(requirements_file),
                '--python', str(venv_python)
            ],
            timeout=600  # 10 minute timeout for large installs
        )

        if success:
            print("✓ Dependencies installed successfully")
            if stdout:
                print(stdout)
            return True
        else:
            print(f"Error installing dependencies: {stderr}")
            return False

    def _install_dependencies_with_progress(self, tag: str) -> bool:
        """
        Install dependencies with detailed progress tracking (Phase 6.2.5b)

        Args:
            tag: Version tag

        Returns:
            True if successful
        """
        version_path = self.versions_dir / tag

        if not version_path.exists():
            print(f"Version {tag} not found")
            return False

        requirements_file = version_path / "requirements.txt"

        if not requirements_file.exists():
            print(f"No requirements.txt found for {tag}")
            return True  # No requirements is not an error

        venv_python = version_path / "venv" / "bin" / "python"

        if not venv_python.exists():
            print(f"Virtual environment not found for {tag}")
            return False

        # Parse requirements to get package list
        requirements = parse_requirements_file(requirements_file)
        package_count = len(requirements)

        print(f"Installing {package_count} dependencies for {tag}...")

        # Update progress tracker with dependency count
        current_state = self.progress_tracker.get_current_state()
        if current_state:
            current_state['dependency_count'] = package_count
            self.progress_tracker.update_dependency_progress(
                "Preparing...",
                0,
                package_count
            )

        # Use UV to install requirements
        # Note: UV doesn't provide per-package callbacks, so we track overall progress
        success, stdout, stderr = run_command(
            [
                'uv', 'pip', 'install',
                '-r', str(requirements_file),
                '--python', str(venv_python)
            ],
            timeout=600  # 10 minute timeout for large installs
        )

        if success:
            # Mark all dependencies as completed
            for i, (package, version_spec) in enumerate(requirements.items(), 1):
                self.progress_tracker.update_dependency_progress(
                    f"{package}{version_spec}",
                    i,
                    package_count
                )
                self.progress_tracker.add_completed_item(package, 'package')

            print("✓ Dependencies installed successfully")
            if stdout:
                print(stdout)
            return True
        else:
            print(f"Error installing dependencies: {stderr}")
            return False

    def remove_version(self, tag: str) -> bool:
        """
        Remove an installed version

        Args:
            tag: Version tag to remove

        Returns:
            True if successful
        """
        if tag not in self.get_installed_versions():
            print(f"Version {tag} is not installed")
            return False

        # Check if it's the active version
        if self.get_active_version() == tag:
            print(f"Cannot remove active version {tag}")
            print("Please switch to a different version first")
            return False

        version_path = self.versions_dir / tag

        try:
            # Remove directory
            print(f"Removing {tag}...")
            shutil.rmtree(version_path)

            # Update metadata
            versions_metadata = self.metadata_manager.load_versions()

            if tag in versions_metadata.get('installed', {}):
                del versions_metadata['installed'][tag]

            self.metadata_manager.save_versions(versions_metadata)

            print(f"✓ Removed version {tag}")
            return True

        except Exception as e:
            print(f"Error removing version {tag}: {e}")
            return False

    def launch_version(
        self,
        tag: str,
        extra_args: Optional[List[str]] = None
    ) -> Tuple[bool, Optional[subprocess.Popen]]:
        """
        Launch a ComfyUI version

        Args:
            tag: Version tag to launch
            extra_args: Optional extra arguments for main.py

        Returns:
            Tuple of (success, process) - process is None if launch failed
        """
        if tag not in self.get_installed_versions():
            print(f"Version {tag} is not installed")
            return (False, None)

        # Set as active version
        if not self.set_active_version(tag):
            print("Failed to activate version")
            return (False, None)

        # Check dependencies
        dep_status = self.check_dependencies(tag)
        if dep_status['missing']:
            print(f"Warning: {len(dep_status['missing'])} missing dependencies")
            print("Install dependencies with install_dependencies() first")
            # Continue anyway - user may want to run with missing deps

        # Validate symlinks
        repair_report = self.resource_manager.validate_and_repair_symlinks(tag)
        if repair_report['broken']:
            print(f"Warning: Repaired {len(repair_report['repaired'])} broken symlinks")

        version_path = self.versions_dir / tag
        main_py = version_path / "main.py"

        if not main_py.exists():
            print(f"main.py not found in {tag}")
            return (False, None)

        venv_python = version_path / "venv" / "bin" / "python"

        if not venv_python.exists():
            print(f"Virtual environment not found for {tag}")
            return (False, None)

        # Build command
        cmd = [str(venv_python), str(main_py)]

        if extra_args:
            cmd.extend(extra_args)

        print(f"Launching ComfyUI {tag}...")
        print(f"Command: {' '.join(cmd)}")

        try:
            # Launch as subprocess (non-blocking)
            process = subprocess.Popen(
                cmd,
                cwd=str(version_path),
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                text=True
            )

            print(f"✓ ComfyUI {tag} started (PID: {process.pid})")
            print("Use the returned process object to monitor or terminate")

            return (True, process)

        except Exception as e:
            print(f"Error launching ComfyUI: {e}")
            return (False, None)

    def get_version_status(self) -> Dict[str, any]:
        """
        Get comprehensive status of all versions

        Returns:
            Dict with version status information
        """
        installed = self.get_installed_versions()
        active = self.get_active_version()

        status = {
            'installedCount': len(installed),
            'activeVersion': active,
            'versions': {}
        }

        for tag in installed:
            version_info = self.get_version_info(tag)
            dep_status = self.check_dependencies(tag)

            status['versions'][tag] = {
                'info': version_info,
                'dependencies': dep_status,
                'isActive': tag == active
            }

        return status


if __name__ == "__main__":
    # For testing - demonstrate version manager
    from backend.utils import get_launcher_root

    launcher_root = get_launcher_root()
    launcher_data_dir = launcher_root / "launcher-data"

    # Initialize components
    metadata_mgr = MetadataManager(launcher_data_dir)
    github_fetcher = GitHubReleasesFetcher(metadata_mgr)
    resource_mgr = ResourceManager(launcher_root, metadata_mgr)
    version_mgr = VersionManager(launcher_root, metadata_mgr, github_fetcher, resource_mgr)

    print("=== ComfyUI Version Manager ===\n")

    # Get available releases
    print("Fetching available releases...")
    releases = version_mgr.get_available_releases()
    print(f"Found {len(releases)} releases\n")

    # Show installed versions
    installed = version_mgr.get_installed_versions()
    print(f"Installed versions: {len(installed)}")
    for tag in installed:
        info = version_mgr.get_version_info(tag)
        print(f"  - {tag} (installed: {info['installDate']})")

    # Show active version
    active = version_mgr.get_active_version()
    if active:
        print(f"\nActive version: {active}")
    else:
        print("\nNo active version")
