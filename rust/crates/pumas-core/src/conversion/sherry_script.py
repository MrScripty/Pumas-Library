#!/usr/bin/env python3
"""Sherry QAT (Quantization-Aware Training) script using AngelSlim.

Produces 1.25-bit ternary quantized models via Tencent's AngelSlim framework.
This performs actual training, not post-training quantization.

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
    parser = argparse.ArgumentParser(description="Sherry QAT quantization")
    parser.add_argument("--model-dir", required=True, help="Path to source model directory")
    parser.add_argument("--output-dir", required=True, help="Path to output directory")
    parser.add_argument("--calibration-file", default=None, help="Path to calibration/training text")
    parser.add_argument("--epochs", type=int, default=3, help="Number of QAT epochs")
    args = parser.parse_args()

    report("setup", message="Loading model and AngelSlim...")

    try:
        import torch
        from transformers import AutoModelForCausalLM, AutoTokenizer
        from angelslim import TernaryQuantizer
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

    # Prepare calibration/training data
    if args.calibration_file and os.path.exists(args.calibration_file):
        with open(args.calibration_file, "r") as f:
            train_text = f.read()
    else:
        train_text = (
            "The quick brown fox jumps over the lazy dog. "
            "In machine learning, quantization reduces model precision. "
            "Large language models have transformed NLP research. "
        ) * 100

    report("training", message=f"Starting QAT for {args.epochs} epochs...")

    # Initialize ternary quantizer
    quantizer = TernaryQuantizer(model, tokenizer)

    # Run QAT
    for epoch in range(args.epochs):
        report(
            "training",
            message=f"Epoch {epoch + 1}/{args.epochs}",
            epoch=epoch + 1,
            epochs_total=args.epochs,
        )
        quantizer.train_epoch(train_text, max_length=512)

    # Export quantized model
    report("exporting", message="Exporting ternary model...")
    os.makedirs(args.output_dir, exist_ok=True)

    quantizer.save_pretrained(args.output_dir)
    tokenizer.save_pretrained(args.output_dir)

    report("complete", message="Sherry QAT complete")


if __name__ == "__main__":
    main()
