from pathlib import Path

from backend.version_manager_components.launcher import LauncherMixin


class DummyMetadata:
    def __init__(self, launcher_data_dir: Path):
        self.launcher_data_dir = launcher_data_dir


class DummyLauncher(LauncherMixin):
    def __init__(self, tmp_path: Path):
        self.metadata_manager = DummyMetadata(tmp_path / "launcher-data")
        self.metadata_manager.launcher_data_dir.mkdir()
        self.logs_dir = tmp_path / "logs"
        self.logs_dir.mkdir()
        self.versions_dir = tmp_path / "versions"
        self.versions_dir.mkdir()
        self.resource_manager = type(
            "RM", (), {"validate_and_repair_symlinks": lambda *_: {"broken": []}}
        )()

    def get_installed_versions(self):
        return ["v1"]

    def set_active_version(self, _tag):
        return True

    def check_dependencies(self, _tag):
        return {"missing": []}

    def install_dependencies(self, _tag):
        return True


def test_slugify_tag(tmp_path):
    dummy = DummyLauncher(tmp_path)
    assert dummy._slugify_tag("v0.5.1") == "0-5-1"
    assert dummy._slugify_tag("My Tag") == "my-tag"


def test_ensure_version_run_script(tmp_path):
    dummy = DummyLauncher(tmp_path)
    version_path = dummy.versions_dir / "v1"
    version_path.mkdir()

    script_path = dummy._ensure_version_run_script("v1", version_path)
    assert script_path.exists()
    content = script_path.read_text()
    assert "Starting ComfyUI v1" in content


def test_tail_log(tmp_path):
    dummy = DummyLauncher(tmp_path)
    log_file = tmp_path / "log.txt"
    log_file.write_text("a\nb\nc\n")
    assert dummy._tail_log(log_file, lines=2) == ["b", "c"]


def test_wait_for_server_ready_success(monkeypatch, tmp_path):
    dummy = DummyLauncher(tmp_path)

    class FakeProcess:
        def __init__(self):
            self.returncode = None

        def poll(self):
            return None

    class FakeResponse:
        def __enter__(self):
            return type("Resp", (), {"status": 200})()

        def __exit__(self, exc_type, exc, tb):
            return False

    monkeypatch.setattr(
        "backend.version_manager_components.launcher.url_request.urlopen",
        lambda *_args, **_kwargs: FakeResponse(),
    )

    ready, err = dummy._wait_for_server_ready("http://localhost", FakeProcess(), Path("/tmp/log"))
    assert ready is True
    assert err is None


def test_open_frontend_uses_default_browser(monkeypatch, tmp_path):
    dummy = DummyLauncher(tmp_path)
    calls = {}

    def fake_popen(cmd, stdout=None, stderr=None):
        calls["cmd"] = cmd
        return None

    monkeypatch.setattr("backend.version_manager_components.launcher.shutil.which", lambda _: None)
    monkeypatch.setattr("backend.version_manager_components.launcher.subprocess.Popen", fake_popen)

    dummy._open_frontend("http://localhost", "slug")
    assert calls["cmd"][0] == "xdg-open"


def test_launch_version_success(monkeypatch, tmp_path):
    dummy = DummyLauncher(tmp_path)
    version_path = dummy.versions_dir / "v1"
    version_path.mkdir()
    (version_path / "main.py").write_text("")
    (version_path / "venv" / "bin").mkdir(parents=True)
    (version_path / "venv" / "bin" / "python").write_text("")

    monkeypatch.setattr(dummy, "_wait_for_server_ready", lambda *_args, **_kwargs: (True, None))
    monkeypatch.setattr(dummy, "_open_frontend", lambda *_args, **_kwargs: None)

    class FakeProcess:
        pid = 123

        def poll(self):
            return None

    monkeypatch.setattr(
        "backend.version_manager_components.launcher.subprocess.Popen",
        lambda *args, **kwargs: FakeProcess(),
    )

    success, process, log_path, err, ready = dummy.launch_version("v1")
    assert success is True
    assert ready is True
    assert process is not None
    assert err is None
