import json
import hashlib
import logging
import shutil
from pathlib import Path
from typing import Optional
from huggingface_hub import HfApi, login, hf_hub_download

logging.basicConfig(
    level=logging.INFO,
    filename=Path.home() / '.ai_models' / 'logs' / 'importer.log',
    format='%(asctime)s - %(levelname)s - %(message)s'
)
logger = logging.getLogger(__name__)

class ModelImporter:
    def __init__(self, central_root: Path = Path.home() / 'AI_Models'):
        self.central_root = central_root.resolve()
        self.hf_token = os.getenv('HF_TOKEN')
        if self.hf_token:
            login(self.hf_token)
        self.api = HfApi()

        self.type_patterns = {
            "checkpoints": ["*.ckpt", "*.safetensors", "*.gguf"],
            "loras": ["*.safetensors", "*.pt"],
            "vae": ["*.pt", "*.safetensors"],
            "controlnet": ["*.safetensors", "*.pt", "*.gguf"],
            "embeddings": ["*.pt"],
            "llm": ["*.gguf", "*.bin", "*.json", "*.pt"]
        }

    def _compute_sha256(self, path: Path) -> str:
        h = hashlib.sha256()
        with path.open('rb') as f:
            for chunk in iter(lambda: f.read(8192 * 1024), b''):
                h.update(chunk)
        return h.hexdigest().lower()

    def _compute_blake3(self, path: Path) -> str:
        try:
            import blake3
            h = blake3.blake3()
            with path.open('rb') as f:
                for chunk in iter(lambda: f.read(8192 * 1024), b''):
                    h.update(chunk)
            return h.hexdigest().lower()
        except ImportError:
            return ""

    def detect_type(self, file_path: Path) -> tuple[str, str]:
        ext = file_path.suffix.lower()
        for cat, patterns in self.type_patterns.items():
            if any(ext == pat.strip('*').lower() for pat in patterns):
                model_type = "diffusion" if cat not in ["llm"] else "llm"
                subtype = cat if model_type == "diffusion" else ""
                return model_type, subtype
        return "llm", ""

    def import_model(
        self,
        local_path: Path,
        family: str,
        model_id: str,
        repo_id: Optional[str] = None
    ):
        local_path = local_path.resolve()
        if not local_path.exists():
            raise FileNotFoundError(f"Local path not found: {local_path}")

        model_type, subtype = self.detect_type(local_path) if local_path.is_file() else ("llm", "")
        category_path = self.central_root / model_type / family / model_id
        category_path.mkdir(parents=True, exist_ok=True)

        if local_path.is_file():
            shutil.copy2(local_path, category_path / local_path.name)
        else:
            for item in local_path.iterdir():
                if item.is_file():
                    shutil.copy2(item, category_path / item.name)

        logger.info(f"Imported files from {local_path} to {category_path}")

        meta_path = category_path / 'metadata.json'
        metadata = {
            "family": family,
            "model_id": model_id,
            "model_type": model_type,
            "subtype": subtype,
            "preview_image": "",
            "tags": [],
            "base_model": "",
            "hashes": {"sha256": "", "blake3": ""}
        }

        # Compute hashes on largest model file
        candidates = sorted(category_path.rglob('*'), key=lambda p: p.stat().st_size if p.is_file() else 0, reverse=True)
        main_file = next((p for p in candidates if p.is_file() and p.suffix in {'.gguf', '.safetensors', '.ckpt', '.pt', '.bin'}), None)
        if main_file:
            metadata["hashes"]["sha256"] = self._compute_sha256(main_file)
            metadata["hashes"]["blake3"] = self._compute_blake3(main_file)

        if repo_id:
            try:
                info = self.api.model_info(repo_id)
                metadata.update({
                    "release_date": info.last_modified.isoformat() if info.last_modified else "",
                    "download_url": f"https://huggingface.co/{repo_id}",
                    "model_card": info.card_data.to_dict() if info.card_data else {},
                    "tags": info.tags or [],
                    "base_model": info.card_data.get('base_model', '') if info.card_data else ""
                })
                for sibling in info.siblings:
                    if sibling.rfilename.lower().endswith(('.png', '.jpg', '.jpeg')):
                        preview_path = category_path / 'preview.png'
                        hf_hub_download(repo_id=repo_id, filename=sibling.rfilename, local_dir=category_path)
                        preview_path.rename(category_path / 'preview.png')
                        metadata["preview_image"] = "preview.png"
                        break
                logger.info(f"Fetched metadata from Hugging Face repo {repo_id}")
            except Exception as e:
                logger.warning(f"Failed to fetch metadata from HF: {e}")

        with open(meta_path, 'w') as f:
            json.dump(metadata, f, indent=4)

if __name__ == '__main__':
    import argparse
    parser = argparse.ArgumentParser()
    parser.add_argument('--local_path', required=True)
    parser.add_argument('--family', required=True)
    parser.add_argument('--model_id', required=True)
    parser.add_argument('--repo_id', default=None)
    args = parser.parse_args()

    importer = ModelImporter()
    importer.import_model(Path(args.local_path), args.family, args.model_id, args.repo_id)
