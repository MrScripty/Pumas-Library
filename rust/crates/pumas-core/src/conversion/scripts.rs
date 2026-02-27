//! Embedded Python conversion scripts and deployment utilities.
//!
//! Scripts are stored as string constants and written to disk on first use
//! or when the embedded version changes (detected via hash comparison).

use crate::error::IoResultExt;
use crate::Result;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use tracing::info;

/// Python requirements for the conversion virtual environment.
pub const REQUIREMENTS: &str = "\
gguf>=0.10.0
safetensors>=0.4.0
numpy>=1.24.0
sentencepiece>=0.2.0
";

/// Python script for GGUF to Safetensors conversion.
pub const GGUF_TO_SAFETENSORS_SCRIPT: &str = r#"#!/usr/bin/env python3
"""Convert a GGUF model file to Safetensors format.

Reads GGUF tensors, dequantizes to float16/float32, and writes safetensors output.
Reports per-tensor progress as JSON lines on stdout.
"""
import argparse
import json
import os
import sys
import numpy as np

def progress(stage, **kwargs):
    """Emit a JSON progress line to stdout."""
    print(json.dumps({"stage": stage, **kwargs}), flush=True)

def main():
    parser = argparse.ArgumentParser(description="Convert GGUF to Safetensors")
    parser.add_argument("--input", required=True, nargs="+", help="Input GGUF file path(s)")
    parser.add_argument("--output-dir", required=True, help="Output directory")
    args = parser.parse_args()

    try:
        from gguf import GGUFReader
        from safetensors.numpy import save_file
    except ImportError as e:
        progress("error", message=f"Missing required package: {e}")
        sys.exit(1)

    progress("validating", message="Reading GGUF header...")

    input_path = args.input[0]
    reader = GGUFReader(input_path)

    os.makedirs(args.output_dir, exist_ok=True)

    # Extract metadata from GGUF for config.json generation
    metadata = {}
    for field in reader.fields.values():
        if hasattr(field, 'parts'):
            # String field
            try:
                name = field.name
                if len(field.parts) > 0:
                    val = field.parts[-1].tolist()
                    if isinstance(val, list) and len(val) > 0:
                        val = bytes(val).decode("utf-8", errors="replace")
                    metadata[name] = val
            except Exception:
                pass

    tensor_count = len(reader.tensors)
    progress("validating", message=f"Found {tensor_count} tensors")

    # Convert tensors
    tensors = {}
    bytes_written = 0

    for i, tensor in enumerate(reader.tensors):
        name = tensor.name
        progress("converting",
                 tensor_index=i,
                 tensor_count=tensor_count,
                 tensor_name=name,
                 bytes_written=bytes_written)

        # Get tensor data - GGUFReader handles dequantization
        data = tensor.data
        if hasattr(data, 'copy'):
            data = data.copy()

        # Convert to float16 for storage efficiency (unless already small dtype)
        if data.dtype in (np.float32, np.float64):
            data = data.astype(np.float16)

        tensors[name] = data
        bytes_written += data.nbytes

    # Write safetensors file
    progress("writing", message="Writing safetensors file...")
    output_path = os.path.join(args.output_dir, "model.safetensors")
    save_file(tensors, output_path)

    output_size = os.path.getsize(output_path)

    # Write config.json from GGUF metadata
    arch = metadata.get("general.architecture", "unknown")
    config = {
        "architectures": [arch],
        "model_type": arch,
    }
    # Map common GGUF metadata to config fields
    field_mapping = {
        "general.name": "model_name",
        f"{arch}.embedding_length": "hidden_size",
        f"{arch}.block_count": "num_hidden_layers",
        f"{arch}.attention.head_count": "num_attention_heads",
        f"{arch}.attention.head_count_kv": "num_key_value_heads",
        f"{arch}.feed_forward_length": "intermediate_size",
        f"{arch}.context_length": "max_position_embeddings",
        f"{arch}.rope.freq_base": "rope_theta",
        f"{arch}.attention.layer_norm_rms_epsilon": "rms_norm_eps",
    }
    for gguf_key, config_key in field_mapping.items():
        if gguf_key in metadata:
            config[config_key] = metadata[gguf_key]

    config_path = os.path.join(args.output_dir, "config.json")
    with open(config_path, "w") as f:
        json.dump(config, f, indent=2, default=str)

    progress("complete", output_path=output_path, output_size=output_size)

if __name__ == "__main__":
    main()
"#;

/// Python script for Safetensors to GGUF conversion.
pub const SAFETENSORS_TO_GGUF_SCRIPT: &str = r#"#!/usr/bin/env python3
"""Convert Safetensors model file(s) to GGUF format.

Reads safetensors tensors and writes GGUF output with architecture metadata.
Reports per-tensor progress as JSON lines on stdout.
"""
import argparse
import json
import os
import sys
import numpy as np

def progress(stage, **kwargs):
    """Emit a JSON progress line to stdout."""
    print(json.dumps({"stage": stage, **kwargs}), flush=True)

