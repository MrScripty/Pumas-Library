#!/usr/bin/env python3
"""Measure a real FLUX.2 Klein load and image generation in the isolated Torch v2.14 runtime.

Run from the repository root, for example:

    systemd-run --user --scope -p MemoryMax=32G -p MemorySwapMax=0 \
      -- scripts/acceptance/flux2_memory_probe.py --output /tmp/flux2-memory.png

The script re-execs itself with the qualified v2.14 virtualenv's Python. It
prints one JSON record per phase and a final PNG SHA-256. The systemd scope is
the hard memory bound; this process also has a 15-minute wall-clock timeout.
"""

import argparse
import hashlib
import json
import os
from pathlib import Path
import signal
import sys
import threading
import time
import traceback


ROOT = Path(__file__).resolve().parents[2]
RUNTIME = ROOT / "launcher-data/cache/torch-qualification/v214-flux2-e2e-root/torch-versions/v2.14.0"
PYTHON = RUNTIME / "venv/bin/python"
DEFAULT_CHECKPOINT = ROOT / "shared-resources/models/diffusion/black-forest-labs/flux_2-klein-9b-kv-fp8/flux-2-klein-9b-kv-fp8.safetensors"
DEFAULT_ENCODER = ROOT / "shared-resources/models/llm/qwen3/qwen--qwen3-8b__files_97c4978527af-fp8"
DEFAULT_VAE = ROOT / "shared-resources/models/diffusion/flux2/comfy-org--flux2-dev__files_7432f8b613b4/split_files/vae/flux2-vae.safetensors"


def emit(phase, **fields):
    print(json.dumps({"time": time.time(), "phase": phase, **fields}, sort_keys=True), flush=True)


def number(path):
    try:
        value = Path(path).read_text().strip()
        return None if value == "max" else int(value)
    except (OSError, ValueError):
        return None


def memory_events(path):
    try:
        return {key: int(value) for key, value in (line.split() for line in Path(path).read_text().splitlines())}
    except (OSError, ValueError):
        return None


def cgroup_memory():
    try:
        line = next(line for line in Path("/proc/self/cgroup").read_text().splitlines() if line.startswith("0::"))
        group = line.split("::", 1)[1].lstrip("/")
        base = Path("/sys/fs/cgroup") / group
        return base
    except (OSError, StopIteration):
        return None


def rss_bytes():
    try:
        for line in Path("/proc/self/status").read_text().splitlines():
            if line.startswith("VmRSS:"):
                return int(line.split()[1]) * 1024
    except (OSError, ValueError, IndexError):
        pass
    return None


def mem_available_bytes():
    try:
        for line in Path("/proc/meminfo").read_text().splitlines():
            if line.startswith("MemAvailable:"):
                return int(line.split()[1]) * 1024
    except (OSError, ValueError, IndexError):
        pass
    return None


