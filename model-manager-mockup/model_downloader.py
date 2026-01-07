import os
import hashlib
import json
import logging
from pathlib import Path
from typing import Optional, List
from pydantic import BaseModel, validator
from huggingface_hub import hf_hub_download, snapshot_download, login, HfApi
from tenacity import retry, stop_after_attempt, wait_exponential

logging.basicConfig(
    level=logging.INFO,
    filename=Path.home() / '.ai_models' / 'logs' / 'downloader.log',
    format='%(asctime)s - %(levelname)s - %(message)s'
)
logger = logging.getLogger(__name__)

class ModelManifest(BaseModel):
    id: str
    family: str
    tags: List[str] = []
    mirrors: List[str]
    main_file: Optional[str] = None  # filename to verify hash against
    hash_sha256: Optional[str] = None
    hash_blake3: Optional[str] = None
    size: Optional[int] = None
    compatible_apps: List[str] = []

    @validator('mirrors')
    def validate_mirrors(cls, v):
        if not v:
            raise ValueError("At least one mirror required")
        return v

class ModelDownloader:
    def __init__(
        self,
        central_root: Path = Path.home() / 'AI_Models',
        manifests_dir: Path = Path('manifests')
    ):
        self.central_root = central_root.resolve()
        self.manifests_dir = manifests_dir.resolve()
        self.hf_token = os.getenv('HF_TOKEN')
        if self.hf_token:
            login(self.hf_token)
        self.api = HfApi()

    def load_manifest(self, model_id: str) -> ModelManifest:
        manifest_path = self.manifests_dir / f"{model_id}.json"
        if not manifest_path.exists():
            raise FileNotFoundError(f"Manifest not found: {manifest_path}")
        with open(manifest_path, 'r') as f:
            data = json.load(f)
        return ModelManifest(**data)

    def _compute_sha256(self, file_path: Path) -> str:
        h = hashlib.sha256()
        with file_path.open('rb') as f:
            for chunk in iter(lambda: f.read(8192 * 1024), b''):
                h.update(chunk)
        return h.hexdigest().lower()

    def _compute_blake3(self, file_path: Path) -> str:
        try:
            import blake3  # optional dependency
            h = blake3.blake3()
            with file_path.open('rb') as f:
                for chunk in iter(lambda: f.read(8192 * 1024), b''):
                    h.update(chunk)
            return h.hexdigest().lower()
        except ImportError:
            logger.warning("blake3 not installed – skipping BLAKE3 hash")
            return ""

    def _verify_hashes(self, model_dir: Path, manifest: ModelManifest) -> bool:
        if not manifest.main_file and not (manifest.hash_sha256 or manifest.hash_blake3):
            return True  # no verification requested

        target_file = model_dir / manifest.main_file if manifest.main_file else None
        if not target_file:
            # fallback: find largest file
            candidates = sorted(model_dir.rglob('*'), key=lambda p: p.stat().st_size if p.is_file() else 0, reverse=True)
            target_file = next((p for p in candidates if p.is_file() and p.suffix in {'.gguf', '.safetensors', '.ckpt', '.pt', '.bin'}), None)

        if not target_file or not target_file.exists():
            logger.warning("Could not locate main model file for hash verification")
            return False

        verified = True
        if manifest.hash_sha256:
            computed = self._compute_sha256(target_file)
            if computed != manifest.hash_sha256.lower():
                logger.error(f"SHA256 mismatch for {target_file}: expected {manifest.hash_sha256}, got {computed}")
                verified = False
            else:
                logger.info("SHA256 hash verified")

        if manifest.hash_blake3:
            computed = self._compute_blake3(target_file)
            if computed and computed != manifest.hash_blake3.lower():
                logger.error(f"BLAKE3 mismatch for {target_file}")
                verified = False
            elif computed:
                logger.info("BLAKE3 hash verified")

        return verified

    @retry(stop=stop_after_attempt(5), wait=wait_exponential(multiplier=1, min=4, max=10))
    def download_model(self, manifest: ModelManifest) -> Path:
        model_type = "llm" if any(t in manifest.tags for t in ["llm", "text-generation"]) else "diffusion"
        category_path = self.central_root / model_type / manifest.family / manifest.id
        category_path.mkdir(parents=True, exist_ok=True)

        repo_id = manifest.mirrors[0]
        logger.info(f"Downloading {manifest.id} from {repo_id}")

        snapshot_download(
            repo_id=repo_id,
            local_dir=category_path,
            local_dir_use_symlinks=False,
            ignore_patterns=["*.md", "*.txt", "*.gitattributes"]
        )

        if not self._verify_hashes(category_path, manifest):
            raise ValueError(f"Hash verification failed for {manifest.id}")

        self.fetch_metadata(repo_id, category_path)
        logger.info(f"Successfully downloaded and verified {manifest.id}")
        return category_path

    def fetch_metadata(self, repo_id: str, model_dir: Path):
        meta_path = model_dir / 'metadata.json'
        try:
            info = self.api.model_info(repo_id)
            metadata = {
                "family": repo_id.split('/')[-1].split('-')[0].lower(),
                "model_id": model_dir.name,
                "release_date": info.last_modified.isoformat() if info.last_modified else "",
                "download_url": f"https://huggingface.co/{repo_id}",
                "model_card": info.card_data.to_dict() if info.card_data else {},
                "tags": info.tags or [],
                "model_type": "llm" if "text-generation" in (info.pipeline_tag or "") else "diffusion",
                "subtype": "",
                "base_model": info.card_data.get('base_model', '') if info.card_data else "",
                "preview_image": "",
                "hashes": {"sha256": "", "blake3": ""}
            }

            for sibling in info.siblings:
                if sibling.rfilename.lower().endswith(('.png', '.jpg', '.jpeg')):
                    preview_path = model_dir / 'preview.png'
                    hf_hub_download(repo_id=repo_id, filename=sibling.rfilename, local_dir=model_dir)
                    preview_path.rename(model_dir / 'preview.png')
                    metadata["preview_image"] = "preview.png"
                    break

            hf_hub_download(repo_id=repo_id, filename="README.md", local_dir=model_dir)

            with open(meta_path, 'w') as f:
                json.dump(metadata, f, indent=4)
            logger.info(f"Fetched and saved metadata for {repo_id}")
        except Exception as e:
            logger.warning(f"Failed to fetch metadata for {repo_id}: {e}. Using defaults.")
            # fallback to minimal metadata
            minimal = {
                "family": manifest.family if 'manifest' in locals() else "",
                "model_id": model_dir.name,
                "model_type": "llm",
                "tags": manifest.tags if 'manifest' in locals() else [],
                "hashes": {"sha256": "", "blake3": ""}
            }
            with open(meta_path, 'w') as f:
                json.dump(minimal, f, indent=4)

if __name__ == '__main__':
    import argparse
    parser = argparse.ArgumentParser()
    parser.add_argument('--model_id', required=True)
    args = parser.parse_args()

    downloader = ModelDownloader()
    manifest = downloader.load_manifest(args.model_id)
    downloader.download_model(manifest)
