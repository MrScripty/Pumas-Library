import json
from pathlib import Path
import sys

if len(sys.argv) < 2:
    print("Usage: python generate_metadata.py /path/to/model_dir")
    sys.exit(1)

model_dir = Path(sys.argv[1]).resolve()

if not model_dir.is_dir():
    print(f"Error: {model_dir} is not a valid directory.")
    sys.exit(1)

meta_path = model_dir / 'metadata.json'

if meta_path.exists():
    print("metadata.json already exists.")
    sys.exit(0)

default_meta = {
    "family": "",
    "model_id": model_dir.name,
    "release_date": "",
    "download_url": "",
    "model_card": "",
    "model_type": "llm",  # "llm" or "diffusion"
    "subtype": "",       # e.g., "sdxl", "flux", "lora", "controlnet"
    "base_model": "",
    "tags": [],
    "preview_image": "",  # relative path, e.g., "preview.png"
    "inference_settings": {
        "k_sample": 40,
        "temperature": 1.0
    },
    "compatible_apps": [
        "lm_studio",
        "comfyui",
        "open_webui_ollama",
        "invokeai",
        "krita_diffusion"
    ],
    "hashes": {
        "sha256": "",
        "blake3": ""
    },
    "notes": ""
}

with open(meta_path, 'w') as f:
    json.dump(default_meta, f, indent=4)

print(f"Created blank metadata.json in {model_dir}")
