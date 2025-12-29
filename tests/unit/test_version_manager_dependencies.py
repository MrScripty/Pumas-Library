import json
from pathlib import Path

from backend.version_manager_components.dependencies import DependenciesMixin


class DummyMetadata:
    def __init__(self, cache_dir: Path):
        self.cache_dir = cache_dir


class DummyFetcher:
    def get_release_by_tag(self, _tag):
        return None


class DummyProgress:
    def __init__(self):
        self.state = {}
        self.completed = []

    def set_dependency_weights(self, _specs):
        self.state["weights_set"] = True

    def get_current_state(self):
        return self.state

    def update_dependency_progress(self, _message, _current, _total):
        self.state["progress"] = True

    def complete_package(self, name):
        self.completed.append(name)

    def add_completed_item(self, name, _kind):
        self.completed.append(name)

    def set_error(self, _message):
        self.state["error"] = True


class DummyDeps(DependenciesMixin):
    def __init__(self, tmp_path: Path):
        self.metadata_manager = DummyMetadata(tmp_path / "cache")
        self.metadata_manager.cache_dir.mkdir()
        self.launcher_root = tmp_path
        self.versions_dir = tmp_path / "versions"
        self.versions_dir.mkdir()
        self.pip_cache_dir = tmp_path / "pip-cache"
        self.active_pip_cache_dir = self.pip_cache_dir
        self._cancel_installation = False
        self._current_process = None
        self.github_fetcher = DummyFetcher()
        self.progress_tracker = DummyProgress()
        self._log_messages = []

    def _build_constraints_for_tag(self, _tag, _req, _release):
        return None

    def _log_install(self, message: str):
        self._log_messages.append(message)


def test_build_pip_env_sets_cache_dir(tmp_path):
    dummy = DummyDeps(tmp_path)
    env = dummy._build_pip_env()
    assert env["PIP_CACHE_DIR"].endswith("pip-cache")
    assert dummy.active_pip_cache_dir == dummy.pip_cache_dir


def test_create_space_safe_requirements(tmp_path):
    dummy = DummyDeps(tmp_path)
    req = tmp_path / "requirements.txt"
    constraints = tmp_path / "constraints.txt"
    req.write_text("foo==1.0\n")
    constraints.write_text("foo==1.0\n")

    safe_req, safe_constraints = dummy._create_space_safe_requirements("v1", req, constraints)
    assert safe_req is not None and safe_req.exists()
    assert safe_constraints is not None and safe_constraints.exists()
    assert "requirements-safe" in str(safe_req)


def test_get_installed_package_names_json(monkeypatch, tmp_path):
    dummy = DummyDeps(tmp_path)

    def fake_run_command(_cmd, timeout=None, env=None):
        return True, json.dumps([{"name": "Foo"}, {"name": "bar"}]), ""

    monkeypatch.setattr(
        "backend.version_manager_components.dependencies.run_command", fake_run_command
    )

    installed = dummy._get_installed_package_names("v1", Path("/tmp/python"))
    assert installed == {"foo", "bar"}


def test_get_installed_package_names_freeze_fallback(monkeypatch, tmp_path):
    dummy = DummyDeps(tmp_path)
    calls = {"count": 0}

    def fake_run_command(_cmd, timeout=None, env=None):
        calls["count"] += 1
        if calls["count"] == 1:
            return False, "", "json failure"
        return True, "Foo==1.0\nbar @ git+https://example.com\n", ""

    monkeypatch.setattr(
        "backend.version_manager_components.dependencies.run_command", fake_run_command
    )

    installed = dummy._get_installed_package_names("v1", Path("/tmp/python"))
    assert installed == {"foo", "bar"}


def test_check_dependencies_reports_missing(tmp_path):
    dummy = DummyDeps(tmp_path)
    version_dir = dummy.versions_dir / "v1"
    version_dir.mkdir()
    (version_dir / "requirements.txt").write_text("foo==1.0\n")

    status = dummy.check_dependencies("v1")
    assert set(status["missing"]) == {"foo", "setproctitle"}
    assert status["installed"] == []


def test_install_dependencies_success(monkeypatch, tmp_path):
    dummy = DummyDeps(tmp_path)
    version_dir = dummy.versions_dir / "v1"
    (version_dir / "venv" / "bin").mkdir(parents=True)
    (version_dir / "venv" / "bin" / "python").write_text("")
    (version_dir / "requirements.txt").write_text("foo==1.0\n")

    def fake_run_command(_cmd, timeout=None, env=None):
        return True, "ok", ""

    monkeypatch.setattr(
        "backend.version_manager_components.dependencies.run_command", fake_run_command
    )

    assert dummy.install_dependencies("v1") is True


def test_install_dependencies_with_progress_success(monkeypatch, tmp_path):
    dummy = DummyDeps(tmp_path)
    version_dir = dummy.versions_dir / "v1"
    (version_dir / "venv" / "bin").mkdir(parents=True)
    (version_dir / "venv" / "bin" / "python").write_text("")
    (version_dir / "requirements.txt").write_text("foo==1.0\n")

    def fake_run_command(_cmd, timeout=None, env=None):
        return True, "", ""

    monkeypatch.setattr(
        "backend.version_manager_components.dependencies.run_command", fake_run_command
    )

    class FakeStdout:
        def __init__(self, lines):
            self.lines = list(lines)

        def readline(self):
            if self.lines:
                return self.lines.pop(0)
            return ""

    class FakeProcess:
        def __init__(self):
            self.stdout = FakeStdout(
                [
                    "Collecting foo\n",
                    "Downloading foo (1.0 MB)\n",
                    "Successfully installed foo-1.0\n",
                ]
            )
            self.returncode = None
            self.pid = 123

        def poll(self):
            if not self.stdout.lines:
                self.returncode = 0
                return self.returncode
            return None

        def communicate(self):
            return "", ""

    monkeypatch.setattr(
        "backend.version_manager_components.dependencies.subprocess.Popen",
        lambda *args, **kwargs: FakeProcess(),
    )

    class FakeIOTracker:
        def __init__(self, **_kwargs):
            pass

        def should_update(self, min_interval_sec=0.0):
            return False

        def get_download_metrics(self):
            return None, None

    monkeypatch.setattr(
        "backend.version_manager_components.dependencies.ProcessIOTracker", FakeIOTracker
    )
    import select

    monkeypatch.setattr(select, "select", lambda _r, _w, _e, _t: (_r, [], []))

    assert dummy._install_dependencies_with_progress("v1") is True
    assert dummy.progress_tracker.state.get("progress") is True
    assert "foo" in {name.lower() for name in dummy.progress_tracker.completed}
