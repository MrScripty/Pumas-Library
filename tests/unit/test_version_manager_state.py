from pathlib import Path

from backend.metadata_manager import MetadataManager
from backend.version_manager_components.state import StateMixin


class DummyState(StateMixin):
    def __init__(self, tmp_path: Path):
        self.launcher_root = tmp_path
        self.metadata_manager = MetadataManager(tmp_path / "launcher-data")
        self.versions_dir = tmp_path / "comfyui-versions"
        self.versions_dir.mkdir(parents=True, exist_ok=True)
        self.active_version_file = tmp_path / ".active-version"
        self._active_version = None

    def check_dependencies(self, _tag: str):
        return {"installed": ["foo"], "missing": [], "requirementsFile": None}


def _make_complete_version(version_path: Path):
    version_path.mkdir(parents=True, exist_ok=True)
    (version_path / "main.py").write_text("def main():\n    return 'ok'\n")
    venv_python = version_path / "venv" / "bin" / "python"
    venv_python.parent.mkdir(parents=True, exist_ok=True)
    venv_python.write_text("")


def test_get_installed_versions_cleans_metadata(tmp_path):
    dummy = DummyState(tmp_path)
    versions = dummy.metadata_manager.load_versions()
    versions["installed"]["v1"] = {"path": "comfyui-versions/v1"}
    versions["installed"]["v2"] = {"path": "comfyui-versions/v2"}
    dummy.metadata_manager.save_versions(versions)

    _make_complete_version(dummy.versions_dir / "v1")
    (dummy.versions_dir / "v2").mkdir(parents=True, exist_ok=True)

    installed = dummy.get_installed_versions()
    assert installed == ["v1"]

    cleaned = dummy.metadata_manager.load_versions()
    assert list(cleaned.get("installed", {}).keys()) == ["v1"]


def test_validate_installations_removes_orphaned_dir(tmp_path):
    dummy = DummyState(tmp_path)
    orphan_dir = dummy.versions_dir / "orphan"
    orphan_dir.mkdir(parents=True, exist_ok=True)

    report = dummy.validate_installations()
    assert report["had_invalid"] is True
    assert "orphan" in report["removed"]
    assert not orphan_dir.exists()


def test_set_active_version_updates_state(tmp_path):
    dummy = DummyState(tmp_path)
    versions = dummy.metadata_manager.load_versions()
    versions["installed"]["v1"] = {"path": "comfyui-versions/v1"}
    dummy.metadata_manager.save_versions(versions)
    _make_complete_version(dummy.versions_dir / "v1")

    assert dummy.set_active_version("v1") is True
    assert dummy.active_version_file.read_text() == "v1"

    updated = dummy.metadata_manager.load_versions()
    assert updated.get("lastSelectedVersion") == "v1"


def test_get_version_status_includes_dependencies(tmp_path):
    dummy = DummyState(tmp_path)
    versions = dummy.metadata_manager.load_versions()
    versions["installed"]["v1"] = {"path": "comfyui-versions/v1"}
    dummy.metadata_manager.save_versions(versions)
    _make_complete_version(dummy.versions_dir / "v1")

    status = dummy.get_version_status()
    assert status["installedCount"] == 1
    assert status["versions"]["v1"]["dependencies"]["installed"] == ["foo"]


def test_get_active_version_uses_default(tmp_path):
    dummy = DummyState(tmp_path)
    versions = dummy.metadata_manager.load_versions()
    versions["installed"]["v1"] = {"path": "comfyui-versions/v1"}
    versions["defaultVersion"] = "v1"
    dummy.metadata_manager.save_versions(versions)
    _make_complete_version(dummy.versions_dir / "v1")

    assert dummy.get_active_version() == "v1"
