"""Local Klein 9B KV FP8 checkpoint with explicit scaled BF16 execution."""

import json
from pathlib import Path

import torch
from safetensors import safe_open

from diffusion import DiffusionPipelineAdapter


def load_scaled_fp8(checkpoint):
    """Decode the declared scalar-scaled FP8 format without discarding scales."""
    with safe_open(checkpoint, framework="pt", device="cpu") as source:
        metadata = json.loads((source.metadata() or {}).get("_quantization_metadata", "{}"))
        if not isinstance(metadata, dict) or metadata.get("format_version") != "1.0":
            raise ValueError("Unsupported FP8 metadata version")
        layers = metadata.get("layers")
        if not isinstance(layers, dict) or not layers:
            raise ValueError("FP8 checkpoint has no declared quantized layers")
        keys = set(source.keys())
        scales = {}
        auxiliary = set()
        for layer, description in layers.items():
            if not isinstance(description, dict) or description != {"format": "float8_e4m3fn"}:
                raise ValueError("Unsupported FP8 layer format")
            weight = f"{layer}.weight"
            if weight not in keys or source.get_slice(weight).get_dtype() != "F8_E4M3":
                raise ValueError("Declared FP8 weight is missing or has the wrong dtype")
            for suffix in ("weight_scale", "input_scale"):
                key = f"{layer}.{suffix}"
                if key not in keys:
                    raise ValueError("FP8 scale is missing")
                value = source.get_tensor(key)
                if value.dtype != torch.float32 or value.numel() != 1:
                    raise ValueError("FP8 scales must be scalar float32 values")
                if not torch.isfinite(value).all() or value.item() <= 0:
                    raise ValueError("FP8 scale must be finite and positive")
                auxiliary.add(key)
                if suffix == "weight_scale":
                    scales[weight] = value
        for key in keys - auxiliary:
            dtype = source.get_slice(key).get_dtype()
            if dtype == "F8_E4M3" and key not in scales:
                raise ValueError("Undeclared FP8 weight")
            if dtype not in {"F8_E4M3", "BF16", "F32"} or key.endswith("_scale"):
                raise ValueError("Unsupported checkpoint tensor")
        state = {}
        for key in sorted(keys - auxiliary):
            value = source.get_tensor(key)
            if key in scales:
                value = value.float().mul_(scales[key])
            state[key] = value.to(torch.bfloat16)
        # input_scale calibrates FP8 activations. This adapter executes BF16
        # activations and explicitly reports that policy instead of FP8 compute.
        return state


class Flux2Klein(DiffusionPipelineAdapter):
    steps = 4
    guidance = 1.0
    memory_policy = "scaled_fp8_to_bf16_sequential_cpu_offload"

    def __init__(self, checkpoint, encoder_path, vae_path, device):
        from diffusers import (
            AutoencoderKLFlux2,
            FlowMatchEulerDiscreteScheduler,
            Flux2KleinPipeline,
            Flux2Transformer2DModel,
        )
        from diffusers.loaders.single_file_utils import (
            convert_flux2_transformer_checkpoint_to_diffusers,
        )
        from safetensors.torch import load_file
        from transformers import Qwen2TokenizerFast, Qwen3ForCausalLM

        if device.type != "cuda" or torch.cuda.get_device_capability(device) != (12, 0):
            raise ValueError("This Klein recipe requires the qualified sm_120 CUDA device")
        if Path(checkpoint).name != "flux-2-klein-9b-kv-fp8.safetensors":
            raise ValueError("Select the qualified Klein 9B KV FP8 checkpoint")
        if not vae_path or not Path(vae_path).is_file():
            raise ValueError("The library-managed FLUX.2 VAE is missing")
        config = json.loads((Path(encoder_path) / "config.json").read_text())
        if config.get("model_type") != "qwen3" or config.get("hidden_size") != 4096:
            raise ValueError("Klein 9B requires the matching Qwen3-8B encoder")
        with torch.device("meta"):
            transformer = Flux2Transformer2DModel(
                num_layers=8,
                num_single_layers=24,
                num_attention_heads=32,
                attention_head_dim=128,
                joint_attention_dim=12288,
                guidance_embeds=False,
            )
            vae = AutoencoderKLFlux2()
        vae.load_state_dict(load_file(vae_path), strict=True, assign=True)
        # The standalone VAE is stored in FP32; Klein produces BF16 latents.
        vae.to(dtype=torch.bfloat16).eval()
        state = convert_flux2_transformer_checkpoint_to_diffusers(load_scaled_fp8(checkpoint))
        transformer.load_state_dict(state, strict=True, assign=True)
        del state
        transformer.eval()
        encoder = Qwen3ForCausalLM.from_pretrained(
            encoder_path,
            torch_dtype=torch.bfloat16,
            local_files_only=True,
            low_cpu_mem_usage=True,
        )
        tokenizer = Qwen2TokenizerFast.from_pretrained(encoder_path, local_files_only=True)
        self.pipeline = Flux2KleinPipeline(
            scheduler=FlowMatchEulerDiscreteScheduler(use_dynamic_shifting=True, shift=3.0),
            vae=vae,
            text_encoder=encoder,
            tokenizer=tokenizer,
            transformer=transformer,
            is_distilled=True,
        )
        self.pipeline.enable_sequential_cpu_offload(gpu_id=device.index or 0)
