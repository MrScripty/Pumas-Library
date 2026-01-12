"""Model type identification from file contents.

Reads embedded metadata from model files to determine type without
relying on file extensions or external lookups.

This module provides content-based model identification for:
- GGUF files: Reads `general.architecture` field to identify LLM family
- Safetensors files: Analyzes tensor name patterns to distinguish LLM vs diffusion

Usage:
    from backend.model_library.model_identifier import identify_model_type

    model_type, family, extra = identify_model_type(Path("model.gguf"))
    # model_type: "llm" or "diffusion" or None
    # family: "llama", "mistral", "stable-diffusion", etc.
    # extra: dict with additional metadata for search refinement
"""

import json
import struct
from pathlib import Path
from typing import Optional, Tuple

from backend.logging_config import get_logger

logger = get_logger(__name__)

# Make gguf import defensive - allows launcher to start even if dependency is missing
try:
    from gguf import GGUFReader

    HAS_GGUF = True
except ImportError:
    HAS_GGUF = False
    GGUFReader = None  # type: ignore
    logger.warning("gguf library not found; GGUF content detection will be disabled")


def identify_model_type(file_path: Path) -> Tuple[Optional[str], Optional[str], dict]:
    """Identify model type by reading file contents.

    Args:
        file_path: Path to model file

    Returns:
        Tuple of (model_type, family, extra_metadata)
        - model_type: "llm" or "diffusion" or None if unknown
        - family: e.g., "llama", "mistral", "stable-diffusion" or None
        - extra_metadata: dict with additional info for HF search refinement
    """
    suffix = file_path.suffix.lower()

    if suffix == ".gguf":
        return _identify_gguf(file_path)
    elif suffix == ".safetensors":
        return _identify_safetensors(file_path)

    return None, None, {}


def _identify_gguf(file_path: Path) -> Tuple[Optional[str], Optional[str], dict]:
    """Read GGUF header to identify model architecture using the gguf library.

    GGUF files contain a `general.architecture` field that directly
    identifies the model family (llama, mistral, falcon, etc.).
    All known GGUF architectures are LLMs.

    Note: GGUF format was created specifically for llama.cpp (LLMs), so
    any valid GGUF file is assumed to be an LLM even if parsing fails.
    """
    if not HAS_GGUF:
        # Fall back to extension-based detection when gguf library is unavailable
        logger.debug(
            "GGUF library not available, using extension-based detection for %s", file_path
        )
        return "llm", None, {"warning": "gguf-library-missing"}

    try:
        # First do a quick magic check without loading the full file
        with open(file_path, "rb") as f:
            magic = f.read(4)
            if magic != b"GGUF":
                return None, None, {}

        # Use the official gguf library to read metadata
        reader = GGUFReader(file_path)

        # Extract architecture from metadata
        arch_field = reader.get_field("general.architecture")
        if arch_field is not None:
            # The field value is stored in parts, accessed via data indices
            architecture = str(bytes(arch_field.parts[arch_field.data[0]]), "utf-8").lower()
        else:
            architecture = ""

        # Known LLM architectures in GGUF
        llm_architectures = {
            "llama",
            "mistral",
            "mixtral",
            "gemma",
            "phi",
            "phi2",
            "phi3",
            "qwen",
            "qwen2",
            "yi",
            "falcon",
            "mpt",
            "bloom",
            "gptneox",
            "gptj",
            "gpt2",
            "starcoder",
            "starcoder2",
            "codellama",
            "deepseek",
            "internlm",
            "baichuan",
            "chatglm",
            "orion",
            "minicpm",
            "stablelm",
            "mamba",
            "rwkv",
            "olmo",
            "command-r",
            "dbrx",
            "jamba",
            "granite",
            "exaone",
            "solar",
            "nomic-bert",
        }

        if architecture in llm_architectures:
            # Map architecture to family name
            family = _normalize_gguf_architecture(architecture)
            logger.info("GGUF identified: architecture=%s, family=%s", architecture, family)
            return "llm", family, {"gguf_architecture": architecture}

        # Unknown architecture - still LLM since GGUF is LLM-specific
        if architecture:
            logger.info("Unknown GGUF architecture: %s, defaulting to LLM", architecture)
            return "llm", architecture, {"gguf_architecture": architecture}

        # No architecture field but valid GGUF - assume LLM
        logger.info("GGUF file without architecture field, defaulting to LLM")
        return "llm", None, {}

    except OSError as e:
        logger.warning("Failed to read GGUF file %s: %s", file_path, e)
        return None, None, {}
    except ValueError as e:
        # Parsing error but we saw GGUF magic - still an LLM
        logger.info("GGUF parsing error for %s: %s, assuming LLM", file_path, e)
        return "llm", None, {}
    except struct.error as e:
        # Struct unpacking error but we saw GGUF magic - still an LLM
        logger.info("GGUF struct error for %s: %s, assuming LLM", file_path, e)
        return "llm", None, {}
    except UnicodeDecodeError as e:
        # String decoding error but we saw GGUF magic - still an LLM
        logger.info("GGUF decode error for %s: %s, assuming LLM", file_path, e)
        return "llm", None, {}
    except IndexError as e:
        # Corrupted header but we saw GGUF magic - still an LLM
        logger.info("GGUF index error for %s: %s, assuming LLM", file_path, e)
        return "llm", None, {}


