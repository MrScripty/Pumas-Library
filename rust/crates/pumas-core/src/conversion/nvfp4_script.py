#!/usr/bin/env python3
"""Calibrate and export local Transformers packages as packed ModelOpt NVFP4.

Uses the shared conversion worker's temporary output directory. Publication,
source preservation, cancellation and library indexing belong to that worker.
"""

import argparse
import copy
import json
from pathlib import Path
import sys


def report(stage, **kwargs):
    print(json.dumps({"stage": stage, **kwargs}), flush=True)


def main():
    parser = argparse.ArgumentParser(description="NVFP4 Safetensors conversion")
    parser.add_argument("--model-dir", required=True)
    parser.add_argument("--output-dir", required=True)
    parser.add_argument("--calibration-file")
    args = parser.parse_args()

    import torch
    from safetensors import safe_open
    from transformers import AutoConfig, AutoModelForCausalLM, AutoTokenizer
    import modelopt.torch.quantization as mtq
    from modelopt.torch.export import export_hf_checkpoint

    source = Path(args.model_dir).resolve(strict=True)
    output = Path(args.output_dir).resolve()
    if source == output or source in output.parents:
        raise ValueError("Output must be separate from the source model package")
    config = AutoConfig.from_pretrained(source, local_files_only=True, trust_remote_code=False)
    if getattr(config, "quantization_config", None):
        raise ValueError("NVFP4 conversion requires unquantized floating-point source weights")
    if not torch.cuda.is_available():
        raise ValueError("NVFP4 calibration requires an available CUDA GPU")

    if args.calibration_file:
        text = Path(args.calibration_file).read_text(encoding="utf-8")
        samples = [text[i:i + 512] for i in range(0, min(len(text), 65536), 512)]
        if not samples or not text.strip():
            raise ValueError("Calibration text must not be empty")
    else:
        # A bounded offline fallback. Task-specific text can be supplied through
        # the conversion API for better calibration of a particular workload.
        samples = [
            "The quick brown fox jumps over the lazy dog.",
            "Explain how a computer stores and processes information.",
            "A red ceramic teapot and a yellow lemon on a blue wooden table.",
            "A mountain landscape at sunrise, soft light and detailed clouds.",
            "Write a short story about a traveler who discovers a quiet village.",
            "Describe the shapes, colors, lighting and composition of a photograph.",
            "Mathematics studies numbers, patterns, structures and relationships.",
            "Compare the benefits and limitations of two approaches to a problem.",
        ]

    report("loading", message="Loading local floating-point model for NVFP4 calibration...")
    tokenizer = AutoTokenizer.from_pretrained(source, local_files_only=True, trust_remote_code=False)
    model = AutoModelForCausalLM.from_pretrained(
        source, config=config, torch_dtype=torch.bfloat16, device_map="auto",
        local_files_only=True, trust_remote_code=False, attn_implementation="eager",
    ).eval()

    def calibrate_loop(calibration_model):
        report("calibrating", message="Calibrating NVFP4 weights and activations...")
        with torch.inference_mode():
            for text in samples:
                inputs = tokenizer(text, return_tensors="pt", truncation=True, max_length=512)
                inputs = {key: value.to(calibration_model.device) for key, value in inputs.items()}
                calibration_model(**inputs, use_cache=False)

    report("quantizing", message="Applying NVIDIA NVFP4 quantization...")
    mtq.quantize(model, copy.deepcopy(mtq.NVFP4_DEFAULT_CFG), forward_loop=calibrate_loop)
    report("exporting", message="Packing NVFP4 weights into Safetensors...")
    output.mkdir(parents=True, exist_ok=True)
    export_hf_checkpoint(model, export_dir=str(output))
    tokenizer.save_pretrained(output)

    # A successful fake-quantization pass alone is not a converted checkpoint.
    # Require the actual packed byte weights and scaling tensors from the exporter.
    exported_config = json.loads((output / "config.json").read_text(encoding="utf-8"))
    if exported_config.get("quantization_config", {}).get("quant_algo") != "NVFP4":
        raise ValueError("Exporter did not declare an NVFP4 checkpoint")
    keys = set()
    packed_weights = []
    for shard in output.glob("*.safetensors"):
        with safe_open(shard, framework="pt", device="cpu") as tensors:
            keys.update(tensors.keys())
            packed_weights.extend(
                key for key in tensors.keys()
                if key.endswith(".weight") and tensors.get_slice(key).get_dtype() == "U8"
            )
    for key in packed_weights:
        prefix = key.removesuffix(".weight")
        if not {prefix + ".weight_scale", prefix + ".weight_scale_2"}.issubset(keys):
            raise ValueError(f"Packed NVFP4 weight has missing scales: {key}")
    packed = len(packed_weights)
    if packed == 0:
        raise ValueError("Exporter produced no packed NVFP4 weights")
    report("complete", message="NVFP4 Safetensors package written", packed_weights=packed)


if __name__ == "__main__":
    try:
        main()
    except Exception as error:
        report("error", message=str(error))
        sys.exit(1)
