"""Dependency management helpers for VersionManager."""

from __future__ import annotations

import json
import os
import re
import shutil
import subprocess
from pathlib import Path
from typing import Callable, Optional

try:
    import psutil
except ImportError:  # pragma: no cover - optional dependency
    psutil = None

from packaging.utils import canonicalize_name

from backend.config import INSTALLATION
from backend.logging_config import get_logger
from backend.models import DependencyStatus
from backend.process_io_tracker import ProcessIOTracker
from backend.utils import ensure_directory, parse_requirements_file, run_command, safe_filename
from backend.validators import validate_version_tag
from backend.version_manager_components.protocols import DependenciesContext, MixinBase

logger = get_logger(__name__)


class DependenciesMixin(MixinBase, DependenciesContext):
    """Mix-in for dependency creation, inspection, and installation."""

    _current_process: Optional[subprocess.Popen[str]]
    _cancel_installation: bool

    def _get_process_io_bytes(self, pid: int, include_children: bool = True) -> Optional[int]:
        """Return total read+write bytes for a process (and optionally its children)."""
        if not psutil or not pid:
            return None
        try:
            proc = psutil.Process(pid)
            procs = [proc]
            if include_children:
                procs += proc.children(recursive=True)
            total = 0
            for proc_item in procs:
                try:
                    io = proc_item.io_counters()
                    total += io.read_bytes + io.write_bytes
                except (AttributeError, OSError):
                    continue
            return total
        except (AttributeError, OSError):
            return None

    def _build_pip_env(self) -> dict[str, str]:
        """Build environment variables for pip commands, ensuring cache is shared."""
        env = os.environ.copy()
        cache_dir = self.pip_cache_dir
        if ensure_directory(cache_dir):
            env["PIP_CACHE_DIR"] = str(cache_dir)
            self.active_pip_cache_dir = cache_dir
        else:
            logger.warning(f"Unable to create pip cache directory at {cache_dir}")
            self.active_pip_cache_dir = self.pip_cache_dir
        return env

    def _create_space_safe_requirements(
        self, tag: str, requirements_file: Optional[Path], constraints_path: Optional[Path]
    ) -> tuple[Optional[Path], Optional[Path]]:
        """Copy requirements/constraints to a cache dir without spaces."""
        if not requirements_file and not constraints_path:
            return None, None

        safe_dir = self.metadata_manager.cache_dir / "requirements-safe"
        try:
            safe_dir.mkdir(parents=True, exist_ok=True)
        except (OSError, PermissionError) as exc:
            logger.warning(f"Could not create safe requirements dir: {exc}")
            return requirements_file, constraints_path

        safe_tag = safe_filename(tag) or "req"
        safe_req = None
        safe_constraints = None

        try:
            if requirements_file and requirements_file.exists():
                safe_req = safe_dir / f"{safe_tag}-requirements.txt"
                shutil.copyfile(requirements_file, safe_req)
        except (IOError, OSError) as exc:
            logger.warning(f"Could not copy requirements.txt to safe path: {exc}")
            safe_req = requirements_file

        try:
            if constraints_path and constraints_path.exists():
                safe_constraints = safe_dir / f"{safe_tag}-constraints.txt"
                shutil.copyfile(constraints_path, safe_constraints)
        except (IOError, OSError) as exc:
            logger.warning(f"Could not copy constraints to safe path: {exc}")
            safe_constraints = constraints_path

        return safe_req or requirements_file, safe_constraints or constraints_path

    def _get_global_required_packages(self) -> list[str]:
        """Packages that must be installed in every ComfyUI venv."""
        return ["setproctitle"]

    def _create_venv(self, version_path: Path) -> bool:
        """Create virtual environment for a version using python3."""
        venv_path = version_path / "venv"

        logger.info("Creating virtual environment with python3...")
        pip_env = self._build_pip_env()
        success, stdout, stderr = run_command(
            ["python3", "-m", "venv", str(venv_path)],
            timeout=INSTALLATION.VENV_CREATION_TIMEOUT_SEC,
            env=pip_env,
        )

        if not success:
            logger.error(f"Failed to create venv: {stderr}")
            return False

        venv_python = venv_path / "bin" / "python"
        if venv_python.exists():
            run_command([str(venv_python), "-m", "ensurepip", "--upgrade"], env=pip_env)
            run_command([str(venv_python), "-m", "pip", "install", "--upgrade", "pip"], env=pip_env)

        logger.info("✓ Virtual environment created")
        return True

    def _get_python_version(self, version_path: Path) -> str:
        """Get Python version for a version's venv."""
        venv_python = version_path / "venv" / "bin" / "python"

        if not venv_python.exists():
            return "unknown"

        success, stdout, _stderr = run_command(
            [str(venv_python), "--version"], timeout=INSTALLATION.SUBPROCESS_QUICK_TIMEOUT_SEC
        )

        if success:
            return stdout.strip()

        return "unknown"

    def check_dependencies(self, tag: str) -> DependencyStatus:
        """Check dependency installation status for a version."""
        if not validate_version_tag(tag):
            logger.error(f"Invalid version tag for dependency check: {tag!r}")
            return {"installed": [], "missing": [], "requirementsFile": None}
        version_path = self.versions_dir / tag

        if not version_path.exists():
            return {"installed": [], "missing": [], "requirementsFile": None}

        requirements_file = version_path / "requirements.txt"
        requirements_file_rel = (
            str(requirements_file.relative_to(self.launcher_root))
            if requirements_file.exists()
            else None
        )

        requirements = (
            parse_requirements_file(requirements_file) if requirements_file.exists() else {}
        )

        optional_requirements: set[str] = set()
        if requirements_file.exists():
            try:
                optional_mode = False
                with open(requirements_file, "r", encoding="utf-8") as f:
                    for line in f:
                        raw = line.strip()
                        if not raw:
                            continue
                        if raw.startswith("#"):
                            if raw.lower().startswith("#non essential dependencies"):
                                optional_mode = True
                            continue
                        if raw.startswith("-"):
                            continue
                        if optional_mode:
                            pkg = (
                                raw.split("==")[0]
                                .split(">=")[0]
                                .split("<=")[0]
                                .split("<")[0]
                                .split(">")[0]
                                .split("@")[0]
                                .strip()
                            )
                            if pkg:
                                optional_requirements.add(canonicalize_name(pkg))
            except (IOError, OSError, KeyError, ValueError) as exc:
                logger.warning(
                    f"Could not parse optional dependencies in {requirements_file}: {exc}"
                )

        global_required = self._get_global_required_packages()
        existing_canon = {canonicalize_name(pkg) for pkg in requirements}
        for pkg in global_required:
            canon = canonicalize_name(pkg)
            if canon not in existing_canon:
                requirements[pkg] = ""
                existing_canon.add(canon)

        venv_python = version_path / "venv" / "bin" / "python"

        if not venv_python.exists():
            return {
                "installed": [],
                "missing": list(requirements.keys()),
                "requirementsFile": requirements_file_rel,
            }

        pip_env = self._build_pip_env()
        pip_ok, _stdout, _stderr = run_command(
            [str(venv_python), "-m", "pip", "--version"],
            timeout=INSTALLATION.SUBPROCESS_STANDARD_TIMEOUT_SEC,
            env=pip_env,
        )
        if not pip_ok:
            run_command([str(venv_python), "-m", "ensurepip", "--upgrade"], env=pip_env)
            run_command([str(venv_python), "-m", "pip", "install", "--upgrade", "pip"], env=pip_env)

        installed: list[str] = []
        missing: list[str] = []

        installed_names = self._get_installed_package_names(tag, venv_python)
        if installed_names is None:
            logger.warning(
                f"Could not inspect installed packages for {tag}, treating dependencies as missing"
            )
            installed_names = set()

        for package in requirements.keys():
            canon = canonicalize_name(package)
            if canon in optional_requirements:
                continue
            if canon in installed_names:
                installed.append(package)
            else:
                missing.append(package)

        return {
            "installed": installed,
            "missing": missing,
            "requirementsFile": requirements_file_rel,
        }

    def _get_installed_package_names(self, tag: str, venv_python: Path) -> Optional[set[str]]:
        """Inspect installed packages in the version venv."""
        installed_names: set[str] = set()
        pip_env = self._build_pip_env()
        errors: list[str] = []

        success, stdout, stderr = run_command(
            [str(venv_python), "-m", "pip", "list", "--format=json"],
            timeout=INSTALLATION.SUBPROCESS_STANDARD_TIMEOUT_SEC,
            env=pip_env,
        )

        if success:
            try:
                parsed = json.loads(stdout)
                installed_names = {
                    canonicalize_name(pkg.get("name", "")) for pkg in parsed if pkg.get("name")
                }
                return installed_names
            except (json.JSONDecodeError, KeyError, ValueError) as exc:
                errors.append(f"pip json parse: {exc}")
                logger.error(f"Error parsing pip list JSON for {tag}: {exc}", exc_info=True)
        else:
            errors.append(f"pip json: {stderr}")

        success, stdout, stderr = run_command(
            [str(venv_python), "-m", "pip", "list", "--format=freeze"],
            timeout=INSTALLATION.SUBPROCESS_STANDARD_TIMEOUT_SEC,
            env=pip_env,
        )
        if success:
            for line in stdout.splitlines():
                line = line.strip()
                if not line:
                    continue
                pkg = line.split("==")[0].split("@")[0].strip()
                if pkg:
                    installed_names.add(canonicalize_name(pkg))
            return installed_names
        else:
            errors.append(f"pip freeze: {stderr}")

        error_msg = "; ".join([msg for msg in errors if msg]) or "unknown error"
        logger.warning(f"Dependency inspection failed for {tag}: {error_msg}")
        return None

    def install_dependencies(
        self, tag: str, progress_callback: Optional[Callable[[str], None]] = None
    ) -> bool:
        """Install dependencies for a version."""
        if not validate_version_tag(tag):
            logger.error(f"Invalid version tag for dependency install: {tag!r}")
            return False
        version_path = self.versions_dir / tag

        if not version_path.exists():
            logger.error(f"Version {tag} not found")
            return False

        requirements_file = version_path / "requirements.txt"

        if not requirements_file.exists():
            logger.info(
                f"No requirements.txt found for {tag} (will still install global dependencies)"
            )

        venv_python = version_path / "venv" / "bin" / "python"

        if not venv_python.exists():
            logger.info(f"Virtual environment not found for {tag}; creating...")
            if not self._create_venv(version_path):
                return False
            venv_python = version_path / "venv" / "bin" / "python"
            if not venv_python.exists():
                logger.error(f"Virtual environment not found for {tag}")
                return False

        logger.info(f"Installing dependencies for {tag}...")

        if progress_callback:
            progress_callback("Installing Python packages...")

        global_required = self._get_global_required_packages()
        constraints_path = None
        try:
            release = self.github_fetcher.get_release_by_tag(tag)
        except (KeyError, ValueError, TypeError):
            release = None
        if requirements_file.exists():
            constraints_path = self._build_constraints_for_tag(tag, requirements_file, release)
            if constraints_path:
                logger.info(f"Using pinned constraints for {tag}: {constraints_path}")
                self._log_install(f"Using constraints file: {constraints_path}")

        pip_env = self._build_pip_env()
        safe_req, safe_constraints = self._create_space_safe_requirements(
            tag, requirements_file if requirements_file.exists() else None, constraints_path
        )

        run_command([str(venv_python), "-m", "ensurepip", "--upgrade"], env=pip_env)
        run_command([str(venv_python), "-m", "pip", "install", "--upgrade", "pip"], env=pip_env)

        install_cmd = [str(venv_python), "-m", "pip", "install"]
        if safe_req:
            install_cmd += ["-r", str(Path(safe_req))]
        if safe_constraints:
            install_cmd += ["-c", str(Path(safe_constraints))]
        install_cmd += global_required

        success, stdout, stderr = run_command(
            install_cmd, timeout=INSTALLATION.PIP_FALLBACK_TIMEOUT_SEC, env=pip_env
        )

        if success:
            logger.info("✓ Dependencies installed successfully")
            if stdout:
                logger.debug(stdout)
                self._log_install(stdout)
            return True

        logger.error(f"Dependency installation failed: {stderr}")
        self._log_install(f"pip dependency install failed: {stderr}")
        return False

    def _install_dependencies_with_progress(self, tag: str) -> bool:
        """Install Python dependencies with real-time progress tracking."""
        if not validate_version_tag(tag):
            logger.error(f"Invalid version tag for dependency install: {tag!r}")
            return False
        version_path = self.versions_dir / tag

        if not version_path.exists():
            logger.error(f"Version {tag} not found")
            return False

        requirements_file = version_path / "requirements.txt"

        venv_python = version_path / "venv" / "bin" / "python"

        if not venv_python.exists():
            logger.info(f"Virtual environment not found for {tag}; creating...")
            if not self._create_venv(version_path):
                return False
            venv_python = version_path / "venv" / "bin" / "python"
            if not venv_python.exists():
                logger.error(f"Virtual environment not found for {tag}")
                return False

        requirements = (
            parse_requirements_file(requirements_file) if requirements_file.exists() else {}
        )
        global_required = self._get_global_required_packages()
        constraints_path = None
        try:
            release = self.github_fetcher.get_release_by_tag(tag)
        except (KeyError, ValueError, TypeError):
            release = None
        if requirements_file.exists():
            constraints_path = self._build_constraints_for_tag(tag, requirements_file, release)
            if constraints_path:
                self._log_install(f"Using constraints for {tag}: {constraints_path}")

        existing_canon = {canonicalize_name(pkg) for pkg in requirements}
        extra_global = []
        for pkg in global_required:
            canon = canonicalize_name(pkg)
            if canon not in existing_canon:
                extra_global.append(pkg)
                existing_canon.add(canon)

        package_entries = list(requirements.items()) + [(pkg, "") for pkg in extra_global]
        package_count = len(package_entries)

        logger.info(f"Installing {package_count} dependencies for {tag}...")

        package_specs = [f"{pkg}{ver}" if ver else pkg for pkg, ver in package_entries]
        self.progress_tracker.set_dependency_weights(package_specs)

        current_state = self.progress_tracker.get_current_state()
        if current_state:
            current_state["dependency_count"] = package_count
            self.progress_tracker.update_dependency_progress("Preparing...", 0, package_count)

        logger.info("Starting dependency installation...")
        pip_env = self._build_pip_env()
        safe_req, safe_constraints = self._create_space_safe_requirements(
            tag, requirements_file if requirements_file.exists() else None, constraints_path
        )

        run_command([str(venv_python), "-m", "ensurepip", "--upgrade"], env=pip_env)
        run_command([str(venv_python), "-m", "pip", "install", "--upgrade", "pip"], env=pip_env)

        cache_dir = self.active_pip_cache_dir or self.pip_cache_dir
        pip_stdout = ""
        pip_stderr = ""
        pip_cmd = [str(venv_python), "-m", "pip", "install"]
        if safe_req:
            pip_cmd += ["-r", str(Path(safe_req))]
        if safe_constraints:
            pip_cmd += ["-c", str(Path(safe_constraints))]
        pip_cmd += extra_global

        self._current_process = subprocess.Popen(
            pip_cmd,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            text=True,
            bufsize=1,
            preexec_fn=os.setsid if hasattr(os, "setsid") else None,
            env=pip_env,
        )

        io_tracker = ProcessIOTracker(
            pid=self._current_process.pid if self._current_process else None,
            cache_dir=cache_dir,
            io_bytes_getter=self._get_process_io_bytes,
        )

        completed_packages: set[str] = set()
        current_package = None

        try:
            while self._current_process.poll() is None:
                import select

                if self._current_process.stdout:
                    readable, _, _ = select.select([self._current_process.stdout], [], [], 0.1)

                    if readable:
                        line = self._current_process.stdout.readline()
                        if line:
                            pip_stdout += line
                            line_lower = line.lower()

                            if "collecting" in line_lower:
                                match = re.search(
                                    r"collecting\s+([a-zA-Z0-9_-]+)", line, re.IGNORECASE
                                )
                                if match:
                                    current_package = match.group(1)
                                    self.progress_tracker.update_dependency_progress(
                                        f"Collecting {current_package}",
                                        len(completed_packages),
                                        package_count,
                                    )

                            elif "downloading" in line_lower and current_package:
                                size_match = re.search(r"\(([0-9.]+\s*[KMG]?B)\)", line)
                                if size_match:
                                    size_str = size_match.group(1)
                                    self.progress_tracker.update_dependency_progress(
                                        f"Downloading {current_package} ({size_str})",
                                        len(completed_packages),
                                        package_count,
                                    )

                            elif "successfully installed" in line_lower:
                                match = re.search(
                                    r"successfully installed\s+(.+)", line, re.IGNORECASE
                                )
                                if match:
                                    packages_str = match.group(1).strip()
                                    for pkg_ver in packages_str.split():
                                        pkg_name = pkg_ver.split("-")[0]
                                        if pkg_name and pkg_name.lower() not in completed_packages:
                                            completed_packages.add(pkg_name.lower())
                                            self.progress_tracker.complete_package(pkg_name)
                                            self.progress_tracker.add_completed_item(
                                                pkg_name, "package"
                                            )

                if io_tracker.should_update(min_interval_sec=0.75):
                    downloaded, speed = io_tracker.get_download_metrics()

                    if downloaded is not None:
                        current_state = self.progress_tracker.get_current_state()
                        if current_state:
                            current_state["downloaded_bytes"] = downloaded
                            if speed is not None:
                                current_state["download_speed"] = speed

                if self._cancel_installation:
                    raise InterruptedError("Installation cancelled during dependency installation")

            remaining_output, _ = self._current_process.communicate()
            if remaining_output:
                pip_stdout += remaining_output

            success = self._current_process.returncode == 0
            pip_stderr = ""

        except InterruptedError:
            raise
        except (subprocess.SubprocessError, OSError) as exc:
            logger.error(f"Error during dependency installation: {exc}", exc_info=True)
            if self._current_process:
                try:
                    if hasattr(os, "killpg"):
                        os.killpg(os.getpgid(self._current_process.pid), 9)
                    else:
                        self._current_process.kill()
                except (OSError, PermissionError):
                    pass
            return False
        finally:
            self._current_process = None

        if success:
            for package, _ in package_entries:
                if package.lower() not in completed_packages:
                    self.progress_tracker.complete_package(package)
                    self.progress_tracker.add_completed_item(package, "package")
                    completed_packages.add(package.lower())

            self.progress_tracker.update_dependency_progress(
                "Complete", package_count, package_count
            )

            logger.info("✓ Dependencies installed successfully")
            if pip_stdout:
                self._log_install(pip_stdout)
            return True

        error_msg = f"Dependency installation failed via pip: {pip_stderr[:500]}"
        logger.error(error_msg)
        self._log_install(error_msg)
        self.progress_tracker.set_error(error_msg)
        return False
