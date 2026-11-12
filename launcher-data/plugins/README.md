# Inference Plugin Manifests

These source-controlled JSON descriptors define the inference integrations
shipped with Pumas: Ollama, llama.cpp, ONNX Runtime, and Torch. They are
machine-consumed configuration, not user-installed desktop-app plugins.

The manifest separates presentation metadata, runtime installation strategy,
capabilities, connection details, model compatibility, and panel layout.
`installationType` is behavioral:

- `binary`: managed external executable;
- `python-venv`: managed Python environment and sidecar; and
- `in-process`: executed inside the Pumas Rust process. ONNX Runtime's native
  library is still a downloaded runtime with installed, active and default
  versions (`hasVersionManagement`).

These files are source defaults. A launcher root may contain copied or migrated
runtime state; changing the source descriptor does not by itself migrate every
existing installation.

When changing a descriptor, update and verify all consumers of its IDs,
provider names, capability fields, installation strategy, and panel types.
Test both the default inference-enabled build and the library-only build. A new
provider also requires coordinated core, RPC, frontend, packaging, and security
work; a manifest alone cannot establish support.

See [Architecture](../../docs/ARCHITECTURE.md) and
[ADR 0001](../../docs/adr/0001-onnx-runtime-provider-model.md).
