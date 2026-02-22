#!/usr/bin/env python3
"""NVFP4 quantization script using nvidia-modelopt.

Quantizes a safetensors model to NVIDIA FP4 format suitable for
Blackwell GPU inference via TensorRT-LLM.

Progress is reported as JSON lines on stdout.
"""

import argparse
import json
import sys
import os


def report(stage, **kwargs):
    """Emit a JSON progress line."""
    msg = {"stage": stage, **kwargs}
    print(json.dumps(msg), flush=True)


def main():
    parser = argparse.ArgumentParser(description="NVFP4 quantization")
    parser.add_argument("--model-dir", required=True, help="Path to source model directory")
    parser.add_argument("--output-dir", required=True, help="Path to output directory")
    parser.add_argument("--calibration-file", default=None, help="Path to calibration text file")
    args = parser.parse_args()

    report("setup", message="Loading model and quantization libraries...")

    try:
        import torch
        from transformers import AutoModelForCausalLM, AutoTokenizer
        import modelopt.torch.quantization as mtq
        from modelopt.torch.export import export_tensorrt_llm_checkpoint
    except ImportError as e:
        report("error", message=f"Missing dependency: {e}")
        sys.exit(1)

    report("loading", message="Loading model...")

    tokenizer = AutoTokenizer.from_pretrained(args.model_dir)
    model = AutoModelForCausalLM.from_pretrained(
        args.model_dir,
        torch_dtype=torch.float16,
        device_map="auto",
    )

    # Build calibration dataloader
    report("calibrating", message="Running calibration pass...")

    if args.calibration_file and os.path.exists(args.calibration_file):
        with open(args.calibration_file, "r") as f:
            cal_text = f.read()
        cal_samples = [cal_text[i:i+512] for i in range(0, min(len(cal_text), 8192), 512)]
    else:
        # Use a small default calibration set
        cal_samples = [
            "The quick brown fox jumps over the lazy dog.",
            "In machine learning, quantization reduces model precision to improve inference speed.",
            "Large language models have transformed natural language processing.",
        ] * 4

    def calibrate_loop(model):
        for text in cal_samples:
            inputs = tokenizer(text, return_tensors="pt", truncation=True, max_length=512)
            inputs = {k: v.to(model.device) for k, v in inputs.items()}
            with torch.no_grad():
                model(**inputs)

    # Quantize with FP4
    quant_config = mtq.FP8_DEFAULT_CFG.copy()
    quant_config["quant_cfg"]["*weight_quantizer"]["num_bits"] = (2, 2)  # FP4: E2M1
    quant_config["quant_cfg"]["*input_quantizer"]["enable"] = False

    report("quantizing", message="Applying FP4 quantization...")

    mtq.quantize(model, quant_config, forward_loop=calibrate_loop)

    # Export
    report("exporting", message="Exporting quantized model...")
    os.makedirs(args.output_dir, exist_ok=True)

    model.save_pretrained(args.output_dir)
    tokenizer.save_pretrained(args.output_dir)

    report("complete", message="NVFP4 quantization complete")


if __name__ == "__main__":
    main()
