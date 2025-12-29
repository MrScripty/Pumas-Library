import io
import json
from datetime import datetime, timezone
from pathlib import Path

from backend.version_manager_components.constraints import ConstraintsMixin


class DummyConstraints(ConstraintsMixin):
    def __init__(self, tmp_path: Path):
        self.constraints_dir = tmp_path / "constraints"
        self.constraints_dir.mkdir()
        self._constraints_cache_file = tmp_path / "constraints-cache.json"
        self._constraints_cache = {}
        self._pypi_release_cache = {}


class DummyResponse:
    def __init__(self, payload):
        self.payload = payload

    def __enter__(self):
        return io.StringIO(json.dumps(self.payload))

    def __exit__(self, exc_type, exc, tb):
        return False


def test_fetch_pypi_versions_caches(monkeypatch, tmp_path):
    payload = {
        "releases": {
            "1.0.0": [{"upload_time_iso_8601": "2024-01-01T00:00:00Z"}],
            "2.0.0": [{"upload_time": "2024-02-01T00:00:00Z"}],
        }
    }
    calls = {"count": 0}

    def fake_urlopen(_url, timeout=0):
        calls["count"] += 1
        return DummyResponse(payload)

    monkeypatch.setattr(
        "backend.version_manager_components.constraints.url_request.urlopen", fake_urlopen
    )

    dummy = DummyConstraints(tmp_path)
    versions = dummy._fetch_pypi_versions("Example")
    assert "1.0.0" in versions
    assert versions["1.0.0"].tzinfo == timezone.utc

    cached = dummy._fetch_pypi_versions("Example")
    assert cached == versions
    assert calls["count"] == 1


def test_select_version_for_date(monkeypatch, tmp_path):
    dummy = DummyConstraints(tmp_path)
    release_date = datetime(2024, 2, 15, tzinfo=timezone.utc)
    fake_versions = {
        "1.0.0": datetime(2024, 1, 1, tzinfo=timezone.utc),
        "1.1.0": datetime(2024, 2, 10, tzinfo=timezone.utc),
        "2.0.0": datetime(2024, 3, 1, tzinfo=timezone.utc),
    }

    monkeypatch.setattr(dummy, "_fetch_pypi_versions", lambda _pkg: fake_versions)

    selected = dummy._select_version_for_date("example", ">=1.0", release_date)
    assert selected == "1.1.0"


def test_build_constraints_for_tag_writes_file(monkeypatch, tmp_path):
    dummy = DummyConstraints(tmp_path)
    requirements_file = tmp_path / "requirements.txt"
    requirements_file.write_text("foo>=1.0\nbar==2.0\nbaz\n")

    def fake_select(_pkg, _spec, _release_date):
        return "1.5.0"

    monkeypatch.setattr(dummy, "_select_version_for_date", fake_select)

    constraints_path = dummy._build_constraints_for_tag("v1", requirements_file, None)
    assert constraints_path is not None
    content = constraints_path.read_text()
    assert "foo==1.5.0" in content
    assert "bar==2.0" in content
    assert "baz==1.5.0" in content
    assert "v1" in dummy._constraints_cache


def test_constraints_cache_round_trip(tmp_path):
    dummy = DummyConstraints(tmp_path)
    dummy._constraints_cache = {"v1": {"foo": "==1.0"}}
    dummy._save_constraints_cache()
    dummy._constraints_cache = {}

    loaded = dummy._load_constraints_cache()
    assert loaded == {"v1": {"foo": "==1.0"}}


def test_get_release_date_parses(tmp_path):
    dummy = DummyConstraints(tmp_path)
    release = {"published_at": "2024-01-01T00:00:00Z"}
    parsed = dummy._get_release_date("v1", release)
    assert parsed is not None
    assert parsed.tzinfo == timezone.utc
