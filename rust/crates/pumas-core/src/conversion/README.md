# Conversion Module

Model format conversion between GGUF and Safetensors.

## Contents

| File | Purpose |
|------|---------|
| `mod.rs` | Module root, public re-exports |
| `manager.rs` | `ConversionManager` — orchestration, venv setup, subprocess lifecycle |
| `progress.rs` | `ConversionProgressTracker` — thread-safe progress state |
| `scripts.rs` | Embedded Python scripts and disk deployment utilities |
| `types.rs` | Shared types: `ConversionRequest`, `ConversionProgress`, `ConversionSource`, etc. |

## Design Decisions

- **Python subprocess**: Conversion uses the `gguf` and `safetensors` Python packages
  rather than native Rust parsing, because GGUF has 20+ quantization formats and the
  Python ecosystem has battle-tested implementations.
- **Embedded scripts**: Python scripts are compiled into the binary as string constants
  and deployed to disk on first use, with hash-based staleness detection for updates.
- **Dedicated venv**: A separate virtual environment (`converter-venv/`) isolates
  conversion dependencies from ComfyUI or other Python environments.
- **Progress via stdout JSON**: The Python scripts emit one JSON object per line on
  stdout, which the Rust side parses to update the `ConversionProgressTracker`.

## Dependencies

**Internal:** `model_library` (ModelLibrary, ModelImporter), `cancel` (CancellationToken), `error` (PumasError)

**External (Python):** `gguf`, `safetensors`, `numpy`, `sentencepiece`