def main():
    parser = argparse.ArgumentParser(description="Convert Safetensors to GGUF")
    parser.add_argument("--input", required=True, nargs="+", help="Input safetensors file path(s)")
    parser.add_argument("--output", required=True, help="Output GGUF file path")
    parser.add_argument("--config", default=None, help="Path to config.json")
    parser.add_argument("--quant", default="F16", help="Quantization type (default: F16)")
    args = parser.parse_args()

    try:
        from safetensors import safe_open
        from gguf import GGUFWriter
    except ImportError as e:
        progress("error", message=f"Missing required package: {e}")
        sys.exit(1)

    progress("validating", message="Reading safetensors header...")

    # Load config if available
    config = {}
    if args.config and os.path.exists(args.config):
        with open(args.config) as f:
            config = json.load(f)
    else:
        # Try to find config.json next to the input file
        input_dir = os.path.dirname(args.input[0])
        config_path = os.path.join(input_dir, "config.json")
        if os.path.exists(config_path):
            with open(config_path) as f:
                config = json.load(f)

    if not config:
        progress("error", message="No config.json found. Cannot determine model architecture for GGUF conversion.")
        sys.exit(1)

    # Determine architecture
    arch = "llama"  # Default fallback
    if "architectures" in config:
        arch_name = config["architectures"][0].lower()
        # Map HuggingFace architecture names to GGUF architecture names
        arch_map = {
            "llamaforcausallm": "llama",
            "mistralformcausallm": "mistral",
            "mistralforcausallm": "mistral",
            "gemmaforcausallm": "gemma",
            "gemma2forcausallm": "gemma2",
            "phiforcausallm": "phi",
            "phi3forcausallm": "phi3",
            "qwen2forcausallm": "qwen2",
            "falconforcausallm": "falcon",
            "deepseekv2forcausallm": "deepseek2",
            "commandrforcausallm": "command-r",
        }
        arch = arch_map.get(arch_name, arch_name.replace("forcausallm", ""))
    elif "model_type" in config:
        arch = config["model_type"]

    progress("validating", message=f"Architecture: {arch}")

    # Collect all tensors from all input files
    all_tensor_names = []
    for path in args.input:
        with safe_open(path, framework="numpy") as f:
            all_tensor_names.extend(f.keys())

    tensor_count = len(all_tensor_names)
    progress("validating", message=f"Found {tensor_count} tensors")

    # Create GGUF writer
    os.makedirs(os.path.dirname(os.path.abspath(args.output)), exist_ok=True)
    writer = GGUFWriter(args.output, arch)

    # Write metadata from config
    if "model_name" in config:
        writer.add_name(config["model_name"])
    if "max_position_embeddings" in config:
        writer.add_context_length(config["max_position_embeddings"])
    if "hidden_size" in config:
        writer.add_embedding_length(config["hidden_size"])
    if "num_hidden_layers" in config:
        writer.add_block_count(config["num_hidden_layers"])
    if "num_attention_heads" in config:
        writer.add_head_count(config["num_attention_heads"])
    if "num_key_value_heads" in config:
        writer.add_head_count_kv(config["num_key_value_heads"])
    if "intermediate_size" in config:
        writer.add_feed_forward_length(config["intermediate_size"])

    # Write tensors
    bytes_written = 0
    tensor_idx = 0

    for path in args.input:
        with safe_open(path, framework="numpy") as f:
            for name in f.keys():
                progress("converting",
                         tensor_index=tensor_idx,
                         tensor_count=tensor_count,
                         tensor_name=name,
                         bytes_written=bytes_written)

                data = f.get_tensor(name)

                # Convert to float16 for F16 quantization
                if args.quant == "F16" and data.dtype == np.float32:
                    data = data.astype(np.float16)

                writer.add_tensor(name, data)
                bytes_written += data.nbytes
                tensor_idx += 1

    progress("writing", message="Finalizing GGUF file...")
    writer.write_header_to_file()
    writer.write_kv_data_to_file()
    writer.write_tensors_to_file()
    writer.close()

    output_size = os.path.getsize(args.output)
    progress("complete", output_path=args.output, output_size=output_size)

if __name__ == "__main__":
    main()
"#;

/// Compute a short hash of a string for staleness checking.
fn content_hash(content: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content.as_bytes());
    let hash = hasher.finalize();
    hex::encode(&hash[..8])
}

/// Get the path to the converter scripts directory.
pub fn scripts_dir(launcher_root: &Path) -> PathBuf {
    launcher_root
        .join("launcher-data")
        .join("converter-scripts")
}

/// Get the path to the converter virtual environment.
pub fn venv_dir(launcher_root: &Path) -> PathBuf {
    launcher_root.join("launcher-data").join("converter-venv")
}

/// Get the path to the Python binary inside the converter venv.
pub fn venv_python(launcher_root: &Path) -> PathBuf {
    venv_dir(launcher_root).join("bin").join("python")
}

/// Deploy embedded scripts to disk if missing or outdated.
///
/// Uses a `.hash` sidecar file to detect when the embedded script has changed
/// and needs to be rewritten.
pub fn ensure_scripts_deployed(launcher_root: &Path) -> Result<()> {
    let dir = scripts_dir(launcher_root);
    std::fs::create_dir_all(&dir).with_path(&dir)?;

    deploy_script(
        &dir,
        "convert_gguf_to_safetensors.py",
        GGUF_TO_SAFETENSORS_SCRIPT,
    )?;
    deploy_script(
        &dir,
        "convert_safetensors_to_gguf.py",
        SAFETENSORS_TO_GGUF_SCRIPT,
    )?;
    deploy_script(&dir, "requirements.txt", REQUIREMENTS)?;

    info!("Conversion scripts deployed to {}", dir.display());
    Ok(())
}

fn deploy_script(dir: &Path, filename: &str, content: &str) -> Result<()> {
    let script_path = dir.join(filename);
    let hash_path = dir.join(format!("{}.hash", filename));
    let current_hash = content_hash(content);

    // Check if script is already up to date
    if script_path.exists() {
        if let Ok(stored_hash) = std::fs::read_to_string(&hash_path) {
            if stored_hash.trim() == current_hash {
                return Ok(());
            }
        }
    }

    std::fs::write(&script_path, content).with_path(&script_path)?;
    std::fs::write(&hash_path, &current_hash).with_path(&hash_path)?;
    Ok(())
}
