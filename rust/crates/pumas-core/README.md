# pumas-library

Headless library for AI model management, search, and HuggingFace integration.

## Quick Start

```toml
[dependencies]
pumas-library = "0.1"
```

## Usage

```rust
use pumas_library::{PumasApi, Result};

#[tokio::main]
async fn main() -> Result<()> {
    let api = PumasApi::builder("./my-models")
        .auto_create_dirs(true)
        .build()
        .await?;

    let models = api.list_models().await?;
    println!("Found {} models", models.len());
    Ok(())
}
```

## Builder Options

The builder pattern provides fine-grained control over initialization:

```rust
let api = PumasApi::builder("/path/to/root")
    .auto_create_dirs(true)   // Create directories if missing
    .with_hf_client(true)     // Enable HuggingFace integration
    .with_process_manager(true) // Enable process management
    .build()
    .await?;
```

## Feature Flags

| Feature | Default | Description |
|---------|---------|-------------|
| `full` | ✓ | Enable all features |
| `hf-client` | ✓ | HuggingFace model search & download |
| `process-manager` | ✓ | ComfyUI process management |
| `gpu-monitor` | ✓ | GPU monitoring |

Minimal build:
```bash
cargo add pumas-library --no-default-features
```

## Core Features

- **Model Library**: Index, search, and manage local AI models
- **HuggingFace Integration**: Search and download models from HuggingFace Hub
- **Model Mapping**: Link models to application directories with symlinks
- **Process Management**: Launch and monitor ComfyUI processes
- **System Utilities**: GPU monitoring, disk space, system resources

## License

MIT
