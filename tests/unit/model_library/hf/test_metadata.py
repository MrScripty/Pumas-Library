"""Tests for metadata utilities."""

from __future__ import annotations

from unittest.mock import MagicMock

import pytest

from backend.model_library.hf.metadata import (
    KIND_TAG_MAPPING,
    coerce_int,
    collect_paths_with_sizes,
    infer_kind_from_tags,
)


@pytest.mark.unit
class TestKindTagMapping:
    """Tests for KIND_TAG_MAPPING constant."""

    def test_mapping_contains_common_kinds(self):
        """Test that common model kinds are present."""
        assert "text-to-image" in KIND_TAG_MAPPING
        assert "text-to-video" in KIND_TAG_MAPPING
        assert "text-to-audio" in KIND_TAG_MAPPING

    def test_mapping_values_are_lists(self):
        """Test that mapping values are lists."""
        for kind, needles in KIND_TAG_MAPPING.items():
            assert isinstance(needles, list)
            assert len(needles) > 0


@pytest.mark.unit
class TestInferKindFromTags:
    """Tests for infer_kind_from_tags function."""

    def test_exact_task_match(self):
        """Test exact task tag matching."""
        assert infer_kind_from_tags(["text-to-image"]) == "text-to-image"

    def test_partial_task_match(self):
        """Test partial tag matching."""
        assert infer_kind_from_tags(["diffusion-text-to-image-model"]) == "text-to-image"

    def test_case_insensitive(self):
        """Test case insensitive matching."""
        assert infer_kind_from_tags(["TEXT-TO-IMAGE"]) == "text-to-image"
        assert infer_kind_from_tags(["Text2Img"]) == "text-to-image"

    def test_text2video_shorthand(self):
        """Test text2video shorthand."""
        assert infer_kind_from_tags(["text2video"]) == "text-to-video"

    def test_speech_recognition(self):
        """Test speech recognition variants."""
        assert infer_kind_from_tags(["automatic-speech-recognition"]) == "audio-to-text"
        assert infer_kind_from_tags(["asr"]) == "audio-to-text"

    def test_fallback_video(self):
        """Test fallback to generic video kind."""
        assert infer_kind_from_tags(["video-generation"]) == "video"

    def test_fallback_audio(self):
        """Test fallback to generic audio kind."""
        assert infer_kind_from_tags(["audio-processing"]) == "audio"

    def test_fallback_image(self):
        """Test fallback to generic image kind."""
        assert infer_kind_from_tags(["image-classification"]) == "image"

    def test_fallback_text(self):
        """Test fallback to generic text kind."""
        assert infer_kind_from_tags(["text-classification"]) == "text"

    def test_fallback_3d(self):
        """Test fallback to generic 3d kind."""
        assert infer_kind_from_tags(["3d-generation"]) == "3d"

    def test_unknown_when_no_match(self):
        """Test unknown return when no tags match."""
        assert infer_kind_from_tags(["pytorch", "transformers"]) == "unknown"

    def test_empty_tags(self):
        """Test empty tags list."""
        assert infer_kind_from_tags([]) == "unknown"

    def test_priority_specific_over_generic(self):
        """Test that specific matches take priority."""
        assert infer_kind_from_tags(["text-to-image", "image"]) == "text-to-image"


@pytest.mark.unit
class TestCoerceInt:
    """Tests for coerce_int function."""

    def test_int_passthrough(self):
        """Test integer passthrough."""
        assert coerce_int(42) == 42

    def test_float_to_int(self):
        """Test float truncation."""
        assert coerce_int(3.7) == 3
        assert coerce_int(3.2) == 3

    def test_string_to_int(self):
        """Test string conversion."""
        assert coerce_int("100") == 100

    def test_bool_to_int(self):
        """Test boolean conversion."""
        assert coerce_int(True) == 1
        assert coerce_int(False) == 0

    def test_invalid_string_returns_zero(self):
        """Test invalid string returns zero."""
        assert coerce_int("not a number") == 0
        assert coerce_int("") == 0

    def test_none_returns_zero(self):
        """Test None returns zero."""
        assert coerce_int(None) == 0

    def test_list_returns_zero(self):
        """Test list returns zero."""
        assert coerce_int([1, 2, 3]) == 0

    def test_dict_returns_zero(self):
        """Test dict returns zero."""
        assert coerce_int({"key": "value"}) == 0


@pytest.mark.unit
class TestCollectPathsWithSizes:
    """Tests for collect_paths_with_sizes function."""

    def test_extracts_from_rfilename_and_size(self):
        """Test extraction from rfilename and size attributes."""
        sibling = MagicMock()
        sibling.rfilename = "model.safetensors"
        sibling.size = 1000

        result = collect_paths_with_sizes([sibling])
        assert result == [("model.safetensors", 1000)]

    def test_extracts_size_from_lfs(self):
        """Test extraction of size from lfs dict."""
        sibling = MagicMock()
        sibling.rfilename = "model.safetensors"
        sibling.size = None
        sibling.lfs = {"size": 2000}

        result = collect_paths_with_sizes([sibling])
        assert result == [("model.safetensors", 2000)]

    def test_skips_missing_rfilename(self):
        """Test skipping siblings without rfilename."""
        sibling = MagicMock(spec=[])
        result = collect_paths_with_sizes([sibling])
        assert result == []

    def test_skips_empty_rfilename(self):
        """Test skipping siblings with empty rfilename."""
        sibling = MagicMock()
        sibling.rfilename = ""
        sibling.size = 1000

        result = collect_paths_with_sizes([sibling])
        assert result == []

    def test_skips_zero_size(self):
        """Test skipping files with zero size."""
        sibling = MagicMock()
        sibling.rfilename = "model.safetensors"
        sibling.size = 0

        result = collect_paths_with_sizes([sibling])
        assert result == []

    def test_skips_negative_size(self):
        """Test skipping files with negative size."""
        sibling = MagicMock()
        sibling.rfilename = "model.safetensors"
        sibling.size = -1

        result = collect_paths_with_sizes([sibling])
        assert result == []

    def test_handles_invalid_size_type(self):
        """Test handling of invalid size type."""
        sibling = MagicMock()
        sibling.rfilename = "model.safetensors"
        sibling.size = "not a number"

        result = collect_paths_with_sizes([sibling])
        assert result == []

    def test_multiple_siblings(self):
        """Test multiple siblings."""
        sibling1 = MagicMock()
        sibling1.rfilename = "model1.safetensors"
        sibling1.size = 1000

        sibling2 = MagicMock()
        sibling2.rfilename = "model2.bin"
        sibling2.size = 2000

        result = collect_paths_with_sizes([sibling1, sibling2])
        assert len(result) == 2
        assert ("model1.safetensors", 1000) in result
        assert ("model2.bin", 2000) in result

    def test_empty_siblings(self):
        """Test empty siblings list."""
        assert collect_paths_with_sizes([]) == []

    def test_string_size_conversion(self):
        """Test string size conversion."""
        sibling = MagicMock()
        sibling.rfilename = "model.safetensors"
        sibling.size = "3000"

        result = collect_paths_with_sizes([sibling])
        assert result == [("model.safetensors", 3000)]