class MemorySampler:
    def __init__(self):
        self.group = cgroup_memory()
        self.stop = threading.Event()
        self.peak_rss = 0
        self.peak_cgroup = 0
        self.thread = threading.Thread(target=self._sample, daemon=True)

    def _sample(self):
        while not self.stop.is_set():
            self.peak_rss = max(self.peak_rss, rss_bytes() or 0)
            if self.group:
                self.peak_cgroup = max(self.peak_cgroup, number(self.group / "memory.current") or 0)
            self.stop.wait(0.2)

    def start(self):
        self.thread.start()

    def finish(self):
        self.stop.set()
        self.thread.join(timeout=1)

    def snapshot(self, torch=None):
        data = {
            "rss_bytes": rss_bytes(),
            "sampled_peak_rss_bytes": self.peak_rss,
            "host_mem_available_bytes": mem_available_bytes(),
        }
        if self.group:
            data.update(
                cgroup_current_bytes=number(self.group / "memory.current"),
                cgroup_peak_bytes=number(self.group / "memory.peak"),
                sampled_peak_cgroup_bytes=self.peak_cgroup,
                cgroup_limit_bytes=number(self.group / "memory.max"),
                cgroup_swap_limit_bytes=number(self.group / "memory.swap.max"),
                cgroup_swap_current_bytes=number(self.group / "memory.swap.current"),
                cgroup_events=memory_events(self.group / "memory.events"),
            )
        if torch is not None and torch.cuda.is_available():
            free, total = torch.cuda.mem_get_info()
            data.update(
                cuda_free_bytes=free,
                cuda_total_bytes=total,
                cuda_allocated_bytes=torch.cuda.memory_allocated(),
                cuda_reserved_bytes=torch.cuda.memory_reserved(),
                cuda_peak_allocated_bytes=torch.cuda.max_memory_allocated(),
                cuda_peak_reserved_bytes=torch.cuda.max_memory_reserved(),
            )
        return data


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--checkpoint", type=Path, default=DEFAULT_CHECKPOINT)
    parser.add_argument("--encoder", type=Path, default=DEFAULT_ENCODER)
    parser.add_argument("--vae", type=Path, default=DEFAULT_VAE)
    parser.add_argument("--output", type=Path, default=Path("/tmp/flux2-memory.png"))
    parser.add_argument("--prompt", default="A red ceramic teapot beside a yellow lemon on a blue table")
    parser.add_argument("--width", type=int, default=512)
    parser.add_argument("--height", type=int, default=512)
    parser.add_argument("--seed", type=int, default=42)
    parser.add_argument("--timeout-seconds", type=int, default=900)
    parser.add_argument("--max-memory-gib", type=int, default=32, help="largest permitted cgroup memory.max in GiB")
    args = parser.parse_args()

    if Path(sys.prefix).resolve() != (RUNTIME / "venv").resolve():
        if not PYTHON.is_file():
            parser.error(f"isolated Torch interpreter missing: {PYTHON}")
        os.execv(str(PYTHON), [str(PYTHON), str(Path(__file__).resolve()), *sys.argv[1:]])

    if args.width < 64 or args.height < 64 or args.width > 2048 or args.height > 2048 or args.width % 16 or args.height % 16:
        parser.error("width and height must be multiples of 16 from 64 through 2048")
    if not 1 <= args.timeout_seconds <= 3600:
        parser.error("timeout must be from 1 to 3600 seconds")
    if not 1 <= args.max_memory_gib <= 64:
        parser.error("max-memory-gib must be from 1 to 64")
    for label, path in (("checkpoint", args.checkpoint), ("vae", args.vae)):
        if not path.is_file():
            parser.error(f"{label} missing: {path}")
    for name in ("config.json", "model.safetensors.index.json", "tokenizer.json"):
        if not (args.encoder / name).is_file():
            parser.error(f"encoder component missing: {args.encoder / name}")
    config = json.loads((args.encoder / "config.json").read_text())
    if config.get("model_type") != "qwen3" or config.get("hidden_size") != 4096:
        parser.error("encoder must be the validated Qwen3-8B component")
    if config.get("quantization_config", {}).get("quant_method") != "fp8":
        parser.error("encoder must be the Pumas FP8 Qwen3-8B component")

    group = cgroup_memory()
    limit = number(group / "memory.max") if group else None
    swap_limit = number(group / "memory.swap.max") if group else None
    if limit is None or limit > args.max_memory_gib * 1024**3 or swap_limit != 0:
        parser.error(
            f"run in a bounded cgroup with memory.max <= {args.max_memory_gib} GiB "
            f"and memory.swap.max = 0 (observed memory.max={limit}, swap.max={swap_limit})"
        )
    for name in ("flux2.py", "diffusion.py"):
        if not (RUNTIME / name).is_file():
            parser.error(f"isolated Torch adapter missing: {RUNTIME / name}")

    os.environ.update(HF_HUB_OFFLINE="1", TRANSFORMERS_OFFLINE="1", DIFFUSERS_OFFLINE="1")
    sys.path.insert(0, str(RUNTIME))
    sampler = MemorySampler()
    sampler.start()
    started = time.monotonic()
    torch = None

    def timeout(_signum, _frame):
        raise TimeoutError(f"probe exceeded {args.timeout_seconds} seconds")

    signal.signal(signal.SIGALRM, timeout)
    signal.alarm(args.timeout_seconds)
    try:
        emit("start", runtime=str(RUNTIME), checkpoint=str(args.checkpoint), encoder=str(args.encoder), encoder_quantization_config=config["quantization_config"], vae=str(args.vae), output=str(args.output), memory=sampler.snapshot())
        import torch as torch_module
        torch = torch_module
        from flux2 import Flux2Klein

        if not torch.cuda.is_available() or torch.cuda.get_device_capability(0) != (12, 0):
            raise RuntimeError("qualified sm_120 CUDA device 0 is unavailable")
        emit("imports_complete", torch_version=torch.__version__, memory=sampler.snapshot(torch))
        load_start = time.monotonic()
        emit("load_start", memory=sampler.snapshot(torch))
        adapter = Flux2Klein(str(args.checkpoint), str(args.encoder), str(args.vae), torch.device("cuda:0"))
        emit("load_complete", elapsed_seconds=round(time.monotonic() - load_start, 3), memory=sampler.snapshot(torch))

        generation_start = time.monotonic()
        emit("generate_start", width=args.width, height=args.height, steps=adapter.steps, seed=args.seed, memory=sampler.snapshot(torch))
        image = adapter.generate(args.prompt, args.width, args.height, args.seed, threading.Event())
        emit("generate_complete", elapsed_seconds=round(time.monotonic() - generation_start, 3), memory=sampler.snapshot(torch))
        args.output.parent.mkdir(parents=True, exist_ok=True)
        image.save(args.output, format="PNG")
        digest = hashlib.sha256(args.output.read_bytes()).hexdigest()
        emit("complete", elapsed_seconds=round(time.monotonic() - started, 3), png=str(args.output), png_sha256=digest, png_bytes=args.output.stat().st_size, memory=sampler.snapshot(torch))
    except BaseException as error:
        emit("failed", error_type=type(error).__name__, error=str(error), elapsed_seconds=round(time.monotonic() - started, 3), memory=sampler.snapshot(torch))
        traceback.print_exc()
        raise SystemExit(1) from error
    finally:
        signal.alarm(0)
        sampler.finish()


if __name__ == "__main__":
    main()
