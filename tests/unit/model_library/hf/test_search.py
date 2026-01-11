"""Tests for HuggingFace model search functionality."""

from __future__ import annotations

from unittest.mock import MagicMock, patch

import pytest

from backend.model_library.hf.search import (
    _extract_downloads,
    _extract_release_date,
    _list_repo_files_safe,
    list_repo_tree_paths,
    search_models,
)


@pytest.mark.unit
class TestListRepoTreePaths:
    """Tests for list_repo_tree_paths function."""

    def test_extracts_paths_and_sizes(self):
        """Test extraction of paths and sizes from repo tree."""
        mock_api = MagicMock()
        item1 = MagicMock()
        item1.path = "model.safetensors"
        item1.size = 1000

        item2 = MagicMock()
        item2.path = "config.json"
        item2.size = 100

        mock_api.list_repo_tree.return_value = [item1, item2]

        result = list_repo_tree_paths(mock_api, "user/model")

        assert len(result) == 2
        assert ("model.safetensors", 1000) in result
        assert ("config.json", 100) in result

    def test_uses_rfilename_fallback(self):
        """Test fallback to rfilename when path is missing."""
        mock_api = MagicMock()
        item = MagicMock(spec=["rfilename", "size"])
        item.rfilename = "model.gguf"
        item.size = 2000

        mock_api.list_repo_tree.return_value = [item]

        result = list_repo_tree_paths(mock_api, "user/model")
        assert result == [("model.gguf", 2000)]

    def test_skips_items_without_path(self):
        """Test skipping items without path or rfilename."""
        mock_api = MagicMock()
        item = MagicMock(spec=["size"])
        item.size = 1000

        mock_api.list_repo_tree.return_value = [item]

        result = list_repo_tree_paths(mock_api, "user/model")
        assert result == []

    def test_skips_zero_size(self):
        """Test skipping items with zero size."""
        mock_api = MagicMock()
        item = MagicMock()
        item.path = "empty.txt"
        item.size = 0

        mock_api.list_repo_tree.return_value = [item]

        result = list_repo_tree_paths(mock_api, "user/model")
        assert result == []

    def test_handles_oserror(self):
        """Test handling of OSError."""
        mock_api = MagicMock()
        mock_api.list_repo_tree.side_effect = OSError("Network error")

        result = list_repo_tree_paths(mock_api, "user/model")
        assert result == []

    def test_handles_runtimeerror(self):
        """Test handling of RuntimeError."""
        mock_api = MagicMock()
        mock_api.list_repo_tree.side_effect = RuntimeError("API error")

        result = list_repo_tree_paths(mock_api, "user/model")
        assert result == []

    def test_handles_valueerror(self):
        """Test handling of ValueError."""
        mock_api = MagicMock()
        mock_api.list_repo_tree.side_effect = ValueError("Invalid repo")

        result = list_repo_tree_paths(mock_api, "user/model")
        assert result == []

    def test_handles_invalid_size_type(self):
        """Test handling of invalid size type."""
        mock_api = MagicMock()
        item = MagicMock()
        item.path = "model.bin"
        item.size = "not a number"

        mock_api.list_repo_tree.return_value = [item]

        result = list_repo_tree_paths(mock_api, "user/model")
        assert result == []


@pytest.mark.unit
class TestExtractReleaseDate:
    """Tests for _extract_release_date function."""

    def test_extracts_isoformat(self):
        """Test extraction of ISO format date."""
        from datetime import datetime

        info = MagicMock()
        info.last_modified = datetime(2024, 1, 15, 12, 30, 0)

        result = _extract_release_date(info)
        assert "2024-01-15" in result

    def test_returns_empty_when_none(self):
        """Test empty return when last_modified is None."""
        info = MagicMock()
        info.last_modified = None

        result = _extract_release_date(info)
        assert result == ""

    def test_handles_non_datetime(self):
        """Test handling of non-datetime values."""
        info = MagicMock()
        info.last_modified = "2024-01-15"

        result = _extract_release_date(info)
        assert result == "2024-01-15"


@pytest.mark.unit
class TestExtractDownloads:
    """Tests for _extract_downloads function."""

    def test_extracts_int(self):
        """Test extraction of integer downloads."""
        info = MagicMock()
        info.downloads = 1000

        result = _extract_downloads(info)
        assert result == 1000

    def test_extracts_string_int(self):
        """Test extraction of string integer."""
        info = MagicMock()
        info.downloads = "5000"

        result = _extract_downloads(info)
        assert result == 5000

    def test_returns_none_when_none(self):
        """Test None return when downloads is None."""
        info = MagicMock()
        info.downloads = None

        result = _extract_downloads(info)
        assert result is None

    def test_returns_none_on_invalid(self):
        """Test None return on invalid value."""
        info = MagicMock()
        info.downloads = "not a number"

        result = _extract_downloads(info)
        assert result is None


@pytest.mark.unit
class TestListRepoFilesSafe:
    """Tests for _list_repo_files_safe function."""

    def test_returns_file_list(self):
        """Test returning file list."""
        mock_api = MagicMock()
        mock_api.list_repo_files.return_value = ["model.bin", "config.json"]

        result = _list_repo_files_safe(mock_api, "user/model")
        assert result == ["model.bin", "config.json"]

    def test_handles_oserror(self):
        """Test handling of OSError."""
        mock_api = MagicMock()
        mock_api.list_repo_files.side_effect = OSError("Network error")

        result = _list_repo_files_safe(mock_api, "user/model")
        assert result == []

    def test_handles_runtimeerror(self):
        """Test handling of RuntimeError."""
        mock_api = MagicMock()
        mock_api.list_repo_files.side_effect = RuntimeError("API error")

        result = _list_repo_files_safe(mock_api, "user/model")
        assert result == []

    def test_handles_valueerror(self):
        """Test handling of ValueError."""
        mock_api = MagicMock()
        mock_api.list_repo_files.side_effect = ValueError("Invalid repo")

        result = _list_repo_files_safe(mock_api, "user/model")
        assert result == []


