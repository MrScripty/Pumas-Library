"""Tests for quantization token utilities."""

from __future__ import annotations

import pytest

from backend.model_library.hf.quant import (
    QUANT_TOKENS,
    extract_quants_from_paths,
    normalize_quant_source,
    quant_sizes_from_paths,
    sorted_quants,
    token_in_normalized,
)


@pytest.mark.unit
class TestQuantTokens:
    """Tests for QUANT_TOKENS constant."""

    def test_quant_tokens_is_tuple(self):
        """Test that QUANT_TOKENS is immutable."""
        assert isinstance(QUANT_TOKENS, tuple)

    def test_quant_tokens_contains_common_quants(self):
        """Test that common quant tokens are present."""
        common = ["q4_k_m", "q5_k_m", "q8_0", "fp16", "bf16"]
        for token in common:
            assert token in QUANT_TOKENS

    def test_quant_tokens_has_iq_variants(self):
        """Test that imatrix quant tokens are present."""
        iq_tokens = [t for t in QUANT_TOKENS if t.startswith("iq")]
        assert len(iq_tokens) > 0


@pytest.mark.unit
class TestNormalizeQuantSource:
    """Tests for normalize_quant_source function."""

    def test_lowercase_conversion(self):
        """Test that input is lowercased."""
        assert normalize_quant_source("Q4_K_M") == "q4_k_m"

    def test_special_chars_replaced(self):
        """Test that special characters become underscores."""
        assert normalize_quant_source("model-q4.k.m") == "model_q4_k_m"

    def test_leading_trailing_underscores_stripped(self):
        """Test that leading/trailing underscores are removed."""
        assert normalize_quant_source("--model--") == "model"

    def test_empty_string(self):
        """Test empty string handling."""
        assert normalize_quant_source("") == ""

    def test_multiple_special_chars(self):
        """Test multiple consecutive special chars."""
        assert normalize_quant_source("foo---bar") == "foo_bar"


@pytest.mark.unit
class TestTokenInNormalized:
    """Tests for token_in_normalized function."""

    def test_exact_match(self):
        """Test exact token match."""
        assert token_in_normalized("q4_k_m", "q4_k_m") is True

    def test_token_at_start(self):
        """Test token at start of string."""
        assert token_in_normalized("q4_k_m_model", "q4_k_m") is True

    def test_token_at_end(self):
        """Test token at end of string."""
        assert token_in_normalized("model_q4_k_m", "q4_k_m") is True

    def test_token_in_middle(self):
        """Test token in middle of string."""
        assert token_in_normalized("model_q4_k_m_v2", "q4_k_m") is True

    def test_partial_match_rejected(self):
        """Test that partial matches are rejected."""
        assert token_in_normalized("q4_k_m_extra", "q4_k") is True
        assert token_in_normalized("q4k_m", "q4_k_m") is False

    def test_empty_normalized(self):
        """Test empty normalized string."""
        assert token_in_normalized("", "q4_k_m") is False

    def test_empty_token(self):
        """Test empty token."""
        assert token_in_normalized("model", "") is False

    def test_both_empty(self):
        """Test both empty."""
        assert token_in_normalized("", "") is False

    def test_token_longer_than_string(self):
        """Test token longer than normalized string."""
        assert token_in_normalized("q4", "q4_k_m") is False


@pytest.mark.unit
class TestSortedQuants:
    """Tests for sorted_quants function."""

    def test_sorts_by_token_order(self):
        """Test that quants are sorted by QUANT_TOKENS order."""
        result = sorted_quants(["q8_0", "q4_k_m", "q5_k_m"])
        assert result.index("q4_k_m") < result.index("q5_k_m")
        assert result.index("q5_k_m") < result.index("q8_0")

    def test_removes_duplicates(self):
        """Test that duplicates are removed."""
        result = sorted_quants(["q4_k_m", "q4_k_m", "q5_k_m"])
        assert result.count("q4_k_m") == 1

    def test_removes_empty_strings(self):
        """Test that empty strings are filtered out."""
        result = sorted_quants(["q4_k_m", "", "q5_k_m"])
        assert "" not in result

    def test_empty_input(self):
        """Test empty input."""
        assert sorted_quants([]) == []

    def test_unknown_tokens_sorted_last(self):
        """Test that unknown tokens are sorted after known ones."""
        result = sorted_quants(["unknown_quant", "q4_k_m"])
        assert result[-1] == "unknown_quant"


@pytest.mark.unit
class TestExtractQuantsFromPaths:
    """Tests for extract_quants_from_paths function."""

    def test_extracts_from_filename(self):
        """Test extraction from filename."""
        paths = ["model-q4_k_m.gguf"]
        result = extract_quants_from_paths(paths, [])
        assert "q4_k_m" in result

    def test_extracts_from_tags(self):
        """Test extraction from tags."""
        result = extract_quants_from_paths([], ["gguf", "Q4_K_M"])
        assert "q4_k_m" in result

    def test_extracts_multiple_quants(self):
        """Test extraction of multiple quants."""
        paths = ["model-q4_k_m.gguf", "model-q8_0.gguf"]
        result = extract_quants_from_paths(paths, [])
        assert "q4_k_m" in result
        assert "q8_0" in result

    def test_longest_match_wins(self):
        """Test that longest matching token is selected."""
        paths = ["model-q4_k_m.gguf"]
        result = extract_quants_from_paths(paths, [])
        assert "q4_k_m" in result
        assert "q4" not in result

    def test_empty_inputs(self):
        """Test empty inputs."""
        assert extract_quants_from_paths([], []) == []

    def test_no_quants_found(self):
        """Test when no quants are found."""
        paths = ["model.safetensors", "config.json"]
        result = extract_quants_from_paths(paths, [])
        assert result == []


@pytest.mark.unit
class TestQuantSizesFromPaths:
    """Tests for quant_sizes_from_paths function."""

    def test_accumulates_sizes_per_quant(self):
        """Test size accumulation per quant."""
        paths = [
            ("model-q4_k_m-part1.gguf", 1000),
            ("model-q4_k_m-part2.gguf", 2000),
        ]
        result = quant_sizes_from_paths(paths)
        assert result["q4_k_m"] == 3000

    def test_separates_different_quants(self):
        """Test different quants have separate sizes."""
        paths = [
            ("model-q4_k_m.gguf", 1000),
            ("model-q8_0.gguf", 2000),
        ]
        result = quant_sizes_from_paths(paths)
        assert result["q4_k_m"] == 1000
        assert result["q8_0"] == 2000

    def test_shared_files_added_to_all_quants(self):
        """Test that shared config files are added to all quant sizes."""
        paths = [
            ("model-q4_k_m.gguf", 1000),
            ("model-q8_0.gguf", 2000),
            ("config.json", 100),
        ]
        result = quant_sizes_from_paths(paths)
        assert result["q4_k_m"] == 1100
        assert result["q8_0"] == 2100

    def test_shared_files_not_added_when_no_quants(self):
        """Test shared files alone don't create entries."""
        paths = [
            ("config.json", 100),
            ("tokenizer.json", 200),
        ]
        result = quant_sizes_from_paths(paths)
        assert result == {}

    def test_empty_input(self):
        """Test empty input."""
        assert quant_sizes_from_paths([]) == {}

    def test_shared_extensions(self):
        """Test various shared file extensions."""
        paths = [
            ("model-q4_k_m.gguf", 1000),
            ("config.json", 10),
            ("params.yml", 20),
            ("settings.yaml", 30),
            ("readme.txt", 40),
            ("notes.md", 50),
        ]
        result = quant_sizes_from_paths(paths)
        assert result["q4_k_m"] == 1150
