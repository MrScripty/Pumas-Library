import zipfile
from pathlib import Path

from backend.metadata_manager import MetadataManager
from backend.version_manager_components.installer import InstallationMixin


class DummyFetcher:
    def __init__(self, release: dict):
        self.release = release

    def get_release_by_tag(self, tag: str):
        if tag == self.release.get("tag_name"):
            return self.release
        return None


class DummyResourceManager:
    def __init__(self):
        self.calls = []

    def setup_version_symlinks(self, tag: str):
        self.calls.append(tag)


class DummyProgressTracker:
    def __init__(self):
        self.started = False
        self.error = None
        self.completed = None
        self.completed_items = []

    def start_installation(self, _tag, log_path=None, **_kwargs):
        self.started = True
        self.log_path = log_path

    def update_stage(self, _stage, _progress=0, _current_item=None):
        return None

    def update_download_progress(self, _downloaded, _total=None, _speed=None):
        return None

    def add_completed_item(self, name, _kind, _size=None):
        self.completed_items.append(name)

    def set_error(self, message):
        self.error = message

    def complete_installation(self, success):
        self.completed = success


class DummyInstaller(InstallationMixin):
    def __init__(self, tmp_path: Path, release: dict):
        self.launcher_root = tmp_path
        self.metadata_manager = MetadataManager(tmp_path / "launcher-data")
        self.github_fetcher = DummyFetcher(release)
        self.resource_manager = DummyResourceManager()
        self.versions_dir = tmp_path / "comfyui-versions"
        self.versions_dir.mkdir(parents=True, exist_ok=True)
        self.logs_dir = tmp_path / "launcher-data" / "logs"
        self.logs_dir.mkdir(parents=True, exist_ok=True)
        self.progress_tracker = DummyProgressTracker()
        self._cancel_installation = False
        self._installing_tag = None
        self._current_process = None
        self._current_downloader = None
        self._install_log_handle = None
        self._current_install_log_path = None

    def _create_venv(self, version_path: Path) -> bool:
        venv_python = version_path / "venv" / "bin" / "python"
        venv_python.parent.mkdir(parents=True, exist_ok=True)
        venv_python.write_text("")
        return True

    def _install_dependencies_with_progress(self, _tag: str) -> bool:
        return True

    def _get_python_version(self, _version_path: Path) -> str:
        return "Python 3.12.0"

    def get_installed_versions(self):
        versions = self.metadata_manager.load_versions()
        return list(versions.get("installed", {}).keys())


def test_open_install_log_writes_header(tmp_path):
    release = {"tag_name": "v1", "zipball_url": "https://example.com/zipball/v1"}
    dummy = DummyInstaller(tmp_path, release)
    log_path = dummy._open_install_log("install-test")
    assert log_path.exists()
    dummy._install_log_handle.close()
    content = log_path.read_text()
    assert "INSTALL START" in content


def test_cancel_installation_without_active_returns_false(tmp_path):
    release = {"tag_name": "v1", "zipball_url": "https://example.com/zipball/v1"}
    dummy = DummyInstaller(tmp_path, release)
    assert dummy.cancel_installation() is False


def test_install_version_success(monkeypatch, tmp_path):
    release = {"tag_name": "v1", "zipball_url": "https://example.com/zipball/v1"}
    dummy = DummyInstaller(tmp_path, release)

    def fake_download_with_retry(_url, destination, progress_callback=None):
        with zipfile.ZipFile(destination, "w") as zipf:
            zipf.writestr("repo-root/main.py", "def main():\n    return 'hello'\n")
            zipf.writestr("repo-root/requirements.txt", "foo==1.0\n")
        if progress_callback:
            progress_callback(10, 10, 1.0)
        return True

    class FakeDownloader:
        def download_with_retry(self, url, destination, progress_callback=None):
            return fake_download_with_retry(url, destination, progress_callback)

    monkeypatch.setattr(
        "backend.version_manager_components.installer.DownloadManager",
        FakeDownloader,
    )

    assert dummy.install_version("v1") is True
    version_path = dummy.versions_dir / "v1"
    assert (version_path / "main.py").exists()
    versions = dummy.metadata_manager.load_versions()
    assert "v1" in versions.get("installed", {})
    assert dummy.resource_manager.calls == ["v1"]
