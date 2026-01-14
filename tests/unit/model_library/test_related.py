"""Tests for related model utilities."""

from __future__ import annotations

import pytest

from backend.model_library.related import (
    extract_base_model_repo_id,
    extract_repo_id,
    has_related_metadata,
    normalize_family_token,
)


@pytest.mark.unit
class TestExtractRepoId:
    """Coverage for repo id extraction helpers."""

    def test_extracts_from_hf_url(self) -> None:
        """Extract repo id from HuggingFace URL."""
        assert extract_repo_id("https://huggingface.co/org/model") == "org/model"

    def test_extracts_from_repo_string(self) -> None:
        """Extract repo id from plain repo string."""
        assert extract_repo_id("org/model") == "org/model"

    def test_extracts_from_nested_dict(self) -> None:
        """Extract repo id from nested dict payloads."""
        assert extract_repo_id({"repoId": "org/model"}) == "org/model"

    def test_extracts_from_list_payload(self) -> None:
        """Extract repo id from list-based metadata."""
        assert extract_repo_id([{"model": "org/model"}]) == "org/model"

    def test_returns_none_for_invalid(self) -> None:
        """Return None when no repo id is present."""
        assert extract_repo_id("not-a-repo") is None
        assert extract_repo_id(None) is None


@pytest.mark.unit
class TestExtractBaseModelRepoId:
    """Coverage for base model repo resolution."""

    def test_prefers_base_model_field(self) -> None:
        """Use base_model when present."""
        metadata = {"base_model": "org/model"}
        assert extract_base_model_repo_id(metadata) == "org/model"

    def test_uses_model_card_base_model(self) -> None:
        """Fallback to model card base_model."""
        metadata = {"model_card": {"base_model": "org/model"}}
        assert extract_base_model_repo_id(metadata) == "org/model"

    def test_falls_back_to_download_url(self) -> None:
        """Fallback to download_url when base_model is missing."""
        metadata = {"download_url": "https://huggingface.co/org/model"}
        assert extract_base_model_repo_id(metadata) == "org/model"


@pytest.mark.unit
class TestRelatedMetadataSignals:
    """Coverage for related metadata checks."""

    def test_normalize_family_token(self) -> None:
        """Normalize family tokens and filter reserved values."""
        assert normalize_family_token("My Model") == "mymodel"
        assert normalize_family_token("unknown") == ""
        assert normalize_family_token("Imported") == ""

    def test_has_related_metadata(self) -> None:
        """Require family and base model repo id."""
        assert has_related_metadata({"family": "Foo", "base_model": "org/model"}) is True
        assert has_related_metadata({"family": "unknown", "base_model": "org/model"}) is False
        assert has_related_metadata({"family": "Foo", "base_model": "not-a-repo"}) is False
