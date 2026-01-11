"""Tests for model type identification from file contents."""

import json
import struct
from pathlib import Path

import pytest
from gguf import GGUFWriter

from backend.model_library.model_identifier import (
    _identify_gguf,
    _identify_safetensors,
    _infer_diffusion_family,
    _infer_family_from_tensors,
    _normalize_gguf_architecture,
    identify_model_type,
)


@pytest.mark.unit
class TestIdentifyModelType:
    """Tests for the main identify_model_type function."""

    def test_unknown_extension_returns_none(self, tmp_path: Path) -> None:
        """Test that unknown extensions return None."""
        model_file = tmp_path / "model.txt"
        model_file.write_text("not a model")
        model_type, family, extra = identify_model_type(model_file)
        assert model_type is None
        assert family is None

    def test_nonexistent_file_returns_none(self, tmp_path: Path) -> None:
        """Test that nonexistent files return None."""
        model_file = tmp_path / "nonexistent.gguf"
        model_type, family, extra = identify_model_type(model_file)
        assert model_type is None
        assert family is None

    def test_safetensors_extension_uses_safetensors_detection(self, tmp_path: Path) -> None:
        """Test that .safetensors extension triggers safetensors detection."""
        model_file = tmp_path / "model.safetensors"
        # Create valid safetensors with diffusion patterns
        header = {
            "down_blocks.0.weight": {"dtype": "F16", "shape": [320, 320], "data_offsets": [0, 100]},
            "down_blocks.1.weight": {
                "dtype": "F16",
                "shape": [320, 320],
                "data_offsets": [100, 200],
            },
            "up_blocks.0.weight": {"dtype": "F16", "shape": [320, 320], "data_offsets": [200, 300]},
            "up_blocks.1.weight": {"dtype": "F16", "shape": [320, 320], "data_offsets": [300, 400]},
            "mid_block.weight": {"dtype": "F16", "shape": [320, 320], "data_offsets": [400, 500]},
            "time_embedding.weight": {
                "dtype": "F16",
                "shape": [320, 320],
                "data_offsets": [500, 600],
            },
        }
        header_json = json.dumps(header).encode("utf-8")
        header_len = struct.pack("<Q", len(header_json))
        model_file.write_bytes(header_len + header_json + b"\x00" * 1000)

        model_type, family, extra = identify_model_type(model_file)
        assert model_type == "diffusion"

    def test_gguf_extension_uses_gguf_detection(self, tmp_path: Path) -> None:
        """Test that .gguf extension triggers GGUF detection."""
        model_file = tmp_path / "model.gguf"
        _create_gguf_file(model_file, "llama")

        model_type, family, extra = identify_model_type(model_file)
        assert model_type == "llm"
        assert family == "llama"


def _create_gguf_file(path: Path, architecture: str) -> None:
    """Helper to create a valid GGUF file with the given architecture."""
    writer = GGUFWriter(str(path), architecture)
    writer.write_header_to_file()
    writer.write_kv_data_to_file()
    writer.close()


@pytest.mark.unit
class TestIdentifyGGUF:
    """Tests for GGUF file identification."""

    def test_invalid_magic_returns_none(self, tmp_path: Path) -> None:
        """Test that files without GGUF magic return None."""
        model_file = tmp_path / "model.gguf"
        model_file.write_bytes(b"NOTG" + b"\x00" * 100)
        model_type, family, extra = _identify_gguf(model_file)
        assert model_type is None

    def test_empty_file_returns_none(self, tmp_path: Path) -> None:
        """Test that empty files return None."""
        model_file = tmp_path / "model.gguf"
        model_file.touch()
        model_type, family, extra = _identify_gguf(model_file)
        assert model_type is None

    def test_valid_gguf_with_llama_architecture(self, tmp_path: Path) -> None:
        """Test that valid GGUF with llama architecture is identified."""
        model_file = tmp_path / "model.gguf"
        _create_gguf_file(model_file, "llama")

        model_type, family, extra = _identify_gguf(model_file)
        assert model_type == "llm"
        assert family == "llama"
        assert extra.get("gguf_architecture") == "llama"

    def test_valid_gguf_with_mistral_architecture(self, tmp_path: Path) -> None:
        """Test that valid GGUF with mistral architecture is identified."""
        model_file = tmp_path / "model.gguf"
        _create_gguf_file(model_file, "mistral")

        model_type, family, extra = _identify_gguf(model_file)
        assert model_type == "llm"
        assert family == "mistral"

    def test_valid_gguf_with_unknown_architecture_defaults_to_llm(self, tmp_path: Path) -> None:
        """Test that unknown GGUF architectures default to LLM."""
        model_file = tmp_path / "model.gguf"
        _create_gguf_file(model_file, "some_new_architecture")

        model_type, family, extra = _identify_gguf(model_file)
        assert model_type == "llm"  # Unknown but still LLM since GGUF is LLM-specific
        assert family == "some_new_architecture"

    def test_gguf_parsing_error_defaults_to_llm(self, tmp_path: Path) -> None:
        """Test that GGUF files with parsing errors still return LLM."""
        model_file = tmp_path / "model.gguf"

        # Valid magic but corrupted data after
        data = b"GGUF"
        data += struct.pack("<I", 3)  # version
        data += struct.pack("<Q", 0)  # tensor_count
        data += struct.pack("<Q", 1)  # metadata_count = 1
        # Corrupted/incomplete metadata follows
        data += b"\xff" * 100  # garbage data

        model_file.write_bytes(data)

        model_type, family, extra = _identify_gguf(model_file)
        # Should still be LLM because GGUF magic was valid
        assert model_type == "llm"


