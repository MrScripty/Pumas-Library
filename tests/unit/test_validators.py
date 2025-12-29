from pathlib import Path

import pytest

from backend.exceptions import ValidationError
from backend.validators import (
    sanitize_path,
    validate_package_name,
    validate_url,
    validate_version_tag,
)


def test_validate_version_tag_accepts_safe_tags():
    assert validate_version_tag("v1.2.3")
    assert validate_version_tag("1.2.3")
    assert validate_version_tag("v1-rc1")


def test_validate_version_tag_rejects_invalid():
    assert not validate_version_tag("")
    assert not validate_version_tag("v1/../../etc")
    assert not validate_version_tag("v1_2")
    assert not validate_version_tag("v1 2")


def test_validate_url_accepts_http_https():
    assert validate_url("https://example.com/resource")
    assert validate_url("http://example.com")


def test_validate_url_rejects_invalid():
    assert not validate_url("")
    assert not validate_url("ftp://example.com")
    assert not validate_url("https://")


def test_sanitize_path_allows_relative_under_base(tmp_path):
    base = tmp_path / "base"
    base.mkdir()
    target = sanitize_path("data/file.txt", base)
    assert target == (base / "data" / "file.txt").resolve()


def test_sanitize_path_rejects_traversal(tmp_path):
    base = tmp_path / "base"
    base.mkdir()
    with pytest.raises(ValidationError):
        sanitize_path("../escape.txt", base)


def test_validate_package_name_accepts_safe_names():
    assert validate_package_name("foo")
    assert validate_package_name("foo-bar")
    assert validate_package_name("foo_bar.baz")


def test_validate_package_name_rejects_invalid():
    assert not validate_package_name("")
    assert not validate_package_name("foo bar")
    assert not validate_package_name("foo/bar")
