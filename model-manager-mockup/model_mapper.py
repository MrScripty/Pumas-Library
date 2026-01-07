import os
import json
import logging
from pathlib import Path

logging.basicConfig(
    level=logging.INFO,
    filename=Path.home() / '.ai_models' / 'logs' / 'mapper.log',
    format='%(asctime)s - %(levelname)s - %(message)s'
)
logger = logging.getLogger(__name__)

CENTRAL_ROOT = Path(os.getenv('AI_MODELS_ROOT', str(Path.home() / 'AI_Models')))

APP_ROOTS = {
    "lm_studio": Path.home() / '.cache' / 'lm-studio' / 'models',
    "comfyui": Path.home() / 'ComfyUI' / 'models',
    "invokeai": Path.home() / 'InvokeAI' / 'models',
    "krita_diffusion": Path.home() / '.krita' / 'diffusion' / 'models',
    "open_webui_ollama": Path.home() / '.ollama' / 'models'
}

def load_metadata(model_dir: Path) -> dict:
    meta_path = model_dir / 'metadata.json'
    if meta_path.exists():
        with open(meta_path, 'r') as f:
            return json.load(f)
    return {}

def create_symlink(source: Path, target: Path):
    target.parent.mkdir(parents=True, exist_ok=True)
    if target.exists():
        if target.is_symlink():
            target.unlink()
            logger.info(f"Removed old symlink: {target}")
        else:
            logger.warning(f"Target exists and is not a symlink: {target}")
            return
    try:
        os.symlink(source, target)
        logger.info(f"Symlink created: {target} -> {source}")
    except OSError as e:
        logger.error(f"Failed to create symlink {target}: {e}")

def handle_ollama(app_root: Path, model_dir: Path):
    # Placeholder for Ollama manifest/blob handling
    logger.info(f"Special handling for Ollama model at {model_dir} (placeholder)")

def main():
    mapping_path = Path('mapping.json')
    if not mapping_path.exists():
        logger.error("mapping.json not found")
        return

    with open(mapping_path, 'r') as f:
        mappings = json.load(f)

    for app, app_configs in mappings.items():
        app_root = APP_ROOTS.get(app)
        if not app_root:
            logger.warning(f"App root not defined for {app}")
            continue

        if app_configs.get('special_handler') == "ollama":
            central_cat = app_configs['central_category']
            for model_dir in CENTRAL_ROOT.glob(f"{central_cat}/**"):
                if model_dir.is_dir():
                    handle_ollama(app_root, model_dir)
            continue

        for subdir, config in app_configs.items():
            central_cat = config['central_category']
            method = config.get('method', 'symlink')
            patterns = config.get('patterns', [])
            filters = config.get('filters', {})

            target_dir = app_root / subdir
            for model_dir in CENTRAL_ROOT.glob(f"{central_cat}/**"):
                if not model_dir.is_dir():
                    continue
                metadata = load_metadata(model_dir)

                # Apply filters
                if 'model_type' in filters and metadata.get('model_type') not in filters['model_type']:
                    continue
                if 'tags' in filters:
                    model_tags = set(metadata.get('tags', []))
                    if not model_tags.intersection(filters['tags']):
                        continue

                for pattern in patterns:
                    for source_file in model_dir.glob(pattern):
                        if source_file.is_file():
                            target_file = target_dir / source_file.name
                            if method == "symlink":
                                create_symlink(source_file, target_file)
                            elif method == "config":
                                logger.info(f"Config method: would add {source_file.parent} to app config")

if __name__ == '__main__':
    main()