@pytest.mark.unit
class TestIdentifySafetensors:
    """Tests for safetensors file identification."""

    def test_invalid_header_returns_none(self, tmp_path: Path) -> None:
        """Test that invalid safetensors files return None."""
        model_file = tmp_path / "model.safetensors"
        model_file.write_bytes(b"not valid")
        model_type, family, extra = _identify_safetensors(model_file)
        assert model_type is None

    def test_empty_file_returns_none(self, tmp_path: Path) -> None:
        """Test that empty files return None."""
        model_file = tmp_path / "model.safetensors"
        model_file.touch()
        model_type, family, extra = _identify_safetensors(model_file)
        assert model_type is None

    def test_valid_safetensors_with_llm_tensors(self, tmp_path: Path) -> None:
        """Test that safetensors with LLM tensor patterns are identified."""
        model_file = tmp_path / "model.safetensors"

        # Create valid safetensors header with LLM-style tensor names
        header = {
            "model.layers.0.self_attn.q_proj.weight": {
                "dtype": "F16",
                "shape": [4096, 4096],
                "data_offsets": [0, 100],
            },
            "model.layers.0.self_attn.k_proj.weight": {
                "dtype": "F16",
                "shape": [4096, 4096],
                "data_offsets": [100, 200],
            },
            "model.layers.0.self_attn.v_proj.weight": {
                "dtype": "F16",
                "shape": [4096, 4096],
                "data_offsets": [200, 300],
            },
            "model.layers.0.mlp.up_proj.weight": {
                "dtype": "F16",
                "shape": [4096, 11008],
                "data_offsets": [300, 400],
            },
            "model.layers.0.mlp.down_proj.weight": {
                "dtype": "F16",
                "shape": [11008, 4096],
                "data_offsets": [400, 500],
            },
            "lm_head.weight": {"dtype": "F16", "shape": [32000, 4096], "data_offsets": [500, 600]},
            "embed_tokens.weight": {
                "dtype": "F16",
                "shape": [32000, 4096],
                "data_offsets": [600, 700],
            },
        }
        header_json = json.dumps(header).encode("utf-8")
        header_len = struct.pack("<Q", len(header_json))

        model_file.write_bytes(header_len + header_json + b"\x00" * 1000)

        model_type, family, extra = _identify_safetensors(model_file)
        assert model_type == "llm"

    def test_valid_safetensors_with_diffusion_tensors(self, tmp_path: Path) -> None:
        """Test that safetensors with diffusion tensor patterns are identified."""
        model_file = tmp_path / "model.safetensors"

        header = {
            "down_blocks.0.attentions.0.to_q.weight": {
                "dtype": "F16",
                "shape": [320, 320],
                "data_offsets": [0, 100],
            },
            "down_blocks.0.attentions.0.to_k.weight": {
                "dtype": "F16",
                "shape": [320, 320],
                "data_offsets": [100, 200],
            },
            "down_blocks.0.attentions.0.to_v.weight": {
                "dtype": "F16",
                "shape": [320, 320],
                "data_offsets": [200, 300],
            },
            "up_blocks.0.attentions.0.to_q.weight": {
                "dtype": "F16",
                "shape": [320, 320],
                "data_offsets": [300, 400],
            },
            "mid_block.attentions.0.to_q.weight": {
                "dtype": "F16",
                "shape": [1280, 1280],
                "data_offsets": [400, 500],
            },
            "time_embedding.linear_1.weight": {
                "dtype": "F16",
                "shape": [1280, 320],
                "data_offsets": [500, 600],
            },
            "conv_in.weight": {"dtype": "F16", "shape": [320, 4, 3, 3], "data_offsets": [600, 700]},
        }
        header_json = json.dumps(header).encode("utf-8")
        header_len = struct.pack("<Q", len(header_json))

        model_file.write_bytes(header_len + header_json + b"\x00" * 1000)

        model_type, family, extra = _identify_safetensors(model_file)
        assert model_type == "diffusion"


