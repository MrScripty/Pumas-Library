"""CPU conversion of local Transformers packages to block-scaled FP8 Safetensors."""

import argparse
import json
import shutil
from pathlib import Path


def report(stage, **values):
    print(json.dumps({"stage": stage, **values}), flush=True)


def convert(source, output):
    import torch
    from safetensors import safe_open
    from safetensors.torch import save_file
    from transformers import AutoConfig, AutoModelForCausalLM, FineGrainedFP8Config

    config = AutoConfig.from_pretrained(source, local_files_only=True, trust_remote_code=False)
    if getattr(config, "quantization_config", None):
        raise ValueError("Source is already quantized; select its original floating-point package")
    with torch.device("meta"):
        model = AutoModelForCausalLM.from_config(config, trust_remote_code=False)
    output_layer = model.get_output_embeddings()
    layers = {
        f"{name}.weight"
        for name, module in model.named_modules()
        if isinstance(module, torch.nn.Linear) and module is not output_layer
    }
    expected = set(model.state_dict())
    del model
    if not layers:
        raise ValueError("Model has no supported linear layers")
    index_path = source / "model.safetensors.index.json"
    if index_path.is_file():
        index = json.loads(index_path.read_text())
        files = sorted(set(index["weight_map"].values()))
    else:
        files = ["model.safetensors"]
    if any(Path(name).name != name for name in files):
        raise ValueError("Checkpoint shards must be inside the package")
    output.mkdir(parents=True, exist_ok=True)
    weight_map, seen, converted = {}, set(), set()
    total_bytes = 0
    for number, name in enumerate(files):
        report("quantizing", tensor_index=number, tensor_count=len(files), tensor_name=name)
        state = {}
        with safe_open(source / name, framework="pt", device="cpu") as shard:
            for key in shard.keys():
                if key in seen or key not in expected:
                    raise ValueError(f"Unexpected or repeated weight: {key}")
                seen.add(key)
                value = shard.get_tensor(key)
                if value.dtype not in (torch.float16, torch.bfloat16, torch.float32):
                    raise ValueError(f"Unsupported source dtype: {key}: {value.dtype}")
                if not torch.isfinite(value).all():
                    raise ValueError(f"Non-finite source weight: {key}")
                if key in layers:
                    rows, columns = value.shape
                    if rows % 128 or columns % 128:
                        raise ValueError(f"FP8 requires linear dimensions divisible by 128: {key}")
                    blocks = value.float().reshape(rows // 128, 128, columns // 128, 128)
                    scales = blocks.abs().amax(dim=(1, 3)).clamp_min(1e-12) / 448.0
                    quantized = (blocks / scales[:, None, :, None]).clamp(-448, 448)
                    state[key] = quantized.to(torch.float8_e4m3fn).reshape(rows, columns)
                    state[key.removesuffix("weight") + "weight_scale_inv"] = scales.contiguous()
                    converted.add(key)
                else:
                    state[key] = value.to(torch.bfloat16)
        total_bytes += sum(value.numel() * value.element_size() for value in state.values())
        weight_map.update({key: name for key in state})
        save_file(state, output / name, metadata={"format": "pt"})
        del state
    # Tied embeddings can omit the output layer, but every quantized layer must exist.
    if converted != layers:
        raise ValueError("Source is missing linear weights")
    missing = expected - seen
    if missing and not (config.tie_word_embeddings and missing == {"lm_head.weight"}):
        raise ValueError(f"Source is incomplete: {sorted(missing)}")
    for path in source.iterdir():
        if path.is_file() and path.suffix in {".json", ".txt", ".model", ".jinja"}:
            if path.name not in {
                "config.json",
                "metadata.json",
                "conversion.json",
                "model.safetensors.index.json",
            }:
                shutil.copy2(path, output / path.name)
    config.quantization_config = FineGrainedFP8Config().to_dict()
    config.torch_dtype = torch.bfloat16
    config.save_pretrained(output)
    if len(files) > 1:
        (output / "model.safetensors.index.json").write_text(
            json.dumps(
                {"metadata": {"total_size": total_bytes}, "weight_map": weight_map}, indent=2
            )
        )
    report("complete", output_size=total_bytes, message="FP8 Safetensors package written")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--model-dir", type=Path, required=True)
    parser.add_argument("--output-dir", type=Path, required=True)
    args = parser.parse_args()
    try:
        convert(args.model_dir, args.output_dir)
    except Exception as error:
        report("error", message=str(error))
        raise