def _identify_safetensors(file_path: Path) -> Tuple[Optional[str], Optional[str], dict]:
    """Read safetensors header to identify model type from tensor names.

    LLM models have tensor names like:
      - model.layers.*.self_attn.*_proj.weight
      - lm_head.weight, embed_tokens.weight

    Diffusion models have tensor names like:
      - down_blocks.*, up_blocks.*, mid_block.*
      - time_embedding.*, time_emb_proj.*
    """
    try:
        with open(file_path, "rb") as f:
            # Read header length (8 bytes, little-endian uint64)
            header_len_bytes = f.read(8)
            if len(header_len_bytes) < 8:
                return None, None, {}

            header_len = struct.unpack("<Q", header_len_bytes)[0]

            # Sanity check - header shouldn't be gigabytes
            if header_len > 100 * 1024 * 1024:  # 100MB max header
                logger.warning("Safetensors header too large (%d bytes), skipping", header_len)
                return None, None, {}

            # Read JSON header
            header_json = f.read(header_len).decode("utf-8")
            header = json.loads(header_json)

        # Get tensor names (exclude __metadata__ key)
        tensor_names = [k for k in header.keys() if not k.startswith("__")]

        # Check for LLM patterns
        llm_indicators = 0
        diffusion_indicators = 0

        for name in tensor_names:
            name_lower = name.lower()

            # LLM indicators
            if any(
                p in name_lower
                for p in [
                    "self_attn",
                    "lm_head",
                    "embed_tokens",
                    "model.layers.",
                    "transformer.h.",
                    "gpt_neox",
                    "mlp.up_proj",
                    "mlp.down_proj",
                    "input_layernorm",
                    "post_attention_layernorm",
                    "rotary_emb",
                ]
            ):
                llm_indicators += 1

            # Diffusion indicators
            if any(
                p in name_lower
                for p in [
                    "down_blocks",
                    "up_blocks",
                    "mid_block",
                    "time_embedding",
                    "time_emb_proj",
                    "conv_in",
                    "conv_out",
                    "unet",
                    "vae",
                    "encoder.down",
                    "decoder.up",
                    "quant_conv",
                    "post_quant_conv",
                ]
            ):
                diffusion_indicators += 1

        # Check __metadata__ for additional hints
        metadata = header.get("__metadata__", {})
        if not isinstance(metadata, dict):
            metadata = {}

        extra = {"safetensors_metadata": metadata}

        logger.debug(
            "Safetensors analysis: llm_indicators=%d, diffusion_indicators=%d",
            llm_indicators,
            diffusion_indicators,
        )

        # Decide based on indicators
        if llm_indicators > diffusion_indicators and llm_indicators > 5:
            family = _infer_family_from_tensors(tensor_names)
            logger.info(
                "Safetensors identified as LLM: family=%s, indicators=%d", family, llm_indicators
            )
            return "llm", family, extra
        elif diffusion_indicators > llm_indicators and diffusion_indicators > 5:
            family = _infer_diffusion_family(tensor_names, metadata)
            logger.info(
                "Safetensors identified as diffusion: family=%s, indicators=%d",
                family,
                diffusion_indicators,
            )
            return "diffusion", family, extra

        # Ambiguous - check file size and other heuristics
        # Large single files (>1GB) with some LLM patterns are likely LLMs
        file_size = file_path.stat().st_size
        if file_size > 1_000_000_000 and llm_indicators > 0:
            logger.info(
                "Large safetensors file (%d bytes) with LLM patterns, assuming LLM", file_size
            )
            return "llm", None, extra

        # Unable to determine
        logger.debug(
            "Could not determine safetensors type (llm=%d, diffusion=%d)",
            llm_indicators,
            diffusion_indicators,
        )

    except OSError as e:
        logger.warning("Failed to read safetensors file %s: %s", file_path, e)
    except json.JSONDecodeError as e:
        logger.warning("Failed to parse safetensors JSON header from %s: %s", file_path, e)
    except struct.error as e:
        logger.warning("Failed to unpack safetensors header from %s: %s", file_path, e)
    except UnicodeDecodeError as e:
        logger.warning("Failed to decode safetensors header from %s: %s", file_path, e)

    return None, None, {}