@pytest.mark.unit
class TestNormalizeArchitecture:
    """Tests for architecture name normalization."""

    def test_llama_variants(self) -> None:
        """Test that llama variants are normalized."""
        assert _normalize_gguf_architecture("llama") == "llama"
        assert _normalize_gguf_architecture("codellama") == "llama"

    def test_mistral_variants(self) -> None:
        """Test that mistral variants are normalized."""
        assert _normalize_gguf_architecture("mistral") == "mistral"
        assert _normalize_gguf_architecture("mixtral") == "mistral"

    def test_phi_variants(self) -> None:
        """Test that phi variants are normalized."""
        assert _normalize_gguf_architecture("phi") == "phi"
        assert _normalize_gguf_architecture("phi2") == "phi"
        assert _normalize_gguf_architecture("phi3") == "phi"

    def test_unknown_architecture_passthrough(self) -> None:
        """Test that unknown architectures pass through unchanged."""
        assert _normalize_gguf_architecture("some_new_arch") == "some_new_arch"


@pytest.mark.unit
class TestInferFamilyFromTensors:
    """Tests for LLM family inference from tensor names."""

    def test_infer_llama_family(self) -> None:
        """Test that llama patterns are detected."""
        tensor_names = ["llama.layers.0.weight", "llama.embed.weight"]
        assert _infer_family_from_tensors(tensor_names) == "llama"

    def test_infer_mistral_family(self) -> None:
        """Test that mistral patterns are detected."""
        tensor_names = ["mistral.layers.0.weight", "mistral.embed.weight"]
        assert _infer_family_from_tensors(tensor_names) == "mistral"

    def test_infer_falcon_family(self) -> None:
        """Test that falcon patterns are detected."""
        tensor_names = ["falcon.layers.0.weight", "falcon.embed.weight"]
        assert _infer_family_from_tensors(tensor_names) == "falcon"

    def test_infer_gpt_neox_family(self) -> None:
        """Test that gpt_neox patterns are detected."""
        tensor_names = ["gpt_neox.layers.0.weight", "gpt_neox.embed.weight"]
        assert _infer_family_from_tensors(tensor_names) == "gpt-neox"

    def test_infer_phi_family(self) -> None:
        """Test that phi patterns are detected."""
        tensor_names = ["phi.layers.0.weight", "phi.embed.weight"]
        assert _infer_family_from_tensors(tensor_names) == "phi"

    def test_infer_qwen_family(self) -> None:
        """Test that qwen patterns are detected."""
        tensor_names = ["qwen.layers.0.weight", "qwen.embed.weight"]
        assert _infer_family_from_tensors(tensor_names) == "qwen"

    def test_infer_gemma_family(self) -> None:
        """Test that gemma patterns are detected."""
        tensor_names = ["gemma.layers.0.weight", "gemma.embed.weight"]
        assert _infer_family_from_tensors(tensor_names) == "gemma"

    def test_infer_unknown_returns_none(self) -> None:
        """Test that unknown patterns return None."""
        tensor_names = ["model.layers.0.weight", "model.embed.weight"]
        assert _infer_family_from_tensors(tensor_names) is None


@pytest.mark.unit
class TestInferDiffusionFamily:
    """Tests for diffusion family inference."""

    def test_infer_sdxl_from_tensors(self) -> None:
        """Test that sdxl patterns are detected from tensors."""
        tensor_names = ["sdxl.unet.weight", "sdxl.vae.weight"]
        assert _infer_diffusion_family(tensor_names, {}) == "sdxl"

    def test_infer_sdxl_from_metadata(self) -> None:
        """Test that sdxl patterns are detected from metadata."""
        tensor_names = ["model.weight"]
        metadata = {"name": "SDXL_model"}
        assert _infer_diffusion_family(tensor_names, metadata) == "sdxl"

    def test_infer_flux_from_tensors(self) -> None:
        """Test that flux patterns are detected from tensors."""
        tensor_names = ["flux.unet.weight", "flux.vae.weight"]
        assert _infer_diffusion_family(tensor_names, {}) == "flux"

    def test_infer_flux_from_metadata(self) -> None:
        """Test that flux patterns are detected from metadata."""
        tensor_names = ["model.weight"]
        metadata = {"name": "Flux_model"}
        assert _infer_diffusion_family(tensor_names, metadata) == "flux"

    def test_infer_sd3_from_tensors(self) -> None:
        """Test that sd3 patterns are detected from tensors."""
        tensor_names = ["sd3.unet.weight", "sd3.vae.weight"]
        assert _infer_diffusion_family(tensor_names, {}) == "sd3"

    def test_infer_sd3_from_metadata(self) -> None:
        """Test that sd3 patterns are detected from metadata."""
        tensor_names = ["model.weight"]
        metadata = {"name": "SD3_model"}
        assert _infer_diffusion_family(tensor_names, metadata) == "sd3"

    def test_infer_default_stable_diffusion(self) -> None:
        """Test that unknown diffusion defaults to stable-diffusion."""
        tensor_names = ["model.weight"]
        assert _infer_diffusion_family(tensor_names, {}) == "stable-diffusion"
