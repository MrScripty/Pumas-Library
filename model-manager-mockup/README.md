# Centralized Local AI Model Management System

## Overview

This project defines a **lightweight, pure-Python backend system** for managing a large collection of local AI models — including LLMs (e.g., Llama, Qwen) and diffusion models (checkpoints, LoRAs, VAEs, ControlNets, etc.) — across multiple desktop applications.

It solves the common problems of **duplication**, **manual configuration**, and **disorganization** by maintaining a single centralized repository of models and automatically sharing them with supported applications via symbolic links or configuration injection.

Designed specifically for integration into a larger desktop application, this proposal focuses exclusively on **backend logic**. UI, notifications, progress displays, and drag-and-drop handling are out of scope and assumed to be provided by the parent application.

The design draws inspiration from Stability Matrix (particularly its shared-folder and metadata-driven approach) while extending support to LLM-focused tools and maintaining a clean, scriptable, configuration-driven architecture.

## Goals

- **Eliminate Duplication**: Store each model once, preventing gigabytes of wasted disk space from copies across apps.
- **Simplify Management**: Add, import, or organize models in one location; propagate changes to all apps with a single operation.
- **Preserve Organization**: Logical hierarchical structure with rich per-model metadata for tracking provenance, settings, and compatibility.
- **Seamless Integration**: Applications see models in their expected locations without manual reconfiguration.
- **Robustness**: Hash validation, logging, retries, idempotent operations, and graceful error handling.
- **Extensibility**: Add new applications or model types via JSON configuration without code changes.

## Why This Design?

Local AI workflows typically involve multiple specialized tools (e.g., LM Studio or Ollama for LLMs, ComfyUI or InvokeAI for diffusion), each with strict expectations about model file locations.

Traditional approaches suffer from:

- **Wasteful Duplication** of large (5–50 GB) model files.
- **Tedious Maintenance** via manual symlinks or config edits that break after updates.
- **Disorganization** as related files (checkpoint + VAE, model + tokenizer) become scattered.

This system addresses these issues with:

- **Centralized Storage**: One root directory with intuitive hierarchy; related files stay together in dedicated model folders.
- **Declarative Mapping** (`mapping.json`): Rules per app with patterns, filters, and sharing method ("symlink" or "config").
- **Zero-Copy Sharing**: Symlinks give apps direct access; config method prepares for future path injection.
- **Rich Metadata-Driven Design**: Per-model `metadata.json` (with tags, hashes, preview images, base model, inference settings) enables filtering and future advanced features.
- **Hugging Face Integration**: Automated download, hash verification, metadata enrichment, and thumbnail fetching.
- **Pure Python**: Cross-platform, minimal dependencies, type hints, logging, and SOLID principles for long-term maintainability.
- **Safety & Idempotency**: Scripts clean up broken links and can be re-run safely.

## Key Features in Current Proposal

- Backend-only modular components.
- Enhanced metadata schema including `hashes` (SHA256 + optional BLAKE3), `preview_image`, `model_id`, and richer fields.
- Cryptographic hash validation during download and import.
- Robust type detection and hierarchical organization.
- Progress/resume-capable downloads via `huggingface_hub` (internally uses tqdm – ready for UI consumption when needed).
- Foundation prepared for future httpx-based downloader for non-HF sources.
- Planned SQLite + JSON1 central index for fast, powerful querying (future extension).

## How It Works

1. **Set Up Central Collection**
   Place models under `~/AI_Models/` (configurable via `AI_MODELS_ROOT`). Organize by type/family/model.

2. **Add Metadata**
   Run `generate_metadata.py` to create a richly-structured `metadata.json` in any model directory.

3. **Download Models** (Optional)
   Use `model_downloader.py` to fetch from Hugging Face with resume support, hash verification, metadata enrichment, and thumbnail download.

4. **Import Local Models**
   Use `model_importer.py` to copy existing files/directories into the central structure, detect type, compute hashes, and optionally enrich from HF.

5. **Configure Sharing**
   Edit `mapping.json` to define per-app rules (patterns, filters, method, special handlers).

6. **Run Mapper**
   Execute `model_mapper.py` to walk the collection, apply filters, and create/update symlinks (or placeholder config entries).

7. **Maintenance**
   Re-run the mapper after changes. Future automation via file watcher possible.

The parent application can integrate the importer/downloader directly (e.g., for drag-and-drop) and consume logs/progress as needed.

## File Structure

~/AI_Tools/ # Project/scripts location
├── generate_metadata.py
├── model_downloader.py
├── model_importer.py
├── model_mapper.py
├── mapping.json
├── manifests/ # Download manifests
└── README.md

~/AI_Models/ # Central model root (configurable)
├── llm/
│ └── qwen/
│ └── qwen2-8b/
│ ├── model.gguf
│ ├── tokenizer.json
│ ├── preview.png
│ └── metadata.json
├── diffusion/
│ └── checkpoints/
│ └── juggernaut-xl/
│ ├── juggernaut-xl.safetensors
│ ├── preview.png
│ └── metadata.json
└── models.db # Future: SQLite index



Logs are stored in `~/.ai_models/logs/` for debugging.

## Installation and Usage

**Dependencies**

```bash
pip install huggingface_hub pydantic tenacity blake3  # blake3 optional but recommended for faster hashing


**Environment Variables**

- HF_TOKEN – Hugging Face authentication token (for private/restricted repos).
- AI_MODELS_ROOT – Override default central models directory.

**Commands**


# Generate blank metadata

python generate_metadata.py ~/AI_Models/llm/qwen/qwen2-8b

# Download a model (requires corresponding manifest)

python model_downloader.py --model_id qwen2-8b

# Import local model

python model_importer.py --local_path ./my_model.gguf --family qwen --model_id qwen2-8b --repo_id Qwen/Qwen2-8B

# Apply mappings to all apps

python model_mapper.py


**Customization**

Add new applications by extending mapping.json and updating APP_ROOTS in model_mapper.py.

## Limitations and Future Ideas

- Symbolic links work best on Unix-like systems; Windows requires Developer Mode or admin privileges for NTFS symlinks.
- "config" method currently logs placeholders – ready for real YAML/INI writing in production.
- No built-in GUI or terminal progress bars (backend-only focus).
- Hash verification ensures file integrity; future lightweight validation (e.g., quick header check) possible without full inference.
- Future extensions:
  - Central SQLite database with JSON1 for advanced metadata queries.
  - File watcher automation.
  - CivitAI and OpenModelDB metadata support.
  - httpx-based downloader for additional sources.
  - Model update checking.
  - CLI query interface.

This proposal provides a robust, maintainable, and extensible backend foundation for centralized AI model management, ready for seamless integration into a larger desktop application.
