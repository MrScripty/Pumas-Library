"""Concrete, offline diffusion adapters; model paths are resolved by Pumas."""

import json
import logging
import threading
from pathlib import Path

import torch

from validation import ModelLoadRefusal

logger = logging.getLogger(__name__)

NUNCHAKU_Z_IMAGE = "nunchaku-z-image-turbo"
FLUX2_KLEIN = "flux2-klein-9b-kv-fp8"
IMAGE_ADAPTERS = (NUNCHAKU_Z_IMAGE, FLUX2_KLEIN)

# Z-Image Turbo's model_index.json may name only these (library, class) pairs.
# diffusers imports the named library and instantiates the named class, so an
# unchecked index lets a downloaded file choose which installed code runs.
Z_IMAGE_PIPELINE_CLASS = "ZImagePipeline"
Z_IMAGE_COMPONENTS = {
    "scheduler": {("diffusers", "FlowMatchEulerDiscreteScheduler")},
    "text_encoder": {("transformers", "Qwen3Model")},
    "tokenizer": {("transformers", "Qwen2Tokenizer"), ("transformers", "Qwen2TokenizerFast")},
    "transformer": {("diffusers", "ZImageTransformer2DModel")},
    "vae": {("diffusers", "AutoencoderKL")},
}


class GenerationCancelled(RuntimeError):
    """The worker has reached a cancellation checkpoint and stopped computing."""


def validate_pipeline_index(index_path: Path, pipeline_class: str, components: dict) -> None:
    """Refuse a model_index.json that names any class outside the adapter's allowlist."""
    try:
        index = json.loads(index_path.read_text(encoding="utf-8"))
    except (OSError, UnicodeDecodeError, json.JSONDecodeError):
        raise ModelLoadRefusal("Pipeline model_index.json is unreadable") from None
    if not isinstance(index, dict) or index.get("_class_name") != pipeline_class:
        raise ModelLoadRefusal(f"Pipeline model_index.json must declare {pipeline_class}")
    for name, value in index.items():
        if name.startswith("_"):
            continue
        allowed = components.get(name)
        if allowed is None:
            raise ModelLoadRefusal(f"Pipeline component is not supported: {name}")
        if (
            not isinstance(value, list)
            or len(value) != 2
            or not all(isinstance(part, str) for part in value)
            or tuple(value) not in allowed
        ):
            raise ModelLoadRefusal(f"Pipeline component {name} names an unsupported class")
    missing = sorted(set(components) - set(index))
    if missing:
        raise ModelLoadRefusal(f"Pipeline model_index.json is missing: {', '.join(missing)}")


def finish_device_work(device, failure: BaseException | None) -> None:
    """Wait for queued CUDA kernels before the caller releases the device lease.

    Non-CUDA runs have nothing to wait for. When a failure is already
    propagating, a synchronize error is logged rather than raised, so it cannot
    replace the original cause.
    """
    if not torch.cuda.is_available():
        return
    try:
        if device is None:
            torch.cuda.synchronize()
        else:
            torch.cuda.synchronize(device)
    except Exception:
        if failure is None:
            raise
        logger.warning("CUDA synchronize failed after an earlier failure", exc_info=True)


class DiffusionPipelineAdapter:
    """Shared denoising cancellation and offload cleanup ownership."""

    dimension_multiple = 1
    device = None
    # Admission facts each adapter declares (see image_api.admit_dimensions).
    # max_pixels: the largest output the adapter's qualified recipe supports.
    # peak_bytes_per_pixel: device memory one output pixel needs at peak under
    # the adapter's offload policy; full-resolution VAE decode dominates.
    max_pixels = None
    peak_bytes_per_pixel = None

    def generate(self, prompt: str, width: int, height: int, seed: int, cancel: threading.Event):
        def checkpoint(_pipeline, _step, _timestep, callback_kwargs):
            if cancel.is_set():
                raise GenerationCancelled("Generation cancelled")
            return callback_kwargs

        if cancel.is_set():
            raise GenerationCancelled("Generation cancelled")
        multiple = self.dimension_multiple
        pipeline_width = (width + multiple - 1) // multiple * multiple
        pipeline_height = (height + multiple - 1) // multiple * multiple
        failure = None
        try:
            with torch.inference_mode():
                result = self.pipeline(
                    prompt=prompt,
                    width=pipeline_width,
                    height=pipeline_height,
                    num_inference_steps=self.steps,
                    guidance_scale=self.guidance,
                    generator=torch.Generator(device="cpu").manual_seed(seed),
                    callback_on_step_end=checkpoint,
                ).images[0]
            if cancel.is_set():
                raise GenerationCancelled("Generation cancelled")
            if (pipeline_width, pipeline_height) != (width, height) and result.size == (
                pipeline_width,
                pipeline_height,
            ):
                left = (pipeline_width - width) // 2
                top = (pipeline_height - height) // 2
                result = result.crop((left, top, left + width, top + height))
            return result
        except BaseException as error:
            failure = error
            raise
        finally:
            # CUDA kernels and offload hooks must finish before the caller releases
            # the device lease, even on callback cancellation or out-of-memory.
            try:
                finish_device_work(self.device, failure)
            finally:
                self.pipeline.maybe_free_model_hooks()


class NunchakuZImage(DiffusionPipelineAdapter):
    """FP4 Z-Image Turbo with explicit sequential CPU offload."""

    steps = 8
    guidance = 0.0
    memory_policy = "sequential_cpu_offload"
    dimension_multiple = 16
    # Z-Image Turbo is published for outputs up to 2048x2048.
    max_pixels = 2048 * 2048
    peak_bytes_per_pixel = 1024

    def __init__(self, checkpoint: str, pipeline_path: str, device: torch.device):
        from diffusers import ZImagePipeline
        from nunchaku_compat import PumasNunchakuZImageTransformer
        from safetensors import safe_open

        if device.type != "cuda" or torch.cuda.get_device_capability(device) != (12, 0):
            raise ModelLoadRefusal("Nunchaku FP4 requires an sm_120 CUDA device")
        self.device = device
        checkpoint_path = Path(checkpoint)
        if checkpoint_path.name != "svdq-fp4_r128-z-image-turbo.safetensors":
            raise ModelLoadRefusal(
                "Select the qualified Nunchaku Z-Image Turbo FP4 rank-128 checkpoint"
            )
        if not checkpoint_path.is_file():
            raise ModelLoadRefusal("Nunchaku checkpoint is missing")
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
            raise ModelLoadRefusal("Checkpoint is not a Nunchaku Z-Image FP4 rank-128 model")
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
                raise ModelLoadRefusal(f"Pipeline component is missing: {relative}")
        validate_pipeline_index(
            base / "model_index.json", Z_IMAGE_PIPELINE_CLASS, Z_IMAGE_COMPONENTS
        )
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