def _normalize_gguf_architecture(arch: str) -> str:
    """Normalize GGUF architecture name to family name."""
    mappings = {
        "llama": "llama",
        "codellama": "llama",
        "mistral": "mistral",
        "mixtral": "mistral",
        "gemma": "gemma",
        "phi": "phi",
        "phi2": "phi",
        "phi3": "phi",
        "qwen": "qwen",
        "qwen2": "qwen",
        "falcon": "falcon",
        "starcoder": "starcoder",
        "starcoder2": "starcoder",
        "deepseek": "deepseek",
        "yi": "yi",
        "mpt": "mpt",
        "bloom": "bloom",
        "gptneox": "gpt-neox",
        "gptj": "gpt-j",
        "gpt2": "gpt2",
        "internlm": "internlm",
        "baichuan": "baichuan",
        "chatglm": "chatglm",
        "mamba": "mamba",
        "rwkv": "rwkv",
        "olmo": "olmo",
        "command-r": "command-r",
        "dbrx": "dbrx",
        "jamba": "jamba",
        "granite": "granite",
        "exaone": "exaone",
        "solar": "solar",
    }
    return mappings.get(arch, arch)


def _infer_family_from_tensors(tensor_names: list) -> Optional[str]:
    """Try to infer LLM family from tensor naming patterns."""
    sample = " ".join(tensor_names[:50]).lower()

    if "llama" in sample:
        return "llama"
    if "mistral" in sample:
        return "mistral"
    if "falcon" in sample:
        return "falcon"
    if "gpt_neox" in sample or "gptneox" in sample:
        return "gpt-neox"
    if "phi" in sample:
        return "phi"
    if "qwen" in sample:
        return "qwen"
    if "gemma" in sample:
        return "gemma"

    return None


def _infer_diffusion_family(tensor_names: list, metadata: dict) -> Optional[str]:
    """Try to infer diffusion model family."""
    sample = " ".join(tensor_names[:50]).lower()
    metadata_str = str(metadata).lower()

    if "sdxl" in sample or "sdxl" in metadata_str:
        return "sdxl"
    if "flux" in sample or "flux" in metadata_str:
        return "flux"
    if "sd3" in sample or "sd3" in metadata_str:
        return "sd3"

    return "stable-diffusion"  # Default diffusion family


# ============================================================================
# GGUF Embedded Metadata Extraction
# ============================================================================


def extract_gguf_metadata(file_path: Path) -> Optional[dict]:
    """Extract all embedded metadata from a GGUF file.

    This reads the GGUF header and extracts all metadata fields stored
    in the file, which can include:
    - general.architecture, general.name, general.author
    - general.description, general.license
    - quantization parameters, context length, etc.

    Args:
        file_path: Path to the GGUF file

    Returns:
        Dict of metadata key-value pairs, or None if extraction fails
    """
    if not HAS_GGUF:
        logger.warning("gguf library not available, cannot extract metadata from %s", file_path)
        return None

    try:
        # Quick magic check
        with open(file_path, "rb") as f:
            magic = f.read(4)
            if magic != b"GGUF":
                logger.debug("File %s is not a GGUF file (magic: %s)", file_path, magic)
                return None

        # Use the official gguf library to read metadata
        reader = GGUFReader(file_path)

        metadata: dict = {}

        # Extract all fields from the reader using the library's built-in method
        for field in reader.fields.values():
            try:
                key = field.name

                # Use the library's built-in contents() method for proper type handling
                # This correctly handles strings, arrays, and numeric types
                value = field.contents()

                if value is not None:
                    # Convert numpy types to Python native types for JSON serialization
                    if hasattr(value, "tolist"):
                        value = value.tolist()
                    elif hasattr(value, "item"):
                        value = value.item()

                    metadata[key] = value

            except (IndexError, ValueError, TypeError) as e:  # noqa: multi-exception
                logger.debug("Failed to extract field %s: %s", field.name, e)
                continue
            except Exception as e:  # noqa: generic-exception
                logger.debug("Unexpected error extracting field %s: %s", field.name, e)
                continue

        logger.info("Extracted %d metadata fields from GGUF file %s", len(metadata), file_path.name)
        return metadata if metadata else None

    except OSError as e:
        logger.warning("Failed to read GGUF file %s: %s", file_path, e)
        return None
    except ValueError as e:
        logger.warning("Failed to parse GGUF file %s: %s", file_path, e)
        return None
    except struct.error as e:
        logger.warning("Struct error reading GGUF file %s: %s", file_path, e)
        return None
    except UnicodeDecodeError as e:
        logger.warning("Unicode error reading GGUF file %s: %s", file_path, e)
        return None
    except IndexError as e:
        logger.warning("Index error reading GGUF file %s: %s", file_path, e)
        return None
