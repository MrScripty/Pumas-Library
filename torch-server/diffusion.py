"""Concrete, offline diffusion adapters; model paths are resolved by Pumas."""

import json
import threading
from pathlib import Path

import torch

NUNCHAKU_Z_IMAGE = "nunchaku-z-image-turbo"
FLUX2_KLEIN = "flux2-klein-9b-kv-fp8"
IMAGE_ADAPTERS = (NUNCHAKU_Z_IMAGE, FLUX2_KLEIN)


class GenerationCancelled(RuntimeError):
    """The worker has reached a cancellation checkpoint and stopped computing."""


class DiffusionPipelineAdapter:
    """Shared denoising cancellation and offload cleanup ownership."""

    def generate(self, prompt: str, width: int, height: int, seed: int, cancel: threading.Event):
        def checkpoint(_pipeline, _step, _timestep, callback_kwargs):
            if cancel.is_set():
                raise GenerationCancelled("Generation cancelled")
            return callback_kwargs

        if cancel.is_set():
            raise GenerationCancelled("Generation cancelled")
        try:
            with torch.inference_mode():
                result = self.pipeline(
                    prompt=prompt,
                    width=width,
                    height=height,
                    num_inference_steps=self.steps,
                    guidance_scale=self.guidance,
                    generator=torch.Generator(device="cpu").manual_seed(seed),
                    callback_on_step_end=checkpoint,
                ).images[0]
            if cancel.is_set():
                raise GenerationCancelled("Generation cancelled")
            return result
        finally:
            # CUDA kernels and offload hooks must finish before the caller releases
            # the device lease, even on callback cancellation or out-of-memory.
            torch.cuda.synchronize()
            self.pipeline.maybe_free_model_hooks()


class NunchakuZImage(DiffusionPipelineAdapter):
    """FP4 Z-Image Turbo with explicit sequential CPU offload."""

    steps = 8
    guidance = 0.0
    memory_policy = "sequential_cpu_offload"

    def __init__(self, checkpoint: str, pipeline_path: str, device: torch.device):
        from diffusers import ZImagePipeline
        from nunchaku_compat import PumasNunchakuZImageTransformer
        from safetensors import safe_open

        if device.type != "cuda" or torch.cuda.get_device_capability(device) != (12, 0):
            raise ValueError("Nunchaku FP4 requires an sm_120 CUDA device")
        checkpoint_path = Path(checkpoint)
        if checkpoint_path.name != "svdq-fp4_r128-z-image-turbo.safetensors":
            raise ValueError("Select the qualified Nunchaku Z-Image Turbo FP4 rank-128 checkpoint")
        if not checkpoint_path.is_file():
            raise ValueError("Nunchaku checkpoint is missing")
        with safe_open(checkpoint_path, framework="pt", device="cpu") as source:
            metadata = source.metadata() or {}
        quantization = json.loads(metadata.get("quantization_config", "{}"))
        if (
            metadata.get("model_class") != "NunchakuZImageTransformer2DModel"
            or not isinstance(quantization, dict)
            or quantization.get("rank") != 128
            or not isinstance(quantization.get("weight"), dict)
            or quantization["weight"].get("dtype") != "fp4_e2m1_all"
        ):
            raise ValueError("Checkpoint is not a Nunchaku Z-Image FP4 rank-128 model")
        base = Path(pipeline_path)
        for relative in (
            "model_index.json",
            "text_encoder/config.json",
            "text_encoder/model.safetensors.index.json",
            "tokenizer/tokenizer_config.json",
            "vae/config.json",
            "vae/diffusion_pytorch_model.safetensors",
            "scheduler/scheduler_config.json",
        ):
            if not (base / relative).is_file():
                raise ValueError(f"Pipeline component is missing: {relative}")
        transformer = PumasNunchakuZImageTransformer.from_pretrained(
            str(checkpoint_path), torch_dtype=torch.bfloat16
        )
        self.pipeline = ZImagePipeline.from_pretrained(
            str(base),
            transformer=transformer,
            torch_dtype=torch.bfloat16,
            local_files_only=True,
            low_cpu_mem_usage=False,
        )
        self.pipeline.enable_sequential_cpu_offload(gpu_id=device.index or 0)
