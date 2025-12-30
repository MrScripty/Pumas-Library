"""Constraint resolution helpers for VersionManager."""

from __future__ import annotations

import json
from datetime import datetime, timezone
from pathlib import Path
from typing import Dict, Optional
from urllib import error as url_error
from urllib import request as url_request

from packaging.specifiers import SpecifierSet
from packaging.utils import canonicalize_name
from packaging.version import Version

from backend.config import INSTALLATION
from backend.file_utils import atomic_write_json
from backend.logging_config import get_logger
from backend.models import GitHubRelease
from backend.utils import parse_requirements_file, safe_filename
from backend.version_manager_components.protocols import ConstraintsContext, MixinBase

logger = get_logger(__name__)


class ConstraintsMixin(MixinBase, ConstraintsContext):
    """Mix-in for resolving and caching dependency constraints."""

    def _get_release_date(self, tag: str, release: Optional[GitHubRelease]) -> Optional[datetime]:
        """Return release date in UTC for a tag."""
        if not release:
            return None
        published_at = release.get("published_at")
        if not published_at:
            return None
        try:
            if isinstance(published_at, str):
                ts = published_at.replace("Z", "+00:00")
                return datetime.fromisoformat(ts).astimezone(timezone.utc)
        except (ValueError, TypeError) as exc:
            logger.warning(f"Could not parse release date for {tag}: {exc}")
        return None

    def _get_constraints_path(self, tag: str) -> Path:
        """Path to the cached constraints file for a tag."""
        safe_tag = safe_filename(tag) or "unknown"
        return self.constraints_dir / f"{safe_tag}.txt"

    def _fetch_pypi_versions(self, package: str) -> Dict[str, datetime]:
        """Fetch release versions and upload times for a package from PyPI."""
        canon = canonicalize_name(package)
        if canon in self._pypi_release_cache:
            return self._pypi_release_cache[canon]

        url = f"https://pypi.org/pypi/{package}/json"
        try:
            with url_request.urlopen(url, timeout=INSTALLATION.URL_FETCH_TIMEOUT_SEC) as resp:
                data = json.load(resp)
        except (url_error.URLError, json.JSONDecodeError, KeyError) as exc:
            logger.warning(f"Failed to fetch PyPI data for {package}: {exc}")
            return {}

        releases = data.get("releases", {})
        result: Dict[str, datetime] = {}

        for version_str, files in releases.items():
            upload_times = []
            for file_entry in files or []:
                upload_time = file_entry.get("upload_time_iso_8601") or file_entry.get(
                    "upload_time"
                )
                if upload_time:
                    try:
                        upload_times.append(
                            datetime.fromisoformat(upload_time.replace("Z", "+00:00")).astimezone(
                                timezone.utc
                            )
                        )
                    except (ValueError, TypeError):
                        continue
            if upload_times:
                result[version_str] = max(upload_times)

        self._pypi_release_cache[canon] = result
        return result

    def _select_version_for_date(
        self, package: str, spec: str, release_date: Optional[datetime]
    ) -> Optional[str]:
        """Choose the newest version that satisfies the spec and release date."""
        releases = self._fetch_pypi_versions(package)
        if not releases:
            return None

        try:
            spec_set = SpecifierSet(spec) if spec else SpecifierSet()
        except (ValueError, TypeError) as exc:
            logger.warning(f"Invalid specifier for {package} ({spec}): {exc}")
            spec_set = SpecifierSet()

        candidates = []
        for ver_str, uploaded_at in releases.items():
            try:
                ver = Version(ver_str)
            except (ValueError, TypeError):
                continue
            if spec_set and ver not in spec_set:
                continue
            if release_date and uploaded_at and uploaded_at > release_date:
                continue
            candidates.append((ver, ver_str))

        if not candidates:
            return None

        candidates.sort()
        return candidates[-1][1]

    def _build_constraints_for_tag(
        self, tag: str, requirements_file: Path, release: Optional[GitHubRelease]
    ) -> Optional[Path]:
        """Build a constraints file when requirements are not fully pinned."""
        constraints_path = self._get_constraints_path(tag)
        if constraints_path.exists():
            return constraints_path

        if not requirements_file.exists():
            return None

        requirements = parse_requirements_file(requirements_file)
        unpinned = {
            pkg: spec for pkg, spec in requirements.items() if not spec or not spec.startswith("==")
        }
        if not unpinned:
            return None

        release_date = self._get_release_date(tag, release)
        resolved: Dict[str, str] = {}

        for pkg, spec in unpinned.items():
            version_str = self._select_version_for_date(pkg, spec, release_date)
            if version_str:
                resolved[pkg] = f"=={version_str}"
            else:
                resolved[pkg] = spec or ""
                logger.warning(f"Unable to resolve pinned version for {pkg} (spec: '{spec}')")

        combined: Dict[str, str] = {}
        for pkg, spec in requirements.items():
            combined[pkg] = resolved.get(pkg, spec if spec else "")

        try:
            with open(constraints_path, "w", encoding="utf-8") as f:
                for pkg, spec in combined.items():
                    if spec:
                        f.write(f"{pkg}{spec}\n")
                    else:
                        f.write(f"{pkg}\n")
        except (IOError, OSError) as exc:
            logger.warning(f"Failed to write constraints file for {tag}: {exc}")
            return None

        self._constraints_cache[tag] = combined
        self._save_constraints_cache()

        return constraints_path

    def _load_constraints_cache(self) -> Dict[str, Dict[str, str]]:
        """Load cached per-tag constraints to avoid recomputation."""
        try:
            if self._constraints_cache_file.exists():
                with open(self._constraints_cache_file, "r", encoding="utf-8") as f:
                    data = json.load(f)
                    return data if isinstance(data, dict) else {}
        except (json.JSONDecodeError, IOError, OSError) as exc:
            logger.warning(f"Unable to read constraints cache: {exc}")
        return {}

    def _save_constraints_cache(self) -> None:
        """Persist constraints cache safely."""
        try:
            lock = getattr(self, "_constraints_cache_lock", None)
            atomic_write_json(
                self._constraints_cache_file, self._constraints_cache, lock=lock, keep_backup=True
            )
        except (IOError, OSError, TypeError, ValueError, json.JSONDecodeError) as exc:
            logger.warning(f"Unable to write constraints cache: {exc}")
