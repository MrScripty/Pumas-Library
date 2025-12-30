"""Auto-updater for the launcher application"""

import json
import logging
import subprocess
from datetime import datetime, timedelta
from pathlib import Path
from typing import TYPE_CHECKING, Any, Callable, Dict, Optional

import requests

logger = logging.getLogger(__name__)

if TYPE_CHECKING:
    from backend.metadata_manager import MetadataManager


class LauncherUpdater:
    """Manages launcher updates via git"""

    def __init__(
        self,
        metadata_manager: "MetadataManager",
        repo_owner: str = "MrScripty",
        repo_name: str = "Linux-ComfyUI-Launcher",
    ):
        self.metadata = metadata_manager
        self.repo_owner = repo_owner
        self.repo_name = repo_name
        self.repo_url = f"https://github.com/{repo_owner}/{repo_name}.git"
        self.api_url = f"https://api.github.com/repos/{repo_owner}/{repo_name}"
        self.launcher_root: Path = Path(__file__).parent.parent
        self.cache_file = (
            self.launcher_root / "launcher-data" / "cache" / "launcher-update-check.json"
        )

    def check_for_updates(self, force_refresh: bool = False) -> Dict[str, Any]:
        """
        Check if updates are available on GitHub

        Returns:
            {
                'hasUpdate': bool,
                'currentCommit': str,
                'latestCommit': str,
                'commitsBehind': int,
                'commits': List[Dict],  # Recent commit info
                'branch': str,
                'error': Optional[str]
            }
        """
        try:
            # Get current commit
            current_commit = self._get_current_commit()
            if not current_commit:
                return {"hasUpdate": False, "error": "Not a git repository"}

            # Check upstream main branch for updates
            current_branch = "main"

            # Check cache first (unless force refresh)
            if not force_refresh:
                cached_data = self._get_cached_update_info()
                if cached_data:
                    cache_time = datetime.fromisoformat(cached_data["lastChecked"])
                    if datetime.now() - cache_time < timedelta(hours=1):
                        logger.info("Using cached update info")
                        cached_result = cached_data.get("result")
                        if isinstance(cached_result, dict):
                            return cached_result

            # Fetch latest commits from GitHub API
            commits_url = f"{self.api_url}/commits"
            params = {"sha": current_branch, "per_page": 10}

            logger.info(f"Fetching commits from {commits_url}")
            response = requests.get(commits_url, params=params, timeout=10)
            response.raise_for_status()

            commits = response.json()

            if not commits:
                return {"hasUpdate": False, "error": "No commits found"}

            latest_commit = commits[0]["sha"][:7]

            # Check if we're behind
            has_update = latest_commit != current_commit

            # Count how many commits behind
            commits_behind = 0
            commit_list = []

            for commit in commits:
                commit_sha = commit["sha"][:7]
                commit_list.append(
                    {
                        "sha": commit_sha,
                        "message": commit["commit"]["message"].split("\n")[0],  # First line only
                        "author": commit["commit"]["author"]["name"],
                        "date": commit["commit"]["author"]["date"],
                    }
                )

                if commit_sha == current_commit:
                    break
                commits_behind += 1

            result = {
                "hasUpdate": has_update,
                "currentCommit": current_commit,
                "latestCommit": latest_commit,
                "commitsBehind": commits_behind,
                "commits": commit_list,
                "branch": current_branch,
            }

            # Cache the result
            self._cache_update_info(result)

            logger.info(f"Update check complete: hasUpdate={has_update}, behind={commits_behind}")
            return result

        except requests.RequestException as e:
            logger.error(f"Network error during update check: {e}")
            return {"hasUpdate": False, "error": f"Network error: {str(e)}"}
        except (json.JSONDecodeError, KeyError, TypeError, ValueError) as e:
            logger.error(f"Update check failed: {e}")
            return {"hasUpdate": False, "error": f"Update check failed: {str(e)}"}

    def apply_update(self, progress_callback: Optional[Callable] = None) -> Dict[str, Any]:
        """
        Pull latest changes and rebuild the application

        Args:
            progress_callback: Optional callback for progress updates

        Returns:
            {
                'success': bool,
                'message': str,
                'newCommit': Optional[str],
                'error': Optional[str]
            }
        """
        try:
            # Safety checks
            if not self.is_git_repo():
                return {"success": False, "error": "Not a git repository"}

            # Check for uncommitted changes
            if self.has_uncommitted_changes():
                return {
                    "success": False,
                    "error": "Uncommitted changes detected. Please commit or stash them first.",
                }

            # Step 1: Backup current state (record commit)
            current_commit = self._get_current_commit()
            if not current_commit:
                return {"success": False, "error": "Unable to determine current commit"}
            logger.info(f"Starting update from commit {current_commit}")

            if progress_callback:
                progress_callback(
                    {"stage": "backup", "message": "Recording current version...", "percent": 10}
                )

            # Step 2: Pull latest changes
            if progress_callback:
                progress_callback(
                    {"stage": "pull", "message": "Fetching updates from GitHub...", "percent": 20}
                )

            logger.info("Running git pull")
            pull_result = subprocess.run(
                ["git", "pull", "origin", self._get_current_branch()],
                cwd=self.launcher_root,
                capture_output=True,
                text=True,
                timeout=60,
            )

            if pull_result.returncode != 0:
                error_msg = f"Git pull failed: {pull_result.stderr}"
                logger.error(error_msg)
                return {"success": False, "error": error_msg}

            # Check if anything was actually updated
            if (
                "Already up to date" in pull_result.stdout
                or "Already up-to-date" in pull_result.stdout
            ):
                logger.info("Already up to date")
                return {
                    "success": True,
                    "message": "Already up to date",
                    "newCommit": current_commit,
                }

            new_commit = self._get_current_commit()
            logger.info(f"Updated to commit {new_commit}")

            # Step 3: Update Python dependencies (in case requirements.txt changed)
            if progress_callback:
                progress_callback(
                    {
                        "stage": "dependencies",
                        "message": "Updating Python dependencies...",
                        "percent": 40,
                    }
                )

            logger.info("Updating Python dependencies")
            pip_result = subprocess.run(
                ["pip", "install", "-r", "requirements.txt", "--upgrade"],
                cwd=self.launcher_root,
                capture_output=True,
                text=True,
                timeout=300,
            )

            if pip_result.returncode != 0:
                logger.warning(f"pip install warning: {pip_result.stderr}")
                # Don't fail on pip warnings, just log them

            # Step 4: Update frontend dependencies
            if progress_callback:
                progress_callback(
                    {"stage": "npm", "message": "Updating frontend dependencies...", "percent": 60}
                )

            frontend_dir = self.launcher_root / "frontend"
            logger.info("Running npm install")
            npm_install = subprocess.run(
                ["npm", "install"], cwd=frontend_dir, capture_output=True, text=True, timeout=300
            )

            if npm_install.returncode != 0:
                logger.warning(f"npm install warning: {npm_install.stderr}")

            # Step 5: Rebuild frontend
            if progress_callback:
                progress_callback(
                    {"stage": "build", "message": "Rebuilding frontend...", "percent": 80}
                )

            logger.info("Running npm build")
            build_result = subprocess.run(
                ["npm", "run", "build"],
                cwd=frontend_dir,
                capture_output=True,
                text=True,
                timeout=300,
            )

            if build_result.returncode != 0:
                # Rollback on build failure
                logger.error(f"Frontend build failed: {build_result.stderr}")
                self._rollback(current_commit)
                return {
                    "success": False,
                    "error": f"Frontend build failed. Rolled back to {current_commit}.",
                }

            if progress_callback:
                progress_callback(
                    {"stage": "complete", "message": "Update complete!", "percent": 100}
                )

            logger.info("Update completed successfully")
            return {
                "success": True,
                "message": "Update applied successfully. Please restart the launcher.",
                "newCommit": new_commit,
                "previousCommit": current_commit,
            }

        except subprocess.TimeoutExpired as e:
            logger.error(f"Update timed out: {e}")
            return {"success": False, "error": "Update timed out"}
        except (OSError, RuntimeError, TypeError, ValueError, subprocess.SubprocessError) as e:
            logger.error(f"Update failed: {e}")
            return {"success": False, "error": f"Update failed: {str(e)}"}

    def _rollback(self, commit_sha: str):
        """Rollback to a previous commit"""
        try:
            logger.warning(f"Rolling back to commit {commit_sha}")
            subprocess.run(
                ["git", "reset", "--hard", commit_sha],
                cwd=self.launcher_root,
                timeout=30,
                check=True,
            )
            logger.info("Rollback successful")
        except (
            subprocess.CalledProcessError,
            FileNotFoundError,
            OSError,
            subprocess.SubprocessError,
        ) as e:
            logger.error(f"Rollback failed: {e}")

    def _get_current_commit(self) -> Optional[str]:
        """Get current git commit SHA (short)"""
        try:
            result = subprocess.run(
                ["git", "rev-parse", "--short=7", "HEAD"],
                cwd=self.launcher_root,
                capture_output=True,
                text=True,
                timeout=5,
            )
            if result.returncode == 0:
                return result.stdout.strip()
        except (subprocess.SubprocessError, FileNotFoundError, OSError) as e:
            logger.error(f"Failed to get current commit: {e}")
        return None

    def _get_current_branch(self) -> str:
        """Get current git branch"""
        try:
            result = subprocess.run(
                ["git", "rev-parse", "--abbrev-ref", "HEAD"],
                cwd=self.launcher_root,
                capture_output=True,
                text=True,
                timeout=5,
            )
            if result.returncode == 0:
                return result.stdout.strip()
        except (subprocess.SubprocessError, FileNotFoundError, OSError):
            pass
        return "main"

    def _get_cached_update_info(self) -> Optional[Dict[str, Any]]:
        """Get cached update check result"""
        if self.cache_file.exists():
            try:
                with open(self.cache_file, "r") as f:
                    data = json.load(f)
                    if isinstance(data, dict):
                        last_checked = data.get("lastChecked")
                        result = data.get("result")
                        if isinstance(last_checked, str) and isinstance(result, dict):
                            return {"lastChecked": last_checked, "result": result}
            except (json.JSONDecodeError, OSError, ValueError) as e:
                logger.warning(f"Failed to read cache: {e}")
        return None

    def _cache_update_info(self, result: Dict[str, Any]) -> None:
        """Cache update check result"""
        self.cache_file.parent.mkdir(parents=True, exist_ok=True)

        cache_data = {"lastChecked": datetime.now().isoformat(), "result": result}

        try:
            with open(self.cache_file, "w") as f:
                json.dump(cache_data, f, indent=2)
        except (OSError, TypeError, ValueError) as e:
            logger.warning(f"Failed to write cache: {e}")

    def is_git_repo(self) -> bool:
        """Check if launcher is in a git repository"""
        git_dir = self.launcher_root / ".git"
        return git_dir.exists()

    def has_uncommitted_changes(self) -> bool:
        """Check if there are uncommitted changes"""
        try:
            result = subprocess.run(
                ["git", "status", "--porcelain"],
                cwd=self.launcher_root,
                capture_output=True,
                text=True,
                timeout=5,
            )
            has_changes = bool(result.stdout.strip())
            if has_changes:
                logger.warning("Uncommitted changes detected")
            return has_changes
        except (subprocess.SubprocessError, FileNotFoundError, OSError) as e:
            logger.error(f"Failed to check git status: {e}")
            return False