@pytest.mark.unit
class TestSearchModels:
    """Tests for search_models function."""

    def test_basic_search(self):
        """Test basic model search."""
        mock_api = MagicMock()

        model_info = MagicMock()
        model_info.modelId = "user/test-model"
        model_info.id = None
        model_info.tags = ["gguf", "q4_k_m"]
        model_info.siblings = []
        model_info.author = "user"
        model_info.pipeline_tag = "text-generation"
        model_info.last_modified = None
        model_info.downloads = 100

        mock_api.list_models.return_value = [model_info]
        mock_api.list_repo_files.return_value = ["model-q4_k_m.gguf"]
        mock_api.list_repo_tree.return_value = []

        result = search_models(mock_api, "test")

        assert len(result) == 1
        assert result[0]["repoId"] == "user/test-model"
        assert result[0]["developer"] == "user"
        assert result[0]["kind"] == "text-generation"

    def test_uses_id_fallback(self):
        """Test fallback to id when modelId is missing."""
        mock_api = MagicMock()

        model_info = MagicMock()
        model_info.modelId = None
        model_info.id = "org/another-model"
        model_info.tags = []
        model_info.siblings = []
        model_info.author = None
        model_info.pipeline_tag = "unknown"
        model_info.last_modified = None
        model_info.downloads = None

        mock_api.list_models.return_value = [model_info]
        mock_api.list_repo_files.return_value = []
        mock_api.list_repo_tree.return_value = []

        result = search_models(mock_api, "test")

        assert len(result) == 1
        assert result[0]["repoId"] == "org/another-model"
        assert result[0]["developer"] == "org"

    def test_skips_model_without_id(self):
        """Test skipping models without any ID."""
        mock_api = MagicMock()

        model_info = MagicMock()
        model_info.modelId = None
        model_info.id = None

        mock_api.list_models.return_value = [model_info]

        result = search_models(mock_api, "test")
        assert result == []

    def test_infers_kind_from_tags(self):
        """Test kind inference from tags."""
        mock_api = MagicMock()

        model_info = MagicMock()
        model_info.modelId = "user/model"
        model_info.tags = ["text-to-image", "diffusion"]
        model_info.siblings = []
        model_info.author = "user"
        model_info.pipeline_tag = None
        model_info.last_modified = None
        model_info.downloads = None

        mock_api.list_models.return_value = [model_info]
        mock_api.list_repo_files.return_value = []
        mock_api.list_repo_tree.return_value = []

        result = search_models(mock_api, "test")

        assert result[0]["kind"] == "text-to-image"

    def test_with_kind_filter_fallback(self):
        """Test search with kind filter falls back when ModelFilter unavailable."""
        mock_api = MagicMock()
        mock_api.list_models.return_value = []

        # When ModelFilter is not available, the kind string is passed directly
        search_models(mock_api, "test", kind="text-generation")

        mock_api.list_models.assert_called_once()
        call_kwargs = mock_api.list_models.call_args[1]
        # Falls back to passing kind string directly when ModelFilter unavailable
        assert call_kwargs["filter"] in ("text-generation", None) or hasattr(
            call_kwargs["filter"], "task"
        )

    def test_extracts_formats_and_quants(self):
        """Test extraction of formats and quants from siblings."""
        mock_api = MagicMock()

        sibling = MagicMock()
        sibling.rfilename = "model-q4_k_m.gguf"
        sibling.size = 5000

        model_info = MagicMock()
        model_info.modelId = "user/model"
        model_info.tags = []
        model_info.siblings = [sibling]
        model_info.author = "user"
        model_info.pipeline_tag = "text-generation"
        model_info.last_modified = None
        model_info.downloads = None

        mock_api.list_models.return_value = [model_info]

        result = search_models(mock_api, "test")

        assert "gguf" in result[0]["formats"]
        assert "q4_k_m" in result[0]["quants"]

    def test_builds_download_options(self):
        """Test building download options."""
        mock_api = MagicMock()

        sibling = MagicMock()
        sibling.rfilename = "model-q4_k_m.gguf"
        sibling.size = 5000
        sibling.lfs = None

        model_info = MagicMock()
        model_info.modelId = "user/model"
        model_info.tags = []
        model_info.siblings = [sibling]
        model_info.author = "user"
        model_info.pipeline_tag = "text-generation"
        model_info.last_modified = None
        model_info.downloads = None

        mock_api.list_models.return_value = [model_info]

        result = search_models(mock_api, "test")

        assert "downloadOptions" in result[0]
        assert isinstance(result[0]["downloadOptions"], list)

    def test_limit_parameter(self):
        """Test limit parameter is passed correctly."""
        mock_api = MagicMock()
        mock_api.list_models.return_value = []

        search_models(mock_api, "test", limit=50)

        mock_api.list_models.assert_called_once()
        call_kwargs = mock_api.list_models.call_args[1]
        assert call_kwargs["limit"] == 50
