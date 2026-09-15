# Pumas Library v0.7.0

Pumas Library 0.7.0 expands the headless model-management core introduced in
0.6. Applications can discover, acquire, validate, retain, convert, and access
local AI models through a shared library service.

Optional inference integrations provide convenient local execution for
applications, development, and testing. Model management remains Pumas's primary
role; the gateway does not provide production scheduling, distributed inference,
GPU arbitration across tenants, autoscaling, or production request batching.

## Headless operation and model intent

The core and RPC backend can be built and run independently of Electron and the
frontend. Direct Cargo builds require no Node or Corepack; the convenience
launcher wrappers still require Node. GUI and inference integrations can be
selected independently.

The new **Model Intent API** lets an application describe what it needs and ask
Pumas to reconcile that requirement against the local library. It supports local
queries, model requests, optional upstream acquisition, acquisition status, and
durable consumer declarations. Requirements can constrain source, Hugging Face
repository, revision, artifact, format, and quantization. Upstream branch and tag
selectors are resolved to immutable commits before acquisition.

Durable declarations survive client disconnection and Pumas restarts. Different
consumers retain independent requirements, and declarations participate in
checks that protect models from destructive operations. Releasing a declaration
removes the consumer's requirement without deleting the model.

The RPC operations are `intent_query_models`, `intent_get_model`,
`intent_get_model_status`, `intent_ensure_model`, `intent_get_ensure_status`,
`intent_list_declarations`, and `intent_release_model`.

## Ownership, concurrency, and recovery

0.7 strengthens the explicit ownership model established in 0.6:
`PumasApi` and `PumasLibraryInstance` own a library root, while `PumasLocalClient`
connects to an existing owner and `PumasReadOnlyLibrary` provides direct read-only
access. A second owning instance fails when another live process owns the root.

Downloads and acquisition now have stronger root-level mutation exclusion,
durable ownership, restart reconciliation, cancellation, and shutdown handling.
Admitted work is retained by the backend across initiating-client disconnection.
Destructive-operation checks and explicit uncertain outcomes help preserve
recovery state when an operation is interrupted.

Hugging Face downloads also gain stricter destination validation, resumable
acquisition handling, and coordination with local metadata and recovery state.

## Backend-owned conversion

Applications can request conversion independently of the GUI. Pumas manages
conversion dependencies, setup state, process ownership, cancellation, output
publication, and incomplete-operation recovery.

Workflows include GGUF conversion through managed llama.cpp tooling and new FP8
and NVFP4 Safetensors outputs:

- **FP8:** compatible local Transformers models, using E4M3FN linear weights,
  block scaling, and BF16 embeddings and output weights where appropriate.
- **NVFP4:** managed NVIDIA Model Optimizer tooling, with packed E2M1 weights
  and associated scaling metadata.

Environment and root locking, readiness validation, and collision-safe output
publication protect ongoing conversions and existing model files.

## Optional local inference

The Pumas gateway exposes provider capabilities and live endpoints through RPC.
Backend state distinguishes installed runtimes, runtime profiles, model routes,
loading models, ready served instances, and their available capabilities.
Model-library, download, runtime-profile, serving, and telemetry events keep
clients informed without making the desktop UI the state owner.

### ONNX Runtime embeddings

The in-process ONNX Runtime provider loads supported models, tokenizes input,
executes inference, post-processes embeddings, and unloads sessions. Embedding
models are available through the gateway without a Python sidecar.

### Torch image generation

The optional Torch integration adds adapters for Nunchaku Z-Image-Turbo and
FLUX.2 Klein. Ready models advertise `image_generation`; bounded single-image
requests use supported output dimensions and return PNG image data.

The integration manages model components and runtime dependencies, with hardware
telemetry, per-device operation ownership, cancellation, and unload protection
while generation is active. These adapters require a compatible managed runtime
and device; this does not establish support for every desktop platform or GPU.

### llama.cpp reliability

Managed llama.cpp serving gains library/router reconciliation, clearer loading
state, explicit start/stop controls, model-specific context configuration,
readiness checks, stronger process identity and shutdown ownership, and automatic
discovery of compatible sibling `mmproj` vision projectors.

## Integration and upgrade notes

Windows supports download/library operations and durable metadata publication.
The release candidate requires native Windows tests and startup checks for both
the installed application and portable executable, alongside Linux and macOS.

- Use the `pumas-library` Rust crate in `pumas-core` for direct integration, or
  `pumas-rpc` when process separation is useful. Language-binding adapters remain
  in the source tree, but no host-binding release bundle is currently supported.
- Applications still relying on older automatic owner/client selection should
  choose `PumasApi` / `PumasLibraryInstance`, `PumasLocalClient`, or
  `PumasReadOnlyLibrary` explicitly.
- Consider the Intent API when a model must remain available across restarts or
  for multiple independent consumers.
- The package-facts contract is now version 2, with structured Diffusers,
  image-family, GGUF, value-source, and inspection-manifest data. Update consumers
  that decode those DTOs directly.
- Inference integrations remain optional for applications that only need model
  management.

### Optional ONNX build dependency

Rust consumers using `default-features = false` can now omit ONNX Runtime,
tokenizers and half-precision tensor dependencies. Enable the core
`onnx-runtime` feature to use its ONNX execution APIs. Default core builds and
the RPC `inference-plugins` feature continue to include ONNX support.

## Outside this release

Fleet/cluster operation, remote-node reconciliation, distributed acquisition,
network-wide capability discovery, MCP integration, shared/community Hugging Face
caches, and production inference orchestration remain deferred.
